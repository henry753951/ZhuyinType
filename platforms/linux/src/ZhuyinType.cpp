#include "ZhuyinType.h"
#include "Keys.h"
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h>
#include <fcitx/surroundingtext.h>
#include <unistd.h>
#include <utility>

namespace zhuyintype {
namespace {
bool protectedField(fcitx::CapabilityFlags flags) {
    return flags.testAny(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Password, fcitx::CapabilityFlag::Disable});
}
// Sensitive data (a private window, Qt's ImhSensitiveData, GTK's private hint): typing works, nothing is learned or
// read (IPC.md `private`).
bool privateField(fcitx::CapabilityFlags flags) {
    return flags.test(fcitx::CapabilityFlag::Sensitive);
}
nlohmann::json geometry(fcitx::InputContext &ic) {
    const auto &r = ic.cursorRect();
    return {{"rect", {r.left(), r.top(), r.width(), r.height()}}, {"scale", 1}};
}
// 前文 (IPC.md): the program and, where the field reports it, up to 255 characters before the cursor.  Never for a
// protected field.
void addContext(fcitx::InputContext &ic, nlohmann::json &fields) {
    fields["app"] = ic.program().substr(0, 256);
    if (!ic.capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText)) return;
    const auto &surrounding = ic.surroundingText();
    if (!surrounding.isValid()) return;
    const std::string &text = surrounding.text();
    // The cursor counts characters; walk the UTF-8 to its byte offset.
    size_t offset = 0;
    for (unsigned int characters = 0; offset < text.size() && characters < surrounding.cursor(); ++characters) {
        ++offset;
        while (offset < text.size() && (static_cast<unsigned char>(text[offset]) & 0xC0) == 0x80) ++offset;
    }
    size_t start = offset > 765 ? offset - 765 : 0;
    while (start < offset && (static_cast<unsigned char>(text[start]) & 0xC0) == 0x80) ++start;
    fields["before"] = text.substr(start, offset - start);
}
// The words of the composition (IPC.md); left out by older hosts, ignored when malformed.
std::vector<std::pair<size_t, size_t>> clauses(const nlohmann::json &response) {
    std::vector<std::pair<size_t, size_t>> result;
    const auto it = response.find("clauses");
    if (it == response.end() || !it->is_array()) return result;
    for (const auto &clause : *it) {
        if (!clause.is_array() || clause.size() != 2 || !clause[0].is_number_unsigned() ||
            !clause[1].is_number_unsigned()) return {};
        result.emplace_back(clause[0].get<size_t>(), clause[1].get<size_t>());
    }
    return result;
}
std::optional<size_t> index(const nlohmann::json &response, const char *key) {
    const auto it = response.find(key);
    if (it == response.end() || !it->is_number_unsigned()) return std::nullopt;
    return it->get<size_t>();
}
}
// A fresh focus epoch gets a fresh context token. Reconnection never replays input.
struct ZhuyinType::State final : fcitx::InputContextProperty {
    std::string context;
    std::string preedit;
    bool secure = false;
    bool capsSynced = false;
    bool inlinePreedit = true;
    bool english = false;
    // The user switched to ZhuyinType from another input method: the next focus says so (IPC `activated`).
    bool activated = false;
};
ZhuyinType::ZhuyinType(fcitx::Instance &instance)
    : instance_(instance), factory_([](fcitx::InputContext &) { return new State; }) {
    if (!instance.inputContextManager().registerProperty("zhuyintype", &factory_))
        throw std::runtime_error("無法登記 ZhuyinType 組字狀態。");
    capabilityWatcher_ = instance.watchEvent(fcitx::EventType::InputContextCapabilityAboutToChange,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
            auto &change = static_cast<fcitx::CapabilityAboutToChangeEvent &>(event);
            auto &ic = *change.inputContext();
            if (active_.get() != &ic) return;
            auto &state = *ic.propertyFor(&factory_);
            const bool inlinePreedit = change.newFlags().test(fcitx::CapabilityFlag::Preedit);
            const bool inlineChanged = state.inlinePreedit != inlinePreedit;
            state.inlinePreedit = inlinePreedit;
            secure(ic, state, protectedField(change.newFlags()));
            if (inlineChanged && broker_.connected()) {
                auto fields = geometry(ic);
                fields.update({{"secure", state.secure}, {"inlinePreedit", state.inlinePreedit}});
                exchange(ic, state, "focus", fields);
            }
        });
    destroyWatcher_ = instance.watchEvent(fcitx::EventType::InputContextDestroyed,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
            auto &destroy = static_cast<fcitx::InputContextEvent &>(event);
            if (active_.get() != destroy.inputContext()) return;
            destroy.inputContext()->propertyFor(&factory_)->preedit.clear();
            broker_.disconnect(); // Host discards the destroyed context on socket close.
            active_.unwatch();
            timer_->setEnabled(false);
        });
    const std::array<std::pair<const char *, const char *>, 5> menu{{{"zhuyintype-settings", "ZhuyinType 設定…"},
        {"zhuyintype-words", "自訂詞與學到的詞…"}, {"zhuyintype-wizard", "設定嚮導…"},
        {"zhuyintype-tour", "新功能導覽…"}, {"zhuyintype-about", "關於 ZhuyinType"}}};
    const std::array<const char *, 5> places{nullptr, "words", "wizard", "tour", "about"};
    for (size_t i = 0; i < menu.size(); ++i) {
        actions_[i] = std::make_unique<fcitx::SimpleAction>();
        actions_[i]->setShortText(menu[i].second);
        const char *place = places[i];
        actions_[i]->connect<fcitx::SimpleAction::Activated>([this, place](fcitx::InputContext *) { showSettings(place); });
        instance.userInterfaceManager().registerAction(menu[i].first, actions_[i].get());
    }
    timer_ = instance.eventLoop().addTimeEvent(CLOCK_MONOTONIC, 0, 1000,
        [this](fcitx::EventSourceTime *source, uint64_t) {
            poll();
            source->setNextInterval(20000);
            source->setEnabled(active_.get() && broker_.connected());
            return true;
        });
    timer_->setEnabled(false);
}
ZhuyinType::~ZhuyinType() = default;
void ZhuyinType::clear(fcitx::InputContext &ic) {
    ic.inputPanel().reset();
    ic.updatePreedit();
    ic.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}
void ZhuyinType::recover(fcitx::InputContext &ic, State &state) {
    const auto known = std::exchange(state.preedit, std::string());
    if (active_.get() == &ic && ic.hasFocus() && !state.secure &&
        !protectedField(ic.capabilityFlags()) && !known.empty()) ic.commitString(known);
    clear(ic);
}
bool ZhuyinType::exchange(fcitx::InputContext &ic, State &state, const char *op, nlohmann::json fields,
                          std::string_view delivered) {
    const auto response = broker_.request(state.context, op, std::move(fields));
    if (!response) { recover(ic, state); timer_->setEnabled(false); return false; }
    // Synchronous exchange and tracked focus bind every result to its originating context.
    if (active_.get() != &ic || state.secure) { clear(ic); return false; }
    if (const bool english = (*response)["english"].get<bool>(); english != state.english) {
        state.english = english;
        ic.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
    }
    const auto &preedit = (*response)["preedit"].get_ref<const std::string &>();
    state.preedit = preedit;
    if (const auto commit = undelivered((*response)["commit"].get_ref<const std::string &>(), delivered); !commit.empty())
        ic.commitString(std::string(commit));
    auto &panel = ic.inputPanel();
    panel.reset();
    if (state.inlinePreedit) {
        fcitx::Text text;
        for (auto &part : preeditParts(preedit, clauses(*response), index(*response, "focus"), index(*response, "target"))) {
            fcitx::TextFormatFlags format{fcitx::TextFormatFlag::Underline};
            if (part.mark == Mark::Focus) format |= fcitx::TextFormatFlag::HighLight;
            if (part.mark == Mark::Target) format |= fcitx::TextFormatFlag::Bold;
            text.append(std::move(part.text), format);
        }
        text.setCursor(*cursorBytes(preedit, (*response)["cursor"].get<size_t>()));
        panel.setClientPreedit(text);
    }
    ic.updatePreedit();
    ic.updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    return (*response)["handled"].get<bool>();
}
bool ZhuyinType::focus(fcitx::InputContext &ic, State &state) {
    if (auto *old = active_.get(); old && broker_.connected()) blur(*old, *old->propertyFor(&factory_));
    active_ = ic.watch();
    state.capsSynced = false;
    state.preedit.clear();
    state.secure = protectedField(ic.capabilityFlags());
    state.inlinePreedit = ic.capabilityFlags().test(fcitx::CapabilityFlag::Preedit);
    static uint64_t serial = 0;
    state.context = "fcitx-" + std::to_string(getpid()) + "-" + std::to_string(++serial);
    if (!broker_.connect()) { Broker::launchHost(); clear(ic); return false; }
    auto fields = geometry(ic);
    const bool isPrivate = privateField(ic.capabilityFlags());
    fields.update({{"secure", state.secure}, {"private", isPrivate}, {"inlinePreedit", state.inlinePreedit}});
    if (std::exchange(state.activated, false)) fields["activated"] = true;
    if (!state.secure && !isPrivate) addContext(ic, fields);
    exchange(ic, state, "focus", fields);
    timer_->setNextInterval(20000);
    timer_->setEnabled(broker_.connected() && ic.hasFocus());
    return broker_.connected();
}
void ZhuyinType::secure(fcitx::InputContext &ic, State &state, bool next) {
    if (next == state.secure) return;
    state.secure = next;
    state.preedit.clear();
    state.capsSynced = false; // Lock keys are deliberately not forwarded while protected.
    exchange(ic, state, "secure", {{"secure", next}});
    clear(ic);
}
void ZhuyinType::blur(fcitx::InputContext &ic, State &state, std::string_view delivered) {
    exchange(ic, state, "blur", nlohmann::json::object(), delivered);
    clear(ic);
    state.context.clear();
    state.preedit.clear();
    active_.unwatch();
    timer_->setEnabled(false);
}
void ZhuyinType::activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto &ic = *event.inputContext();
    auto &state = *ic.propertyFor(&factory_);
    // Picked from another input method (not focus moving between fields): 中/英 starts as the setting says.
    if (event.type() == fcitx::EventType::InputContextSwitchInputMethod) state.activated = true;
    for (auto &action : actions_) ic.statusArea().addAction(fcitx::StatusGroup::InputMethod, action.get());
    focus(ic, state);
}
std::string ZhuyinType::subMode(const fcitx::InputMethodEntry &, fcitx::InputContext &ic) {
    return ic.propertyFor(&factory_)->english ? "英文" : "中文";
}
std::string ZhuyinType::subModeLabelImpl(const fcitx::InputMethodEntry &, fcitx::InputContext &ic) {
    return ic.propertyFor(&factory_)->english ? "英" : "中";
}
void ZhuyinType::showSettings(const char *place) {
    nlohmann::json fields = nlohmann::json::object();
    if (place) fields["place"] = place;
    if (!broker_.connected() && !broker_.connect()) { Broker::launchHost(); return; }
    broker_.request("settings", "show-settings", fields);
}
void ZhuyinType::deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto &ic = *event.inputContext();
    if (active_.get() != &ic) return;
    auto &state = *ic.propertyFor(&factory_);
    const bool capabilityChange = event.type() == fcitx::EventType::InputContextSwitchInputMethod &&
        static_cast<fcitx::InputContextSwitchInputMethodEvent &>(event).reason() == fcitx::InputMethodSwitchedReason::CapabilityChanged;
    // Fcitx deactivates before exposing new password capability flags.
    secure(ic, state, capabilityChange || protectedField(ic.capabilityFlags()));
    // When focus leaves, the composition shown in the field is already committed there: by Fcitx itself, or by the
    // application's Fcitx module (ClientUnfocusCommit: GTK, Qt, Wayland) before it reported the focus change.  The
    // host commits it on blur as well; sending that again would type it twice, or into the field focus moved to.
    std::string delivered;
    if (event.type() == fcitx::EventType::InputContextFocusOut) delivered = ic.inputPanel().clientPreedit().toStringForCommit();
    blur(ic, state, delivered);
}
void ZhuyinType::reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto &ic = *event.inputContext();
    auto &state = *ic.propertyFor(&factory_);
    state.preedit.clear();
    if (active_.get() == &ic) exchange(ic, state, "reset");
    clear(ic);
}
void ZhuyinType::keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) {
    auto &ic = *event.inputContext();
    if (!ic.hasFocus()) return;
    auto &state = *ic.propertyFor(&factory_);
    if ((active_.get() != &ic || !broker_.connected()) && !focus(ic, state)) return;
    secure(ic, state, protectedField(ic.capabilityFlags()));
    if (state.secure) return; // Never serialize keys or text from protected fields.
    const auto key = translateKey(event.rawKey());
    if (!key) {
        exchange(ic, state, "blur");
        auto fields = geometry(ic);
        fields.update({{"secure", false}, {"inlinePreedit", state.inlinePreedit}});
        exchange(ic, state, "focus", fields);
        return;
    }
    if (!state.capsSynced && key->key != 0x01000024) {
        auto initial = geometry(ic);
        initial.update({{"secure", false}, {"inlinePreedit", state.inlinePreedit}, {"capsLock", event.rawKey().states().test(fcitx::KeyState::CapsLock)}});
        exchange(ic, state, "focus", initial);
        if (!broker_.connected()) return;
    }
    state.capsSynced = true; // CapsLock key events toggle in the host; do not also synchronize them.
    auto fields = geometry(ic);
    fields.update({{"key", key->key}, {"modifiers", key->modifiers}, {"shiftSide", key->shiftSide},
        {"release", event.isRelease()}, {"repeat", event.rawKey().states().test(fcitx::KeyState::Repeat)}, {"secure", false},
        {"private", privateField(ic.capabilityFlags())}});
    if (!event.isRelease() && state.preedit.empty() && !privateField(ic.capabilityFlags())) addContext(ic, fields);
    if (exchange(ic, state, "key", fields)) event.filterAndAccept();
}
void ZhuyinType::poll() {
    auto *ic = active_.get();
    if (!ic) { broker_.disconnect(); timer_->setEnabled(false); return; }
    if (!ic->hasFocus()) { blur(*ic, *ic->propertyFor(&factory_)); return; }
    if (!broker_.connected()) { timer_->setEnabled(false); return; }
    auto &state = *ic->propertyFor(&factory_);
    secure(*ic, state, protectedField(ic->capabilityFlags()));
    exchange(*ic, state, "poll", geometry(*ic));
}
}

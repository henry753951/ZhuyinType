#include "ZhuyinType.h"
#include "Keys.h"
#include <fcitx-utils/utf8.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h>
#include <fcitx/surroundingtext.h>
#include <cstdlib>
#include <functional>
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
// UTF-16 ranges of the composition (IPC.md): its words (`clauses`), the words shown through an automatic correction
// (`corrections`).  Left out by older hosts, ignored when malformed.
std::vector<std::pair<size_t, size_t>> ranges(const nlohmann::json &response, const char *key) {
    std::vector<std::pair<size_t, size_t>> result;
    const auto it = response.find(key);
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
// Fcitx draws the candidate list (IPC `panel: true`) where the app's own window cannot follow the caret: in a Wayland
// session no client may place its own windows (the app's window would be an ordinary window wherever the compositor
// puts it), and a Wayland field reports its caret relative to its own window.  ZT_FCITX_PANEL=1 or 0 decides (tests).
bool fcitxDrawsPanel(fcitx::InputContext &ic) {
    if (const char *forced = std::getenv("ZT_FCITX_PANEL"); forced && *forced) return std::string_view(forced) == "1";
    if (ic.display().starts_with("wayland")) return true;
    const char *wayland = std::getenv("WAYLAND_DISPLAY");
    return wayland && *wayland;
}
std::string text(const nlohmann::json &object, const char *key) {
    const auto it = object.find(key);
    return it != object.end() && it->is_string() && fcitx::utf8::validate(it->get_ref<const std::string &>())
        ? it->get<std::string>() : std::string();
}
size_t number(const nlohmann::json &object, const char *key) {
    const auto it = object.find(key);
    return it != object.end() && it->is_number_unsigned() ? it->get<size_t>() : 0;
}
class PanelWord final : public fcitx::CandidateWord {
public:
    PanelWord(fcitx::Text text, fcitx::Text comment, std::function<void(fcitx::InputContext *)> pick)
        : fcitx::CandidateWord(std::move(text)), pick_(std::move(pick)) { setComment(std::move(comment)); }
    void select(fcitx::InputContext *ic) const override { pick_(ic); }
private:
    std::function<void(fcitx::InputContext *)> pick_;
};
// One page of the host's list; paging asks the host (the list is rebuilt from its reply).
class PanelList final : public fcitx::CandidateList, public fcitx::PageableCandidateList {
public:
    PanelList(fcitx::CandidateLayoutHint hint, int cursor, int page, int pages, std::function<void(int)> turn)
        : hint_(hint), cursor_(cursor), page_(page), pages_(pages), turn_(std::move(turn)) { setPageable(this); }
    void add(fcitx::Text label, std::unique_ptr<PanelWord> word) {
        labels_.push_back(std::move(label));
        words_.push_back(std::move(word));
    }
    const fcitx::Text &label(int idx) const override { return labels_.at(static_cast<size_t>(idx)); }
    const fcitx::CandidateWord &candidate(int idx) const override { return *words_.at(static_cast<size_t>(idx)); }
    int size() const override { return static_cast<int>(words_.size()); }
    int cursorIndex() const override { return cursor_; }
    fcitx::CandidateLayoutHint layoutHint() const override { return hint_; }
    bool hasPrev() const override { return page_ > 0; }
    bool hasNext() const override { return page_ + 1 < pages_; }
    void prev() override { turn_(-1); }
    void next() override { turn_(1); }
    bool usedNextBefore() const override { return page_ > 0; }
    int totalPages() const override { return pages_; }
    int currentPage() const override { return page_; }
private:
    fcitx::CandidateLayoutHint hint_;
    int cursor_, page_, pages_;
    std::function<void(int)> turn_;
    std::vector<fcitx::Text> labels_;
    std::vector<std::unique_ptr<PanelWord>> words_;
};
}
// A fresh focus epoch gets a fresh context token. Reconnection never replays input.
struct ZhuyinType::State final : fcitx::InputContextProperty {
    std::string context;
    std::string preedit;
    bool secure = false;
    bool capsSynced = false;
    bool inlinePreedit = true;
    bool english = false;
    // 快速設定: 簡體 output, 暫停學習 and 中文標點 width, as every reply reports them.
    bool simplified = false;
    bool learningPaused = false;
    bool fullWidth = true;
    // The user switched to ZhuyinType from another input method: the next focus says so (IPC `activated`).
    bool activated = false;
    // Fcitx draws the candidate list (fcitxDrawsPanel); `panel` is the host's last one (null: hidden).
    bool drawsPanel = false;
    nlohmann::json panel;
    // The host shows its candidate window, where a click can change the composition: poll for it.
    bool polls = false;
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
            if (inlineChanged && broker_.connected()) exchange(ic, state, "focus", focusFields(ic, state));
        });
    destroyWatcher_ = instance.watchEvent(fcitx::EventType::InputContextDestroyed,
        fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
            auto &destroy = static_cast<fcitx::InputContextEvent &>(event);
            if (active_.get() != destroy.inputContext()) return;
            destroy.inputContext()->propertyFor(&factory_)->preedit.clear();
            broker_.disconnect(); // Host discards the destroyed context on socket close.
            watchConnection();
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
    const std::array<std::pair<const char *, const char *>, 4> quickItems{{{"zhuyintype-quick-english", "英文"},
        {"zhuyintype-quick-half-width", "半形標點"}, {"zhuyintype-quick-simplified", "簡體"},
        {"zhuyintype-quick-pause-learning", "暫停學習"}}};
    for (size_t i = 0; i < quickItems.size(); ++i) {
        quick_[i] = std::make_unique<fcitx::SimpleAction>();
        quick_[i]->setShortText(quickItems[i].second);
        quick_[i]->setCheckable(true);
        const char *item = quickItems[i].first;
        quick_[i]->connect<fcitx::SimpleAction::Activated>([this, item](fcitx::InputContext *ic) { quick(ic, item); });
        instance.userInterfaceManager().registerAction(item, quick_[i].get());
    }
    quickSeparator_ = std::make_unique<fcitx::SimpleAction>();
    quickSeparator_->setSeparator(true);
    instance.userInterfaceManager().registerAction("zhuyintype-quick-separator", quickSeparator_.get());
    timer_ = instance.eventLoop().addTimeEvent(CLOCK_MONOTONIC, 0, 1000,
        [this](fcitx::EventSourceTime *source, uint64_t) {
            poll();
            source->setNextInterval(20000);
            auto *ic = active_.get();
            source->setEnabled(ic && broker_.connected() && ic->propertyFor(&factory_)->polls);
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
    watchConnection();
    if (!response) { recover(ic, state); timer_->setEnabled(false); return false; }
    // Synchronous exchange and tracked focus bind every result to its originating context.
    if (active_.get() != &ic || state.secure) { state.polls = false; clear(ic); return false; }
    // Only a click in the host's own window changes anything between keys (IPC `poll`): no wake-ups otherwise.
    // A screen reader runs: the host asks Fcitx to show the list, which screen readers read (IPC `fcitxPanel`).
    if (const auto it = response->find("fcitxPanel"); it != response->end() && it->is_boolean() && it->get<bool>())
        state.drawsPanel = true;
    state.polls = (*response)["candidates"].get<bool>() && !state.drawsPanel;
    if (state.polls && !timer_->isEnabled()) {
        timer_->setNextInterval(20000);
        timer_->setEnabled(true);
    } else if (!state.polls) {
        timer_->setEnabled(false);
    }
    // Older hosts leave the 快速設定 fields out.
    const auto flag = [&](const char *key, bool fallback) {
        const auto it = response->find(key);
        return it != response->end() && it->is_boolean() ? it->get<bool>() : fallback;
    };
    const bool english = (*response)["english"].get<bool>();
    const bool simplified = flag("simplified", false), paused = flag("learningPaused", false);
    const bool fullWidth = flag("fullWidth", state.fullWidth);
    if (english != state.english || simplified != state.simplified || paused != state.learningPaused ||
        fullWidth != state.fullWidth) {
        state.english = english;
        state.simplified = simplified;
        state.learningPaused = paused;
        state.fullWidth = fullWidth;
        syncQuick(state);
        ic.updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
    }
    if (state.drawsPanel) {
        // Left out: unchanged since the last reply.
        if (const auto it = response->find("panel"); it != response->end() && (it->is_object() || it->is_null()))
            state.panel = *it;
        // The 中/英 badge (「中英提示」).
        if (const auto toast = text(*response, "toast"); !toast.empty()) instance_.showCustomInputMethodInformation(&ic, toast);
    }
    const auto &preedit = (*response)["preedit"].get_ref<const std::string &>();
    state.preedit = preedit;
    if (const auto commit = undelivered((*response)["commit"].get_ref<const std::string &>(), delivered); !commit.empty())
        ic.commitString(std::string(commit));
    auto &panel = ic.inputPanel();
    panel.reset();
    if (state.inlinePreedit) {
        fcitx::Text text;
        for (auto &part : preeditParts(preedit, ranges(*response, "clauses"), index(*response, "focus"),
                                       index(*response, "target"), ranges(*response, "corrections"))) {
            fcitx::TextFormatFlags format{fcitx::TextFormatFlag::Underline};
            if (part.mark == Mark::Focus) format |= fcitx::TextFormatFlag::HighLight;
            if (part.mark == Mark::Target) format |= fcitx::TextFormatFlag::Bold;
            // 「標示自動修正」: Fcitx has no dotted underline; italic is the lightest mark GTK and Qt clients draw that
            // is not already taken (bold: `target`, highlight: `focus`) and keeps the word's underline.
            if (part.corrected) format |= fcitx::TextFormatFlag::Italic;
            text.append(std::move(part.text), format);
        }
        text.setCursor(*cursorBytes(preedit, (*response)["cursor"].get<size_t>()));
        panel.setClientPreedit(text);
    }
    if (state.drawsPanel) showPanel(ic, state);
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
    state.drawsPanel = fcitxDrawsPanel(ic);
    state.panel = nullptr;
    static uint64_t serial = 0;
    state.context = "fcitx-" + std::to_string(getpid()) + "-" + std::to_string(++serial);
    if (!broker_.connect()) { Broker::launchHost(); clear(ic); return false; }
    auto fields = focusFields(ic, state);
    if (std::exchange(state.activated, false)) fields["activated"] = true;
    if (!state.secure && !privateField(ic.capabilityFlags())) addContext(ic, fields);
    exchange(ic, state, "focus", fields);
    return broker_.connected();
}
void ZhuyinType::secure(fcitx::InputContext &ic, State &state, bool next) {
    if (next == state.secure) return;
    state.secure = next;
    state.preedit.clear();
    state.capsSynced = false; // Lock keys are deliberately not forwarded while protected.
    exchange(ic, state, "secure", {{"secure", next}, {"private", privateField(ic.capabilityFlags())}});
    clear(ic);
}
nlohmann::json ZhuyinType::focusFields(fcitx::InputContext &ic, const State &state) const {
    auto fields = geometry(ic);
    fields.update({{"secure", state.secure}, {"private", privateField(ic.capabilityFlags())},
        {"inlinePreedit", state.inlinePreedit}, {"panel", state.drawsPanel}});
    return fields;
}
void ZhuyinType::showPanel(fcitx::InputContext &ic, const State &state) {
    if (!state.panel.is_object() || !state.panel.contains("model") || !state.panel["model"].is_object()) return;
    const auto &model = state.panel["model"];
    auto &panel = ic.inputPanel();
    // The composition, for a field that cannot show it (IPC `preview`).
    if (const auto preview = text(model, "preview"); !preview.empty()) panel.setPreedit(fcitx::Text(preview));
    if (const auto reading = text(model, "reading"); !reading.empty()) panel.setAuxUp(fcitx::Text(reading));
    // 聯想詞 (next-word suggestions after a commit) have no reading: the label says what the list is.
    if (const auto associated = model.find("associated"); associated != model.end() && associated->is_boolean() &&
        associated->get<bool>()) panel.setAuxUp(fcitx::Text("聯想詞・↓ 選擇"));
    if (const auto notice = text(model, "notice"); !notice.empty()) panel.setAuxDown(fcitx::Text(notice));
    // 「你打的是 ㄊㄜˇ · ⌫ 還原」 for a span shown through an automatic correction (a notice comes first).
    else if (const auto typed = text(model, "typed"); !typed.empty()) panel.setAuxDown(fcitx::Text(typed));
    const auto rows = model.find("rows");
    if (rows == model.end() || !rows->is_array() || rows->empty()) return;
    const bool horizontal = state.panel.contains("style") && state.panel["style"].is_object() &&
        text(state.panel["style"], "layout") == "horizontal";
    const auto selected = model.find("selected");
    const int cursor = selected != model.end() && selected->is_number_unsigned() ? selected->get<int>() : -1;
    const auto watched = ic.watch();
    auto list = std::make_unique<PanelList>(
        horizontal ? fcitx::CandidateLayoutHint::Horizontal : fcitx::CandidateLayoutHint::Vertical, cursor,
        static_cast<int>(number(model, "page")), static_cast<int>(number(model, "page_count")),
        [this, watched](int delta) { if (auto *ic = watched.get()) action(*ic, "page", delta); });
    int row = 0;
    for (const auto &item : *rows) {
        if (!item.is_object()) continue;
        const auto key = text(item, "key");
        // A correction's reading as the candidate's comment.
        auto word = std::make_unique<PanelWord>(fcitx::Text(text(item, "text")), fcitx::Text(text(item, "annotation")),
            [this, row](fcitx::InputContext *ic) { if (ic) action(*ic, "pick", row); });
        list->add(fcitx::Text(key.empty() ? std::string() : key + ". "), std::move(word));
        ++row;
    }
    panel.setCandidateList(std::move(list));
}
void ZhuyinType::action(fcitx::InputContext &ic, const char *kind, int value) {
    auto &state = *ic.propertyFor(&factory_);
    if (active_.get() != &ic || !broker_.connected() || state.secure) return;
    exchange(ic, state, "action", {{"action", kind}, {"value", value}});
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
    for (auto &action : quick_) ic.statusArea().addAction(fcitx::StatusGroup::InputMethod, action.get());
    ic.statusArea().addAction(fcitx::StatusGroup::InputMethod, quickSeparator_.get());
    for (auto &action : actions_) ic.statusArea().addAction(fcitx::StatusGroup::InputMethod, action.get());
    syncQuick(state);
    focus(ic, state);
}
std::string ZhuyinType::subMode(const fcitx::InputMethodEntry &, fcitx::InputContext &ic) {
    const auto &state = *ic.propertyFor(&factory_);
    const std::string mode = state.english ? "英文" : state.simplified ? "簡體中文" : "中文";
    return state.learningPaused ? mode + "（暫停學習）" : mode;
}
std::string ZhuyinType::subModeLabelImpl(const fcitx::InputMethodEntry &, fcitx::InputContext &ic) {
    const auto &state = *ic.propertyFor(&factory_);
    return state.english ? "英" : state.simplified ? "简" : "中";
}
void ZhuyinType::syncQuick(const State &state) {
    quick_[0]->setChecked(state.english);
    quick_[1]->setChecked(!state.fullWidth);
    quick_[2]->setChecked(state.simplified);
    quick_[3]->setChecked(state.learningPaused);
}
void ZhuyinType::quick(fcitx::InputContext *ic, const char *item) {
    if (!ic) ic = active_.get();
    if (!ic || active_.get() != ic || !broker_.connected()) return;
    auto &state = *ic->propertyFor(&factory_);
    if (state.secure) return;
    const std::string_view name(item);
    nlohmann::json fields = nlohmann::json::object();
    if (name == "zhuyintype-quick-english") fields["english"] = !state.english;
    else if (name == "zhuyintype-quick-half-width") fields["fullWidth"] = !state.fullWidth;
    else if (name == "zhuyintype-quick-simplified") fields["simplified"] = !state.simplified;
    else fields["learningPaused"] = !state.learningPaused;
    exchange(*ic, state, "quick", fields);
    syncQuick(state);
    ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}
void ZhuyinType::showSettings(const char *place) {
    nlohmann::json fields = nlohmann::json::object();
    if (place) fields["place"] = place;
    if (!broker_.connected() && !broker_.connect()) { Broker::launchHost(); return; }
    broker_.request("settings", "show-settings", fields);
    watchConnection();
}
void ZhuyinType::watchConnection() {
    if (!broker_.connected()) { hangup_.reset(); watched_ = 0; return; }
    if (hangup_ && watched_ == broker_.generation()) return;
    watched_ = broker_.generation();
    hangup_.reset(); // Before the new socket is watched: it may have the old one's number.
    hangup_ = instance_.eventLoop().addIOEvent(broker_.fd(),
        fcitx::IOEventFlags{fcitx::IOEventFlag::In, fcitx::IOEventFlag::Hup, fcitx::IOEventFlag::Err},
        [this](fcitx::EventSourceIO *source, int, fcitx::IOEventFlags) {
            // As a failed request: the field keeps the composition it showed, once (recover).
            source->setEnabled(false);
            broker_.disconnect();
            timer_->setEnabled(false);
            if (auto *ic = active_.get()) recover(*ic, *ic->propertyFor(&factory_));
            return true;
        });
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
        exchange(ic, state, "focus", focusFields(ic, state));
        return;
    }
    if (!state.capsSynced && key->key != 0x01000024) {
        auto initial = focusFields(ic, state);
        initial["capsLock"] = event.rawKey().states().test(fcitx::KeyState::CapsLock);
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
    if (!ic) { broker_.disconnect(); watchConnection(); timer_->setEnabled(false); return; }
    if (!ic->hasFocus()) { blur(*ic, *ic->propertyFor(&factory_)); return; }
    if (!broker_.connected()) { timer_->setEnabled(false); return; }
    auto &state = *ic->propertyFor(&factory_);
    secure(*ic, state, protectedField(ic->capabilityFlags()));
    exchange(*ic, state, "poll", geometry(*ic));
}
}

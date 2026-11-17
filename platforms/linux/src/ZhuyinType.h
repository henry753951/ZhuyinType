#pragma once
#include "Broker.h"
#include <fcitx-utils/event.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/action.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <array>
#include <memory>
#include <string_view>

namespace zhuyintype {
class ZhuyinType final : public fcitx::InputMethodEngineV2 {
public:
    explicit ZhuyinType(fcitx::Instance &instance);
    ~ZhuyinType() override;
    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &) override;
    // 中 or 英 in Fcitx's status area and indicator (the macOS menu bar and Windows taskbar show it the same way).
    std::string subMode(const fcitx::InputMethodEntry &, fcitx::InputContext &) override;
    std::string subModeLabelImpl(const fcitx::InputMethodEntry &, fcitx::InputContext &) override;
private:
    struct State;
    fcitx::Instance &instance_;
    Broker broker_;
    // The input method's menu in Fcitx (the macOS input menu): settings, words, the wizard, the tour, about.
    std::array<std::unique_ptr<fcitx::SimpleAction>, 5> actions_;
    void showSettings(const char *place);
    fcitx::FactoryFor<State> factory_;
    fcitx::TrackableObjectReference<fcitx::InputContext> active_;
    std::unique_ptr<fcitx::EventSourceTime> timer_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> capabilityWatcher_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> destroyWatcher_;
    bool focus(fcitx::InputContext &, State &);
    void secure(fcitx::InputContext &, State &, bool);
    // `delivered`: the composition Fcitx or the application already committed when focus left (see deactivate).
    void blur(fcitx::InputContext &, State &, std::string_view delivered = {});
    void clear(fcitx::InputContext &);
    void recover(fcitx::InputContext &, State &);
    bool exchange(fcitx::InputContext &, State &, const char *, nlohmann::json = nlohmann::json::object(),
                  std::string_view delivered = {});
    // What every `focus` says about the field: caret, secure, private, inline composition, who draws the list.
    nlohmann::json focusFields(fcitx::InputContext &, const State &) const;
    // The host's candidate window (IPC `panel`) as Fcitx's own candidate list, where Fcitx draws it.
    void showPanel(fcitx::InputContext &, const State &);
    // A click in Fcitx's candidate list (IPC `action`): `pick` a row, or `page` by -1 or 1.
    void action(fcitx::InputContext &, const char *, int);
    void poll();
};
}

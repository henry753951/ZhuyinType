#pragma once
#include "Broker.h"
#include <fcitx-utils/event.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>

namespace zhuyintype {
class ZhuyinType final : public fcitx::InputMethodEngine {
public:
    explicit ZhuyinType(fcitx::Instance &instance);
    ~ZhuyinType() override;
    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override;
    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &) override;
private:
    struct State;
    Broker broker_;
    fcitx::FactoryFor<State> factory_;
    fcitx::TrackableObjectReference<fcitx::InputContext> active_;
    std::unique_ptr<fcitx::EventSourceTime> timer_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> capabilityWatcher_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> destroyWatcher_;
    bool focus(fcitx::InputContext &, State &);
    void secure(fcitx::InputContext &, State &, bool);
    void blur(fcitx::InputContext &, State &);
    void clear(fcitx::InputContext &);
    void recover(fcitx::InputContext &, State &);
    bool exchange(fcitx::InputContext &, State &, const char *, nlohmann::json = nlohmann::json::object());
    void poll();
};
}

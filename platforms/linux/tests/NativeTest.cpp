#include "ZhuyinType.h"
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <iostream>
#include <chrono>
#include <algorithm>
#include <cstdlib>
#include <stdexcept>

#define CHECK(x) do { if (!(x)) throw std::runtime_error("check failed at line " + std::to_string(__LINE__)); } while (false)
namespace {
class TestInstance final : public fcitx::Instance {
public:
    TestInstance(int argc, char **argv) : Instance(argc, argv) { initialize(); }
};
class Client final : public fcitx::InputContext {
public:
    std::vector<std::string> commits;
    explicit Client(fcitx::InputContextManager &manager) : InputContext(manager,"zt-broker-test") {
        created(); setCapabilityFlags(fcitx::CapabilityFlag::Preedit); focusIn();
    }
    ~Client() override { destroy(); }
    const char *frontend() const override { return "zhuyintype-test"; }
    std::string preedit() { return inputPanel().clientPreedit().toString(); }
private:
    void commitStringImpl(const std::string &text) override { commits.push_back(text); }
    void deleteSurroundingTextImpl(int,unsigned int) override { throw std::runtime_error("unexpected surrounding text access"); }
    void forwardKeyImpl(const fcitx::ForwardKeyEvent &) override {}
    void updatePreeditImpl() override {}
};
}
int main() {
    try {
        {
            zhuyintype::Broker broker;
            CHECK(broker.connect());
            CHECK(broker.request("no-inline-presentation", "focus", {{"secure",false}, {"inlinePreedit",false}}));
            const auto response = broker.request("no-inline-presentation", "key", {{"secure",false}, {"key",'S'},
                {"modifiers",0}, {"release",false}, {"repeat",false}});
            CHECK(response && !(*response)["preedit"].get<std::string>().empty());
            CHECK((*response)["candidates"].get<bool>()); // Real Qt popup owns composition-only presentation.
            CHECK(broker.request("no-inline-presentation", "reset"));
            CHECK(broker.request("no-inline-presentation", "blur"));
        }
        char name[]="native-test", disable[]="--disable=all";
        char *args[]={name,disable,nullptr};
        TestInstance instance(2,args);
        zhuyintype::ZhuyinType addon(instance);
        Client a(instance.inputContextManager());
        Client b(instance.inputContextManager());
        fcitx::InputMethodEntry entry{"zhuyintype","ZhuyinType","zh_TW","zhuyintype"};
        const auto activate=[&](Client &ic) { ic.focusIn(); fcitx::FocusInEvent e(&ic); addon.activate(entry,e); };
        const auto key=[&](Client &ic,fcitx::KeySym sym,bool release=false,fcitx::KeyStates mods=fcitx::KeyStates()) {
            fcitx::KeyEvent e(&ic,fcitx::Key(sym,mods),release); addon.keyEvent(entry,e); return e.accepted();
        };
        const auto type=[&](Client &ic,std::string_view s) {
            for(char c:s) CHECK(key(ic,static_cast<fcitx::KeySym>(c)));
        };
        const std::string endpoint = std::getenv("ZT_IME_SOCKET");
        setenv("ZT_IME_SOCKET", "/tmp/zhuyintype-missing-test-endpoint", 1);
        const auto before = std::chrono::steady_clock::now();
        activate(a);
        CHECK(!key(a,FcitxKey_s));
        CHECK(std::chrono::steady_clock::now() - before < std::chrono::milliseconds(200));
        CHECK(a.preedit().empty()); CHECK(a.commits.empty());
        setenv("ZT_IME_SOCKET", endpoint.c_str(), 1);
        type(a,"su3cl3"); // next key reconnects without reloading the addon

        CHECK(a.preedit()=="你好");
        CHECK(a.inputPanel().clientPreedit().cursor()==6);
        CHECK(!a.inputPanel().candidateList());
        CHECK(key(a,FcitxKey_Return));
        CHECK(a.commits==std::vector<std::string>{"你好"});
        CHECK(a.preedit().empty());
        type(a,"su3");
        fcitx::InputContextEvent reset(&a,fcitx::EventType::InputContextReset); addon.reset(entry,reset);
        CHECK(a.preedit().empty()); CHECK(a.commits.size()==1);
        type(a,"su3");
        activate(b); // adapter must blur a before focusing b even if deactivate was omitted
        CHECK(a.commits.size()==2); CHECK(a.commits.back()=="你"); CHECK(b.commits.empty());
        type(b,"cl3"); CHECK(b.preedit()=="好");
        b.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,fcitx::CapabilityFlag::Sensitive});
        CHECK(b.preedit().empty()); CHECK(!key(b,FcitxKey_s)); CHECK(b.commits.empty());
        CHECK(!key(b,FcitxKey_Caps_Lock)); // Protected fields never serialize actual keys.
        b.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
        CHECK(!key(b,FcitxKey_s,false,fcitx::KeyState::CapsLock));
        CHECK(b.preedit().empty()); CHECK(b.commits.empty());
        CHECK(!key(b,FcitxKey_Caps_Lock,false,fcitx::KeyState::CapsLock));
        CHECK(!key(b,FcitxKey_Caps_Lock,true));
        type(b,"su3");
        fcitx::FocusOutEvent blur(&b); addon.deactivate(entry,blur);
        CHECK(b.commits==std::vector<std::string>{"你"});
        activate(a);
        // A short Shift tap is handled by the shared controller, including releases.
        CHECK(!key(a,FcitxKey_Shift_L,false,fcitx::KeyState::Shift));
        CHECK(!key(a,FcitxKey_Shift_L,true));
        CHECK(!key(a,FcitxKey_s)); CHECK(a.preedit().empty());
        CHECK(!key(a,FcitxKey_Shift_L,false,fcitx::KeyState::Shift));
        CHECK(!key(a,FcitxKey_Shift_L,true));
        type(a,"su3"); CHECK(!key(a,FcitxKey_c,false,fcitx::KeyState::Ctrl));
        CHECK(a.preedit().empty()); CHECK(a.commits.size()==3);
        a.setCapabilityFlags(fcitx::CapabilityFlags());
        activate(a);
        type(a,"su3cl3");
        CHECK(a.preedit().empty()); CHECK(a.inputPanel().preedit().empty()); CHECK(!a.inputPanel().candidateList());
        CHECK(key(a,FcitxKey_Return)); CHECK(a.commits.back()=="你好");
        a.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
        type(a,"su3"); CHECK(!a.preedit().empty());
        a.setCapabilityFlags(fcitx::CapabilityFlags());
        CHECK(a.preedit().empty()); CHECK(a.inputPanel().preedit().empty());
        CHECK(key(a,FcitxKey_Return)); CHECK(a.commits.back()=="你");
        a.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
        activate(a);
        CHECK(!key(a,FcitxKey_s,false,fcitx::KeyState::CapsLock));
        CHECK(a.preedit().empty());
        CHECK(!key(a,FcitxKey_Caps_Lock,false,fcitx::KeyState::CapsLock));
        CHECK(!key(a,FcitxKey_Caps_Lock,true));
        type(a,"su3");
        addon.reset(entry,reset);
        std::vector<double> timings;
        for (int i=0;i<200;++i) {
            const auto start=std::chrono::steady_clock::now();
            CHECK(!key(a,FcitxKey_Control_L));
            timings.push_back(std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-start).count());
        }
        std::sort(timings.begin(),timings.end());
        std::cout << "Broker Fcitx key roundtrip ms (200 modifier presses): p50=" << timings[99]
            << " p95=" << timings[189] << " p99=" << timings[197] << '\n';
        const auto deliveredBeforeDestroy = a.commits.size();
        {
            Client destroyed(instance.inputContextManager());
            activate(destroyed);
            type(destroyed,"su3");
        }
        activate(a);
        CHECK(a.preedit().empty()); CHECK(a.commits.size()==deliveredBeforeDestroy);
        std::cout << "PASS real Qt broker: Fcitx preedit/commit, reset, focus isolation, secure transition, Shift release, shortcuts\n";
    } catch(const std::exception &e) { std::cerr << e.what() << '\n'; return 1; }
}

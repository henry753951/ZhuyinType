#include "ZhuyinType.h"
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputpanel.h>
#include <fcitx/userinterfacemanager.h>
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
            for(char c:s) if (!key(ic,static_cast<fcitx::KeySym>(c)))
                throw std::runtime_error("key '" + std::string(1,c) + "' of \"" + std::string(s) + "\" was not accepted");
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
        b.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,fcitx::CapabilityFlag::Password});
        CHECK(b.preedit().empty()); CHECK(!key(b,FcitxKey_s)); CHECK(b.commits.empty());
        CHECK(!key(b,FcitxKey_Caps_Lock)); // Protected fields never serialize actual keys.
        b.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
        CHECK(!key(b,FcitxKey_s,false,fcitx::KeyState::CapsLock));
        CHECK(b.preedit().empty()); CHECK(b.commits.empty());
        CHECK(!key(b,FcitxKey_Caps_Lock,false,fcitx::KeyState::CapsLock));
        CHECK(!key(b,FcitxKey_Caps_Lock,true));
        type(b,"su3");
        // Focus leaving: Fcitx (or the application's module) commits the composition the field shows, so the addon
        // does not send it again (it would be typed twice, or into the field focus moved to).
        CHECK(b.preedit()=="你");
        fcitx::FocusOutEvent blur(&b); addon.deactivate(entry,blur);
        CHECK(b.commits.empty());
        // Switching to another input method: nothing else commits, so the addon does.
        activate(b);
        type(b,"su3");
        fcitx::InputContextSwitchInputMethodEvent switched(fcitx::InputMethodSwitchedReason::Trigger, "zhuyintype", &b);
        addon.deactivate(entry,switched);
        CHECK(b.commits==std::vector<std::string>{"你"});
        // A private field (Sensitive: a private window) types Chinese; only learning and 前文 stop.
        activate(b);
        b.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,fcitx::CapabilityFlag::Sensitive});
        type(b,"cl3"); CHECK(b.preedit()=="好");
        CHECK(key(b,FcitxKey_Return)); CHECK(b.commits.back()=="好");
        b.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
        fcitx::FocusOutEvent leavePrivate(&b); addon.deactivate(entry,leavePrivate);
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
        // Without inline composition the field shows nothing that Fcitx could commit on focus out: the addon does.
        type(a,"su3");
        const auto beforeFocusOut = a.commits.size();
        fcitx::FocusOutEvent leaveNoInline(&a); addon.deactivate(entry,leaveNoInline);
        CHECK(a.commits.size()==beforeFocusOut+1); CHECK(a.commits.back()=="你");
        activate(a);
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
        {
            // 快速設定 in the input method's menu (IPC `quick`): check items that follow the host, and 简 for 簡體.
            auto &ui = instance.userInterfaceManager();
            const auto item = [&](const char *name) { auto *action = ui.lookupAction(name); CHECK(action); return action; };
            CHECK(addon.subModeLabelImpl(entry,a) == "中");
            CHECK(!item("zhuyintype-quick-simplified")->isChecked(&a));
            item("zhuyintype-quick-simplified")->activate(&a);
            CHECK(item("zhuyintype-quick-simplified")->isChecked(&a));
            CHECK(addon.subModeLabelImpl(entry,a) == "简");
            item("zhuyintype-quick-pause-learning")->activate(&a);
            CHECK(item("zhuyintype-quick-pause-learning")->isChecked(&a));
            CHECK(addon.subMode(entry,a) == "簡體中文（暫停學習）");
            item("zhuyintype-quick-half-width")->activate(&a);
            CHECK(item("zhuyintype-quick-half-width")->isChecked(&a));
            item("zhuyintype-quick-english")->activate(&a);
            CHECK(item("zhuyintype-quick-english")->isChecked(&a) && addon.subModeLabelImpl(entry,a) == "英");
            for (const char *name : {"zhuyintype-quick-english", "zhuyintype-quick-half-width",
                     "zhuyintype-quick-simplified", "zhuyintype-quick-pause-learning"}) {
                item(name)->activate(&a);
                CHECK(!item(name)->isChecked(&a));
            }
            CHECK(addon.subModeLabelImpl(entry,a) == "中" && addon.subMode(entry,a) == "中文");
            CHECK(ui.lookupAction("zhuyintype-quick-separator")->isSeparator());
        }
        {
            // Fcitx draws the list (a Wayland session): the host's list becomes Fcitx's candidate list, and a click
            // in it picks through the host.
            setenv("ZT_FCITX_PANEL", "1", 1);
            Client drawn(instance.inputContextManager());
            activate(drawn);
            type(drawn,"su3");
            CHECK(!drawn.inputPanel().candidateList());
            CHECK(key(drawn,FcitxKey_Down));
            const auto list = drawn.inputPanel().candidateList();
            CHECK(list && list->size() >= 2 && list->cursorIndex() == 0);
            CHECK(list->label(0).toString() == "1. ");
            int other = -1;
            for (int i = 0; i < list->size(); ++i) if (list->candidate(i).text().toString() == "妳") other = i;
            CHECK(other > 0);
            list->candidate(other).select(&drawn);
            CHECK(drawn.preedit() == "妳"); CHECK(!drawn.inputPanel().candidateList());
            CHECK(key(drawn,FcitxKey_Return)); CHECK(drawn.commits == std::vector<std::string>{"妳"});
            // The 中/英 badge goes to Fcitx too (a short Shift tap).
            CHECK(!key(drawn,FcitxKey_Shift_L,false,fcitx::KeyState::Shift));
            CHECK(!key(drawn,FcitxKey_Shift_L,true));
            unsetenv("ZT_FCITX_PANEL");
        }
        std::cout << "PASS real host broker: Fcitx preedit/commit, reset, focus isolation, secure transition, Shift release, shortcuts, quick settings\n";
    } catch(const std::exception &e) { std::cerr << e.what() << '\n'; return 1; }
}

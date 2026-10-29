#include "Identity.h"
#include "Composition.h"
#include "Keyboard.h"
#include "InputModeButton.h"
#include <new>

namespace zt {
namespace {
template<class F> HRESULT Guard(F action) noexcept {
    try { return action(); } catch (const std::bad_alloc&) { return E_OUTOFMEMORY; } catch (...) { return E_FAIL; }
}
class TextService final : public ITfTextInputProcessorEx, public ITfThreadMgrEventSink,
    public ITfThreadFocusSink, public ITfKeyEventSink, public ITfCompositionSink {
public:
    TextService() { ++moduleObjects; }
    ~TextService() { --moduleObjects; }
    STDMETHODIMP QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out=nullptr;
        if (iid==IID_IUnknown || iid==IID_ITfTextInputProcessor || iid==IID_ITfTextInputProcessorEx)
            *out=static_cast<ITfTextInputProcessorEx*>(this);
        else if (iid==IID_ITfThreadMgrEventSink) *out=static_cast<ITfThreadMgrEventSink*>(this);
        else if (iid==IID_ITfThreadFocusSink) *out=static_cast<ITfThreadFocusSink*>(this);
        else if (iid==IID_ITfKeyEventSink) *out=static_cast<ITfKeyEventSink*>(this);
        else if (iid==IID_ITfCompositionSink) *out=static_cast<ITfCompositionSink*>(this);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
    STDMETHODIMP_(ULONG) Release() override { const auto n=--refs_; if (!n) delete this; return n; }
    STDMETHODIMP Activate(ITfThreadMgr* manager,TfClientId client) override { return ActivateEx(manager,client,0); }
    STDMETHODIMP ActivateEx(ITfThreadMgr* manager,TfClientId client,DWORD flags) override {
        return Guard([&]() -> HRESULT {
            if (!manager || manager_) return E_INVALIDARG;
            manager_=manager; client_=client;
            // TSF secure-mode and UI-less activation cannot use a desktop candidate window.
            restricted_=(flags & (TF_TMAE_SECUREMODE | TF_TMAE_COMLESS | TF_TMAE_UIELEMENTENABLEDONLY))!=0;
            HRESULT hr=manager->QueryInterface(IID_PPV_ARGS(&source_));
            if (SUCCEEDED(hr)) hr=source_->AdviseSink(IID_ITfThreadMgrEventSink,static_cast<ITfThreadMgrEventSink*>(this),&managerCookie_);
            if (SUCCEEDED(hr)) hr=source_->AdviseSink(IID_ITfThreadFocusSink,static_cast<ITfThreadFocusSink*>(this),&focusCookie_);
            if (SUCCEEDED(hr)) hr=manager->QueryInterface(IID_PPV_ARGS(&keys_));
            if (SUCCEEDED(hr)) hr=keys_->AdviseKeyEventSink(client,static_cast<ITfKeyEventSink*>(this),TRUE);
            if (FAILED(hr)) { Deactivate(); return hr; }
            wchar_t unique[80]{};
            swprintf_s(unique,L"ZhuyinType.Tsf.%lu.%p",GetCurrentThreadId(),static_cast<void*>(this));
            windowClass_=unique;
            WNDCLASSW klass{};
            klass.hInstance=moduleHandle; klass.lpfnWndProc=WindowProc; klass.lpszClassName=windowClass_.c_str();
            if (!RegisterClassW(&klass)) { Deactivate(); return HRESULT_FROM_WIN32(GetLastError()); }
            window_=CreateWindowExW(0,windowClass_.c_str(),L"",0,0,0,0,0,HWND_MESSAGE,nullptr,moduleHandle,this);
            if (!window_) { Deactivate(); return HRESULT_FROM_WIN32(GetLastError()); }
            // The 中/英 indicator in the taskbar; the input method works without it if the language bar refuses it.
            if (SUCCEEDED(manager->QueryInterface(IID_PPV_ARGS(&langBar_)))) {
                modeButton_=new InputModeButton([this](bool rightClick,POINT point) { rightClick ? ShowMenu(point) : ToggleMode(); });
                if (FAILED(langBar_->AddItem(modeButton_))) { modeButton_->ClearAction(); modeButton_->Release(); modeButton_=nullptr; langBar_.Reset(); }
            }
            threadFocused_=GetFocus()!=nullptr;
            return RefreshFocus();
        });
    }
    STDMETHODIMP Deactivate() override {
        return Guard([&]() -> HRESULT {
            SwitchContext(nullptr);
            if (modeButton_) {
                modeButton_->ClearAction();
                if (langBar_) langBar_->RemoveItem(modeButton_);
                modeButton_->Release(); modeButton_=nullptr;
            }
            langBar_.Reset();
            if (window_) { DestroyWindow(window_); window_=nullptr; }
            if (menuOwner_) { DestroyWindow(menuOwner_); menuOwner_=nullptr; }
            if (!windowClass_.empty()) { UnregisterClassW(windowClass_.c_str(),moduleHandle); windowClass_.clear(); }
            if (keys_) { keys_->UnadviseKeyEventSink(client_); keys_.Reset(); }
            if (source_) {
                if (managerCookie_!=TF_INVALID_COOKIE) source_->UnadviseSink(managerCookie_);
                if (focusCookie_!=TF_INVALID_COOKIE) source_->UnadviseSink(focusCookie_);
            }
            managerCookie_=focusCookie_=TF_INVALID_COOKIE;
            source_.Reset(); manager_.Reset(); client_=TF_CLIENTID_NULL;
            broker_.disconnect();
            return S_OK;
        });
    }
    STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr*) override { return S_OK; }
    STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr* document) override {
        return Guard([&]() -> HRESULT {
            if (context_) { Com<ITfDocumentMgr> owner; context_->GetDocumentMgr(&owner); if (owner.Get()==document) SwitchContext(nullptr); }
            return S_OK;
        });
    }
    STDMETHODIMP OnSetFocus(ITfDocumentMgr*,ITfDocumentMgr*) override { return Guard([&] { return RefreshFocus(); }); }
    STDMETHODIMP OnPushContext(ITfContext*) override { return Guard([&] { return RefreshFocus(); }); }
    STDMETHODIMP OnPopContext(ITfContext* popped) override {
        return Guard([&]() -> HRESULT { if (context_.Get()==popped) SwitchContext(nullptr); return RefreshFocus(); });
    }
    STDMETHODIMP OnSetThreadFocus() override {
        return Guard([&] { threadFocused_=true; return RefreshFocus(); });
    }
    STDMETHODIMP OnKillThreadFocus() override {
        return Guard([&]() -> HRESULT { threadFocused_=false; SwitchContext(nullptr); return S_OK; });
    }
    STDMETHODIMP OnSetFocus(BOOL foreground) override {
        return foreground ? OnSetThreadFocus() : OnKillThreadFocus();
    }
    STDMETHODIMP OnTestKeyDown(ITfContext* context,WPARAM vk,LPARAM flags,BOOL* eaten) override {
        return TestKey(context,vk,flags,false,eaten);
    }
    STDMETHODIMP OnTestKeyUp(ITfContext* context,WPARAM vk,LPARAM flags,BOOL* eaten) override {
        return TestKey(context,vk,flags,true,eaten);
    }
    STDMETHODIMP OnKeyDown(ITfContext* context,WPARAM vk,LPARAM flags,BOOL* eaten) override {
        return KeyEvent(context,vk,flags,false,eaten);
    }
    STDMETHODIMP OnKeyUp(ITfContext* context,WPARAM vk,LPARAM flags,BOOL* eaten) override {
        return KeyEvent(context,vk,flags,true,eaten);
    }
    STDMETHODIMP OnPreservedKey(ITfContext*,REFGUID,BOOL* eaten) override { if (!eaten) return E_POINTER; *eaten=FALSE; return S_OK; }
    STDMETHODIMP OnCompositionTerminated(TfEditCookie,ITfComposition* ended) override {
        return Guard([&]() -> HRESULT {
            if (composition_.Get()==ended) {
                composition_.Reset();
                if (!applying_ && broker_.connected() && !contextName_.empty()) {
                    if (broker_.busy()) { resetDeferred_=true; ++generation_; }
                    else broker_.request(contextName_,L"reset",Json{});
                }
            }
            return S_OK;
        });
    }
private:
    // A test is a pure local prediction; TSF may call it repeatedly without OnKeyDown. It must be right: a key the
    // test claims is taken from the application even if OnKeyDown then declines it.
    HRESULT TestKey(ITfContext* context,WPARAM vk,LPARAM flags,bool release,BOOL* eaten) noexcept {
        if (!eaten) return E_POINTER;
        *eaten=context && !restricted_ && threadFocused_ && !broker_.busy() && PotentialKey(vk) && Claims(vk,flags,release);
        return S_OK;
    }
    bool Claims(WPARAM vk,LPARAM flags,bool release) const noexcept {
        // Caps Lock is not claimed when idle; if it changed since the host's last reply its 英 may be stale, so
        // let the host decide (a declined character is then typed by OnKeyDown, never lost).
        const bool capsChanged=((GetKeyState(VK_CAPITAL)&1)!=0)!=capsAtReply_;
        return HostWantsKey(TranslateKey(vk,flags,release),composing_ || composition_.Get()!=nullptr,english_ && !capsChanged);
    }
    HRESULT RefreshFocus() {
        if (!manager_ || !threadFocused_) return S_OK;
        Com<ITfDocumentMgr> doc; Com<ITfContext> next;
        if (SUCCEEDED(manager_->GetFocus(&doc)) && doc) doc->GetTop(&next);
        SwitchContext(next.Get());
        return S_OK;
    }
    void SwitchContext(ITfContext* next) {
        if (broker_.busy()) {
            deferredContext_=next; focusDeferred_=true; ++generation_;
            if (window_) KillTimer(window_,1);
            return;
        }
        Com<ITfContext> keepNext=next;
        if (next==context_.Get()) return;
        ++generation_;
        if (window_) KillTimer(window_,1);
        auto old=context_;
        auto oldComposition=composition_;
        composition_.Reset();
        if (old) {
            std::optional<Reply> reply;
            if (broker_.connected() && hostFocused_) reply=broker_.request(contextName_,L"blur",Json{});
            const bool secure=secure_;
            // Blur is deliberately bound to the old document, even if its edit lock is deferred.
            // With no broker reply, leave visible composition text committed in that document.
            Edit(old.Get(),client_,false,[old,oldComposition,reply,secure](TfEditCookie cookie) mutable -> HRESULT {
                if (secure || IsSecure(old.Get(),cookie)) return FinishComposition(cookie,oldComposition,true);
                if (reply) return ApplyReply(old.Get(),cookie,nullptr,oldComposition,*reply);
                return FinishComposition(cookie,oldComposition,false);
            });
        }
        if (focusDeferred_) { keepNext=deferredContext_; deferredContext_.Reset(); focusDeferred_=false; }
        next=keepNext.Get();
        context_=next; hostFocused_=false; secure_=true; rect_={}; scale_=1;
        contextName_.clear();
        if (!next) { broker_.disconnect(); return; }
        GUID unique{};
        if (FAILED(CoCreateGuid(&unique))) return;
        wchar_t contextId[40]{}; StringFromGUID2(unique,contextId,40); contextName_=contextId;
        Schedule();
    }
    bool Prepare(TfEditCookie cookie,bool capsKey=false) {
        const auto generation=generation_;
        const bool secure=restricted_ || IsSecure(context_.Get(),cookie);
        if (modeButton_) modeButton_->SetEnabled(!secure);
        if (secure) {
            applying_=true;
            const auto hr=FinishComposition(cookie,composition_,true);
            applying_=false;
            if (FAILED(hr)) return false;
        }
        Caret(context_.Get(),cookie,rect_,scale_);
        if (!broker_.connected()) {
            hostFocused_=false;
            // A reconnect starts a new host session; locally preserve prior composition first.
            applying_=true;
            FinishComposition(cookie,composition_,secure);
            applying_=false;
            if (!broker_.connect()) return false;
        }
        Json fields;
        Put(fields,L"secure",secure);
        PutRect(fields,rect_,scale_);
        if (!hostFocused_) {
            if (!capsKey) Put(fields,L"capsLock",(GetKeyState(VK_CAPITAL)&1)!=0);
            const auto reply=broker_.request(contextName_,L"focus",fields);
            if (!reply || generation!=generation_) return false;
            hostFocused_=true;
            secure_=secure;
            if (!Apply(cookie,*reply)) return false;
        } else if (secure!=secure_) {
            secure_=secure;
            if (!secure && !capsKey) Put(fields,L"capsLock",(GetKeyState(VK_CAPITAL)&1)!=0);
            const auto reply=broker_.request(contextName_,L"secure",fields);
            if (!reply || generation!=generation_) return false;
            if (!Apply(cookie,*reply)) return false;
        }
        if (window_ && broker_.connected()) SetTimer(window_,1,20,nullptr);
        return true;
    }
    bool Apply(TfEditCookie cookie,const Reply& reply) {
        applying_=true;
        const HRESULT hr=secure_ ? FinishComposition(cookie,composition_,true) :
            ApplyReply(context_.Get(),cookie,static_cast<ITfCompositionSink*>(this),composition_,reply);
        applying_=false;
        if (FAILED(hr)) { broker_.disconnect(true); hostFocused_=false; return false; }
        english_=reply.english;
        composing_=reply.composing;
        capsAtReply_=(GetKeyState(VK_CAPITAL)&1)!=0;
        if (modeButton_) modeButton_->SetEnglish(reply.english);
        return true;
    }
    // The taskbar indicator: a left click switches 中/英 in the focused field, a right click opens the settings.
    bool CanToggle() const noexcept { return context_ && threadFocused_ && hostFocused_ && !secure_ && !broker_.busy(); }
    void OpenSettings() {
        Guard([&]() -> HRESULT {
            if (broker_.busy()) return S_OK;
            if (broker_.connected() || broker_.connect()) broker_.request(contextName_.empty() ? L"settings" : contextName_,L"show-settings",Json{});
            return S_OK;
        });
    }
    // The right-click menu of the 中／英 indicator, the usual place for an input method's settings on Windows.
    void ShowMenu(POINT point) {
        Guard([&]() -> HRESULT {
            // A menu needs a real (not message-only) owner window; it is never shown.
            if (!menuOwner_) menuOwner_=CreateWindowExW(WS_EX_TOOLWINDOW,windowClass_.c_str(),L"",WS_POPUP,0,0,0,0,nullptr,nullptr,moduleHandle,this);
            if (!menuOwner_) return S_OK;
            HMENU menu=CreatePopupMenu();
            if (!menu) return S_OK;
            AppendMenuW(menu,MF_STRING | (CanToggle() ? 0 : MF_GRAYED),1,english_ ? L"切換成中文" : L"切換成英文");
            AppendMenuW(menu,MF_SEPARATOR,0,nullptr);
            AppendMenuW(menu,MF_STRING,2,L"ZhuyinType 設定…");
            SetForegroundWindow(menuOwner_);
            const auto command=TrackPopupMenu(menu,TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RIGHTALIGN | TPM_NONOTIFY,
                point.x,point.y,0,menuOwner_,nullptr);
            PostMessageW(menuOwner_,WM_NULL,0,0);
            DestroyMenu(menu);
            if (command==1) ToggleMode();
            else if (command==2) OpenSettings();
            return S_OK;
        });
    }
    void ToggleMode() {
        Guard([&]() -> HRESULT {
            if (broker_.busy()) return S_OK;
            if (!context_ || !threadFocused_ || !hostFocused_ || secure_) return S_OK;
            const auto generation=generation_;
            Com<TextService> self=this;
            return Edit(context_.Get(),client_,false,[self,generation](TfEditCookie cookie) -> HRESULT {
                if (generation!=self->generation_ || !self->hostFocused_ || self->secure_) return S_OK;
                Json fields;
                Put(fields,L"english",!self->english_);
                const auto reply=self->broker_.request(self->contextName_,L"mode",fields);
                if (generation!=self->generation_) { self->RecoverDeferred(); return S_OK; }
                if (reply) self->Apply(cookie,*reply);
                self->RecoverDeferred();
                return S_OK;
            });
        });
    }
    void RecoverDeferred() {
        if (broker_.busy()) return;
        if (focusDeferred_) {
            auto next=deferredContext_; deferredContext_.Reset(); focusDeferred_=false;
            SwitchContext(next.Get());
        } else if (resetDeferred_) {
            resetDeferred_=false;
            if (broker_.connected() && hostFocused_) broker_.request(contextName_,L"reset",Json{});
        }
    }
    HRESULT KeyEvent(ITfContext* context,WPARAM vk,LPARAM flags,bool release,BOOL* eaten) noexcept {
        if (!eaten) return E_POINTER;
        *eaten=FALSE;
        return Guard([&]() -> HRESULT {
            if (!context || !threadFocused_ || restricted_ || broker_.busy() || !PotentialKey(vk)) return S_OK;
            // Only keys the test claimed reach here; TSF has already taken them from the application.
            const bool claimed=Claims(vk,flags,release);
            if (context_.Get()!=context) SwitchContext(context);
            bool handled=false;
            const auto generation=generation_;
            // The broker is called only after TSF grants a synchronous write lock. Failed locks
            // therefore pass the original event to the application without advancing the engine.
            const auto hr=Edit(context,client_,true,[&](TfEditCookie cookie) -> HRESULT {
                if (!Prepare(cookie,vk==VK_CAPITAL)) {
                    if (!focusDeferred_) { applying_=true; FinishComposition(cookie,composition_,false); applying_=false; }
                    return S_OK;
                }
                if (secure_) return S_OK; // No key identity or text leaves a secure context.
                const auto key=TranslateKey(vk,flags,release);
                if (!key.key) return S_OK;
                Json fields;
                Put(fields,L"key",key.key); Put(fields,L"modifiers",key.modifiers);
                Put(fields,L"release",key.release); Put(fields,L"repeat",key.repeat);
                Put(fields,L"shiftSide",key.shiftSide); Put(fields,L"secure",false);
                if (vk!=VK_CAPITAL) Put(fields,L"capsLock",(GetKeyState(VK_CAPITAL)&1)!=0);
                PutRect(fields,rect_,scale_);
                const auto reply=broker_.request(contextName_,L"key",fields);
                if (generation!=generation_) return S_OK;
                if (reply && Apply(cookie,*reply)) {
                    handled=reply->handled;
                    // A typed character the host declined (the mode changed in between): put it in the document
                    // ourselves rather than lose it.
                    if (!handled && claimed && !release && !composing_) {
                        if (const auto typed=TypedCharacter(vk,flags)) {
                            Reply local; local.commit.assign(1,typed); local.english=english_;
                            if (Apply(cookie,local)) handled=true;
                        }
                    }
                } else {
                    applying_=true; FinishComposition(cookie,composition_,false); applying_=false;
                    hostFocused_=false;
                }
                return S_OK;
            });
            RecoverDeferred();
            if (SUCCEEDED(hr)) *eaten=handled;
            if (!broker_.connected() && window_) KillTimer(window_,1);
            return S_OK;
        });
    }
    void Schedule() {
        if (pending_ || !context_ || !threadFocused_ || broker_.busy()) return;
        pending_=true;
        const auto generation=generation_;
        Com<TextService> self=this;
        const auto hr=Edit(context_.Get(),client_,false,[self,generation](TfEditCookie cookie) -> HRESULT {
            self->pending_=false;
            if (generation!=self->generation_) { self->Schedule(); return S_OK; }
            if (!self->context_ || !self->threadFocused_) return S_OK;
            if (!self->Prepare(cookie)) { if (self->window_) KillTimer(self->window_,1); self->RecoverDeferred(); return S_OK; }
            Json fields; PutRect(fields,self->rect_,self->scale_);
            const auto reply=self->broker_.request(self->contextName_,L"poll",fields);
            if (generation!=self->generation_) { self->RecoverDeferred(); return S_OK; }
            if (reply) self->Apply(cookie,*reply);
            else { self->hostFocused_=false; if (self->window_) KillTimer(self->window_,1); }
            return S_OK;
        });
        if (FAILED(hr)) pending_=false;
    }
    static LRESULT CALLBACK WindowProc(HWND window,UINT message,WPARAM wp,LPARAM lp) noexcept {
        auto* self=reinterpret_cast<TextService*>(GetWindowLongPtrW(window,GWLP_USERDATA));
        if (message==WM_NCCREATE) {
            self=static_cast<TextService*>(reinterpret_cast<CREATESTRUCTW*>(lp)->lpCreateParams);
            SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(self));
        }
        if (message==WM_TIMER && wp==1 && self) {
            Guard([&]() -> HRESULT { if (self->broker_.connected()) self->Schedule(); else KillTimer(window,1); return S_OK; });
            return 0;
        }
        return DefWindowProcW(window,message,wp,lp);
    }
    ULONG refs_=1;
    Com<ITfThreadMgr> manager_;
    Com<ITfSource> source_;
    Com<ITfKeystrokeMgr> keys_;
    DWORD managerCookie_=TF_INVALID_COOKIE,focusCookie_=TF_INVALID_COOKIE;
    TfClientId client_=TF_CLIENTID_NULL;
    Com<ITfContext> context_;
    Com<ITfContext> deferredContext_;
    Com<ITfComposition> composition_;
    BrokerClient broker_;
    std::wstring contextName_,windowClass_;
    HWND window_=nullptr,menuOwner_=nullptr;
    unsigned long long generation_=0;
    bool threadFocused_=false,hostFocused_=false,secure_=true,restricted_=false,pending_=false,applying_=false;
    bool focusDeferred_=false,resetDeferred_=false,english_=false,composing_=false,capsAtReply_=false;
    Com<ITfLangBarItemMgr> langBar_;
    InputModeButton* modeButton_=nullptr;
    RECT rect_{};
    double scale_=1;
};
}
HRESULT CreateService(REFIID iid,void** result) noexcept {
    return Guard([&] { auto* service=new TextService; const auto hr=service->QueryInterface(iid,result); service->Release(); return hr; });
}
}

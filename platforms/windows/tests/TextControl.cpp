#include "TextStore.h"
#include "Identity.h"
#include "PopupAutomation.h"
#include "ThreadManager.h"
#include "KeyboardInput.h"
#include "TestFocus.h"
#include <cstdio>
#include <stdexcept>
#include <chrono>

namespace zt { std::atomic<long> moduleObjects{0}; HMODULE moduleHandle=nullptr; }

namespace {
void Check(bool good,const char* label) { if (!good) throw std::runtime_error(label); printf("PASS %s\n",label); fflush(stdout); }
void Hr(HRESULT hr,const char* label) { if (FAILED(hr)) { fprintf(stderr,"HRESULT %08lX %s\n",static_cast<unsigned long>(hr),label); throw std::runtime_error(label); } }
void Pump(DWORD milliseconds) {
    const auto until=GetTickCount64()+milliseconds;
    do {
        MSG msg{};
        while (PeekMessageW(&msg,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&msg); DispatchMessageW(&msg); }
        MsgWaitForMultipleObjects(0,nullptr,FALSE,5,QS_ALLINPUT);
    } while (GetTickCount64()<until);
}
struct Control {
    zt::Com<TextStore> store;
    zt::Com<ITfDocumentMgr> document;
    zt::Com<ITfContext> context;
    TfEditCookie cookie=0;
    explicit Control(ITfThreadMgr* manager,TfClientId client,HWND window) {
        store.Attach(new TextStore); store->window=window;
        Hr(manager->CreateDocumentMgr(&document),"CreateDocumentMgr");
        Hr(document->CreateContext(client,0,static_cast<ITextStoreACP*>(store.Get()),&context,&cookie),"CreateContext");
        Hr(document->Push(context.Get()),"Push context");
    }
    ~Control() { if (document) document->Pop(TF_POPF_ALL); }
};
LRESULT CALLBACK WindowProc(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    if (message==WM_PAINT) {
        PAINTSTRUCT paint{}; const auto dc=BeginPaint(window,&paint);
        const auto* store=reinterpret_cast<TextStore*>(GetWindowLongPtrW(window,GWLP_USERDATA));
        const wchar_t* title=L"ZhuyinType 原生 TSF 測試";
        TextOutW(dc,20,20,title,static_cast<int>(wcslen(title)));
        if (store) {
            const auto value=store->password?std::wstring(store->text.size(),L'●'):store->text;
            TextOutW(dc,20,60,value.data(),static_cast<int>(value.size()));
        }
        EndPaint(window,&paint); return 0;
    }
    return DefWindowProcW(window,message,wp,lp);
}
void Focus(ITfThreadMgr* manager,Control& control) {
    SetWindowLongPtrW(control.store->window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(control.store.Get()));
    zt::Com<ITfDocumentMgr> previous;
    Hr(manager->AssociateFocus(control.store->window,control.document.Get(),&previous),"AssociateFocus control");
    ActivateTestWindow(control.store->window,control.store->window);
    Hr(manager->SetFocus(control.document.Get()),"SetFocus document");
    Pump(60);
    ActivateTestWindow(control.store->window,control.store->window);
    Pump(10);
}
bool Key(ITfKeystrokeMgr* keys,TextStore& store,UINT vk,bool up=false,bool checkTest=true) {
    const auto old=store.text;
    const auto count=store.starts;
    const LPARAM flags=(static_cast<LPARAM>(MapVirtualKeyW(vk,MAPVK_VK_TO_VSC))<<16)|(up?(3LL<<30):0);
    BOOL tested=FALSE,handled=FALSE;
    Hr(up?keys->TestKeyUp(vk,flags,&tested):keys->TestKeyDown(vk,flags,&tested),"TestKey");
    if (checkTest) {
        if (old!=store.text || count!=store.starts) throw std::runtime_error("test key mutated control");
        Hr(up?keys->TestKeyUp(vk,flags,&tested):keys->TestKeyDown(vk,flags,&tested),"repeated TestKey");
        if (old!=store.text || count!=store.starts) throw std::runtime_error("repeated test key mutated control");
    }
    if (tested) Hr(up?keys->KeyUp(vk,flags,&handled):keys->KeyDown(vk,flags,&handled),"Key");
    return handled!=FALSE;
}
void Tap(ITfKeystrokeMgr* keys,TextStore& store,UINT vk) { Key(keys,store,vk); Key(keys,store,vk,true); Pump(5); }
void Reading(ITfKeystrokeMgr* keys,TextStore& store) { for (const auto vk:{'S','U','3'}) Tap(keys,store,static_cast<UINT>(vk)); }
void PopupTest(ITfKeystrokeMgr* keys,TextStore& store,HWND window) {
    store.Replace(L"");
    Reading(keys,store);
    Tap(keys,store,VK_DOWN);
    Pump(250);
    PopupAutomation popup;
    Check(popup.Visible(),"Qt candidate popup visible");
    RECT popupRect{}; GetWindowRect(FindWindowW(nullptr,L"ZhuyinType 候選字"),&popupRect);
    POINT caret{20+12*store.end,88}; ClientToScreen(window,&caret);
    MONITORINFO monitor{sizeof(monitor)}; GetMonitorInfoW(MonitorFromWindow(window,MONITOR_DEFAULTTONEAREST),&monitor);
    const LONG expectedX=std::clamp(caret.x,monitor.rcWork.left,std::max(monitor.rcWork.left,monitor.rcWork.right-(popupRect.right-popupRect.left)));
    const LONG height=popupRect.bottom-popupRect.top;
    const LONG y=caret.y+3+height<=monitor.rcWork.bottom?caret.y+3:caret.y-28-height-3;
    const LONG expectedY=std::clamp(y,monitor.rcWork.top,std::max(monitor.rcWork.top,monitor.rcWork.bottom-height));
    Check(std::abs(popupRect.left-expectedX)<=2 && std::abs(popupRect.top-expectedY)<=2,"Qt popup anchored at native caret");
    const auto first=PopupAutomation::Name(popup.FirstCandidate().Get());
    const auto next=popup.Named(L"下一頁");
    Check(next!=nullptr,"candidate paging accessible");
    PopupAutomation::Click(next.Get()); Pump(250);
    Check(PopupAutomation::Name(popup.FirstCandidate().Get())!=first,"mouse advances candidate page");
    auto previous=popup.Named(L"上一頁"); PopupAutomation::Click(previous.Get()); Pump(250);
    Check(PopupAutomation::Name(popup.FirstCandidate().Get())==first,"mouse returns candidate page");
    PopupAutomation::Click(popup.Named(L"下一頁").Get()); Pump(250);
    const auto candidate=PopupAutomation::Name(popup.FirstCandidate().Get());
    auto expected=candidate.substr(2); const auto annotation=expected.find(L'，'); if (annotation!=std::wstring::npos) expected.resize(annotation);
    PopupAutomation::Click(popup.FirstCandidate().Get()); Pump(120);
    Check(::GetFocus()==window,"candidate mouse preserves native focus");
    Check(!expected.empty() && store.text.find(expected)!=std::wstring::npos,"candidate mouse result delivered by focused poll");
    if (store.composing) { Tap(keys,store,VK_RETURN); Tap(keys,store,VK_RETURN); }
    Check(!store.composing,"candidate selection can commit to control");
    const auto committed=store.text; Pump(100);
    Check(store.text==committed,"poll does not duplicate commit");
}
void ReportLatency(std::vector<double> values) {
    std::sort(values.begin(),values.end());
    const auto q=[&](double p) { return values[static_cast<size_t>((values.size()-1)*p)]; };
    printf("LATENCY key roundtrip+TSF edit n=%zu p50=%.3fms p95=%.3fms p99=%.3fms\n",values.size(),q(.5),q(.95),q(.99));
}
}
int wmain(int argc,wchar_t** argv) {
    const bool unavailable=argc>1 && !wcscmp(argv[1],L"--unavailable");
    const bool interactive=argc>1 && !wcscmp(argv[1],L"--interactive");
    const bool popupTest=argc>1 && !wcscmp(argv[1],L"--popup");
    try {
        winrt::init_apartment(winrt::apartment_type::single_threaded);
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if (unavailable) SetEnvironmentVariableW(L"ZT_IME_SOCKET",L"\\\\.\\pipe\\ZhuyinType-absent-tsf-test");
        if (!unavailable) {
            zt::BrokerClient probe;
            Check(probe.connect(),"broker named pipe connects");
            zt::Json fields; zt::Put(fields,L"secure",false); zt::PutRect(fields,RECT{100,100,102,120},1);
            Check(probe.request(L"native-probe",L"focus",fields).has_value(),"broker focus response validates");
            Check(probe.request(L"native-probe",L"blur",zt::Json{}).has_value(),"broker probe releases focus");
        }
        WNDCLASSW klass{}; klass.lpfnWndProc=WindowProc; klass.hInstance=GetModuleHandleW(nullptr);
        klass.lpszClassName=L"ZhuyinType.Tsf.TestControl"; klass.hbrBackground=reinterpret_cast<HBRUSH>(COLOR_WINDOW+1);
        RegisterClassW(&klass);
        const auto window=CreateWindowExW(0,klass.lpszClassName,L"ZhuyinType 原生 TSF 測試",WS_OVERLAPPEDWINDOW|WS_VISIBLE,
            100,100,650,300,nullptr,nullptr,klass.hInstance,nullptr);
        Check(window!=nullptr,"native window");
        Check(ActivateTestWindow(window,window),"test window activated");
        Check(ReleaseTestModifiers(),"neutral synthetic keyboard"); Pump(60);
        ActivateTestWindow(window,window);
        zt::Com<ITfThreadMgr> manager;
        Hr(CurrentThreadManager(&manager),"ThreadMgr");
        TfClientId client=TF_CLIENTID_NULL;
        Hr(manager->Activate(&client),"Activate TSF");
        Control first(manager.Get(),client,window), second(manager.Get(),client,window);
        zt::Com<ITfInputProcessorProfiles> languages;
        Hr(CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&languages)),"Profiles");
        LANGID previous=0; languages->GetCurrentLanguage(&previous);
        Hr(languages->ChangeCurrentLanguage(zt::language),"Traditional Chinese language");
        zt::Com<ITfInputProcessorProfileMgr> profiles;
        Hr(languages.As(&profiles),"ProfileMgr");
        const auto activated=profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS);
        Hr(activated,"Activate registered profile"); Check(activated==S_OK,"registered profile enabled");
        Focus(manager.Get(),first);
        Check(GetForegroundWindow()==window && GetFocus()==window,"native test window foreground");
        Check(GetModuleHandleW(L"zhuyintype-tsf.dll")!=nullptr,"TSF loaded registered DLL");
        Hr(zt::Edit(first.context.Get(),client,true,[&](TfEditCookie cookie) -> HRESULT {
            Check(!zt::IsSecure(first.context.Get(),cookie),"normal text store is not secure");
            return S_OK;
        }),"normal store write session");
        zt::Com<ITfKeystrokeMgr> keys; Hr(manager.As(&keys),"KeystrokeMgr");
        CLSID foreground{};
        Hr(keys->GetForeground(&foreground),"foreground key service");
        Check(foreground==zt::serviceId,"registered key service foreground");
        if (unavailable) {
            const auto start=GetTickCount64();
            Check(!Key(keys.Get(),*first.store.Get(),'S'),"unavailable key passes to application");
            Check(first.store->text.empty() && !first.store->composing,"unavailable preserves empty control");
            Check(GetTickCount64()-start<150,"unavailable bounded latency");
        } else {
            Check(Key(keys.Get(),*first.store.Get(),'S'),"broker handles first key");
            Check(first.store->composing && !first.store->text.empty(),"native composition visible");
            Key(keys.Get(),*first.store.Get(),'S',true);
            Tap(keys.Get(),*first.store.Get(),'U'); Tap(keys.Get(),*first.store.Get(),'3');
            Check(first.store->textExtCalls>0,"caret GetTextExt used");
            Tap(keys.Get(),*first.store.Get(),VK_RETURN);
            Check(!first.store->composing && !first.store->text.empty(),"commit ends native composition");
            if (popupTest) PopupTest(keys.Get(),*first.store.Get(),window);
            first.store->Replace(L"");
            Reading(keys.Get(),*first.store.Get());
            Check(first.store->composing,"second composition");
            Focus(manager.Get(),second);
            Pump(100);
            Check(!first.store->composing && !first.store->text.empty(),"blur commits old document");
            Check(second.store->text.empty(),"focus switch has no stale commit");
            second.store->Password(true); Pump(80);
            const auto count=second.store->starts;
            Check(!Key(keys.Get(),*second.store.Get(),'S'),"secure key passes to application");
            Check(second.store->text.empty() && second.store->starts==count,"secure field has no composition");
            second.store->Password(false); Pump(80);
            Check(Key(keys.Get(),*second.store.Get(),'S'),"normal input resumes after secure field");
            Tap(keys.Get(),*second.store.Get(),VK_ESCAPE); Tap(keys.Get(),*second.store.Get(),VK_ESCAPE);
            Tap(keys.Get(),*second.store.Get(),VK_RETURN); second.store->Replace(L"");
            second.store->denyLock=true;
            Check(!Key(keys.Get(),*second.store.Get(),'S'),"denied edit lock passes key");
            second.store->denyLock=false;
            Check(second.store->text.empty(),"denied edit lock has no mutation");
            std::vector<double> timings;
            for (int i=0;i<40;++i) {
                const auto begin=std::chrono::steady_clock::now();
                Key(keys.Get(),*second.store.Get(),'S');
                timings.push_back(std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-begin).count());
                Key(keys.Get(),*second.store.Get(),'S',true);
                Tap(keys.Get(),*second.store.Get(),VK_RETURN); second.store->Replace(L"");
            }
            ReportLatency(std::move(timings));
            if (interactive) { Reading(keys.Get(),*second.store.Get()); Tap(keys.Get(),*second.store.Get(),VK_SPACE); Pump(60000); }
        }
        Hr(manager->SetFocus(nullptr),"blur final document"); Pump(50);
        profiles->DeactivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS);
        if (previous) languages->ChangeCurrentLanguage(previous);
        first.document->Pop(TF_POPF_ALL); second.document->Pop(TF_POPF_ALL);
        manager->Deactivate(); DestroyWindow(window);
        printf("PASS registered native TSF integration\n");
        return 0;
    } catch (const std::exception& e) { fprintf(stderr,"FAIL %s\n",e.what()); return 1; }
    catch (...) { fprintf(stderr,"FAIL Windows API\n"); return 1; }
}

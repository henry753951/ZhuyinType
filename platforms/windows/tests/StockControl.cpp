#include "Composition.h"
#include "Identity.h"
#include "KeyboardInput.h"
#include "TestFocus.h"
#include <richedit.h>
#include <imm.h>
#include <cstdio>
#include <algorithm>
#include <stdexcept>

namespace {
bool Chinese(const std::wstring& text) { return !text.empty() && std::all_of(text.begin(),text.end(),[](wchar_t c) { return c>=0x3400 && c<=0x9fff; }); }
struct NativeObservation { WNDPROC previous=nullptr; unsigned starts=0,ends=0,results=0; bool resultMatches=false; std::wstring preedit; };
LRESULT CALLBACK ObserveControl(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    auto* observation=static_cast<NativeObservation*>(GetPropW(window,L"ZhuyinType.TestObserver"));
    if (observation) {
        if (message==WM_IME_STARTCOMPOSITION) ++observation->starts;
        if (message==WM_IME_ENDCOMPOSITION) ++observation->ends;
        if (message==WM_IME_COMPOSITION && (lp&(GCS_COMPSTR|GCS_RESULTSTR))) {
            const auto context=ImmGetContext(window);
            wchar_t result[64]{};
            if (context) {
                if (lp&GCS_COMPSTR) {
                    const auto bytes=ImmGetCompositionStringW(context,GCS_COMPSTR,result,sizeof(result));
                    if (bytes>0 && bytes<=static_cast<LONG>(sizeof(result))) observation->preedit.assign(result,static_cast<size_t>(bytes)/sizeof(wchar_t));
                }
                if (lp&GCS_RESULTSTR) {
                    ++observation->results;
                    const auto bytes=ImmGetCompositionStringW(context,GCS_RESULTSTR,result,sizeof(result));
                    observation->resultMatches=bytes>0 && bytes<=static_cast<LONG>(sizeof(result)) &&
                        std::wstring(result,static_cast<size_t>(bytes)/sizeof(wchar_t))==observation->preedit;
                }
                ImmReleaseContext(window,context);
            }
        }
        return CallWindowProcW(observation->previous,window,message,wp,lp);
    }
    return DefWindowProcW(window,message,wp,lp);
}
void Observe(HWND window,NativeObservation& observation) {
    SetPropW(window,L"ZhuyinType.TestObserver",&observation);
    observation.previous=reinterpret_cast<WNDPROC>(SetWindowLongPtrW(window,GWLP_WNDPROC,reinterpret_cast<LONG_PTR>(ObserveControl)));
}
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
void Tap(HWND window,HWND edit,WORD vk) {
    Check(GetForegroundWindow()==window && GetFocus()==edit,"SendInput destination focused");
    INPUT events[2]{};
    for (auto& event:events) { event.type=INPUT_KEYBOARD; event.ki.wVk=vk; }
    events[1].ki.dwFlags=KEYEVENTF_KEYUP;
    Check(SendInput(2,events,sizeof(INPUT))==2,"Windows accepts SendInput");
    Pump(70);
}
std::wstring Text(HWND edit) {
    const auto n=GetWindowTextLengthW(edit);
    std::wstring result(static_cast<size_t>(n)+1,L'\0');
    result.resize(GetWindowTextW(edit,result.data(),n+1)); return result;
}
LRESULT CALLBACK WindowProc(HWND window,UINT message,WPARAM wp,LPARAM lp) { return DefWindowProcW(window,message,wp,lp); }
bool Exercise(HWND window,HWND edit,NativeObservation& observation,const char* label) {
    Check(ActivateTestWindow(window,edit),"stock test window activated"); Pump(150);
    for (const WORD vk:{WORD('S'),WORD('U'),WORD('3')}) Tap(window,edit,vk);
    const auto expected=observation.preedit.empty()?Text(edit):observation.preedit;
    Check(Chinese(expected),"native application exposes Chinese composition");
    Tap(window,edit,VK_RETURN);
    for (unsigned i=0;i<10 && Text(edit)!=expected;++i) Pump(50);
    const auto committed=Text(edit);
    const bool delivered=committed==expected;
    printf("%s %s\n",delivered?"PASS":"FAIL",label);
    printf("EVIDENCE committed length=%zu IME starts=%u ends=%u results=%u fixtureResult=%u\n",committed.size(),observation.starts,observation.ends,observation.results,observation.resultMatches);
    Pump(100); Check(Text(edit)==committed,"stock control commit delivered once");
    return delivered;
}
}
int wmain() {
    try {
        winrt::init_apartment(winrt::apartment_type::single_threaded);
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        const auto rich=LoadLibraryW(L"msftedit.dll"); Check(rich!=nullptr,"Windows RichEdit loaded");
        WNDCLASSW klass{}; klass.hInstance=GetModuleHandleW(nullptr); klass.lpfnWndProc=WindowProc;
        klass.lpszClassName=L"ZhuyinType.StockControl.Test"; klass.hbrBackground=reinterpret_cast<HBRUSH>(COLOR_WINDOW+1);
        RegisterClassW(&klass);
        const auto window=CreateWindowExW(0,klass.lpszClassName,L"ZhuyinType Windows 控制項測試",WS_OVERLAPPEDWINDOW|WS_VISIBLE,
            100,100,700,400,nullptr,nullptr,klass.hInstance,nullptr);
        Check(window!=nullptr,"stock control test window");
        const auto edit=CreateWindowExW(WS_EX_CLIENTEDGE,L"EDIT",L"",WS_CHILD|WS_VISIBLE|ES_MULTILINE|ES_AUTOVSCROLL,20,20,620,100,window,nullptr,klass.hInstance,nullptr);
        const auto richEdit=CreateWindowExW(WS_EX_CLIENTEDGE,MSFTEDIT_CLASS,L"",WS_CHILD|WS_VISIBLE|ES_MULTILINE|ES_AUTOVSCROLL,20,150,620,100,window,nullptr,klass.hInstance,nullptr);
        Check(edit && richEdit,"Windows EDIT and RichEdit controls");
        NativeObservation editObservation,richObservation;
        Observe(edit,editObservation); Observe(richEdit,richObservation);
        SendMessageW(richEdit,EM_SETEDITSTYLE,SES_USECTF,SES_USECTF);
        Check(ActivateTestWindow(window,edit),"stock test foreground"); Pump(100);
        Check(ReleaseTestModifiers(),"neutral synthetic keyboard"); Pump(60);
        ActivateTestWindow(window,edit);
        zt::Com<ITfThreadMgr> manager;
        Hr(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&manager)),"ThreadMgr");
        TfClientId client=0; Hr(manager->Activate(&client),"Activate TSF");
        zt::Com<ITfInputProcessorProfiles> languages;
        Hr(CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&languages)),"Profiles");
        LANGID previous=0; languages->GetCurrentLanguage(&previous);
        Hr(languages->ChangeCurrentLanguage(zt::language),"Chinese language");
        zt::Com<ITfInputProcessorProfileMgr> profiles; Hr(languages.As(&profiles),"ProfileMgr");
        TF_INPUTPROCESSORPROFILE registered{};
        Hr(profiles->GetProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,&registered),"registered profile lookup");
        Check(registered.clsid==zt::serviceId && registered.guidProfile==zt::profileId,"real TSF profile registered");
        Hr(profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS),"ActivateProfile");
        TF_INPUTPROCESSORPROFILE active{}; Hr(profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD,&active),"active keyboard profile");
        Check(active.clsid==zt::serviceId && active.guidProfile==zt::profileId,"real TSF profile active");
        // This message loop deliberately uses only TranslateMessage/DispatchMessage. The
        // controls and Windows dispatch keys; no test calls ITfKeystrokeMgr or OnKeyDown.
        const bool editPassed=Exercise(window,edit,editObservation,"Windows EDIT SendInput commit");
        const bool richPassed=Exercise(window,richEdit,richObservation,"Windows RichEdit SendInput commit");
        const auto password=CreateWindowExW(WS_EX_CLIENTEDGE,L"EDIT",L"",WS_CHILD|WS_VISIBLE|ES_PASSWORD,20,270,620,35,window,nullptr,klass.hInstance,nullptr);
        ::SetFocus(password); Pump(100);
        Tap(window,password,'S'); Tap(window,password,'U'); Tap(window,password,'3');
        Check(Text(password)==L"su3","stock password field receives ordinary keys");
        Check(editPassed && richPassed,"both native controls commit expected fixture");
        profiles->DeactivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,zt::language,zt::serviceId,zt::profileId,nullptr,TF_IPPMF_FORPROCESS);
        if (previous) languages->ChangeCurrentLanguage(previous);
        manager->Deactivate(); DestroyWindow(window); FreeLibrary(rich);
        printf("PASS Windows-dispatched stock text controls\n"); return 0;
    } catch (const std::exception& e) {
        fprintf(stderr,"FAIL %s\n",e.what());
        return 1;
    }
    catch (...) { fprintf(stderr,"FAIL Windows API\n"); return 1; }
}

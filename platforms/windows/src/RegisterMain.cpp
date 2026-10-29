#include "Identity.h"
#include <cstdio>
#include <string>
int wmain(int argc,wchar_t** argv) {
    if (argc<2 || (wcscmp(argv[1],L"install") && wcscmp(argv[1],L"uninstall"))) {
        fwprintf(stderr,L"用法：zt-tsf-register install DLL路徑 [--icon EXE路徑] [--machine] | uninstall [--machine]\n"); return 2;
    }
    const bool install=wcscmp(argv[1],L"install")==0;
    bool machine=false;
    const wchar_t* icon=nullptr;
    for (int i=install?3:2;i<argc;++i) {
        if (!wcscmp(argv[i],L"--machine")) machine=true;
        else if (!wcscmp(argv[i],L"--icon") && i+1<argc) icon=argv[++i];
        else return 2;
    }
    if (install && (argc<3 || !wcscmp(argv[2],L"--machine"))) return 2;
    const auto init=CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    if (FAILED(init)) return 1;
    HRESULT hr=E_FAIL;
    if (install) {
        wchar_t full[32768]{};
        const auto length=GetFullPathNameW(argv[2],32768,full,nullptr);
        wchar_t fullIcon[32768]{};
        if (icon && (!GetFullPathNameW(icon,32768,fullIcon,nullptr) || GetFileAttributesW(fullIcon)==INVALID_FILE_ATTRIBUTES)) {
            CoUninitialize(); return 2;
        }
        if (length && length<32768) hr=zt::Register(full,machine,icon?fullIcon:nullptr);
    } else hr=zt::Unregister(machine);
    CoUninitialize();
    if (FAILED(hr)) { fwprintf(stderr,L"TSF 註冊失敗：0x%08lX。系統層級註冊須以系統管理員執行 --machine。\n",static_cast<unsigned long>(hr)); return 1; }
    wprintf(L"TSF %ls完成（COM：%ls；TSF profile：Windows API）。\n",install?L"註冊":L"移除",machine?L"本機":L"目前使用者");
    return 0;
}

#pragma once
#include "Composition.h"
inline HRESULT CurrentThreadManager(ITfThreadMgr** manager) {
    if (!manager) return E_POINTER;
    *manager=nullptr;
    const auto module=GetModuleHandleW(L"msctf.dll");
    using GetManager=HRESULT(WINAPI*)(ITfThreadMgr**);
    const auto get=module?reinterpret_cast<GetManager>(GetProcAddress(module,"TF_GetThreadMgr")):nullptr;
    if (get && SUCCEEDED(get(manager)) && *manager) return S_OK;
    return CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(manager));
}

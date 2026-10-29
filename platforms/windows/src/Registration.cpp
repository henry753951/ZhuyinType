#include "Identity.h"
#include <msctf.h>
#include <wrl/client.h>
#include <string>

namespace zt {
namespace {
std::wstring ClassKey() {
    wchar_t id[40]{}; StringFromGUID2(serviceId,id,40);
    return std::wstring(L"Software\\Classes\\CLSID\\")+id;
}
HRESULT StringValue(HKEY root,const std::wstring& path,const wchar_t* key,const wchar_t* value) {
    HKEY handle=nullptr;
    auto status=RegCreateKeyExW(root,path.c_str(),0,nullptr,0,KEY_WRITE,nullptr,&handle,nullptr);
    if (status!=ERROR_SUCCESS) return HRESULT_FROM_WIN32(status);
    status=RegSetValueExW(handle,key,0,REG_SZ,reinterpret_cast<const BYTE*>(value),static_cast<DWORD>((wcslen(value)+1)*sizeof(wchar_t)));
    RegCloseKey(handle);
    return HRESULT_FROM_WIN32(status);
}
constexpr const GUID* categories[]={&GUID_TFCAT_TIP_KEYBOARD,&GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,&GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT};
}
HRESULT Register(const wchar_t* path,bool machine,const wchar_t* iconPath) noexcept {
    try {
        if (!path || !*path || GetFileAttributesW(path)==INVALID_FILE_ATTRIBUTES) return E_INVALIDARG;
        const auto* icon=iconPath?iconPath:path;
        if (GetFileAttributesW(icon)==INVALID_FILE_ATTRIBUTES) return E_INVALIDARG;
        const auto root=machine?HKEY_LOCAL_MACHINE:HKEY_CURRENT_USER;
        const auto key=ClassKey();
        auto hr=StringValue(root,key,nullptr,name);
        if (SUCCEEDED(hr)) hr=StringValue(root,key+L"\\InprocServer32",nullptr,path);
        if (SUCCEEDED(hr)) hr=StringValue(root,key+L"\\InprocServer32",L"ThreadingModel",L"Apartment");
        if (FAILED(hr)) return hr;
        Microsoft::WRL::ComPtr<ITfInputProcessorProfileMgr> profiles;
        hr=CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles));
        if (SUCCEEDED(hr)) hr=profiles->RegisterProfile(serviceId,language,profileId,name,static_cast<ULONG>(wcslen(name)),
            icon,static_cast<ULONG>(wcslen(icon)),0,nullptr,0,TRUE,0);
        if (FAILED(hr)) { RegDeleteTreeW(root,key.c_str()); return hr; }
        Microsoft::WRL::ComPtr<ITfCategoryMgr> manager;
        hr=CoCreateInstance(CLSID_TF_CategoryMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&manager));
        if (SUCCEEDED(hr)) for (const auto* category:categories) {
            hr=manager->RegisterCategory(serviceId,*category,serviceId);
            if (FAILED(hr)) break;
        }
        if (FAILED(hr)) { Unregister(machine); return hr; }
        return S_OK;
    } catch (...) { return E_FAIL; }
}
HRESULT Unregister(bool machine) noexcept {
    try {
        HRESULT result=S_OK;
        Microsoft::WRL::ComPtr<ITfCategoryMgr> categoriesManager;
        auto hr=CoCreateInstance(CLSID_TF_CategoryMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&categoriesManager));
        if (FAILED(hr)) return hr;
        for (const auto* category:categories) {
            hr=categoriesManager->UnregisterCategory(serviceId,*category,serviceId);
            if (FAILED(hr)) result=hr;
        }
        Microsoft::WRL::ComPtr<ITfInputProcessorProfileMgr> profiles;
        hr=CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles));
        if (SUCCEEDED(hr)) hr=profiles->UnregisterProfile(serviceId,language,profileId,0);
        if (FAILED(hr)) result=hr;
        const auto status=RegDeleteTreeW(machine?HKEY_LOCAL_MACHINE:HKEY_CURRENT_USER,ClassKey().c_str());
        if (status!=ERROR_SUCCESS && status!=ERROR_FILE_NOT_FOUND) result=HRESULT_FROM_WIN32(status);
        return result;
    } catch (...) { return E_FAIL; }
}
}

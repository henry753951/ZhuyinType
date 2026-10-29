#include "Identity.h"
#include <new>
namespace zt {
std::atomic<long> moduleObjects{0};
HMODULE moduleHandle=nullptr;
namespace {
class Factory final : public IClassFactory {
public:
    Factory() { ++moduleObjects; }
    ~Factory() { --moduleObjects; }
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        if (!out) return E_POINTER;
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_IClassFactory) return E_NOINTERFACE;
        *out=static_cast<IClassFactory*>(this); AddRef(); return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
    STDMETHODIMP_(ULONG) Release() override { const auto n=--refs_; if (!n) delete this; return n; }
    STDMETHODIMP CreateInstance(IUnknown* outer,REFIID iid,void** out) override {
        if (!out) return E_POINTER;
        *out=nullptr;
        return outer?CLASS_E_NOAGGREGATION:CreateService(iid,out);
    }
    STDMETHODIMP LockServer(BOOL lock) override { if (lock) ++moduleObjects; else --moduleObjects; return S_OK; }
private:
    ULONG refs_=1;
};
}
}
BOOL WINAPI DllMain(HINSTANCE instance,DWORD reason,LPVOID) {
    if (reason==DLL_PROCESS_ATTACH) zt::moduleHandle=instance;
    return TRUE;
}
extern "C" HRESULT __stdcall DllGetClassObject(REFCLSID clsid,REFIID iid,void** out) {
    if (!out) return E_POINTER;
    *out=nullptr;
    if (clsid!=zt::serviceId) return CLASS_E_CLASSNOTAVAILABLE;
    auto* factory=new(std::nothrow) zt::Factory;
    if (!factory) return E_OUTOFMEMORY;
    const auto hr=factory->QueryInterface(iid,out); factory->Release(); return hr;
}
extern "C" HRESULT __stdcall DllCanUnloadNow() { return zt::moduleObjects==0?S_OK:S_FALSE; }
extern "C" HRESULT __stdcall DllRegisterServer() {
    wchar_t path[32768]{};
    const auto n=GetModuleFileNameW(zt::moduleHandle,path,32768);
    if (!n || n==32768) return E_FAIL;
    return zt::Register(path,false);
}
extern "C" HRESULT __stdcall DllUnregisterServer() { return zt::Unregister(false); }

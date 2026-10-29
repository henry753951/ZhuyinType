#pragma once
#include <windows.h>
#include <objbase.h>
#include <atomic>

namespace zt {
inline constexpr CLSID serviceId{0x25bc3548,0xb611,0x4f92,{0x91,0xd0,0xc9,0x29,0x1d,0x42,0x7e,0xa8}};
inline constexpr GUID profileId{0xe8207cf3,0x7ce3,0x42ae,{0x9b,0x18,0x45,0x32,0x84,0xa7,0x39,0x01}};
inline constexpr LANGID language = 0x0404;
inline constexpr wchar_t name[] = L"ZhuyinType 注音";
extern std::atomic<long> moduleObjects;
extern HMODULE moduleHandle;
HRESULT CreateService(REFIID iid, void** result) noexcept;
HRESULT Register(const wchar_t* dllPath, bool machine, const wchar_t* iconPath=nullptr) noexcept;
HRESULT Unregister(bool machine) noexcept;
}

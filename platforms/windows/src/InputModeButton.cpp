#include "InputModeButton.h"
#include "Identity.h"
#include <olectl.h>
#include <vector>

namespace zt {
namespace {
// GUID_LBI_INPUTMODE from msctf.h, spelled out so no translation unit needs INITGUID.
constexpr GUID inputModeItem{0x2c77a81e,0x41cc,0x4178,{0xa3,0xa7,0x5f,0x8a,0x98,0x75,0x68,0xe6}};
constexpr DWORD sinkCookie = 1;
}

InputModeButton::InputModeButton(Action action) : action_(std::move(action)) { ++moduleObjects; }
InputModeButton::~InputModeButton() { if (sink_) sink_->Release(); --moduleObjects; }

void InputModeButton::SetEnglish(bool english) noexcept {
    if (english == english_) return;
    english_ = english;
    Notify(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP);
}
void InputModeButton::SetEnabled(bool enabled) noexcept {
    if (enabled == enabled_) return;
    enabled_ = enabled;
    Notify(TF_LBI_STATUS);
}
void InputModeButton::Notify(DWORD flags) noexcept { if (sink_) sink_->OnUpdate(flags); }

STDMETHODIMP InputModeButton::QueryInterface(REFIID iid, void** out) {
    if (!out) return E_POINTER;
    *out = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfLangBarItem || iid == IID_ITfLangBarItemButton)
        *out = static_cast<ITfLangBarItemButton*>(this);
    else if (iid == IID_ITfSource) *out = static_cast<ITfSource*>(this);
    else return E_NOINTERFACE;
    AddRef();
    return S_OK;
}
STDMETHODIMP_(ULONG) InputModeButton::AddRef() { return ++refs_; }
STDMETHODIMP_(ULONG) InputModeButton::Release() { const auto n = --refs_; if (!n) delete this; return n; }

STDMETHODIMP InputModeButton::GetInfo(TF_LANGBARITEMINFO* info) {
    if (!info) return E_INVALIDARG;
    info->clsidService = serviceId;
    info->guidItem = inputModeItem;
    info->dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY;
    info->ulSort = 0;
    wcscpy_s(info->szDescription, L"中英切換");
    return S_OK;
}
STDMETHODIMP InputModeButton::GetStatus(DWORD* status) {
    if (!status) return E_INVALIDARG;
    *status = enabled_ ? 0 : TF_LBI_STATUS_DISABLED;
    return S_OK;
}
STDMETHODIMP InputModeButton::Show(BOOL) { return E_NOTIMPL; }
STDMETHODIMP InputModeButton::GetTooltipString(BSTR* tooltip) {
    if (!tooltip) return E_INVALIDARG;
    *tooltip = SysAllocString(english_ ? L"英文（按一下切換成中文）" : L"中文（按一下切換成英文）");
    return *tooltip ? S_OK : E_OUTOFMEMORY;
}
STDMETHODIMP InputModeButton::OnClick(TfLBIClick click, POINT point, const RECT*) {
    if (action_) action_(click == TF_LBI_CLK_RIGHT, point);
    return S_OK;
}
STDMETHODIMP InputModeButton::InitMenu(ITfMenu*) { return E_NOTIMPL; }
STDMETHODIMP InputModeButton::OnMenuSelect(UINT) { return E_NOTIMPL; }
STDMETHODIMP InputModeButton::GetIcon(HICON* icon) {
    if (!icon) return E_INVALIDARG;
    // One plain glyph, like Windows' own IMEs; the input method's square logo sits to its right.
    *icon = CreateGlyphIcon(english_ ? L"英" : L"中");
    return *icon ? S_OK : E_FAIL;
}
STDMETHODIMP InputModeButton::GetText(BSTR* text) {
    if (!text) return E_INVALIDARG;
    *text = SysAllocString(english_ ? L"英" : L"中");
    return *text ? S_OK : E_OUTOFMEMORY;
}
STDMETHODIMP InputModeButton::AdviseSink(REFIID iid, IUnknown* sink, DWORD* cookie) {
    if (!sink || !cookie) return E_INVALIDARG;
    if (iid != IID_ITfLangBarItemSink) return CONNECT_E_CANNOTCONNECT;
    if (sink_) return CONNECT_E_ADVISELIMIT;
    if (FAILED(sink->QueryInterface(IID_ITfLangBarItemSink, reinterpret_cast<void**>(&sink_)))) { sink_ = nullptr; return E_NOINTERFACE; }
    *cookie = sinkCookie;
    return S_OK;
}
STDMETHODIMP InputModeButton::UnadviseSink(DWORD cookie) {
    if (cookie != sinkCookie || !sink_) return CONNECT_E_NOCONNECTION;
    sink_->Release();
    sink_ = nullptr;
    return S_OK;
}

bool TaskbarIsLight() noexcept {
    DWORD value = 0, size = sizeof(value);
    const auto status = RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        L"SystemUsesLightTheme", RRF_RT_REG_DWORD, nullptr, &value, &size);
    return status == ERROR_SUCCESS && value != 0;
}

HICON CreateGlyphIcon(const wchar_t* glyph) noexcept {
    const UINT dpi = GetDpiForSystem();
    const int size = GetSystemMetricsForDpi(SM_CXSMICON, dpi);
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = size;
    info.bmiHeader.biHeight = -size;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    void* bits = nullptr;
    HDC screen = GetDC(nullptr);
    HDC dc = CreateCompatibleDC(screen);
    HBITMAP color = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    ReleaseDC(nullptr, screen);
    if (!dc || !color || !bits) { if (color) DeleteObject(color); if (dc) DeleteDC(dc); return nullptr; }
    auto* pixels = static_cast<DWORD*>(bits);
    std::fill(pixels, pixels + size * size, 0u);
    const auto oldBitmap = SelectObject(dc, color);
    // Coverage first: white text on black, without ClearType so every channel carries the same coverage.
    HFONT font = CreateFontW(-MulDiv(size, 15, 16), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY, DEFAULT_PITCH, L"Microsoft JhengHei UI");
    const auto oldFont = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, RGB(255, 255, 255));
    RECT area{0, 0, size, size};
    DrawTextW(dc, glyph, -1, &area, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    SelectObject(dc, oldFont);
    DeleteObject(font);
    SelectObject(dc, oldBitmap);
    DeleteDC(dc);
    // Then the taskbar's text colour, premultiplied by that coverage.
    const BYTE ink = TaskbarIsLight() ? 0x1b : 0xff;
    for (int i = 0; i < size * size; ++i) {
        const BYTE coverage = static_cast<BYTE>(pixels[i] & 0xff);
        const BYTE value = static_cast<BYTE>(ink * coverage / 255);
        pixels[i] = (static_cast<DWORD>(coverage) << 24) | (value << 16) | (value << 8) | value;
    }
    std::vector<BYTE> maskBits(static_cast<size_t>(((size + 15) / 16) * 2 * size), 0);
    HBITMAP mask = CreateBitmap(size, size, 1, 1, maskBits.data());
    ICONINFO icon{TRUE, 0, 0, mask, color};
    HICON result = mask ? CreateIconIndirect(&icon) : nullptr;
    if (mask) DeleteObject(mask);
    DeleteObject(color);
    return result;
}
}

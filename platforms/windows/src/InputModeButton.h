#pragma once
#include <windows.h>
#include <msctf.h>
#include <functional>
#include <string>

namespace zt {
// The 中/英 indicator Windows shows in the taskbar's input indicator (GUID_LBI_INPUTMODE), like the built-in
// Bopomofo IME: a monochrome 中 or 英 drawn for the taskbar's light or dark colour. A left click asks the host to
// switch modes; a right click opens the settings. The button owns no state but what the host last reported.
class InputModeButton final : public ITfLangBarItemButton, public ITfSource {
public:
    using Action = std::function<void(bool rightClick)>;
    explicit InputModeButton(Action action);
    ~InputModeButton();
    InputModeButton(const InputModeButton&) = delete;
    InputModeButton& operator=(const InputModeButton&) = delete;
    /// Shows the host's mode; repaints the taskbar only when it changed.
    void SetEnglish(bool english) noexcept;
    /// Drops the click action before the text service that owns it goes away; TSF may still hold the button.
    void ClearAction() noexcept { action_ = nullptr; }
    /// Hides the button while the field cannot take Chinese input (secure or no host).
    void SetEnabled(bool enabled) noexcept;
    STDMETHODIMP QueryInterface(REFIID iid, void** out) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;
    STDMETHODIMP GetInfo(TF_LANGBARITEMINFO* info) override;
    STDMETHODIMP GetStatus(DWORD* status) override;
    STDMETHODIMP Show(BOOL show) override;
    STDMETHODIMP GetTooltipString(BSTR* tooltip) override;
    STDMETHODIMP OnClick(TfLBIClick click, POINT point, const RECT* area) override;
    STDMETHODIMP InitMenu(ITfMenu* menu) override;
    STDMETHODIMP OnMenuSelect(UINT id) override;
    STDMETHODIMP GetIcon(HICON* icon) override;
    STDMETHODIMP GetText(BSTR* text) override;
    STDMETHODIMP AdviseSink(REFIID iid, IUnknown* sink, DWORD* cookie) override;
    STDMETHODIMP UnadviseSink(DWORD cookie) override;
private:
    void Notify(DWORD flags) noexcept;
    ULONG refs_ = 1;
    Action action_;
    ITfLangBarItemSink* sink_ = nullptr;
    bool english_ = false, enabled_ = true;
};
/// Draws one character as a small monochrome icon in the taskbar's text colour.
HICON CreateGlyphIcon(const wchar_t* glyph) noexcept;
/// True when the taskbar is light (dark glyphs), from the user's personalisation settings.
bool TaskbarIsLight() noexcept;
}

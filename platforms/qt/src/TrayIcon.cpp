#include "TrayIcon.h"
#include <QTimer>

#ifdef Q_OS_WIN
#include <windows.h>
#include <shellapi.h>
#include <algorithm>
#include <vector>

namespace {
constexpr UINT trayMessage = WM_APP + 1;
constexpr UINT trayId = 1;

bool taskbarIsLight() {
    DWORD value = 0, size = sizeof(value);
    return RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
               L"SystemUsesLightTheme", RRF_RT_REG_DWORD, nullptr, &value, &size) == ERROR_SUCCESS && value != 0;
}

// The logo at the small-icon size: drawn four times larger as coverage (white square, black ㄓ), then averaged down
// so the corners and the glyph are antialiased, and painted in the taskbar's text colour.
HICON logoIcon() {
    const UINT dpi = GetDpiForSystem();
    const int size = GetSystemMetricsForDpi(SM_CXSMICON, dpi), scale = 4, big = size * scale;
    BITMAPINFO info{};
    info.bmiHeader = {sizeof(BITMAPINFOHEADER), big, -big, 1, 32, BI_RGB};
    void *bits = nullptr;
    HDC screen = GetDC(nullptr);
    HDC dc = CreateCompatibleDC(screen);
    HBITMAP canvas = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    ReleaseDC(nullptr, screen);
    if (!dc || !canvas || !bits) { if (canvas) DeleteObject(canvas); if (dc) DeleteDC(dc); return nullptr; }
    auto *pixels = static_cast<DWORD *>(bits);
    std::fill(pixels, pixels + big * big, 0u);
    const auto oldBitmap = SelectObject(dc, canvas);
    const int inset = big / 16, radius = big * 5 / 16;
    HBRUSH white = CreateSolidBrush(RGB(255, 255, 255));
    const auto oldBrush = SelectObject(dc, white);
    const auto oldPen = SelectObject(dc, GetStockObject(NULL_PEN));
    RoundRect(dc, inset, inset, big - inset + 1, big - inset + 1, radius, radius);
    HFONT font = CreateFontW(-big * 11 / 16, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                             CLIP_DEFAULT_PRECIS, NONANTIALIASED_QUALITY, DEFAULT_PITCH, L"Microsoft JhengHei UI");
    const auto oldFont = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, RGB(0, 0, 0));
    RECT area{0, 0, big, big};
    DrawTextW(dc, L"ㄓ", -1, &area, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    SelectObject(dc, oldFont); DeleteObject(font);
    SelectObject(dc, oldPen); SelectObject(dc, oldBrush); DeleteObject(white);
    SelectObject(dc, oldBitmap); DeleteDC(dc);

    info.bmiHeader.biWidth = size; info.bmiHeader.biHeight = -size;
    void *smallBits = nullptr;
    screen = GetDC(nullptr);
    HBITMAP color = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &smallBits, nullptr, 0);
    ReleaseDC(nullptr, screen);
    if (!color || !smallBits) { DeleteObject(canvas); if (color) DeleteObject(color); return nullptr; }
    auto *out = static_cast<DWORD *>(smallBits);
    const BYTE ink = taskbarIsLight() ? 0x1b : 0xff;
    for (int y = 0; y < size; ++y)
        for (int x = 0; x < size; ++x) {
            unsigned sum = 0;
            for (int dy = 0; dy < scale; ++dy)
                for (int dx = 0; dx < scale; ++dx) sum += pixels[(y * scale + dy) * big + x * scale + dx] & 0xff;
            const auto alpha = static_cast<BYTE>(sum / (scale * scale));
            const auto value = static_cast<BYTE>(ink * alpha / 255);
            out[y * size + x] = (DWORD(alpha) << 24) | (DWORD(value) << 16) | (DWORD(value) << 8) | value;
        }
    DeleteObject(canvas);
    std::vector<BYTE> maskBits(size_t((size + 15) / 16 * 2 * size), 0);
    HBITMAP mask = CreateBitmap(size, size, 1, 1, maskBits.data());
    ICONINFO icon{TRUE, 0, 0, mask, color};
    HICON result = mask ? CreateIconIndirect(&icon) : nullptr;
    if (mask) DeleteObject(mask);
    DeleteObject(color);
    return result;
}

// Windows 11 puts a new notification-area icon in the overflow (^). The user wants the logo on the taskbar, so the
// first time Explorer records our icon we mark it shown; once the user has decided (IsPromoted exists), we leave it.
void promoteOnce() {
    wchar_t self[MAX_PATH]{};
    if (!GetModuleFileNameW(nullptr, self, MAX_PATH)) return;
    HKEY settings = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, L"Control Panel\\NotifyIconSettings", 0, KEY_READ, &settings) != ERROR_SUCCESS) return;
    wchar_t name[64];
    for (DWORD i = 0;; ++i) {
        DWORD length = 64;
        if (RegEnumKeyExW(settings, i, name, &length, nullptr, nullptr, nullptr, nullptr) != ERROR_SUCCESS) break;
        HKEY entry = nullptr;
        if (RegOpenKeyExW(settings, name, 0, KEY_READ | KEY_SET_VALUE, &entry) != ERROR_SUCCESS) continue;
        wchar_t path[MAX_PATH]{};
        DWORD bytes = sizeof(path);
        const bool ours = RegGetValueW(entry, nullptr, L"ExecutablePath", RRF_RT_REG_SZ, nullptr, path, &bytes) == ERROR_SUCCESS
            && CompareStringOrdinal(path, -1, self, -1, TRUE) == CSTR_EQUAL;
        if (ours && RegGetValueW(entry, nullptr, L"IsPromoted", RRF_RT_REG_DWORD, nullptr, nullptr, nullptr) != ERROR_SUCCESS) {
            const DWORD shown = 1;
            RegSetValueExW(entry, L"IsPromoted", 0, REG_DWORD, reinterpret_cast<const BYTE *>(&shown), sizeof(shown));
        }
        RegCloseKey(entry);
    }
    RegCloseKey(settings);
}

NOTIFYICONDATAW data(HWND window) {
    NOTIFYICONDATAW d{};
    d.cbSize = sizeof(d);
    d.hWnd = window;
    d.uID = trayId;
    return d;
}
}

struct TrayWindow {
    static LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wp, LPARAM lp) {
        auto *self = reinterpret_cast<TrayIcon *>(GetWindowLongPtrW(window, GWLP_USERDATA));
        if (self) {
            if (message == trayMessage && (LOWORD(lp) == WM_LBUTTONUP || LOWORD(lp) == NIN_SELECT || LOWORD(lp) == NIN_KEYSELECT
                                           || LOWORD(lp) == WM_CONTEXTMENU)) {
                emit self->activated();
                return 0;
            }
            // The taskbar was restarted (explorer.exe): add the icon again.
            if (message == self->taskbarCreated_) { self->add(); return 0; }
            // Light/dark taskbar or a DPI change: redraw in the new colour and size.
            if (message == WM_SETTINGCHANGE || message == WM_DPICHANGED || message == WM_DISPLAYCHANGE) self->refresh();
        }
        return DefWindowProcW(window, message, wp, lp);
    }
};

TrayIcon::TrayIcon(QObject *parent) : QObject(parent) {
    WNDCLASSW klass{};
    klass.lpfnWndProc = TrayWindow::procedure;
    klass.hInstance = GetModuleHandleW(nullptr);
    klass.lpszClassName = L"ZhuyinType.Tray";
    RegisterClassW(&klass);
    // A hidden top-level window (not message-only): only those receive TaskbarCreated and WM_SETTINGCHANGE.
    auto *window = CreateWindowExW(WS_EX_TOOLWINDOW, klass.lpszClassName, L"ZhuyinType", WS_POPUP, 0, 0, 0, 0, nullptr, nullptr,
                                   klass.hInstance, nullptr);
    if (!window) return;
    window_ = window;
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(this));
    taskbarCreated_ = RegisterWindowMessageW(L"TaskbarCreated");
    shown_ = add();
    // Explorer writes the icon's settings entry a moment after it first appears.
    if (shown_)
        for (const int delay : {1500, 6000}) QTimer::singleShot(delay, this, [] { promoteOnce(); });
}

bool TrayIcon::add() {
    auto *window = static_cast<HWND>(window_);
    if (icon_) DestroyIcon(static_cast<HICON>(icon_));
    icon_ = logoIcon();
    auto d = data(window);
    d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    d.uCallbackMessage = trayMessage;
    d.hIcon = static_cast<HICON>(icon_);
    wcscpy_s(d.szTip, L"ZhuyinType 注音：按一下開啟設定");
    Shell_NotifyIconW(NIM_DELETE, &d);
    if (!Shell_NotifyIconW(NIM_ADD, &d)) return false;
    d.uVersion = NOTIFYICON_VERSION_4;
    Shell_NotifyIconW(NIM_SETVERSION, &d);
    return true;
}

void TrayIcon::refresh() {
    auto *window = static_cast<HWND>(window_);
    HICON next = logoIcon();
    if (!next) return;
    auto d = data(window);
    d.uFlags = NIF_ICON;
    d.hIcon = next;
    Shell_NotifyIconW(NIM_MODIFY, &d);
    if (icon_) DestroyIcon(static_cast<HICON>(icon_));
    icon_ = next;
}

TrayIcon::~TrayIcon() {
    if (window_) {
        auto d = data(static_cast<HWND>(window_));
        Shell_NotifyIconW(NIM_DELETE, &d);
        DestroyWindow(static_cast<HWND>(window_));
    }
    if (icon_) DestroyIcon(static_cast<HICON>(icon_));
}
#else
TrayIcon::TrayIcon(QObject *parent) : QObject(parent) {}
TrayIcon::~TrayIcon() = default;
#endif

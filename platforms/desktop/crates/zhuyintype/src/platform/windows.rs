//! Windows: the candidate window and the mode toast must never take keyboard focus from the field being typed in.
//!
//! GPUI's `WindowKind::PopUp` on Windows sets only `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`, so a click on the window, or
//! showing it, activates it and the application loses its text focus (and the input method its context).  These
//! functions fix that on the window's HWND (through raw-window-handle):
//!
//! ```ignore
//! // Open the popup hidden and unfocused, then make it non-activating before it is first shown.
//! let options = WindowOptions { kind: WindowKind::PopUp, focus: false, show: false, ..Default::default() };
//! cx.open_window(options, |window, cx| {
//!     #[cfg(windows)]
//!     crate::platform::windows::make_non_activating(window);
//!     cx.new(|cx| CandidateWindow::new(window, cx))
//! })?;
//! // On every UiEvent::Popup: move it next to the caret (logical screen pixels, as PopupState carries them), then
//! // show it without activating it.
//! #[cfg(windows)]
//! {
//!     use crate::platform::windows::{ScreenRect, logical_to_physical, move_window, show_without_activating};
//!     move_window(window, logical_to_physical(ScreenRect { x, y, width, height }));
//!     show_without_activating(window);
//! }
//! ```
//!
//! All of them run on the GPUI main thread (the window's thread) and do nothing if the window has no Win32 handle.

#![allow(unsafe_code)] // Win32 calls on the window's own HWND.

use std::ffi::c_void;

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, MA_NOACTIVATE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_MOUSEACTIVATE,
    WM_NCDESTROY, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::core::BOOL;

/// A rectangle on the desktop.  `move_window` takes physical pixels (what Windows positions windows in, since the
/// app is per-monitor DPI aware); `logical_to_physical` converts from the logical pixels the broker reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

const SUBCLASS_ID: usize = 0x5a54_4e41; // "ZTNA"

fn hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

/// Clicks on the window do not activate it.
unsafe extern "system" fn no_activate(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: called by the system for our subclassed window.
    unsafe {
        if message == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(window, Some(no_activate), SUBCLASS_ID);
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

/// Makes the window a non-activating, topmost tool window: `WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`
/// (no taskbar button, never the foreground window), and `WM_MOUSEACTIVATE` answered with `MA_NOACTIVATE`, so
/// clicking a candidate keeps the focus in the application.  Call once, before the window is first shown.  Returns
/// false if the window has no Win32 handle.
#[allow(clippy::unnecessary_cast)] // GetWindowLongPtrW is GetWindowLongW (i32) on 32-bit Windows.
pub fn make_non_activating(window: &Window) -> bool {
    let Some(window) = hwnd(window) else { return false };
    // SAFETY: the window's own HWND, on its thread.
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as isize;
        let style = (style & !(WS_EX_APPWINDOW.0 as isize))
            | (WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0) as isize;
        SetWindowLongPtrW(window, GWL_EXSTYLE, style as _);
        let _ = SetWindowSubclass(window, Some(no_activate), SUBCLASS_ID, 0);
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    true
}

/// Moves and sizes the window (physical screen pixels) and keeps it on top, without activating it.
pub fn move_window(window: &Window, rect: ScreenRect) -> bool {
    let Some(window) = hwnd(window) else { return false };
    // SAFETY: the window's own HWND, on its thread.
    unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .is_ok()
    }
}

/// Shows the window without activating it (`SW_SHOWNOACTIVATE`); GPUI's own show activates.
pub fn show_without_activating(window: &Window) -> bool {
    let Some(window) = hwnd(window) else { return false };
    // SAFETY: the window's own HWND, on its thread.
    let _ = unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    true
}

/// Logical screen pixels (as the TSF text service reports the caret: `origin + (physical - origin) / scale` on the
/// caret's monitor) back to physical pixels for `move_window`.  The monitor is the one whose logical rectangle
/// holds the rectangle's top-left corner; none (an unplugged monitor) keeps the values.
pub fn logical_to_physical(rect: ScreenRect) -> ScreenRect {
    struct Search {
        point: POINT,
        found: Option<(POINT, f64)>,
    }
    unsafe extern "system" fn visit(monitor: HMONITOR, _dc: HDC, _area: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` is the Search passed below, alive for the enumeration.
        let search = unsafe { &mut *(data.0 as *mut Search) };
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        // SAFETY: a monitor handle from the enumeration.
        unsafe {
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return true.into();
            }
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        }
        let scale = f64::from(dpi_x.max(1)) / 96.0;
        let area = info.rcMonitor;
        let (left, top) = (f64::from(area.left), f64::from(area.top));
        let right = left + f64::from(area.right - area.left) / scale;
        let bottom = top + f64::from(area.bottom - area.top) / scale;
        let (x, y) = (f64::from(search.point.x), f64::from(search.point.y));
        if x >= left && x < right && y >= top && y < bottom {
            search.found = Some((POINT { x: area.left, y: area.top }, scale));
            return false.into();
        }
        true.into()
    }
    let mut search = Search { point: POINT { x: rect.x, y: rect.y }, found: None };
    // SAFETY: the callback only reads monitor information and writes `search`.
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(visit), LPARAM(&raw mut search as isize)) };
    let Some((origin, scale)) = search.found else { return rect };
    let physical = |value: i32, origin: i32| origin + (f64::from(value - origin) * scale).round() as i32;
    ScreenRect {
        x: physical(rect.x, origin.x),
        y: physical(rect.y, origin.y),
        width: (f64::from(rect.width) * scale).round() as i32,
        height: (f64::from(rect.height) * scale).round() as i32,
    }
}

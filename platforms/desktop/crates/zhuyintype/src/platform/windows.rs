//! Windows: the candidate window and the mode badge must never take keyboard focus from the field being typed in,
//! and GPUI can neither move a window that exists nor hide one without closing it.
//!
//! GPUI's `WindowKind::PopUp` on Windows sets only `WS_EX_TOOLWINDOW | WS_EX_TOPMOST` and answers `WM_MOUSEACTIVATE`
//! with `MA_ACTIVATE`, so a click (or showing it) activates it and the application loses its text focus.  These
//! functions work on the window's HWND (through raw-window-handle); `candidate/platform.rs` calls them with the same
//! names and meaning as its X11 versions.  They run on the GPUI main thread, the window's thread, and do nothing
//! (returning false) if the window has no Win32 handle.
//!
//! A subclass of the window (`SetWindowSubclass`) answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE` and, while an
//! input region is set, `WM_NCHITTEST` with `HTTRANSPARENT` outside it (the shadow margin).  With no input region at
//! all (the 中/英 badge) the window is layered and `WS_EX_TRANSPARENT`, so every click goes to what is beneath.

#![allow(unsafe_code)] // Win32 calls on the window's own HWND.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, HTTRANSPARENT, HWND_TOPMOST, LWA_ALPHA, MA_NOACTIVATE, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_MOUSEACTIVATE, WM_NCDESTROY,
    WM_NCHITTEST, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

use crate::candidate::platform::DeviceRect;

const SUBCLASS_ID: usize = 0x5a54_4e41; // "ZTNA"

thread_local! {
    /// The input region of each subclassed window (window-relative device pixels), while one is set.
    static REGIONS: RefCell<HashMap<isize, RECT>> = RefCell::new(HashMap::new());
}

fn hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

fn key(window: HWND) -> isize {
    window.0 as isize
}

/// Ex-style of the window, with `add` set and `remove` cleared.
#[allow(clippy::unnecessary_cast)] // GetWindowLongPtrW is GetWindowLongW (i32) on 32-bit Windows.
fn update_ex_style(window: HWND, add: u32, remove: u32) {
    // SAFETY: the window's own HWND, on its thread.
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as isize;
        let style = (style & !(remove as isize)) | add as isize;
        SetWindowLongPtrW(window, GWL_EXSTYLE, style as _);
    }
}

unsafe extern "system" fn subclass(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: called by the system for our subclassed window, on its thread.
    unsafe {
        match message {
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_NCHITTEST => {
                let region = REGIONS.with(|regions| regions.borrow().get(&key(window)).copied());
                if let Some(region) = region {
                    let mut frame = RECT::default();
                    let _ = GetWindowRect(window, &mut frame);
                    // Screen coordinates, signed 16-bit each.
                    let x = i32::from(lparam.0 as u32 as u16 as i16) - frame.left;
                    let y = i32::from((lparam.0 as u32 >> 16) as u16 as i16) - frame.top;
                    if x < region.left || x >= region.right || y < region.top || y >= region.bottom {
                        return LRESULT(HTTRANSPARENT as isize);
                    }
                }
            }
            WM_NCDESTROY => {
                REGIONS.with(|regions| regions.borrow_mut().remove(&key(window)));
                let _ = RemoveWindowSubclass(window, Some(subclass), SUBCLASS_ID);
            }
            _ => {}
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

/// Never lets the window take focus: `WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW` (no taskbar button,
/// never the foreground window) and `WM_MOUSEACTIVATE` answered with `MA_NOACTIVATE`, so clicking a candidate keeps
/// the focus in the application.  Called once after the window opens; calling it again changes nothing.
pub fn make_non_activating(window: &Window) -> bool {
    let Some(window) = hwnd(window) else { return false };
    update_ex_style(window, (WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW).0, WS_EX_APPWINDOW.0);
    // SAFETY: the window's own HWND; the same subclass id replaces rather than stacks.
    unsafe {
        let _ = SetWindowSubclass(window, Some(subclass), SUBCLASS_ID, 0);
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

/// Moves and sizes the window (device pixels, i.e. physical: the app is per-monitor DPI aware) on top of other
/// windows, without activating it.
pub fn place(window: &Window, rect: DeviceRect) -> bool {
    let Some(window) = hwnd(window) else { return false };
    let size = |value: u32| i32::try_from(value.max(1)).unwrap_or(i32::MAX);
    // SAFETY: the window's own HWND, on its thread.
    unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            size(rect.width),
            size(rect.height),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .is_ok()
    }
}

/// Shows (`SW_SHOWNOACTIVATE`) or hides (`SW_HIDE`) the window without closing or activating it.
pub fn set_visible(window: &Window, visible: bool) -> bool {
    let Some(window) = hwnd(window) else { return false };
    // SAFETY: the window's own HWND; the return value is the previous visibility, not an error.
    let _ = unsafe { ShowWindow(window, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
    true
}

/// Clicks reach only `region` (window-relative device pixels); outside it (the shadow) the window is transparent to
/// the mouse.  `None`: every click goes through the window (the 中/英 badge).
pub fn set_input_region(window: &Window, region: Option<DeviceRect>) -> bool {
    let Some(window) = hwnd(window) else { return false };
    // SAFETY: the window's own HWND; the subclass is (re)installed with the same id.
    let _ = unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS_ID, 0) };
    match region {
        Some(rect) => {
            let right = rect.x.saturating_add(i32::try_from(rect.width).unwrap_or(i32::MAX));
            let bottom = rect.y.saturating_add(i32::try_from(rect.height).unwrap_or(i32::MAX));
            let region = RECT { left: rect.x, top: rect.y, right, bottom };
            REGIONS.with(|regions| regions.borrow_mut().insert(key(window), region));
            update_ex_style(window, 0, WS_EX_TRANSPARENT.0);
        }
        None => {
            REGIONS.with(|regions| regions.borrow_mut().remove(&key(window)));
            // Click-through across processes needs a layered window; fully opaque, so it draws as before.
            update_ex_style(window, (WS_EX_LAYERED | WS_EX_TRANSPARENT).0, 0);
            // SAFETY: the window's own HWND, now layered.
            let _ = unsafe { SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA) };
        }
    }
    true
}

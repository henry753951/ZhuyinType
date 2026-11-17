//! What GPUI does not offer for the floating windows: moving a window that exists, hiding it without closing it,
//! letting clicks through its shadow, and keeping it from ever taking focus.
//!
//! - X11: GPUI's `WindowKind::PopUp` is an override-redirect window (the window manager never focuses, decorates or
//!   places it), typed `_NET_WM_WINDOW_TYPE_NOTIFICATION`.  GPUI only resizes windows, so this module moves, maps and
//!   unmaps it with its own x11rb connection on the window's XID (from `raw-window-handle`); GPUI hears the
//!   ConfigureNotify, MapNotify and UnmapNotify like any other and keeps its bounds and visibility in step.  The input
//!   shape is cut to the surface, so the transparent margin around it (the shadow) never catches a click.
//! - Windows: GPUI gives PopUp `WS_EX_TOOLWINDOW | WS_EX_TOPMOST` but answers `WM_MOUSEACTIVATE` with `MA_ACTIVATE`,
//!   so a click would activate it.  `platform/windows.rs` adds `WS_EX_NOACTIVATE`, answers `MA_NOACTIVATE`, moves
//!   with `SetWindowPos(SWP_NOACTIVATE)`, hides and shows with `SW_HIDE`/`SW_SHOWNOACTIVATE`, and makes the shadow
//!   (or the whole badge) transparent to clicks.
//! - Wayland: a client cannot place its own windows; these return false and the popup reopens its window (the
//!   compositor places it).  Fcitx5 draws its own candidate window there when ours cannot follow the caret.

use gpui_kit::Window;

/// A window rectangle in device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg(target_os = "linux")]
mod x11 {
    use std::cell::OnceCell;

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
    use x11rb::protocol::xproto::{ClipOrdering, ConfigureWindowAux, ConnectionExt as _, Rectangle, StackMode};
    use x11rb::rust_connection::RustConnection;

    use super::DeviceRect;

    thread_local! {
        static CONNECTION: OnceCell<Option<(RustConnection, usize)>> = const { OnceCell::new() };
    }

    fn with<R>(f: impl FnOnce(&RustConnection, usize) -> Option<R>) -> Option<R> {
        CONNECTION.with(|cell| {
            let connection = cell.get_or_init(|| {
                x11rb::connect(None).map_err(|error| log::debug!("no X11 connection for the popups: {error}")).ok()
            });
            connection.as_ref().and_then(|(connection, screen)| f(connection, *screen))
        })
    }

    /// The window's XID, when GPUI runs on X11.
    pub fn xid(window: &gpui_kit::Window) -> Option<u32> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
            RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
            _ => None,
        }
    }

    pub fn place(xid: u32, rect: DeviceRect) -> bool {
        with(|connection, _| {
            let aux = ConfigureWindowAux::new()
                .x(rect.x)
                .y(rect.y)
                .width(rect.width.max(1))
                .height(rect.height.max(1))
                .stack_mode(StackMode::ABOVE);
            connection.configure_window(xid, &aux).ok()?;
            connection.flush().ok()
        })
        .is_some()
    }

    pub fn set_mapped(xid: u32, mapped: bool) -> bool {
        with(|connection, _| {
            if mapped {
                connection.map_window(xid).ok()?;
                connection.configure_window(xid, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE)).ok()?;
            } else {
                connection.unmap_window(xid).ok()?;
            }
            connection.flush().ok()
        })
        .is_some()
    }

    pub fn set_input_region(xid: u32, region: Option<DeviceRect>) -> bool {
        with(|connection, _| {
            let rectangles: Vec<Rectangle> = region
                .map(|r| Rectangle {
                    x: r.x as i16,
                    y: r.y as i16,
                    width: r.width.min(u16::MAX as u32) as u16,
                    height: r.height.min(u16::MAX as u32) as u16,
                })
                .into_iter()
                .collect();
            connection.shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, xid, 0, 0, &rectangles).ok()?;
            connection.flush().ok()
        })
        .is_some()
    }

    /// A compositing manager runs (transparent windows show what is behind them; without one they show black).
    pub fn compositing() -> bool {
        with(|connection, screen| {
            let name = format!("_NET_WM_CM_S{screen}");
            let atom = connection.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom;
            Some(connection.get_selection_owner(atom).ok()?.reply().ok()?.owner != 0)
        })
        .unwrap_or(false)
    }
}

/// The window is an X11 one: carets from Fcitx are then in its device pixels (`popup::in_logical_pixels`).
pub fn x11_pixels(window: &Window) -> bool {
    #[cfg(target_os = "linux")]
    {
        x11::xid(window).is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        false
    }
}

/// Moves and sizes `window` (device pixels) without activating it.  False: this platform cannot; reopen it there.
pub fn place(window: &Window, rect: DeviceRect) -> bool {
    #[cfg(target_os = "linux")]
    {
        x11::xid(window).is_some_and(|xid| x11::place(xid, rect))
    }
    #[cfg(windows)]
    {
        crate::platform::windows::place(window, rect)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (window, rect);
        false
    }
}

/// Shows or hides `window` without closing it and without activating it.  False: this platform cannot.
pub fn set_visible(window: &Window, visible: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        x11::xid(window).is_some_and(|xid| x11::set_mapped(xid, visible))
    }
    #[cfg(windows)]
    {
        crate::platform::windows::set_visible(window, visible)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (window, visible);
        false
    }
}

/// Clicks reach only `region` (window-relative device pixels); None lets every click through (the 中/英 badge).
pub fn set_input_region(window: &Window, region: Option<DeviceRect>) {
    #[cfg(target_os = "linux")]
    {
        if let Some(xid) = x11::xid(window) {
            x11::set_input_region(xid, region);
        }
    }
    #[cfg(windows)]
    {
        crate::platform::windows::set_input_region(window, region);
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (window, region);
    }
}

/// Never let the window take focus: called once after it opens.
pub fn make_non_activating(window: &Window) {
    #[cfg(windows)]
    {
        crate::platform::windows::make_non_activating(window);
    }
    // X11: override-redirect windows are never focused by the window manager, and nothing here asks for focus.
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

/// Whether transparent windows show what is behind them (rounded corners and a shadow need it).
pub fn transparent_windows() -> bool {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return true;
        }
        x11::compositing()
    }
    // Windows: the popup is square and edge to edge; DWM rounds it and draws its shadow like the system's own
    // menus (`platform::windows::make_non_activating`), which needs no transparent window.
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// The system draws the floating windows' edge: Windows rounds them (DWM, small corners) and draws their border and
/// shadow like the system's own menus, so the surface is square and edge to edge with no border of its own.
pub fn system_frames_popups() -> bool {
    cfg!(windows)
}

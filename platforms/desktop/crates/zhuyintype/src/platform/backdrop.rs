//! The settings and wizard windows' backdrop (「透明效果」) and their light or dark frame (「主題」).
//!
//! - Windows 11 22H2 on: Mica (`DWMSBT_MAINWINDOW`) behind the whole window, GPUI clearing to transparent; the frame
//!   and the page paint their tints over it (`theme::Palette::new`).  DWM tints Mica and draws the border light or
//!   dark from `DWMWA_USE_IMMERSIVE_DARK_MODE`, which follows the 主題 rather than the system.  Mica shows only while
//!   the window is active; DWM draws a solid colour otherwise.
//! - KDE Plasma: GPUI's `Blurred` (Wayland: the background-effect or KDE blur protocol, cut to the rounded frame);
//!   on X11 also `_KDE_NET_WM_BLUR_BEHIND_REGION` over the frame (inside the shadow), which KWin blurs.
//! - Elsewhere the window is opaque.
//!
//! [`apply`] runs in every render of those windows and touches the window system only when the palette, the system
//! appearance or (X11) the size changed.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui_kit::{App, Window, WindowId};
use zt_core::settings::Settings;

use crate::theme::{self, Material};

/// The backdrop the settings ask for, where this system has one.
pub fn material(settings: &Settings) -> Material {
    if !zt_core::appearance::transparency(settings) {
        return Material::Opaque;
    }
    // Screenshots on a virtual display without a compositor: draw as over KDE's blur (what is behind shows black).
    #[cfg(target_os = "linux")]
    if std::env::var("ZT_SHOT_MATERIAL").as_deref() == Ok("blur") {
        return Material::Blur;
    }
    #[cfg(windows)]
    {
        Material::Mica
    }
    #[cfg(target_os = "linux")]
    {
        if linux::blur_possible() { Material::Blur } else { Material::Opaque }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Material::Opaque
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Applied {
    dark: bool,
    material: Material,
    /// GPUI sets the dark-mode attribute to the system's when that changes (Windows): apply ours again.
    system_dark: bool,
    /// Device pixels (the X11 blur region follows the size).
    size: (i32, i32),
}

thread_local! {
    static APPLIED: RefCell<HashMap<WindowId, Applied>> = RefCell::new(HashMap::new());
}

/// Gives `window` the palette's light or dark frame and backdrop.  Call in `render`, after `theme::sync`.
pub fn apply(window: &mut Window, cx: &App) {
    let p = theme::palette(cx);
    let scale = window.scale_factor();
    let viewport = window.viewport_size();
    let applied = Applied {
        dark: p.dark,
        material: p.material,
        system_dark: theme::system_dark(window),
        size: ((f32::from(viewport.width) * scale) as i32, (f32::from(viewport.height) * scale) as i32),
    };
    let id = window.window_handle().window_id();
    if APPLIED.with(|map| map.borrow().get(&id) == Some(&applied)) {
        return;
    }
    APPLIED.with(|map| map.borrow_mut().insert(id, applied));
    #[cfg(windows)]
    crate::platform::windows::set_backdrop(window, p.dark, p.material == Material::Mica);
    #[cfg(target_os = "linux")]
    linux::set_blur(window, p.material == Material::Blur);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = window;
}

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::OnceCell;
    use std::sync::OnceLock;

    use gpui_kit::{Decorations, Window, WindowBackgroundAppearance};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    use crate::theme::size;

    thread_local! {
        static CONNECTION: OnceCell<Option<(RustConnection, usize)>> = const { OnceCell::new() };
    }

    fn with<R>(f: impl FnOnce(&RustConnection, usize) -> Option<R>) -> Option<R> {
        CONNECTION.with(|cell| {
            let connection = cell.get_or_init(|| x11rb::connect(None).ok());
            connection.as_ref().and_then(|(connection, screen)| f(connection, *screen))
        })
    }

    /// GPUI runs on Wayland (it prefers it whenever the session has one).
    fn wayland() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
    }

    /// Transparent windows show what is behind them: always on Wayland; on X11 while a compositor runs (KWin's
    /// compositing can be turned off, and transparent windows are then black).
    pub(super) fn blur_possible() -> bool {
        static X11_COMPOSITING: OnceLock<bool> = OnceLock::new();
        wayland()
            || *X11_COMPOSITING.get_or_init(|| {
                with(|connection, screen| {
                    let name = format!("_NET_WM_CM_S{screen}");
                    let atom = connection.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom;
                    Some(connection.get_selection_owner(atom).ok()?.reply().ok()?.owner != 0)
                })
                .unwrap_or(false)
            })
    }

    fn xid(window: &Window) -> Option<u32> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
            RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
            _ => None,
        }
    }

    /// The frame inside the client-side shadow, as rectangles (device pixels) whose rows follow its rounded corners.
    fn frame_region(window: &Window) -> Vec<u32> {
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width) * scale, f32::from(viewport.height) * scale);
        let (inset, radius) = match window.window_decorations() {
            Decorations::Client { tiling } if !(tiling.top || tiling.bottom || tiling.left || tiling.right) => {
                let inset = window.client_inset().map_or(0., f32::from) * scale;
                (inset, f32::from(size::R_XL) * scale)
            }
            _ => (0., 0.),
        };
        let (x, y) = (inset.round(), inset.round());
        let (w, h) = ((width - inset * 2.).round().max(1.), (height - inset * 2.).round().max(1.));
        let r = radius.round().min(w / 2.).min(h / 2.);
        let mut rects = Vec::new();
        let mut push = |rx: f32, ry: f32, rw: f32, rh: f32| {
            if rw > 0. && rh > 0. {
                rects.extend([rx as u32, ry as u32, rw as u32, rh as u32]);
            }
        };
        for row in 0..r as i32 {
            let dy = r - row as f32 - 0.5;
            let dx = (r - (r * r - dy * dy).max(0.).sqrt()).round();
            push(x + dx, y + row as f32, w - dx * 2., 1.);
            push(x + dx, y + h - 1. - row as f32, w - dx * 2., 1.);
        }
        push(x, y + r, w, h - r * 2.);
        rects
    }

    pub(super) fn set_blur(window: &mut Window, blur: bool) {
        window.set_background_appearance(if blur {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        });
        if wayland() {
            return;
        }
        let Some(xid) = xid(window) else { return };
        let region = blur.then(|| frame_region(window));
        let done = with(|connection, _| {
            let atom = connection.intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION").ok()?.reply().ok()?.atom;
            match &region {
                Some(rects) => {
                    connection.change_property32(PropMode::REPLACE, xid, atom, AtomEnum::CARDINAL, rects).ok()?;
                }
                None => {
                    connection.delete_property(xid, atom).ok()?;
                }
            }
            connection.flush().ok()
        });
        if done.is_none() {
            log::debug!("no X11 connection for the blur region");
        }
    }
}

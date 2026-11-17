//! The system's accessibility look for the windows this app draws (`zt_panel::a11y`): its accent colour, high
//! contrast, and whether a screen reader runs.  The Windows text service reads its own copy in the application's
//! process (`zt-tsf` `system_look`); this one is for the GPUI app (Linux's candidate window, every window's accent and
//! the previews).
//!
//! - **Linux**: the XDG desktop portal's `org.freedesktop.appearance` `accent-color` and `contrast`; else GNOME's
//!   `accent-color` (GNOME 47 on) and 高對比 (`org.gnome.desktop.a11y.interface high-contrast`); KDE's `AccentColor`
//!   (or its colour scheme's selection) in `kdeglobals`.  A screen reader: AT-SPI's `org.a11y.Status`
//!   `ScreenReaderEnabled`, or GNOME's `screen-reader-enabled`.
//! - **Windows**: `HKCU\Software\Microsoft\Windows\DWM` `AccentColor`; high contrast from `HKCU\Control Panel\
//!   Accessibility\HighContrast` `Flags` with the theme's colours from `HKCU\Control Panel\Colors`.
//!
//! [`watch`] reads it every few seconds and calls back when it changes.

use std::sync::Mutex;
use std::time::Duration;

use zt_panel::a11y::{AccentChoice, Look, SystemColors};
use zt_panel::tokens::Rgba;

use crate::settings::Settings;

/// 「強調色」.
#[must_use]
pub fn accent(settings: &Settings) -> AccentChoice {
    AccentChoice::from_setting(&settings.string("accentColor"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    accent: Option<Rgba>,
    /// High contrast is on; `colors`: the theme's own colours where the system gives them.
    high_contrast: bool,
    colors: Option<SystemColors>,
    screen_reader: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> State {
    let Ok(mut cached) = STATE.lock() else { return read() };
    *cached.get_or_insert_with(read)
}

/// The system's look for a window that is `dark` (high contrast without the system's own colours uses
/// `zt_panel::a11y::high_contrast_palette`).
#[must_use]
pub fn look(dark: bool) -> Look {
    let state = state();
    let high_contrast =
        state.high_contrast.then(|| state.colors.unwrap_or_else(|| zt_panel::a11y::high_contrast_palette(dark)));
    Look { accent: state.accent, high_contrast }
}

/// A screen reader runs (Linux: the candidate list is then shown by Fcitx, whose list screen readers read).
#[must_use]
pub fn screen_reader() -> bool {
    state().screen_reader
}

/// Calls `changed` (on its own thread) whenever the accent, high contrast or the screen reader changes.  Call once.
pub fn watch(changed: impl Fn() + Send + 'static) {
    let spawned = std::thread::Builder::new().name("zt-system-look".into()).spawn(move || {
        loop {
            let now = read();
            let before = STATE.lock().ok().and_then(|mut cached| cached.replace(now));
            if before.is_some_and(|before| before != now) {
                changed();
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    if let Err(error) = spawned {
        log::warn!("watching the system look: {error}");
    }
}

#[cfg(target_os = "linux")]
fn read() -> State {
    linux::read()
}

#[cfg(windows)]
fn read() -> State {
    windows::read()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn read() -> State {
    State::default()
}

/// `r,g,b` (0…255), as KDE writes colours.
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "Linux and tests"))]
fn parse_rgb_list(text: &str) -> Option<Rgba> {
    let parts: Vec<u8> = text.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [r, g, b] = parts.get(..3)?.try_into().ok()?;
    Some(Rgba::hex((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b), 1.))
}

/// `r g b` (0…255), as Windows writes `Control Panel\Colors`.
#[cfg_attr(not(windows), allow(dead_code, reason = "Windows and tests"))]
fn parse_rgb_spaced(text: &str) -> Option<Rgba> {
    parse_rgb_list(&text.split_whitespace().collect::<Vec<_>>().join(","))
}

/// The portal's `accent-color`, as gdbus prints it: `(<(0.2078, 0.5176, 0.8941)>,)`.  Components outside 0…1 mean
/// "no accent" (the portal's convention).
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "Linux and tests"))]
fn parse_portal_accent(text: &str) -> Option<Rgba> {
    let inner: String = text.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | 'e')).collect();
    let values: Vec<f32> = inner.split(',').filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let [r, g, b] = values.get(..3)?.try_into().ok()?;
    let valid = |c: f32| c.is_finite() && (0. ..=1.).contains(&c);
    (valid(r) && valid(g) && valid(b)).then_some(Rgba { r, g, b, a: 1. })
}

/// GNOME's named accent colours (`org.gnome.desktop.interface accent-color`, GNOME 47 on), as libadwaita draws them.
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "Linux and tests"))]
fn gnome_accent(name: &str) -> Option<Rgba> {
    let hex = match name.trim().trim_matches('\'') {
        "blue" => 0x3584e4,
        "teal" => 0x2190a4,
        "green" => 0x3a944a,
        "yellow" => 0xc88800,
        "orange" => 0xed5b00,
        "red" => 0xe62d42,
        "pink" => 0xd56199,
        "purple" => 0x9141ac,
        "slate" => 0x6f8396,
        _ => return None,
    };
    Some(Rgba::hex(hex, 1.))
}

/// KDE's accent in `kdeglobals`: `[General] AccentColor`, else the colour scheme's selection.
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "Linux and tests"))]
fn kde_accent(kdeglobals: &str) -> Option<Rgba> {
    let (mut section, mut accent, mut selection) = ("", None, None);
    for line in kdeglobals.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
        } else if let Some((key, value)) = line.split_once('=') {
            match (section, key.trim()) {
                ("[General]", "AccentColor") => accent = parse_rgb_list(value),
                ("[Colors:Selection]", "BackgroundNormal") => selection = parse_rgb_list(value),
                _ => {}
            }
        }
    }
    accent.or(selection)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};

    use super::{State, gnome_accent, kde_accent, parse_portal_accent};

    fn output(program: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    fn portal(key: &str) -> Option<String> {
        output(
            "gdbus",
            &[
                "call",
                "--session",
                "--timeout",
                "2",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                "/org/freedesktop/portal/desktop",
                "--method",
                "org.freedesktop.portal.Settings.ReadOne",
                "org.freedesktop.appearance",
                key,
            ],
        )
    }

    fn gsettings(schema: &str, key: &str) -> Option<String> {
        output("gsettings", &["get", schema, key])
    }

    fn kdeglobals() -> Option<String> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config")))?;
        std::fs::read_to_string(config.join("kdeglobals")).ok()
    }

    fn accent() -> Option<super::Rgba> {
        if let Some(color) = portal("accent-color").as_deref().and_then(parse_portal_accent) {
            return Some(color);
        }
        if let Some(color) = gsettings("org.gnome.desktop.interface", "accent-color").as_deref().and_then(gnome_accent)
        {
            return Some(color);
        }
        let kde = std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|desktop| desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE")));
        kde.then(kdeglobals).flatten().as_deref().and_then(kde_accent)
    }

    fn high_contrast() -> bool {
        // The portal's `contrast`: 1 is "higher contrast".
        if portal("contrast").is_some_and(|value| value.contains("uint32 1")) {
            return true;
        }
        gsettings("org.gnome.desktop.a11y.interface", "high-contrast").is_some_and(|v| v == "true")
    }

    fn screen_reader() -> bool {
        let status = output(
            "gdbus",
            &[
                "call",
                "--session",
                "--timeout",
                "2",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.freedesktop.DBus.Properties.Get",
                "org.a11y.Status",
                "ScreenReaderEnabled",
            ],
        );
        status.is_some_and(|s| s.contains("true"))
            || gsettings("org.gnome.desktop.a11y.applications", "screen-reader-enabled").is_some_and(|v| v == "true")
    }

    pub(super) fn read() -> State {
        // ZT_HIGH_CONTRAST / ZT_ACCENT (#rrggbb) / ZT_SCREEN_READER: forced, for screenshots and tests.
        let forced = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let accent = match forced("ZT_ACCENT") {
            Some(hex) if hex == "none" => None,
            Some(hex) => u32::from_str_radix(hex.trim_start_matches('#'), 16).ok().map(|v| super::Rgba::hex(v, 1.)),
            None => accent(),
        };
        let high_contrast = forced("ZT_HIGH_CONTRAST").map_or_else(high_contrast, |v| v == "1");
        let screen_reader = forced("ZT_SCREEN_READER").map_or_else(screen_reader, |v| v == "1");
        State { accent, high_contrast, colors: None, screen_reader }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // Registry reads.
mod windows {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
    use zt_panel::a11y::SystemColors;
    use zt_panel::tokens::Rgba;

    use super::{State, parse_rgb_spaced};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn dword(key: &str, name: &str) -> Option<u32> {
        let (key, name) = (wide(key), wide(name));
        let mut value: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: NUL-terminated names; a DWORD read into a DWORD of the given size.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&raw mut value).cast::<c_void>(),
                &raw mut size,
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    fn string(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut buffer = [0u16; 64];
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: NUL-terminated names; a string read into a buffer of the given size.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast::<c_void>(),
                &raw mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end]))
    }

    pub(super) fn read() -> State {
        // AccentColor is 0xAABBGGRR.
        let accent = dword("Software\\Microsoft\\Windows\\DWM", "AccentColor")
            .map(|v| Rgba::hex(((v & 0xff) << 16) | (v & 0xff00) | ((v >> 16) & 0xff), 1.));
        // HCF_HIGHCONTRASTON is bit 0 of the flags.
        let high_contrast = string("Control Panel\\Accessibility\\HighContrast", "Flags")
            .and_then(|flags| flags.trim().parse::<u32>().ok())
            .is_some_and(|flags| flags & 1 != 0);
        let color = |name: &str| string("Control Panel\\Colors", name).as_deref().and_then(parse_rgb_spaced);
        let colors = high_contrast
            .then(|| {
                Some(SystemColors {
                    window: color("Window")?,
                    text: color("WindowText")?,
                    highlight: color("Hilight")?,
                    highlight_text: color("HilightText")?,
                    gray_text: color("GrayText")?,
                })
            })
            .flatten();
        State { accent, high_contrast, colors, screen_reader: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_systems_colours() {
        assert_eq!(parse_rgb_list("61,174,233"), Some(Rgba::hex(0x3daee9, 1.)));
        assert_eq!(parse_rgb_list("61, 174"), None);
        assert_eq!(parse_rgb_list("300,0,0"), None);
        assert_eq!(parse_rgb_spaced("0 120 215"), Some(Rgba::hex(0x0078d7, 1.)));
        let portal = parse_portal_accent("(<(1.0, 0.5, 0.0)>,)").unwrap();
        assert!((portal.r - 1.).abs() < 1e-6 && (portal.g - 0.5).abs() < 1e-6 && portal.b == 0.);
        // Out of range: the desktop has no accent.
        assert_eq!(parse_portal_accent("(<(-1.0, -1.0, -1.0)>,)"), None);
        assert_eq!(parse_portal_accent("Error: no such key"), None);
        assert_eq!(gnome_accent("'teal'"), Some(Rgba::hex(0x2190a4, 1.)));
        assert_eq!(gnome_accent("'beige'"), None);
    }

    #[test]
    fn kde_prefers_its_accent_over_the_selection() {
        let both = "[Colors:Selection]\nBackgroundNormal=61,174,233\n[General]\nAccentColor=233,100,10\n";
        assert_eq!(kde_accent(both), Some(Rgba::hex(0xe9640a, 1.)));
        assert_eq!(kde_accent("[Colors:Selection]\nBackgroundNormal=61,174,233\n"), Some(Rgba::hex(0x3daee9, 1.)));
        assert_eq!(kde_accent("[General]\nColorScheme=BreezeDark\n"), None);
    }

    #[test]
    fn the_setting_reads_back() {
        let settings = Settings::open(None);
        assert_eq!(accent(&settings), AccentChoice::System);
        settings.set("accentColor", serde_json::Value::from("zhuyintype")).unwrap();
        assert_eq!(accent(&settings), AccentChoice::Zhuyintype);
        assert!(settings.set("accentColor", serde_json::Value::from("pink")).is_err());
    }
}

//! 主題 and 透明效果 (「視窗與提示 › 外觀」): the user's choice, and the system settings they follow.
//!
//! - **主題** (`theme`): 跟隨系統, 淺色 or 深色, for every window the app draws.  The system's light or dark comes from
//!   the window system itself (GPUI: Windows' `AppsUseLightTheme`, the XDG desktop portal's `color-scheme`); on a
//!   Linux session without the portal [`system_dark_fallback`] reads GNOME's or KDE's own setting.
//! - **透明效果** (`transparency`): 跟隨系統, 開啟 or 關閉.  The system's own switch is Windows' 「透明效果」
//!   (`EnableTransparency`); KDE Plasma has none, so there 跟隨系統 means on.  Where the system has no backdrop the app
//!   can use ([`backdrop_support`]) the windows stay opaque whatever the setting says.
//!
//! [`watch`] calls back when one of the system settings changes, so both follow it live.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::settings::Settings;

/// A three-way choice: the system's, or forced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    System,
    Light,
    Dark,
}

impl Choice {
    /// Dark, when the system's own appearance is `system_dark`.
    #[must_use]
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// 「主題」.
#[must_use]
pub fn theme(settings: &Settings) -> Choice {
    match settings.string("theme").as_str() {
        "light" => Choice::Light,
        "dark" => Choice::Dark,
        _ => Choice::System,
    }
}

/// 「候選字視窗」: its own choice, else the 主題.
#[must_use]
pub fn panel_theme(settings: &Settings) -> Choice {
    match settings.string("panelAppearance").as_str() {
        "light" => Choice::Light,
        "dark" => Choice::Dark,
        _ => theme(settings),
    }
}

/// Whether this system has a backdrop the windows can use; Err: why not, as the settings row says it.
pub fn backdrop_support() -> Result<(), &'static str> {
    #[cfg(windows)]
    {
        // DWMWA_SYSTEMBACKDROP_TYPE (Mica, acrylic) is Windows 11 22H2, build 22621.
        if windows::build() >= 22621 { Ok(()) } else { Err("需要 Windows 11 22H2 或更新的版本") }
    }
    #[cfg(target_os = "linux")]
    {
        if linux::kde() { Ok(()) } else { Err("只在 KDE Plasma 上提供") }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("這個系統不提供")
    }
}

/// The system's own 透明效果 switch (Windows), or true where it has none.
#[must_use]
pub fn system_transparency() -> bool {
    #[cfg(windows)]
    {
        cached(&SYSTEM_TRANSPARENCY, windows::transparency)
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Whether the windows show the system's backdrop: the user's 透明效果 (or the system's), where there is one.
#[must_use]
pub fn transparency(settings: &Settings) -> bool {
    let wanted = match settings.string("transparency").as_str() {
        "on" => true,
        "off" => false,
        _ => system_transparency(),
    };
    wanted && backdrop_support().is_ok()
}

/// Linux without the XDG desktop portal's `color-scheme`: the desktop's own light or dark (GNOME's `color-scheme` or
/// GTK theme, KDE's colour scheme), as [`watch`] last read it.  None: the portal answers (GPUI follows it), or this
/// is not Linux.
#[must_use]
pub fn system_dark_fallback() -> Option<bool> {
    match FALLBACK_DARK.load(Ordering::Relaxed) {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

/// Calls `changed` (on its own thread) whenever a system setting behind 主題 or 透明效果 changes: Windows'
/// personalisation key (`AppsUseLightTheme`, `EnableTransparency`); on Linux without the portal, the desktop's own
/// setting.  Call once.
pub fn watch(changed: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    {
        let spawned = std::thread::Builder::new().name("zt-appearance".into()).spawn(move || {
            windows::watch(|| {
                SYSTEM_TRANSPARENCY.store(stored(windows::transparency()), Ordering::Relaxed);
                changed();
            });
        });
        if let Err(error) = spawned {
            log::warn!("watching the system appearance: {error}");
        }
    }
    #[cfg(target_os = "linux")]
    {
        let spawned = std::thread::Builder::new().name("zt-appearance".into()).spawn(move || linux::watch(changed));
        if let Err(error) = spawned {
            log::warn!("watching the system appearance: {error}");
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = changed;
    }
}

/// 0: not read yet; 1: false; 2: true.
#[cfg(windows)]
static SYSTEM_TRANSPARENCY: AtomicU8 = AtomicU8::new(0);
static FALLBACK_DARK: AtomicU8 = AtomicU8::new(0);

fn stored(value: bool) -> u8 {
    if value { 2 } else { 1 }
}

#[cfg(windows)]
fn cached(cell: &AtomicU8, read: impl FnOnce() -> bool) -> bool {
    match cell.load(Ordering::Relaxed) {
        0 => {
            let value = read();
            cell.store(stored(value), Ordering::Relaxed);
            value
        }
        value => value == 2,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // Registry reads and change notifications.
mod windows {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET,
        RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
    };

    const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn dword(root: HKEY, key: &str, name: &str) -> Option<u32> {
        let (key, name) = (wide(key), wide(name));
        let mut value: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: NUL-terminated names; a DWORD read into a DWORD of the given size.
        let status = unsafe {
            RegGetValueW(
                root,
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

    /// Settings › Personalization › Colors › 透明效果 (on when the value is missing, as Windows treats it).
    pub(super) fn transparency() -> bool {
        dword(HKEY_CURRENT_USER, PERSONALIZE, "EnableTransparency") != Some(0)
    }

    /// The Windows build number (0 when it cannot be read), read once.
    pub(super) fn build() -> u32 {
        static BUILD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *BUILD.get_or_init(read_build)
    }

    fn read_build() -> u32 {
        let (key, name) = (wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"), wide("CurrentBuildNumber"));
        let mut buffer = [0u16; 16];
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: NUL-terminated names; a string read into a buffer of the given size.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast::<c_void>(),
                &raw mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return 0;
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end]).trim().parse().unwrap_or(0)
    }

    /// Blocks, calling `changed` after every change to a value of the personalisation key.
    pub(super) fn watch(changed: impl Fn()) {
        let path = wide(PERSONALIZE);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: a NUL-terminated path; the key is closed below.
        let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_NOTIFY | KEY_READ, &raw mut key) };
        if status != ERROR_SUCCESS {
            log::warn!("opening the personalisation key: error {status}");
            return;
        }
        loop {
            // SAFETY: our open key; synchronous (no event), so this returns after the next change.
            let status =
                unsafe { RegNotifyChangeKeyValue(key, 0, REG_NOTIFY_CHANGE_LAST_SET, std::ptr::null_mut(), 0) };
            if status != ERROR_SUCCESS {
                log::warn!("watching the personalisation key: error {status}");
                break;
            }
            changed();
        }
        // SAFETY: our open key.
        unsafe { RegCloseKey(key) };
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::{FALLBACK_DARK, stored};

    /// KDE Plasma's KWin, whose blur effect the windows can ask for (Wayland: GPUI's background-effect or KDE blur
    /// protocol; X11: `_KDE_NET_WM_BLUR_BEHIND_REGION`).
    pub(super) fn kde() -> bool {
        std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|desktop| desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE")))
            || std::env::var("KDE_FULL_SESSION").is_ok_and(|value| value == "true")
    }

    fn output(program: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Whether the XDG desktop portal answers for `org.freedesktop.appearance color-scheme` (GPUI then follows it).
    fn portal_answers() -> bool {
        let args = [
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
            "color-scheme",
        ];
        // No gdbus at all: GPUI's own portal connection is the only source; nothing better to read.
        match Command::new("gdbus").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()
        {
            Ok(status) => status.success(),
            Err(_) => true,
        }
    }

    /// The desktop's own light or dark, without the portal: GNOME's `color-scheme`, else its GTK theme's name; KDE's
    /// colour scheme in `kdeglobals`.
    fn desktop_dark() -> Option<bool> {
        if let Some(scheme) = output("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"]) {
            if scheme.contains("prefer-dark") {
                return Some(true);
            }
            if scheme.contains("prefer-light") {
                return Some(false);
            }
        }
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config")));
        if let Some(text) = config.and_then(|dir| std::fs::read_to_string(dir.join("kdeglobals")).ok()) {
            let mut general = false;
            for line in text.lines().map(str::trim) {
                if line.starts_with('[') {
                    general = line == "[General]";
                } else if general && let Some(name) = line.strip_prefix("ColorScheme=") {
                    return Some(name.to_ascii_lowercase().contains("dark"));
                }
            }
        }
        output("gsettings", &["get", "org.gnome.desktop.interface", "gtk-theme"])
            .map(|theme| theme.to_ascii_lowercase().contains("dark"))
    }

    /// Without the portal, reads the desktop's setting now and every few seconds, calling `changed` when it changes.
    pub(super) fn watch(changed: impl Fn()) {
        if portal_answers() {
            return;
        }
        log::info!("no XDG desktop portal colour scheme: following the desktop's own setting");
        loop {
            let value = desktop_dark().map_or(0, stored);
            if FALLBACK_DARK.swap(value, Ordering::Relaxed) != value {
                changed();
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn the_panel_follows_the_theme_unless_forced() {
        let settings = Settings::open(None);
        assert_eq!(theme(&settings), Choice::System);
        assert_eq!(panel_theme(&settings), Choice::System);
        settings.set("theme", Value::from("dark")).unwrap();
        assert_eq!(panel_theme(&settings), Choice::Dark);
        settings.set("panelAppearance", Value::from("light")).unwrap();
        assert_eq!(panel_theme(&settings), Choice::Light);
        assert!(settings.set("theme", Value::from("blue")).is_err());
        assert!(!Choice::Light.dark(true) && Choice::Dark.dark(false) && Choice::System.dark(true));
    }

    #[test]
    fn transparency_off_is_off_everywhere() {
        let settings = Settings::open(None);
        settings.set("transparency", Value::from("off")).unwrap();
        assert!(!transparency(&settings));
        settings.set("transparency", Value::from("on")).unwrap();
        assert_eq!(transparency(&settings), backdrop_support().is_ok());
    }
}

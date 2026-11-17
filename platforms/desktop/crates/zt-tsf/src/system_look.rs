//! Windows' look that the candidate window follows: high contrast (Settings › Accessibility › 對比佈景主題), with its
//! system colours, and the accent colour (Settings › Personalization › Colors).  Read in the application's process,
//! kept until a window of ours hears that a setting, the system colours, the theme or the DWM colour changed
//! (`refresh`), so drawing costs nothing extra.
//!
//! The colour conversions are plain Rust and tested on any system; reading the system is Windows only.

use zt_panel::tokens::Rgba;

/// A `COLORREF` (0x00BBGGRR), as GetSysColor returns it.
pub(crate) fn colorref(value: u32) -> Rgba {
    abgr(value)
}

/// The DWM's `AccentColor` value (HKCU\Software\Microsoft\Windows\DWM), 0xAABBGGRR; the alpha is ignored (an accent
/// is opaque).
pub(crate) fn abgr(value: u32) -> Rgba {
    let r = value & 0xff;
    let g = (value >> 8) & 0xff;
    let b = (value >> 16) & 0xff;
    Rgba::hex((r << 16) | (g << 8) | b, 1.)
}

/// DwmGetColorizationColor's colour, 0xAARRGGBB; the alpha is ignored.
pub(crate) fn argb(value: u32) -> Rgba {
    Rgba::hex(value & 0x00ff_ffff, 1.)
}

#[cfg(windows)]
pub(crate) use windows_read::{look, refresh};

#[cfg(windows)]
mod windows_read {
    use std::sync::Mutex;

    use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
    use windows::Win32::Graphics::Gdi::{
        COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor,
        SYS_COLOR_INDEX,
    };
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    use windows::core::{BOOL, w};
    use zt_panel::a11y::{Look, SystemColors};
    use zt_panel::tokens::Rgba;

    use super::{abgr, argb, colorref};

    /// The look last read; None: read it again at the next drawing.
    static LOOK: Mutex<Option<Look>> = Mutex::new(None);

    /// The system's look now (read once, then kept until `refresh`).
    pub(crate) fn look() -> Look {
        let Ok(mut cached) = LOOK.lock() else { return read() };
        *cached.get_or_insert_with(read)
    }

    /// Forgets the kept look: a setting, the system colours, the theme or the DWM colour changed.
    pub(crate) fn refresh() {
        if let Ok(mut cached) = LOOK.lock() {
            *cached = None;
        }
    }

    fn read() -> Look {
        Look { accent: accent(), high_contrast: high_contrast() }
    }

    fn high_contrast() -> Option<SystemColors> {
        let mut info = HIGHCONTRASTW { cbSize: size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
        // SAFETY: SPI_GETHIGHCONTRAST fills a sized HIGHCONTRASTW, which outlives the call.
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                info.cbSize,
                Some((&raw mut info).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        if read.is_err() || !info.dwFlags.contains(HCF_HIGHCONTRASTON) {
            return None;
        }
        // SAFETY: GetSysColor takes any index.
        let system = |index: SYS_COLOR_INDEX| colorref(unsafe { GetSysColor(index) });
        Some(SystemColors {
            window: system(COLOR_WINDOW),
            text: system(COLOR_WINDOWTEXT),
            highlight: system(COLOR_HIGHLIGHT),
            highlight_text: system(COLOR_HIGHLIGHTTEXT),
            gray_text: system(COLOR_GRAYTEXT),
        })
    }

    /// The accent colour: the DWM's `AccentColor` (what Settings › Colors shows), else the colorization colour.
    fn accent() -> Option<Rgba> {
        let mut value: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: a DWORD read into a DWORD.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\DWM"),
                w!("AccentColor"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut value).cast()),
                Some(&raw mut size),
            )
        };
        if result.is_ok() {
            return Some(abgr(value));
        }
        let (mut color, mut opaque) = (0u32, BOOL(0));
        // SAFETY: two out values that outlive the call.
        unsafe { DwmGetColorizationColor(&raw mut color, &raw mut opaque) }.ok().map(|()| argb(color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_orders() {
        // Windows' default blue accent: AccentColor 0xffd77800 is RGB 0x0078d7.
        assert_eq!(abgr(0xffd7_7800), Rgba::hex(0x0078d7, 1.));
        // The alpha byte never makes an accent translucent.
        assert_eq!(abgr(0x00d7_7800), Rgba::hex(0x0078d7, 1.));
        assert_eq!(argb(0xc40078d7), Rgba::hex(0x0078d7, 1.));
        // A COLORREF is 0x00BBGGRR: 0x00ffff00 is cyan, RGB(0, 255, 255).
        assert_eq!(colorref(0x00ff_ff00), Rgba::hex(0x00ffff, 1.));
        assert_eq!(colorref(0x0000_00ff), Rgba::hex(0xff0000, 1.));
    }
}

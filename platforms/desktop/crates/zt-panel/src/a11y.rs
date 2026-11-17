//! 「強調色」 and high contrast: how the system's look changes the candidate window's colours ([`resolve`]).
//!
//! - **High contrast** (Windows' high contrast themes, GNOME's 高對比): the window uses the system's own colours,
//!   solid (no backdrop), with 2 px borders and an outline around the selection.  It overrides 透明效果 and 強調色.
//! - **強調色** `system` (the default): the selection is a solid fill of the system's accent colour, made darker
//!   (light windows) or lighter (dark windows) until it stands out from the window by [`SELECTION_CONTRAST`] and the
//!   text on it reads at [`TEXT_CONTRAST`]; an accent that cannot get there falls back to ZhuyinType's indigo.
//!   `zhuyintype`, or a system without an accent colour: ZhuyinType's own look (a neutral selection).
//!
//! Contrast is WCAG 2's: relative luminance of sRGB, `(L1 + 0.05) / (L2 + 0.05)`.

use serde::{Deserialize, Serialize};

use crate::tokens::{self, Colors, Rgba};

/// A selection fill against the window around it (WCAG 1.4.11, non-text contrast).
pub const SELECTION_CONTRAST: f32 = 3.;
/// Text against what it sits on (WCAG 1.4.3).
pub const TEXT_CONTRAST: f32 = 4.5;

/// The `accentColor` setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccentChoice {
    /// 跟隨系統.
    #[default]
    System,
    /// ZhuyinType 藍: the app's own look.
    Zhuyintype,
}

impl AccentChoice {
    /// The setting's stored value; anything else is the default.
    #[must_use]
    pub fn from_setting(value: &str) -> Self {
        if value == "zhuyintype" { Self::Zhuyintype } else { Self::System }
    }
}

/// A high contrast theme's colours (Windows: `COLOR_WINDOW`, `COLOR_WINDOWTEXT`, `COLOR_HIGHLIGHT`,
/// `COLOR_HIGHLIGHTTEXT`, `COLOR_GRAYTEXT`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemColors {
    pub window: Rgba,
    pub text: Rgba,
    pub highlight: Rgba,
    pub highlight_text: Rgba,
    pub gray_text: Rgba,
}

/// High contrast where the system asks for it but gives no colours (GNOME's 高對比): black on white or white on
/// black, a strong blue selection.
#[must_use]
pub const fn high_contrast_palette(dark: bool) -> SystemColors {
    if dark {
        SystemColors {
            window: Rgba::hex(0x000000, 1.),
            text: Rgba::hex(0xffffff, 1.),
            highlight: Rgba::hex(0x78aeed, 1.),
            highlight_text: Rgba::hex(0x000000, 1.),
            gray_text: Rgba::hex(0xc0c0c0, 1.),
        }
    } else {
        SystemColors {
            window: Rgba::hex(0xffffff, 1.),
            text: Rgba::hex(0x000000, 1.),
            highlight: Rgba::hex(0x1a5fb4, 1.),
            highlight_text: Rgba::hex(0xffffff, 1.),
            gray_text: Rgba::hex(0x505050, 1.),
        }
    }
}

/// What the system says about its look right now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// The system's accent colour, where it has one.
    pub accent: Option<Rgba>,
    /// A high contrast theme is on, with these colours.
    pub high_contrast: Option<SystemColors>,
}

/// The candidate window's colours: high contrast, else the accent (with `choice` System), else ZhuyinType's own.
#[must_use]
pub fn resolve(dark: bool, choice: AccentChoice, look: &Look) -> Colors {
    if let Some(system) = look.high_contrast {
        return high_contrast(system);
    }
    let base = tokens::colors(dark);
    match (choice, look.accent) {
        (AccentChoice::System, Some(accent)) => with_accent(base, accent, dark),
        _ => base,
    }
}

/// The window must be drawn solid, whatever 透明效果 says: high contrast is on.
#[must_use]
pub fn opaque(look: &Look) -> bool {
    look.high_contrast.is_some()
}

/// `base` with its selection, switches and focus in `accent` (adjusted for contrast; ZhuyinType's indigo when even
/// that fails).
#[must_use]
pub fn with_accent(base: Colors, accent: Rgba, dark: bool) -> Colors {
    let surface = base.surface;
    let Some((fill, ink)) = selection(accent, surface, dark).or_else(|| selection(indigo(dark), surface, dark)) else {
        return base;
    };
    let ink_for_text = readable(accent, surface, dark).unwrap_or(base.accent);
    Colors {
        selection: fill,
        selection_text: ink,
        selection_secondary: ink.fade(0.85),
        accent: ink_for_text,
        keycap_on: fill.fade(if dark { 0.3 } else { 0.18 }),
        ..base
    }
}

/// The window in a high contrast theme's colours: solid, 2 px strokes, an outlined selection.
#[must_use]
pub fn high_contrast(system: SystemColors) -> Colors {
    let SystemColors { window, text, highlight, highlight_text, gray_text } = system;
    Colors {
        surface: window,
        backdrop_tint: window,
        border: text,
        selection: highlight,
        hover: text.fade(0.12),
        hairline: gray_text,
        keycap: text.fade(0.14),
        keycap_on: highlight,
        text,
        text_secondary: text,
        text_tertiary: text,
        success: text,
        shadow: Rgba::hex(0x000000, 1.),
        selection_text: highlight_text,
        selection_secondary: highlight_text,
        accent: highlight,
        stroke: 2.,
        selection_outline: Some(text),
    }
}

/// ZhuyinType's indigo as a selection fill.
fn indigo(dark: bool) -> Rgba {
    Rgba::hex(if dark { 0x5f72e6 } else { 0x4257d8 }, 1.)
}

/// How many steps of [`STEP`] toward black (light) or white (dark) an accent may move.
const STEPS: u8 = 8;
const STEP: f32 = 0.08;

/// The accent, `steps` steps darker (light windows) or lighter (dark windows).
fn shifted(accent: Rgba, dark: bool, steps: u8) -> Rgba {
    let target = if dark { 1. } else { 0. };
    let t = f32::from(steps) * STEP;
    let mix = |c: f32| c + (target - c) * t;
    Rgba { r: mix(accent.r), g: mix(accent.g), b: mix(accent.b), a: 1. }
}

/// The first shift of `accent` that stands out from `surface` and carries readable text (white or black, whichever
/// reads better), with that text.
fn selection(accent: Rgba, surface: Rgba, dark: bool) -> Option<(Rgba, Rgba)> {
    (0..=STEPS).map(|steps| shifted(accent, dark, steps)).find_map(|fill| {
        if contrast(fill, surface) < SELECTION_CONTRAST {
            return None;
        }
        let ink = on(fill);
        (contrast(ink, fill) >= TEXT_CONTRAST).then_some((fill, ink))
    })
}

/// The first shift of `accent` that reads as text on `surface`.
fn readable(accent: Rgba, surface: Rgba, dark: bool) -> Option<Rgba> {
    (0..=STEPS).map(|steps| shifted(accent, dark, steps)).find(|&ink| contrast(ink, surface) >= TEXT_CONTRAST)
}

/// White or black, whichever reads better on `fill`.
#[must_use]
pub fn on(fill: Rgba) -> Rgba {
    let (white, black) = (Rgba::hex(0xffffff, 1.), Rgba::hex(0x000000, 1.));
    if contrast(white, fill) >= contrast(black, fill) { white } else { black }
}

/// `top` over `below` (straight alpha).
#[must_use]
pub fn over(top: Rgba, below: Rgba) -> Rgba {
    let a = top.a + below.a * (1. - top.a);
    if a <= 0. {
        return Rgba { r: 0., g: 0., b: 0., a: 0. };
    }
    let mix = |t: f32, b: f32| (t * top.a + b * below.a * (1. - top.a)) / a;
    Rgba { r: mix(top.r, below.r), g: mix(top.g, below.g), b: mix(top.b, below.b), a }
}

/// WCAG relative luminance of an opaque sRGB colour (alpha ignored; composite with [`over`] first).
#[must_use]
pub fn luminance(color: Rgba) -> f32 {
    let linear = |c: f32| {
        let c = if c.is_finite() { c.clamp(0., 1.) } else { 0. };
        if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// The WCAG contrast ratio of two colours, 1…21 (translucent `a` is first composited over `b`).
#[must_use]
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    let a = if a.a < 1. { over(a, b) } else { a };
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.02
    }

    #[test]
    fn contrast_matches_wcag() {
        let (white, black) = (Rgba::hex(0xffffff, 1.), Rgba::hex(0x000000, 1.));
        assert!(close(contrast(white, black), 21.));
        assert!(close(contrast(black, white), 21.));
        assert!(close(contrast(white, white), 1.));
        // #777 on white is the classic just-below-AA grey.
        assert!(close(contrast(Rgba::hex(0x777777, 1.), white), 4.48));
        // Half-transparent black over white is 50 % grey.
        let grey = Rgba { r: 0.5, g: 0.5, b: 0.5, a: 1. };
        assert!(close(contrast(black.fade(0.5), white), contrast(grey, white)));
        assert!(luminance(Rgba { r: f32::NAN, g: 0., b: 0., a: 1. }).is_finite());
    }

    #[test]
    fn the_accent_is_adjusted_until_it_reads() {
        for dark in [false, true] {
            let surface = tokens::colors(dark).surface;
            // Windows' blue, a pale yellow, and a grey close to the surface.
            for accent in [0x0078d7, 0xffd700, 0x888888, 0xfefdfb, 0x2a2927] {
                let c = with_accent(tokens::colors(dark), Rgba::hex(accent, 1.), dark);
                assert!(contrast(c.selection, surface) >= SELECTION_CONTRAST, "{accent:06x} dark {dark}");
                assert!(contrast(c.selection_text, c.selection) >= TEXT_CONTRAST, "{accent:06x} dark {dark}");
                assert!(contrast(c.accent, surface) >= TEXT_CONTRAST || c.accent == tokens::colors(dark).accent);
            }
        }
        // A dark accent on a light window is used as it is, with white text.
        let c = with_accent(tokens::LIGHT, Rgba::hex(0x1a4a8a, 1.), false);
        assert_eq!(c.selection, Rgba::hex(0x1a4a8a, 1.));
        assert_eq!(c.selection_text, Rgba::hex(0xffffff, 1.));
    }

    #[test]
    fn resolve_order_high_contrast_then_accent_then_own() {
        let accent = Rgba::hex(0xc42b1c, 1.);
        let plain = Look::default();
        assert_eq!(resolve(false, AccentChoice::System, &plain), tokens::LIGHT);
        let accented = Look { accent: Some(accent), high_contrast: None };
        assert_eq!(resolve(true, AccentChoice::Zhuyintype, &accented), tokens::DARK);
        assert_ne!(resolve(true, AccentChoice::System, &accented).selection, tokens::DARK.selection);
        assert!(!opaque(&accented));
        let hc = Look { accent: Some(accent), high_contrast: Some(high_contrast_palette(true)) };
        let c = resolve(false, AccentChoice::System, &hc);
        assert!(opaque(&hc));
        assert_eq!(c.surface, Rgba::hex(0x000000, 1.));
        assert_eq!(c.stroke, 2.);
        assert!(c.selection_outline.is_some());
        for dark in [false, true] {
            let c = high_contrast(high_contrast_palette(dark));
            assert!(contrast(c.text, c.surface) >= 7.);
            assert!(contrast(c.selection_text, c.selection) >= TEXT_CONTRAST);
            assert!(contrast(c.selection, c.surface) >= SELECTION_CONTRAST);
        }
    }

    #[test]
    fn the_setting_parses_and_serializes() {
        assert_eq!(AccentChoice::from_setting("zhuyintype"), AccentChoice::Zhuyintype);
        assert_eq!(AccentChoice::from_setting("system"), AccentChoice::System);
        assert_eq!(AccentChoice::from_setting("purple"), AccentChoice::System);
        assert_eq!(serde_json::to_string(&AccentChoice::Zhuyintype).unwrap(), "\"zhuyintype\"");
    }

    #[test]
    fn the_own_look_keeps_its_text_on_the_selection() {
        for c in [tokens::LIGHT, tokens::DARK] {
            assert_eq!(c.selection_text, c.text);
            assert_eq!(c.selection_secondary, c.text_secondary);
            assert!(contrast(c.text, over(c.selection, c.surface)) >= TEXT_CONTRAST);
        }
    }
}

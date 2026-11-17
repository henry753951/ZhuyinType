//! The candidate window's sizes and colours: every place that draws the window (the GPUI app's floating window and
//! previews, the Windows text service inside applications) uses these, so they look the same.

/// The shell: outer radius = padding + row radius, so the selection's corners run parallel to the window's.
pub const RADIUS: f32 = 10.;
pub const PADDING: f32 = 4.;
pub const ROW_RADIUS: f32 = RADIUS - PADDING;
/// Key caps of the reading row, the footer and the notices: borderless, `keycap` fill.
pub const KEYCAP_H: f32 = 18.;
pub const KEYCAP_RADIUS: f32 = 4.;
pub const KEYCAP_FONT: f32 = 11.;
/// The reading row's and the footer's labels.
pub const LABEL_FONT: f32 = 12.;
/// Between the selection key column and the candidate.
pub const NUMBER_GAP: f32 = 8.;
/// The number column's text, relative to the candidates' font size.
pub const NUMBER_SCALE: f32 = 0.62;
/// Between candidates in the horizontal layout.
pub const HORIZONTAL_GAP: f32 = 14.;
/// The vertical hairline between the last candidate and the footer in the horizontal layout.
pub const DIVIDER_H: f32 = 14.;
/// The paging chevrons.
pub const CHEVRON: f32 = 14.;
/// The 中/英 badge.
pub const BADGE: f32 = 44.;
pub const BADGE_FONT: f32 = 20.;
/// A notice on its own.
pub const NOTICE_H: f32 = 32.;
pub const NOTICE_ICON: f32 = 14.;

/// A colour, straight (not premultiplied) alpha, 0…1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    /// `0xRRGGBB` at `alpha`.
    #[must_use]
    pub const fn hex(rgb: u32, alpha: f32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as f32 / 255.,
            g: ((rgb >> 8) & 0xff) as f32 / 255.,
            b: (rgb & 0xff) as f32 / 255.,
            a: alpha,
        }
    }

    /// This colour at `alpha` times its own.
    #[must_use]
    pub fn fade(self, alpha: f32) -> Self {
        Self { a: self.a * alpha, ..self }
    }
}

/// The window's colours in one appearance.  Fills are translucent inks, so they read the same on the window's solid
/// surface and on a system backdrop (Windows 11's acrylic) behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// The window's own surface (where the system gives it no backdrop).
    pub surface: Rgba,
    /// A tint over a system backdrop, so the text keeps its contrast on any wallpaper.
    pub backdrop_tint: Rgba,
    pub border: Rgba,
    pub selection: Rgba,
    pub hover: Rgba,
    /// Hairlines inside the window (above the footer, before the horizontal footer).
    pub hairline: Rgba,
    pub keycap: Rgba,
    /// A footer switch that is on.
    pub keycap_on: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub success: Rgba,
    /// The shadows' ink.
    pub shadow: Rgba,
    /// The candidate on the selected row (`text`, or what reads on an accent or high-contrast fill).
    pub selection_text: Rgba,
    /// The selected row's selection key and annotation.
    pub selection_secondary: Rgba,
    /// The accent: focus rings, a footer switch that is on (ZhuyinType's indigo, or the system's accent).
    pub accent: Rgba,
    /// The window's border and the selection outline, in logical pixels: 1, or 2 in high contrast.
    pub stroke: f32,
    /// High contrast: an outline `stroke` wide around the selected row.
    pub selection_outline: Option<Rgba>,
}

/// The light window.
pub const LIGHT: Colors = {
    let ink = 0x1e1c18;
    Colors {
        surface: Rgba::hex(0xfefdfb, 1.),
        backdrop_tint: Rgba::hex(0xfbfaf7, 0.55),
        border: Rgba::hex(ink, 0.10),
        selection: Rgba::hex(ink, 0.07),
        hover: Rgba::hex(ink, 0.04),
        hairline: Rgba::hex(ink, 0.08),
        keycap: Rgba::hex(ink, 0.06),
        keycap_on: Rgba::hex(0x1f1e1b, 0.12),
        text: Rgba::hex(0x1f1e1b, 1.),
        text_secondary: Rgba::hex(0x6b6962, 1.),
        text_tertiary: Rgba::hex(0x8b887f, 1.),
        success: Rgba::hex(0x2f8f5b, 1.),
        shadow: Rgba::hex(ink, 1.),
        selection_text: Rgba::hex(0x1f1e1b, 1.),
        selection_secondary: Rgba::hex(0x6b6962, 1.),
        accent: Rgba::hex(0x4257d8, 1.),
        stroke: 1.,
        selection_outline: None,
    }
};

/// The dark window.
pub const DARK: Colors = {
    let ink = 0xfffcf0;
    Colors {
        surface: Rgba::hex(0x2a2927, 1.),
        backdrop_tint: Rgba::hex(0x232220, 0.55),
        border: Rgba::hex(ink, 0.10),
        selection: Rgba::hex(ink, 0.09),
        hover: Rgba::hex(ink, 0.04),
        hairline: Rgba::hex(ink, 0.08),
        keycap: Rgba::hex(ink, 0.07),
        keycap_on: Rgba::hex(0xedebe5, 0.14),
        text: Rgba::hex(0xedebe5, 1.),
        text_secondary: Rgba::hex(0xa6a39b, 1.),
        text_tertiary: Rgba::hex(0x85827a, 1.),
        success: Rgba::hex(0x5cc28a, 1.),
        shadow: Rgba::hex(0x000000, 1.),
        selection_text: Rgba::hex(0xedebe5, 1.),
        selection_secondary: Rgba::hex(0xa6a39b, 1.),
        accent: Rgba::hex(0x7a8cf0, 1.),
        stroke: 1.,
        selection_outline: None,
    }
};

/// The colours for `dark`.
#[must_use]
pub fn colors(dark: bool) -> Colors {
    if dark { DARK } else { LIGHT }
}

/// How a page change and the end of the list move (macOS `PageMotion`), in milliseconds and logical pixels: short,
/// eased out, a few pixels along the list.
pub mod motion {
    /// The whole turn.
    pub const DURATION: f32 = 140.;
    /// The old rows are gone after the first 35 % and the new ones start at 30 %, so two pages of text are never
    /// legible at once.
    pub const OUT_DURATION: f32 = 50.;
    pub const IN_DELAY: f32 = 42.;
    /// How far the pages travel.
    pub const OFFSET: f32 = 10.;
    /// The nudge at the first or last page.
    pub const BOUNCE: f32 = 3.;
    pub const BOUNCE_OUT: f32 = 60.;
    pub const BOUNCE_BACK: f32 = 120.;
    /// The bounce under reduced motion: the page dims to this opacity instead of moving.
    pub const BOUNCE_DIM: f32 = 0.7;
    /// The 中/英 badge stays this long.
    pub const TOAST_HOLD: f32 = 800.;

    /// Ease-out.
    #[must_use]
    pub fn ease_out(t: f32) -> f32 {
        let t = t.clamp(0., 1.);
        1. - (1. - t) * (1. - t)
    }

    /// Ease-in-out.
    #[must_use]
    pub fn ease_in_out(t: f32) -> f32 {
        let t = t.clamp(0., 1.);
        if t < 0.5 { 2. * t * t } else { 1. - (-2. * t + 2.).powi(2) / 2. }
    }
}

//! Design tokens: every colour, radius, size and duration the UI uses comes from here, never from a literal in a page.
//!
//! ZhuyinType's own design language for Linux and Windows: not a copy of macOS nor of Windows.  Warm neutral
//! surfaces, monochrome line icons, hairline dividers between rows instead of filled cards, one calm indigo accent
//! used sparingly, Inter for Latin text and digits over the system's CJK font.  The window is the sidebar's colour;
//! the page sits on an inset rounded surface.  Light and dark are complete sets; the system appearance picks one and
//! switching is live.

use std::time::Duration;

use gpui_kit::component::{Theme, ThemeMode, ThemeTokens};
use gpui_kit::{App, Global, Hsla, Pixels, Window, WindowAppearance, px, rgb, rgba};

/// The colours of one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    /// The window: title bar and the frame around the page surface.
    pub window: Hsla,
    /// The sidebar (the same as the window: one continuous frame).
    pub sidebar: Hsla,
    /// The page surface the content sits on.
    pub surface: Hsla,
    /// Raised things: inputs, menus, popovers, demo frames.
    pub card: Hsla,
    /// A faint fill: key caps, segment tracks, chips, the selected sidebar row.
    pub subtle: Hsla,
    /// The selected candidate and other selected list rows: a neutral fill, never the accent.
    pub selection: Hsla,
    /// A row under the pointer.
    pub hover: Hsla,
    /// A pressed control.
    pub pressed: Hsla,
    /// Hairlines between rows and around cards.
    pub separator: Hsla,
    /// Main text.
    pub text: Hsla,
    /// Subtitles and values.
    pub text_secondary: Hsla,
    /// Section headers, hints, disabled text.
    pub text_tertiary: Hsla,
    /// The accent: selection, links, switches that are on.
    pub accent: Hsla,
    /// Text on the accent.
    pub on_accent: Hsla,
    /// The accent, faint (highlights, arrival pulse).
    pub accent_faint: Hsla,
    /// Controls' fill (pop-up buttons, key caps, segmented controls).
    pub control: Hsla,
    /// A control's border.
    pub control_border: Hsla,
    /// The knob of a switch.
    pub knob: Hsla,
    /// Destructive actions and conflicts.
    pub danger: Hsla,
    /// Positive state (saved, done).
    pub success: Hsla,
    /// The candidate window's own background (it floats over other apps).
    pub panel: Hsla,
    /// The candidate window's border.
    pub panel_border: Hsla,
    /// Shadows of floating surfaces.
    pub shadow: Hsla,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn ca(hex_with_alpha: u32) -> Hsla {
    rgba(hex_with_alpha).into()
}

impl Palette {
    pub fn light() -> Self {
        Self {
            dark: false,
            window: c(0xf1efe9),
            sidebar: c(0xf1efe9),
            surface: c(0xfbfaf7),
            card: c(0xffffff),
            subtle: ca(0x1e1c180d),
            selection: ca(0x1e1c1814),
            hover: ca(0x1e1c180a),
            pressed: ca(0x1e1c1814),
            separator: ca(0x1e1c1817),
            text: c(0x1f1e1b),
            text_secondary: c(0x6b6962),
            text_tertiary: c(0x9d9a92),
            accent: c(0x4257d8),
            on_accent: c(0xffffff),
            accent_faint: ca(0x4257d81c),
            control: c(0xffffff),
            control_border: ca(0x1e1c1824),
            knob: c(0xffffff),
            danger: c(0xd5483f),
            success: c(0x2f8f5b),
            panel: c(0xfefdfb),
            panel_border: ca(0x1e1c181f),
            shadow: ca(0x1e1c1822),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            window: c(0x1b1a18),
            sidebar: c(0x1b1a18),
            surface: c(0x232220),
            card: c(0x2c2b28),
            subtle: ca(0xfffcf00f),
            selection: ca(0xfffcf01c),
            hover: ca(0xfffcf00b),
            pressed: ca(0xfffcf017),
            separator: ca(0xfffcf014),
            text: c(0xedebe5),
            text_secondary: c(0xa6a39b),
            text_tertiary: c(0x716f68),
            accent: c(0x8a9cf5),
            on_accent: c(0x14151f),
            accent_faint: ca(0x8a9cf526),
            control: c(0x2c2b28),
            control_border: ca(0xfffcf01f),
            knob: c(0xf5f3ee),
            danger: c(0xf0766d),
            success: c(0x5cc28a),
            panel: c(0x2a2927),
            panel_border: ca(0xfffcf024),
            shadow: ca(0x0000006b),
        }
    }
}

/// Sizes, radii, durations: the same in both appearances.
pub mod size {
    use super::*;

    pub const TITLE_BAR: Pixels = px(44.);
    pub const SIDEBAR: Pixels = px(228.);
    pub const SIDEBAR_ROW: Pixels = px(32.);
    pub const SIDEBAR_ICON: Pixels = px(16.);
    /// The page surface's inset from the window edge (right and bottom; the sidebar is on the left).
    pub const SURFACE_INSET: Pixels = px(8.);
    /// The page's content column never grows past this, centred in wide windows.
    pub const CONTENT_MAX: Pixels = px(760.);
    pub const PAGE_PADDING_X: Pixels = px(36.);
    pub const PAGE_PADDING_TOP: Pixels = px(28.);
    pub const SECTION_GAP: Pixels = px(32.);
    pub const ROW_MIN: Pixels = px(52.);
    pub const ROW_PADDING_X: Pixels = px(10.);
    pub const ROW_PADDING_Y: Pixels = px(12.);
    pub const RADIUS_SURFACE: Pixels = px(12.);
    pub const RADIUS_CARD: Pixels = px(10.);
    pub const RADIUS_CONTROL: Pixels = px(8.);
    pub const RADIUS_SMALL: Pixels = px(6.);
    pub const CONTROL_H: Pixels = px(30.);
    pub const WINDOW_MIN_W: Pixels = px(640.);
    pub const WINDOW_MIN_H: Pixels = px(440.);
    pub const WINDOW_W: Pixels = px(900.);
    pub const WINDOW_H: Pixels = px(640.);
}

/// Text sizes.
pub mod text {
    use super::*;

    /// The page's own title.
    pub const PAGE_TITLE: Pixels = px(22.);
    pub const TITLE: Pixels = px(20.);
    pub const HEADLINE: Pixels = px(15.);
    pub const BODY: Pixels = px(14.);
    pub const SUBTITLE: Pixels = px(13.);
    /// Section headings: primary colour, semibold, a little larger than body.
    pub const SECTION: Pixels = px(14.5);
    pub const CAPTION: Pixels = px(12.);
}

/// Motion: the macOS app's durations.  Every animation checks `Motion::reduced` first.
pub mod motion {
    use super::*;

    pub const FAST: Duration = Duration::from_millis(120);
    pub const NORMAL: Duration = Duration::from_millis(200);
    pub const PAGE: Duration = Duration::from_millis(240);
    /// The candidate window's page turn (macOS: 140 ms, 10 px).
    pub const PAGE_TURN: Duration = Duration::from_millis(140);
}

/// The UI font: Inter (bundled, `assets/fonts`) for Latin text and digits; CJK and Zhuyin fall back to the
/// system's font.
pub fn ui_font() -> &'static str {
    "Inter"
}

/// The CJK fonts, in fallback order, for each system.
pub fn cjk_fallbacks() -> Vec<String> {
    let list: &[&str] = if cfg!(target_os = "windows") {
        &["Microsoft JhengHei UI", "Microsoft JhengHei", "Segoe UI", "Segoe UI Symbol", "Segoe UI Emoji"]
    } else {
        // DejaVu Sans and Noto Sans Symbols 2 have ⇧ ⌫ and the arrows, which the CJK fonts lack.
        &[
            "Noto Sans CJK TC",
            "Noto Sans TC",
            "Source Han Sans TC",
            "WenQuanYi Micro Hei",
            "DejaVu Sans",
            "Noto Sans Symbols 2",
            "Noto Color Emoji",
        ]
    };
    list.iter().map(|name| (*name).to_owned()).collect()
}

/// The font every view starts from: Inter with the CJK fallbacks.
pub fn font() -> gpui_kit::Font {
    gpui_kit::Font {
        family: ui_font().into(),
        features: gpui_kit::FontFeatures::default(),
        fallbacks: Some(gpui_kit::FontFallbacks::from_fonts(cjk_fallbacks())),
        weight: gpui_kit::FontWeight::NORMAL,
        style: gpui_kit::FontStyle::Normal,
    }
}

/// Registers the bundled fonts with the text system.  Call once at start-up.
pub fn load_fonts(cx: &App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Medium.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::warn!("loading the bundled fonts: {error:#}");
    }
}

/// The current palette and appearance, a global every view reads.
pub struct Tokens {
    pub palette: Palette,
    /// The user's choice: None follows the system.
    pub forced: Option<bool>,
}

impl Global for Tokens {}

/// The palette in effect.
pub fn palette(cx: &App) -> Palette {
    cx.global::<Tokens>().palette
}

/// Installs the tokens for the system appearance and themes the component library to match.
pub fn init(window_appearance: WindowAppearance, cx: &mut App) {
    let dark = matches!(window_appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark);
    cx.set_global(Tokens { palette: if dark { Palette::dark() } else { Palette::light() }, forced: None });
    apply_component_theme(None, cx);
}

/// Follows the system appearance unless the user forced one.
pub fn sync(window: &mut Window, cx: &mut App) {
    let forced = cx.global::<Tokens>().forced;
    let dark =
        forced.unwrap_or_else(|| matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark));
    if cx.global::<Tokens>().palette.dark != dark {
        cx.global_mut::<Tokens>().palette = if dark { Palette::dark() } else { Palette::light() };
        apply_component_theme(Some(window), cx);
        cx.refresh_windows();
    }
}

/// Forces light or dark (`None`: follow the system); renders and the screenshot mode use this.
pub fn force(dark: Option<bool>, window: &mut Window, cx: &mut App) {
    cx.global_mut::<Tokens>().forced = dark;
    let target =
        dark.unwrap_or_else(|| matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark));
    cx.global_mut::<Tokens>().palette = if target { Palette::dark() } else { Palette::light() };
    apply_component_theme(Some(window), cx);
    cx.refresh_windows();
}

/// The component library (inputs, sliders, scrollbars, popovers) draws with our palette.
fn apply_component_theme(window: Option<&mut Window>, cx: &mut App) {
    let p = palette(cx);
    Theme::change(if p.dark { ThemeMode::Dark } else { ThemeMode::Light }, window, cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = ui_font().into();
    theme.font_size = text::BODY;
    theme.radius = size::RADIUS_CONTROL;
    theme.radius_lg = size::RADIUS_CARD;
    // Linux with client-side decorations: rounded corners, a hairline and the shadow window_border draws.
    theme.window_radius = size::RADIUS_SURFACE;
    theme.shadow = true;
    let colors = &mut theme.colors;
    colors.background = p.surface;
    colors.foreground = p.text;
    colors.muted = p.subtle;
    colors.muted_foreground = p.text_secondary;
    colors.border = p.separator;
    colors.input = p.control_border;
    colors.ring = p.accent;
    colors.primary = p.accent;
    colors.primary_foreground = p.on_accent;
    colors.accent = p.hover;
    colors.accent_foreground = p.text;
    colors.popover = p.card;
    colors.secondary = p.subtle;
    colors.secondary_foreground = p.text;
    colors.secondary_hover = p.pressed;
    colors.secondary_active = p.pressed;
    colors.popover_foreground = p.text;
    colors.selection = p.accent_faint;
    colors.switch = if p.dark { c(0x48484e) } else { c(0xd5d5da) };
    colors.title_bar = p.sidebar;
    colors.title_bar_border = p.separator;
    colors.window_border = if p.dark { ca(0xfffcf021) } else { ca(0x1e1c182e) };
    colors.sidebar = p.sidebar;
    colors.sidebar_foreground = p.text;
    colors.sidebar_border = p.separator;
    colors.sidebar_accent = p.accent;
    colors.sidebar_accent_foreground = p.on_accent;
    colors.danger = p.danger;
    colors.success = p.success;
    colors.scrollbar = Hsla::transparent_black();
    colors.scrollbar_thumb = if p.dark { ca(0xffffff33) } else { ca(0x00000033) };
    colors.switch_thumb = p.knob;
    theme.tokens = ThemeTokens::from(&theme.colors);
}

// MARK: - Candidate window tokens (append-only block, owned by the candidate window)

/// The candidate window's own tokens: it floats over other apps, so it has its own surface, hairlines, selection and
/// shadows (an elevation-2 surface: panel colour, hairline border, two shadows).  Every place that draws the window
/// (the floating window, the 「視窗與提示」 preview, the 介紹 stage, the demos' mini lists) uses these.
pub mod candidate {
    use gpui_kit::{BoxShadow, Hsla, point, px};

    use super::Palette;

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

    /// The candidate window's colours in one appearance.
    #[derive(Clone, Debug)]
    pub struct Colors {
        pub surface: Hsla,
        pub border: Hsla,
        /// A second, outer ring (dark: the window's edge against dark apps), drawn as a 1 px spread shadow.
        pub outline: Option<Hsla>,
        pub selection: Hsla,
        pub hover: Hsla,
        /// Hairlines inside the window (above the footer, before the horizontal footer).
        pub hairline: Hsla,
        pub keycap: Hsla,
        /// A footer switch that is on.
        pub keycap_on: Hsla,
        pub shadows: [BoxShadow; 2],
    }

    /// `color`'s ink at exactly `alpha` (the spec's opacities on the palette's warm ink).
    fn at(color: Hsla, alpha: f32) -> Hsla {
        Hsla { a: alpha, ..color }
    }

    /// The window's colours for `p`'s appearance: the palette's panel, selection, hover and shadow inks at the
    /// candidate window's own opacities (text colours come from `p` itself).
    pub fn colors(p: &Palette) -> Colors {
        let dark = p.dark;
        let pick = |light: f32, dark_value: f32| if dark { dark_value } else { light };
        let shadow = |alpha: f32, y: f32, blur: f32| BoxShadow {
            color: at(p.shadow, alpha),
            offset: point(px(0.), px(y)),
            blur_radius: px(blur),
            spread_radius: px(if blur > 2. { -4. } else { 0. }),
            inset: false,
        };
        Colors {
            surface: p.panel,
            border: at(p.panel_border, 0.10),
            outline: dark.then(|| at(gpui_kit::black(), 0.4)),
            selection: at(p.selection, pick(0.07, 0.09)),
            hover: at(p.hover, 0.04),
            hairline: at(p.panel_border, 0.08),
            keycap: at(p.subtle, pick(0.06, 0.07)),
            keycap_on: at(p.text, pick(0.12, 0.14)),
            shadows: [shadow(pick(0.06, 0.3), 1., 2.), if dark { shadow(0.5, 8., 24.) } else { shadow(0.16, 6., 20.) }],
        }
    }

    impl Colors {
        /// The surface's shadows, with the dark outer ring.
        pub fn shadow_list(&self) -> Vec<BoxShadow> {
            let ring = |color| BoxShadow {
                color,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            };
            let mut list: Vec<BoxShadow> = self.outline.map(ring).into_iter().collect();
            list.extend(self.shadows.iter().cloned());
            list
        }
    }

    /// Digits of equal width (Inter `tnum`): the number column and the page label do not jitter.
    pub fn tabular() -> gpui_kit::FontFeatures {
        gpui_kit::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
    }
}

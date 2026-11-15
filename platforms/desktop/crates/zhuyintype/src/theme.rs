//! Design tokens: every colour, radius, size and duration the UI uses comes from here, never from a literal in a page.
//!
//! The palette follows the macOS app's hierarchy (grouped rows on cards over a slightly darker window, a sidebar,
//! one accent) with ZhuyinType's own accent taken from the app icon.  Light and dark are complete sets; the system
//! appearance picks one and switching is live.

use std::time::Duration;

use gpui_kit::component::{Theme, ThemeMode, ThemeTokens};
use gpui_kit::{App, Global, Hsla, Pixels, Window, WindowAppearance, px, rgb, rgba};

/// The colours of one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    /// The window behind the cards.
    pub window: Hsla,
    /// The sidebar and the title bar over it.
    pub sidebar: Hsla,
    /// Cards that group rows.
    pub card: Hsla,
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
            window: c(0xf3f3f5),
            sidebar: c(0xe9e9ec),
            card: c(0xffffff),
            hover: ca(0x0000000a),
            pressed: ca(0x00000014),
            separator: ca(0x0000001a),
            text: c(0x1d1d1f),
            text_secondary: c(0x6e6e73),
            text_tertiary: c(0x9a9aa0),
            accent: c(0x2f6bf2),
            on_accent: c(0xffffff),
            accent_faint: ca(0x2f6bf21f),
            control: c(0xffffff),
            control_border: ca(0x00000024),
            knob: c(0xffffff),
            danger: c(0xe5484d),
            success: c(0x2fa84f),
            panel: ca(0xfbfbfdf5),
            panel_border: ca(0x0000001f),
            shadow: ca(0x00000026),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            window: c(0x1c1c1f),
            sidebar: c(0x232327),
            card: c(0x2a2a2e),
            hover: ca(0xffffff0d),
            pressed: ca(0xffffff1a),
            separator: ca(0xffffff17),
            text: c(0xf2f2f4),
            text_secondary: c(0xa1a1a8),
            text_tertiary: c(0x6f6f76),
            accent: c(0x4b82ff),
            on_accent: c(0xffffff),
            accent_faint: ca(0x4b82ff2e),
            control: c(0x3a3a3f),
            control_border: ca(0xffffff1f),
            knob: c(0xf5f5f7),
            danger: c(0xff6369),
            success: c(0x46c46a),
            panel: ca(0x2a2a2ef5),
            panel_border: ca(0xffffff24),
            shadow: ca(0x00000066),
        }
    }
}

/// Sizes, radii, durations: the same in both appearances.
pub mod size {
    use super::*;

    pub const TITLE_BAR: Pixels = px(40.);
    pub const SIDEBAR: Pixels = px(216.);
    pub const SIDEBAR_ROW: Pixels = px(34.);
    pub const SIDEBAR_ICON: Pixels = px(22.);
    /// The page's content column never grows past this, centred in wide windows.
    pub const CONTENT_MAX: Pixels = px(720.);
    pub const PAGE_PADDING_X: Pixels = px(28.);
    pub const PAGE_PADDING_TOP: Pixels = px(20.);
    pub const SECTION_GAP: Pixels = px(22.);
    pub const ROW_MIN: Pixels = px(44.);
    pub const ROW_PADDING_X: Pixels = px(14.);
    pub const ROW_PADDING_Y: Pixels = px(9.);
    pub const RADIUS_CARD: Pixels = px(10.);
    pub const RADIUS_CONTROL: Pixels = px(6.);
    pub const RADIUS_SMALL: Pixels = px(4.);
    pub const WINDOW_MIN_W: Pixels = px(640.);
    pub const WINDOW_MIN_H: Pixels = px(440.);
    pub const WINDOW_W: Pixels = px(860.);
    pub const WINDOW_H: Pixels = px(600.);
}

/// Text sizes.
pub mod text {
    use super::*;

    pub const TITLE: Pixels = px(20.);
    pub const HEADLINE: Pixels = px(15.);
    pub const BODY: Pixels = px(14.);
    pub const SUBTITLE: Pixels = px(12.5);
    pub const SECTION: Pixels = px(12.5);
    pub const CAPTION: Pixels = px(11.5);
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

/// The fonts, in fallback order: the UI font of each system, then a CJK font that has Zhuyin.
pub fn ui_font() -> &'static str {
    if cfg!(target_os = "windows") { "Microsoft JhengHei UI" } else { "Noto Sans CJK TC" }
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
    theme.shadow = true;
    let colors = &mut theme.colors;
    colors.background = p.window;
    colors.foreground = p.text;
    colors.muted = p.control;
    colors.muted_foreground = p.text_secondary;
    colors.border = p.separator;
    colors.input = p.control_border;
    colors.ring = p.accent;
    colors.primary = p.accent;
    colors.primary_foreground = p.on_accent;
    colors.accent = p.hover;
    colors.accent_foreground = p.text;
    colors.popover = p.card;
    colors.popover_foreground = p.text;
    colors.selection = p.accent_faint;
    colors.switch = if p.dark { c(0x48484e) } else { c(0xd5d5da) };
    colors.title_bar = p.sidebar;
    colors.title_bar_border = p.separator;
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

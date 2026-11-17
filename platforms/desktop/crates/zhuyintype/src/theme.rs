//! Design tokens: every colour, radius, size and duration the UI uses comes from here, never from a literal in a page.
//!
//! ZhuyinType's own design language for Linux and Windows: not a copy of macOS nor of Windows.  Warm neutral
//! surfaces, monochrome line icons, hairline dividers between rows instead of filled cards, one calm indigo accent
//! used sparingly, Inter for Latin text and digits over the system's CJK font.  The window is the sidebar's colour;
//! the page sits on an inset rounded surface.  Light and dark are complete sets; 「主題」 (or the system appearance)
//! picks one and switching is live.  With 「透明效果」 the window frame and the page let the system's backdrop through
//! (`Material`): Windows 11's Mica, KDE Plasma's blur; `platform::backdrop` asks the window system for it.

use std::time::Duration;

use gpui_kit::component::{Theme, ThemeMode, ThemeTokens};
use gpui_kit::{App, Global, Hsla, Pixels, Window, WindowAppearance, px, rgb, rgba};

/// What is behind a window's content: nothing (an opaque window), or a system backdrop that shows through the frame
/// and, fainter, through the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Opaque,
    /// Windows 11 22H2 on: Mica (`DWMSBT_MAINWINDOW`), the desktop's colour, blurred and tinted by the system.
    #[cfg_attr(not(windows), allow(dead_code, reason = "Windows only"))]
    Mica,
    /// KDE Plasma: what is behind the window, blurred by KWin.  The window itself must keep the text readable, so its
    /// fills stay mostly opaque.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "Linux only"))]
    Blur,
}

/// The colours of one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    /// The settings and wizard windows' backdrop.
    pub material: Material,
    /// The window frame as drawn (title bar, sidebar): `window`, or a tint over the backdrop.
    pub frame: Hsla,
    /// The page surface as drawn: `surface`, or a stronger tint over the backdrop.
    pub page: Hsla,
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
    /// The page surface's edge against the window frame (a little stronger than a row's hairline, so the inset
    /// surface reads as a surface).
    pub surface_border: Hsla,
    /// Main text.
    pub text: Hsla,
    /// Subtitles and values.
    pub text_secondary: Hsla,
    /// Section headers, hints, disabled text.
    pub text_tertiary: Hsla,
    /// The accent as text: links, the arrival cue, the 已自訂 dot, a key being recorded.
    pub accent: Hsla,
    /// The accent as a fill: a switch that is on, progress, chart bars.
    pub accent_fill: Hsla,
    /// The primary (default) button: an inverted neutral, never the accent.
    pub primary: Hsla,
    /// Text on `primary`.
    pub on_primary: Hsla,
    /// The accent, faint (highlights, arrival pulse).
    pub accent_faint: Hsla,
    /// Controls' fill (pop-up buttons, key caps, segmented controls).
    pub control: Hsla,
    /// A control's border.
    pub control_border: Hsla,
    /// The knob of a switch.
    pub knob: Hsla,
    /// The track of a switch that is off.
    pub switch_off: Hsla,
    /// Destructive actions and conflicts.
    pub danger: Hsla,
    /// Positive state (saved, done).
    pub success: Hsla,
    /// The candidate window's own background (it floats over other apps).
    pub panel: Hsla,
    /// The candidate window's border.
    pub panel_border: Hsla,
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
            material: Material::Opaque,
            frame: c(0xefede6),
            page: c(0xfbfaf7),
            window: c(0xefede6),
            sidebar: c(0xefede6),
            surface: c(0xfbfaf7),
            card: c(0xffffff),
            subtle: ca(0x1e1c180d),
            selection: ca(0x1e1c1814),
            hover: ca(0x1e1c180a),
            pressed: ca(0x1e1c1814),
            separator: ca(0x1e1c1817),
            surface_border: ca(0x1e1c1824),
            text: c(0x1f1e1b),
            text_secondary: c(0x6b6962),
            text_tertiary: c(0x8b887f),
            accent: c(0x4257d8),
            accent_fill: c(0x4257d8),
            primary: c(0x1f1e1b),
            on_primary: c(0xfbfaf7),
            accent_faint: ca(0x4257d81c),
            control: c(0xffffff),
            control_border: ca(0x1e1c1824),
            knob: c(0xffffff),
            switch_off: c(0xdcd9d2),
            danger: c(0xd5483f),
            success: c(0x2f8f5b),
            panel: c(0xfefdfb),
            panel_border: ca(0x1e1c181f),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            material: Material::Opaque,
            frame: c(0x1b1a18),
            page: c(0x232220),
            window: c(0x1b1a18),
            sidebar: c(0x1b1a18),
            surface: c(0x232220),
            card: c(0x2c2b28),
            subtle: ca(0xfffcf00f),
            selection: ca(0xfffcf01c),
            hover: ca(0xfffcf00b),
            pressed: ca(0xfffcf017),
            separator: ca(0xfffcf014),
            surface_border: ca(0xfffcf01f),
            text: c(0xedebe5),
            text_secondary: c(0xa6a39b),
            text_tertiary: c(0x85827a),
            accent: c(0x7a8cf0),
            accent_fill: c(0x5f72e6),
            primary: c(0xedebe5),
            on_primary: c(0x1b1a18),
            accent_faint: ca(0x7a8cf526),
            control: c(0x2c2b28),
            control_border: ca(0xfffcf01f),
            knob: c(0xffffff),
            switch_off: c(0x4a4843),
            danger: c(0xf0766d),
            success: c(0x5cc28a),
            panel: c(0x2a2927),
            panel_border: ca(0xfffcf024),
        }
    }

    /// The palette for `dark` over `material`.
    pub fn new(dark: bool, material: Material) -> Self {
        let mut p = if dark { Self::dark() } else { Self::light() };
        p.material = material;
        // Mica is already an opaque, quiet colour (a still sample of the wallpaper): the frame is mostly Mica with a
        // little of the warm window colour, and the page a light veil so rows read as one surface.  A blur shows what
        // is behind the window, moving, so both stay close to opaque for the text's contrast.
        let (frame, page) = match (material, dark) {
            (Material::Opaque, _) => return p,
            (Material::Mica, false) => (0.35, 0.72),
            (Material::Mica, true) => (0.3, 0.55),
            (Material::Blur, false) => (0.78, 0.9),
            (Material::Blur, true) => (0.8, 0.88),
        };
        p.frame = p.window.opacity(frame);
        p.page = p.surface.opacity(page);
        p
    }

    /// The palette with 「強調色」 applied: with 跟隨系統 and a system accent (and no high contrast), links, focus
    /// rings, switches and highlights use the accent, adjusted for contrast as the candidate window's selection is
    /// (`zt_panel::a11y::with_accent`).
    #[must_use]
    pub fn accented(mut self, choice: zt_panel::a11y::AccentChoice) -> Self {
        let look = zt_core::system_look::look(self.dark);
        let (zt_panel::a11y::AccentChoice::System, Some(accent), None) = (choice, look.accent, look.high_contrast)
        else {
            return self;
        };
        let c = zt_panel::a11y::with_accent(zt_panel::tokens::colors(self.dark), accent, self.dark);
        let hsla = |color: zt_panel::tokens::Rgba| -> Hsla {
            gpui_kit::Rgba { r: color.r, g: color.g, b: color.b, a: color.a }.into()
        };
        self.accent = hsla(c.accent);
        self.accent_fill = hsla(c.selection);
        self.accent_faint = hsla(c.selection.fade(if self.dark { 0.15 } else { 0.11 }));
        self
    }
}

/// The spacing scale: every gap, padding and margin is one of these.
pub mod space {
    use super::*;

    pub const S2: Pixels = px(2.);
    pub const S4: Pixels = px(4.);
    pub const S6: Pixels = px(6.);
    pub const S8: Pixels = px(8.);
    pub const S12: Pixels = px(12.);
    pub const S16: Pixels = px(16.);
    pub const S20: Pixels = px(20.);
    pub const S24: Pixels = px(24.);
    pub const S32: Pixels = px(32.);
    pub const S40: Pixels = px(40.);
}

/// Layout metrics, control heights, radii and icon sizes: the same in both appearances.
pub mod size {
    use super::*;

    // The window and the page.
    pub const TITLE_BAR: Pixels = px(40.);
    pub const SIDEBAR: Pixels = px(228.);
    pub const SIDEBAR_ROW: Pixels = px(32.);
    /// The page surface's inset from the window edge (right and bottom; the sidebar is on the left).
    pub const SURFACE_INSET: Pixels = space::S8;
    /// The page's content column never grows past this, centred in wide windows.
    pub const CONTENT_MAX: Pixels = px(760.);
    pub const PAGE_PADDING_X: Pixels = space::S32;
    pub const PAGE_PADDING_TOP: Pixels = space::S24;
    pub const PAGE_PADDING_BOTTOM: Pixels = space::S40;
    /// Between the page title and the first section.
    pub const TITLE_GAP: Pixels = space::S20;
    pub const SECTION_GAP: Pixels = space::S32;
    /// Between a section and an untitled one that continues it (with a hairline above its first row).
    pub const SECTION_GAP_UNTITLED: Pixels = space::S16;
    /// Under a section's heading, above its hairline.
    pub const SECTION_HEADING_PB: Pixels = space::S8;
    /// Above the content of a section that is not a list (tiles, a grid, a stage).
    pub const SECTION_BARE_PT: Pixels = space::S12;
    /// Above a section's footer.
    pub const FOOTER_PT: Pixels = space::S8;
    pub const ROW_MIN: Pixels = px(48.);
    /// Rows of a list page (one line each).
    pub const ROW_MIN_SINGLE: Pixels = px(44.);
    pub const ROW_PADDING_X: Pixels = space::S12;
    pub const ROW_PADDING_Y: Pixels = space::S12;
    /// Between a row's text and its control.
    pub const ROW_GAP: Pixels = space::S12;
    /// Dependent rows are indented by this.
    pub const ROW_INDENT: Pixels = space::S20;
    pub const HAIRLINE: Pixels = px(1.);

    // Control heights.
    /// Key caps, chips.
    pub const H_XS: Pixels = px(20.);
    /// Icon buttons in text, tabs, the back link, 試打.
    pub const H_SM: Pixels = px(24.);
    /// Every control in a row: pop-up, push button, segmented, search, ＋, stepper.
    pub const H_MD: Pixels = px(28.);
    /// Dialog and wizard footer buttons.
    pub const H_LG: Pixels = px(32.);
    /// Text inputs.
    pub const H_FIELD: Pixels = px(36.);
    /// The small key cap (the candidate window's footer).
    pub const KEYCAP_SM: Pixels = px(18.);

    // Radii: an inner radius is the outer one minus the padding between them.
    /// Key caps, checkboxes, segments.
    pub const R_XS: Pixels = px(4.);
    /// Buttons, inputs, pop-ups, the segmented control, a row's hover, menu items.
    pub const R_SM: Pixels = px(6.);
    /// Sidebar rows.
    pub const R_MD: Pixels = px(8.);
    /// Cards, popovers, menus, the candidate window.
    pub const R_LG: Pixels = px(10.);
    /// The page surface and the window.
    pub const R_XL: Pixels = px(12.);

    // Icons.
    pub const ICON_XS: Pixels = px(12.);
    pub const ICON_SM: Pixels = px(14.);
    pub const ICON_MD: Pixels = px(16.);
    #[allow(dead_code, reason = "part of the scale; the candidate window and wizard use their own today")]
    pub const ICON_LG: Pixels = px(20.);
    pub const ICON_XL: Pixels = px(24.);
    pub const SIDEBAR_ICON: Pixels = ICON_MD;

    pub const WINDOW_MIN_W: Pixels = px(640.);
    /// Below this window width pages drop decorations that only repeat a row's value.
    pub const NARROW_W: Pixels = px(760.);
    pub const WINDOW_MIN_H: Pixels = px(440.);
    pub const WINDOW_W: Pixels = px(900.);
    pub const WINDOW_H: Pixels = px(640.);
}

/// The type scale.  Only two weights: `weight::REGULAR` and `weight::SEMIBOLD` (CJK fonts have no medium).
pub mod text {
    use super::*;

    /// The wizard's welcome only.
    pub const DISPLAY: Pixels = px(28.);
    /// A page's own title.
    pub const TITLE: Pixels = px(22.);
    /// A wizard step's title.
    #[allow(dead_code, reason = "the wizard's step titles")]
    pub const STEP_TITLE: Pixels = px(24.);
    /// Section, card and dialog titles.
    pub const HEADLINE: Pixels = px(15.);
    pub const BODY: Pixels = px(14.);
    pub const SUBTITLE: Pixels = px(13.);
    /// Sidebar groups, key caps, chart labels.
    pub const CAPTION: Pixels = px(12.);
    /// The candidate window only.
    #[allow(dead_code, reason = "the candidate window's own sizes")]
    pub const MICRO: Pixels = px(11.);
}

/// Font weights: CJK fonts have only regular and bold, so nothing in between.
pub mod weight {
    use gpui_kit::FontWeight;

    pub const REGULAR: FontWeight = FontWeight::NORMAL;
    pub const SEMIBOLD: FontWeight = FontWeight::SEMIBOLD;
}

/// Elevation: 0 the page surface; 1 a card (card colour and a hairline, no shadow); 2 a floating surface (menus,
/// popovers, the candidate window): panel colour, a hairline and two shadows.
pub mod elevation {
    use gpui_kit::{BoxShadow, point, px};

    use super::Palette;

    /// The shadows of a floating surface.
    pub fn floating(p: &Palette) -> Vec<BoxShadow> {
        let (near, far) = if p.dark { (0.3, 0.5) } else { (0.06, 0.16) };
        let ink = if p.dark { gpui_kit::black() } else { p.text };
        vec![
            BoxShadow {
                color: ink.opacity(near),
                offset: point(px(0.), px(1.)),
                blur_radius: px(2.),
                spread_radius: px(0.),
                inset: false,
            },
            BoxShadow {
                color: ink.opacity(far),
                offset: point(px(0.), px(6.)),
                blur_radius: px(20.),
                spread_radius: px(-4.),
                inset: false,
            },
        ]
    }
}

/// Motion: the macOS app's durations.  Every animation checks `Motion::reduced` first.
pub mod motion {
    use super::*;

    pub const FAST: Duration = Duration::from_millis(120);
    pub const NORMAL: Duration = Duration::from_millis(200);
    /// A sub-page sliding in or out (macOS: 0.22 s).
    pub const PAGE: Duration = Duration::from_millis(220);
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

/// Zhuyin readings (the candidate window's reading row and annotations): the system's CJK font first, so the tone
/// marks (ˇ ˊ ˋ ˙, which Inter also has, in its Latin shapes) come from the same font as the Bopomofo letters.
pub fn zhuyin_font() -> gpui_kit::Font {
    let mut fonts = cjk_fallbacks();
    let family = fonts.remove(0);
    gpui_kit::Font {
        family: family.into(),
        features: gpui_kit::FontFeatures::default(),
        fallbacks: Some(gpui_kit::FontFallbacks::from_fonts(fonts)),
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
    let dark = zt_core::appearance::system_dark_fallback()
        .unwrap_or(matches!(window_appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark));
    cx.set_global(Tokens { palette: Palette::new(dark, Material::Opaque), forced: None });
    apply_component_theme(None, cx);
}

/// Whether the system is dark: the window system's appearance (GPUI follows Windows' setting and the XDG desktop
/// portal), or on Linux without the portal the desktop's own setting.
pub fn system_dark(window: &Window) -> bool {
    zt_core::appearance::system_dark_fallback()
        .unwrap_or_else(|| matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark))
}

/// Light or dark and the backdrop that 「主題」 and 「透明效果」 (or a forced appearance) ask for now.
fn wanted(window: &Window, cx: &App) -> (bool, Material, zt_panel::a11y::AccentChoice) {
    let settings = cx.try_global::<crate::app::AppState>().map(|state| state.settings.clone());
    let dark = cx.global::<Tokens>().forced.unwrap_or_else(|| {
        let choice = settings.as_ref().map_or(zt_core::appearance::Choice::System, zt_core::appearance::theme);
        choice.dark(system_dark(window))
    });
    let material = settings.as_ref().map_or(Material::Opaque, crate::platform::backdrop::material);
    let accent = settings.as_ref().map_or_else(Default::default, zt_core::system_look::accent);
    (dark, material, accent)
}

/// Follows 「主題」 (or the system appearance) and 「透明效果」, unless an appearance was forced.  Every window that
/// draws with the palette calls this first in `render`.
pub fn sync(window: &mut Window, cx: &mut App) {
    let (dark, material, accent) = wanted(window, cx);
    let current = cx.global::<Tokens>().palette;
    let next = Palette::new(dark, material).accented(accent);
    if current.dark != dark || current.material != material || current.accent != next.accent {
        cx.global_mut::<Tokens>().palette = next;
        apply_component_theme(Some(window), cx);
        cx.refresh_windows();
    }
}

/// Forces light or dark (`None`: follow 「主題」); renders and the screenshot mode use this.
pub fn force(dark: Option<bool>, window: &mut Window, cx: &mut App) {
    cx.global_mut::<Tokens>().forced = dark;
    let (dark, material, accent) = wanted(window, cx);
    cx.global_mut::<Tokens>().palette = Palette::new(dark, material).accented(accent);
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
    theme.radius = size::R_SM;
    theme.radius_lg = size::R_LG;
    // Linux with client-side decorations: rounded corners, a hairline and the shadow window_border draws.
    theme.window_radius = size::R_XL;
    theme.shadow = true;
    let colors = &mut theme.colors;
    colors.background = p.surface;
    colors.foreground = p.text;
    colors.muted = p.subtle;
    colors.muted_foreground = p.text_secondary;
    colors.border = p.separator;
    colors.input = p.control_border;
    colors.ring = p.accent;
    // Primary buttons are an inverted neutral; switches pass `accent_fill` themselves (`ui::controls`).
    colors.primary = p.primary;
    colors.primary_foreground = p.on_primary;
    colors.primary_hover = p.primary.opacity(0.88);
    colors.primary_active = p.primary.opacity(0.8);
    colors.button_primary = p.primary;
    colors.button_primary_foreground = p.on_primary;
    colors.button_primary_hover = p.primary.opacity(0.88);
    colors.button_primary_active = p.primary.opacity(0.8);
    colors.button = p.control;
    colors.button_foreground = p.text;
    colors.button_hover = p.hover;
    colors.button_active = p.pressed;
    colors.link = p.accent;
    colors.link_hover = p.accent;
    colors.link_active = p.accent.opacity(0.8);
    colors.progress_bar = p.accent_fill;
    colors.slider_bar = p.accent_fill;
    colors.slider_thumb = p.knob;
    colors.list_active = p.selection;
    colors.list_active_border = Hsla::transparent_black();
    colors.list_hover = p.hover;
    colors.tab_bar_segmented = p.subtle;
    colors.caret = p.text;
    colors.accent = p.hover;
    colors.accent_foreground = p.text;
    colors.popover = p.card;
    colors.secondary = p.subtle;
    colors.secondary_foreground = p.text;
    colors.secondary_hover = p.pressed;
    colors.secondary_active = p.pressed;
    colors.popover_foreground = p.text;
    colors.selection = p.accent_faint;
    colors.switch = p.switch_off;
    colors.title_bar = p.sidebar;
    colors.title_bar_border = p.separator;
    colors.window_border = if p.dark { ca(0xfffcf021) } else { ca(0x1e1c182e) };
    colors.sidebar = p.sidebar;
    colors.sidebar_foreground = p.text;
    colors.sidebar_border = p.separator;
    colors.sidebar_accent = p.pressed;
    colors.sidebar_accent_foreground = p.text;
    colors.danger = p.danger;
    colors.success = p.success;
    colors.scrollbar = Hsla::transparent_black();
    colors.scrollbar_thumb = if p.dark { ca(0xffffff33) } else { ca(0x00000033) };
    colors.switch_thumb = p.knob;
    theme.tokens = ThemeTokens::from(&theme.colors);
    // Over a backdrop the window's root paints nothing: the frame and the page draw their own tints.
    if p.material != Material::Opaque {
        theme.tokens.background = Hsla::transparent_black().into();
    }
    // The base layer (scrollbars, text views) keeps its own copy of the theme, made by `Theme::change` above, before
    // the font and colours were set: without this it reads the platform's ".SystemUIFont" (no CJK fallbacks; on Linux
    // possibly no font at all) and the library's default colours.
    Theme::sync_base(cx);
}

// MARK: - Candidate window tokens (append-only block, owned by the candidate window)

/// The candidate window's own tokens: it floats over other apps, so it has its own surface, hairlines, selection and
/// shadows (an elevation-2 surface: panel colour, hairline border, two shadows).  Every place that draws the window
/// (the floating window, the 「視窗與提示」 preview, the 介紹 stage, the demos' mini lists) uses these.
pub mod candidate {
    use std::sync::atomic::{AtomicBool, Ordering};

    use gpui_kit::{BoxShadow, Hsla, point, px};
    use zt_panel::a11y::AccentChoice;

    use super::Palette;

    pub use zt_panel::tokens::{CHEVRON, KEYCAP_FONT, KEYCAP_RADIUS, NOTICE_ICON, RADIUS};

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
        pub text: Hsla,
        pub text_secondary: Hsla,
        pub text_tertiary: Hsla,
        pub success: Hsla,
        /// The selected row's candidate, and its number and annotation (`zt_panel::tokens::Colors`).
        pub selection_text: Hsla,
        pub selection_secondary: Hsla,
        /// The window's border width: 1, or 2 in high contrast.
        pub stroke: f32,
        /// High contrast: an outline around the selected row.
        pub selection_outline: Option<Hsla>,
    }

    /// 「強調色」 as the settings last said it (`set_accent`): the candidate window is drawn from a palette alone, in
    /// many places (the floating window, previews, the wizard), and each of them reads the settings just before.
    static ACCENT: AtomicBool = AtomicBool::new(false);

    /// Records 「強調色」 for [`colors`]: true is ZhuyinType's own look.
    pub fn set_accent(choice: AccentChoice) {
        ACCENT.store(choice == AccentChoice::Zhuyintype, Ordering::Relaxed);
    }

    fn accent() -> AccentChoice {
        if ACCENT.load(Ordering::Relaxed) { AccentChoice::Zhuyintype } else { AccentChoice::System }
    }

    fn hsla(color: zt_panel::tokens::Rgba) -> Hsla {
        gpui_kit::Rgba { r: color.r, g: color.g, b: color.b, a: color.a }.into()
    }

    /// The window's colours for `p`'s appearance (`zt_panel::tokens`, shared with the Windows text service) with the
    /// system's accent or high contrast (`zt_panel::a11y::resolve`), and the shadows this app draws itself.
    pub fn colors(p: &Palette) -> Colors {
        let dark = p.dark;
        let shared = zt_panel::a11y::resolve(dark, accent(), &zt_core::system_look::look(dark));
        let pick = |light: f32, dark_value: f32| if dark { dark_value } else { light };
        let shadow = |alpha: f32, y: f32, blur: f32| BoxShadow {
            color: hsla(shared.shadow.fade(alpha)),
            offset: point(px(0.), px(y)),
            blur_radius: px(blur),
            spread_radius: px(if blur > 2. { -4. } else { 0. }),
            inset: false,
        };
        Colors {
            surface: hsla(shared.surface),
            border: hsla(shared.border),
            outline: dark.then(|| gpui_kit::black().opacity(0.4)),
            selection: hsla(shared.selection),
            hover: hsla(shared.hover),
            hairline: hsla(shared.hairline),
            keycap: hsla(shared.keycap),
            keycap_on: hsla(shared.keycap_on),
            text: hsla(shared.text),
            text_secondary: hsla(shared.text_secondary),
            text_tertiary: hsla(shared.text_tertiary),
            success: hsla(shared.success),
            selection_text: hsla(shared.selection_text),
            selection_secondary: hsla(shared.selection_secondary),
            stroke: shared.stroke,
            selection_outline: shared.selection_outline.map(hsla),
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

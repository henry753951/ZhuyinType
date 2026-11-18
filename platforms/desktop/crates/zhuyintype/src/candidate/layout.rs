//! The candidate window's geometry (`zt_panel::layout`, shared with the Windows text service), measured with GPUI's
//! text system, for a `Presentation`.

use gpui_kit::{FontWeight, TextRun, Window, px};
use zt_core::presentation::Presentation;
pub use zt_panel::layout::{
    BADGE_FONT, BADGE_H, BADGE_W, CONTROL_H, Estimate, Frame, HintLayout, KEYCAP_HEIGHT, Measure, MenuLayout,
    NoticeLayout, PanelLayout, RowLayout, SMALL_FONT, ToggleLayout, UNDO_LABEL, annotation_font,
};

use super::style::PanelStyle;
use crate::theme;

/// Measures with the window's text system (shaped lines are cached per frame by GPUI).
pub struct WindowMeasure<'a>(pub &'a Window);

impl WindowMeasure<'_> {
    fn measure(&self, text: &str, size: f32, run_font: gpui_kit::Font) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let run = TextRun {
            len: text.len(),
            font: run_font,
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(self.0.text_system().layout_line(text, px(size), &[run], None).width)
    }
}

impl Measure for WindowMeasure<'_> {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32 {
        let mut font = theme::font();
        font.weight = if medium { FontWeight::MEDIUM } else { FontWeight::NORMAL };
        self.measure(text, size, font)
    }

    /// The reading row, the typed reading and the annotations are drawn in `theme::zhuyin_font`.
    fn zhuyin_width(&mut self, text: &str, size: f32) -> f32 {
        self.measure(text, size, theme::zhuyin_font())
    }
}

/// What the window shows besides the presentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras<'a> {
    /// The composition, shown in the window when the app cannot show it inline.
    pub preview: Option<&'a str>,
    /// The right-click menu is open on this row of the page.
    pub menu: Option<usize>,
}

/// Lays the window out for `p` (`zt_panel::layout::compute`).
pub fn compute(
    p: &Presentation,
    extras: Extras<'_>,
    style: &PanelStyle,
    max_width: f32,
    steady: (f32, f32),
    measure: &mut dyn Measure,
) -> PanelLayout {
    let mut model = p.panel_model(extras.preview);
    model.menu = extras.menu;
    zt_panel::layout::compute(&model, style, max_width, steady, measure)
}

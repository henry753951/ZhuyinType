//! The candidate window's geometry (`zt_panel::layout`, shared with the Windows text service), measured with GPUI's
//! text system, for a `Presentation`.

use gpui_kit::{FontWeight, TextRun, Window, px};
use zt_core::presentation::Presentation;
pub use zt_panel::layout::{
    BADGE_FONT, BADGE_H, BADGE_W, CONTROL_H, Estimate, Frame, HintLayout, KEYCAP_HEIGHT, Measure, NoticeLayout,
    PanelLayout, RowLayout, SMALL_FONT, ToggleLayout, annotation_font,
};

use super::style::PanelStyle;
use crate::theme;

/// Measures with the window's text system (shaped lines are cached per frame by GPUI).
pub struct WindowMeasure<'a>(pub &'a Window);

impl Measure for WindowMeasure<'_> {
    fn width(&mut self, text: &str, size: f32, medium: bool) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let mut run_font = theme::font();
        run_font.weight = if medium { FontWeight::MEDIUM } else { FontWeight::NORMAL };
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

/// What the window shows besides the presentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras<'a> {
    /// The composition, shown in the window when the app cannot show it inline.
    pub preview: Option<&'a str>,
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
    zt_panel::layout::compute(&p.panel_model(extras.preview), style, max_width, steady, measure)
}

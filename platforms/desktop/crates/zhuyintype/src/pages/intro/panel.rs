//! The candidate window on the 介紹 stage: the shared candidate window element (`candidate::panel`) at the default
//! size, so the stage shows exactly the window the user will see.  Everything else draws through these two
//! functions only.

use gpui_kit::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use zt_core::presentation::Presentation;

use super::script::PanelKeys;
use crate::candidate::layout::{self, Estimate, Extras, WindowMeasure};
use crate::candidate::panel::CandidatePanel;
use crate::candidate::style::{KeyLabels, Layout, PanelStyle};
use crate::theme::Palette;

/// The stage's candidates' size (the window's default 字級).
const FONT_SIZE: f32 = 18.;

/// From the window's left edge to its candidates' text: the window is placed so that text lines up with the
/// composition, as the real one.  (The element shifts itself by the difference to the measured inset, so the text
/// lines up whatever the number column's measured width.)
pub const TEXT_INSET: f32 = 27.;

/// The window's style for the stage's keys.
fn style(keys: &PanelKeys) -> PanelStyle {
    PanelStyle {
        layout: if keys.vertical { Layout::Vertical } else { Layout::Horizontal },
        font_size: FONT_SIZE,
        compact: false,
        shows_zhuyin_hint: true,
        shows_annotations: true,
        shows_list_switches: keys.switches,
        keys: KeyLabels {
            zhuyin: keys.zhuyin.clone().unwrap_or_default(),
            single: keys.single.clone().unwrap_or_default(),
            corrections: keys.corrections.clone().unwrap_or_default(),
            zhuyin_clickable: false,
            ..KeyLabels::default()
        },
    }
}

/// The window's natural height for `p` (the stage keeps room for the tallest).  Heights do not depend on text
/// widths, so an estimate measures.
pub fn panel_height(p: &Presentation, keys: &PanelKeys) -> f32 {
    if p.rows.is_empty() && p.notice.is_none() {
        return 0.;
    }
    layout::compute(p, Extras::default(), &style(keys), 4000., (0., 0.), &mut Estimate).height
}

/// The candidate window showing `p`, in `palette` (the window's own appearance).
pub fn candidate_panel(p: &Presentation, keys: &PanelKeys, palette: &Palette, _cx: &App) -> AnyElement {
    StagePanel { presentation: p.clone(), style: style(keys), palette: *palette }.into_any_element()
}

#[derive(IntoElement)]
struct StagePanel {
    presentation: Presentation,
    style: PanelStyle,
    palette: Palette,
}

impl RenderOnce for StagePanel {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let mut panel = CandidatePanel::new();
        panel.hover = false;
        panel.animates = false;
        panel.update(&self.presentation, Extras::default(), &self.style, 4000., false, &mut WindowMeasure(window));
        let inset = panel.layout().map_or(TEXT_INSET, |layout| layout.text_inset);
        div().ml(px(TEXT_INSET - inset)).child(panel.render(&self.palette, None, window))
    }
}

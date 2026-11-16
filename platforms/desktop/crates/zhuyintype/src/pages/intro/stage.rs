//! The 介紹 stage (macOS `IntroStage`): a plain document, the line being typed with its composition underlined and
//! the caret, the key just pressed beside the caret, and the candidate window lined up with
//! the composition below (or above) the line, as the real window sits on screen.

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, IntoElement, ParentElement, Pixels, SharedString, Styled, div,
    ease_out_quint, px,
};
use zt_core::presentation::Presentation;

use super::panel::{self, TEXT_INSET};
use super::script::{PanelKeys, Segment};
use crate::candidate::panel::mode_badge;
use crate::theme::{self, Palette, size, space, text};
use crate::ui::{self, h};

const MARGIN: f32 = 20.;
const LINE_TOP: f32 = 16.;
const LINE_FONT: f32 = 16.;
const LINE_H: f32 = 24.;
/// Between the line and the window (macOS `InputPanelPlacement.gap`).
const GAP: f32 = 4.;
const BOTTOM: f32 = 16.;
/// The 中/英 badge's side.
const BADGE: f32 = 44.;
/// Where a control in the stage's upper right corner sits (the 試打 button, `H_SM` tall): centred on the first line,
/// `MARGIN` in from the right edge.
pub const CORNER_TOP: Pixels = px(LINE_TOP + (LINE_H - 24.) / 2.);
pub const CORNER_RIGHT: Pixels = px(MARGIN);

/// What the stage shows at one moment.
pub struct Scene<'a> {
    pub committed: &'a str,
    pub marked: &'a [Segment],
    /// Text after the caret (the try-it field).
    pub after: &'a str,
    pub panel: Option<&'a Presentation>,
    pub toast: Option<&'a str>,
    pub keys: &'a [String],
    /// Changes with every step, so the pressed key presses again.
    pub trigger: usize,
    /// The step where the window opened, so it animates in once.
    pub panel_since: usize,
}

/// The stage's height for the tallest window it will show.
pub fn height(tallest_panel: f32) -> f32 {
    LINE_TOP + LINE_H + GAP + tallest_panel + BOTTOM
}

/// The composition's clauses as segments (the try-it field).
pub fn segments_of(p: &Presentation) -> Vec<Segment> {
    let units: Vec<u16> = p.composition.encode_utf16().collect();
    let mut segments = Vec::new();
    let mut at = 0;
    for (index, range) in p.clauses.iter().enumerate() {
        if range.start > at && range.start <= units.len() {
            segments.push(Segment { text: String::from_utf16_lossy(&units[at..range.start]), focus: false });
        }
        if range.start <= range.end && range.end <= units.len() {
            segments.push(Segment {
                text: String::from_utf16_lossy(&units[range.clone()]),
                focus: p.focused_clause == Some(index),
            });
            at = range.end;
        }
    }
    if at < units.len() {
        segments.push(Segment { text: String::from_utf16_lossy(&units[at..]), focus: false });
    }
    segments
}

fn line_text(text: impl Into<SharedString>) -> gpui_kit::Div {
    div().text_size(px(LINE_FONT)).line_height(px(LINE_H)).whitespace_nowrap().child(text.into())
}

/// The stage, `height` tall, showing `scene`; `window` is the candidate window's own palette.
#[allow(clippy::too_many_arguments)]
pub fn stage(
    scene: &Scene<'_>,
    keys: &PanelKeys,
    above: bool,
    animated: bool,
    height: f32,
    window: &Palette,
    caret_blinks: bool,
    cx: &App,
) -> AnyElement {
    let p = theme::palette(cx);
    let document = p.card;
    let line_top = if above { height - LINE_TOP - LINE_H } else { LINE_TOP };
    let caret = div().w(px(1.5)).h(px(LINE_H - 4.)).bg(p.text);
    let caret: AnyElement = if caret_blinks {
        caret
            .with_animation("intro-caret", Animation::new(Duration::from_millis(1060)).repeat(), |this, t| {
                this.opacity(if t < 0.5 { 1. } else { 0. })
            })
            .into_any_element()
    } else {
        caret.into_any_element()
    };
    // The key just pressed, in the stage's lower right corner like a keystroke overlay (beside the caret it read as a
    // button in the text).  A key written as a shortcut (「⇧↑」) shows one cap per key.
    let pressed = (!scene.keys.is_empty()).then(|| {
        h().absolute()
            .right(px(MARGIN))
            .bottom(px(BOTTOM))
            .gap(space::S8)
            .children(scene.keys.iter().map(|key| ui::keycap::binding(key, None)))
            .with_animation(
                SharedString::from(format!("intro-key-{}", scene.trigger)),
                Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
                |this, t| this.opacity((t * 3.).min(1.)).mt(px(2. * (1. - t))),
            )
    });
    let line = h()
        .absolute()
        .top(px(line_top))
        .left(px(MARGIN))
        .text_color(p.text)
        .child(line_text(scene.committed.to_owned()))
        // The composition underlined, the clause being chosen with a heavier line on a faint fill; neutral, as the
        // rest of the window (no accent in text).
        .child(h().gap(px(1.5)).children(scene.marked.iter().map(|segment| {
            line_text(segment.text.clone())
                .when(!segment.focus, |this| this.border_b_1().border_color(p.text_tertiary))
                .when(segment.focus, |this| this.bg(p.selection).border_b_2().border_color(p.text))
        })))
        .child(caret)
        .child(line_text(scene.after.to_owned()));
    let panel_top = if above { None } else { Some(line_top + LINE_H + GAP) };
    let panel = scene.panel.filter(|p| !p.rows.is_empty() || p.notice.is_some()).map(|presentation| {
        let natural = panel::panel_height(presentation, keys);
        let top = panel_top.unwrap_or(line_top - GAP - natural);
        let window_element = panel::candidate_panel(presentation, keys, window, cx);
        // The window's candidates line up with the clause they replace: the text before it (committed, and the
        // composition's earlier clauses) is laid out invisibly to find its x.
        let lead = if scene.marked.iter().any(|segment| segment.focus) {
            let before = scene.marked.iter().take_while(|segment| !segment.focus).count();
            h().child(line_text(scene.committed.to_owned())).when(before > 0, |this| {
                this.child(
                    h().gap(px(1.5))
                        .children(scene.marked[..before].iter().map(|segment| line_text(segment.text.clone())))
                        .child(div()),
                )
            })
        } else {
            h().child(line_text(scene.committed.to_owned()))
        };
        let layer = h()
            .absolute()
            .top(px(top))
            .left(px(MARGIN))
            .items_start()
            .child(lead.invisible())
            .child(div().ml(px(-TEXT_INSET)).child(window_element));
        if animated {
            let slide = if above { 6. } else { -6. };
            layer
                .with_animation(
                    SharedString::from(format!("intro-panel-{}", scene.panel_since)),
                    Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
                    move |this, t| this.opacity(t).mt(px(slide * (1. - t))),
                )
                .into_any_element()
        } else {
            layer.into_any_element()
        }
    });
    let toast = scene.toast.map(|label| {
        let marked: String = scene.marked.iter().map(|s| s.text.as_str()).collect();
        let top = if above { line_top - GAP - BADGE } else { line_top + LINE_H + GAP };
        h().absolute()
            .top(px(top))
            .left(px(MARGIN))
            .child(line_text(format!("{}{marked}", scene.committed)).invisible())
            .child(mode_badge(label, window, true))
            .with_animation(
                SharedString::from(format!("intro-toast-{}", scene.trigger)),
                Animation::new(Duration::from_millis(160)),
                |this, t| this.opacity(t),
            )
    });
    div()
        .relative()
        .w_full()
        .h(px(height))
        .rounded(size::R_LG)
        .overflow_hidden()
        .bg(document)
        .border_1()
        .border_color(p.separator)
        .child(line)
        .children(panel)
        .children(toast)
        .children(pressed)
        .into_any_element()
}

/// The caption under the stage.
pub fn caption(caption: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    div()
        .w_full()
        .min_h(text::SUBTITLE * 1.5)
        .text_center()
        .text_size(text::SUBTITLE)
        .text_color(p.text_secondary)
        .child(caption.into())
}

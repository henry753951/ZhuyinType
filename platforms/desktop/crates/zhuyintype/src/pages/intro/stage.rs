//! The 介紹 stage (macOS `IntroStage`): a plain document with a few grey lines, the line being typed with its
//! composition underlined and the caret, the key just pressed beside the caret, and the candidate window lined up with
//! the composition below (or above) the line, as the real window sits on screen.

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    ease_out_quint, px, relative,
};
use zt_core::presentation::Presentation;

use super::panel::{self, TEXT_INSET};
use super::script::{PanelKeys, Segment};
use crate::theme::{self, Palette, text};
use crate::ui::{self, h, v};

const MARGIN: f32 = 20.;
const LINE_TOP: f32 = 18.;
const LINE_FONT: f32 = 16.;
const LINE_H: f32 = 22.;
/// Between the line and the window (macOS `InputPanelPlacement.gap`).
const GAP: f32 = 4.;
const BOTTOM: f32 = 22.;

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
    let fractions = [0.92, 0.74, 0.86, 0.55, 0.8, 0.66, 0.9, 0.5, 0.78, 0.6, 0.84, 0.7, 0.88, 0.62, 0.8, 0.7];
    let backdrop = v()
        .absolute()
        .top(px(14.))
        .left(px(MARGIN))
        .right(px(MARGIN))
        .gap(px(11.))
        .children(fractions.iter().map(|f| div().h(px(7.)).w(relative(*f)).rounded_full().bg(p.pressed.opacity(0.55))));
    let band = div().absolute().left_0().right_0().top(px(line_top - 5.)).h(px(LINE_H + 10.)).bg(document);
    let caret = div().w(px(1.5)).h(px(LINE_H - 2.)).bg(p.text);
    let caret: AnyElement = if caret_blinks {
        caret
            .with_animation(
                "intro-caret",
                Animation::new(Duration::from_millis(1060)).repeat(),
                |this, t| this.opacity(if t < 0.5 { 1. } else { 0. }),
            )
            .into_any_element()
    } else {
        caret.into_any_element()
    };
    let pressed = (!scene.keys.is_empty()).then(|| {
        h().ml(px(9.))
            .gap(px(3.))
            .children(scene.keys.iter().map(|key| ui::keycap(key.clone())))
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
        .child(h().gap(px(1.5)).children(scene.marked.iter().map(|segment| {
            line_text(segment.text.clone())
                .when(!segment.focus, |this| this.border_b_1().border_color(p.text_secondary))
                .when(segment.focus, |this| this.bg(p.accent_faint).border_b_2().border_color(p.accent))
        })))
        .child(caret)
        .child(line_text(scene.after.to_owned()))
        .children(pressed);
    let panel_top = if above { None } else { Some(line_top + LINE_H + GAP) };
    let panel = scene.panel.filter(|p| !p.rows.is_empty() || p.notice.is_some()).map(|presentation| {
        let natural = panel::panel_height(presentation, keys);
        let top = panel_top.unwrap_or(line_top - GAP - natural);
        let window_element = panel::candidate_panel(presentation, keys, window, cx);
        let layer = h()
            .absolute()
            .top(px(top))
            .left(px(MARGIN))
            .items_start()
            .child(line_text(scene.committed.to_owned()).invisible())
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
        let top = if above { line_top - GAP - 48. } else { line_top + LINE_H + GAP };
        h().absolute()
            .top(px(top))
            .left(px(MARGIN))
            .child(line_text(format!("{}{marked}", scene.committed)).invisible())
            .child(
                div()
                    .size(px(48.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .bg(window.panel)
                    .border_1()
                    .border_color(window.panel_border)
                    .shadow_lg()
                    .text_size(px(22.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(window.text)
                    .child(label.to_owned()),
            )
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
        .rounded(px(10.))
        .overflow_hidden()
        .bg(document)
        .border_1()
        .border_color(p.separator)
        .child(backdrop)
        .child(band)
        .child(line)
        .children(panel)
        .children(toast)
        .into_any_element()
}

/// The caption under the stage.
pub fn caption(caption: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    div().w_full().min_h(px(18.)).text_center().text_size(text::SUBTITLE).text_color(p.text_secondary).child(caption.into())
}

//! One moment of the 「視窗與提示」 preview (and of the floating preview after a change) (macOS `PreviewScene`, `SceneLayout`, `PreviewBlock`): a line of a
//! document with the keys just pressed beside it, and the real candidate window (or the 中/英 badge) lined up with the
//! composition below or above it, as on the screen.  The window is always drawn at 100 %; a scene too narrow for it
//! shows fewer candidates instead (the preview decides).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};
use zt_core::presentation::Presentation;

use super::layout::{self, BADGE_H, BADGE_W, Extras, Measure, PanelLayout};
use super::panel::{CandidatePanel, key_glyphs, mode_badge};
use super::style::PanelStyle;
use crate::theme::{self, Palette, size, text};
use crate::ui::h;

/// A segment of the composition.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub text: String,
    /// The span the list changes (or that is selected): tinted, with an accent underline.
    pub focus: bool,
}

pub fn seg(text: &str) -> Segment {
    Segment { text: text.to_owned(), focus: false }
}

pub fn focus(text: &str) -> Segment {
    Segment { text: text.to_owned(), focus: true }
}

/// One still moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub title: &'static str,
    pub committed: String,
    pub marked: Vec<Segment>,
    pub keys: Vec<String>,
    pub note: Option<String>,
    pub list: Option<Presentation>,
    pub notice: Option<String>,
    pub badge: Option<&'static str>,
    /// 「不顯示提示」 instead of the badge (中英提示 is off).
    pub badge_off: bool,
}

impl Scene {
    /// The window's content: the list, or the notice alone.
    pub fn presentation(&self) -> Option<Presentation> {
        match (&self.list, &self.notice) {
            (Some(list), notice) => Some(Presentation { notice: notice.clone(), ..list.clone() }),
            (None, Some(notice)) => Some(Presentation { notice: Some(notice.clone()), ..Presentation::default() }),
            (None, None) => None,
        }
    }
}

/// The document line's text: `theme::text::HEADLINE`, as a number for measuring.
pub const FONT: f32 = 15.;
pub const LINE_HEIGHT: f32 = 20.;
pub const INSET: f32 = 12.;
const GAP: f32 = 4.;
const SEGMENT_GAP: f32 = 1.5;

/// Where things go in a scene's card at the card's width.
#[derive(Clone, Debug)]
pub struct SceneLayout {
    pub width: f32,
    pub height: f32,
    pub above: bool,
    pub line_top: f32,
    /// The window's size and place (x; y set by `fitted`).
    pub panel: Option<(f32, f32, f32, f32)>,
    pub badge: Option<(f32, f32)>,
    pub keycap_x: f32,
    /// Each marked segment's x and width.
    pub segments: Vec<(f32, f32)>,
    pub caret_x: f32,
}

impl SceneLayout {
    pub fn new(scene: &Scene, width: f32, style: &PanelStyle, above: bool, measure: &mut dyn Measure) -> Self {
        let committed_w = measure.width(&scene.committed, FONT, false);
        let start = INSET + committed_w;
        let mut x = start;
        let mut segments = Vec::new();
        for (index, segment) in scene.marked.iter().enumerate() {
            if index > 0 {
                x += SEGMENT_GAP;
            }
            let w = measure.width(&segment.text, FONT, false);
            segments.push((x, w));
            x += w;
        }
        let caret_x = x;
        let panel = scene.presentation().map(|presentation| {
            let layout = panel_layout(&presentation, style, measure);
            let x = (start - layout.text_inset).max(6.).min((width - layout.width - 6.).max(6.));
            (x, 0., layout.width, layout.height)
        });
        let badge = scene.badge.map(|_| (caret_x.max(6.).min((width - BADGE_W - 6.).max(6.)), 0.));
        let mut layout = Self {
            width,
            height: 0.,
            above,
            line_top: 0.,
            panel,
            badge,
            keycap_x: caret_x + 7.,
            segments,
            caret_x,
        };
        let natural = layout.natural_height(scene);
        layout.fit(natural);
        layout
    }

    pub fn natural_height(&self, scene: &Scene) -> f32 {
        let content = self
            .panel
            .map(|panel| panel.3)
            .or(self.badge.map(|_| BADGE_H))
            .unwrap_or(if scene.badge_off { 16. } else { 0. });
        INSET * 2. + LINE_HEIGHT + if content > 0. { GAP + content } else { 0. }
    }

    /// The window is wider than the scene (the caller shows fewer candidates).
    pub fn overflows(&self) -> bool {
        self.panel.is_some_and(|(x, _, w, _)| x + w > self.width - 4.)
    }

    /// The same scene in a card `height` tall: the line at the top, or at the bottom when the window opens above.
    pub fn fit(&mut self, height: f32) {
        self.height = height;
        self.line_top = if self.above { height - INSET - LINE_HEIGHT } else { INSET };
        let below = self.line_top + LINE_HEIGHT + GAP;
        if let Some(panel) = &mut self.panel {
            panel.1 = if self.above { self.line_top - GAP - panel.3 } else { below };
        }
        if let Some(badge) = &mut self.badge {
            badge.1 = if self.above { self.line_top - GAP - BADGE_H } else { below };
        }
    }
}

/// The window's layout for a scene, measured with the real layout (no motion state).
pub fn panel_layout(presentation: &Presentation, style: &PanelStyle, measure: &mut dyn Measure) -> PanelLayout {
    layout::compute(presentation, Extras::default(), style, 4000., (0., 0.), measure)
}

/// What a scene's card draws: the scene, where things go, and the preview's window (already updated for it).
pub struct Card<'a> {
    pub scene: &'a Scene,
    pub layout: &'a SceneLayout,
    pub panel: &'a CandidatePanel,
    pub panel_palette: &'a Palette,
    /// Changes when the keys are pressed again (they press once per change).
    pub key_generation: SharedString,
    pub animated: bool,
}

/// Draws a scene's card.
pub fn scene_card(card: Card<'_>, window: &mut Window, cx: &App) -> AnyElement {
    let Card { scene, layout, panel, panel_palette, key_generation, animated } = card;
    let p = theme::palette(cx);
    let line_top = layout.line_top;
    let document = div()
        .absolute()
        .left(px(INSET))
        .top(px(line_top))
        .h(px(LINE_HEIGHT))
        .text_size(text::HEADLINE)
        .line_height(px(LINE_HEIGHT))
        .text_color(p.text)
        .whitespace_nowrap()
        .child(SharedString::from(scene.committed.clone()));
    let segments = scene.marked.iter().zip(&layout.segments).map(|(segment, (x, w))| {
        div()
            .absolute()
            .left(px(*x))
            .top(px(line_top))
            .w(px(*w))
            .h(px(LINE_HEIGHT + 2.))
            .when(segment.focus, |this| this.bg(p.accent_faint))
            .text_size(text::HEADLINE)
            .line_height(px(LINE_HEIGHT))
            .text_color(p.text)
            .whitespace_nowrap()
            .child(SharedString::from(segment.text.clone()))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .w_full()
                    .h(px(if segment.focus { 2. } else { 1. }))
                    .bg(if segment.focus { p.accent } else { p.text }),
            )
    });
    let caret = (!scene.marked.is_empty() || !scene.committed.is_empty()).then(|| {
        div().absolute().left(px(layout.caret_x + 0.5)).top(px(line_top)).w(px(1.5)).h(px(LINE_HEIGHT + 1.)).bg(p.text)
    });
    let keys = (!scene.keys.is_empty()).then(|| {
        h().absolute()
            .left(px(layout.keycap_x))
            .top(px(line_top - 3.))
            .gap(px(4.))
            .children(scene.keys.iter().map(|key| pressed_keycap(key, &p)))
            .when_some(scene.note.clone(), |this, note| {
                this.child(
                    div().pl(px(2.)).text_size(text::CAPTION).text_color(p.text_tertiary).child(SharedString::from(note)),
                )
            })
            .with_animation(
                SharedString::from(format!("keys-{key_generation}")),
                Animation::new(std::time::Duration::from_millis(if animated { 260 } else { 1 })),
                |this, t| {
                    // Pressed and let go: down for the first third, back up after.
                    let down = if t < 0.35 { t / 0.35 } else { 1. - (t - 0.35) / 0.65 };
                    this.mt(px(1.5 * down.clamp(0., 1.)))
                },
            )
    });
    let window_box = layout.panel.map(|(x, y, w, h)| {
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .h(px(h))
            .child(panel.render(panel_palette, None, window))
    });
    let badge = match (layout.badge, scene.badge) {
        (Some((x, y)), Some(text)) => Some(div().absolute().left(px(x)).top(px(y)).child(mode_badge(text, panel_palette, true))),
        _ => None,
    };
    let badge_off = scene.badge_off.then(|| {
        div()
            .absolute()
            .left(px(INSET))
            .top(px(if layout.above { line_top - 20. } else { line_top + LINE_HEIGHT + 4. }))
            .text_size(text::CAPTION)
            .text_color(p.text_tertiary)
            .child("不顯示提示")
    });
    div()
        .relative()
        .w(px(layout.width))
        .h(px(layout.height))
        .rounded(size::RADIUS_CARD)
        .bg(p.window)
        .border_1()
        .border_color(p.separator)
        .overflow_hidden()
        .child(document)
        .children(segments)
        .children(caret)
        .children(keys)
        .children(window_box)
        .children(badge)
        .children(badge_off)
        .into_any_element()
}

/// A key cap as just pressed (macOS `PressedKeycap` at rest).
fn pressed_keycap(key: &str, p: &Palette) -> impl IntoElement {
    div()
        .h(px(24.))
        .min_w(px(24.))
        .px(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(size::RADIUS_CONTROL)
        .bg(p.control)
        .border_1()
        .border_color(p.control_border)
        .shadow_xs()
        .font_weight(FontWeight::MEDIUM)
        .child(key_glyphs(key, 13., p.text))
}

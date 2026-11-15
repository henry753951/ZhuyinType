//! Small looping demos that show what an option does instead of describing it in a paragraph (macOS
//! `SettingsComponents.swift`: `StoryClock`, `DemoLine`, `MiniDemo`, `MiniList`, `ChoiceDemo`, `FactRow`,
//! `PressedKeycap`, `PlayPauseButton`).
//!
//! A demo is a list of `DemoFrame`s: the key just pressed, the text line after it, maybe a small candidate list, and
//! a caption.  `StoryClock` steps through them, holding each for its own time, and loops; with reduced motion it starts
//! paused on its resting frame.  The clock is element state (`story_clock`), so a demo keeps its place while it is
//! shown and stops when it is gone (an info card closes, a page changes).

// Shared components: the 介紹 page and the setup wizard use the parts these pages do not yet.
#![allow(dead_code)]

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, ElementId, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Task, Window, div, ease_out_quint, px,
};

use super::keycap::keycap;
use super::{h, v};
use crate::theme::{self, Palette, motion, size, space, text, weight};

/// What an ⓘ card shows, built when the card opens: pages list their cards as (title, content) for `--popover`.
pub type InfoContent = fn(&mut Window, &mut App) -> AnyElement;

// MARK: - Frames

/// How a run of demo text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentStyle {
    /// Committed text in the document.
    Document,
    /// Composed text: a thin underline per word.
    Marked,
    /// The part the candidate window changes: a thick accent underline on a neutral wash.
    Focus,
    /// The character ↓ would change: a thick accent underline.
    Target,
    /// What a demo points at, such as the text read before the cursor.
    Highlight,
    /// Text the demo shows as no longer used.
    Faded,
}

/// A run of text on a demo line: a word, or Zhuyin still being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub text: SharedString,
    pub style: SegmentStyle,
}

impl Segment {
    pub fn new(text: impl Into<SharedString>, style: SegmentStyle) -> Self {
        Self { text: text.into(), style }
    }
    pub fn document(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Document)
    }
    pub fn marked(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Marked)
    }
    pub fn focus(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Focus)
    }
    pub fn target(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Target)
    }
    pub fn highlight(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Highlight)
    }
    pub fn faded(text: impl Into<SharedString>) -> Self {
        Self::new(text, SegmentStyle::Faded)
    }

    fn composed(&self) -> bool {
        matches!(self.style, SegmentStyle::Marked | SegmentStyle::Focus | SegmentStyle::Target)
    }
}

/// One step of a demo.
#[derive(Clone, Debug)]
pub struct DemoFrame {
    /// The keys just pressed, drawn as key caps.
    pub keys: Vec<SharedString>,
    pub line: Vec<Segment>,
    /// The caret after the segment at this index (None: after everything).
    pub caret: Option<usize>,
    /// A small candidate list under the line; "今 ㄐㄧㄣ" is a candidate with its corrected reading.
    pub list: Option<Vec<SharedString>>,
    pub caption: SharedString,
    pub hold: Duration,
}

impl DemoFrame {
    /// A frame showing `line`, captioned, held for `hold` seconds.
    pub fn new(line: impl IntoIterator<Item = Segment>, caption: impl Into<SharedString>, hold: f32) -> Self {
        Self {
            keys: Vec::new(),
            line: line.into_iter().collect(),
            caret: None,
            list: None,
            caption: caption.into(),
            hold: Duration::from_secs_f32(hold),
        }
    }

    pub fn keys(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keys = keys.into_iter().map(Into::into).collect();
        self
    }

    pub fn caret(mut self, after: usize) -> Self {
        self.caret = Some(after);
        self
    }

    pub fn list(mut self, items: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.list = Some(items.into_iter().map(Into::into).collect());
        self
    }
}

// MARK: - The story clock

/// Steps through frames held for their own times, and loops.  Paused, it rests on `resting` (or stays where it is).
/// With reduced motion it starts paused on its resting frame (the last one when none is given).
pub struct StoryClock {
    index: usize,
    /// Bumped on every step, so a key pressed again presses again.
    step: usize,
    playing: bool,
    holds: Vec<Duration>,
    resting: Option<usize>,
    task: Option<Task<()>>,
}

impl StoryClock {
    pub fn new(holds: Vec<Duration>, resting: Option<usize>, cx: &mut Context<Self>) -> Self {
        let reduced = cx.reduce_motion();
        let last = holds.len().saturating_sub(1);
        let mut clock = Self { index: 0, step: 0, playing: !reduced, holds, resting, task: None };
        if reduced {
            clock.index = resting.unwrap_or(last).min(last);
        }
        // `ZT_DEMO_FRAME=N`: every demo holds still on frame N (the last one when shorter), for screenshots.
        if let Some(frame) = std::env::var("ZT_DEMO_FRAME").ok().and_then(|text| text.parse::<usize>().ok()) {
            clock.index = frame.min(last);
            clock.playing = false;
        }
        clock.schedule(cx);
        clock
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn step(&self) -> usize {
        self.step
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Play or pause; pausing goes to the resting frame.
    pub fn set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        self.playing = playing;
        if !playing && let Some(resting) = self.resting {
            self.show_index(resting);
        }
        self.schedule(cx);
        cx.notify();
    }

    /// Jump to frame `index` (a dot under a story); the clock goes on from there if it plays.
    pub fn show(&mut self, index: usize, cx: &mut Context<Self>) {
        self.show_index(index);
        self.schedule(cx);
        cx.notify();
    }

    fn show_index(&mut self, index: usize) {
        let index = index.min(self.holds.len().saturating_sub(1));
        if index != self.index {
            self.index = index;
            self.step += 1;
        }
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        if !self.playing || self.holds.len() < 2 {
            return;
        }
        let hold = self.holds[self.index.min(self.holds.len() - 1)];
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(hold).await;
            let _ = this.update(cx, |clock, cx| {
                let next = (clock.index + 1) % clock.holds.len();
                clock.index = next;
                clock.step += 1;
                clock.schedule(cx);
                cx.notify();
            });
        }));
    }
}

/// The clock of the demo `id`, kept while the demo is drawn.
pub fn story_clock(
    id: impl Into<SharedString>,
    holds: Vec<Duration>,
    resting: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<StoryClock> {
    let id: SharedString = id.into();
    window.use_keyed_state(ElementId::Name(format!("{id}-clock").into()), cx, move |_, cx| {
        StoryClock::new(holds, resting, cx)
    })
}

/// Play and pause for a looping demo (macOS `PlayPauseButton`).
pub fn play_pause(id: impl Into<SharedString>, clock: Entity<StoryClock>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let playing = clock.read(cx).playing();
    let id: SharedString = id.into();
    div()
        .id(SharedString::from(format!("{id}-play")))
        .size(size::H_XS)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(size::R_XS)
        .bg(p.subtle)
        .cursor_pointer()
        .hover(|style| style.bg(p.pressed))
        .on_click(move |_, _, cx| clock.update(cx, |clock, cx| clock.set_playing(!clock.playing(), cx)))
        .child(
            Icon::new(if playing { IconName::Pause } else { IconName::Play })
                .size(size::ICON_XS)
                .text_color(p.text_secondary),
        )
}

// MARK: - Surfaces and keys

/// The fill of a demo's text field: the card colour in both appearances (a stage is raised above the page).
pub fn field_fill(p: &Palette) -> Hsla {
    p.card
}

/// A text field drawn for a demo: a stage, so a card (card colour and a hairline).
pub fn field(cx: &App) -> Div {
    let p = theme::palette(cx);
    div().w_full().rounded(size::R_SM).bg(field_fill(&p)).border_1().border_color(p.separator).overflow_hidden()
}

/// A demo set into a section as a row: boxed, it spans the rows' full width.
pub fn block(content: impl IntoElement) -> Div {
    div().w_full().py(size::ROW_PADDING_Y).child(content)
}

/// Open content (a table, text) set into a section as a row, inset like a row's text.
pub fn text_block(content: impl IntoElement) -> Div {
    div().w_full().px(size::ROW_PADDING_X).py(size::ROW_PADDING_Y).child(content)
}

/// The keys just pressed in a demo: key caps that press in each time `step` changes.
#[derive(IntoElement)]
pub struct DemoKeys {
    id: SharedString,
    keys: Vec<SharedString>,
    step: usize,
}

pub fn demo_keys(id: impl Into<SharedString>, keys: Vec<SharedString>, step: usize) -> DemoKeys {
    DemoKeys { id: id.into(), keys, step }
}

impl RenderOnce for DemoKeys {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        h().flex_shrink_0().gap(space::S4).relative().children(self.keys.into_iter().map(keycap)).with_animation(
            SharedString::from(format!("{}-press-{}", self.id, self.step)),
            Animation::new(motion::FAST).with_easing(ease_out_quint()),
            |this, t| this.top(px(1.5 * (1. - t))).opacity(0.55 + 0.45 * t),
        )
    }
}

// MARK: - The demo line

/// One line of a text document: committed text, the composition with its underlines, and the caret.
#[derive(IntoElement)]
pub struct DemoLine {
    segments: Vec<Segment>,
    caret: Option<usize>,
    font_size: Pixels,
    shows_caret: bool,
    blinks: bool,
}

pub fn demo_line(segments: impl IntoIterator<Item = Segment>) -> DemoLine {
    DemoLine {
        segments: segments.into_iter().collect(),
        caret: None,
        font_size: text::HEADLINE,
        shows_caret: true,
        blinks: true,
    }
}

impl DemoLine {
    /// The caret after segment `after` (None: after everything).
    pub fn caret(mut self, after: Option<usize>) -> Self {
        self.caret = after;
        self
    }

    pub fn font_size(mut self, size: Pixels) -> Self {
        self.font_size = size;
        self
    }

    /// No caret (a field that is not focused).
    pub fn without_caret(mut self) -> Self {
        self.shows_caret = false;
        self
    }

    /// A caret that does not blink.
    pub fn steady_caret(mut self) -> Self {
        self.blinks = false;
        self
    }
}

fn caret(height: Pixels, blinks: bool, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    let bar = div().flex_shrink_0().w(px(1.5)).h(height).mx(px(0.5)).bg(p.text);
    if blinks && !cx.reduce_motion() {
        bar.with_animation("caret", Animation::new(Duration::from_millis(1060)).repeat(), |this, t| {
            this.opacity(if t < 0.5 || t >= 1. { 1. } else { 0. })
        })
        .into_any_element()
    } else {
        bar.into_any_element()
    }
}

fn segment(segment: &Segment, font_size: Pixels, p: &Palette) -> AnyElement {
    // Spaces would collapse; an en space keeps their width as on macOS.
    let label: SharedString = segment.text.replace(' ', "\u{2002}").into();
    match segment.style {
        SegmentStyle::Document => div().text_color(p.text).child(label).into_any_element(),
        SegmentStyle::Faded => div().text_color(p.text_tertiary).child(label).into_any_element(),
        SegmentStyle::Highlight => {
            div().rounded(space::S2).bg(p.selection).text_color(p.text).child(label).into_any_element()
        }
        style => {
            let thick = style != SegmentStyle::Marked;
            div()
                .relative()
                .text_color(p.text)
                .when(style == SegmentStyle::Focus, |this| this.bg(p.selection))
                .child(label)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(if thick { -2.5 } else { -1.5 }) - font_size * 0.02)
                        .h(px(if thick { 2.5 } else { 1. }))
                        .bg(if thick { p.accent } else { p.text.opacity(0.75) }),
                )
                .into_any_element()
        }
    }
}

impl RenderOnce for DemoLine {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let caret_height = self.font_size * 1.25;
        let count = self.segments.len();
        let mut children: Vec<AnyElement> = Vec::new();
        for (index, item) in self.segments.iter().enumerate() {
            // Words of a composition are underlined separately, with a small gap between them.
            let gap = index + 1 < count && item.composed() && self.segments[index + 1].composed();
            children.push(
                div().when(gap, |this| this.mr(px(1.5))).child(segment(item, self.font_size, &p)).into_any_element(),
            );
            if self.shows_caret && self.caret == Some(index) {
                children.push(caret(caret_height, self.blinks, cx));
            }
        }
        if self.shows_caret && self.caret.is_none_or(|after| after >= count) {
            children.push(caret(caret_height, self.blinks, cx));
        }
        h().min_h(caret_height).text_size(self.font_size).whitespace_nowrap().children(children)
    }
}

// MARK: - The mini list

/// A few candidates under a demo line, in the candidate window's style; the first is selected.
#[derive(IntoElement)]
pub struct MiniList {
    items: Vec<SharedString>,
}

pub fn mini_list(items: impl IntoIterator<Item = impl Into<SharedString>>) -> MiniList {
    MiniList { items: items.into_iter().map(Into::into).collect() }
}

impl RenderOnce for MiniList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        v().p(px(4.))
            .min_w(px(92.))
            .rounded(px(9.))
            .bg(p.panel)
            .border_1()
            .border_color(p.panel_border)
            .shadow_md()
            .children(self.items.into_iter().enumerate().map(|(index, item)| {
                let selected = index == 0;
                let (word, reading) = match item.split_once(' ') {
                    Some((word, reading)) => {
                        (SharedString::from(word.to_owned()), Some(SharedString::from(reading.to_owned())))
                    }
                    None => (item.clone(), None),
                };
                h().h(px(20.))
                    .px(px(6.))
                    .gap(px(6.))
                    .rounded(px(5.))
                    .when(selected, |this| this.bg(p.accent))
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if selected { p.on_accent.opacity(0.85) } else { p.text_secondary })
                            .child(SharedString::from((index + 1).to_string())),
                    )
                    .child(div().text_size(px(13.)).text_color(if selected { p.on_accent } else { p.text }).child(word))
                    .when_some(reading, |this, reading| {
                        this.child(
                            div()
                                .text_size(px(10.))
                                .text_color(if selected { p.on_accent.opacity(0.8) } else { p.text_tertiary })
                                .child(reading),
                        )
                    })
            }))
    }
}

// MARK: - The mini demo

/// A demo field tall enough for the line and a small candidate list.
const FIELD_WITH_LIST_H: Pixels = px(112.);

/// A text field that plays its frames in a loop, with the key pressed and a caption for each step (macOS `MiniDemo`).
#[derive(IntoElement)]
pub struct MiniDemo {
    id: SharedString,
    frames: Rc<[DemoFrame]>,
    resting: Option<usize>,
}

/// A demo playing `frames`; `id` must be unique among the demos shown together.
pub fn mini_demo(id: impl Into<SharedString>, frames: Vec<DemoFrame>) -> MiniDemo {
    MiniDemo { id: id.into(), frames: frames.into(), resting: None }
}

impl MiniDemo {
    /// The frame shown while paused (and under reduced motion).
    pub fn resting(mut self, frame: usize) -> Self {
        self.resting = Some(frame);
        self
    }
}

impl RenderOnce for MiniDemo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let holds = self.frames.iter().map(|frame| frame.hold).collect();
        let clock = story_clock(self.id.clone(), holds, self.resting, window, cx);
        let (index, step) = {
            let clock = clock.read(cx);
            (clock.index(), clock.step())
        };
        let frame = self.frames.get(index.min(self.frames.len().saturating_sub(1))).cloned();
        let has_list = self.frames.iter().any(|frame| frame.list.is_some());
        let looping = self.frames.len() > 1;
        let Some(frame) = frame else { return div().into_any_element() };
        v().w_full()
            .gap(space::S8)
            .child(
                field(cx)
                    .h(if has_list { FIELD_WITH_LIST_H } else { size::H_FIELD })
                    .px(space::S12)
                    .py(space::S6)
                    .child(
                        v().gap(space::S4)
                            .child(demo_line(frame.line.clone()).caret(frame.caret))
                            // In a row, so the list keeps its own width as the candidate window does.
                            .when_some(frame.list.clone(), |this, list| this.child(h().child(mini_list(list)))),
                    ),
            )
            .child(
                h().w_full()
                    .gap(space::S8)
                    .when(looping, |this| this.child(play_pause(self.id.clone(), clock.clone(), cx)))
                    .when(!frame.keys.is_empty(), |this| {
                        this.child(demo_keys(self.id.clone(), frame.keys.clone(), step))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(text::SUBTITLE)
                            .text_color(p.text_secondary)
                            .child(frame.caption.clone()),
                    ),
            )
            .into_any_element()
    }
}

// MARK: - Choices, facts, pressed keys

/// One choice of an option with its demo, marked when it is the current setting (macOS `ChoiceDemo`).
#[derive(IntoElement)]
pub struct ChoiceDemo {
    title: SharedString,
    selected: bool,
    demo: MiniDemo,
}

pub fn choice_demo(title: impl Into<SharedString>, selected: bool, demo: MiniDemo) -> ChoiceDemo {
    ChoiceDemo { title: title.into(), selected, demo }
}

impl RenderOnce for ChoiceDemo {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        v().w_full()
            .gap(space::S6)
            .child(
                h().gap(space::S6)
                    .child(
                        div()
                            .text_size(text::SUBTITLE)
                            .font_weight(weight::SEMIBOLD)
                            .text_color(p.text)
                            .child(self.title),
                    )
                    .when(self.selected, |this| {
                        this.child(
                            h().h(size::H_XS)
                                .px(space::S6)
                                .rounded(size::R_XS)
                                .bg(p.subtle)
                                .text_size(text::CAPTION)
                                .text_color(p.text_secondary)
                                .child("目前的設定"),
                        )
                    }),
            )
            .child(self.demo)
    }
}

/// A short fact inside an info card: an icon, then one sentence (macOS `FactRow`).
#[derive(IntoElement)]
pub struct FactRow {
    icon: IconName,
    text: SharedString,
}

pub fn fact_row(icon: IconName, text: impl Into<SharedString>) -> FactRow {
    FactRow { icon, text: text.into() }
}

impl RenderOnce for FactRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        super::content::fact_row(self.icon, self.text, cx)
    }
}

/// Keycaps beside text, as if just pressed: they settle in with a short press, and press again whenever `trigger`
/// changes (macOS `PressedKeycap`).  Under reduced motion they only appear.
#[derive(IntoElement)]
pub struct PressedKeycap {
    id: SharedString,
    keys: Vec<SharedString>,
    note: Option<SharedString>,
    trigger: usize,
}

pub fn pressed_keycap(
    id: impl Into<SharedString>,
    keys: impl IntoIterator<Item = impl Into<SharedString>>,
) -> PressedKeycap {
    PressedKeycap { id: id.into(), keys: keys.into_iter().map(Into::into).collect(), note: None, trigger: 0 }
}

impl PressedKeycap {
    /// Small text after the keys, such as "×3".
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Press again when this changes (a scene of a story).
    pub fn trigger(mut self, trigger: usize) -> Self {
        self.trigger = trigger;
        self
    }
}

impl RenderOnce for PressedKeycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let key = self.keys.join(" ");
        h().relative()
            .flex_shrink_0()
            .gap(space::S4)
            .children(self.keys.into_iter().map(keycap))
            .when_some(self.note, |this, note| {
                this.child(div().text_size(text::CAPTION).text_color(p.text_secondary).child(note))
            })
            .with_animation(
                SharedString::from(format!("{}-{key}-{}", self.id, self.trigger)),
                Animation::new(Duration::from_millis(420)),
                |this, t| {
                    // Fade in over the first 40 %, go down around 45 %, spring back up.
                    let shown = (t / 0.4).min(1.);
                    let press = if t < 0.3 {
                        0.
                    } else if t < 0.45 {
                        (t - 0.3) / 0.15
                    } else {
                        (1. - (t - 0.45) / 0.55).max(0.)
                    };
                    this.opacity(shown).top(px(1.2 * press))
                },
            )
    }
}

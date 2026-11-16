//! The one view of a command's keys, wherever they are shown or set (macOS `ShortcutField`): each binding is its key
//! caps (「⇧」「↓」, `ui::keycap`), alternatives are more bindings further apart.  At rest nothing else shows.  With the
//! pointer on the row: a ghost ＋ to add a key and 還原預設 beside the field when the keys differ from the defaults; on
//! a binding: its ✕.  Click a binding to record a new combination (Esc cancels): it becomes an accent outline with the
//! modifiers held so far and a breathing caret.  An accepted key settles with a small hop, a refused
//! one shakes and the reason slides in below in red.  A key another command also uses is drawn in red with that
//! command's name, 改按鍵 and 前往.  With `max_visible`, more keys than that fold into 「+N」.
//!
//! The rows that hold a field give it their hover group (`Row::hover_group`), so its controls show wherever the pointer
//! is on the row.  State that outlives a frame (the recorder, the last refusal, the accepted chip) lives in a global
//! keyed by command, so the same command's field on two pages agrees.

pub mod keystroke;
pub mod store;

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, FocusHandle, Global, Hsla, InteractiveElement, IntoElement,
    Keystroke, MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Window, div, ease_in_out, ease_out_quint, px, relative,
};
use zt_core::shortcuts::MAX_BINDINGS;

use self::store::KeyStore;
use super::keycap::{binding, key_glyphs};
use super::{h, v};
use crate::app::{self, Place};
use crate::theme::{self, size, space, text};

/// The field's line height: a key cap's.
const CHIP_H: gpui_kit::Pixels = size::H_XS;
/// Between the bindings of one command (the caps of one binding are `S4` apart).
const BINDING_GAP: gpui_kit::Pixels = space::S12;
/// A refusal's message wraps at this width.
const PROBLEM_MAX_W: gpui_kit::Pixels = px(280.);
/// The room 「新增按鍵」 takes in a field without keys.
const EMPTY_W: gpui_kit::Pixels = px(96.);
/// The ✕ that removes a binding.
const REMOVE_BADGE: gpui_kit::Pixels = px(16.);

/// A state to draw without interaction (screenshots), set with [`set_preview`].
#[derive(Clone, Debug, PartialEq)]
pub enum FieldPreview {
    /// The pointer on the row.
    Hover,
    /// The pointer on chip `n`.
    ChipHover(usize),
    /// The 「+N」 opened.
    Expanded,
    /// Recording binding `index`, with these modifiers (Qt flags) held.
    Recording {
        index: usize,
        held: u32,
    },
    Refused(String),
    Accepted(usize),
}

#[derive(Default)]
struct FieldState {
    problem: Option<SharedString>,
    /// Bumped with every refusal: the shake and the message's entrance key on it.
    refusals: usize,
    settled: Option<usize>,
    accepts: usize,
    expanded: bool,
}

struct Recorder {
    command: &'static str,
    index: usize,
    held: u32,
    focus: FocusHandle,
    _intercept: Subscription,
}

/// Every field's state, and the one recorder.
#[derive(Default)]
struct Fields {
    states: HashMap<&'static str, FieldState>,
    recorder: Option<Recorder>,
    previews: HashMap<&'static str, FieldPreview>,
}

impl Global for Fields {}

fn fields(cx: &mut App) -> &mut Fields {
    cx.default_global::<Fields>()
}

/// Draws `command`'s field in a fixed state (UI screenshots).
pub fn set_preview(command: &'static str, preview: FieldPreview, cx: &mut App) {
    fields(cx).previews.insert(command, preview);
}

/// A command's keys, editable.
#[derive(IntoElement)]
pub struct ShortcutField {
    command: &'static str,
    max_visible: Option<usize>,
    disabled: bool,
    group: Option<SharedString>,
    off_link: Option<(SharedString, Place)>,
}

/// The field of `command` (an id of [`store::commands`]).
pub fn shortcut_field(command: &'static str) -> ShortcutField {
    ShortcutField { command, max_visible: None, disabled: false, group: None, off_link: None }
}

impl ShortcutField {
    /// At most this many chips, then 「+N」 (the overview); without it every key shows, wrapping.
    pub fn max_visible(mut self, count: usize) -> Self {
        self.max_visible = Some(count);
        self
    }

    /// Dimmed and not editable (its feature is off or does nothing now).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The hover group of the row the field is in: the ＋ and 還原預設 show while the pointer is anywhere on it.
    pub fn hover_group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// While disabled, a link under the chips to where the feature is turned on (「在「選字」開啟」).
    #[allow(dead_code, reason = "for the feature pages' 快捷鍵 rows, built on other branches")]
    pub fn off_link(mut self, label: impl Into<SharedString>, place: Place) -> Self {
        self.off_link = Some((label.into(), place));
        self
    }
}

/// One binding as key caps: 「⇧」「↓」.  `tint`: a conflict (red) or the accepted accent.
#[derive(IntoElement)]
pub struct KeyChip {
    text: SharedString,
    tint: Option<Hsla>,
}

pub fn key_chip(text: impl Into<SharedString>) -> KeyChip {
    KeyChip { text: text.into(), tint: None }
}

impl KeyChip {
    pub fn tint(mut self, tint: Option<Hsla>) -> Self {
        self.tint = tint;
        self
    }
}

impl RenderOnce for KeyChip {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        binding(&self.text, self.tint)
    }
}

/// A key cap's box for things that are not keys (「+2」, the recorder).
fn cap_box(cx: &App) -> gpui_kit::Div {
    let p = theme::palette(cx);
    h().h(CHIP_H)
        .min_w(CHIP_H)
        .px(space::S6)
        .justify_center()
        .rounded(size::R_XS)
        .flex_shrink_0()
        .bg(p.subtle)
        .text_size(text::CAPTION)
        .whitespace_nowrap()
}

// MARK: - Recording

fn start(command: &'static str, index: usize, window: &mut Window, cx: &mut App) {
    stop(cx);
    let focus = cx.focus_handle();
    window.focus(&focus, cx);
    let intercept = cx.intercept_keystrokes(|event, window, cx| on_keystroke(&event.keystroke, window, cx));
    let fields = fields(cx);
    fields.recorder = Some(Recorder { command, index, held: 0, focus, _intercept: intercept });
    let state = fields.states.entry(command).or_default();
    state.problem = None;
    cx.refresh_windows();
}

fn stop(cx: &mut App) {
    if let Some(recorder) = fields(cx).recorder.take() {
        // Dropped after this event: the interceptor may be the one calling.
        cx.defer(move |_| drop(recorder));
        cx.refresh_windows();
    }
}

fn on_keystroke(keystroke: &Keystroke, _window: &mut Window, cx: &mut App) {
    let Some((command, index)) = fields(cx).recorder.as_ref().map(|r| (r.command, r.index)) else { return };
    cx.stop_propagation();
    if keystroke.is_modifier_key() {
        return;
    }
    let plain = keystroke::qt_modifiers(&keystroke.modifiers) == 0;
    if keystroke.key == "escape" && plain {
        fields(cx).states.entry(command).or_default().problem = None;
        stop(cx);
        return;
    }
    match keystroke::qt_key(keystroke) {
        None => refuse(command, "這個鍵不能用".into(), cx),
        Some((code, modifiers)) => match store::store(cx).record(command, code, modifiers, index) {
            Ok(()) => {
                stop(cx);
                accept(command, index, cx);
            }
            Err(error) => refuse(command, error.to_string(), cx),
        },
    }
    cx.refresh_windows();
}

fn refuse(command: &'static str, message: String, cx: &mut App) {
    let fields = fields(cx);
    if let Some(recorder) = fields.recorder.as_mut() {
        recorder.held = 0;
    }
    let state = fields.states.entry(command).or_default();
    state.problem = Some(message.into());
    state.refusals += 1;
    cx.refresh_windows();
}

fn accept(command: &'static str, index: usize, cx: &mut App) {
    let state = fields(cx).states.entry(command).or_default();
    state.problem = None;
    state.settled = Some(index);
    state.accepts += 1;
    let generation = state.accepts;
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(1000)).await;
        cx.update(|cx| {
            let state = fields(cx).states.entry(command).or_default();
            if state.accepts == generation {
                state.settled = None;
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

fn remove(command: &'static str, index: usize, cx: &mut App) {
    match store::store(cx).remove(command, index) {
        Ok(()) => fields(cx).states.entry(command).or_default().problem = None,
        Err(error) => refuse(command, error.to_string(), cx),
    }
    cx.refresh_windows();
}

fn restore(command: &'static str, cx: &mut App) {
    match store::store(cx).reset(Some(command)) {
        Ok(()) => fields(cx).states.entry(command).or_default().problem = None,
        Err(error) => refuse(command, error.to_string(), cx),
    }
    cx.refresh_windows();
}

// MARK: - Drawing

impl RenderOnce for ShortcutField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let command = self.command;
        let keys: KeyStore = store::store(cx);
        let bindings = keys.bindings(command);
        let conflicts = keys.conflicts_of(command);
        let customised = keys.is_customised(command);
        let preview = fields(cx).previews.get(command).cloned();
        let fields_ref = fields(cx);
        let recorder = fields_ref.recorder.as_ref().filter(|r| r.command == command);
        let (recording, held, focus) = match (&preview, recorder) {
            (Some(FieldPreview::Recording { index, held }), _) => (Some(*index), *held, None),
            (_, Some(r)) => (Some(r.index), r.held, Some(r.focus.clone())),
            _ => (None, 0, None),
        };
        let state = fields_ref.states.entry(command).or_default();
        let problem = match &preview {
            Some(FieldPreview::Refused(message)) => Some(SharedString::from(message.clone())),
            _ => state.problem.clone(),
        };
        let (refusals, accepts) = (state.refusals, state.accepts);
        let settled = match &preview {
            Some(FieldPreview::Accepted(index)) => Some(*index),
            _ => state.settled,
        };
        let expanded = state.expanded || preview == Some(FieldPreview::Expanded);
        let pinned_hover =
            matches!(preview, Some(FieldPreview::Hover | FieldPreview::Expanded | FieldPreview::ChipHover(_)));
        let chip_hover = match preview {
            Some(FieldPreview::ChipHover(index)) => Some(index),
            _ => None,
        };
        let group: SharedString = self.group.clone().unwrap_or_else(|| format!("field-{command}").into());
        let editable = !self.disabled && recording.is_none();
        let shows_all = self.max_visible.is_none() || expanded || recording.is_some();
        let visible =
            if shows_all { bindings.len() } else { bindings.len().min(self.max_visible.unwrap_or(usize::MAX)) };

        // Visible while the pointer is on the row (or pinned for a screenshot).
        let on_hover = move |el: gpui_kit::Stateful<gpui_kit::Div>, group: &SharedString| {
            if pinned_hover {
                el.opacity(1.)
            } else {
                el.opacity(0.).group_hover(group.clone(), |style| style.opacity(1.))
            }
        };

        let mut chips: Vec<AnyElement> = Vec::new();
        if !bindings.is_empty() && bindings.len() < MAX_BINDINGS && !self.disabled {
            let count = bindings.len();
            let ghost = ghost_add(format!("ghost-{command}"), None, cx)
                .tooltip(|window, cx| Tooltip::new("新增按鍵").build(window, cx))
                .when(editable, |this| this.on_click(move |_, window, cx| start(command, count, window, cx)));
            chips.push(if editable {
                on_hover(ghost, &group).into_any_element()
            } else {
                ghost.opacity(0.).into_any_element()
            });
        }
        if bindings.is_empty() && recording.is_none() {
            let ghost = ghost_add(format!("empty-{command}"), Some("新增按鍵"), cx)
                .absolute()
                .right_0()
                .top_0()
                .when(editable, |this| this.on_click(move |_, window, cx| start(command, 0, window, cx)));
            let dash = div()
                .id(SharedString::from(format!("dash-{command}")))
                .h(CHIP_H)
                .flex()
                .items_center()
                .text_color(p.text_tertiary)
                .child("—");
            let dash = if pinned_hover || !editable {
                dash.when(pinned_hover, |this| this.opacity(0.)).into_any_element()
            } else {
                dash.group_hover(group.clone(), |style| style.opacity(0.)).into_any_element()
            };
            chips.push(
                div()
                    .relative()
                    .min_w(EMPTY_W)
                    .h(CHIP_H)
                    .flex()
                    .justify_end()
                    .child(dash)
                    .when(editable, |this| this.child(on_hover(ghost, &group)))
                    .into_any_element(),
            );
        }
        for (index, binding) in bindings.iter().take(visible).enumerate() {
            if recording == Some(index) {
                chips.push(recorder_chip(command, held, focus.clone(), cx));
                continue;
            }
            let conflict = conflicts.contains_key(&index);
            let accepted = settled == Some(index);
            chips.push(binding_chip(
                command,
                index,
                binding.label.clone(),
                binding.spoken.clone(),
                conflict,
                accepted,
                accepts,
                editable,
                chip_hover == Some(index),
                cx,
            ));
        }
        if visible < bindings.len() {
            let more = bindings.len() - visible;
            chips.push(
                cap_box(cx)
                    .id(SharedString::from(format!("more-{command}")))
                    .text_color(p.text_secondary)
                    .cursor_pointer()
                    .tooltip(move |window, cx| Tooltip::new(format!("還有 {more} 個按鍵")).build(window, cx))
                    .on_hover(move |hovered, _, cx| {
                        if *hovered {
                            fields(cx).states.entry(command).or_default().expanded = true;
                            cx.refresh_windows();
                        }
                    })
                    .on_click(move |_, _, cx| {
                        fields(cx).states.entry(command).or_default().expanded = true;
                        cx.refresh_windows();
                    })
                    .child(format!("+{more}"))
                    .into_any_element(),
            );
        }
        if let Some(index) = recording
            && index >= bindings.len()
        {
            chips.push(recorder_chip(command, held, focus, cx));
        }

        let restore_link = (customised && !self.disabled).then(|| {
            let link = div()
                .id(SharedString::from(format!("restore-{command}")))
                .absolute()
                .right(relative(1.))
                .top_0()
                .h(CHIP_H)
                .pr(space::S12)
                .flex()
                .items_center()
                .whitespace_nowrap()
                .text_size(text::SUBTITLE)
                .text_color(p.accent)
                .cursor_pointer()
                .tooltip(move |window, cx| {
                    Tooltip::new(format!("還原成 {}", store::defaults_spoken(command))).build(window, cx)
                })
                .on_click(move |_, _, cx| restore(command, cx))
                .child("還原預設");
            if recording.is_some() {
                link.opacity(0.).into_any_element()
            } else {
                on_hover(link, &group).into_any_element()
            }
        });

        let row = h().relative().flex_wrap().justify_end().gap(BINDING_GAP).children(restore_link).children(chips);
        let row = if refusals > 0 {
            row.with_animation(
                SharedString::from(format!("shake-{command}-{refusals}")),
                Animation::new(Duration::from_millis(250)),
                |this, t| {
                    // -6, 5, -3, 2, 0 over five steps, as macOS.
                    let steps = [0., -6., 5., -3., 2., 0.];
                    let position = t * 5.;
                    let i = (position.floor() as usize).min(4);
                    let f = position - i as f32;
                    this.left(px(steps[i] + (steps[i + 1] - steps[i]) * f))
                },
            )
            .into_any_element()
        } else {
            row.into_any_element()
        };

        let notes = (recording.is_none()).then(|| {
            conflicts
                .iter()
                .map(|(&index, &other)| conflict_line(command, index, other, cx).into_any_element())
                .collect::<Vec<_>>()
        });

        v().id(SharedString::from(format!("field-{command}")))
            .when(self.group.is_none(), |this| this.group(group.clone()))
            .items_end()
            .gap(space::S4)
            .when(self.disabled, |this| this.opacity(0.45))
            .on_hover(move |hovered, _, cx| {
                if !hovered {
                    let state = fields(cx).states.entry(command).or_default();
                    if state.expanded {
                        state.expanded = false;
                        cx.refresh_windows();
                    }
                }
            })
            .child(row)
            .children(notes.into_iter().flatten())
            .when_some(problem, |this, message| {
                this.child(
                    div()
                        .max_w(PROBLEM_MAX_W)
                        .text_size(text::SUBTITLE)
                        .text_color(p.danger)
                        .text_right()
                        .child(message)
                        .with_animation(
                            SharedString::from(format!("problem-{command}-{refusals}")),
                            Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
                            |this, t| this.opacity(t).mt(px(-6. * (1. - t))),
                        ),
                )
            })
            .when_some(self.off_link.filter(|_| self.disabled), |this, (label, place)| {
                this.child(super::text_link(
                    SharedString::from(format!("off-{command}")),
                    label,
                    move |_, _, cx| app::go(place.clone(), cx),
                    cx,
                ))
            })
    }
}

/// A ghost ＋ (and `label`) that adds a key: no border, a hover fill.
fn ghost_add(id: String, label: Option<&'static str>, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = theme::palette(cx);
    h().id(SharedString::from(id))
        .flex_shrink_0()
        .h(CHIP_H)
        .min_w(CHIP_H)
        .px(if label.is_some() { space::S6 } else { space::S4 })
        .gap(space::S4)
        .justify_center()
        .rounded(size::R_XS)
        .text_size(text::SUBTITLE)
        .text_color(p.text_secondary)
        .cursor_pointer()
        .hover(|style| style.bg(p.hover).text_color(p.text))
        .child(Icon::new(IconName::Plus).size(size::ICON_XS))
        .children(label)
}

#[allow(clippy::too_many_arguments)]
fn binding_chip(
    command: &'static str,
    index: usize,
    label: String,
    spoken: String,
    conflict: bool,
    accepted: bool,
    accepts: usize,
    editable: bool,
    pinned_remove: bool,
    cx: &App,
) -> AnyElement {
    let p = theme::palette(cx);
    let group = SharedString::from(format!("chip-{command}-{index}"));
    let tint = if conflict {
        Some(p.danger)
    } else if accepted {
        Some(p.accent)
    } else {
        None
    };
    let help = if conflict {
        format!("{spoken}：也用於另一個功能；點一下改按鍵")
    } else {
        format!("{spoken}：點一下改按鍵")
    };
    let remove_spoken = spoken.clone();
    // A small badge on the binding's corner, so showing it moves nothing.
    let cross = div()
        .id(SharedString::from(format!("remove-{command}-{index}")))
        .absolute()
        .top(-space::S6)
        .right(-space::S6)
        .size(REMOVE_BADGE)
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(p.card)
        .border_1()
        .border_color(p.control_border)
        .text_color(p.text_secondary)
        .hover(|style| style.text_color(p.text))
        .cursor_pointer()
        .tooltip(move |window, cx| Tooltip::new(format!("刪除 {remove_spoken}")).build(window, cx))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            remove(command, index, cx);
        })
        .child(Icon::new(IconName::X).size(space::S8 + space::S2));
    let cross = if pinned_remove {
        cross.opacity(1.)
    } else {
        cross.opacity(0.).group_hover(group.clone(), |style| style.opacity(1.))
    };
    let chip = div()
        .id(SharedString::from(format!("chip-{command}-{index}")))
        .group(group)
        .relative()
        .rounded(size::R_XS)
        .when(editable, |this| this.cursor_pointer().on_click(move |_, window, cx| start(command, index, window, cx)))
        .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
        .child(key_chip(label).tint(tint))
        .when(editable, |this| this.child(cross));
    if accepted {
        chip.with_animation(
            SharedString::from(format!("accept-{command}-{accepts}")),
            Animation::new(Duration::from_millis(320)).with_easing(ease_in_out),
            |this, t| this.top(px(-3. * (t * std::f32::consts::PI).sin())),
        )
        .into_any_element()
    } else {
        chip.into_any_element()
    }
}

fn recorder_chip(command: &'static str, held: u32, focus: Option<FocusHandle>, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    let caret = div().w(px(1.5)).h(size::ICON_XS).bg(p.accent).with_animation(
        "recorder-caret",
        Animation::new(Duration::from_millis(1100)).repeat(),
        |this, t| this.opacity(1. - 0.75 * (t * std::f32::consts::PI).sin()),
    );
    cap_box(cx)
        .id(SharedString::from(format!("recorder-{command}")))
        .min_w(space::S32)
        .gap(space::S2)
        .bg(p.control)
        .border_1()
        .border_color(p.accent)
        .text_color(p.text)
        .tooltip(|window, cx| Tooltip::new("按下新的組合；Esc 取消").build(window, cx))
        .when_some(focus, |this, focus| {
            this.track_focus(&focus)
                .on_modifiers_changed(move |event, _, cx| {
                    if let Some(recorder) = fields(cx).recorder.as_mut() {
                        recorder.held = keystroke::qt_modifiers(&event.modifiers);
                        cx.refresh_windows();
                    }
                })
                .on_mouse_down_out(|_, _, cx| stop(cx))
        })
        .child(key_glyphs(&store::modifier_label(held), f32::from(text::CAPTION), p.text))
        .child(caret)
        .into_any_element()
}

fn conflict_line(command: &'static str, index: usize, other: &'static str, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let link = |id: String, label: &'static str| {
        div()
            .id(SharedString::from(id))
            .text_color(p.accent)
            .cursor_pointer()
            .hover(|style| style.opacity(0.8))
            .child(label)
    };
    let other_label = store::command_label(other);
    h().id(SharedString::from(format!("conflict-{command}-{index}")))
        .gap(space::S6)
        .text_size(text::SUBTITLE)
        .text_color(p.danger)
        .tooltip(move |window, cx| {
            Tooltip::new(format!("「{other_label}」也用這個按鍵；兩個功能都按下時只有一個會作用")).build(window, cx)
        })
        .child(Icon::new(IconName::TriangleAlert).size(size::ICON_XS))
        .child(div().whitespace_nowrap().child(format!("也用於「{other_label}」")))
        .child(
            h().pl(space::S4)
                .gap(space::S12)
                .child(link(format!("replace-{command}-{index}"), "改按鍵").on_click(move |_, window, cx| {
                    start(command, index, window, cx);
                }))
                .child(link(format!("goto-{command}-{index}"), "前往").on_click(move |_, _, cx| {
                    app::go(store::command_place(other), cx);
                })),
        )
}

/// The chips' height, for rows that line other content up with them.
pub fn chip_height() -> gpui_kit::Pixels {
    CHIP_H
}

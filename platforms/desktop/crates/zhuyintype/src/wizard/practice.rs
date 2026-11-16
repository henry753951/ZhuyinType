//! The wizard's practice field (macOS `PracticeField`, `PracticeFieldView`): the task, a one-line text box that types
//! with ZhuyinType's own controller (`zt_core::practice::PracticeSession`, a sandbox on the wizard's draft settings),
//! and under it, inside the wizard window, the real candidate window (`candidate::panel`) and the 中/英 badge, placed
//! as the floating windows are placed beside a caret.  Room for a full page of the window is kept under the box (over
//! it when the window opens above the caret), so the page does not move while typing.
//!
//! Keys reach the field while it has focus: GPUI key events go through `ui::shortcuts::keystroke` to the core's
//! `Key`; Shift and Caps Lock arrive as modifier changes (X11 sends no key event for a modifier alone).  What the core
//! leaves to the app is typed into the field as a single-line text field takes it.  Nothing reaches the user's
//! settings, words or statistics.
//!
//! Typing stays cheap: a key changes the session and bumps a generation; the window's layout is recomputed only when
//! the generation, the draft's revision or the appearance changed, and the presentation is read by reference.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Bounds, Context, EventEmitter, FocusHandle, Font, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, ModifiersChangedEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Render, SharedString,
    Styled, TextRun, Window, black, canvas, div, px,
};
use zt_core::keys::{Key, code, modifier};
use zt_core::practice::{PracticeEvent, PracticeSession};
use zt_core::presentation::Presentation;
use zt_core::service::PopupAction;
use zt_core::settings::Settings;

use super::data::{Check, Task};
use crate::candidate::layout::{BADGE_H, Extras, WindowMeasure};
use crate::candidate::panel::{ActionHandler, CandidatePanel, ease_out, mode_badge};
use crate::candidate::sample;
use crate::candidate::scene::panel_layout;
use crate::candidate::style::{CurrentKeys, Layout, PanelAppearance, PanelStyle, selection_keys};
use crate::theme::{self, size, space, text, weight};
use crate::ui::demo::field_fill;
use crate::ui::shortcuts::keystroke;
use crate::ui::{h, v};

/// The field's text size, the box's height and the room around the line (macOS `PracticeFieldView`).
const FONT: f32 = 18.;
const BOX_H: f32 = 46.;
const LINE_H: f32 = 24.;
const PADDING: f32 = 14.;
/// The box's radius: the inputs' (`size::R_SM`).
const RADIUS: Pixels = size::R_SM;
/// Between the task and the box.
const TASK_GAP: Pixels = space::S12;
/// Between the line and the window (macOS `InputPanelPlacement.gap`).
const GAP: f32 = 4.;
/// The window's entrance: it starts this much further from the caret (macOS `PanelMotion.slide`, `.appear`).
const SLIDE: f32 = 4.;
const APPEAR_MS: f32 = 120.;
/// The window may reach this far past the box's sides: it stays inside the stage the field sits on (the wizard's
/// `S16` padding), `S8` from its edge.
const STAGE_MARGIN: f32 = 8.;
/// The window may reach this close to the wizard window's sides.
const EDGE: f32 = 12.;
/// The 中/英 badge: shown, then faded (macOS `ModeToast`).
const BADGE_SHOWN: f32 = 0.8;
const BADGE_FADE: f32 = 0.2;
/// 「完成」: the outline turns green over this long.
const DONE_MS: f32 = 200.;

gpui_kit::actions!(practice_field, [PracticeTab, PracticeBackTab]);

/// The key context of the field: Tab and ⇧Tab are the input method's here (翻頁), not the window's focus keys.
const CONTEXT: &str = "PracticeField";

/// Takes Tab and ⇧Tab from the window's focus navigation while the field has the keyboard.  Call once.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("tab", PracticeTab, Some(CONTEXT)),
        gpui_kit::KeyBinding::new("shift-tab", PracticeBackTab, Some(CONTEXT)),
    ]);
}

/// What the field tells the wizard.
pub enum FieldEvent {
    /// The task's goals were all met (once).
    Completed,
    /// User phrases added while practising, (word, reading), all of them so far.
    Phrases(Vec<(String, String)>),
}

/// Where the field's box is in the window, measured while painting.
#[derive(Clone, Copy, Default)]
struct Placed {
    box_bounds: Option<Bounds<Pixels>>,
}

/// The draft's look, computed once per revision: the window's style and the room a full page needs.
struct Look {
    key: (u64, bool),
    style: PanelStyle,
    reserve: f32,
    above: bool,
    animates: bool,
    toast: bool,
}

pub struct PracticeField {
    session: Option<PracticeSession>,
    /// The draft settings (the session's own copy when there is a session).
    settings: Settings,
    error: Option<String>,
    pub focus: FocusHandle,
    font: Font,
    task: Option<Task>,
    checks: Vec<Check>,
    done: Option<Instant>,
    reported_phrases: usize,
    panel: CandidatePanel,
    /// What the panel was laid out for: (generation, draft revision, dark, window width).
    panel_key: Option<(u64, u64, bool, i32)>,
    /// When the window appeared (it is showing).
    panel_since: Option<Instant>,
    generation: u64,
    badge: Option<(bool, Instant)>,
    shift: bool,
    capslock: Option<bool>,
    scroll: f32,
    look: Option<Look>,
    placed: Rc<Cell<Placed>>,
    /// Screenshots: no blinking, no entrances.
    still: bool,
}

impl EventEmitter<FieldEvent> for PracticeField {}

/// The text of `text` before UTF-16 offset `units`.
fn prefix_utf16(text: &str, units: usize) -> &str {
    let mut count = 0;
    for (index, c) in text.char_indices() {
        if count >= units {
            return &text[..index];
        }
        count += c.len_utf16();
    }
    text
}

/// The byte range of UTF-16 range `start..end` in `text`.
fn byte_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    (prefix_utf16(text, start).len(), prefix_utf16(text, end).len())
}

impl PracticeField {
    /// A field on a copy of `original` (the user's settings), with the service's practice session when there is one
    /// (`Service::practice`).
    pub fn new(
        session: Option<anyhow::Result<PracticeSession>>,
        original: &Settings,
        still: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let (session, error) = match session {
            Some(Ok(session)) => (Some(session), None),
            Some(Err(error)) => (None, Some(error.to_string())),
            None => (None, Some("無法載入詞庫".to_owned())),
        };
        let settings = session.as_ref().map_or_else(|| original.sandbox(), |s| s.settings().clone());
        let mut panel = CandidatePanel::new();
        panel.hover = true;
        Self {
            session,
            settings,
            error,
            focus: cx.focus_handle(),
            font: theme::font(),
            task: None,
            checks: Vec::new(),
            done: None,
            reported_phrases: 0,
            panel,
            panel_key: None,
            panel_since: None,
            generation: 0,
            badge: None,
            shift: false,
            capslock: None,
            scroll: 0.,
            look: None,
            placed: Rc::new(Cell::new(Placed::default())),
            still,
        }
    }

    /// The draft settings the wizard's choices change (in effect in the field at once).
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Starts `task` (or free typing): the field starts over with the task's text.
    pub fn start(&mut self, task: Option<Task>, cx: &mut Context<Self>) {
        self.task = task;
        self.checks = task.map(|t| t.goals.iter().map(|goal| goal.check()).collect()).unwrap_or_default();
        self.done = None;
        self.badge = None;
        self.scroll = 0.;
        if let Some(session) = &mut self.session {
            session.reset(task.map_or("", |t| t.initial));
            session.take_events();
            self.reported_phrases = session.outcome().phrases.len();
        }
        self.panel.reset();
        self.panel_since = None;
        self.generation += 1;
        cx.notify();
    }

    /// The draft changed: the field follows at once, even in the middle of a composition (an open list opens
    /// again with the new settings).
    pub fn settings_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            session.controller_mut().refresh_settings();
        }
        self.changed(cx);
    }

    /// The wizard moved away: the composition is sent, as when an app loses focus.
    pub fn end_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            session.end_session();
        }
        self.changed(cx);
    }

    /// Takes the keyboard.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if self.capslock.is_none() {
            let on = window.capslock().on;
            self.capslock = Some(on);
            if let Some(session) = &mut self.session {
                session.controller_mut().synchronize_caps_lock(on);
                session.take_events();
            }
        }
    }

    /// After a key, a click or a change: what the field shows next, and whether the task is done now.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let Some(session) = &mut self.session else {
            cx.notify();
            return;
        };
        for event in session.take_events() {
            match event {
                PracticeEvent::ModeToast { english } => {
                    if session.settings().bool("showModeToast") {
                        self.badge = Some((english, Instant::now()));
                    }
                }
                PracticeEvent::Boundary(direction) => self.panel.bounce(direction),
            }
        }
        let outcome = session.outcome();
        if self.done.is_none() && !self.checks.is_empty() && self.checks.iter().all(|check| check.is_met(outcome)) {
            self.done = Some(Instant::now());
            cx.emit(FieldEvent::Completed);
        }
        if outcome.phrases.len() > self.reported_phrases {
            self.reported_phrases = outcome.phrases.len();
            cx.emit(FieldEvent::Phrases(session.added_phrases()));
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if event.keystroke.is_modifier_key() {
            return;
        }
        let Some(session) = &mut self.session else { return };
        let Some(key) = keystroke::core_key(&event.keystroke, event.is_held) else { return };
        if session.key_down(&key) {
            cx.stop_propagation();
        }
        self.changed(cx);
    }

    /// Tab and ⇧Tab, which arrive as actions (the window binds them to focus navigation).
    fn tab(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else { return };
        let key = Key::new(code::TAB, if back { modifier::SHIFT } else { 0 });
        session.key_down(&key);
        session.key_up(&key);
        self.changed(cx);
    }

    fn key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if event.keystroke.is_modifier_key() {
            return;
        }
        let Some(session) = &mut self.session else { return };
        if let Some(key) = keystroke::core_key(&event.keystroke, false) {
            session.key_up(&key);
            self.changed(cx);
        }
    }

    /// Shift and Caps Lock: the platforms report them as modifier changes.
    fn modifiers_changed(&mut self, event: &ModifiersChangedEvent, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else { return };
        let modifiers = keystroke::qt_modifiers(&event.modifiers);
        if event.modifiers.shift != self.shift {
            self.shift = event.modifiers.shift;
            let key = Key::new(code::SHIFT, modifiers);
            if self.shift {
                session.key_down(&key);
            } else {
                session.key_up(&key);
            }
        }
        let on = event.capslock.on;
        match self.capslock {
            Some(known) if known != on => {
                session.key_down(&Key::new(code::CAPS_LOCK, modifiers));
                session.key_up(&Key::new(code::CAPS_LOCK, modifiers));
            }
            None => session.controller_mut().synchronize_caps_lock(on),
            _ => {}
        }
        self.capslock = Some(on);
        self.changed(cx);
    }

    fn popup_action(&mut self, action: PopupAction, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            session.popup_action(action);
            self.changed(cx);
        }
    }

    /// A click in the box: the composition is sent, then the insertion point goes to the nearest character boundary.
    fn click(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(bounds) = self.placed.get().box_bounds else { return };
        let target = f32::from(event.position.x - bounds.origin.x) - PADDING + self.scroll;
        let Some(session) = &mut self.session else { return };
        session.end_session();
        let text = session.text().to_owned();
        let mut best = (0, target.abs());
        let mut units = 0;
        for (index, c) in text.char_indices() {
            units += c.len_utf16();
            let width = measure(window, &self.font, &text[..index + c.len_utf8()]);
            let distance = (width - target).abs();
            if distance < best.1 {
                best = (units, distance);
            }
        }
        session.move_insertion(best.0);
        self.changed(cx);
    }

    /// Types `script` (screenshots): characters as on a US keyboard, `~d` ↓, `~u` ↑, `~D` ⇧↓, `~U` ⇧↑, `~t` Tab,
    /// `~r` Return, `~l` ←, `~L` ⇧←, `~R` ⇧→, `~s` a Shift tap.
    pub fn feed(&mut self, script: &str, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else { return };
        let mut chars = script.chars();
        while let Some(c) = chars.next() {
            let key = if c == '~' {
                let (code, modifiers) = match chars.next() {
                    Some('d') => (code::DOWN, 0),
                    Some('u') => (code::UP, 0),
                    Some('D') => (code::DOWN, modifier::SHIFT),
                    Some('U') => (code::UP, modifier::SHIFT),
                    Some('t') => (code::TAB, 0),
                    Some('r') => (code::RETURN, 0),
                    Some('l') => (code::LEFT, 0),
                    Some('L') => (code::LEFT, modifier::SHIFT),
                    Some('R') => (code::RIGHT, modifier::SHIFT),
                    Some('s') => {
                        let shift = Key::new(code::SHIFT, modifier::SHIFT);
                        session.key_down(&shift);
                        session.key_up(&Key::new(code::SHIFT, 0));
                        continue;
                    }
                    _ => continue,
                };
                Key::new(code, modifiers)
            } else {
                match Key::typing(c) {
                    Some(key) => key,
                    None => continue,
                }
            };
            session.key_down(&key);
            session.key_up(&key);
        }
        self.changed(cx);
    }

    /// The draft's look for the window, cached per revision and appearance.
    fn look(&mut self, dark: bool, window: &Window) -> &Look {
        let key = (self.settings.revision(), dark);
        if self.look.as_ref().is_none_or(|look| look.key != key) {
            let keys = CurrentKeys::sandboxed(&self.settings);
            let style = PanelStyle::from_settings(&self.settings, &keys);
            let selection = selection_keys(&self.settings);
            let sample =
                sample::list(sample::PANEL_WORDS, &[], false, selection.len().max(1), 0).presentation("", &selection);
            let height = panel_layout(&sample, &style, &mut WindowMeasure(window)).height;
            self.look = Some(Look {
                key,
                reserve: height.max(BADGE_H) + GAP,
                above: self.settings.string("candidatePlacement") == "above",
                animates: self.settings.bool("panelAnimation"),
                toast: self.settings.bool("showModeToast"),
                style,
            });
        }
        self.look.as_ref().expect("the look was just computed")
    }

    /// The field's whole height (the box and the room for the window).
    fn area_height(&mut self, dark: bool, window: &Window) -> f32 {
        let reserve = self.look(dark, window).reserve;
        BOX_H + reserve - (BOX_H - LINE_H) / 2. + SLIDE
    }
}

/// The width of `text` in the field's font.
fn measure(window: &Window, font: &Font, text: &str) -> f32 {
    if text.is_empty() {
        return 0.;
    }
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(window.text_system().layout_line(text, px(FONT), &[run], None).width)
}

/// A clause of the composition under the line: its x, width and how it is marked.
struct Mark {
    x: f32,
    w: f32,
    style: MarkStyle,
}

#[derive(Clone, Copy, PartialEq)]
enum MarkStyle {
    /// A word of the composition: a thin underline.
    Clause,
    /// What the list changes: a faint wash and a thick underline.
    Focus,
    /// What ↓ would change: a thick underline.
    Target,
}

/// The composition's clauses as marks, `start` being the composition's x.
fn marks(p: &Presentation, start: f32, window: &Window, font: &Font) -> Vec<Mark> {
    let composition = &p.composition;
    let mut result = Vec::with_capacity(p.clauses.len().max(1));
    if p.clauses.is_empty() {
        result.push(Mark { x: start, w: measure(window, font, composition), style: MarkStyle::Clause });
        return result;
    }
    for (index, range) in p.clauses.iter().enumerate() {
        let (from, to) = byte_range(composition, range.start, range.end);
        if from >= to {
            continue;
        }
        let x = start + measure(window, font, &composition[..from]);
        let w = measure(window, font, &composition[..to]) - (x - start);
        let style = if p.focused_clause == Some(index) {
            MarkStyle::Focus
        } else if p.target_clause == Some(index) {
            MarkStyle::Target
        } else {
            MarkStyle::Clause
        };
        result.push(Mark { x, w, style });
    }
    result
}

fn mix(a: gpui_kit::Hsla, b: gpui_kit::Hsla, t: f32) -> gpui_kit::Hsla {
    let t = t.clamp(0., 1.);
    let ra = a.to_rgb();
    let rb = b.to_rgb();
    gpui_kit::Rgba {
        r: ra.r + (rb.r - ra.r) * t,
        g: ra.g + (rb.g - ra.g) * t,
        b: ra.b + (rb.b - ra.b) * t,
        a: ra.a + (rb.a - ra.a) * t,
    }
    .into()
}

impl PracticeField {
    /// The task line: the prompt, 「完成」 once done, and the hint.
    fn task_line(&self, window: &mut Window, cx: &App) -> Option<AnyElement> {
        let task = self.task?;
        let p = theme::palette(cx);
        let done = self.done.map(|at| {
            let t = if self.still || cx.reduce_motion() { 1. } else { (at.elapsed().as_secs_f32() / 0.3).min(1.) };
            if t < 1. {
                window.request_animation_frame();
            }
            // A spring: grows from 60 % with a little overshoot (only a fade under Reduce Motion).
            let scale = if cx.reduce_motion() { 1. } else { 0.6 + 0.4 * back_out(t) };
            h().gap(space::S4)
                .flex_shrink_0()
                .opacity(t.min(1.))
                .text_size(px(f32::from(text::HEADLINE) * scale))
                .font_weight(weight::SEMIBOLD)
                .text_color(p.success)
                .child(Icon::new(IconName::CircleCheck).size(px(f32::from(size::ICON_MD) * scale)))
                .child("完成")
        });
        Some(
            v().gap(space::S4)
                .child(
                    h().gap(space::S8)
                        .child(Icon::new(IconName::Keyboard).size(size::ICON_MD).text_color(p.text_secondary))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(text::HEADLINE)
                                .font_weight(weight::SEMIBOLD)
                                .text_color(p.text)
                                .child(task.prompt),
                        )
                        .children(done),
                )
                .when_some(task.hint, |this, hint| {
                    this.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(hint))
                })
                .into_any_element(),
        )
    }
}

/// Ease-out with a small overshoot (a soft spring).
fn back_out(t: f32) -> f32 {
    let c = 1.4;
    let t = t - 1.;
    1. + (c + 1.) * t * t * t + c * t * t
}

impl Render for PracticeField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let reduced = cx.reduce_motion();
        let area_h = self.area_height(p.dark, window);
        let (above, animates, toast_on, style) = {
            let look = self.look(p.dark, window);
            (look.above, look.animates, look.toast, look.style.clone())
        };
        let focused = self.focus.is_focused(window);
        let viewport_w = f32::from(window.viewport_size().width);
        let box_top = if above { area_h - BOX_H } else { 0. };
        let line_top = (BOX_H - LINE_H) / 2.;
        let placed = self.placed.get();
        let field_x = placed.box_bounds.map_or(0., |b| f32::from(b.origin.x));
        let box_w = placed.box_bounds.map_or(400., |b| f32::from(b.size.width));
        let room = box_w - PADDING * 2.;
        let generation = self.generation;
        let revision = self.settings.revision();
        let panel_palette = PanelAppearance::from_settings(&self.settings).palette(p.dark);
        let font = self.font.clone();

        // The line: the sent text with the composition at the insertion point, its marks and the caret.
        let mut line_text = String::new();
        let mut marks_list = Vec::new();
        let mut caret_x = 0.;
        let mut composition_x = 0.;
        let mut shows_panel = false;
        let mut panel_size = (0., 0.);
        let mut text_inset = 0.;
        if let Some(session) = &mut self.session {
            let insertion = session.insertion();
            let (before_len, before_w) = {
                let sent = session.text();
                let before = prefix_utf16(sent, insertion);
                line_text.push_str(sent);
                (before.len(), measure(window, &font, before))
            };
            let presentation = session.controller_mut().presentation();
            let composition = presentation.composition.as_str();
            line_text.insert_str(before_len, composition);
            composition_x = before_w;
            caret_x = if composition.is_empty() {
                before_w
            } else {
                marks_list = marks(presentation, before_w, window, &font);
                before_w + measure(window, &font, prefix_utf16(composition, presentation.cursor))
            };
            // Scroll so the caret stays in the box.
            if caret_x - self.scroll > room {
                self.scroll = caret_x - room;
            }
            if caret_x < self.scroll {
                self.scroll = (caret_x - room / 3.).max(0.);
            }
            if measure(window, &font, &line_text) <= room {
                self.scroll = 0.;
            }
            // The candidate window, laid out again only after a change.
            shows_panel = presentation.has_list() || presentation.notice.is_some();
            let key = (generation, revision, p.dark, viewport_w.round() as i32);
            if shows_panel && self.panel_key != Some(key) {
                let max_width = (viewport_w - EDGE * 2.).max(200.);
                let mut style = style.clone();
                let mut measure_window = WindowMeasure(window);
                self.panel.animates = animates && !reduced;
                let visible = self.panel_since.is_some();
                // A horizontal page too wide for the window drops the 單字／修正 buttons (a list switched on keeps
                // its button) before its candidates are squeezed.
                if style.layout == Layout::Horizontal
                    && style.shows_list_switches
                    && panel_layout(presentation, &style, &mut measure_window).width > max_width
                {
                    style.shows_list_switches = false;
                }
                self.panel.update(presentation, Extras::default(), &style, max_width, visible, &mut measure_window);
                self.panel_key = Some(key);
            }
            if shows_panel {
                if self.panel_since.is_none() {
                    self.panel_since = Some(Instant::now());
                }
                if let Some(layout) = self.panel.layout() {
                    panel_size = (layout.width, layout.height);
                    text_inset = layout.text_inset;
                }
            } else if self.panel_since.is_some() {
                self.panel_since = None;
                self.panel_key = None;
                self.panel.reset();
            }
        }

        let scroll = self.scroll;
        let empty = line_text.is_empty();
        let placeholder: SharedString = match (&self.session, &self.error) {
            (Some(_), _) => "在這裡試打".into(),
            (None, Some(error)) => error.clone().into(),
            (None, None) => "正在準備…".into(),
        };
        let done_t = self.done.map_or(0., |at| {
            if self.still {
                1.
            } else {
                let t = at.elapsed().as_secs_f32() * 1000. / DONE_MS;
                if t < 1. {
                    window.request_animation_frame();
                }
                ease_out(t)
            }
        });
        let resting = if focused { p.accent.opacity(0.6) } else { p.separator };
        let border = if self.done.is_some() { mix(resting, p.success, done_t) } else { resting };
        let border_w = if self.done.is_some() {
            2.
        } else if focused {
            1.5
        } else {
            1.
        };

        let caret = (focused && self.session.is_some()).then(|| {
            let bar = div()
                .absolute()
                .left(px(PADDING + caret_x - scroll))
                .top(px(line_top + 1.))
                .w(px(1.5))
                .h(px(LINE_H - 2.))
                .bg(p.text);
            if self.still || reduced {
                bar.into_any_element()
            } else {
                use gpui_kit::{Animation, AnimationExt as _};
                bar.with_animation(
                    SharedString::from(format!("practice-caret-{generation}")),
                    Animation::new(Duration::from_millis(1060)).repeat(),
                    |this, t| this.opacity(if t < 0.5 { 1. } else { 0. }),
                )
                .into_any_element()
            }
        });
        let mark_elements = marks_list.iter().map(|mark| {
            let thick = mark.style != MarkStyle::Clause;
            div()
                .absolute()
                .left(px(PADDING + mark.x - scroll))
                .top(px(line_top))
                .w(px(mark.w.max(1.)))
                .h(px(LINE_H + 1.))
                .when(mark.style == MarkStyle::Focus, |this| this.bg(p.selection))
                // Neutral like the rest of the composition: the thick line says which clause, not a colour.
                .child(div().absolute().left_0().bottom_0().w_full().h(px(if thick { 2. } else { 1. })).bg(if thick {
                    p.text
                } else {
                    p.text_tertiary
                }))
                .into_any_element()
        });
        let placed_cell = self.placed.clone();
        let probe = canvas(
            move |bounds, _, _| {
                let mut placed = placed_cell.get();
                placed.box_bounds = Some(bounds);
                placed_cell.set(placed);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let the_box = div()
            .id("practice-box")
            .absolute()
            .left_0()
            .top(px(box_top))
            .w_full()
            .h(px(BOX_H))
            .rounded(RADIUS)
            .bg(field_fill(&p))
            .border(px(border_w))
            .border_color(border)
            .overflow_hidden()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| this.click(event, window, cx)),
            )
            .child(probe)
            .children(mark_elements)
            .when(empty, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(PADDING + 3.))
                        .top(px(line_top))
                        .h(px(LINE_H))
                        .text_size(px(FONT))
                        .line_height(px(LINE_H))
                        .text_color(p.text_tertiary)
                        .child(placeholder),
                )
            })
            .when(!empty, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(PADDING - scroll))
                        .top(px(line_top))
                        .h(px(LINE_H))
                        .whitespace_nowrap()
                        .text_size(px(FONT))
                        .line_height(px(LINE_H))
                        .text_color(p.text)
                        .child(SharedString::from(line_text)),
                )
            })
            .children(caret);

        // The window, lined up with the composition below (or above) the line, kept inside the wizard window.
        let anchor_x = PADDING + composition_x - scroll;
        let line_y = box_top + line_top;
        // Within the stage around the field (its padding less `STAGE_MARGIN`) when it fits there, else within the
        // wizard window.
        let place_x = |width: f32, inset: f32| {
            let x = anchor_x - inset;
            let min = (EDGE - field_x).max(-STAGE_MARGIN);
            let max = (viewport_w - EDGE - field_x - width).min(box_w + STAGE_MARGIN - width);
            if max >= min {
                x.clamp(min, max)
            } else {
                x.clamp(EDGE - field_x, (viewport_w - EDGE - field_x - width).max(EDGE - field_x))
            }
        };
        let panel = (shows_panel && panel_size.0 > 0.).then(|| {
            let (w, height) = panel_size;
            let y = if above { line_y - GAP - height } else { line_y + LINE_H + GAP };
            let since = self.panel_since.map_or(1., |at| at.elapsed().as_secs_f32() * 1000. / APPEAR_MS);
            let t = if self.still || !animates { 1. } else { ease_out(since) };
            if t < 1. {
                window.request_animation_frame();
            }
            let slide = if reduced { 0. } else { SLIDE * (1. - t) * if above { -1. } else { 1. } };
            let weak = cx.weak_entity();
            let actions: ActionHandler = Rc::new(move |action, _window, cx| {
                let _ = weak.update(cx, |field, cx| field.popup_action(action, cx));
            });
            div()
                .absolute()
                .left(px(place_x(w, text_inset)))
                .top(px(y + slide))
                .w(px(w))
                .h(px(height))
                .opacity(t)
                .child(self.panel.render(&panel_palette, Some(actions), window))
        });
        let badge = self.badge.and_then(|(english, at)| {
            let elapsed = at.elapsed().as_secs_f32();
            let end = BADGE_SHOWN + BADGE_FADE;
            if elapsed >= end || !toast_on {
                return None;
            }
            window.request_animation_frame();
            let fade = if animates && elapsed > BADGE_SHOWN { 1. - (elapsed - BADGE_SHOWN) / BADGE_FADE } else { 1. };
            let y = if above { line_y - GAP - BADGE_H } else { line_y + LINE_H + GAP };
            Some(
                div()
                    .absolute()
                    .left(px(place_x(crate::candidate::layout::BADGE_W, 0.)))
                    .top(px(y))
                    .opacity(fade)
                    .child(mode_badge(if english { "英" } else { "中" }, &panel_palette, true)),
            )
        });
        if self.badge.is_some_and(|(_, at)| at.elapsed().as_secs_f32() >= BADGE_SHOWN + BADGE_FADE) {
            self.badge = None;
        }

        v().w_full().gap(TASK_GAP).children(self.task_line(window, cx)).child(
            div()
                .id("practice-field")
                .relative()
                .w_full()
                .h(px(area_h))
                .track_focus(&self.focus)
                .key_context(CONTEXT)
                .on_action(cx.listener(|this, _: &PracticeTab, _, cx| this.tab(false, cx)))
                .on_action(cx.listener(|this, _: &PracticeBackTab, _, cx| this.tab(true, cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.key_down(event, cx)))
                .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| this.key_up(event, cx)))
                .on_modifiers_changed(
                    cx.listener(|this, event: &ModifiersChangedEvent, _, cx| this.modifiers_changed(event, cx)),
                )
                .child(the_box)
                .children(panel)
                .children(badge),
        )
    }
}

/// A still box with the task, for a step that is leaving (the live field has moved on to the next task).
pub fn placeholder(task: &Task, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    v().w_full()
        .gap(TASK_GAP)
        .child(
            v().gap(space::S4)
                .child(
                    h().gap(space::S8)
                        .child(Icon::new(IconName::Keyboard).size(size::ICON_MD).text_color(p.text_secondary))
                        .child(
                            div()
                                .text_size(text::HEADLINE)
                                .font_weight(weight::SEMIBOLD)
                                .text_color(p.text)
                                .child(task.prompt),
                        ),
                )
                .when_some(task.hint, |this, hint| {
                    this.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(hint))
                }),
        )
        .child(div().w_full().h(px(BOX_H)).rounded(RADIUS).bg(field_fill(&p)).border_1().border_color(p.separator))
        .into_any_element()
}

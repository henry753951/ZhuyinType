//! The input controller (Qt `InputController`, macOS `ControllerCore`): keys in, composer inputs out, with the
//! shortcuts, Shift taps for 中/英 (and which Shift), Caps Lock, secure input, the text before the insertion point (the
//! app's, or what was sent in the same app within 10 minutes), pass-through of keys the composer does not take, and
//! the observers: usage events and the correction log.
//!
//! Hosts own focus, secure-input detection and inserting [`ControllerEvent::Commit`] into the app.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Instant;

use zhuyin_core::api::{KeyInput, PunctuationWidth};

use crate::corrections::{CandidateDetails, CorrectionLog, CorrectionRecorder, ObservedInput, correction_settings};
use crate::engine::EngineSession;
use crate::keys::{Key, ShiftSide, code, modifier};
use crate::presentation::Presentation;
use crate::settings::Settings;
use crate::shortcuts::{Action, Shortcuts, is_command_input};
use crate::stats::{PickList, UsageEvent};

/// Receives every usage event while 記錄使用統計 is on.
pub type UsageSink = Arc<dyn Fn(&UsageEvent) + Send + Sync>;

/// What happened, for the host.
#[derive(Clone, Debug, PartialEq)]
pub enum ControllerEvent {
    /// Text for the app (an engine commit or an inserted newline), in order.
    Commit(String),
    /// 中/英 changed (from any cause).
    ModeChanged,
    /// The user switched 中/英 (a Shift tap, Caps Lock, the indicator): the mode toast shows `english`.
    ModeSwitched { english: bool },
    /// The user switched 中文標點 between full and half width: the badge shows 全 or 半.
    WidthSwitched { full: bool },
    /// A page turn or highlight move stopped at the list's end (1 past the last, -1 before the first).
    Boundary(i32),
    /// A candidate was picked by a click.
    CandidateChosen { elapsed_ms: f64 },
}

/// The characters read before the insertion point.
pub const CONTEXT_READ: usize = 128;
/// The characters kept as context.
pub const CONTEXT_KEPT: usize = 255;
/// How long what was sent in an app stays its context, in ms (macOS `ControllerCore.contextWindow`: 60 s).
pub const CONTEXT_WINDOW_MS: i64 = 60_000;
/// The longest Shift press that is a tap, in seconds.
pub const SHIFT_TAP_SECONDS: f64 = 0.35;
/// A gap longer than this is time away, not thinking.
const AWAY_AFTER: f64 = 60.0;

/// The observer's state of one input session (macOS `UsageStatsObserver`).
#[derive(Default)]
struct UsageState {
    last_commit: Option<f64>,
    deleted_since_commit: bool,
    literal_before: Option<String>,
    corrected: bool,
    last_key: Option<f64>,
    /// The reranker showed a sentence with a typing correction during this composition (macOS `sawCorrected`).
    saw_corrected: bool,
}

fn starts_composition(input: KeyInput) -> bool {
    matches!(input, KeyInput::Char(_) | KeyInput::ShiftedChar(_) | KeyInput::Printed(_) | KeyInput::ControlChar(_))
}

/// The last `count` Unicode scalars of `text`.
fn last_scalars(text: &str, count: usize) -> String {
    let total = text.chars().count();
    text.chars().skip(total.saturating_sub(count)).collect()
}

/// The same text may be both a recent commit and the app's document; their overlap is included once (macOS
/// `ControllerCore.joinedContext`).
fn joined_context(recent: &str, before: &str) -> String {
    if recent.is_empty() || before.ends_with(recent) {
        return before.to_owned();
    }
    if before.is_empty() || recent.ends_with(before) {
        return recent.to_owned();
    }
    let r: Vec<char> = recent.chars().collect();
    let b: Vec<char> = before.chars().collect();
    for length in (1..=r.len().min(b.len())).rev() {
        if r[r.len() - length..] == b[..length] {
            return format!("{recent}{}", b[length..].iter().collect::<String>());
        }
    }
    format!("{recent}\n{before}")
}

/// How `input` moves through an open list, for keys that stop at its ends: the direction and whether it turns pages
/// (macOS `ControllerCore.listMove`).  Space goes round to the first page, so it never stops.
fn list_move(input: KeyInput, vertical: bool) -> Option<(i32, bool)> {
    match input {
        KeyInput::Tab | KeyInput::PageDown => Some((1, true)),
        KeyInput::ShiftTab | KeyInput::PageUp => Some((-1, true)),
        KeyInput::Down => Some((1, !vertical)),
        KeyInput::Up => Some((-1, !vertical)),
        KeyInput::Right if !vertical => Some((1, false)),
        KeyInput::Left if !vertical => Some((-1, false)),
        _ => None,
    }
}

/// The input controller.  Owns the engine session; used by one thread at a time.
pub struct InputController {
    session: EngineSession,
    settings: Settings,
    shortcuts: Shortcuts,
    settings_revision: Option<u64>,
    english: bool,
    caps_lock: bool,
    secure: bool,
    /// A private field (a browser's private window): typing works, nothing is learned, read or remembered.
    private: bool,
    held_shifts: BTreeSet<ShiftSide>,
    shift_tap: Option<(ShiftSide, f64)>,
    recent_text: String,
    recent_time: i64,
    app: String,
    client_before: Option<String>,
    app_context: HashMap<String, (String, i64)>,
    usage: UsageState,
    usage_sink: Option<UsageSink>,
    recorder: Option<CorrectionRecorder>,
    events: Vec<ControllerEvent>,
    clock: Box<dyn Fn() -> f64 + Send>,
    now: Box<dyn Fn() -> i64 + Send>,
    /// The gap that ends a burst of typing (four times the usual key interval, at least 2 s); the statistics store
    /// may set it from the user's rhythm.
    pub pause_threshold: f64,
}

impl InputController {
    /// A controller on `session` with `settings` and `shortcuts` (read live: changes apply from the next call).
    #[must_use]
    pub fn new(session: EngineSession, settings: Settings, shortcuts: Shortcuts) -> Self {
        let started = Instant::now();
        let english = settings.bool("startInEnglish");
        let mut controller = Self {
            session,
            settings,
            shortcuts,
            settings_revision: None,
            english,
            caps_lock: false,
            secure: false,
            private: false,
            held_shifts: BTreeSet::new(),
            shift_tap: None,
            recent_text: String::new(),
            recent_time: i64::MIN / 2,
            app: String::new(),
            client_before: None,
            app_context: HashMap::new(),
            usage: UsageState::default(),
            usage_sink: None,
            recorder: None,
            events: Vec::new(),
            clock: Box::new(move || started.elapsed().as_secs_f64()),
            now: Box::new(|| chrono::Utc::now().timestamp_millis()),
            pause_threshold: 2.0,
        };
        controller.sync_settings();
        controller.events.clear();
        controller
    }

    #[must_use]
    pub fn session(&self) -> &EngineSession {
        &self.session
    }

    /// The engine session.  Call [`InputController::refresh_settings`] after opening an engine on it.
    pub fn session_mut(&mut self) -> &mut EngineSession {
        &mut self.session
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    #[must_use]
    pub fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// What the composer shows now.
    #[must_use]
    pub fn presentation(&self) -> &Presentation {
        self.session.presentation()
    }

    /// Applies the settings to the engine again (after `open`, or to pick up a change before the next key).
    pub fn refresh_settings(&mut self) {
        self.settings_revision = None;
        self.sync_settings();
    }

    /// Settings changed since the last call: the engine, the recorder and the Shift tap follow.
    fn sync_settings(&mut self) {
        let revision = self.settings.revision();
        if self.settings_revision == Some(revision) {
            return;
        }
        let first = self.settings_revision.is_none();
        self.settings_revision = Some(revision);
        if let Err(error) = self.session.apply(&self.settings) {
            log::warn!("settings refused by the engine: {error}");
        }
        self.shift_tap = None;
        if !self.settings.bool("usesPrecedingText") {
            self.app_context.clear();
        }
        if !self.secure && self.english() && self.session.ready() && self.session.composing() {
            self.pass_through();
        }
        self.sync_recorder();
        if !first {
            self.events.push(ControllerEvent::ModeChanged);
        }
    }

    fn sync_recorder(&mut self) {
        let width = self.session.punctuation_width();
        if let Some(recorder) = &mut self.recorder {
            recorder.settings = correction_settings(&self.settings, Some(width));
            recorder.selection_keys = self.settings.string("selectionKeys").chars().collect();
            recorder.records_context = self.settings.bool("recordsCorrectionContext");
        }
    }

    /// Keys type English (英, or Caps Lock with 「Caps Lock 打英文」).
    #[must_use]
    pub fn english(&self) -> bool {
        self.english || (self.caps_lock && self.settings.bool("feature.capsLockEnglish"))
    }

    fn clock(&self) -> f64 {
        (self.clock)()
    }

    // MARK: Commits and the text before the insertion point

    /// A finalised syllable may change spelling, but a shortened prefix must not discard unfinished input (macOS
    /// `preservingVisibleSuffix`).
    fn preserving_visible(&self, commit: String, before: &Presentation) -> String {
        if !commit.is_empty() && self.session.presentation().is_empty() && before.composition.starts_with(&commit) {
            before.composition.clone()
        } else {
            commit
        }
    }

    fn committed(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.events.push(ControllerEvent::Commit(text.to_owned()));
        if self.secure || self.private {
            return;
        }
        self.recent_text.push_str(text);
        self.remember();
    }

    fn remember(&mut self) {
        if self.secure || self.private || !self.settings.bool("usesPrecedingText") {
            self.recent_text.clear();
            self.app_context.remove(&self.app);
            return;
        }
        self.recent_text = last_scalars(&self.recent_text, CONTEXT_KEPT);
        self.recent_time = (self.now)();
        self.app_context.insert(self.app.clone(), (self.recent_text.clone(), self.recent_time));
    }

    /// Before a composition starts: the app's text before the insertion point, joined with what was sent in this app
    /// within the window (macOS `ControllerCore.prepareContext`).
    fn prepare_context(&mut self) {
        // Secure input and private fields: no text is read or remembered, and nothing is learned from this
        // composition.
        if self.secure || self.private {
            self.recent_text.clear();
            if self.private && self.session.ready() {
                let _ = self.session.set_preceding_text("");
            }
            return;
        }
        let time = (self.now)();
        if time - self.recent_time > CONTEXT_WINDOW_MS {
            self.recent_text.clear();
        }
        if !self.settings.bool("usesPrecedingText") {
            return;
        }
        let saved = self
            .app_context
            .get(&self.app)
            .filter(|(_, at)| (0..=CONTEXT_WINDOW_MS).contains(&(time - *at)))
            .map(|(text, _)| text.clone());
        let base = match (saved, &self.client_before) {
            (None, None) => self.recent_text.clone(),
            (saved, before) => joined_context(
                saved.as_deref().unwrap_or_default(),
                &last_scalars(before.as_deref().unwrap_or_default(), CONTEXT_READ),
            ),
        };
        self.recent_text = last_scalars(&base, CONTEXT_KEPT);
        let _ = self.session.set_preceding_text(&self.recent_text);
        self.recent_time = time;
    }

    /// Sends everything composed (a key for the app arrived); the key itself goes to the app.
    fn pass_through(&mut self) -> bool {
        if !self.session.ready() {
            return false;
        }
        if self.session.composing() {
            let before = self.session.presentation().clone();
            let commit = self.session.commit_all().map(|o| o.commit).unwrap_or_default();
            let commit = self.preserving_visible(commit, &before);
            self.committed(&commit);
            let time = self.clock();
            self.observe(ObservedInput::Commit, &before, &commit, true, None, None, time);
        }
        let _ = self.session.break_context();
        if let Some(recorder) = &mut self.recorder {
            recorder.break_context();
        }
        false
    }

    fn candidate_details(&self, input: &ObservedInput, before: &Presentation) -> Option<CandidateDetails> {
        let recorder = self.recorder.as_ref()?;
        if self.secure || self.private || !self.settings.bool("recordsCorrections") {
            return None;
        }
        let index = recorder.candidate_index(input, before)?;
        self.session.candidate_details(index)
    }

    /// One composer input with its observers.
    fn engine_input(&mut self, input: KeyInput, time: f64) -> bool {
        if !self.session.composing() && starts_composition(input) {
            self.prepare_context();
        }
        // With 「送出後刪掉重打」 off, ⌫ on sent text is an ordinary key for the app: the next composition starts a
        // sentence and nothing is learned from retyping.
        if input == KeyInput::Backspace && !self.session.composing() && !self.settings.bool("feature.deleteAfterCommit")
        {
            let _ = self.session.break_context();
        }
        let before = self.session.presentation().clone();
        let observed = ObservedInput::Key { input, time };
        let candidate = self.candidate_details(&observed, &before);
        let Ok(outcome) = self.session.input(input) else { return false };
        if let Some((direction, page)) = list_move(input, self.settings.string("candidateLayout") != "horizontal") {
            let after = self.session.presentation();
            if before.has_list()
                && after.has_list()
                && before.count == after.count
                && before.page_size == after.page_size
                && if page { before.page == after.page } else { before.selection == after.selection }
            {
                self.events.push(ControllerEvent::Boundary(direction));
            }
        }
        let commit = self.preserving_visible(outcome.commit, &before);
        self.committed(&commit);
        self.observe(observed, &before, &commit, outcome.handled, outcome.elapsed_ms, candidate, time);
        if !outcome.handled && !self.secure && !self.private {
            // Keys the app receives extend or trim the text before the insertion point.
            match input {
                KeyInput::Backspace => {
                    self.recent_text.pop();
                    self.remember();
                }
                KeyInput::Space => {
                    self.recent_text.push(' ');
                    self.remember();
                }
                KeyInput::Enter | KeyInput::ShiftEnter => {
                    self.recent_text.push('\n');
                    self.remember();
                }
                _ => {}
            }
        }
        outcome.handled
    }

    // MARK: Keys

    /// A key went down; false: the app gets it (after any [`ControllerEvent::Commit`]).
    pub fn press(&mut self, key: &Key) -> bool {
        self.sync_settings();
        let time = self.clock();
        if key.code == code::SHIFT {
            self.shift_pressed(key, time);
            return false;
        }
        self.shift_tap = None;
        // Modifier-down is not a shortcut yet; the composition waits for the key that follows.
        if key.is_modifier() {
            return false;
        }
        if key.code == code::CAPS_LOCK {
            if !key.repeat {
                let before = self.english();
                self.caps_lock = !self.caps_lock;
                if !self.secure && self.settings.bool("feature.capsLockEnglish") {
                    self.pass_through();
                }
                self.mode_changed(before);
            }
            return false;
        }
        if self.secure || !self.session.ready() {
            return false;
        }
        if self.english() {
            return self.pass_through();
        }
        if let Some(command) = self.shortcuts.resolve(key.code, key.modifiers) {
            if !self.shortcuts.is_enabled(command.id) {
                return self.pass_through();
            }
            return self.dispatch(command.id, command.action, key, time);
        }
        // A command's input comes only through its key; its old default key, now unbound, is the app's.
        let Some(input) = key.input().filter(|input| !is_command_input(*input) && !self.turned_off(*input)) else {
            return self.pass_through();
        };
        let handled = self.engine_input(input, time);
        // Keys the engine passes through (or whose feature is off) send the composition first.
        if !handled && self.session.composing() {
            return self.pass_through();
        }
        handled
    }

    /// ⌃ and ⌥ symbol keys whose feature is off go to the app.
    fn turned_off(&self, input: KeyInput) -> bool {
        match input {
            KeyInput::ControlChar(_) => !self.settings.bool("feature.controlPunctuation"),
            KeyInput::Printed(_) => !self.settings.bool("feature.optionSymbols"),
            _ => false,
        }
    }

    fn dispatch(&mut self, id: &str, action: Action, key: &Key, time: f64) -> bool {
        match action {
            Action::Neighbour(delta) => {
                let arrow = if delta < 0 { KeyInput::Left } else { KeyInput::Right };
                let p = self.session.presentation().clone();
                if !p.has_list() || self.settings.string("candidateLayout") != "horizontal" {
                    return self.engine_input(arrow, time);
                }
                // The list moves to the neighbouring character in the same mode (macOS `ControllerCore.neighbour`).
                let reopen = if p.single {
                    KeyInput::ShiftDown
                } else if p.corrections {
                    KeyInput::ShiftUp
                } else {
                    KeyInput::Down
                };
                for input in [KeyInput::Escape, arrow, reopen] {
                    let _ = self.session.input(input);
                }
                true
            }
            Action::TogglePunctuationWidth => {
                let Ok(width) = self.session.toggle_punctuation_width() else { return false };
                self.events.push(ControllerEvent::WidthSwitched { full: width != PunctuationWidth::Half });
                let after = self.session.presentation().clone();
                let secure = self.secure;
                if let Some(recorder) = &mut self.recorder {
                    recorder.punctuation_width_changed(width, &after, secure, time);
                }
                true
            }
            Action::Input(input) => {
                let handled = self.engine_input(input, time);
                if !handled && id == "commitWithNewline" && key.canonical() != (code::RETURN, modifier::SHIFT) {
                    // A remapped 送出並換行: the newline is the input method's to type.
                    self.events.push(ControllerEvent::Commit("\n".to_owned()));
                    return true;
                }
                if !handled && self.session.composing() {
                    return self.pass_through();
                }
                handled
            }
        }
    }

    fn mode_changed(&mut self, before: bool) {
        if before != self.english() {
            self.events.push(ControllerEvent::ModeChanged);
            if !self.secure {
                self.events.push(ControllerEvent::ModeSwitched { english: self.english() });
            }
        }
    }

    fn shift_pressed(&mut self, key: &Key, time: f64) {
        if key.repeat || self.held_shifts.contains(&key.shift_side) {
            return;
        }
        self.held_shifts.insert(key.shift_side);
        let allowed = match self.settings.string("shiftToggle").as_str() {
            "left" => key.shift_side == ShiftSide::Left,
            "right" => key.shift_side == ShiftSide::Right,
            _ => true,
        };
        self.shift_tap = None;
        if !self.secure && allowed && self.held_shifts.len() == 1 && key.modifiers & !modifier::SHIFT == 0 {
            self.shift_tap = Some((key.shift_side, time));
        }
    }

    /// A key went up (Shift taps).
    pub fn release(&mut self, key: &Key) {
        self.sync_settings();
        if key.code != code::SHIFT {
            self.shift_tap = None;
            return;
        }
        if !self.held_shifts.remove(&key.shift_side) {
            return;
        }
        let time = self.clock();
        let tap = self.shift_tap.take();
        let toggle = tap
            .is_some_and(|(side, at)| side == key.shift_side && time >= at && time - at <= SHIFT_TAP_SECONDS)
            && self.held_shifts.is_empty()
            && key.modifiers & !modifier::SHIFT == 0
            && self.settings.bool("shiftSwitch");
        if toggle && !self.secure {
            if self.session.ready() && self.session.composing() {
                self.pass_through();
                if self.session.composing() {
                    return;
                }
            }
            self.switch_mode(!self.english);
        }
    }

    fn switch_mode(&mut self, english: bool) {
        if self.session.ready() {
            let _ = self.session.break_context();
        }
        if let Some(recorder) = &mut self.recorder {
            recorder.break_context();
        }
        self.english = english;
        self.events.push(ControllerEvent::ModeChanged);
        self.events.push(ControllerEvent::ModeSwitched { english: self.english() });
    }

    /// Sets 中/英 from outside the keyboard (the taskbar indicator), like a Shift tap: not while composing.
    pub fn set_english(&mut self, english: bool) {
        self.sync_settings();
        if self.secure || english == self.english || (self.session.ready() && self.session.composing()) {
            return;
        }
        self.switch_mode(english);
    }

    /// Picks candidate `index` of the whole list (a click).
    pub fn choose(&mut self, index: usize) -> bool {
        self.sync_settings();
        if self.secure || self.english() || !self.session.ready() {
            return false;
        }
        self.shift_tap = None;
        let before = self.session.presentation().clone();
        let observed = ObservedInput::Click { index };
        let candidate = self.candidate_details(&observed, &before);
        let Ok(outcome) = self.session.choose(index) else { return false };
        if !outcome.handled {
            return false;
        }
        let commit = self.preserving_visible(outcome.commit, &before);
        self.committed(&commit);
        let time = self.clock();
        self.observe(observed, &before, &commit, true, None, candidate, time);
        self.events.push(ControllerEvent::CandidateChosen { elapsed_ms: outcome.elapsed_ms.unwrap_or(0.0) });
        true
    }

    /// A key from the candidate window's own controls (page buttons, list switches), observed like keys.
    pub fn feed(&mut self, key: &Key) -> bool {
        self.sync_settings();
        if self.secure || self.english() || !self.session.ready() {
            return false;
        }
        self.shift_tap = None;
        let Some(input) = key.input() else { return false };
        let time = self.clock();
        self.engine_input(input, time)
    }

    /// The candidate window's corrections switch.
    pub fn toggle_corrections(&mut self) {
        self.sync_settings();
        if self.secure || self.english() || !self.session.ready() || !self.session.presentation().has_list() {
            return;
        }
        let before = self.session.presentation().clone();
        let _ = self.session.toggle_corrections();
        let time = self.clock();
        self.observe(ObservedInput::ToggleCorrections, &before, "", true, None, None, time);
    }

    /// Secure input on or off: the composition is discarded and learning paused while on.
    pub fn set_secure(&mut self, secure: bool) {
        self.sync_settings();
        self.held_shifts.clear();
        self.shift_tap = None;
        if secure || self.secure {
            self.discard_secure_composition();
        }
        self.secure = secure;
        if secure {
            self.recent_text.clear();
            if self.usage.last_key.is_some()
                && let Some(sink) = &self.usage_sink
                && self.settings.bool("recordsUsageStats")
            {
                sink(&UsageEvent::Reset);
            }
            self.usage = UsageState::default();
            if let Some(recorder) = &mut self.recorder {
                let empty = Presentation::default();
                recorder.observe(&ObservedInput::Commit, &empty, &empty, "", true, true, None, 0.0, &String::new);
            }
        }
        if self.session.ready() {
            let _ = self.session.set_learning_suspended(secure || self.private);
        }
    }

    /// The input method was selected again after another input source: 中/英 starts as 「開始輸入時使用英文」 says
    /// (macOS `EngineHost` on activation).
    pub fn activated(&mut self) {
        let english = self.settings.bool("startInEnglish");
        if english == self.english || (self.session.ready() && self.session.composing()) {
            return;
        }
        self.english = english;
        self.events.push(ControllerEvent::ModeChanged);
    }

    /// A private field (IPC.md `private`: a browser's private window) on or off: Chinese typing works as anywhere
    /// else, but nothing is learned, no text before the insertion point is read, and nothing sent is remembered as
    /// context — macOS's behaviour under secure input.
    pub fn set_private(&mut self, private: bool) {
        if private == self.private {
            return;
        }
        self.private = private;
        if private {
            self.recent_text.clear();
            self.app_context.remove(&self.app);
        }
        if self.session.ready() {
            let _ = self.session.set_learning_suspended(self.secure || private);
        }
    }

    #[must_use]
    pub fn secure(&self) -> bool {
        self.secure
    }

    #[must_use]
    pub fn private(&self) -> bool {
        self.private
    }

    fn discard_secure_composition(&mut self) {
        if !self.session.ready() {
            return;
        }
        let _ = self.session.set_learning_suspended(true);
        let _ = self.session.reset();
        // The composer ignores the text before the insertion point while composing: clear it after the reset.
        let _ = self.session.set_preceding_text("");
    }

    /// The field lost focus: the composition is sent (discarded under secure input).
    pub fn deactivate(&mut self) {
        self.sync_settings();
        self.held_shifts.clear();
        self.shift_tap = None;
        if self.secure {
            self.discard_secure_composition();
        } else {
            self.pass_through();
        }
        self.client_before = None;
    }

    /// The focused field's app and, when the platform reports it, the text before the insertion point (`None`:
    /// unknown).  Read when a composition starts.
    pub fn set_client(&mut self, app: &str, before: Option<String>) {
        if app != self.app {
            app.clone_into(&mut self.app);
            self.recent_text.clear();
            self.recent_time = i64::MIN / 2;
        }
        self.client_before = before;
    }

    /// The platform's Caps Lock state.
    pub fn synchronize_caps_lock(&mut self, enabled: bool) {
        self.sync_settings();
        if self.caps_lock == enabled {
            return;
        }
        self.caps_lock = enabled;
        if !self.secure && self.english() {
            self.pass_through();
        }
        self.events.push(ControllerEvent::ModeChanged);
    }

    /// Where usage events go (`None`: nowhere).  Events are sent only while 記錄使用統計 is on.
    pub fn set_usage_sink(&mut self, sink: Option<UsageSink>) {
        self.usage_sink = sink;
    }

    /// The correction log records go to (`None`: none), and the app version they carry.
    pub fn set_correction_log(&mut self, log: Option<CorrectionLog>, app_version: &str) {
        self.recorder = log.map(|log| {
            let mut recorder = CorrectionRecorder::new(Box::new(move |record| {
                if let Err(error) = log.append(&serde_json::Value::Object(record)) {
                    log::warn!("correction log: {error}");
                }
            }));
            recorder.app_version = if app_version.is_empty() { "dev".to_owned() } else { app_version.to_owned() };
            recorder
        });
        self.sync_recorder();
    }

    /// What happened since the last call.
    pub fn take_events(&mut self) -> Vec<ControllerEvent> {
        std::mem::take(&mut self.events)
    }

    /// The text before the composition, as the engine reads it.
    #[must_use]
    pub fn recent_text(&self) -> &str {
        &self.recent_text
    }

    /// Replaces the monotonic clock (seconds) used for key times and Shift taps (tests).
    pub fn set_clock(&mut self, clock: impl Fn() -> f64 + Send + 'static) {
        self.clock = Box::new(clock);
    }

    /// Replaces the wall clock (ms) used for the 10-minute context window (tests).
    pub fn set_now(&mut self, now: impl Fn() -> i64 + Send + 'static) {
        self.now = Box::new(now);
    }

    // MARK: Observers

    #[allow(clippy::too_many_arguments)]
    fn observe(
        &mut self,
        input: ObservedInput,
        before: &Presentation,
        commit: &str,
        handled: bool,
        latency_ms: Option<f64>,
        candidate: Option<CandidateDetails>,
        time: f64,
    ) {
        let recording = self.recorder.is_some() && self.settings.bool("recordsCorrections");
        let counting = self.usage_sink.is_some() && self.settings.bool("recordsUsageStats");
        if !recording && !counting {
            return;
        }
        let after = self.session.presentation().clone();
        if recording {
            let secure = self.secure;
            let context = self.recent_text.clone();
            if let Some(recorder) = &mut self.recorder {
                recorder.observe(&input, before, &after, commit, handled, secure, candidate, time, &move || {
                    context.clone()
                });
            }
        }
        if counting {
            self.observe_usage(input, before, &after, commit, handled, latency_ms);
        }
    }

    /// macOS `UsageStatsObserver`, as events (`IPC.md` §使用統計).
    fn observe_usage(
        &mut self,
        input: ObservedInput,
        before: &Presentation,
        after: &Presentation,
        commit: &str,
        handled: bool,
        latency_ms: Option<f64>,
    ) {
        let Some(sink) = self.usage_sink.clone() else { return };
        if self.secure {
            if self.usage.last_key.is_some() {
                sink(&UsageEvent::Reset);
            }
            self.usage = UsageState::default();
            return;
        }
        let composing = !before.is_empty() || !after.is_empty();
        let mut corrected = false;
        if let ObservedInput::Key { time, .. } = input {
            // Rhythm: the gap since the last key decides typing, thinking or away.
            if let Some(last) = self.usage.last_key {
                let gap = time - last;
                if (0.0..=self.pause_threshold).contains(&gap) {
                    sink(&UsageEvent::Interval { ms: (gap * 1000.0).trunc() });
                } else if gap > self.pause_threshold && gap <= AWAY_AFTER {
                    sink(&UsageEvent::Pause { ms: (gap * 1000.0).trunc() });
                }
            }
            self.usage.last_key = Some(time);
            // Every key the input method saw; `composing` when it was handled while composing (a counted key).
            sink(&UsageEvent::Key { latency_ms: latency_ms.unwrap_or(0.0), composing: handled && composing });
        }
        // A pick: the list closed by a click, Return or a selection key.
        let selection = self.settings.string("selectionKeys");
        let picked = match input {
            ObservedInput::Click { .. } => true,
            ObservedInput::Key { input: KeyInput::Enter, .. } => true,
            ObservedInput::Key { input: KeyInput::Char(c), .. } => selection.contains(c),
            _ => false,
        };
        if before.has_list() && !after.has_list() && picked {
            let list = if before.tolerant == Some(true) {
                PickList::Corrections
            } else if before.single {
                PickList::Single
            } else if before.reading.as_deref().unwrap_or_default().is_empty() {
                PickList::Punctuation
            } else {
                PickList::Normal
            };
            sink(&UsageEvent::Pick(list));
            corrected = true;
        }
        let now = self.clock();
        match input {
            ObservedInput::Key { input: key, .. } => {
                let u = &mut self.usage;
                let backtick_or_escape = matches!(key, KeyInput::Char('`') | KeyInput::Escape);
                if handled && backtick_or_escape && u.literal_before.as_deref() == Some(after.composition.as_str()) {
                    sink(&UsageEvent::UndoLiteral);
                    u.literal_before = None;
                } else if key == KeyInput::Char('`')
                    && handled
                    && !before.is_empty()
                    && after.composition != before.composition
                {
                    sink(&UsageEvent::LiteralZhuyin);
                    u.literal_before = Some(before.composition.clone());
                    corrected = true;
                } else {
                    u.literal_before = None;
                    if matches!(key, KeyInput::Backspace | KeyInput::OptionBackspace) {
                        if handled && !before.is_empty() && !before.has_list() {
                            sink(&UsageEvent::Backspace);
                            corrected = true;
                        } else if !handled
                            && before.is_empty()
                            && !u.deleted_since_commit
                            && u.last_commit.is_some_and(|at| now - at <= 30.0)
                        {
                            sink(&UsageEvent::DeleteAfterCommit);
                            u.deleted_since_commit = true;
                            corrected = true;
                        }
                    } else if key == KeyInput::Escape
                        && handled
                        && !before.is_empty()
                        && !before.has_list()
                        && after.composition != before.composition
                    {
                        sink(&UsageEvent::Escape);
                        corrected = true;
                    }
                }
            }
            ObservedInput::Click { .. } => self.usage.literal_before = None,
            _ => {}
        }
        if corrected {
            self.usage.corrected = true;
        }
        let added = after.notice.as_deref().is_some_and(|n| n.starts_with("已加入自訂詞"));
        if added && after.notice != before.notice {
            sink(&UsageEvent::PhraseAdded);
        }
        // 自動修正: the reranker showed a corrected sentence, and whether the one sent was one (macOS
        // `UsageStatsObserver`).
        let shown_corrected = self.session.shown_corrected();
        if shown_corrected && !after.is_empty() {
            self.usage.saw_corrected = true;
        }
        if !commit.is_empty() {
            sink(&UsageEvent::Commit {
                characters: commit.chars().count() as i64,
                composition_ended: after.is_empty(),
                clean: Some(!self.usage.corrected),
                corrected_shown: self.usage.saw_corrected,
                corrected_committed: shown_corrected,
            });
            if after.is_empty() {
                self.usage.saw_corrected = false;
                self.session.clear_shown_corrected();
                self.usage.corrected = false;
                self.usage.last_commit = Some(now);
                self.usage.deleted_since_commit = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_context_once() {
        assert_eq!(joined_context("", "我說"), "我說");
        assert_eq!(joined_context("我說你好", "你好"), "我說你好");
        assert_eq!(joined_context("我說你好", "你好嗎"), "我說你好嗎");
        assert_eq!(joined_context("甲", "乙"), "甲\n乙");
        assert_eq!(last_scalars("一二三", 2), "二三");
    }
}

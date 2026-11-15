//! Practice sessions (macOS `UI/Wizard/Practice/`, Qt `Practice`): the 介紹 try-it field and the setup wizard's
//! practice tasks.  A sandboxed input controller of its own, on an in-memory copy of the user's settings and keys, with
//! in-memory personal data on the shared engine data, typing into a single-line text of its own.  Nothing reaches the
//! user's settings, keys, words, statistics or correction log.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use zhuyin_core::engine::Engine;

use crate::controller::{ControllerEvent, InputController, UsageSink};
use crate::corrections::CorrectionLog;
use crate::engine::{EngineData, EngineSession};
use crate::keys::{Key, code, modifier};
use crate::presentation::Presentation;
use crate::service::PopupAction;
use crate::settings::{self, Settings};
use crate::shortcuts::{self, Shortcuts};
use crate::stats::UsageStats;

/// The events the field puts in [`PracticeOutcome::events`] (macOS `PracticeEventName`).
pub mod event {
    /// A candidate list was opened.
    pub const OPEN: &str = "open";
    /// The candidate list turned to another page.
    pub const PAGE: &str = "page";
    /// A candidate was picked from a list.
    pub const PICK: &str = "pick";
    /// A candidate was picked in place of different text.
    pub const CORRECT: &str = "correct";
    /// The corrections list (⇧↑) was opened.
    pub const CORRECTIONS: &str = "corrections";
    /// 中/英 switched.
    pub const MODE: &str = "mode";
    /// A user phrase was added (to the practice data).
    pub const PHRASE: &str = "phrase";
}

/// What the field reports for a task's check (macOS `PracticeOutcome`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PracticeOutcome {
    /// The text sent into the field.
    pub sent: String,
    /// The field's text as shown (sent text with the composition at the insertion point).
    pub text: String,
    /// What happened ([`event`]), in order.
    pub events: Vec<String>,
    /// Candidates picked.
    pub picks: Vec<String>,
    /// Candidates picked in place of different text.
    pub corrected: Vec<String>,
    /// Picks from the corrections list.
    pub correction_picks: usize,
    /// User phrases added.
    pub phrases: Vec<String>,
}

/// One thing a task asks for (macOS `PracticeGoal`).  Goals about what happened stay met; goals about the text are
/// checked against the field as it is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PracticeGoal {
    /// The sent text is exactly this (whitespace at either end aside).
    Text(String),
    /// The field's text (sent and composed) contains this.
    Contains(String),
    OpenCandidates,
    TurnPage,
    /// A candidate was picked: this one, or any.
    Pick(Option<String>),
    /// This candidate was picked in place of different text.
    Correct(String),
    /// A candidate was picked from the corrections list.
    PickCorrection,
    /// 中/英 switched at least this many times.
    SwitchMode(usize),
    /// This word was added as a user phrase.
    AddUserPhrase(String),
}

impl PracticeGoal {
    /// The goal is met by `outcome`.
    #[must_use]
    pub fn is_met(&self, outcome: &PracticeOutcome) -> bool {
        let happened = |name: &str| outcome.events.iter().any(|e| e == name);
        match self {
            Self::Text(target) => outcome.sent.trim() == target,
            Self::Contains(part) => outcome.text.contains(part.as_str()),
            Self::OpenCandidates => happened(event::OPEN),
            Self::TurnPage => happened(event::PAGE),
            Self::Pick(word) => word.as_ref().map_or(!outcome.picks.is_empty(), |w| outcome.picks.contains(w)),
            Self::Correct(word) => outcome.corrected.contains(word),
            Self::PickCorrection => outcome.correction_picks > 0,
            Self::SwitchMode(times) => outcome.events.iter().filter(|e| *e == event::MODE).count() >= *times,
            Self::AddUserPhrase(word) => outcome.phrases.contains(word),
        }
    }
}

/// What the field asks its view to show.
#[derive(Clone, Debug, PartialEq)]
pub enum PracticeEvent {
    /// The mode toast (中/英).
    ModeToast { english: bool },
    /// A page turn or highlight move stopped at the list's end.
    Boundary(i32),
}

/// The app name the practice field reports: never one of the user's apps.
pub const PRACTICE_APP: &str = "org.zhuyintype.practice";
/// The longest text kept; the start is dropped beyond it, as a single-line field scrolls.
pub const MAX_LENGTH: usize = 400;

/// One practice field.
pub struct PracticeSession {
    user_settings: Settings,
    settings: Settings,
    shortcuts: Shortcuts,
    controller: InputController,
    corrections: CorrectionLog,
    stats: UsageStats,
    text: String,
    /// UTF-16 offset into `text`.
    insertion: usize,
    outcome: PracticeOutcome,
    known_phrases: BTreeSet<(String, String)>,
    added: Vec<(String, String)>,
    events: Vec<PracticeEvent>,
}

fn byte_index(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The user's values for every setting and the keys, to copy into the sandbox.
fn copy_of(user: &Settings) -> BTreeMap<String, serde_json::Value> {
    let mut values: BTreeMap<String, serde_json::Value> =
        settings::descriptors().iter().map(|d| (d.key.clone(), user.get(&d.key))).collect();
    values.insert(shortcuts::STORAGE_KEY.to_owned(), user.get(shortcuts::STORAGE_KEY));
    values
}

impl PracticeSession {
    /// A field on a copy of `user_settings` (taken now) and `engine` (an engine on `data` with in-memory personal
    /// data, shared by the practice fields open at the same time).
    #[must_use]
    pub fn new(user_settings: &Settings, data: &Arc<EngineData>, engine: Arc<Engine>) -> Self {
        let settings = user_settings.sandbox();
        let shortcuts = Shortcuts::new(settings.clone());
        let mut session = EngineSession::new();
        session.open_shared(data, engine);
        let mut controller = InputController::new(session, settings.clone(), shortcuts.clone());
        controller.refresh_settings();
        let corrections = CorrectionLog::open(None);
        let stats = UsageStats::open(None);
        stats.set_enabled(true);
        let sink_stats = stats.clone();
        let sink: UsageSink = Arc::new(move |event| sink_stats.record(event));
        controller.set_usage_sink(Some(sink));
        controller.set_correction_log(Some(corrections.clone()), "practice");
        controller.take_events();
        let known_phrases = controller.session().user_phrases().into_iter().map(|p| (p.reading, p.word)).collect();
        let mut field = Self {
            user_settings: user_settings.clone(),
            settings,
            shortcuts,
            controller,
            corrections,
            stats,
            text: String::new(),
            insertion: 0,
            outcome: PracticeOutcome::default(),
            known_phrases,
            added: Vec::new(),
            events: Vec::new(),
        };
        field.check(&Presentation::default(), None);
        field
    }

    /// The field's own settings (an in-memory copy the wizard's choices change, in effect at once).
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The field's own keys (on its settings copy).
    #[must_use]
    pub fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// The field's correction log (in memory) and statistics (in memory, always on).
    #[must_use]
    pub fn corrections(&self) -> &CorrectionLog {
        &self.corrections
    }

    #[must_use]
    pub fn stats(&self) -> &UsageStats {
        &self.stats
    }

    /// The controller, for tests and advanced views.
    pub fn controller_mut(&mut self) -> &mut InputController {
        &mut self.controller
    }

    /// The candidate window and composition, as every view draws them.
    #[must_use]
    pub fn presentation(&self) -> Presentation {
        self.controller.presentation().clone()
    }

    /// The text sent into the field.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The field as shown: the sent text with the composition at the insertion point.
    #[must_use]
    pub fn shown(&self) -> String {
        let at = byte_index(&self.text, self.insertion);
        format!("{}{}{}", &self.text[..at], self.controller.presentation().composition, &self.text[at..])
    }

    /// The insertion point in [`PracticeSession::text`] (UTF-16), where the composition sits.
    #[must_use]
    pub fn insertion(&self) -> usize {
        self.insertion
    }

    /// Keys type English.
    #[must_use]
    pub fn english(&self) -> bool {
        self.controller.english()
    }

    fn insert(&mut self, string: &str) {
        let at = byte_index(&self.text, self.insertion);
        self.text.insert_str(at, string);
        self.insertion += utf16_len(string);
        let count = self.text.chars().count();
        if count > MAX_LENGTH {
            let cut = self.text.char_indices().nth(count - MAX_LENGTH).map_or(0, |(i, _)| i);
            let dropped = utf16_len(&self.text[..cut]);
            self.text.replace_range(..cut, "");
            self.insertion = self.insertion.saturating_sub(dropped);
        }
    }

    fn delete_backward(&mut self) {
        let at = byte_index(&self.text, self.insertion);
        if let Some((start, c)) = self.text[..at].char_indices().next_back() {
            self.text.replace_range(start..at, "");
            self.insertion -= c.len_utf16();
        }
    }

    fn step_insertion(&mut self, forward: bool) {
        let at = byte_index(&self.text, self.insertion);
        if forward {
            if let Some(c) = self.text[at..].chars().next() {
                self.insertion += c.len_utf16();
            }
        } else if let Some(c) = self.text[..at].chars().next_back() {
            self.insertion -= c.len_utf16();
        }
    }

    /// The text before the insertion point, as an app reports it.
    fn report_client(&mut self) {
        let at = byte_index(&self.text, self.insertion);
        let before = self.text[..at].to_owned();
        self.controller.set_client(PRACTICE_APP, Some(before));
    }

    /// Runs `work` on the controller, then applies what it did to the field and the outcome.
    fn perform<R>(&mut self, pick: Option<usize>, work: impl FnOnce(&mut InputController) -> R) -> R {
        self.report_client();
        let before = self.controller.presentation().clone();
        let result = work(&mut self.controller);
        self.apply_events();
        self.check(&before, pick);
        result
    }

    fn apply_events(&mut self) {
        for event in self.controller.take_events() {
            match event {
                ControllerEvent::Commit(text) => self.insert(&text),
                ControllerEvent::ModeSwitched { english } => {
                    self.outcome.events.push(event::MODE.to_owned());
                    self.events.push(PracticeEvent::ModeToast { english });
                }
                ControllerEvent::Boundary(direction) => self.events.push(PracticeEvent::Boundary(direction)),
                _ => {}
            }
        }
    }

    /// The candidate a key picks from the list in `before` (Return, a selection key), if any.
    fn picked_by(&self, key: &Key, before: &Presentation) -> Option<usize> {
        if !before.has_list() {
            return None;
        }
        match key.input()? {
            zhuyin_core::api::KeyInput::Enter => before.selection,
            zhuyin_core::api::KeyInput::Char(c) => before.rows.iter().find(|row| row.key == Some(c)).map(|r| r.index),
            _ => None,
        }
    }

    /// Brings the outcome up to date after a change.
    fn check(&mut self, before: &Presentation, pick: Option<usize>) {
        let after = self.controller.presentation().clone();
        let o = &mut self.outcome;
        if !before.has_list() && after.has_list() {
            o.events.push(event::OPEN.to_owned());
        }
        if after.has_list() && after.corrections && !(before.has_list() && before.corrections) {
            o.events.push(event::CORRECTIONS.to_owned());
        }
        if before.has_list() && after.has_list() && before.page != after.page && before.count == after.count {
            o.events.push(event::PAGE.to_owned());
        }
        if let Some(index) = pick
            && before.has_list()
            && !after.has_list()
            && let Some(row) = before.rows.iter().find(|row| row.index == index)
        {
            o.events.push(event::PICK.to_owned());
            o.picks.push(row.text.clone());
            if before.focused_text().as_deref() != Some(row.text.as_str()) {
                o.events.push(event::CORRECT.to_owned());
                o.corrected.push(row.text.clone());
            }
            if before.corrections {
                o.correction_picks += 1;
            }
        }
        // A user phrase added by this change (⇧← ⇧→ and Return).
        for phrase in self.controller.session().user_phrases() {
            let key = (phrase.reading.clone(), phrase.word.clone());
            if self.known_phrases.insert(key) {
                self.added.push((phrase.word.clone(), phrase.reading));
                self.outcome.phrases.push(phrase.word);
                self.outcome.events.push(event::PHRASE.to_owned());
            }
        }
        self.refresh_text();
    }

    fn refresh_text(&mut self) {
        self.outcome.sent.clone_from(&self.text);
        self.outcome.text = self.shown();
    }

    /// A key went down; keys the input method leaves to the app are typed into the field as a single-line text field
    /// takes them.  False: neither used it (Tab with nothing composed), so it goes on to the window.
    pub fn key_down(&mut self, key: &Key) -> bool {
        let before = self.controller.presentation().clone();
        let pick = self.picked_by(key, &before);
        let handled = self.perform(pick, |controller| controller.press(key));
        if handled {
            return true;
        }
        let used = self.type_into_field(key);
        self.refresh_text();
        used
    }

    /// Keys the input method gave to the app, as a single-line text field takes them.
    fn type_into_field(&mut self, key: &Key) -> bool {
        if key.is_modifier() || key.code == code::CAPS_LOCK {
            return true;
        }
        let modifiers = key.modifiers & !modifier::KEYPAD;
        if modifiers & (modifier::CONTROL | modifier::META) != 0 {
            return false;
        }
        match key.code {
            code::BACKSPACE => self.delete_backward(),
            code::TAB | code::BACKTAB | code::ESCAPE => return false,
            code::LEFT => self.step_insertion(false),
            code::RIGHT => self.step_insertion(true),
            code::UP | code::HOME | code::PAGE_UP => self.insertion = 0,
            code::DOWN | code::END | code::PAGE_DOWN => self.insertion = utf16_len(&self.text),
            code::RETURN | code::ENTER | code::DELETE => {}
            _ => {
                let text = if key.text.is_empty() { typed_text(key) } else { Some(key.text.clone()) };
                match text.filter(|t| !t.is_empty() && !t.chars().any(char::is_control)) {
                    Some(text) => self.insert(&text),
                    None => return false,
                }
            }
        }
        true
    }

    /// A key went up (Shift taps).
    pub fn key_up(&mut self, key: &Key) {
        self.perform(None, |controller| controller.release(key));
    }

    /// A click or control in the field's candidate window.
    pub fn popup_action(&mut self, action: PopupAction) {
        let p = self.controller.presentation().clone();
        if !p.has_list() && !matches!(action, PopupAction::TypeReading) {
            return;
        }
        match action {
            PopupAction::Pick(row) => {
                let Some(index) = p.rows.get(row).map(|r| r.index) else { return };
                self.perform(Some(index), |c| c.choose(index));
            }
            PopupAction::TurnPage(delta) if delta.abs() == 1 => {
                let key = Key::new(if delta > 0 { code::PAGE_DOWN } else { code::PAGE_UP }, 0);
                self.perform(None, |c| c.feed(&key));
            }
            PopupAction::MoveSelection(delta) if delta.abs() == 1 => {
                let key = Key::new(if delta > 0 { code::DOWN } else { code::UP }, 0);
                self.perform(None, |c| c.feed(&key));
            }
            PopupAction::ToggleSingle => {
                self.perform(None, |c| c.feed(&Key::new(code::DOWN, modifier::SHIFT)));
            }
            PopupAction::ToggleCorrections => self.perform(None, InputController::toggle_corrections),
            PopupAction::TypeReading
                if p.reading.as_deref().is_some_and(|r| !r.is_empty())
                    && self.settings.bool("feature.zhuyinRowClick") =>
            {
                self.perform(None, |c| c.feed(&Key::new(code::QUOTE_LEFT, 0)));
            }
            _ => {}
        }
    }

    /// Puts the insertion point at `offset` (UTF-16; a click in the field), never inside a character.  The
    /// composition is sent first.
    pub fn move_insertion(&mut self, offset: usize) {
        self.end_session();
        let offset = offset.min(utf16_len(&self.text));
        let at = byte_index(&self.text, offset);
        self.insertion = utf16_len(&self.text[..at]);
        let _ = self.controller.session_mut().break_context();
        self.refresh_text();
    }

    /// The field lost focus: the composition is sent, as in an app.
    pub fn end_session(&mut self) {
        self.perform(None, InputController::deactivate);
    }

    /// Starts over with `initial` text and no composition (a new task); the outcome starts over too.
    pub fn reset(&mut self, initial: &str) {
        self.end_session();
        initial.clone_into(&mut self.text);
        self.insertion = utf16_len(initial);
        self.outcome = PracticeOutcome::default();
        let _ = self.controller.session_mut().break_context();
        self.controller.take_events();
        self.refresh_text();
    }

    /// Takes the user's settings and keys again (a preset chosen again).
    pub fn restart(&mut self) {
        if let Err(error) = self.settings.assign(&copy_of(&self.user_settings)) {
            log::warn!("practice: {error}");
        }
        self.controller.refresh_settings();
        self.apply_events();
    }

    /// What the field did so far.
    #[must_use]
    pub fn outcome(&self) -> &PracticeOutcome {
        &self.outcome
    }

    /// User phrases added while practising, (word, reading), for the wizard to offer to keep.
    #[must_use]
    pub fn added_phrases(&self) -> Vec<(String, String)> {
        self.added.clone()
    }

    /// What the view should show since the last call.
    pub fn take_events(&mut self) -> Vec<PracticeEvent> {
        std::mem::take(&mut self.events)
    }
}

/// The text a printable key types on the US layout when the platform did not say.
fn typed_text(key: &Key) -> Option<String> {
    if key.code == code::SPACE {
        return Some(" ".to_owned());
    }
    let c = char::from_u32(u32::try_from(key.code).ok()?).filter(char::is_ascii_graphic)?;
    let shift = key.modifiers & modifier::SHIFT != 0;
    Some(if c.is_ascii_alphabetic() && !shift { c.to_ascii_lowercase() } else { c }.to_string())
}

//! Practice sessions (macOS `UI/Wizard/Practice/`, Qt `Practice`): the 介紹 try-it field and the setup wizard's
//! practice tasks.  A sandboxed input controller of its own, on an in-memory copy of the user's settings and keys, with
//! in-memory personal data on the shared engine data, typing into a single-line text of its own.  Nothing reaches the
//! user's settings, keys, words, statistics or correction log.

use crate::keys::Key;
use crate::presentation::Presentation;
use crate::service::PopupAction;
use crate::settings::Settings;
use crate::shortcuts::Shortcuts;

/// What the field reports for a task's check (macOS `PracticeOutcome`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PracticeOutcome {
    /// The text sent into the field.
    pub sent: String,
    /// The field's text as shown (sent text with the composition at the insertion point).
    pub text: String,
    /// What happened: `open`, `page`, `pick`, `correct`, `corrections`, `mode`, `phrase` (in order).
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

/// What the field asks its view to show.
#[derive(Clone, Debug, PartialEq)]
pub enum PracticeEvent {
    /// The mode toast (中/英).
    ModeToast { english: bool },
    /// A page turn or highlight move stopped at the list's end.
    Boundary(i32),
}

/// One practice field.
pub struct PracticeSession {
    settings: Settings,
    shortcuts: Shortcuts,
}

impl PracticeSession {
    /// The field's own settings (an in-memory copy the wizard's choices change).
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The field's own keys (on its settings copy).
    #[must_use]
    pub fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// The candidate window and composition, as every view draws them.
    #[must_use]
    pub fn presentation(&self) -> Presentation {
        todo!()
    }

    /// The text sent into the field.
    #[must_use]
    pub fn text(&self) -> &str {
        todo!()
    }

    /// The field as shown: the sent text with the composition at the insertion point.
    #[must_use]
    pub fn shown(&self) -> String {
        todo!()
    }

    /// The insertion point in [`PracticeSession::text`] (UTF-16), where the composition sits.
    #[must_use]
    pub fn insertion(&self) -> usize {
        todo!()
    }

    /// Keys type English.
    #[must_use]
    pub fn english(&self) -> bool {
        todo!()
    }

    /// A key went down; keys the input method leaves to the app are typed into the field as a single-line text field
    /// takes them.  False: neither used it (Tab with nothing composed), so it goes on to the window.
    pub fn key_down(&mut self, _key: &Key) -> bool {
        todo!()
    }

    /// A key went up (Shift taps).
    pub fn key_up(&mut self, _key: &Key) {
        todo!()
    }

    /// A click or control in the field's candidate window.
    pub fn popup_action(&mut self, _action: PopupAction) {
        todo!()
    }

    /// Puts the insertion point at `offset` (UTF-16; a click in the field).  The composition is sent first.
    pub fn move_insertion(&mut self, _offset: usize) {
        todo!()
    }

    /// The field lost focus: the composition is sent, as in an app.
    pub fn end_session(&mut self) {
        todo!()
    }

    /// Starts over with `initial` text and no composition (a new task).
    pub fn reset(&mut self, _initial: &str) {
        todo!()
    }

    /// Takes the user's settings and keys again (a preset chosen again).
    pub fn restart(&mut self) {
        todo!()
    }

    /// What the field did so far.
    #[must_use]
    pub fn outcome(&self) -> &PracticeOutcome {
        todo!()
    }

    /// User phrases added while practising, (word, reading), for the wizard to offer to keep.
    #[must_use]
    pub fn added_phrases(&self) -> Vec<(String, String)> {
        todo!()
    }

    /// What the view should show since the last call.
    pub fn take_events(&mut self) -> Vec<PracticeEvent> {
        todo!()
    }
}

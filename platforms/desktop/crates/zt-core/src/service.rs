//! The service (`ARCHITECTURE.md` §Threads): the shared state — settings, the engine session, the input controller,
//! shortcuts, usage statistics, the correction log — and the broker that serves the native adapters.  The broker runs
//! on its own threads and never waits for the UI; the UI receives [`UiEvent`]s on an async channel and calls the
//! service directly for settings, personal data, statistics and candidate-window clicks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;

use crate::corrections::CorrectionLog;
use crate::engine::{DataFiles, Phrase};
use crate::practice::PracticeSession;
use crate::presentation::{Presentation, Rect};
use crate::settings::Settings;
use crate::shortcuts::Shortcuts;
use crate::stats::UsageStats;

/// How to start the service.
#[derive(Clone, Debug, Default)]
pub struct ServiceConfig {
    /// The engine data's directory (`--data`); see [`DataFiles::search_path`] for the order of places looked in.
    pub data_dir: Option<PathBuf>,
    /// Personal data, statistics and the correction log (default: [`crate::paths::data_dir`]).
    pub user_dir: Option<PathBuf>,
    /// The settings file (default: [`crate::paths::settings_file`]); `None` with `in_memory`.
    pub settings_file: Option<PathBuf>,
    /// Keep settings and personal data in memory only (renders, tests).
    pub in_memory: bool,
    /// The broker endpoint (default: `ZT_IME_SOCKET`, else the per-user socket or pipe; `IPC.md`).
    pub endpoint: Option<String>,
    /// Serve the native adapters.  Off for renders and tests that only need the UI's side.
    pub broker: bool,
    /// The app's version, recorded in correction records.
    pub app_version: String,
}

/// The candidate window's content and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupState {
    pub presentation: Presentation,
    /// The caret in screen logical pixels.
    pub caret: Rect,
    /// The app shows the composition itself; false: the window shows it above the list.
    pub inline_preedit: bool,
}

/// What the UI is asked to do.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// Show, update (`Some`) or hide (`None`) the candidate window.
    Popup(Option<PopupState>),
    /// Show 中 or 英 beside the caret (only when 「中英提示」 is on).
    ModeToast { english: bool, anchor: Rect },
    /// Another start of the app asked for the settings window.
    ShowSettings,
    /// A page turn or highlight move stopped at the list's end (1 past the last, -1 before the first).
    Boundary(i32),
}

/// A click or control in the candidate window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupAction {
    /// Picks row `n` of the current page.
    Pick(usize),
    /// Turns one page (1 next, -1 previous).
    TurnPage(i32),
    /// Moves the highlight one step (1 forward, -1 back).
    MoveSelection(i32),
    /// The 只列一個字 switch.
    ToggleSingle,
    /// The 列出修正的字 switch.
    ToggleCorrections,
    /// Types the highlighted span's Zhuyin (the footer's ` action).
    TypeReading,
}

/// The service.  Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct Service {
    inner: Arc<Shared>,
}

struct Shared {
    settings: Settings,
    shortcuts: Shortcuts,
    stats: UsageStats,
    corrections: CorrectionLog,
}

/// Returned by [`Service::start`] when another instance serves this user; it was asked to show its settings.
#[derive(Debug)]
pub struct AlreadyRunning;

impl std::fmt::Display for AlreadyRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("輸入法服務已在執行。")
    }
}

impl std::error::Error for AlreadyRunning {}

impl Service {
    /// Opens the settings and data, loads the engine and (with `config.broker`) starts serving.
    ///
    /// # Errors
    /// [`AlreadyRunning`] when another instance serves this user; otherwise when the endpoint cannot be reserved.
    /// A missing lexicon is not an error: the service runs without an engine and [`Service::engine_error`] says why.
    pub fn start(_config: ServiceConfig) -> Result<(Service, async_channel::Receiver<UiEvent>)> {
        todo!()
    }

    /// A click or control in the candidate window.
    pub fn popup_action(&self, _action: PopupAction) {
        todo!()
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    #[must_use]
    pub fn shortcuts(&self) -> &Shortcuts {
        &self.inner.shortcuts
    }

    #[must_use]
    pub fn stats(&self) -> &UsageStats {
        &self.inner.stats
    }

    #[must_use]
    pub fn corrections(&self) -> &CorrectionLog {
        &self.inner.corrections
    }

    /// The engine data in use, if a lexicon was found.
    #[must_use]
    pub fn data_files(&self) -> Option<DataFiles> {
        todo!()
    }

    /// Why the engine is not running, if it is not.
    #[must_use]
    pub fn engine_error(&self) -> Option<String> {
        todo!()
    }

    /// How long the engine took to load, in ms.
    #[must_use]
    pub fn load_ms(&self) -> Option<f64> {
        todo!()
    }

    /// A new practice field (the 介紹 try-it field, a wizard step), sharing the practice personal data with every
    /// other practice field open at the same time.
    ///
    /// # Errors
    /// When the engine data is missing or cannot be opened.
    pub fn practice(&self) -> Result<PracticeSession> {
        todo!()
    }

    /// 在候選字視窗顯示組字的 App: their composition shows in the candidate window (executable names).
    #[must_use]
    pub fn popup_composition_apps(&self) -> Vec<String> {
        todo!()
    }

    /// Replaces that list.
    ///
    /// # Errors
    /// When it cannot be saved.
    pub fn set_popup_composition_apps(&self, _apps: &[String]) -> Result<()> {
        todo!()
    }

    // MARK: Personal data (the engine session's, under the service's lock)

    /// Increases whenever personal data may have changed.
    #[must_use]
    pub fn personal_revision(&self) -> u64 {
        todo!()
    }

    #[must_use]
    pub fn user_phrases(&self) -> Vec<Phrase> {
        todo!()
    }

    #[must_use]
    pub fn learned_entries(&self) -> Vec<Phrase> {
        todo!()
    }

    #[must_use]
    pub fn excluded_entries(&self) -> Vec<Phrase> {
        todo!()
    }

    /// The readings of `word` the lexicon knows.
    #[must_use]
    pub fn readings(&self, _word: &str) -> Vec<String> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::add_phrase`].
    pub fn add_phrase(&self, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::edit_phrase`].
    pub fn edit_phrase(&self, _old_word: &str, _old_reading: &str, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::remove_phrase`].
    pub fn remove_phrase(&self, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::forget_all_learned`].
    pub fn forget_all_learned(&self) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::forget_learned`].
    pub fn forget_learned(&self, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::exclude_learned`].
    pub fn exclude_learned(&self, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::include_learned`].
    pub fn include_learned(&self, _word: &str, _reading: &str) -> Result<()> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::import_personal`].
    pub fn import_personal(&self, _path: &Path) -> Result<usize> {
        todo!()
    }

    /// # Errors
    /// See [`crate::engine::EngineSession::export_personal`].
    pub fn export_personal(&self, _path: &Path) -> Result<()> {
        todo!()
    }

    /// Writes personal data and statistics now (the settings window opening, the app quitting).
    pub fn flush(&self) {
        todo!()
    }

    /// Stops serving and writes everything (the app quitting).
    pub fn shutdown(&self) {
        todo!()
    }
}

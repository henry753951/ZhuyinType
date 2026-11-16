//! The service (`ARCHITECTURE.md` §Threads): the shared state — settings, the engine session, the input controller,
//! shortcuts, usage statistics, the correction log — and the broker that serves the native adapters.  The broker runs
//! on its own threads and never waits for the UI; the UI receives [`UiEvent`]s on an async channel and calls the
//! service directly for settings, personal data, statistics and candidate-window clicks.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use serde_json::{Map, Value};
use zhuyin_core::engine::Engine;

use crate::broker::host::{Effect, Host};
use crate::broker::{self, Broker, ConnectionId, Handler, ReserveError};
use crate::controller::{InputController, UsageSink};
use crate::corrections::CorrectionLog;
use crate::engine::{DataFiles, EngineData, EngineSession, Phrase};
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
// The popup state is the common event and sent by value; boxing it would only add an allocation per key.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// Show, update (`Some`) or hide (`None`) the candidate window.
    Popup(Option<PopupState>),
    /// Show 中 or 英 (or 全 or 半, after switching 中文標點's width) beside the caret (only when 「中英提示」 is on).
    Toast { text: &'static str, anchor: Rect },
    /// Another start of the app asked for the settings window.
    ShowSettings(Option<&'static str>),
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

/// The settings key of 在候選字視窗顯示組字的 App (a list of executable names).
pub const POPUP_APPS_KEY: &str = "popupCompositionApps";

/// The service.  Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct Service {
    inner: Arc<Shared>,
}

struct Core {
    controller: InputController,
    host: Host,
    popup: Option<PopupState>,
    /// The candidate window last sent in a reply to an adapter that draws it (unchanged ones are left out).
    sent_panel: Option<Value>,
}

struct Shared {
    settings: Settings,
    shortcuts: Shortcuts,
    stats: UsageStats,
    corrections: CorrectionLog,
    core: Mutex<Core>,
    events: async_channel::Sender<UiEvent>,
    data: Option<Arc<EngineData>>,
    engine_error: Option<String>,
    practice_engine: Mutex<Weak<Engine>>,
    broker: Mutex<Option<Broker>>,
    app_version: String,
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

/// The broker's handler: holds the service weakly, so dropping the last clone ends the service.
struct Serving(Weak<Shared>);

impl Handler for Serving {
    fn request(&self, connection: ConnectionId, request: &Map<String, Value>) -> Map<String, Value> {
        match self.0.upgrade() {
            Some(inner) => Service { inner }.request(connection, request),
            None => Map::from_iter([("ok".to_owned(), false.into()), ("handled".to_owned(), false.into())]),
        }
    }

    fn closed(&self, connection: ConnectionId) {
        if let Some(inner) = self.0.upgrade() {
            let service = Service { inner };
            service.with_core(|core| core.host.disconnected(&mut core.controller, connection));
        }
    }
}

fn popup_apps(settings: &Settings) -> Vec<String> {
    match settings.get(POPUP_APPS_KEY) {
        Value::Array(items) => items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    }
}

impl Service {
    /// Opens the settings and data, loads the engine and (with `config.broker`) starts serving.  The endpoint is
    /// reserved first and served only once the engine is loaded, so the first key never meets an engine that is not
    /// ready.
    ///
    /// # Errors
    /// [`AlreadyRunning`] when another instance serves this user (it was asked to show its settings); otherwise
    /// when the endpoint cannot be reserved or served.  A missing lexicon is not an error: the service runs without
    /// an engine and [`Service::engine_error`] says why.
    pub fn start(config: ServiceConfig) -> Result<(Service, async_channel::Receiver<UiEvent>)> {
        let reservation = if config.broker {
            let endpoint =
                config.endpoint.clone().or_else(broker::endpoint).ok_or_else(|| anyhow!("無法取得使用者通訊端。"))?;
            match broker::reserve(&endpoint) {
                Ok(reservation) => Some(reservation),
                Err(ReserveError::AlreadyRunning) => {
                    let _ = broker::show_settings(&endpoint);
                    return Err(AlreadyRunning.into());
                }
                Err(ReserveError::Failed(error)) => return Err(error),
            }
        } else {
            None
        };
        let settings = if config.in_memory {
            Settings::open(None)
        } else {
            Settings::open(config.settings_file.clone().or_else(crate::paths::settings_file))
        };
        let user_dir = if config.in_memory { None } else { config.user_dir.clone().or_else(crate::paths::data_dir) };
        let shortcuts = Shortcuts::new(settings.clone());
        let stats = UsageStats::open(user_dir.as_ref().map(|dir| dir.join("stats.json")));
        stats.set_enabled(settings.bool("recordsUsageStats"));
        let corrections = CorrectionLog::open(user_dir.as_deref());

        let mut session = EngineSession::new();
        let mut engine_error = None;
        let data = DataFiles::find(config.data_dir.as_deref()).map(EngineData::new);
        match &data {
            Some(data) => {
                if let Err(error) = session.open(data, user_dir.as_deref()) {
                    log::error!("engine: {error:#}");
                    engine_error = Some(format!("{error:#}"));
                }
            }
            None => engine_error = Some("找不到詞庫資料（lexicon.bin 或 lexicon.tsv）。".to_owned()),
        }
        let mut controller = InputController::new(session, settings.clone(), shortcuts.clone());
        controller.refresh_settings();
        controller.pause_threshold = stats.pause_threshold();
        let sink_stats = stats.clone();
        let sink: UsageSink = Arc::new(move |event| sink_stats.record(event));
        controller.set_usage_sink(Some(sink));
        controller.set_correction_log(Some(corrections.clone()), &config.app_version);
        controller.take_events();

        let observed = stats.clone();
        let observed_settings = settings.clone();
        settings.observe(move |key| {
            if key == "recordsUsageStats" {
                observed.set_enabled(observed_settings.bool("recordsUsageStats"));
            }
        });
        let (sender, receiver) = async_channel::unbounded();
        let service = Service {
            inner: Arc::new(Shared {
                settings,
                shortcuts,
                stats,
                corrections,
                core: Mutex::new(Core { controller, host: Host::default(), popup: None, sent_panel: None }),
                events: sender,
                data,
                engine_error,
                practice_engine: Mutex::new(Weak::new()),
                broker: Mutex::new(None),
                app_version: config.app_version.clone(),
            }),
        };
        // A changed setting reaches the composition at once (a list open with the old page size, English mode turned
        // on while composing), not only at the next key.  Nothing sets a setting while holding the service's lock.
        let weak = Arc::downgrade(&service.inner);
        service.inner.settings.observe(move |_| {
            if let Some(inner) = weak.upgrade() {
                Service { inner }.with_controller(InputController::refresh_settings);
            }
        });
        if let Some(reservation) = reservation {
            let broker = reservation.listen(Arc::new(Serving(Arc::downgrade(&service.inner))))?;
            *service.inner.broker.lock() = Some(broker);
        }
        Ok((service, receiver))
    }

    /// The endpoint the broker serves, if it runs.
    #[must_use]
    pub fn endpoint(&self) -> Option<String> {
        self.inner.broker.lock().as_ref().map(|broker| broker.endpoint().to_owned())
    }

    fn with_core<R>(&self, work: impl FnOnce(&mut Core) -> R) -> R {
        let (result, effects, popup) = {
            let mut core = self.inner.core.lock();
            core.host.popup_apps = popup_apps(&self.inner.settings);
            let result = work(&mut core);
            let Core { controller, host, popup, .. } = &mut *core;
            let effects = host.drain(controller);
            // An adapter that draws the window itself gets it in its replies; the app's own window stays hidden.
            let next = (host.visible(controller) && !host.draws_panel()).then(|| PopupState {
                presentation: controller.presentation().clone(),
                caret: host.anchor(),
                inline_preedit: host.inline_preedit(),
            });
            let changed = (*popup != next).then(|| next.clone());
            *popup = next;
            (result, effects, changed)
        };
        self.emit(effects, popup);
        result
    }

    fn emit(&self, effects: Vec<Effect>, popup: Option<Option<PopupState>>) {
        let anchor = self.inner.core.lock().host.anchor();
        for effect in effects {
            let event = match effect {
                Effect::ShowSettings(place) => UiEvent::ShowSettings(place),
                Effect::Boundary(direction) => UiEvent::Boundary(direction),
                Effect::Toast { text } => {
                    if !self.inner.settings.bool("showModeToast") {
                        continue;
                    }
                    UiEvent::Toast { text, anchor }
                }
            };
            let _ = self.inner.events.try_send(event);
        }
        if let Some(popup) = popup {
            let _ = self.inner.events.try_send(UiEvent::Popup(popup));
        }
    }

    /// One broker request (also for tests that drive the service without a socket).
    ///
    /// An adapter that draws the candidate window itself (`"panel": true` at focus) gets the window in the reply's
    /// `panel` (null: hidden; left out: unchanged since its last reply), the 中/英 badge in `toast` and a page turn
    /// that stopped at the list's end in `boundary`, instead of the app showing them (`IPC.md` §候選字視窗).
    #[must_use]
    pub fn request(&self, connection: ConnectionId, request: &Map<String, Value>) -> Map<String, Value> {
        let mut effects = Vec::new();
        let mut draws = false;
        let mut reply = self.with_core(|core| {
            let (mut reply, found) = core.host.dispatch(&mut core.controller, connection, request);
            effects = found;
            draws = core.host.draws_panel() && reply.get("ok") == Some(&Value::Bool(true));
            if draws {
                let panel = if core.host.visible(&core.controller) {
                    let shortcuts = &self.inner.shortcuts;
                    let panel = crate::panel::panel(
                        &self.inner.settings,
                        |command| shortcuts.active_label(command),
                        core.controller.presentation(),
                        core.host.inline_preedit(),
                    );
                    serde_json::to_value(panel).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };
                let focus = request.get("op").and_then(Value::as_str) == Some("focus");
                if focus || core.sent_panel.as_ref() != Some(&panel) {
                    reply.insert("panel".into(), panel.clone());
                    core.sent_panel = Some(panel);
                }
            } else {
                core.sent_panel = None;
            }
            reply
        });
        if draws {
            effects.retain(|effect| match effect {
                Effect::Toast { text } => {
                    if self.inner.settings.bool("showModeToast") {
                        reply.insert("toast".into(), Value::from(*text));
                    }
                    false
                }
                Effect::Boundary(direction) => {
                    reply.insert("boundary".into(), Value::from(*direction));
                    false
                }
                Effect::ShowSettings(_) => true,
            });
        }
        self.emit(effects, None);
        reply
    }

    /// Runs `work` on the input controller under the service's lock (advanced use and tests); the popup follows.
    pub fn with_controller<R>(&self, work: impl FnOnce(&mut InputController) -> R) -> R {
        self.with_core(|core| work(&mut core.controller))
    }

    /// What the composer shows now.
    #[must_use]
    pub fn presentation(&self) -> Presentation {
        self.inner.core.lock().controller.presentation().clone()
    }

    /// The candidate window as last sent in [`UiEvent::Popup`].
    #[must_use]
    pub fn popup(&self) -> Option<PopupState> {
        self.inner.core.lock().popup.clone()
    }

    /// A click or control in the candidate window.
    pub fn popup_action(&self, action: PopupAction) {
        self.with_core(|core| {
            let Core { controller, host, .. } = core;
            match action {
                PopupAction::Pick(row) => host.pick(controller, row),
                PopupAction::TurnPage(delta) => host.turn_page(controller, delta),
                PopupAction::MoveSelection(delta) => host.move_selection(controller, delta),
                PopupAction::ToggleSingle => host.toggle_single(controller),
                PopupAction::ToggleCorrections => host.toggle_corrections(controller),
                PopupAction::TypeReading => host.type_reading(controller),
            }
        });
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

    /// The app's version, as given at start.
    #[must_use]
    pub fn app_version(&self) -> &str {
        &self.inner.app_version
    }

    /// The engine data in use, if a lexicon was found.
    #[must_use]
    pub fn data_files(&self) -> Option<DataFiles> {
        self.inner.data.as_ref().map(|data| data.files().clone())
    }

    /// Why the engine is not running, if it is not.
    #[must_use]
    pub fn engine_error(&self) -> Option<String> {
        self.inner.engine_error.clone()
    }

    /// How long the engine took to load, in ms.
    #[must_use]
    /// How long the neural models took to load (reranker, character model).
    pub fn model_load_ms(&self) -> (Option<f64>, Option<f64>) {
        self.inner.core.lock().controller.session().model_load_ms()
    }

    pub fn load_ms(&self) -> Option<f64> {
        self.inner.core.lock().controller.session().load_ms()
    }

    /// A new practice field (the 介紹 try-it field, a wizard step) on a copy of the user's settings and keys, sharing
    /// the practice personal data (in memory) with every other practice field open at the same time.
    ///
    /// # Errors
    /// When the engine data is missing or cannot be opened.
    pub fn practice(&self) -> Result<PracticeSession> {
        let data = self.inner.data.clone().ok_or_else(|| anyhow!("找不到詞庫資料。"))?;
        let engine = {
            let mut shared = self.inner.practice_engine.lock();
            match shared.upgrade() {
                Some(engine) => engine,
                None => {
                    let engine = Arc::new(data.open_engine(None)?);
                    *shared = Arc::downgrade(&engine);
                    engine
                }
            }
        };
        Ok(PracticeSession::new(&self.inner.settings, &data, engine))
    }

    /// 在候選字視窗顯示組字的 App: their composition shows in the candidate window (executable names).
    #[must_use]
    pub fn popup_composition_apps(&self) -> Vec<String> {
        popup_apps(&self.inner.settings)
    }

    /// Replaces that list.
    ///
    /// # Errors
    /// When it cannot be saved.
    pub fn set_popup_composition_apps(&self, apps: &[String]) -> Result<()> {
        self.inner.settings.set(POPUP_APPS_KEY, Value::from(apps.to_vec()))
    }

    // MARK: Personal data (the engine session's, under the service's lock)

    fn session<R>(&self, work: impl FnOnce(&mut EngineSession) -> R) -> R {
        work(self.inner.core.lock().controller.session_mut())
    }

    /// Increases whenever personal data may have changed.
    #[must_use]
    pub fn personal_revision(&self) -> u64 {
        self.session(|s| s.personal_revision())
    }

    #[must_use]
    pub fn user_phrases(&self) -> Vec<Phrase> {
        self.session(|s| s.user_phrases())
    }

    #[must_use]
    pub fn learned_entries(&self) -> Vec<Phrase> {
        self.session(|s| s.learned_entries())
    }

    #[must_use]
    pub fn excluded_entries(&self) -> Vec<Phrase> {
        self.session(|s| s.excluded_entries())
    }

    /// The readings of `word` the lexicon knows.
    #[must_use]
    pub fn readings(&self, word: &str) -> Vec<String> {
        self.session(|s| s.readings(word))
    }

    /// # Errors
    /// See [`EngineSession::add_phrase`].
    pub fn add_phrase(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.add_phrase(word, reading))
    }

    /// # Errors
    /// See [`EngineSession::edit_phrase`].
    pub fn edit_phrase(&self, old_word: &str, old_reading: &str, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.edit_phrase(old_word, old_reading, word, reading))
    }

    /// # Errors
    /// See [`EngineSession::remove_phrase`].
    pub fn remove_phrase(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.remove_phrase(word, reading))
    }

    /// # Errors
    /// See [`EngineSession::forget_all_learned`].
    pub fn forget_all_learned(&self) -> Result<()> {
        self.session(EngineSession::forget_all_learned)
    }

    /// # Errors
    /// See [`EngineSession::forget_learned`].
    pub fn forget_learned(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.forget_learned(word, reading))
    }

    /// # Errors
    /// See [`EngineSession::exclude_learned`].
    pub fn exclude_learned(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.exclude_learned(word, reading))
    }

    /// # Errors
    /// See [`EngineSession::include_learned`].
    pub fn include_learned(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.include_learned(word, reading))
    }

    /// # Errors
    /// See [`EngineSession::import_personal`].
    pub fn import_personal(&self, path: &Path) -> Result<usize> {
        self.session(|s| s.import_personal(path))
    }

    /// # Errors
    /// See [`EngineSession::export_personal`].
    pub fn export_personal(&self, path: &Path) -> Result<()> {
        self.session(|s| s.export_personal(path))
    }

    /// Writes personal data and statistics now (the settings window opening, the app quitting).
    pub fn flush(&self) {
        self.session(|s| {
            if s.ready()
                && let Err(error) = s.flush()
            {
                log::warn!("{error}");
            }
        });
        if let Err(error) = self.inner.stats.save_if_needed() {
            log::warn!("{error}");
        }
    }

    /// Stops serving and writes everything (the app quitting).
    pub fn shutdown(&self) {
        self.inner.broker.lock().take();
        self.with_core(|core| core.controller.deactivate());
        self.flush();
    }
}

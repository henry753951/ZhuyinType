//! The service (`ARCHITECTURE.md` §Threads): the shared state — settings, the engine session, the input controller,
//! shortcuts, usage statistics, the correction log — and the broker that serves the native adapters.  The broker runs
//! on its own threads and never waits for the UI; the UI receives [`UiEvent`]s on an async channel and calls the
//! service directly for settings, personal data, statistics and candidate-window clicks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use serde_json::{Map, Value};
use zhuyin_core::composer::CandidateAction;
use zhuyin_core::engine::Engine;

use crate::broker::host::{Effect, Host};
use crate::broker::{self, Broker, ConnectionId, Handler, ReserveError};
use crate::controller::{InputController, UsageSink};
use crate::corrections::CorrectionLog;
use crate::engine::{DataFiles, EngineData, EngineSession, Phrase};
use crate::phrase_draft::PhraseDraft;
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
    /// When another instance already serves this user, end without asking it for its settings window
    /// (`zhuyintype --background`: sign-in and the Fcitx5 addon start it, and must never open a window).
    pub quiet_if_running: bool,
    /// The app's version, recorded in correction records.
    pub app_version: String,
    /// Another copy may hold the endpoint while it ends (this one was started after it crashed): wait this long for
    /// it to go instead of asking it for its settings window.
    pub wait_for_endpoint: Option<std::time::Duration>,
}

/// The candidate window's content and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupState {
    pub presentation: Presentation,
    /// The caret in screen logical pixels.
    pub caret: Rect,
    /// The app shows the composition itself; false: the window shows it above the list.
    pub inline_preedit: bool,
    /// 符號與 Emoji is open: the window shows its grid instead of `presentation` (its highlighted row as a list).
    pub picker: Option<crate::symbols::PickerView>,
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
    /// The 開啟快速設定 shortcut: show the 快速設定 panel by the caret (screen logical pixels), without taking focus.
    QuickPanel { anchor: Rect },
    /// The next key, or the field changed: the 快速設定 panel closes.
    HideQuickPanel,
    /// 選取的字加入自訂詞: open the 加入自訂詞 form (字詞 › 自訂詞) filled in with the field's selected text.
    AddPhrase(PhraseDraft),
}

/// The 快速設定 switches as they are now (IPC `quick`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuickSwitches {
    pub english: bool,
    pub full_width: bool,
    pub simplified: bool,
    pub learning_paused: bool,
}

/// One check item of 快速設定.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickSwitch {
    /// 英文.
    English,
    /// 半形標點.
    HalfWidth,
    /// 簡體.
    Simplified,
    /// 暫停學習.
    PauseLearning,
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
    /// A word action (不要再推薦, 忘記這個選字, 加入自訂詞) on row `n` of the current page (the right-click menu).
    Word(CandidateAction, usize),
    /// The notice's 「復原」: undoes the last word action.
    UndoWordAction,
    /// Inserts item `n` of the symbol picker's list.
    PickSymbol(usize),
    /// Switches the symbol picker to tab `n`.
    SymbolTab(usize),
}

/// Writes the personal data on a thread of its own: a blur's reply goes out at once and the service's lock is not
/// held while the disk works.  Written all the same (the engine's writer also writes 150 ms after any change, and
/// the app writes everything when it quits).
fn flush_in_background(engine: Arc<Engine>) {
    fn write(engine: &Engine) {
        if let Err(error) = engine.flush_personal_data() {
            log::warn!("blur: personal data not written: {error}");
        }
    }
    let worker = engine.clone();
    if std::thread::Builder::new().name("zt-flush".into()).spawn(move || write(&worker)).is_err() {
        // No thread to spare: write here rather than not at all.
        write(&engine);
    }
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
    /// The app shows the 快速設定 panel ([`UiEvent::QuickPanel`]) until the next key.
    quick_open: AtomicBool,
    /// Uptime, the last key and the adapters connected, for 回報問題 (`health::activity`).
    activity: crate::health::activity::Activity,
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
            Some(inner) => {
                inner.activity.seen(connection);
                Service { inner }.request(connection, request)
            }
            None => Map::from_iter([("ok".to_owned(), false.into()), ("handled".to_owned(), false.into())]),
        }
    }

    fn closed(&self, connection: ConnectionId) {
        if let Some(inner) = self.0.upgrade() {
            inner.activity.closed(connection);
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
    /// [`AlreadyRunning`] when another instance serves this user (it was asked to show its settings, unless
    /// `quiet_if_running` or `wait_for_endpoint`); otherwise when the endpoint cannot be reserved or served.  A
    /// missing lexicon is not an error: the service runs without an engine and [`Service::engine_error`] says why.
    pub fn start(config: ServiceConfig) -> Result<(Service, async_channel::Receiver<UiEvent>)> {
        let reservation = if config.broker {
            let endpoint =
                config.endpoint.clone().or_else(broker::endpoint).ok_or_else(|| anyhow!("無法取得使用者通訊端。"))?;
            let deadline = config.wait_for_endpoint.map(|wait| std::time::Instant::now() + wait);
            loop {
                match broker::reserve(&endpoint) {
                    Ok(reservation) => break Some(reservation),
                    Err(ReserveError::AlreadyRunning)
                        if deadline.is_some_and(|deadline| std::time::Instant::now() < deadline) =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(ReserveError::AlreadyRunning) => {
                        if deadline.is_none() && !config.quiet_if_running {
                            let _ = broker::show_settings(&endpoint);
                        }
                        return Err(AlreadyRunning.into());
                    }
                    Err(ReserveError::Failed(error)) => return Err(error),
                }
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
                data.load_models_in_background();
                if let Err(error) = session.open(data, user_dir.as_deref()) {
                    log::error!("engine: {error:#}");
                    engine_error = Some(format!("{error:#}"));
                }
            }
            None => engine_error = Some("找不到詞庫資料（lexicon.bin 或 lexicon.tsv）。".to_owned()),
        }
        let mut controller = InputController::new(session, settings.clone(), shortcuts.clone());
        controller.set_recent_symbols_dir(user_dir.as_deref());
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
                quick_open: AtomicBool::new(false),
                activity: crate::health::activity::Activity::default(),
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
            controller.sync_word_overlay();
            let mut effects = host.drain(controller);
            if host.adapter_draws_panel() {
                // Its adapter shows the badge and the list's bounce itself, in the field's application; the app
                // drawing them too would show two.  The word actions' tip is only counted, never shown.
                effects.retain(|effect| {
                    matches!(effect, Effect::ShowSettings(_) | Effect::WordActionTip | Effect::AddSelectedText(_))
                });
            }
            // An adapter that draws the window itself gets it in its replies; the app's own window stays hidden.
            let next = (host.visible(controller) && !host.draws_panel()).then(|| PopupState {
                presentation: controller.output_presentation().into_owned(),
                caret: host.anchor(),
                inline_preedit: host.inline_preedit(),
                picker: controller.picker_view(),
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
                // Only an adapter's reply can ask it for the selection.
                Effect::SelectionWanted => continue,
                Effect::AddSelectedText(text) => {
                    let added = |word: &str, reading: &str| {
                        self.user_phrases().iter().any(|p| p.word == word && p.reading == reading)
                    };
                    UiEvent::AddPhrase(crate::phrase_draft::draft(&text, |word| self.readings(word), added))
                }
                Effect::WordActionTip => {
                    // Outside the service's lock: setting a value calls the service's settings observer.
                    let key = crate::controller::WORD_ACTION_TIP_KEY;
                    let count = self.inner.settings.number(key).max(0.0) + 1.0;
                    if let Err(error) = self.inner.settings.set(key, Value::from(count as i64)) {
                        log::warn!("{key}: {error}");
                    }
                    continue;
                }
                Effect::QuickPanel => {
                    self.inner.quick_open.store(true, Ordering::Release);
                    UiEvent::QuickPanel { anchor }
                }
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
        // The panel the app shows closes at the next key or when the field changes.
        let op = request.get("op").and_then(Value::as_str).unwrap_or_default();
        let press = op == "key" && request.get("release") == Some(&Value::Bool(false));
        if op == "diagnostics" {
            return self.diagnostics().reply();
        }
        if press {
            self.inner.activity.key();
        }
        if (press || matches!(op, "focus" | "blur" | "secure" | "reset"))
            && self.inner.quick_open.swap(false, Ordering::AcqRel)
        {
            let _ = self.inner.events.try_send(UiEvent::HideQuickPanel);
        }
        let mut effects = Vec::new();
        let mut draws = false;
        let mut own_badge = false;
        let mut flush = None;
        let context = request.get("context").and_then(Value::as_str).unwrap_or_default();
        let mut script = None;
        let mut reply = self.with_core(|core| {
            let (mut reply, found) = core.host.dispatch(&mut core.controller, connection, request);
            core.controller.sync_word_overlay();
            effects = found;
            effects.extend(core.host.drain(&mut core.controller));
            if core.host.take_flush() {
                flush = core.controller.session().engine().cloned();
            }
            own_badge = core.host.adapter_draws_panel();
            script = core.host.take_pending_script();
            draws = core.host.draws_panel()
                && core.host.owns(connection, context)
                && reply.get("ok") == Some(&Value::Bool(true));
            if draws {
                let panel = self.panel_value(core);
                let focus = request.get("op").and_then(Value::as_str) == Some("focus");
                if focus && core.host.panel_for_screen_reader() {
                    reply.insert("fcitxPanel".into(), Value::Bool(true));
                }
                if focus || core.sent_panel.as_ref() != Some(&panel) {
                    reply.insert("panel".into(), panel.clone());
                    core.sent_panel = Some(panel);
                }
            } else {
                core.sent_panel = None;
            }
            reply
        });
        // 選取的字加入自訂詞: the adapter sends the field's selected text next (op `selection`).
        effects.retain(|effect| {
            let wanted = *effect == Effect::SelectionWanted;
            if wanted {
                reply.insert("selection".into(), Value::Bool(true));
            }
            !wanted
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
                // The adapter shows its own quick menu (the reply's `quickPanel`).
                Effect::QuickPanel => false,
                Effect::ShowSettings(_) | Effect::WordActionTip | Effect::AddSelectedText(_) => true,
                Effect::SelectionWanted => false,
            });
        } else if own_badge {
            // A blur's, say: that adapter draws its badge itself, never the app (the tip is only counted).
            effects.retain(|effect| {
                matches!(effect, Effect::ShowSettings(_) | Effect::WordActionTip | Effect::AddSelectedText(_))
            });
        }
        if let Some(simplified) = script {
            self.set_script(simplified, connection, context, draws, &mut reply);
        }
        self.emit(effects, None);
        if let Some(engine) = flush {
            flush_in_background(engine);
        }
        reply
    }

    /// The candidate window for an adapter that draws it (`IPC.md` §候選字視窗), or null when hidden.
    fn panel_value(&self, core: &Core) -> Value {
        if !core.host.visible(&core.controller) {
            return Value::Null;
        }
        let shortcuts = &self.inner.shortcuts;
        let mut panel = crate::panel::panel(
            &self.inner.settings,
            |command| shortcuts.active_label(command),
            &core.controller.output_presentation(),
            core.host.inline_preedit(),
        );
        if core.controller.picker_open() {
            // 符號與 Emoji: the grid, which the text service draws; and for one that does not know it, the
            // highlighted row of the grid, across like the grid's rows.
            panel.style.layout = zt_panel::style::Layout::Horizontal;
            panel.style.shows_list_switches = false;
            panel.picker = core.controller.picker_view();
        }
        serde_json::to_value(panel).unwrap_or(Value::Null)
    }

    /// 繁／簡 from `op: "quick"`: the `outputScript` setting, set after the request with the service's lock released
    /// (the settings' observer takes it); the requesting field's composition and window in `reply` follow.
    fn set_script(
        &self,
        simplified: bool,
        connection: ConnectionId,
        context: &str,
        draws: bool,
        reply: &mut Map<String, Value>,
    ) {
        let script = if simplified { "simplified" } else { "traditional" };
        if let Err(error) = self.inner.settings.set("outputScript", script.into()) {
            log::warn!("quick: {error}");
        }
        let mut core = self.inner.core.lock();
        if !core.host.owns(connection, context) {
            return;
        }
        // The conversion keeps the length, so `cursor` and `clauses` stay valid.
        if reply.get("preedit").and_then(Value::as_str).is_some_and(|preedit| !preedit.is_empty()) {
            let preedit = core.controller.output_presentation().composition.clone();
            reply.insert("preedit".into(), preedit.into());
        }
        if draws {
            let panel = self.panel_value(&core);
            reply.insert("panel".into(), panel.clone());
            core.sent_panel = Some(panel);
        }
    }

    /// The 快速設定 switches now (for the app's panel).
    #[must_use]
    pub fn quick_switches(&self) -> QuickSwitches {
        let core = self.inner.core.lock();
        let c = &core.controller;
        QuickSwitches {
            english: c.english(),
            full_width: c.full_width(),
            simplified: c.simplified(),
            learning_paused: c.learning_paused(),
        }
    }

    /// Flips one 快速設定 check item (a click in the app's panel), as `op: "quick"` would.
    pub fn toggle_quick(&self, switch: QuickSwitch) {
        self.inner.quick_open.store(false, Ordering::Release);
        if switch == QuickSwitch::Simplified {
            let script = if self.quick_switches().simplified { "traditional" } else { "simplified" };
            if let Err(error) = self.inner.settings.set("outputScript", script.into()) {
                log::warn!("quick: {error}");
            }
            return;
        }
        self.with_controller(|c| match switch {
            QuickSwitch::English => c.set_english(!c.english()),
            QuickSwitch::HalfWidth => c.set_full_width(!c.full_width()),
            QuickSwitch::PauseLearning => c.set_learning_paused(!c.learning_paused()),
            QuickSwitch::Simplified => {}
        });
    }

    /// The app closed its 快速設定 panel (a click in it, or Esc).
    pub fn quick_panel_closed(&self) {
        self.inner.quick_open.store(false, Ordering::Release);
    }

    /// Runs `work` on the input controller under the service's lock (advanced use and tests); the popup follows.
    pub fn with_controller<R>(&self, work: impl FnOnce(&mut InputController) -> R) -> R {
        self.with_core(|core| work(&mut core.controller))
    }

    /// What the composer shows now.
    /// Forgets the recent symbols of 符號與 Emoji and removes their file (回復預設設定, deleting personal data).
    pub fn clear_recent_symbols(&self) {
        self.with_controller(InputController::clear_recent_symbols);
    }

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
                PopupAction::Word(action, row) => host.word_action(controller, action, row),
                PopupAction::UndoWordAction => host.undo_word_action(controller),
                PopupAction::PickSymbol(index) => {
                    if host.visible(controller) {
                        controller.pick_symbol(index);
                    }
                }
                PopupAction::SymbolTab(index) => controller.select_symbol_tab(index),
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

    /// Uptime, the last key, the adapters connected and the recent warnings and errors (回報問題; the broker's
    /// `diagnostics`).
    #[must_use]
    pub fn diagnostics(&self) -> crate::health::activity::Diagnostics {
        self.inner.activity.diagnostics(&self.inner.app_version)
    }

    /// Why the neural models are missing although their files are there (ONNX Runtime not found, a model that
    /// failed to load), once they have been loaded.
    #[must_use]
    pub fn model_error(&self) -> Option<String> {
        self.inner.data.as_ref().and_then(|data| data.loaded_models()).and_then(|models| models.error.clone())
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

    /// Words never suggested (不要再推薦).
    #[must_use]
    pub fn blocked_entries(&self) -> Vec<Phrase> {
        self.session(|s| s.blocked_entries())
    }

    /// # Errors
    /// See [`EngineSession::unblock`].
    pub fn unblock(&self, word: &str, reading: &str) -> Result<()> {
        self.session(|s| s.unblock(word, reading))
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

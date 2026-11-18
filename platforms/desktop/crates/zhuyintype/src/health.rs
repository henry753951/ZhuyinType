//! 檢查輸入法 in the app: runs `zt_core::health` off the UI thread when the settings window opens or becomes active
//! again, keeps the results for 關於 and the sidebar's badge, and carries out a check's fix.

use std::collections::HashSet;
use std::path::PathBuf;

use gpui_kit::{App, Global};
use zt_core::health::common::{DataFacts, ServiceFacts, SettingsFacts, UpdateFacts};
use zt_core::health::details::{AppFacts, DisplayFacts, ModelFacts};
use zt_core::health::report::{Report, Section};
use zt_core::health::{self, Check, Fix, Inputs};
use zt_core::update::{CHECKS_KEY, Found};

use crate::app;
use crate::theme::{self, Material};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The last results and what is running.
#[derive(Default)]
pub struct Health {
    /// None until the first run has finished.
    pub checks: Option<Vec<Check>>,
    /// The report's other sections from the same run.
    pub sections: Vec<Section>,
    /// The checks whose 詳細資料 are shown.
    pub open: HashSet<&'static str>,
    pub running: bool,
    /// The id of the check whose fix is running.
    pub fixing: Option<&'static str>,
    /// What the last fix said when it failed.
    pub fix_error: Option<String>,
}

impl Global for Health {}

/// The results, if any run has finished.
pub fn checks(cx: &App) -> Option<&[Check]> {
    cx.try_global::<Health>()?.checks.as_deref()
}

/// Whether any check needs attention (the sidebar's badge on 關於).
pub fn has_problems(cx: &App) -> bool {
    checks(cx).is_some_and(|checks| health::problems(checks) > 0)
}

/// Shows or hides check `id`'s 詳細資料.
pub fn toggle(id: &'static str, cx: &mut App) {
    if cx.has_global::<Health>() {
        let health = cx.global_mut::<Health>();
        if !health.open.remove(id) {
            health.open.insert(id);
        }
    }
    cx.refresh_windows();
}

/// 「2026-11-18 13:00」.
fn minute(time: &chrono::DateTime<chrono::Local>) -> String {
    time.format("%Y-%m-%d %H:%M").to_string()
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The display and accessibility settings as the windows see them (`scale`: the settings window's).
fn display(scale: Option<f32>, cx: &App) -> DisplayFacts {
    let state = cx.global::<app::AppState>();
    let p = theme::palette(cx);
    let shade = if p.dark { "深色" } else { "淺色" };
    let theme = match zt_core::appearance::theme(&state.settings) {
        zt_core::appearance::Choice::System => format!("跟隨系統（{shade}）"),
        zt_core::appearance::Choice::Light => "淺色".to_owned(),
        zt_core::appearance::Choice::Dark => "深色".to_owned(),
    };
    let transparency = match (zt_core::appearance::transparency(&state.settings), p.material) {
        (false, _) => "關閉".to_owned(),
        (true, Material::Mica) => "開啟（Mica）".to_owned(),
        (true, Material::Blur) => "開啟（模糊）".to_owned(),
        (true, Material::Opaque) => "開啟（這個系統不支援，視窗不透明）".to_owned(),
    };
    DisplayFacts {
        scale,
        theme: Some(theme),
        transparency: Some(transparency),
        screen_reader: Some(zt_core::system_look::screen_reader_if_known()),
        reduced_motion: Some(crate::motion::reduced(cx)),
        high_contrast: Some(zt_core::system_look::look(p.dark).high_contrast.is_some()),
    }
}

/// What the app knows itself, for a run.
fn inputs(cx: &App) -> Inputs {
    let state = cx.global::<app::AppState>();
    let settings = &state.settings;
    let service = state.service.as_ref();
    let (reranker_ms, character_ms) = service.map_or((None, None), zt_core::service::Service::model_load_ms);
    let model_error = service.and_then(zt_core::service::Service::model_error);
    let last_check = cx.try_global::<crate::updates::Updates>().and_then(|updates| updates.last_check);
    let about = AppFacts {
        diagnostics: service.map(zt_core::service::Service::diagnostics),
        response_ms: None,
        models: ModelFacts {
            engine_ms: service.and_then(zt_core::service::Service::load_ms),
            reranker_ms,
            character_ms,
            error: model_error.clone(),
        },
        stored_settings: settings.stored(),
        user_data: zt_core::paths::data_dir(),
        executable: std::env::current_exe().ok(),
        onnx_runtime: zt_core::models::runtime_path(),
        display: display(None, cx),
        automatic_updates: settings.bool(CHECKS_KEY),
        last_update_check: last_check.as_ref().map(minute),
        now: now_unix(),
    };
    let updates = match cx.try_global::<crate::updates::Updates>() {
        _ if !settings.bool(CHECKS_KEY) => UpdateFacts::ChecksOff,
        Some(updates) => match (&updates.error, &updates.found) {
            (Some(error), _) => UpdateFacts::Failed(error.clone()),
            (None, Some(Found::Newer { version, .. })) => UpdateFacts::Newer(version.clone()),
            (None, Some(Found::Current)) => UpdateFacts::Current,
            (None, Some(Found::Unsupported)) => UpdateFacts::Unsupported,
            (None, None) => UpdateFacts::Unknown,
        },
        None => UpdateFacts::Unknown,
    };
    Inputs {
        version: VERSION.to_owned(),
        data: DataFacts {
            files: service.and_then(zt_core::service::Service::data_files),
            lexicon_readable: false,
            engine_error: service.and_then(zt_core::service::Service::engine_error),
            model_error,
        },
        settings: SettingsFacts::probe(settings.path().map(PathBuf::from), settings.unreadable()),
        service: ServiceFacts { running: service.is_some(), error: state.service_error.clone(), reachable: None },
        endpoint: service.and_then(zt_core::service::Service::endpoint),
        updates,
        fcitx_helper: fcitx_helper(),
        about,
    }
}

#[cfg(target_os = "linux")]
fn fcitx_helper() -> Option<PathBuf> {
    crate::platform::linux::fcitx_helper()
}

#[cfg(not(target_os = "linux"))]
fn fcitx_helper() -> Option<PathBuf> {
    None
}

/// Runs the checks again, off the UI thread (once at a time).
pub fn refresh(cx: &mut App) {
    if !cx.has_global::<app::AppState>() {
        return;
    }
    if !cx.has_global::<Health>() {
        cx.set_global(Health::default());
    }
    if std::mem::replace(&mut cx.global_mut::<Health>().running, true) {
        return;
    }
    let mut inputs = inputs(cx);
    let task = cx.background_executor().spawn(async move {
        let model_error = inputs.data.model_error.take();
        inputs.data = DataFacts::probe(inputs.data.files.take(), inputs.data.engine_error.take());
        inputs.data.model_error = model_error;
        health::run(&inputs)
    });
    cx.spawn(async move |cx| {
        let outcome = task.await;
        cx.update(|cx| {
            let state = cx.global_mut::<Health>();
            state.running = false;
            state.checks = Some(outcome.checks);
            state.sections = outcome.sections;
            cx.refresh_windows();
            screenshot_hook(cx);
        });
    })
    .detach();
}

/// Screenshots (`scripts/screenshot.sh about/health`): `ZT_SHOT_HEALTH=details` opens every check's 詳細資料,
/// `report` opens 回報問題 over the window, once, after the first run.
fn screenshot_hook(cx: &mut App) {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Ok(wanted) = std::env::var("ZT_SHOT_HEALTH") else { return };
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let wanted: Vec<&str> = wanted.split(',').map(str::trim).collect();
    if wanted.contains(&"details") {
        let ids: Vec<&'static str> = checks(cx)
            .unwrap_or_default()
            .iter()
            .filter(|check| !check.details.is_empty())
            .map(|check| check.id)
            .collect();
        cx.global_mut::<Health>().open.extend(ids);
        cx.refresh_windows();
    }
    if wanted.contains(&"report")
        && let Some(window) = cx.windows().first().copied()
    {
        window.update(cx, |_, window, cx| crate::pages::about::health::show_report(window, cx)).ok();
    }
}

/// Carries out `fix` for check `id`, then runs the checks again.
pub fn apply(id: &'static str, fix: &Fix, cx: &mut App) {
    let work: Box<dyn FnOnce() -> anyhow::Result<()> + Send> = match fix.clone() {
        Fix::Open { url, .. } => {
            cx.open_url(&url);
            return;
        }
        Fix::Reveal(path) => {
            cx.reveal_path(&path);
            return;
        }
        Fix::AddToInputList => Box::new(add_to_input_list),
        Fix::UseFcitx => Box::new(|| fcitx_setup("framework")),
        Fix::AddToFcitx => Box::new(|| fcitx_setup("enable")),
    };
    let state = cx.global_mut::<Health>();
    if state.fixing.is_some() {
        return;
    }
    state.fixing = Some(id);
    state.fix_error = None;
    cx.refresh_windows();
    let task = cx.background_executor().spawn(async move { work() });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let state = cx.global_mut::<Health>();
            state.fixing = None;
            if let Err(error) = result {
                log::warn!("檢查輸入法 {id}: {error:#}");
                state.fix_error = Some(format!("{error:#}"));
            }
            refresh(cx);
        });
    })
    .detach();
}

#[cfg(windows)]
fn add_to_input_list() -> anyhow::Result<()> {
    zt_core::update::windows::add_input_method_for_user()
}

#[cfg(not(windows))]
fn add_to_input_list() -> anyhow::Result<()> {
    anyhow::bail!("只在 Windows 上提供")
}

/// The package's helper (`fcitx5-setup framework|enable`), as 「改用 Fcitx 5」 above the page runs it.
fn fcitx_setup(command: &str) -> anyhow::Result<()> {
    let helper = fcitx_helper().ok_or_else(|| anyhow::anyhow!("找不到 fcitx5-setup"))?;
    let output = std::process::Command::new(helper).arg(command).output()?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    let reason = reason.trim().trim_start_matches("fcitx5-setup：");
    anyhow::bail!("{}", if reason.is_empty() { output.status.to_string() } else { reason.to_owned() })
}

/// The signed-in user's name, written `<使用者>` in the report.
fn user_name() -> Option<String> {
    ["USERNAME", "USER", "LOGNAME"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|name| !name.is_empty()))
}

/// 回報問題's text, from the last results; `scale`: the settings window's.
pub fn report(scale: f32, cx: &App) -> String {
    let checks = checks(cx).unwrap_or_default();
    let mut sections = cx.try_global::<Health>().map(|health| health.sections.clone()).unwrap_or_default();
    // The display as it is now, with the window's scale.
    let display = zt_core::health::details::display_section(&display(Some(scale), cx));
    match sections.iter_mut().find(|section| section.title == display.title) {
        Some(section) => *section = display,
        None => sections.push(display),
    }
    let errors = health::recent::lines();
    let user = user_name();
    health::report::text(&Report {
        version: VERSION,
        generated: &minute(&chrono::Local::now()),
        system: &health::system(),
        checks,
        sections: &sections,
        errors: Some(&errors),
        home: std::env::home_dir().as_deref(),
        user: user.as_deref(),
    })
}

/// `zhuyintype --health [--fix ID]`: the checks without a window, as 回報問題's text on stdout (support, tests).
/// `--fix ID` first carries out that check's fix when it changes a setting (加入輸入法清單, 改用 Fcitx 5, 加入 Fcitx
/// 5).  Another copy serving the input method counts as the running service.  The exit code is 1 when a check
/// needs attention.
pub fn command_line(fix: Option<&str>) -> i32 {
    let settings = zt_core::settings::Settings::open(zt_core::paths::settings_file());
    let endpoint = zt_core::broker::endpoint();
    let running = endpoint.as_deref().is_some_and(health::answers);
    let configured = settings.string("dataDirectory");
    let configured = (!configured.is_empty()).then(|| PathBuf::from(configured));
    // The serving copy's own facts and recent errors (an older copy does not answer `diagnostics`).
    let diagnostics = endpoint.as_deref().filter(|_| running).and_then(|endpoint| {
        let mut client = zt_core::broker::Client::connect(endpoint).ok()?;
        let reply = client.request("diagnostics", serde_json::json!({ "context": "health" })).ok()?;
        zt_core::health::activity::Diagnostics::from_reply(&reply)
    });
    let about = AppFacts {
        diagnostics: diagnostics.clone(),
        stored_settings: settings.stored(),
        user_data: zt_core::paths::data_dir(),
        executable: std::env::current_exe().ok(),
        onnx_runtime: zt_core::models::runtime_path(),
        display: DisplayFacts::default(),
        automatic_updates: settings.bool(CHECKS_KEY),
        last_update_check: crate::updates::load_last_check().as_ref().map(minute),
        now: now_unix(),
        ..AppFacts::default()
    };
    let inputs = Inputs {
        version: VERSION.to_owned(),
        data: DataFacts::probe(zt_core::engine::DataFiles::find(configured.as_deref()), None),
        settings: SettingsFacts::probe(settings.path().map(PathBuf::from), settings.unreadable()),
        service: ServiceFacts {
            running,
            error: (!running).then(|| "沒有執行中的 ZhuyinType".to_owned()),
            reachable: None,
        },
        endpoint,
        updates: if settings.bool(CHECKS_KEY) { UpdateFacts::Unknown } else { UpdateFacts::ChecksOff },
        fcitx_helper: fcitx_helper(),
        about,
    };
    let mut outcome = health::run(&inputs);
    if let Some(id) = fix {
        let work: Option<fn() -> anyhow::Result<()>> =
            match outcome.checks.iter().find(|check| check.id == id).and_then(|c| c.fix.as_ref()) {
                Some(Fix::AddToInputList) => Some(add_to_input_list),
                Some(Fix::UseFcitx) => Some(|| fcitx_setup("framework")),
                Some(Fix::AddToFcitx) => Some(|| fcitx_setup("enable")),
                _ => None,
            };
        match work {
            Some(work) => {
                if let Err(error) = work() {
                    eprintln!("{id}: {error:#}");
                    return 2;
                }
                outcome = health::run(&inputs);
            }
            None => eprintln!("{id}: 沒有可以執行的修正"),
        }
    }
    // The display needs a window: left out here.
    outcome.sections.retain(|section| section.title != "顯示與輔助功能");
    let errors = diagnostics.map_or_else(health::recent::lines, |diagnostics| diagnostics.recent);
    let user = user_name();
    print!(
        "{}",
        health::report::text(&Report {
            version: VERSION,
            generated: &minute(&chrono::Local::now()),
            system: &health::system(),
            checks: &outcome.checks,
            sections: &outcome.sections,
            errors: Some(&errors),
            home: std::env::home_dir().as_deref(),
            user: user.as_deref(),
        })
    );
    i32::from(health::problems(&outcome.checks) > 0)
}

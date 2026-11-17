//! 檢查輸入法 in the app: runs `zt_core::health` off the UI thread when the settings window opens or becomes active
//! again, keeps the results for 關於 and the sidebar's badge, and carries out a check's fix.

use std::path::PathBuf;

use gpui_kit::{App, Global};
use zt_core::health::common::{DataFacts, ServiceFacts, SettingsFacts, UpdateFacts};
use zt_core::health::{self, Check, Fix, Inputs};
use zt_core::update::{CHECKS_KEY, Found};

use crate::app;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The last results and what is running.
#[derive(Default)]
pub struct Health {
    /// None until the first run has finished.
    pub checks: Option<Vec<Check>>,
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

/// What the app knows itself, for a run.
fn inputs(cx: &App) -> Inputs {
    let state = cx.global::<app::AppState>();
    let settings = &state.settings;
    let service = state.service.as_ref();
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
        },
        settings: SettingsFacts::probe(settings.path().map(PathBuf::from), settings.unreadable()),
        service: ServiceFacts { running: service.is_some(), error: state.service_error.clone(), reachable: None },
        endpoint: service.and_then(zt_core::service::Service::endpoint),
        updates,
        fcitx_helper: fcitx_helper(),
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
        inputs.data = DataFacts::probe(inputs.data.files.take(), inputs.data.engine_error.take());
        health::run(&inputs)
    });
    cx.spawn(async move |cx| {
        let checks = task.await;
        cx.update(|cx| {
            let state = cx.global_mut::<Health>();
            state.running = false;
            state.checks = Some(checks);
            cx.refresh_windows();
        });
    })
    .detach();
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

/// 回報問題's text, from the last results.
pub fn report(cx: &App) -> String {
    let checks = checks(cx).unwrap_or_default();
    let errors = health::recent::lines();
    health::report::text(
        VERSION,
        &chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        &health::system(),
        checks,
        Some(&errors),
        std::env::home_dir().as_deref(),
    )
}

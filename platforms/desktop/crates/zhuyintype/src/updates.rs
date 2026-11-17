//! Updates in the signed-in user's app (macOS `Updater`): checks the signed feed once a day, shows what it found on
//! 關於, and when 自動安裝更新 is on starts the system's updater once nothing is being composed and no settings or
//! wizard window is open.  Installing itself is the updater's job (`zt_core::update`).
//!
//! Linux: the package manager replaces the executable under the running app, so once idle the app starts the new
//! one in its place (Windows: the installer stops the app and starts it again in every session).

use std::time::Duration;

use chrono::{DateTime, Local};
use gpui_kit::{App, Global};
use serde_json::{Value, json};
use zt_core::update::{self, CHECK_INTERVAL, CHECKS_KEY, Found, INSTALLS_KEY};

use crate::app;

/// How often the app looks at the clock, the composition and the executable.
const TICK: Duration = Duration::from_secs(60);
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What 關於 shows.
#[derive(Default)]
pub struct Updates {
    pub last_check: Option<DateTime<Local>>,
    pub found: Option<Found>,
    pub checking: bool,
    /// The system's updater was started for this version.
    pub installing: Option<String>,
    pub error: Option<String>,
    #[cfg(target_os = "linux")]
    executable: Option<(u64, u64)>,
}

impl Global for Updates {}

fn state_file() -> Option<std::path::PathBuf> {
    zt_core::paths::data_dir().map(|dir| dir.join("update.json"))
}

fn load_last_check() -> Option<DateTime<Local>> {
    let text = std::fs::read_to_string(state_file()?).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    DateTime::parse_from_rfc3339(value.get("lastCheck")?.as_str()?).ok().map(|time| time.with_timezone(&Local))
}

fn save_last_check(time: DateTime<Local>) {
    let Some(path) = state_file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let bytes = json!({ "lastCheck": time.to_rfc3339() }).to_string();
    if let Err(error) = zt_core::settings::write_atomically(&path, bytes.as_bytes()) {
        log::warn!("update state: {error:#}");
    }
}

/// The state 關於 reads.
pub fn state(cx: &App) -> &Updates {
    cx.global::<Updates>()
}

/// Starts the checks (the app that serves the input method only; screenshots and second copies never check).
pub fn start(cx: &mut App) {
    cx.set_global(Updates {
        last_check: load_last_check(),
        #[cfg(target_os = "linux")]
        executable: update::executable_identity(),
        ..Updates::default()
    });
    cx.spawn(async move |cx| {
        // Not in the busy first minute after login.
        loop {
            cx.background_executor().timer(TICK).await;
            cx.update(tick);
        }
    })
    .detach();
}

fn tick(cx: &mut App) {
    if !cx.has_global::<Updates>() {
        return;
    }
    let settings = app::settings(cx);
    let (checks, installs) = (settings.bool(CHECKS_KEY), settings.bool(INSTALLS_KEY));
    let state = state(cx);
    let due = !state.checking
        && state
            .last_check
            .is_none_or(|last| Local::now().signed_duration_since(last).to_std().unwrap_or_default() >= CHECK_INTERVAL);
    let ready = matches!(&state.found, Some(Found::Newer { .. })) && state.installing.is_none();
    if checks && due {
        check_now(cx);
    }
    if checks && installs && ready && idle(cx) {
        install_now(cx);
    }
    #[cfg(target_os = "linux")]
    restart_if_replaced(cx);
}

/// Nothing is being composed and no settings or wizard window is open.
fn idle(cx: &mut App) -> bool {
    let composing =
        app::service(cx).is_some_and(|service| !service.presentation().is_empty() || service.popup().is_some());
    !composing && !crate::shell::is_open(cx) && !crate::wizard::is_open(cx)
}

/// 立即檢查: checks the feed now.
pub fn check_now(cx: &mut App) {
    if !cx.has_global::<Updates>() || state(cx).checking {
        return;
    }
    cx.global_mut::<Updates>().checking = true;
    cx.refresh_windows();
    let task = cx.background_executor().spawn(async move { update::check(VERSION) });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let now = Local::now();
            let state = cx.global_mut::<Updates>();
            state.checking = false;
            match result {
                Ok(found) => {
                    state.last_check = Some(now);
                    state.found = Some(found);
                    state.error = None;
                    save_last_check(now);
                }
                Err(error) => {
                    log::warn!("update check: {error:#}");
                    state.error = Some(format!("{error:#}"));
                }
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

/// 立即安裝 (or automatically when idle): starts the system's updater for the version found.
pub fn install_now(cx: &mut App) {
    let Some(Found::Newer { version, .. }) = state(cx).found.clone() else { return };
    // Windows: the installer ends this app; what was learnt since the last write is written now.
    if let Some(service) = app::service(cx) {
        service.flush();
    }
    let state = cx.global_mut::<Updates>();
    match update::start_system_update() {
        Ok(()) => {
            log::info!("installing update {version}");
            state.installing = Some(version);
            state.error = None;
        }
        Err(error) => {
            log::warn!("update: {error:#}");
            state.error = Some(format!("{error:#}"));
            // Not again until the next check.
            state.found = None;
        }
    }
    cx.refresh_windows();
}

/// Linux: the package manager replaced the executable; once idle, become the new version.  The broker socket and
/// its lock close on exec, so the new process takes the input method over at once.
#[cfg(target_os = "linux")]
fn restart_if_replaced(cx: &mut App) {
    use std::os::unix::process::CommandExt as _;
    let started = state(cx).executable;
    if started.is_none() || update::executable_identity() == started || !idle(cx) {
        return;
    }
    if let Some(service) = app::service(cx) {
        service.flush();
    }
    // /proc/self/exe still names the old file; the new one is at the same path.
    let Ok(link) = std::fs::read_link("/proc/self/exe") else { return };
    let path = link.to_string_lossy().trim_end_matches(" (deleted)").to_owned();
    log::info!("{path} was updated; starting the new version");
    let error = std::process::Command::new(&path).arg("--background").exec();
    log::warn!("restarting after the update: {error}");
    cx.global_mut::<Updates>().executable = None;
}

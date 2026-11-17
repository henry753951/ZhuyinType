//! ZhuyinType for Linux and Windows.
//!
//! One process: the settings window, the candidate window, the setup wizard and the input method host that the
//! native adapters (Fcitx5, TSF) talk to.  Starting it again while it runs shows the settings of the running one.

#![allow(unreachable_pub)]
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod candidate;
mod card_window;
mod health;
mod motion;
mod pages;
mod platform;
mod shell;
mod theme;
mod ui;
mod updates;
mod wizard;

use pages::Page;
use zt_core::service::{AlreadyRunning, Service, ServiceConfig};
use zt_core::settings::Settings;

struct Args {
    page: Page,
    /// `--subpage ID`: open one of the page's sub-pages.
    subpage: Option<String>,
    /// `--row ANCHOR`: scroll to a row and show the arrival cue, as a link would.
    row: Option<String>,
    /// `--size WxH`: the window's size in logical pixels (screenshots of narrow and wide layouts).
    size: Option<(f32, f32)>,
    /// `--theme light|dark`: force an appearance (screenshots).
    dark: Option<bool>,
    /// `--settings-file PATH`: keep settings there (screenshots and tests use a temporary file).
    settings_file: Option<std::path::PathBuf>,
    /// `--popover ID`: show one ⓘ card on its own instead of the settings window (screenshots).
    popover: Option<String>,
    /// `--background`: start the input method host without opening the settings window (autostart at login).
    background: bool,
    /// `--no-broker`: don't serve the native input method adapters (screenshots, a second copy for development).
    no_broker: bool,
    /// `--data DIR`: the engine data (else ZT_DATA_DIR, beside the executable, the system data dir, data/build).
    /// `--lexicon FILE` names the data by its lexicon (the Fcitx5 addon's tests).
    data_dir: Option<std::path::PathBuf>,
    /// `--ime-socket PATH`: the broker endpoint (else ZT_IME_SOCKET, else the per-user one; IPC.md).
    endpoint: Option<String>,
    /// `--panel VARIANT`: only a candidate window over a few document lines (screenshots; `candidate::showcase`).
    panel: Option<String>,
    /// `--popup demo`: the real floating windows at a caret, with an example list.
    popup: Option<String>,
    /// `--wizard SPEC` (or `ZT_WIZARD=SPEC`): only the setup wizard, still (screenshots; `wizard::open_preview`).
    wizard: Option<String>,
    /// `--after-crash`: started by a copy whose UI thread panicked; wait for it to end, then take over.
    after_crash: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        page: Page::Intro,
        subpage: None,
        row: None,
        size: None,
        dark: None,
        settings_file: None,
        popover: None,
        background: false,
        no_broker: false,
        data_dir: None,
        endpoint: None,
        panel: None,
        popup: None,
        wizard: std::env::var("ZT_WIZARD").ok().filter(|spec| !spec.is_empty()),
        after_crash: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--page" => {
                if let Some(page) = iter.next().as_deref().and_then(Page::from_id) {
                    args.page = page;
                }
            }
            "--subpage" => args.subpage = iter.next(),
            "--row" => args.row = iter.next(),
            "--size" => {
                args.size = iter.next().and_then(|text| {
                    let (w, h) = text.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
            }
            "--theme" => args.dark = iter.next().map(|theme| theme == "dark"),
            "--settings-file" => args.settings_file = iter.next().map(Into::into),
            "--popover" => args.popover = iter.next(),
            "--background" => args.background = true,
            "--no-broker" => args.no_broker = true,
            "--data" => args.data_dir = iter.next().map(Into::into),
            "--lexicon" => {
                args.data_dir = iter.next().and_then(|file| std::path::Path::new(&file).parent().map(Into::into));
            }
            "--ime-socket" => args.endpoint = iter.next(),
            "--panel" => args.panel = iter.next(),
            "--popup" => args.popup = iter.next(),
            "--wizard" => args.wizard = iter.next(),
            "--after-crash" => args.after_crash = true,
            _ => {}
        }
    }
    args
}

fn main() {
    env_logger_init();
    if let Some(code) = updater_mode() {
        std::process::exit(code);
    }
    let args = parse_args();
    let config = ServiceConfig {
        data_dir: args.data_dir.clone(),
        endpoint: args.endpoint.clone(),
        settings_file: args.settings_file.clone(),
        broker: !args.no_broker && args.popover.is_none() && args.wizard.is_none(),
        // Sign-in and the Fcitx5 addon start `--background`; a second one ends quietly (packaging/linux/README.md).
        quiet_if_running: args.background,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        wait_for_endpoint: args.after_crash.then_some(std::time::Duration::from_secs(10)),
        ..ServiceConfig::default()
    };
    let (service, events, service_error) = match Service::start(config) {
        Ok((service, events)) => (Some(service), Some(events), None),
        // Another copy runs the input method; it was asked to show its settings.
        Err(error) if error.is::<AlreadyRunning>() => return,
        Err(error) => {
            log::error!("service: {error:#}");
            (None, None, Some(format!("{error:#}")))
        }
    };
    if let Some(service) = service.as_ref().filter(|_| !args.no_broker) {
        save_statistics_when_quiet(service.stats().clone());
    }
    // A standard user who installed with an administrator's password got ZhuyinType in that administrator's input
    // method list, not their own; Active Setup adds it at their next sign-in, this at once.
    #[cfg(windows)]
    if service.is_some() && !args.no_broker {
        std::thread::spawn(|| {
            if let Err(error) = zt_core::update::windows::enable_input_method_for_user() {
                log::warn!("input method list: {error:#}");
            }
        });
    }
    // GPUI connects to the input method (XIM, the Wayland text-input) once, when it starts; at login autostart may
    // run before Fcitx 5 is up, and the app's own fields would then go without it for the session.  Adding ZhuyinType
    // to Fcitx's input methods waits as well: a profile edited while Fcitx starts is lost when it saves its own.
    #[cfg(target_os = "linux")]
    if args.background {
        wait_for_fcitx(std::time::Duration::from_secs(15));
    }
    #[cfg(target_os = "linux")]
    if service.is_some() && !args.no_broker {
        std::thread::spawn(linux_enable_fcitx);
    }
    let settings = service.as_ref().map_or_else(
        || Settings::open(args.settings_file.clone().or_else(zt_core::paths::settings_file)),
        |service| service.settings().clone(),
    );
    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        motion::init(cx);
        theme::load_fonts(cx);
        // The copy that serves the input method runs until the session ends: closing the settings window or the
        // wizard must not end it.  GPUI otherwise quits when the last window closes, and on Windows the app has no
        // other window (the text service draws the candidate window inside each application), so typing stopped.
        let serves = service.is_some() && !args.no_broker && args.popover.is_none() && args.wizard.is_none();
        if serves {
            cx.set_quit_mode(gpui_kit::QuitMode::Explicit);
            relaunch_after_panic();
        }
        cx.set_global(app::AppState { settings, service, service_error });
        follow_system_appearance(cx);
        if let Some(events) = events {
            candidate::popup::run(events, cx);
        }
        #[cfg(target_os = "linux")]
        if serves {
            candidate::popup::prewarm(cx);
        }
        if let Some(id) = &args.popover {
            card_window::open(id, args.dark, args.size, cx).expect("the card window opens");
            cx.activate(true);
            return;
        }
        if let Some(variant) = args.panel.clone() {
            candidate::showcase::open(variant, args.dark == Some(true), cx).expect("the panel window opens");
            return;
        }
        if args.popup.is_some() {
            candidate::showcase::popup_demo(cx);
            return;
        }
        if let Some(spec) = &args.wizard {
            wizard::open_preview(spec, args.dark, args.size, cx);
            cx.activate(true);
            return;
        }
        // The copy that serves the input method keeps it up to date (screenshots and second copies never do).
        if !args.no_broker && args.settings_file.is_none() && app::service(cx).is_some() {
            updates::start(cx);
        }
        if args.background {
            return;
        }
        // A window that cannot open (no usable GPU, say) must not take the input method down with it.
        if let Err(error) = shell::open(args.page, args.dark, args.size, cx) {
            log::error!("opening the settings window: {error:#}");
            if serves {
                return;
            }
            panic!("the settings window opens: {error:#}");
        }
        // Screenshots and second copies for development (a settings file, no broker, a forced theme) never do.
        if args.settings_file.is_none() && !args.no_broker && args.dark.is_none() {
            wizard::show_on_first_use(cx);
            #[cfg(target_os = "linux")]
            platform::linux::check(cx);
        }
        if args.subpage.is_some() || args.row.is_some() {
            app::go(
                app::Place {
                    page: args.page,
                    subpage: args.subpage.clone().map(Into::into),
                    row: args.row.clone().map(Into::into),
                },
                cx,
            );
        }
        cx.activate(true);
    });
}

/// 使用統計 are written at most every ten minutes of typing, and nothing writes them when the session ends (the
/// process is ended at sign-out, or by the installer, without a chance to): also write them once typing has stopped
/// for a minute.
fn save_statistics_when_quiet(stats: zt_core::stats::UsageStats) {
    const QUIET: std::time::Duration = std::time::Duration::from_secs(60);
    let spawned = std::thread::Builder::new().name("zt-stats".into()).spawn(move || {
        loop {
            std::thread::sleep(QUIET);
            if let Err(error) = stats.save_if_quiet(QUIET) {
                log::warn!("{error:#}");
            }
        }
    });
    if let Err(error) = spawned {
        log::warn!("{error}");
    }
}

/// A panic on the UI thread ends the process (on Windows it cannot unwind out of the window procedure), and with it
/// the input method until the next sign-in.  Start a new copy in the background; it waits for this one to end
/// (`--after-crash`).  Not within the first minute, so a copy that panics at start does not loop.
fn relaunch_after_panic() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RELAUNCHED: AtomicBool = AtomicBool::new(false);
    let started = std::time::Instant::now();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        if std::thread::current().name() != Some("main")
            || started.elapsed() < std::time::Duration::from_secs(60)
            || RELAUNCHED.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let args = std::env::args().skip(1).filter(|arg| arg != "--background" && arg != "--after-crash");
        match std::process::Command::new(exe).args(args).args(["--background", "--after-crash"]).spawn() {
            Ok(_) => log::error!("the UI thread panicked; started a new copy"),
            Err(error) => log::error!("the UI thread panicked; could not start a new copy: {error}"),
        }
    }));
}

/// Draws every window again when a system setting behind 「主題」 or 「透明效果」 changes (Windows' 透明效果 and
/// light or dark, or the desktop's own setting on Linux without the portal); GPUI itself follows the system's light or
/// dark where it can.
fn follow_system_appearance(cx: &mut gpui_kit::App) {
    let (changed, changes) = async_channel::bounded::<()>(1);
    zt_core::appearance::watch(move || {
        let _ = changed.try_send(());
    });
    cx.spawn(async move |cx| {
        while changes.recv().await.is_ok() {
            cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

/// The system's updater (`zt_core::update`), run as the administrator or root, never with a window:
/// `--update-service` (the Windows service), `--system-update` (the Linux systemd unit, or by hand), and
/// `--relaunch-sessions` (Windows, the installer after an update).  The exit code, or None for the app itself.
fn updater_mode() -> Option<i32> {
    let mode = std::env::args().nth(1)?;
    if mode == "--system-update" {
        return Some(system_update());
    }
    #[cfg(windows)]
    {
        let result: anyhow::Result<()> = match mode.as_str() {
            "--update-service" => zt_core::update::windows::run_service(|| {
                let _ = system_update();
            }),
            "--relaunch-sessions" => zt_core::update::windows::relaunch_sessions().map(|_| ()),
            _ => return None,
        };
        Some(match result {
            Ok(()) => 0,
            Err(error) => {
                log::error!("{mode}: {error:#}");
                1
            }
        })
    }
    #[cfg(not(windows))]
    None
}

/// Installs the newest release when it is newer than this one (`zt_core::update::system_update`).
fn system_update() -> i32 {
    match zt_core::update::system_update(env!("CARGO_PKG_VERSION")) {
        Ok(Some(next)) => {
            log::warn!("installing {next}");
            0
        }
        Ok(None) => 0,
        Err(error) => {
            log::error!("update: {error:#}");
            1
        }
    }
}

/// Adds ZhuyinType to the user's Fcitx 5 input methods once, with the helper the package installs
/// (`packaging/linux/fcitx5-setup`).  Runs as the signed-in user, off the UI thread; keeps every existing entry.  Once
/// it is there a marker in the personal data folder says so: someone who later removes ZhuyinType from Fcitx's input
/// methods keeps it removed.
#[cfg(target_os = "linux")]
fn linux_enable_fcitx() {
    let marker = zt_core::paths::data_dir().map(|dir| dir.join("fcitx5-added"));
    if marker.as_ref().is_some_and(|marker| marker.exists()) {
        return;
    }
    let Some(helper) = platform::linux::fcitx_helper() else { return };
    let Ok(status) = std::process::Command::new(&helper).arg("status").output() else { return };
    let status = String::from_utf8_lossy(&status.stdout);
    if status.lines().any(|line| line == "fcitx5=missing") {
        return;
    }
    let added = status.lines().any(|line| line == "profile=added")
        || match std::process::Command::new(&helper).arg("enable").status() {
            Ok(code) if code.success() => {
                log::info!("added ZhuyinType to the Fcitx 5 input methods");
                true
            }
            Ok(code) => {
                log::warn!("fcitx5-setup enable: {code}");
                false
            }
            Err(error) => {
                log::warn!("fcitx5-setup: {error}");
                false
            }
        };
    if let (true, Some(marker)) = (added, marker) {
        if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(error) = std::fs::write(&marker, b"") {
            log::warn!("{}: {error}", marker.display());
        }
    }
}

/// Waits until an `fcitx5` process runs, at most `limit`; returns at once when Fcitx 5 is not the input method.
#[cfg(target_os = "linux")]
fn wait_for_fcitx(limit: std::time::Duration) {
    let wanted = ["XMODIFIERS", "GTK_IM_MODULE", "QT_IM_MODULE"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| value.contains("fcitx")));
    if !wanted {
        return;
    }
    let running =
        || {
            std::fs::read_dir("/proc").into_iter().flatten().flatten().any(|entry| {
                std::fs::read_to_string(entry.path().join("comm")).is_ok_and(|name| name.trim() == "fcitx5")
            })
        };
    let start = std::time::Instant::now();
    while !running() && start.elapsed() < limit {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Logs to stderr; `RUST_LOG` picks the level (default: warnings).
fn env_logger_init() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::max_level()
        }
        fn log(&self, record: &log::Record<'_>) {
            eprintln!("{} {}: {}", record.level(), record.target(), record.args());
            // 回報問題 lists the recent warnings and errors (zt_core::health::recent; never typed text).
            if record.level() <= log::Level::Warn {
                zt_core::health::recent::record(&format!("{} {}: {}", record.level(), record.target(), record.args()));
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let level = match std::env::var("RUST_LOG").as_deref() {
        Ok("error") => log::LevelFilter::Error,
        Ok("info") => log::LevelFilter::Info,
        Ok("debug") => log::LevelFilter::Debug,
        Ok("trace") => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    };
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}

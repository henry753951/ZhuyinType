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
mod pages;
mod platform;
mod shell;
mod theme;
mod ui;
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
            _ => {}
        }
    }
    args
}

fn main() {
    env_logger_init();
    let args = parse_args();
    let config = ServiceConfig {
        data_dir: args.data_dir.clone(),
        endpoint: args.endpoint.clone(),
        settings_file: args.settings_file.clone(),
        broker: !args.no_broker && args.popover.is_none() && args.wizard.is_none(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
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
    #[cfg(target_os = "linux")]
    if service.is_some() && !args.no_broker {
        std::thread::spawn(linux_enable_fcitx);
    }
    // GPUI connects to the input method (XIM, the Wayland text-input) once, when it starts; at login autostart may
    // run before Fcitx 5 is up, and the app's own fields would then go without it for the session.
    #[cfg(target_os = "linux")]
    if args.background {
        wait_for_fcitx(std::time::Duration::from_secs(15));
    }
    let settings = service.as_ref().map_or_else(
        || Settings::open(args.settings_file.clone().or_else(zt_core::paths::settings_file)),
        |service| service.settings().clone(),
    );
    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        theme::load_fonts(cx);
        cx.set_global(app::AppState { settings, service, service_error });
        if let Some(events) = events {
            candidate::popup::run(events, cx);
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
        if args.background {
            return;
        }
        shell::open(args.page, args.dark, args.size, cx).expect("the settings window opens");
        // Screenshots and second copies for development (a settings file, no broker, a forced theme) never do.
        if args.settings_file.is_none() && !args.no_broker && args.dark.is_none() {
            wizard::show_on_first_use(cx);
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

/// Adds ZhuyinType to the user's Fcitx 5 input methods when it is not there yet, with the helper the package installs
/// (`packaging/linux/fcitx5-setup`).  Runs as the signed-in user, off the UI thread; keeps every existing entry.
#[cfg(target_os = "linux")]
fn linux_enable_fcitx() {
    let exe = std::env::current_exe().ok();
    let candidates = [
        exe.as_ref().and_then(|exe| exe.parent()).map(|bin| bin.join("../lib/zhuyintype/fcitx5-setup")),
        Some(std::path::PathBuf::from("/usr/lib/zhuyintype/fcitx5-setup")),
    ];
    let Some(helper) = candidates.into_iter().flatten().find(|path| path.is_file()) else { return };
    let Ok(status) = std::process::Command::new(&helper).arg("status").output() else { return };
    let status = String::from_utf8_lossy(&status.stdout);
    if status.lines().any(|line| line == "profile=added") || status.lines().any(|line| line == "fcitx5=missing") {
        return;
    }
    match std::process::Command::new(&helper).arg("enable").status() {
        Ok(code) if code.success() => log::info!("added ZhuyinType to the Fcitx 5 input methods"),
        Ok(code) => log::warn!("fcitx5-setup enable: {code}"),
        Err(error) => log::warn!("fcitx5-setup: {error}"),
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

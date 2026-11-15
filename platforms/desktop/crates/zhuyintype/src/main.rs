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
    data_dir: Option<std::path::PathBuf>,
    /// `--panel VARIANT`: only a candidate window over a few document lines (screenshots; `candidate::showcase`).
    panel: Option<String>,
    /// `--popup demo`: the real floating windows at a caret, with an example list.
    popup: Option<String>,
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
        panel: None,
        popup: None,
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
            "--panel" => args.panel = iter.next(),
            "--popup" => args.popup = iter.next(),
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
        settings_file: args.settings_file.clone(),
        broker: !args.no_broker && args.popover.is_none(),
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
        if args.background {
            return;
        }
        shell::open(args.page, args.dark, args.size, cx).expect("the settings window opens");
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

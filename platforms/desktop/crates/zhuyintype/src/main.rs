//! ZhuyinType for Linux and Windows.
//!
//! One process: the settings window, the candidate window, the setup wizard and the input method host that the
//! native adapters (Fcitx5, TSF) talk to.  Starting it again while it runs shows the settings of the running one.

#![allow(unreachable_pub)]
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod pages;
mod shell;
mod theme;
mod ui;

use pages::Page;
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
}

fn parse_args() -> Args {
    let mut args = Args { page: Page::Intro, subpage: None, row: None, size: None, dark: None, settings_file: None };
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
            _ => {}
        }
    }
    args
}

fn main() {
    let args = parse_args();
    let settings = Settings::open(args.settings_file.clone().or_else(zt_core::paths::settings_file));
    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        // The service starts here once zt-core's port lands (Service::start); until then pages show empty states.
        cx.set_global(app::AppState { settings, service: None, service_error: None });
        shell::open(args.page, args.dark, args.size, cx).expect("the settings window opens");
        if args.subpage.is_some() || args.row.is_some() {
            app::go(
                app::Place { page: args.page, subpage: args.subpage.clone().map(Into::into), row: args.row.clone().map(Into::into) },
                cx,
            );
        }
        cx.activate(true);
    });
}

//! ZhuyinType for Linux and Windows.
//!
//! One process: the settings window, the candidate window, the setup wizard and the input method host that the
//! native adapters (Fcitx5, TSF) talk to.  Starting it again while it runs shows the settings of the running one.

#![allow(unreachable_pub)]
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod pages;
mod shell;
mod theme;
mod ui;

use pages::Page;
use zt_core::settings::Settings;

struct Args {
    page: Page,
    /// `--theme light|dark`: force an appearance (screenshots).
    dark: Option<bool>,
    /// `--settings-file PATH`: keep settings there (screenshots and tests use a temporary file).
    settings_file: Option<std::path::PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args { page: Page::Intro, dark: None, settings_file: None };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--page" => {
                if let Some(page) = iter.next().as_deref().and_then(Page::from_id) {
                    args.page = page;
                }
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
        shell::open(args.page, settings, args.dark, cx).expect("the settings window opens");
        cx.activate(true);
    });
}

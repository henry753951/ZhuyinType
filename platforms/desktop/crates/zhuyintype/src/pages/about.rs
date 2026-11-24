//! 「關於」 page (macOS `AboutPane`, `SourcesPane`, `DocumentSheet`): the app's icon, name and version; 更新 (自動檢查更新,
//! 自動安裝更新, a newer version found, 檢查更新 with the last check; `crate::updates`); 程式授權 (AGPL 3.0, read in the
//! app); 原始碼 (this version on GitHub); and the 授權與資料來源 sub-page.  更新內容 (what this version changed), 意見與問題
//! (GitHub issue forms) and 移除 ZhuyinType are in `notes` and `feedback`.

mod document;
mod feedback;
pub(crate) mod health;
mod legal;
mod notes;

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div};

use super::Page;
use super::words::data;
use crate::app::{self, Place};
use crate::theme::size;
use crate::ui::actions::{ButtonKind, link_button, push_button, setting_switch_when};
use crate::ui::alert::notice;
use crate::ui::content::{app_header, disclosure};
use crate::ui::{self, h, v};

const SOURCES: &str = "sources";
const NOTES: &str = "notes";
const REMOVAL: &str = "removal";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = "https://github.com/henry753951/ZhuyinType";

/// 更新內容.
pub fn notes_place() -> Place {
    Place::subpage(Page::About, NOTES)
}

/// 意見與問題 (the input method's menu, the update notification).
pub fn feedback_place() -> Place {
    Place::row(Page::About, feedback::SECTION)
}

pub struct AboutPage {
    /// The entries of 授權與資料來源 that are open.
    open: BTreeSet<String>,
    pending_demo: bool,
}

impl AboutPage {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let open = data::demo("open").map(|id| BTreeSet::from([id])).unwrap_or_default();
        let _ = cx;
        Self { open, pending_demo: true }
    }

    fn open_demo_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pending_demo) {
            return;
        }
        if let Some(file) = data::demo("doc") {
            window.defer(cx, move |window, cx| show_document(&file, "ZhuyinType", window, cx));
        }
    }

    fn main(&self, cx: &mut Context<Self>) -> AnyElement {
        v().gap(size::SECTION_GAP)
            .child(ui::section("").bare().first().row(app_header("ZhuyinType", format!("版本 {VERSION}"), cx)))
            .child(updates_section(cx))
            .child(
                ui::section("更新內容")
                    .row(
                        ui::row("drill-notes", "這一版的更新內容")
                            .value(VERSION)
                            .anchor(NOTES)
                            .on_click(|_, _, cx| app::go(notes_place(), cx)),
                    )
                    .row(ui::setting_row(zt_core::release_notes::SHOW_KEY, cx)),
            )
            .child(feedback::section())
            .child(
                ui::section("")
                    .row(ui::row("row-license", "程式授權").anchor("license").trailing(link_button(
                        "agpl",
                        "AGPL 3.0",
                        |_, window, cx| show_document("AGPL-3.0.txt", "ZhuyinType", window, cx),
                        cx,
                    )))
                    .row(ui::row("row-source", "原始碼").anchor("source").trailing(link_button(
                        "source",
                        "查看此版本",
                        |_, _, cx| cx.open_url(&format!("{REPOSITORY}/tree/v{VERSION}")),
                        cx,
                    )))
                    .row(
                        ui::row("drill-sources", "授權與資料來源")
                            .subtitle("程式庫、詞庫、資料集與模型的授權與署名")
                            .anchor(SOURCES)
                            .on_click(|_, _, cx| app::go(Place::subpage(Page::About, SOURCES), cx)),
                    )
                    .row(feedback::removal_row()),
            )
            .into_any_element()
    }

    fn sources(&self, cx: &mut Context<Self>) -> AnyElement {
        let data_dir = data_directory(cx);
        let Some(groups) = legal::groups(data_dir.as_deref()) else {
            return ui::section("").first().row(ui::row("sources-error", "無法讀取授權與資料來源")).into_any_element();
        };
        let page = cx.entity().downgrade();
        v().gap(size::SECTION_GAP)
            .children(groups.into_iter().map(|group| {
                ui::section(group.title.clone()).rows(group.entries.into_iter().map(|entry| {
                    let open = self.open.contains(&entry.id);
                    let id = entry.id.clone();
                    let toggle_page = page.clone();
                    let body = open.then(|| entry_body(&entry, data_dir.clone(), cx));
                    disclosure(
                        SharedString::from(format!("source-{}", entry.id)),
                        entry.name.clone(),
                        entry.summary.clone(),
                        open,
                        body,
                        move |_, _, cx| {
                            let id = id.clone();
                            toggle_page
                                .update(cx, |page, cx| {
                                    if !page.open.remove(&id) {
                                        page.open.insert(id);
                                    }
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    )
                }))
            }))
            .into_any_element()
    }
}

impl Render for AboutPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.open_demo_states(window, cx);
        match app::nav(cx).subpage.as_deref() {
            Some(SOURCES) => self.sources(cx),
            Some(NOTES) => {
                // Read here: the notification has nothing more to say.
                crate::update_notice::seen(cx);
                notes::page(cx)
            }
            Some(REMOVAL) => feedback::removal(cx),
            _ => self.main(cx),
        }
    }
}

/// 更新, as on macOS: the two switches, a newer version with 立即安裝, and 檢查更新 with the last check.
fn updates_section(cx: &mut Context<AboutPage>) -> impl IntoElement + use<> {
    use zt_core::update::{CHECKS_KEY, Found, INSTALLS_KEY};
    let checks = app::settings(cx).bool(CHECKS_KEY);
    let mut section = ui::section("更新")
        .row(ui::setting_row(CHECKS_KEY, cx))
        .row(ui::setting_row(INSTALLS_KEY, cx).dimmed(!checks).trailing(setting_switch_when(INSTALLS_KEY, checks, cx)));
    let Some(state) = cx.try_global::<crate::updates::Updates>() else {
        return section.row(
            ui::row("row-check-updates", "檢查更新")
                .subtitle("在 GitHub 查看最新版本與更新說明")
                .anchor("updates")
                .trailing(push_button(
                    "open-releases",
                    "開啟發行頁面…",
                    ButtonKind::Normal,
                    true,
                    |_, _, cx| cx.open_url(&format!("{REPOSITORY}/releases")),
                    cx,
                )),
        );
    };
    let (checking, installing, error) = (state.checking, state.installing.clone(), state.error.clone());
    let last = state.last_check.map(|time| format!("上次檢查：{}", time.format("%Y/%m/%d %H:%M")));
    match state.found.clone() {
        Some(Found::Newer { version, notes }) => {
            let (label, enabled) = match &installing {
                Some(_) => ("安裝中…", false),
                None => ("立即安裝", true),
            };
            section = section.row(
                ui::row("row-update-found", format!("有新版本 {version}"))
                    .subtitle(if installing.is_some() {
                        "完成後自動換成新版本"
                    } else {
                        "安裝時會短暫停止輸入法"
                    })
                    .trailing(
                        h().gap(size::ROW_PADDING_Y)
                            .child(link_button("update-notes", "更新說明", move |_, _, cx| cx.open_url(&notes), cx))
                            .child(push_button(
                                "install-update",
                                label,
                                ButtonKind::Default,
                                enabled,
                                |_, _, cx| crate::updates::install_now(cx),
                                cx,
                            )),
                    ),
            );
        }
        Some(Found::Unsupported) => {
            section = section.row(
                ui::row("row-update-unsupported", "這份 ZhuyinType 無法自動更新")
                    .subtitle("不是由安裝程式或套件安裝的；請到發行頁面下載"),
            );
        }
        Some(Found::Current) | None => {}
    }
    let subtitle = match (&error, state.found.as_ref()) {
        (Some(error), _) => format!("無法檢查：{error}"),
        (None, Some(Found::Current)) => {
            format!("已是最新版本（{VERSION}）{}", last.map(|l| format!("。{l}")).unwrap_or_default())
        }
        _ => last.unwrap_or_else(|| "每天自動檢查一次".to_owned()),
    };
    section.row(ui::row("row-check-updates", "檢查更新").subtitle(subtitle).anchor("updates").trailing(push_button(
        "check-updates",
        if checking { "檢查中…" } else { "立即檢查" },
        ButtonKind::Normal,
        !checking,
        |_, _, cx| crate::updates::check_now(cx),
        cx,
    )))
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    match id {
        SOURCES => Some("授權與資料來源"),
        NOTES => Some("更新內容"),
        REMOVAL => Some("移除 ZhuyinType"),
        _ => None,
    }
}

/// The engine's data folder (model notices live there), when the engine runs.
fn data_directory(cx: &App) -> Option<PathBuf> {
    app::service(cx).and_then(|service| service.data_files()).map(|files| files.directory)
}

/// An open entry: copyright, version, changes, the source link, and its files.
fn entry_body(entry: &legal::Entry, data_dir: Option<PathBuf>, cx: &App) -> AnyElement {
    let mut body = v().gap(size::ROW_PADDING_Y);
    for line in [
        entry.copyright.clone(),
        entry.version.as_ref().map(|v| format!("版本：{v}")),
        entry.changes.as_ref().map(|c| format!("修改：{c}")),
    ]
    .into_iter()
    .flatten()
    {
        body = body.child(div().child(line));
    }
    if let Some(url) = entry.url.clone() {
        body = body.child(h().child(link_button(
            SharedString::from(format!("url-{}", entry.id)),
            "來源",
            move |_, _, cx| cx.open_url(&url),
            cx,
        )));
    }
    for file in &entry.files {
        let name = file.clone();
        let owner = entry.name.clone();
        let dir = data_dir.clone();
        body = body.child(h().child(link_button(
            SharedString::from(format!("file-{}-{file}", entry.id)),
            file.clone(),
            move |_, window, cx| match legal::document(&name, dir.as_deref()) {
                Some(document) if document.text.is_some() => document::open(document, owner.clone(), window, cx),
                Some(document) => {
                    if let Some(path) = document.path {
                        cx.reveal_path(&path);
                    }
                }
                None => notice("無法開啟", format!("找不到「{name}」。"), window, cx),
            },
            cx,
        )));
    }
    body.into_any_element()
}

/// Opens built-in licence `file` in the viewer.
fn show_document(file: &str, owner: &str, window: &mut Window, cx: &mut App) {
    match legal::document(file, data_directory(cx).as_deref()) {
        Some(document) if document.text.is_some() => document::open(document, owner.to_owned(), window, cx),
        _ => notice("無法開啟", format!("找不到「{file}」。"), window, cx),
    }
}

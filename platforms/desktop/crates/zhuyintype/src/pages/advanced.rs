//! 「進階」 page (macOS `AdvancedPane`, `CorrectionLogInfo`, `AppsPage`, `AppListController`): the correction log
//! (記錄改字, its context switch, the record count with 顯示, 匯出 and 清除), 在候選字視窗顯示組字的 App, and 回復預設設定.
//! 記錄使用統計 is on the 分析 page, as on macOS.

use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement, PathPromptOptions,
    Render, SharedString, Styled, WeakEntity, Window, div,
};

use super::Page;
use super::words::data;
use crate::app::{self, Place};
use crate::theme::size;
use crate::ui::actions::{ButtonKind, push_button, setting_switch_when};
use crate::ui::alert::{AlertButton, alert, notice};
use crate::ui::content::{FACT_GAP, action_row, fact_row, facts, text_input};
use crate::ui::list::{ListConfig, ListItem, ListSort, ListView};
use crate::ui::{self, h, v};

const APPS: &str = "apps";

pub struct AdvancedPage {
    apps: Entity<ListView>,
    seen_apps: Option<Vec<String>>,
    pending_demo: bool,
}

impl AdvancedPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        data::install_sample(cx);
        let page = cx.entity().downgrade();
        let apps = cx.new(|cx| {
            ListView::new(
                ListConfig {
                    id: "apps",
                    noun: "App",
                    sorts: vec![ListSort::Name],
                    empty_title: "沒有 App".into(),
                    empty_text:
                        "每個 App 都直接在文字中顯示組字。少數 App 需要把組字改放在候選字視窗上方；按 ＋ 加入。".into(),
                },
                window,
                cx,
            )
        });
        apps.update(cx, |list, _| {
            list.set_footer(Some("清單中的 App，組字顯示在候選字視窗上方。".into()));
            let add_page = page.clone();
            list.on_add(move |window, cx| open_add_app(&add_page, window, cx));
            let remove_page = page.clone();
            list.on_remove(move |rows, _, cx| {
                let mut apps = composition_apps(cx).unwrap_or_default();
                apps.retain(|app| !rows.iter().any(|row| row.id.as_ref() == app));
                set_composition_apps(apps, &remove_page, None, cx);
            });
        });
        Self { apps, seen_apps: None, pending_demo: true }
    }

    fn refresh(&mut self, select: Option<SharedString>, cx: &mut Context<Self>) {
        let apps = composition_apps(cx);
        if select.is_none() && self.seen_apps.as_ref() == Some(&apps.clone().unwrap_or_default()) {
            return;
        }
        let available = apps.is_some();
        let apps = apps.unwrap_or_default();
        self.seen_apps = Some(apps.clone());
        let rows = apps.iter().map(|app| ListItem::new(app.clone(), app.clone())).collect();
        self.apps.update(cx, |list, cx| {
            list.set_add_enabled(available);
            list.set_items(rows, select.map(|id| [id].into_iter().collect()), cx);
        });
    }

    fn open_demo_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pending_demo) {
            return;
        }
        let page = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            if data::demo("reset").is_some() {
                confirm_reset(window, cx);
            }
            if data::demo("clear-log").is_some() {
                confirm_clear_log(window, cx);
            }
            if data::demo("add-app").is_some() {
                open_add_app(&page, window, cx);
            }
        });
    }

    fn main(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = app::settings(cx);
        let recording = settings.bool("recordsCorrections");
        let count = correction_count(cx);
        let apps = composition_apps(cx).unwrap_or_default();
        v().gap(size::SECTION_GAP)
            .child(
                ui::section("改字記錄")
                    .row(ui::setting_row("recordsCorrections", cx).info("記錄改字", |_, cx| correction_info(cx)))
                    .row(
                        ui::row("row-recordsCorrectionContext", "一併記錄游標前的文字")
                            .subtitle("重播記錄時使用同樣的前文")
                            .anchor("recordsCorrectionContext")
                            .marked(settings.is_customised("recordsCorrectionContext"))
                            .indent(true)
                            .dimmed(!recording)
                            .trailing(setting_switch_when("recordsCorrectionContext", recording, cx)),
                    )
                    .row(action_row(
                        "row-correction-count",
                        format!("{count} 筆記錄"),
                        [
                            push_button(
                                "reveal-log",
                                reveal_label(),
                                ButtonKind::Normal,
                                true,
                                |_, _, cx| reveal_log(cx),
                                cx,
                            )
                            .into_any_element(),
                            push_button(
                                "export-log",
                                "匯出…",
                                ButtonKind::Normal,
                                count > 0,
                                |_, window, cx| export_log(window, cx),
                                cx,
                            )
                            .into_any_element(),
                            push_button(
                                "clear-log",
                                "清除…",
                                ButtonKind::Normal,
                                count > 0,
                                |_, window, cx| confirm_clear_log(window, cx),
                                cx,
                            )
                            .into_any_element(),
                        ],
                        cx,
                    ))
                    .footer("記錄只存在這台電腦；密碼欄位中不記錄。"),
            )
            .child(
                ui::section("相容性").row(
                    ui::row("drill-apps", "在候選字視窗顯示組字的 App")
                        .subtitle("少數 App 需要")
                        .anchor(APPS)
                        .value(if apps.is_empty() { "沒有".to_owned() } else { format!("{} 個", apps.len()) })
                        .on_click(|_, _, cx| app::go(Place::subpage(Page::Advanced, APPS), cx)),
                ),
            )
            .child(ui::section("").row(
                ui::row("row-reset", "回復預設設定").subtitle("自訂詞與記住的字詞會保留").anchor("reset").trailing(
                    push_button(
                        "reset",
                        "回復…",
                        ButtonKind::Normal,
                        true,
                        |_, window, cx| confirm_reset(window, cx),
                        cx,
                    ),
                ),
            ))
            .into_any_element()
    }
}

impl Render for AdvancedPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(None, cx);
        self.open_demo_states(window, cx);
        match app::nav(cx).subpage.as_deref() {
            Some(APPS) => self.apps.clone().into_any_element(),
            _ => self.main(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    (id == APPS).then_some("在候選字視窗顯示組字的 App")
}

// MARK: - 改字記錄

/// What the correction log records (docs/product/DESIGN.md §2.7), with the current keys.
fn correction_info(cx: &App) -> AnyElement {
    let key = |command: &str| data::key_text(command, cx);
    let lists: Vec<String> = std::iter::once("從清單選字".to_owned())
        .chain(key("correctionsList").map(|k| format!("{k} 修正")))
        .chain(key("singleCharacterList").map(|k| format!("{k} 單字")))
        .collect();
    let editing: Vec<String> = key("typeZhuyin")
        .map(|k| format!("{k} 打出注音"))
        .into_iter()
        .chain(key("selectLeft").zip(key("selectRight")).map(|(left, right)| format!("{left} {right} 加入自訂詞")))
        .chain(std::iter::once("在句中修改".to_owned()))
        .collect();
    facts([
        fact_row(IconName::CircleCheck, "只記錄有改字的組字", cx).into_any_element(),
        fact_row(IconName::Keyboard, "打的鍵與按鍵間隔，以及每一步畫面上的字", cx).into_any_element(),
        fact_row(IconName::ListChecks, lists.join("、"), cx).into_any_element(),
        fact_row(IconName::Delete, "Backspace 或 Esc 刪掉後重打；送出後 30 秒內刪掉重打", cx).into_any_element(),
        fact_row(IconName::TextCursorInput, editing.join("、"), cx).into_any_element(),
        fact_row(IconName::TextAlignStart, "游標前的文字只在開啟第二個選項時記錄", cx).into_any_element(),
        fact_row(IconName::Lock, "密碼欄位中不記錄；記錄中沒有 App 的名稱", cx).into_any_element(),
        fact_row(IconName::HardDrive, "存在這台電腦的 corrections.jsonl，可以匯出", cx).into_any_element(),
    ])
    .into_any_element()
}

fn correction_count(cx: &App) -> usize {
    if let Some(service) = app::service(cx) {
        return service.corrections().count();
    }
    data::sample_corrections(cx).unwrap_or(0)
}

/// `corrections.jsonl` (the service's, or where it will be).
fn log_file(cx: &App) -> Option<PathBuf> {
    match app::service(cx) {
        Some(service) => service.corrections().location(),
        None => zt_core::paths::data_dir().map(|dir| dir.join(zt_core::corrections::FILE_NAME)),
    }
}

/// 「在 Finder 中顯示」 on this platform.
fn reveal_label() -> &'static str {
    if cfg!(windows) { "在檔案總管中顯示" } else { "在檔案管理員中顯示" }
}

fn reveal_log(cx: &mut App) {
    let Some(file) = log_file(cx) else { return };
    if file.is_file() {
        cx.reveal_path(&file);
    } else if let Some(dir) = file.parent().filter(|dir| dir.is_dir()) {
        cx.open_with_system(dir);
    }
}

fn export_log(window: &mut Window, cx: &mut App) {
    let Some(service) = app::service(cx).cloned() else { return };
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let chosen = cx.prompt_for_new_path(&home, Some("ZhuyinType 改字記錄.jsonl"));
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(path))) = chosen.await else { return };
            if let Err(error) = service.corrections().export(&path) {
                cx.update(|window, cx| notice("無法匯出", format!("{error:#}"), window, cx)).ok();
            }
        })
        .detach();
}

fn confirm_clear_log(window: &mut Window, cx: &mut App) {
    alert("要清除所有改字記錄嗎？")
        .message("清除後無法復原。")
        .buttons([
            AlertButton::cancel(),
            AlertButton::new("清除", |window, cx| {
                let Some(service) = app::service(cx) else { return };
                if let Err(error) = service.corrections().clear() {
                    notice("無法清除改字記錄", format!("{error:#}"), window, cx);
                }
                cx.refresh_windows();
            })
            .destructive(),
        ])
        .show(window, cx);
}

// MARK: - 回復預設設定

fn confirm_reset(window: &mut Window, cx: &mut App) {
    alert("要回復所有設定的預設值嗎？")
        .message("自訂詞與記住的字詞會保留。")
        .buttons([
            AlertButton::cancel(),
            AlertButton::new("回復", |_, cx| {
                if let Err(error) = app::settings(cx).reset_all() {
                    log::warn!("resetting settings: {error:#}");
                }
                // As on macOS: the keys set on 快捷鍵 and the app list too.
                if let Some(service) = app::service(cx) {
                    if let Err(error) = service.shortcuts().reset(None) {
                        log::warn!("resetting keys: {error:#}");
                    }
                    if let Err(error) = service.set_popup_composition_apps(&[]) {
                        log::warn!("resetting the app list: {error:#}");
                    }
                }
                cx.refresh_windows();
            })
            .destructive(),
        ])
        .show(window, cx);
}

// MARK: - 在候選字視窗顯示組字的 App

/// The apps whose composition shows in the candidate window (executable names); None without the service.
fn composition_apps(cx: &App) -> Option<Vec<String>> {
    match app::service(cx) {
        Some(service) => Some(service.popup_composition_apps()),
        None => data::sample_apps(cx),
    }
}

fn set_composition_apps(apps: Vec<String>, page: &WeakEntity<AdvancedPage>, select: Option<String>, cx: &mut App) {
    if let Some(service) = app::service(cx) {
        if let Err(error) = service.set_popup_composition_apps(&apps) {
            log::warn!("saving the app list: {error:#}");
        }
    } else {
        data::set_sample_apps(apps, cx);
    }
    page.update(cx, |page, cx| page.refresh(select.map(Into::into), cx)).ok();
    cx.refresh_windows();
}

/// An example executable name for this platform.
fn example_app() -> &'static str {
    if cfg!(windows) { "Code.exe" } else { "wezterm" }
}

/// 加入 App: a program's executable name, typed or picked from a file dialog.
fn open_add_app(page: &WeakEntity<AdvancedPage>, window: &mut Window, cx: &mut App) {
    if composition_apps(cx).is_none() {
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(format!("例如：{}", example_app())));
    let first_frame = Rc::new(std::cell::Cell::new(true));
    let field = input.clone();
    let commit = input.clone();
    let page = page.clone();
    alert("加入 App")
        .message("輸入程式的執行檔名稱，或選擇程式檔案。")
        .width(420.)
        .content(move |window, cx| {
            if first_frame.replace(false) {
                field.focus_handle(cx).focus(window, cx);
            }
            let pick = field.clone();
            h().gap(FACT_GAP)
                .child(div().flex_1().child(text_input(&field)))
                .child(
                    push_button(
                        "choose-app",
                        "選擇程式…",
                        ButtonKind::Normal,
                        true,
                        move |_, window, cx| choose_program(&pick, window, cx),
                        cx,
                    )
                    .h(size::H_FIELD),
                )
                .into_any_element()
        })
        .buttons([
            AlertButton::cancel(),
            AlertButton::new("加入", move |_, cx| {
                let name = commit.read(cx).value().trim().to_owned();
                if name.is_empty() {
                    return;
                }
                let mut apps = composition_apps(cx).unwrap_or_default();
                if !apps.contains(&name) {
                    apps.push(name.clone());
                }
                set_composition_apps(apps, &page, Some(name), cx);
            })
            .default(),
        ])
        .show(window, cx);
}

/// Picks a program file and puts its file name in `field`.
fn choose_program(field: &Entity<InputState>, window: &mut Window, cx: &mut App) {
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("加入".into()),
    });
    let field = field.clone();
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = chosen.await else { return };
            let Some(name) =
                paths.first().and_then(|path| path.file_name()).map(|name| name.to_string_lossy().into_owned())
            else {
                return;
            };
            cx.update(|window, cx| field.update(cx, |input, cx| input.set_value(name, window, cx))).ok();
        })
        .detach();
}

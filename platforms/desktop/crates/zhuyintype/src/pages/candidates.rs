//! 「視窗與提示」 page (macOS `CandidatesPane`): one preview at the top drawn with the real candidate window, then the
//! list's settings, its content, 位置, 外觀 (主題 and 透明效果 for the whole app, then the candidate window's own), 動畫
//! and 提示, with the keys of the commands they govern.  The page has no
//! sub-pages.
//!
//! Keys rows show each command's keys as caps and lead to the command's row on the 快捷鍵 page, where they are
//! changed (macOS edits them in place with its shortcut field).

use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div,
};
use serde_json::Value;
use zt_core::settings::{self, Settings};

use super::Page;
use crate::app::{self, Place};
use crate::candidate::preview::UnifiedPreview;
use crate::candidate::style::CurrentKeys;
use crate::theme::{self, size};
use crate::ui::{self, row, section, v};

pub struct CandidatesPage {
    preview: Entity<UnifiedPreview>,
    font_size: Entity<SliderState>,
    _font_size_changes: Subscription,
}

const FONT_KEY: &str = "candidateFontSize";

impl CandidatesPage {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preview = cx.new(UnifiedPreview::new);
        let current = app::settings(cx).number(FONT_KEY) as f32;
        let font_size = cx.new(|_| SliderState::new().min(13.).max(36.).step(1.).default_value(current));
        let subscription = cx.subscribe(&font_size, |_, _, event: &SliderEvent, cx| {
            let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
            let rounded = value.start().round();
            if (rounded - app::settings(cx).number(FONT_KEY) as f32).abs() >= 1. {
                app::set_setting(FONT_KEY, Value::from(rounded as i64), cx);
            }
        });
        Self { preview, font_size, _font_size_changes: subscription }
    }
}

/// The indented 「快捷鍵」 rows of a setting (macOS `CommandKeyRows`): one per command, named after the command when
/// there are several; each shows the command's keys and leads to its row on the 快捷鍵 page.
fn command_key_rows(commands: &[&'static str], keys: &CurrentKeys, settings: &Settings, cx: &App) -> Vec<AnyElement> {
    commands
        .iter()
        .map(|&command| {
            let title = if commands.len() > 1 { command_label(command) } else { "快捷鍵" };
            let labels = keys.all(command);
            let caps = ui::keycap::bindings(labels);
            let customised = app::service(cx).is_some_and(|service| service.shortcuts().is_customised(command));
            let _ = settings;
            row(SharedString::from(format!("keysrow-{command}")), title)
                .indent(true)
                .anchor(format!("keysrow-{command}"))
                .marked(customised)
                .trailing(caps)
                .on_click(move |_, _, cx| app::go(Place::row(Page::Keys, format!("keys-{command}")), cx))
                .into_any_element()
        })
        .collect()
}

/// The command's short name (macOS `ShortcutCommand.label`).
fn command_label(command: &str) -> &'static str {
    match command {
        "singleCharacterList" => "只列一個字",
        "correctionsList" => "列出修正的字",
        "typeZhuyin" => "打出注音本身",
        "nextPage" => "下一頁",
        "previousPage" => "上一頁",
        "selectLeft" => "往左選取",
        "selectRight" => "往右選取",
        "previousCharacter" => "改前一個字",
        "nextCharacter" => "改後一個字",
        _ => "快捷鍵",
    }
}

/// A switch row for a key feature (macOS `FeatureRow`), followed by its keys rows while it is on.
struct Feature {
    key: &'static str,
    title: &'static str,
    subtitle: String,
    commands: &'static [&'static str],
    anchor: &'static str,
    /// Why the feature cannot be used now, and a link to where that is set.
    unavailable: Option<(&'static str, &'static str, Place)>,
}

fn feature_rows(feature: Feature, keys: &CurrentKeys, settings: &Settings, cx: &App) -> Vec<AnyElement> {
    let on = settings.bool(feature.key);
    let line = match &feature.unavailable {
        Some((note, _, _)) => (*note).to_owned(),
        None if !on && feature.key == "feature.zhuyinRowClick" => "關閉時，點注音列不做事".to_owned(),
        None if !on && !feature.commands.is_empty() => {
            format!("關閉時，{} 交給 App", keys.assigned(feature.commands[0]).join("、"))
        }
        None => feature.subtitle.clone(),
    };
    let mut row = row(SharedString::from(format!("row-{}", feature.key)), feature.title)
        .subtitle(line)
        .anchor(feature.anchor)
        .marked(settings.is_customised(feature.key))
        .dimmed(feature.unavailable.is_some())
        .trailing(ui::setting_switch(feature.key, cx));
    if let Some((_, link_title, place)) = feature.unavailable {
        row = row.link(ui::text_link(
            SharedString::from(format!("link-{}", feature.key)),
            format!("在「{link_title}」設定"),
            move |_, _, cx| app::go(place.clone(), cx),
            cx,
        ));
    }
    let mut rows = vec![row.into_any_element()];
    if on {
        rows.extend(command_key_rows(feature.commands, keys, settings, cx));
    }
    rows
}

/// A registry row with the control macOS draws for it, the registry's subtitle and anchor.
fn registry_row(key: &'static str, subtitle: Option<String>, cx: &App) -> ui::Row {
    let row = ui::setting_row(key, cx);
    match subtitle {
        Some(subtitle) => row.subtitle(subtitle),
        None => row,
    }
}

/// 「透明效果」, or why this system has no backdrop (the row then stays off and cannot be changed).
fn transparency_row(cx: &App) -> ui::Row {
    let row = registry_row("transparency", None, cx);
    match zt_core::appearance::backdrop_support() {
        Ok(()) => row,
        Err(reason) => row.subtitle(reason).dimmed(true).trailing(ui::value_text("關閉", cx)),
    }
}

/// Segments for a two-choice setting (排列, 間距).
fn segments(key: &'static str, cx: &App) -> AnyElement {
    let options: Vec<(SharedString, Value)> = settings::descriptor(key)
        .map(|d| d.options.iter().map(|choice| (choice.label.clone().into(), choice.value.clone())).collect())
        .unwrap_or_default();
    ui::segmented(
        SharedString::from(format!("seg-{key}")),
        options,
        app::settings(cx).get(key),
        move |value, _, cx| app::set_setting(key, value, cx),
        cx,
    )
    .into_any_element()
}

impl Render for CandidatesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let settings = app::settings(cx).clone();
        let keys = CurrentKeys::new(&settings, app::service(cx));
        let vertical = settings.string("candidateLayout") != "horizontal";
        let font_size = settings.number(FONT_KEY);
        if (self.font_size.read(cx).value().start() - font_size as f32).abs() >= 0.5 {
            self.font_size.update(cx, |state, cx| state.set_value(font_size as f32, window, cx));
        }

        // 候選字清單
        let layout_subtitle = if vertical {
            "↑ ↓ 移動反白，← → 改前一個或後一個字".to_owned()
        } else {
            let previous = keys.spoken("previousCharacter");
            format!(
                "← → 移動反白，↑ ↓ 翻頁{}",
                if previous.is_empty() { String::new() } else { format!("，{previous} 改前一個字") }
            )
        };
        let layout_row =
            registry_row("candidateLayout", Some(layout_subtitle), cx).trailing(segments("candidateLayout", cx));
        let font_row = row("row-candidateFontSize", "字級")
            .anchor("candidateFontSize")
            .marked(settings.is_customised(FONT_KEY))
            .value(format!("{} pt", font_size as i64))
            .trailing(ui::slider::sample_slider(&self.font_size, "字", "字級", cx));
        let density_row = registry_row("candidateDensity", None, cx).trailing(segments("candidateDensity", cx));
        let per_page = settings.number("candidatesPerPage") as i64;
        let per_page_row = row("row-candidatesPerPage", "每頁的候選字")
            .subtitle(format!("用 {} 選字；Return 選取反白的字", keys.selection()))
            .anchor("selection")
            .marked(settings.is_customised("candidatesPerPage"))
            .value(format!("{per_page} 個"))
            .trailing(ui::stepper::stepper(
                "per-page",
                per_page,
                3,
                9,
                |value, _, cx| app::set_setting("candidatesPerPage", Value::from(value), cx),
                cx,
            ));
        let mut list_rows: Vec<AnyElement> = vec![layout_row.into_any_element()];
        list_rows.extend(command_key_rows(&["previousCharacter", "nextCharacter"], &keys, &settings, cx));
        list_rows.push(font_row.into_any_element());
        list_rows.push(density_row.into_any_element());
        list_rows.push(per_page_row.into_any_element());
        list_rows.push(registry_row("selectionKeys", None, cx).into_any_element());
        list_rows.extend(feature_rows(
            Feature {
                key: "feature.pageKeys",
                title: "翻頁",
                subtitle: "空白也翻到下一頁".into(),
                commands: &["nextPage", "previousPage"],
                anchor: "pageKeys",
                unavailable: None,
            },
            &keys,
            &settings,
            cx,
        ));
        list_rows.push(registry_row("pageAtListEdge", None, cx).into_any_element());

        // 清單的內容
        let zhuyin = keys.spoken("typeZhuyin");
        let corrections = keys.spoken("correctionsList");
        let single = keys.spoken("singleCharacterList");
        let switches: Vec<String> = [(single.clone(), "單字"), (corrections.clone(), "修正")]
            .into_iter()
            .filter(|(key, _)| !key.is_empty())
            .map(|(key, name)| format!("{key} {name}"))
            .collect();
        let mut content_rows: Vec<AnyElement> = vec![registry_row("showsZhuyinHint", None, cx).into_any_element()];
        content_rows.extend(feature_rows(
            Feature {
                key: "feature.zhuyinRowClick",
                title: "點注音列打出注音",
                subtitle: if zhuyin.is_empty() {
                    "打出注音本身".into()
                } else {
                    format!("和按 {zhuyin} 相同；緊接著再按一次或按 Esc 恢復")
                },
                commands: &[],
                anchor: "zhuyinRow",
                unavailable: (!settings.bool("showsZhuyinHint")).then(|| {
                    (
                        "「顯示注音」已關閉，候選字視窗沒有注音列",
                        "視窗與提示",
                        Place::row(Page::Candidates, "showsZhuyin"),
                    )
                }),
            },
            &keys,
            &settings,
            cx,
        ));
        let annotations = if corrections.is_empty() {
            "修正過的字旁邊以灰字標出注音".to_owned()
        } else {
            format!("修正過的字旁邊以灰字標出注音，例如 {corrections} 列出的字")
        };
        content_rows.push(registry_row("showsAnnotations", Some(annotations), cx).into_any_element());
        content_rows.extend(feature_rows(
            Feature {
                key: "feature.singleCharacterList",
                title: "只列一個字",
                subtitle: "清單只列游標處的一個字；再按一次還原".into(),
                commands: &["singleCharacterList"],
                anchor: "single",
                unavailable: None,
            },
            &keys,
            &settings,
            cx,
        ));
        let switches_row = row("row-showsListSwitches", "單字與修正按鈕")
            .subtitle(if switches.is_empty() {
                "兩種清單都已關閉".to_owned()
            } else {
                format!("清單下方的 {}；關閉時只在使用中顯示", switches.join("、"))
            })
            .anchor("showsListSwitches")
            .marked(settings.is_customised("showsListSwitches"))
            .dimmed(switches.is_empty())
            .trailing(
                // Off while both lists are off (macOS disables it): a cover takes the clicks.
                div()
                    .relative()
                    .child(ui::setting_switch("showsListSwitches", cx))
                    .when(switches.is_empty(), |this| this.child(div().absolute().inset_0().occlude())),
            );
        content_rows.push(switches_row.into_any_element());

        let mode = keys.mode_key();
        let toast_subtitle = match &mode {
            Some(key) => format!("按 {key} 切換時，在游標旁顯示「中」或「英」"),
            None => "切換時在游標旁顯示「中」或「英」".to_owned(),
        };

        // The shell's content column takes its children's unwrapped text width as its minimum (it is a flex item
        // without `min_w_0`), so long subtitles would widen it past a narrow window: cap the page at the room there is.
        let room = (window.viewport_size().width - size::SIDEBAR).min(size::CONTENT_MAX) - size::PAGE_PADDING_X * 2.;
        v().w_full()
            .min_w_0()
            .max_w(room)
            .gap(size::SECTION_GAP)
            .child(
                // One preview at the top, with the real window; it plays its states, and a change below switches it
                // to the state that shows it.  It scrolls away like the rest.
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .p(size::ROW_PADDING_X)
                    .rounded(size::R_LG)
                    .bg(p.card)
                    .border_1()
                    .border_color(p.separator)
                    .child(self.preview.clone()),
            )
            .child(section("候選字清單").rows(list_rows))
            .child(section("清單的內容").rows(content_rows))
            .child(section("位置").row(registry_row("candidatePlacement", None, cx)))
            .child(section("外觀").row(registry_row("theme", None, cx)).row(transparency_row(cx)).row(registry_row(
                "panelAppearance",
                None,
                cx,
            )))
            .child(section("動畫").row(registry_row("panelAnimation", None, cx)))
            .child(section("提示").row(registry_row("showModeToast", Some(toast_subtitle), cx)).footer(ui::text_link(
                "keys-overview",
                "在快捷鍵總覽中查看全部按鍵",
                |_, _, cx| app::go(Place::page(Page::Keys), cx),
                cx,
            )))
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(_id: &str) -> Option<&'static str> {
    None
}

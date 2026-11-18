//! 「候選字視窗」 page (SETTINGS_IA.md §2; macOS `CandidatesPane`): one preview at the top drawn with the real candidate
//! window, then the list's settings, 翻頁, the list's content and 位置與動畫, with the keys of the commands they govern
//! edited in place.  The colours are on 色彩.  The page has no sub-pages.

use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div,
};
use serde_json::Value;
use zt_core::settings;

use super::Page;
use crate::app;
use crate::candidate::preview::UnifiedPreview;
use crate::candidate::style::CurrentKeys;
use crate::theme::{self, size};
use crate::ui::current_keys::Command;
use crate::ui::feature::{command_key_rows, feature_row, keys_overview_link};
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

/// A registry row with the control macOS draws for it, the registry's subtitle and anchor.
fn registry_row(key: &'static str, subtitle: Option<String>, cx: &App) -> ui::Row {
    let row = ui::setting_row(key, cx);
    match subtitle {
        Some(subtitle) => row.subtitle(subtitle),
        None => row,
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
        list_rows.extend(
            command_key_rows(&[Command::PreviousCharacter, Command::NextCharacter], false, cx)
                .into_iter()
                .map(IntoElement::into_any_element),
        );
        list_rows.push(font_row.into_any_element());
        list_rows.push(density_row.into_any_element());
        list_rows.push(per_page_row.into_any_element());
        list_rows.push(registry_row("selectionKeys", None, cx).into_any_element());
        let mut paging_rows = feature_row("feature.pageKeys").rows(cx);
        paging_rows.push(registry_row("pageAtListEdge", None, cx).into_any_element());

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
        let zhuyin_subtitle = if zhuyin.is_empty() {
            "打出注音本身".to_owned()
        } else {
            format!("和按 {zhuyin} 相同；緊接著再按一次或按 Esc 恢復")
        };
        content_rows.extend(feature_row("feature.zhuyinRowClick").subtitle(zhuyin_subtitle).rows(cx));
        let annotations = if corrections.is_empty() {
            "修正過的字旁邊以灰字標出注音".to_owned()
        } else {
            format!("修正過的字旁邊以灰字標出注音，例如 {corrections} 列出的字")
        };
        content_rows.push(registry_row("showsAnnotations", Some(annotations), cx).into_any_element());
        content_rows.extend(feature_row("feature.singleCharacterList").rows(cx));
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
            .child(section("翻頁").rows(paging_rows))
            .child(section("清單的內容").rows(content_rows))
            .child(
                section("位置與動畫")
                    .row(registry_row("candidatePlacement", None, cx))
                    .row(registry_row("panelAnimation", None, cx))
                    .footer(keys_overview_link(Page::Candidates, cx)),
            )
    }
}

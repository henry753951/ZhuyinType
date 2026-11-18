//! 「修正」 page and its sub-page 模糊音 (`fuzzy`), with the ⓘ cards of 按錯鍵修正 and 模糊音 (SETTINGS_IA.md §2; macOS
//! `CorrectionsPane`, `FuzzyPage`; `TypoInfo`, `FuzzyInfo`, `OptionDemos`): 按錯鍵 (the three sources of automatic
//! corrections), 修正過的字 (seeing, undoing and listing them) and 改字 (correcting a choice yourself).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};

use super::Page;
use crate::app;
use crate::theme::{self, size, space, text, weight};
use crate::ui::current_keys::{Command, CurrentKeys};
use crate::ui::demo::{DemoFrame, InfoContent, Segment, block, mini_demo};
use crate::ui::feature::{drill_row, feature_row, keys_overview_link};
use crate::ui::{h, section, setting_row, v};

pub struct CorrectionsPage;

impl CorrectionsPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for CorrectionsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match app::nav(cx).subpage.as_deref() {
            Some("fuzzy") => fuzzy_page(cx),
            _ => corrections_page(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    (id == "fuzzy").then_some("模糊音")
}

const FUZZY: [&str; 5] = ["fuzzy.zZh", "fuzzy.cCh", "fuzzy.sSh", "fuzzy.enEng", "fuzzy.anAng"];

fn corrections_page(cx: &App) -> AnyElement {
    let settings = app::settings(cx);
    let fuzzy = FUZZY.iter().filter(|key| settings.bool(key)).count();
    v().gap(size::SECTION_GAP)
        .child(
            section("按錯鍵")
                .row(setting_row("typoTolerance", cx).info("按錯鍵修正", |_, cx| typo_info(cx)))
                .row(setting_row("readingChannel", cx))
                .row(drill_row(
                    Page::Corrections,
                    "fuzzy",
                    "模糊音",
                    "ㄗ／ㄓ、ㄣ／ㄥ 等容易混的音",
                    if fuzzy == 0 { "關閉".to_owned() } else { format!("已開啟 {fuzzy} 項") },
                )),
        )
        .child(
            section("修正過的字")
                .row(setting_row("showCorrections", cx))
                .row(setting_row("undoCorrectionWithBackspace", cx))
                .rows(feature_row("feature.correctionsList").rows(cx)),
        )
        .child(
            section("改字")
                .rows(feature_row("feature.typeZhuyin").rows(cx))
                .rows(feature_row("feature.deleteAfterCommit").rows(cx))
                .footer(keys_overview_link(Page::Corrections, cx)),
        )
        .into_any_element()
}

fn fuzzy_page(cx: &App) -> AnyElement {
    v().gap(size::SECTION_GAP)
        .child(
            section("")
                .bare()
                .first()
                .row(block(mini_demo("fuzzy-page", fuzzy_frames())))
                .footer("開啟的兩個音互相通用；兩個詞同樣常用時，讀音完全符合的排在前面。"),
        )
        .child(section("").rows(FUZZY.iter().map(|key| setting_row(key, cx))))
        .into_any_element()
}

// MARK: - Info cards

/// The ⓘ cards of this page by id, for `--popover` (screenshots): the title and what the card shows.
pub fn info(id: &str) -> Option<(&'static str, InfoContent)> {
    Some(match id {
        "typo" => ("按錯鍵修正", |_, cx| typo_info(cx)),
        "fuzzy" => ("模糊音", |_, cx| fuzzy_info(cx)),
        _ => return None,
    })
}

fn note(text_: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().text_size(text::SUBTITLE).text_color(theme::palette(cx).text_secondary).child(text_.into())
}

/// A key next to the intended one, corrected; then the list of corrections (macOS `OptionDemos.typo`).  Without the
/// corrections key (its feature off) the demo ends at the correction.
fn typo_frames(corrections_key: Option<String>) -> Vec<DemoFrame> {
    let start = || Segment::document("早安，");
    let mut frames = vec![
        DemoFrame::new([start(), Segment::marked("ㄍ")], "想打 ㄐ，按到旁邊的 ㄍ", 1.2).keys(["ㄍ"]),
        DemoFrame::new([start(), Segment::marked("ㄍㄧㄣ")], "想打 ㄐ，按到旁邊的 ㄍ", 0.9).keys(["ㄣ"]),
        DemoFrame::new([start(), Segment::marked("今")], "自動改成「今」", 1.4).keys(["空白"]),
    ];
    if let Some(key) = corrections_key {
        frames.push(DemoFrame::new([start(), Segment::focus("今")], "列出修正的字", 2.0).keys([key]).list([
            "今 ㄐㄧㄣ",
            "金 ㄐㄧㄣ",
            "斤 ㄐㄧㄣ",
        ]));
    }
    frames
}

fn typo_info(cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    let current = app::settings(cx).get("typoTolerance");
    let key = CurrentKeys::new(cx).text(Command::CorrectionsList);
    let descriptor = zt_core::settings::descriptor("typoTolerance");
    let options = descriptor.map(|d| d.options.clone()).unwrap_or_default();
    v().gap(space::S12)
        .child(mini_demo("typo", typo_frames(key.clone())).resting(2))
        .child(v().gap(space::S6).children(options.into_iter().map(|choice| {
            let selected = choice.value == current;
            let detail = descriptor.and_then(|d| d.subtitle(&choice.value)).unwrap_or_default().to_owned();
            h().items_start()
                .gap(space::S8)
                .text_size(text::SUBTITLE)
                .child(
                    div()
                        .w(space::S40)
                        .flex_shrink_0()
                        .text_color(p.text)
                        .when(selected, |this| this.font_weight(weight::SEMIBOLD))
                        .child(choice.label),
                )
                .child(div().flex_1().min_w(px(0.)).text_color(p.text_secondary).child(detail))
        })))
        .child(note(
            key.map(|key| format!("照按鍵打出的字優先；修正過的字也在 {key} 的清單中。"))
                .unwrap_or_else(|| "照按鍵打出的字優先。".to_owned()),
            cx,
        ))
        .into_any_element()
}

/// ㄙㄨ ㄈㄨˊ typed, 舒服 found with 「ㄙ 與 ㄕ」 on (macOS `OptionDemos.fuzzy`).
fn fuzzy_frames() -> Vec<DemoFrame> {
    let before = || Segment::document("今天很");
    vec![
        DemoFrame::new([before(), Segment::marked("ㄙ")], "打 ㄙㄨ ㄈㄨˊ", 0.5).keys(["ㄙ"]),
        DemoFrame::new([before(), Segment::marked("ㄙㄨ")], "打 ㄙㄨ ㄈㄨˊ", 0.5).keys(["ㄨ"]),
        DemoFrame::new([before(), Segment::marked("ㄙㄨ"), Segment::marked("ㄈ")], "打 ㄙㄨ ㄈㄨˊ", 0.4).keys(["ㄈ"]),
        DemoFrame::new([before(), Segment::marked("ㄙㄨ"), Segment::marked("ㄈㄨ")], "打 ㄙㄨ ㄈㄨˊ", 0.7).keys(["ㄨ"]),
        DemoFrame::new([before(), Segment::marked("舒服")], "開啟「ㄙ 與 ㄕ」：也打得出「舒服」", 2.2).keys(["ˊ"]),
    ]
}

fn fuzzy_info(cx: &App) -> AnyElement {
    v().gap(space::S12)
        .child(mini_demo("fuzzy-info", fuzzy_frames()))
        .child(note("兩個詞同樣常用時，讀音完全符合的排在前面。", cx))
        .into_any_element()
}

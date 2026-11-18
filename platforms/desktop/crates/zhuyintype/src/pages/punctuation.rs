//! 「標點與符號」 page (SETTINGS_IA.md §2; macOS `PunctuationPane`): 標點按法 and 切換標點全半形 with their ⓘ
//! cards, 符號與 Emoji, the punctuation typed with Ctrl and Alt, and the punctuation keys that cannot change
//! (`PunctuationTable`, `PunctuationWidthInfo`, `OptionDemos`).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};

use super::Page;
use crate::app;
use crate::theme::{self, size, space, text, weight};
use crate::ui::current_keys::{Command, CurrentKeys};
use crate::ui::demo::{DemoFrame, InfoContent, Segment, mini_demo};
use crate::ui::feature::{command_row, feature_row, keys_overview_link};
use crate::ui::{h, key_combo, row, section, setting_row, v};

pub struct PunctuationPage;

impl PunctuationPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for PunctuationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v().gap(size::SECTION_GAP)
            .child(
                section("標點")
                    .row(setting_row("punctuationKeys", cx).info("標點按法", |_, cx| punctuation_table(cx)))
                    .row(
                        command_row(
                            Command::TogglePunctuationWidth,
                            "切換標點全半形",
                            "中文預設全形；英數與空白維持半形",
                            cx,
                        )
                        .info("標點全半形", |_, cx| punctuation_width_info(cx)),
                    ),
            )
            .child(section("符號與 Emoji").rows(feature_row("feature.symbolPicker").rows(cx)))
            .child(
                section("修飾鍵打標點")
                    .rows(feature_row("feature.controlPunctuation").rows(cx))
                    .rows(feature_row("feature.optionSymbols").rows(cx)),
            )
            .child(
                section("固定的按鍵")
                    .row(row("brackets", "「 」 、").subtitle("固定").trailing(key_combo(["[", "]", "'"])))
                    .row(row("similar-punctuation", "列出同類的標點").subtitle("在標點上按").trailing(key_combo(["↓"])))
                    .footer(keys_overview_link(Page::Punctuation, cx)),
            )
    }
}

// MARK: - Info cards

/// The ⓘ cards of this page by id, for `--popover` (screenshots): the title and what the card shows.
pub fn info(id: &str) -> Option<(&'static str, InfoContent)> {
    Some(match id {
        "punctuation" => ("標點按法", |_, cx| punctuation_table(cx)),
        "punctuation-width" => ("標點全半形", |_, cx| punctuation_width_info(cx)),
        _ => return None,
    })
}

fn note(text_: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().text_size(text::SUBTITLE).text_color(theme::palette(cx).text_secondary).child(text_.into())
}

/// What Shift, Ctrl and Alt with a symbol key type, for both 標點按法 settings (macOS `PunctuationTable`).
fn punctuation_table(cx: &App) -> AnyElement {
    const ROWS: &[(&[&str], &str, &str)] = &[
        (&["⇧", ","], "＜", "，"),
        (&["⇧", "/"], "？", "？"),
        (&["⇧", "1"], "！", "！"),
        (&["⇧", "-"], "＿", "——"),
        (&["Ctrl", ","], "，", "，"),
        (&["Alt", ","], ",", ","),
    ];
    let p = theme::palette(cx);
    let windows = app::settings(cx).string("punctuationKeys") != "mac";
    // Three equal columns across the card; the current setting's column in the main text colour.
    let cell = || div().flex_1().min_w(px(0.));
    let column = |title: &'static str, current: bool| {
        cell()
            .when(current, |this| this.font_weight(weight::SEMIBOLD).text_color(p.text))
            .when(!current, |this| this.text_color(p.text_secondary))
            .child(title)
    };
    let output = |text_: &'static str, current: bool| {
        cell().text_size(text::HEADLINE).text_color(if current { p.text } else { p.text_tertiary }).child(text_)
    };
    v().w_full()
        .gap(space::S12)
        .child(
            v().w_full()
                .gap(space::S8)
                .child(
                    h().w_full()
                        .gap(space::S16)
                        .text_size(text::SUBTITLE)
                        .child(cell().text_color(p.text_secondary).child("按鍵"))
                        .child(column("Windows", windows))
                        .child(column("macOS", !windows)),
                )
                .children(ROWS.iter().map(|(keys, on_windows, on_mac)| {
                    h().w_full()
                        .gap(space::S16)
                        .child(cell().child(key_combo(keys.iter().copied())))
                        .child(output(on_windows, windows))
                        .child(output(on_mac, !windows))
                })),
        )
        .child(note("表中為全形標點。可切換標點全半形，Alt 加符號鍵輸入半形符號。", cx))
        .into_any_element()
}

/// Full-width punctuation by default; the toggle key switches to half-width and back (macOS
/// `OptionDemos.punctuationWidth`).  Without a toggle key the demo shows the default only.
fn punctuation_width_frames(toggle: Option<Vec<SharedString>>) -> Vec<DemoFrame> {
    let full = DemoFrame::new([Segment::marked("＃")], "中文預設全形標點", 1.5).keys(["⇧", "3"]);
    let Some(toggle) = toggle else { return vec![full] };
    vec![
        full,
        DemoFrame::new([Segment::marked("＃")], "切換為半形，保留已打的字", 1.5).keys(toggle.clone()),
        DemoFrame::new([Segment::marked("＃#")], "之後輸入半形標點", 1.5).keys(["⇧", "3"]),
        DemoFrame::new([Segment::marked("＃#")], "再按一次切回全形", 1.5).keys(toggle),
    ]
}

fn punctuation_width_info(cx: &App) -> AnyElement {
    let toggle = CurrentKeys::new(cx).caps(Command::TogglePunctuationWidth);
    v().gap(space::S12)
        .child(mini_demo("punctuation-width", punctuation_width_frames(toggle)))
        .child(note("只切換標點；英文字母、數字與空白維持半形。英文模式使用半形。", cx))
        .into_any_element()
}

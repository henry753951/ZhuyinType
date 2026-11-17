//! 「打字」 page and its sub-pages 標點與符號 (`punctuation`) and 所有打字的按鍵 (`typingKeys`), with the ⓘ cards of
//! 空白鍵, Esc, 標點按法 and 標點全半形 (macOS `TypingPane`, `PunctuationPage`, `TypingKeysPage` in
//! `SettingsPages.swift`; `SpaceInfo`, `EscapeInfo`, `PunctuationTable`, `PunctuationWidthInfo`, `OptionDemos`).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};

use super::Page;
use crate::app::{self, Place};
use crate::theme::{self, size, space, text, weight};
use crate::ui::current_keys::{Command, CurrentKeys};
use crate::ui::demo::{DemoFrame, InfoContent, Segment, choice_demo, mini_demo, text_block};
use crate::ui::feature::{command_feature_row, command_row, drill_row, feature_row, keys_overview_link, place_link};
use crate::ui::{h, key_combo, row, section, setting_row, v};

pub struct TypingPage;

impl TypingPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for TypingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match app::nav(cx).subpage.as_deref() {
            Some("punctuation") => punctuation_page(cx),
            Some("typingKeys") => typing_keys_page(cx),
            _ => typing_page(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    match id {
        "punctuation" => Some("標點與符號"),
        "typingKeys" => Some("所有打字的按鍵"),
        _ => None,
    }
}

const WIDTH_DETAIL: &str = "中文預設全形標點；英數、空白與英文模式維持半形";
const SPACE_DETAIL: &str = "在組字中插入半形空白，繼續打字";

fn typing_page(cx: &App) -> AnyElement {
    let settings = app::settings(cx);
    let punctuation_off =
        ["feature.controlPunctuation", "feature.optionSymbols"].iter().filter(|key| !settings.bool(key)).count();
    let shift_switch = settings.bool("shiftSwitch");
    v().gap(size::SECTION_GAP)
        .child(
            section("空白與 Esc")
                .row(setting_row("spaceAction", cx).info("空白鍵", |_, cx| space_info(cx)))
                .row(setting_row("escape", cx).info("Esc", |_, cx| escape_info(cx))),
        )
        .child(
            section("標點")
                .row(setting_row("punctuationKeys", cx).info("標點按法", |_, cx| punctuation_table(cx)))
                .row(
                    command_row(Command::TogglePunctuationWidth, WIDTH_DETAIL, cx)
                        .info("標點全半形", |_, cx| punctuation_width_info(cx)),
                )
                .row(command_row(Command::InsertSpace, SPACE_DETAIL, cx))
                .row(drill_row(
                    Page::Typing,
                    "punctuation",
                    "標點與符號的按鍵與說明",
                    "中文預設全形；英數與空白維持半形",
                    if punctuation_off == 0 {
                        "全部開啟".to_owned()
                    } else {
                        format!("已關閉 {punctuation_off} 項")
                    },
                )),
        )
        .child(
            section("中英切換")
                .row(setting_row("shiftSwitch", cx))
                .when(shift_switch, |this| this.row(setting_row("shiftToggle", cx).indent(true).anchor("shiftSide")))
                .rows(feature_row("feature.capsLockEnglish").rows(cx))
                .row(
                    row("mode-toast", "中英提示")
                        .subtitle("切換時在游標旁顯示「中」或「英」")
                        .value(if settings.bool("showModeToast") { "顯示" } else { "不顯示" })
                        .on_click(|_, _, cx| app::go(Place::row(Page::Candidates, "modeToast"), cx)),
                )
                .row(setting_row("startInEnglish", cx)),
        )
        .child(
            section("編輯組字")
                .rows(
                    command_feature_row("送出並換行", "送出組字，App 再換行")
                        .commands([Command::CommitWithNewline])
                        .anchor("newline")
                        .rows(cx),
                )
                .rows(feature_row("feature.deleteWord").rows(cx))
                .row(drill_row(
                    Page::Typing,
                    "typingKeys",
                    "所有打字的按鍵",
                    "注音鍵、聲調、Backspace、方向鍵等固定的按鍵",
                    "",
                ))
                .footer(keys_overview_link(cx)),
        )
        .into_any_element()
}

fn punctuation_page(cx: &App) -> AnyElement {
    let windows = app::settings(cx).string("punctuationKeys") != "mac";
    v().gap(size::SECTION_GAP)
        .child(
            section("標點全半形")
                .row(
                    command_row(Command::TogglePunctuationWidth, WIDTH_DETAIL, cx)
                        .info("標點全半形", |_, cx| punctuation_width_info(cx)),
                )
                .row(command_row(Command::InsertSpace, SPACE_DETAIL, cx))
                .footer("中文預設全形，英數與空白維持半形。英文模式使用半形。"),
        )
        .child(
            section(format!("標點按法：{}", if windows { "Windows" } else { "macOS" }))
                .row(text_block(punctuation_table(cx)))
                .footer(place_link(
                    "punctuation-keys-link",
                    "標點按法在「打字」設定",
                    Place::row(Page::Typing, "punctuationKeys"),
                    cx,
                )),
        )
        .child(
            section("可以關閉的按鍵")
                .rows(feature_row("feature.controlPunctuation").rows(cx))
                .rows(feature_row("feature.optionSymbols").rows(cx))
                .rows(feature_row("feature.symbolPicker").rows(cx)),
        )
        .child(
            section("固定的按鍵")
                .row(row("brackets", "「 」 、").subtitle("固定").trailing(key_combo(["[", "]", "'"])))
                .row(row("similar-punctuation", "列出同類的標點").subtitle("在標點上按").trailing(key_combo(["↓"]))),
        )
        .into_any_element()
}

/// The typing keys that cannot change, with their reasons.
fn typing_keys_page(_cx: &App) -> AnyElement {
    const KEYS: &[(&str, &[&str], &str)] = &[
        ("拼注音", &["注音鍵"], "注音鍵的位置依標準注音鍵盤"),
        ("聲調：二聲、三聲、四聲、輕聲", &["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"], "標準注音鍵盤的聲調鍵"),
        ("一聲", &["空白"], "打注音時，空白完成這個音"),
        ("送出", &["Return"], "送出整段組字"),
        ("刪掉注音符號，或游標前的字", &["⌫"], "和一般的刪除鍵相同"),
        ("刪掉游標後的字", &["Del"], "和一般的刪除鍵相同"),
        ("在組字中移動游標", &["←", "→"], "Home、End 移到開頭或結尾"),
    ];
    v().gap(size::SECTION_GAP)
        .child(
            section("")
                .first()
                .rows(KEYS.iter().enumerate().map(|(index, (title, keys, detail))| {
                    row(SharedString::from(format!("fixed-{index}")), *title)
                        .subtitle(*detail)
                        .trailing(key_combo(keys.iter().copied()))
                }))
                .footer("這些按鍵用在打字本身，無法更改。"),
        )
        .into_any_element()
}

// MARK: - Info cards

/// The ⓘ cards of this page by id, for `--popover` (screenshots): the title and what the card shows.
pub fn info(id: &str) -> Option<(&'static str, InfoContent)> {
    Some(match id {
        "space" => ("空白鍵", |_, cx| space_info(cx)),
        "escape" => ("Esc", |_, cx| escape_info(cx)),
        "punctuation" => ("標點按法", |_, cx| punctuation_table(cx)),
        "punctuation-width" => ("標點全半形", |_, cx| punctuation_width_info(cx)),
        _ => return None,
    })
}

fn note(text_: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().text_size(text::SUBTITLE).text_color(theme::palette(cx).text_secondary).child(text_.into())
}

fn start() -> Segment {
    Segment::document("早安，")
}

/// What 空白 does after a syllable, for each choice of 空白鍵 (macOS `OptionDemos.space`).
fn space_frames(action: &str) -> Vec<DemoFrame> {
    let mut frames = vec![
        DemoFrame::new([start(), Segment::marked("今"), Segment::marked("ㄊ")], "打 ㄐㄧㄣ ㄊㄧㄢ", 0.35).keys(["ㄊ"]),
        DemoFrame::new([start(), Segment::marked("今"), Segment::marked("ㄊㄧㄢ")], "打 ㄐㄧㄣ ㄊㄧㄢ", 1.0)
            .keys(["ㄢ"]),
        DemoFrame::new([start(), Segment::marked("今天")], "完成一聲", 1.2).keys(["空白"]),
    ];
    match action {
        "candidates" => frames.push(
            DemoFrame::new([start(), Segment::focus("今天")], "再按一次：開啟候選字視窗", 2.0)
                .keys(["空白"])
                .list(["今天", "金田", "今"]),
        ),
        "insert" => frames.extend([
            DemoFrame::new([start(), Segment::marked("今天 ")], "再按一次：空白加進組字，還不送出", 1.5).keys(["空白"]),
            DemoFrame::new([start(), Segment::marked("今天 "), Segment::marked("ㄏ")], "繼續打字", 0.5).keys(["ㄏ"]),
            DemoFrame::new([start(), Segment::marked("今天 好")], "繼續打字", 1.0).keys(["ˇ"]),
            DemoFrame::new([Segment::document("早安，今天 好")], "按 Return：連同空白一起送出", 2.0).keys(["Return"]),
        ]),
        _ => frames
            .push(DemoFrame::new([Segment::document("早安，今天 ")], "再按一次：送出並輸入空白", 2.0).keys(["空白"])),
    }
    frames
}

fn space_info(cx: &App) -> AnyElement {
    let current = app::settings(cx).get("spaceAction");
    let options = zt_core::settings::descriptor("spaceAction").map(|d| d.options.clone()).unwrap_or_default();
    v().gap(space::S16)
        .child(note(
            "打注音時，空白是一聲；打完以後，空白依這個設定。清單開著時，空白翻到下一頁；選字用選字鍵或 Return。",
            cx,
        ))
        .children(options.into_iter().map(|choice| {
            let action = choice.value.as_str().unwrap_or_default().to_owned();
            choice_demo(
                choice.label,
                choice.value == current,
                mini_demo(format!("space-{action}"), space_frames(&action)),
            )
        }))
        .into_any_element()
}

/// Esc halfway through a syllable, then again (macOS `OptionDemos.escape`).
fn escape_frames(clears_composition: bool) -> Vec<DemoFrame> {
    vec![
        DemoFrame::new([start(), Segment::marked("今天"), Segment::marked("ㄑㄧ")], "打到一半", 1.1).keys(["ㄧ"]),
        DemoFrame::new([start(), Segment::marked("今天")], "清除注音", 1.2).keys(["Esc"]),
        if clears_composition {
            DemoFrame::new([start()], "再按一次：清除整段組字", 2.0).keys(["Esc"])
        } else {
            DemoFrame::new([start(), Segment::marked("今天")], "再按一次：組字保留", 2.0).keys(["Esc"])
        },
    ]
}

fn escape_info(cx: &App) -> AnyElement {
    let clears = app::settings(cx).string("escape") != "reading";
    v().gap(space::S16)
        .child(choice_demo("清除整段組字", clears, mini_demo("escape-composition", escape_frames(true))))
        .child(choice_demo("只清除注音", !clears, mini_demo("escape-reading", escape_frames(false))))
        .into_any_element()
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

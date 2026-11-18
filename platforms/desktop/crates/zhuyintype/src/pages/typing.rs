//! 「打字」 page and its sub-page 所有打字的按鍵 (`typingKeys`), with the ⓘ cards of 空白鍵 and Esc (macOS
//! `TypingPane`, `TypingKeysPage` in `SettingsPages.swift`; `SpaceInfo`, `EscapeInfo`, `OptionDemos`).  The keys of
//! 編輯組字 are edited right here (SETTINGS_IA.md §2).

use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div};

use super::Page;
use crate::app;
use crate::theme::{self, size, space, text};
use crate::ui::current_keys::Command;
use crate::ui::demo::{DemoFrame, InfoContent, Segment, choice_demo, mini_demo};
use crate::ui::feature::{command_row, drill_row, feature_row, keys_overview_link};
use crate::ui::{key_combo, row, section, setting_row, v};

pub struct TypingPage;

impl TypingPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for TypingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match app::nav(cx).subpage.as_deref() {
            Some("typingKeys") => typing_keys_page(cx),
            _ => typing_page(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    match id {
        "typingKeys" => Some("所有打字的按鍵"),
        _ => None,
    }
}

fn typing_page(cx: &App) -> AnyElement {
    v().gap(size::SECTION_GAP)
        .child(
            section("空白與 Esc")
                .row(setting_row("spaceAction", cx).info("空白鍵", |_, cx| space_info(cx)))
                .row(setting_row("escape", cx).info("Esc", |_, cx| escape_info(cx))),
        )
        .child(
            section("編輯組字")
                .row(command_row(Command::CommitWithNewline, "送出並換行", "送出組字，App 再換行", cx))
                .rows(feature_row("feature.deleteWord").rows(cx))
                .row(command_row(Command::InsertSpace, "在組字中放一個空白", "插入半形空白，繼續打字", cx))
                .row(drill_row(
                    Page::Typing,
                    "typingKeys",
                    "所有打字的按鍵",
                    "注音鍵、聲調、Backspace、方向鍵等固定的按鍵",
                    "",
                ))
                .footer(keys_overview_link(Page::Typing, cx)),
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

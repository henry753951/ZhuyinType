//! 「選字」 page and its sub-page 前文 (`context`), with the ⓘ card of 游標在句中時改 (macOS `ChoosingPane`,
//! `ContextPane.swift`; `AnchorInfo`, `OptionDemos`): 轉換 (游標在句中時改, 台／臺, 繁／簡), 前文 and 聯想詞.

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement, Pixels,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use serde_json::Value;

use super::Page;
use crate::app;
use crate::theme::{self, size, space, text};
use crate::ui::demo::{
    DemoFrame, InfoContent, Segment, SegmentStyle, block, choice_demo, demo_line, field, mini_demo, play_pause,
    story_clock,
};
use crate::ui::feature::drill_row;
use crate::ui::{h, row, section, segmented, setting_row, v};

pub struct ChoosingPage;

impl ChoosingPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ChoosingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match app::nav(cx).subpage.as_deref() {
            Some("context") => context_page(cx),
            _ => choosing_page(cx),
        }
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    match id {
        "context" => Some("前文"),
        _ => None,
    }
}

/// Context numbers of the input controller (macOS `ControllerCore.contextRead`, `contextKept`, `contextWindow`).
// Read them from zt_core's input controller once it exposes them.
const CONTEXT_READ: usize = 128;
const CONTEXT_KEPT: usize = 255;
const CONTEXT_MINUTES: usize = 1;

fn choosing_page(cx: &App) -> AnyElement {
    let settings = app::settings(cx);
    v().gap(size::SECTION_GAP)
        .child(
            section("轉換")
                .row(setting_row("candidateAnchor", cx).info("游標在句中時改", |_, cx| anchor_info(cx)))
                .row(setting_row("characterForms", cx).trailing(segments("characterForms", cx)))
                .row(setting_row("outputScript", cx).trailing(segments("outputScript", cx))),
        )
        .child(section("前文").row(drill_row(
            Page::Choosing,
            "context",
            "參考游標前的文字",
            "依前文選字；只在記憶體中",
            if settings.bool("usesPrecedingText") { "開啟" } else { "關閉" },
        )))
        .child(section("聯想詞").row(setting_row("nextWordSuggestions", cx)))
        .into_any_element()
}

/// A choice setting as segments (台／臺).
fn segments(key: &'static str, cx: &App) -> impl IntoElement {
    let options = zt_core::settings::descriptor(key)
        .map(|d| {
            d.options.iter().map(|choice| (SharedString::from(choice.label.clone()), choice.value.clone())).collect()
        })
        .unwrap_or_default();
    segmented(
        format!("seg-{key}"),
        options,
        app::settings(cx).get(key),
        move |value: Value, _, cx| app::set_setting(key, value, cx),
        cx,
    )
}

fn context_page(cx: &App) -> AnyElement {
    let on = app::settings(cx).bool("usesPrecedingText");
    let fact = |id: &'static str, title: &'static str, value: String| row(id, title).value(value).wrap_value(true);
    v().gap(size::SECTION_GAP)
        .child(section("").bare().first().row(block(ContextIllustration)))
        .child(section("").row(setting_row("usesPrecedingText", cx)))
        .child(
            section("讀取與保留")
                .row(fact(
                    "fact-read",
                    "讀取",
                    if on { format!("游標前最多 {CONTEXT_READ} 個字") } else { "不讀取".to_owned() },
                ))
                .row(fact(
                    "fact-kept",
                    "送出的字",
                    if on {
                        format!("接在前文後面，保留最近 {CONTEXT_KEPT} 個字")
                    } else {
                        "不保留前文".to_owned()
                    },
                ))
                .when(on, |this| {
                    this.row(fact(
                        "fact-app",
                        "同一個 App",
                        format!("沿用最近 {CONTEXT_MINUTES} 分鐘的前文，輸入框清空時也保留"),
                    ))
                })
                .row(fact("fact-secure", "密碼欄位", "不讀取，也不學習".to_owned()))
                .row(fact("fact-memory", "存放位置", "只在記憶體中".to_owned())),
        )
        .into_any_element()
}

// MARK: - 前文: the illustration

/// Four short scenes: the text before the cursor being read, then used; what was sent carrying over to another field
/// of the same app; a password field that is never read (macOS `ContextIllustration`).
#[derive(IntoElement)]
struct ContextIllustration;

const SCENE_HOLDS: [f32; 4] = [2.4, 2.6, 3.2, 2.8];
/// The illustration's height: the tallest scene.
const SCENE_H: Pixels = px(116.);

fn scene_caption(scene: usize) -> String {
    match scene {
        0 => format!("開始組字時，讀取游標前最多 {CONTEXT_READ} 個字"),
        1 => "接著打的字，依前文選字".to_owned(),
        2 => format!("同一個 App 最近 {CONTEXT_MINUTES} 分鐘的前文，輸入框清空時仍可接續"),
        _ => "密碼欄位：不讀取，也不學習".to_owned(),
    }
}

/// A text field of a scene, titled.
fn scene_card(title: &'static str, content: impl IntoElement, cx: &App) -> gpui_kit::Div {
    let p = theme::palette(cx);
    field(cx)
        .px(space::S12)
        .py(space::S8)
        .flex()
        .flex_col()
        .gap(space::S6)
        .child(div().text_size(text::CAPTION).text_color(p.text_secondary).child(title))
        .child(content)
}

fn scene(index: usize, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    match index {
        0 | 1 => v()
            .w_full()
            .gap(space::S6)
            .child(
                h().pl(space::S12)
                    .gap(space::S6)
                    .text_color(p.text_secondary)
                    .child(Icon::new(IconName::ArrowDownLeft).size(size::ICON_XS))
                    .child(div().text_size(text::CAPTION).child(format!("最多 {CONTEXT_READ} 個字"))),
            )
            .child(scene_card(
                "備忘錄",
                demo_line(
                    std::iter::once(Segment::highlight("讀書會改到星期四晚上，地點一樣在"))
                        .chain((index == 1).then(|| Segment::marked("圖書館"))),
                )
                .font_size(text::BODY),
                cx,
            ))
            .into_any_element(),
        2 => h()
            .w_full()
            .gap(space::S12)
            .child(div().flex_1().min_w(px(0.)).child(scene_card(
                "輸入框 A",
                demo_line([Segment::highlight("地點一樣在圖書館")]).font_size(text::SUBTITLE).without_caret(),
                cx,
            )))
            .child(
                v().flex_shrink_0()
                    .items_center()
                    .gap(space::S2)
                    .text_color(p.text_secondary)
                    .child(Icon::new(IconName::ArrowRight).size(size::ICON_SM))
                    .child(div().text_size(text::CAPTION).child(format!("{CONTEXT_MINUTES} 分鐘內"))),
            )
            .child(div().flex_1().min_w(px(0.)).child(scene_card(
                "輸入框 B",
                demo_line([Segment::marked("七點見")]).font_size(text::SUBTITLE),
                cx,
            )))
            .into_any_element(),
        _ => div()
            .w_full()
            .pt(space::S20)
            .child(scene_card(
                "密碼",
                h().gap(space::S8)
                    .child(Icon::new(IconName::Lock).size(size::ICON_SM).text_color(p.text_secondary))
                    .child(div().text_size(text::BODY).text_color(p.text).child("••••••••"))
                    .child(div().flex_1())
                    .children(["不讀取", "不學習"].map(|tag| {
                        h().h(size::H_XS)
                            .px(space::S6)
                            .rounded(size::R_XS)
                            .bg(p.subtle)
                            .text_size(text::CAPTION)
                            .text_color(p.text_secondary)
                            .child(tag)
                    })),
                cx,
            ))
            .into_any_element(),
    }
}

impl gpui_kit::RenderOnce for ContextIllustration {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let holds = SCENE_HOLDS.iter().map(|&hold| Duration::from_secs_f32(hold)).collect();
        let clock = story_clock("context-illustration", holds, None, window, cx);
        let (current, step) = {
            let clock = clock.read(cx);
            (clock.index(), clock.step())
        };
        v().w_full()
            .gap(space::S12)
            .child(div().w_full().h(SCENE_H).flex().flex_col().justify_center().child(
                div().w_full().child(scene(current, cx)).with_animation(
                    SharedString::from(format!("context-scene-{step}")),
                    Animation::new(Duration::from_millis(350)),
                    |this, t| this.opacity(t),
                ),
            ))
            .child(
                h().w_full()
                    .gap(space::S8)
                    .child(play_pause("context-illustration", clock.clone(), cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(text::SUBTITLE)
                            .text_color(p.text_secondary)
                            .child(scene_caption(current)),
                    )
                    .child(h().flex_shrink_0().gap(space::S6).children((0..SCENE_HOLDS.len()).map(|dot| {
                        let clock = clock.clone();
                        div()
                            .id(SharedString::from(format!("context-dot-{dot}")))
                            .size(space::S6)
                            .rounded_full()
                            .cursor_pointer()
                            .bg(if dot == current { p.text_secondary } else { p.control_border })
                            .on_click(move |_, _, cx| clock.update(cx, |clock, cx| clock.show(dot, cx)))
                    }))),
            )
    }
}

// MARK: - Info cards

/// The ⓘ cards of this page by id, for `--popover` (screenshots): the title and what the card shows.
pub fn info(id: &str) -> Option<(&'static str, InfoContent)> {
    Some(match id {
        "anchor" => ("游標在句中時改", |_, cx| anchor_info(cx)),
        _ => return None,
    })
}

/// Which character ↓ changes with the cursor inside the sentence (macOS `OptionDemos.anchor`).
fn anchor_frames(right: bool) -> Vec<DemoFrame> {
    use SegmentStyle::{Focus, Marked, Target};
    let (left_style, right_style) = if right { (Marked, Target) } else { (Target, Marked) };
    let (left_focus, right_focus) = if right { (Marked, Focus) } else { (Focus, Marked) };
    let left_style = |text_: &'static str| Segment::new(text_, left_style);
    let right_style = |text_: &'static str| Segment::new(text_, right_style);
    let left_focus = |text_: &'static str| Segment::new(text_, left_focus);
    let right_focus = |text_: &'static str| Segment::new(text_, right_focus);
    vec![
        DemoFrame::new([Segment::marked("今天"), Segment::marked("天氣")], "游標在句尾：改最後的詞", 1.4),
        DemoFrame::new(
            [Segment::marked("今天"), left_style("天"), right_style("氣")],
            "游標在句中：底線標出要改的字",
            1.6,
        )
        .keys(["←"])
        .caret(1),
        DemoFrame::new(
            [Segment::marked("今天"), left_focus("天"), right_focus("氣")],
            if right { "改游標右邊的字" } else { "改游標左邊的字" },
            2.0,
        )
        .keys(["↓"])
        .caret(1)
        .list(if right { ["氣", "器", "汽"] } else { ["天", "添", "田"] }),
    ]
}

fn anchor_info(cx: &App) -> AnyElement {
    let after = app::settings(cx).string("candidateAnchor") != "before";
    v().gap(space::S16)
        .child(choice_demo("游標右邊的字", after, mini_demo("anchor-after", anchor_frames(true))))
        .child(choice_demo("游標左邊的字", !after, mini_demo("anchor-before", anchor_frames(false))))
        .into_any_element()
}

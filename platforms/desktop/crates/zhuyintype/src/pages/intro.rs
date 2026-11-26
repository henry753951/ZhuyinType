//! 「介紹」 (macOS `IntroPane`): the app's name over the typing story, the current settings that matter most (each
//! leading to its row), the setup wizard, and the main features as cards, each leading to its settings.
//!
//! The story plays in a loop while the page is on screen and the window is visible; under Reduce Motion it rests on the
//! open list.  With the service running, 試打 turns the stage into a field to type in (a `PracticeSession`: nothing
//! reaches the user's settings, words or statistics).

mod panel;
mod script;
mod stage;

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, img, px,
};
use zt_core::practice::{PracticeEvent, PracticeSession};

use self::script::{RESTING_STEP, Script, Segment};
use super::{Page, links};
use crate::app::{self, Place};
use crate::theme::{self, Palette, size, space, text, weight};
use crate::ui::charts::fill_column;
use crate::ui::feature_card::{feature_card, feature_grid};
use crate::ui::shortcuts::{keystroke, store};
use crate::ui::{self, h, v};
use crate::wizard::{self, WizardKind};

/// The try-it field (the service's practice session).
struct Trial {
    session: PracticeSession,
    focus: FocusHandle,
    toast: Option<(&'static str, Instant)>,
}

pub struct IntroPage {
    index: usize,
    /// Shows one step and stays there (`ZT_DEMO=intro-step=N`, screenshots).
    pinned: Option<usize>,
    window_visible: bool,
    trial: Option<Trial>,
    trial_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

fn pinned_step() -> Option<usize> {
    std::env::var("ZT_DEMO").ok()?.split(',').find_map(|item| item.trim().strip_prefix("intro-step=")?.parse().ok())
}

impl IntroPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let page = cx.weak_entity();
        let visibility = window.observe_window_visibility(move |visibility, _, cx| {
            let _ = page.update(cx, |page, cx| {
                page.window_visible = visibility.is_visible();
                cx.notify();
            });
        });
        // The story's clock: one step after another, each held for its time; it waits while it should not play.
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(hold) = this.update(cx, |page, cx| page.hold(cx)) else { break };
                cx.background_executor().timer(hold).await;
                if this.update(cx, |page, cx| page.advance(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            index: 0,
            pinned: pinned_step(),
            window_visible: window.is_visible(),
            trial: None,
            trial_error: None,
            _subscriptions: vec![visibility],
        }
    }

    fn script(cx: &App) -> Script {
        Script::new(app::settings(cx), &store::store(cx))
    }

    /// The story moves on its own now.
    fn playing(&self, cx: &App) -> bool {
        self.pinned.is_none()
            && self.trial.is_none()
            && self.window_visible
            && !cx.reduce_motion()
            && app::nav(cx).page == Page::Intro
    }

    fn hold(&self, cx: &App) -> Duration {
        if !self.playing(cx) {
            return Duration::from_millis(300);
        }
        let script = Self::script(cx);
        Duration::from_secs_f32(script.step(self.index).hold)
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if !self.playing(cx) {
            return;
        }
        let count = Self::script(cx).steps.len();
        self.index = (self.index + 1) % count.max(1);
        cx.notify();
    }

    fn current(&self, script: &Script, cx: &App) -> usize {
        let index = match self.pinned {
            Some(pinned) => pinned,
            None if cx.reduce_motion() => RESTING_STEP,
            None => self.index,
        };
        index.min(script.steps.len() - 1)
    }

    fn toggle_trial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(mut trial) = self.trial.take() {
            trial.session.end_session();
            cx.notify();
            return;
        }
        let Some(service) = app::service(cx) else { return };
        match service.practice() {
            Ok(session) => {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                self.trial = Some(Trial { session, focus, toast: None });
                self.trial_error = None;
            }
            Err(error) => self.trial_error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn trial_key(&mut self, event: &gpui_kit::KeyDownEvent, cx: &mut Context<Self>) {
        let Some(trial) = self.trial.as_mut() else { return };
        let Some(key) = keystroke::core_key(&event.keystroke, event.is_held) else { return };
        if trial.session.key_down(&key) {
            cx.stop_propagation();
        }
        for event in trial.session.take_events() {
            if let PracticeEvent::ModeToast { english } = event {
                trial.toast = Some((if english { "英" } else { "中" }, Instant::now()));
            }
        }
        cx.notify();
    }

    fn stage(&mut self, script: &Script, cx: &mut Context<Self>) -> AnyElement {
        let keys = &script.keys;
        let tallest = script
            .steps
            .iter()
            .filter_map(|step| step.panel.as_ref())
            .map(|p| panel::panel_height(p, keys))
            .fold(0., f32::max);
        let height = stage::height(tallest);
        let app_palette = theme::palette(cx);
        let window_palette = match script.dark {
            Some(true) => Palette::dark(),
            Some(false) => Palette::light(),
            None => app_palette,
        };
        let blinks = self.pinned.is_none();
        if let Some(trial) = self.trial.as_ref() {
            let presentation = trial.session.presentation();
            let text = trial.session.text().to_owned();
            let insertion: usize = trial.session.insertion();
            let units: Vec<u16> = text.encode_utf16().collect();
            let split = insertion.min(units.len());
            let before = String::from_utf16_lossy(&units[..split]);
            let after = String::from_utf16_lossy(&units[split..]);
            let segments: Vec<Segment> = stage::segments_of(&presentation);
            let toast =
                trial.toast.filter(|(_, at)| at.elapsed() < Duration::from_millis(1200)).map(|(label, _)| label);
            let scene = stage::Scene {
                committed: &before,
                marked: &segments,
                after: &after,
                panel: Some(&presentation),
                toast,
                keys: &[],
                trigger: 0,
                panel_since: 0,
            };
            let focus = trial.focus.clone();
            let element = stage::stage(&scene, keys, script.above, script.animated, height, &window_palette, true, cx);
            return div()
                .id("intro-trial")
                .track_focus(&focus)
                .on_key_down(cx.listener(|page, event, _, cx| page.trial_key(event, cx)))
                .on_key_up(cx.listener(|page, event: &gpui_kit::KeyUpEvent, _, cx| {
                    if let Some(trial) = page.trial.as_mut()
                        && let Some(key) = keystroke::core_key(&event.keystroke, false)
                    {
                        trial.session.key_up(&key);
                        cx.notify();
                    }
                }))
                .child(element)
                .into_any_element();
        }
        let index = self.current(script, cx);
        let step = script.step(index);
        // The window animates in at the step where it opened, not at every step it stays open.
        let panel_since = (0..=index).rev().take_while(|i| script.steps[*i].panel.is_some()).last().unwrap_or(index);
        let scene = stage::Scene {
            committed: &step.committed,
            marked: &step.marked,
            after: "",
            panel: step.panel.as_ref(),
            toast: step.toast,
            keys: &step.keys,
            trigger: index,
            panel_since,
        };
        stage::stage(&scene, keys, script.above, script.animated, height, &window_palette, blinks, cx)
    }

    fn header_card(&mut self, script: &Script, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let index = self.current(script, cx);
        let caption = if self.trial.is_some() {
            "在這裡試打；不會改變你的設定、字詞與統計".to_owned()
        } else {
            script.step(index).caption.clone()
        };
        let can_try = app::service(cx).is_some();
        let trying = self.trial.is_some();
        let stage = self.stage(script, cx);
        // A standard push button `H_SM` tall, in the stage's corner: as far in from the right as the text is from
        // the left, centred on the first line.
        let try_button = can_try.then(|| {
            h().id("intro-try")
                .absolute()
                .top(stage::CORNER_TOP)
                .right(stage::CORNER_RIGHT)
                .gap(space::S4)
                .px(space::S8)
                .h(size::H_SM)
                .rounded(size::R_SM)
                .bg(p.control)
                .border_1()
                .border_color(p.control_border)
                .text_size(text::SUBTITLE)
                .text_color(p.text)
                .cursor_pointer()
                .hover(|style| style.bg(p.hover))
                .on_click(cx.listener(|page, _, window, cx| page.toggle_trial(window, cx)))
                .child(
                    Icon::new(if trying { IconName::Play } else { IconName::Keyboard })
                        .size(size::ICON_XS)
                        .text_color(p.text_secondary),
                )
                .child(if trying { "示範" } else { "試打" })
        });
        v().w_full()
            .py(space::S12)
            .gap(space::S16)
            .child(
                h().gap(space::S16).child(img("app-icon.png").size(APP_ICON).rounded(APP_ICON_RADIUS)).child(
                    v().gap(space::S2)
                        .child(
                            div()
                                .text_size(text::TITLE)
                                .font_weight(weight::SEMIBOLD)
                                .text_color(p.text)
                                .child("ZhuyinType"),
                        )
                        .child(
                            div()
                                .text_size(text::BODY)
                                .text_color(p.text_secondary)
                                .child("開源、在本機運作的注音輸入法"),
                        ),
                ),
            )
            .child(
                v().gap(space::S8)
                    .child(div().relative().w_full().child(stage).children(try_button))
                    .child(stage::caption(caption, cx))
                    .when_some(self.trial_error.clone(), |this, error| {
                        this.child(div().text_size(text::SUBTITLE).text_color(p.danger).child(error))
                    }),
            )
            .into_any_element()
    }
}

/// A feature column is never narrower than this (one column in a narrow window).
const FEATURE_MIN_W: f32 = 240.;

/// The app's icon in the header, and its corner.
const APP_ICON: Pixels = px(52.);
const APP_ICON_RADIUS: Pixels = px(12.);

/// A setting of another page and its value, leading there.
fn summary_row(id: &'static str, title: &'static str, value: String, place: Place) -> AnyElement {
    ui::row(id, title).value(value).on_click(move |_, _, cx| app::go(place.clone(), cx)).into_any_element()
}

/// One feature card's content (macOS `IntroFeature`).
struct Feature {
    title: &'static str,
    text: String,
    icon: IconName,
    place: Place,
    link: &'static str,
}

fn features(cx: &App) -> Vec<Feature> {
    let settings = app::settings(cx);
    let keys = store::store(cx);
    let key = |command: &str| Some(keys.active_label(command)).filter(|label| !label.is_empty());
    let punctuation = if settings.string("punctuationKeys") != "mac" {
        format!(
            "Shift 加符號鍵打出鍵上的符號{}。標點預設全形，可切換全半形。",
            if settings.bool("feature.controlPunctuation") { "，Ctrl 加符號鍵打中文標點" } else { "" }
        )
    } else {
        "Shift 加符號鍵打出中文標點。標點預設全形，可切換全半形。".to_owned()
    };
    let width = key("togglePunctuationWidth")
        .map_or_else(|| "可在「標點與符號」設定快捷鍵。".to_owned(), |width| format!("按 {width} 切換。"));
    let corrections = match (settings.bool("showCorrections"), settings.bool("undoCorrectionWithBackspace")) {
        (true, true) => "；修正過的字加上點線，按 ⌫ 還原",
        (true, false) => "；修正過的字加上點線",
        (false, true) => "；按 ⌫ 還原修正",
        (false, false) => "",
    };
    let associated = if settings.bool("nextWordSuggestions") {
        "送出後列出常接在後面的詞，按 ↓ 進入清單再選。"
    } else {
        "開啟後，送出時列出常接在後面的詞，按 ↓ 進入清單再選。"
    };
    let word_actions = match (key("blockCandidate"), key("addCandidatePhrase")) {
        (Some(block), Some(add)) => format!("在候選字上按右鍵，或按 {block} 不要再推薦、{add} 加入自訂詞。"),
        _ => "在候選字上按右鍵，不要再推薦、忘記選字或加入自訂詞。".to_owned(),
    };
    let symbols = key("symbolPicker").map_or_else(
        || "開啟符號與 Emoji 面板，打注音或英文搜尋。".to_owned(),
        |symbols| format!("按 {symbols} 開啟符號與 Emoji 面板，打注音或英文搜尋。"),
    );
    let quick = key("quickSettings").map_or_else(
        || "設定一個按鍵，在游標旁切換英文、半形標點、簡體與暫停學習。".to_owned(),
        |quick| format!("按 {quick} 在游標旁切換英文、半形標點、簡體與暫停學習。"),
    );
    let card = |title: &'static str, text: String, icon: IconName, link: &'static str| {
        let place = links::intro_cards()
            .into_iter()
            .find(|(card, _)| *card == title)
            .map_or_else(|| Place::page(Page::Intro), |(_, place)| place);
        Feature { title, text, icon, place, link }
    };
    vec![
        card("整句輸入", "一句話連續打完，依讀音與前後文選出最通順的寫法。".into(), IconName::WholeWord, "選字"),
        card(
            "按錯鍵也能打對",
            format!("按到相鄰的鍵、多按、漏按或順序打反時試著修正，照打的字優先{corrections}。"),
            IconName::SpellCheck,
            "修正",
        ),
        card(
            "依前文選字",
            "參考游標前最多 128 個字，並接續同一個 App 最近 1 分鐘的前文。".into(),
            IconName::TextAlignStart,
            "前文",
        ),
        card("聯想詞", associated.into(), IconName::MessageSquareText, "選字"),
        card(
            "記住你的用法",
            "選過的字詞下次直接採用；資料只存在這台電腦，也可以暫停學習。".into(),
            IconName::ClockArrowUp,
            "字詞",
        ),
        card("在候選字視窗管理字詞", word_actions, IconName::ListChecks, "字詞"),
        card("標點與全半形", format!("{punctuation}{width}"), IconName::Quote, "標點與符號"),
        card("符號與 Emoji", symbols, IconName::Shapes, "標點與符號"),
        card("快速設定", quick, IconName::Zap, "中英切換"),
        card(
            "色彩與無障礙",
            "淺色或深色、透明效果與強調色；跟隨系統的高對比與減少動態效果。".into(),
            IconName::Palette,
            "色彩",
        ),
        card(
            "檢查輸入法",
            "無法用 ZhuyinType 打字時，找出常見的原因並修正。".into(),
            IconName::Stethoscope,
            "檢查輸入法",
        ),
        card("改字記錄", "可以記錄改字的過程，用來改進選字；只存在這台電腦。".into(), IconName::ClipboardPen, "進階"),
        card("使用分析", "打了多少字、改了幾次、輸入法反應多快。".into(), IconName::ChartColumn, "分析"),
    ]
}

impl Render for IntroPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let script = Self::script(cx);
        let settings = app::settings(cx);
        let space = match settings.string("spaceAction").as_str() {
            "candidates" => "開啟候選字視窗",
            "insert" => "輸入空白，不送出",
            _ => "送出並輸入空白",
        };
        let punctuation = if settings.string("punctuationKeys") == "mac" { "macOS" } else { "Windows" };
        let typo = match settings.string("typoTolerance").as_str() {
            "off" => "關閉",
            "aggressive" => "積極",
            _ => "標準",
        };
        let mode = script::mode_key(settings).map_or("不使用".to_owned(), |key| format!("按 {key}"));
        let new_lessons = wizard::new_lessons_since(cx);
        let header = self.header_card(&script, cx);
        let cards: Vec<AnyElement> = features(cx)
            .into_iter()
            .map(|feature| {
                let place = feature.place.clone();
                feature_card(
                    SharedString::from(format!("feature-{}", feature.title)),
                    feature.icon,
                    feature.title,
                    feature.text,
                    feature.link,
                    move |_, _, cx| app::go(place.clone(), cx),
                    cx,
                )
            })
            .collect();
        fill_column()
            .gap(size::SECTION_GAP)
            .child(ui::section("").bare().first().row(header))
            .child(ui::section("目前的設定").rows([
                summary_row("summary-space", "空白鍵", space.to_owned(), links::space_place()),
                summary_row("summary-punctuation", "標點按法", punctuation.to_owned(), links::punctuation_keys_place()),
                summary_row("summary-typo", "按錯鍵修正", typo.to_owned(), links::typo_place()),
                summary_row("summary-mode", "中英切換", mode, links::shift_tap_place()),
            ]))
            .child(
                ui::section("設定嚮導")
                    .row(
                        ui::row("wizard-quick", "開始設定嚮導")
                            .subtitle("選擇習慣的打法，並在嚮導中試打")
                            .on_click(|_, _, cx| wizard::open(WizardKind::Quick, cx)),
                    )
                    .when_some(new_lessons, |section, since| {
                        section.row(
                            ui::row("wizard-new", "新功能導覽")
                                .subtitle("只介紹這一版新增的功能")
                                .on_click(move |_, _, cx| wizard::open(WizardKind::NewSince(since.clone()), cx)),
                        )
                    })
                    .row(
                        ui::row("wizard-keys", "你會用到的按鍵")
                            .subtitle("依目前的設定列出按鍵")
                            .on_click(|_, _, cx| wizard::open(WizardKind::KeysOnly, cx)),
                    ),
            )
            .child(ui::section("功能").bare().row(feature_grid(FEATURE_MIN_W, cards)))
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(_id: &str) -> Option<&'static str> {
    None
}

//! The 「視窗與提示」 page's preview (macOS `UnifiedPreview`, `PreviewDirector`, `PreviewFlows`): one scene at a time,
//! drawn with the real candidate window, with tabs for its states.  Left alone it plays the states one after another,
//! each as a short flow (the list opening, ⇧↑ switching to corrections, a page turning with the real paging
//! motion…).  Changing a setting switches it to the state that shows that setting; when the preview is scrolled out of
//! view, that state also floats beside the pointer for a moment.  It holds still for 2 s after the user picks a tab,
//! changes a setting or presses in it; the pointer resting over it does not pause it.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement, Styled, Task, Window,
    anchored, canvas, deferred, div, point, px,
};
use zt_core::presentation::Presentation;
use zt_core::settings::Settings;

use super::layout::{Extras, WindowMeasure};
use super::panel::CandidatePanel;
use super::sample;
use super::scene::{Card, Scene, SceneLayout, focus, scene_card, seg};
use super::style::{CurrentKeys, Layout, PanelAppearance, PanelStyle, selection_keys};
use crate::app;
use crate::pages::Page;
use crate::theme::{self, size, text};
use crate::ui::{h, v};

/// What the preview can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewState {
    List,
    Corrections,
    Composing,
    Paging,
    Phrase,
    Mode,
}

impl PreviewState {
    pub const ALL: [PreviewState; 6] =
        [Self::List, Self::Corrections, Self::Composing, Self::Paging, Self::Phrase, Self::Mode];

    pub fn title(self) -> &'static str {
        match self {
            Self::List => "候選字清單",
            Self::Corrections => "修正清單",
            Self::Composing => "組字中",
            Self::Paging => "翻頁",
            Self::Phrase => "加入自訂詞",
            Self::Mode => "中英切換",
        }
    }
}

/// The scenes for the current settings (macOS `PreviewScenes`); the keys come from the settings.
struct Scenes {
    composing: Scene,
    list: Scene,
    corrections: Option<Scene>,
    phrase: Option<Scene>,
    mode: Option<Scene>,
}

fn list_presentation(tolerant: bool, page_size: usize, selected: usize, keys: &[char]) -> Presentation {
    sample::list(sample::WORDS, sample::PREVIEW_CORRECTIONS, tolerant, page_size, selected).presentation("", keys)
}

impl Scenes {
    fn new(settings: &Settings, keys: &CurrentKeys, page_limit: usize) -> Self {
        let selection = selection_keys(settings);
        let page_size = selection.len().min(page_limit).max(1);
        let start = "約了".to_owned();
        let composing = Scene {
            title: "組字中",
            committed: String::new(),
            marked: vec![seg("王"), seg("小"), seg("ㄇㄧㄥ")],
            keys: vec!["ㄥ".into()],
            note: None,
            list: None,
            notice: None,
            badge: None,
            badge_off: false,
        };
        let list = Scene {
            title: "候選字清單",
            committed: start.clone(),
            marked: vec![seg("王"), focus("曉明")],
            keys: vec!["↓".into()],
            note: (settings.string("spaceAction") == "candidates").then(|| "或 空白".to_owned()),
            list: Some(list_presentation(false, page_size, 0, &selection)),
            ..composing.clone()
        };
        let corrections = keys.caps("correctionsList").map(|caps| Scene {
            title: "修正清單",
            keys: caps,
            note: None,
            list: Some(list_presentation(true, page_size, 0, &selection)),
            ..list.clone()
        });
        let phrase = keys.caps("selectLeft").map(|caps| Scene {
            title: "加入自訂詞",
            committed: start.clone(),
            marked: vec![focus("王小明")],
            keys: caps,
            note: Some("×3".into()),
            list: None,
            notice: Some("Enter 加入自訂詞：王小明".into()),
            ..composing.clone()
        });
        let toast = settings.bool("showModeToast");
        let mode = keys.mode_key().map(|key| Scene {
            title: "中英切換",
            committed: "王小明".into(),
            marked: Vec::new(),
            keys: vec![key],
            note: None,
            list: None,
            notice: None,
            badge: toast.then_some("英"),
            badge_off: !toast,
        });
        Self { composing, list, corrections, phrase, mode }
    }

    /// Each state's flow, as the scenes of the real window it passes through.
    fn frames(&self, state: PreviewState, settings: &Settings, keys: &CurrentKeys) -> Vec<Scene> {
        match state {
            PreviewState::List => {
                let before = Scene { list: None, ..self.list.clone() };
                vec![before, self.list.clone()]
            }
            PreviewState::Corrections => match &self.corrections {
                Some(corrections) => vec![self.list.clone(), corrections.clone()],
                None => Vec::new(),
            },
            PreviewState::Composing => {
                let start =
                    Scene { marked: vec![seg("王"), seg("ㄒ")], keys: vec!["ㄒ".into()], ..self.composing.clone() };
                vec![start, self.composing.clone()]
            }
            PreviewState::Paging => {
                let Some(list) = &self.list.list else { return Vec::new() };
                if list.page_count < 2 || !settings.bool("feature.pageKeys") {
                    return Vec::new();
                }
                let selection = selection_keys(settings);
                let next = Scene {
                    list: Some(list_presentation(false, list.page_size, list.page_size, &selection)),
                    keys: keys.caps("nextPage").unwrap_or_else(|| vec!["Tab".into()]),
                    note: None,
                    ..self.list.clone()
                };
                vec![self.list.clone(), next]
            }
            PreviewState::Phrase => match &self.phrase {
                Some(phrase) => {
                    let selecting = Scene {
                        notice: None,
                        note: Some("×1".into()),
                        marked: vec![seg("王"), focus("小明")],
                        ..phrase.clone()
                    };
                    vec![selecting, phrase.clone()]
                }
                None => Vec::new(),
            },
            PreviewState::Mode => match &self.mode {
                Some(mode) => {
                    let before = Scene { badge: None, badge_off: false, keys: Vec::new(), ..mode.clone() };
                    let back = Scene { badge: mode.badge.map(|_| "中"), ..mode.clone() };
                    vec![before, mode.clone(), back]
                }
                None => Vec::new(),
            },
        }
    }
}

/// The settings each state shows, to tell which one a change belongs to (macOS `PreviewSettings`).
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    list: Vec<String>,
    corrections: Vec<String>,
    paging: Vec<String>,
    mode: Vec<String>,
    per_page: String,
}

impl Snapshot {
    fn of(settings: &Settings, keys: &CurrentKeys) -> Self {
        let values = |list: &[&str]| list.iter().map(|key| settings.get(key).to_string()).collect::<Vec<_>>();
        let mut list = values(&[
            "candidateLayout",
            "candidateFontSize",
            "candidateDensity",
            "candidatesPerPage",
            "selectionKeys",
            "showsZhuyinHint",
            "feature.zhuyinRowClick",
            "candidatePlacement",
            "panelAppearance",
            "feature.singleCharacterList",
        ]);
        list.push(keys.text("singleCharacterList"));
        let mut corrections = values(&["showsAnnotations", "showsListSwitches", "feature.correctionsList"]);
        corrections.push(keys.text("correctionsList"));
        let mut paging = values(&["panelAnimation", "pageAtListEdge", "feature.pageKeys"]);
        paging.extend([keys.text("nextPage"), keys.text("previousPage")]);
        let mut mode = values(&["showModeToast"]);
        mode.push(keys.mode_key().unwrap_or_default());
        Self { list, corrections, paging, mode, per_page: settings.get("candidatesPerPage").to_string() }
    }

    /// The state that shows a change, or None when nothing the preview shows changed.
    fn state_for(&self, new: &Snapshot) -> Option<PreviewState> {
        if self.corrections != new.corrections {
            Some(PreviewState::Corrections)
        } else if self.paging != new.paging {
            Some(PreviewState::Paging)
        } else if self.mode != new.mode {
            Some(PreviewState::Mode)
        } else if self.list != new.list {
            Some(PreviewState::List)
        } else {
            None
        }
    }
}

/// What the state shows, beside the scene: its name and the keys that lead to it (macOS `StateNote.text`).
fn note_text(state: PreviewState, settings: &Settings, keys: &CurrentKeys) -> String {
    match state {
        PreviewState::List => format!(
            "按 ↓{} 開啟；{} 選字，Return 選取反白的字。",
            if settings.string("spaceAction") == "candidates" { "或空白" } else { "" },
            keys.selection()
        ),
        PreviewState::Corrections => {
            let key = keys.text("correctionsList");
            format!(
                "{}列出按錯修正與模糊音的字{}",
                if key.is_empty() { String::new() } else { format!("按 {key} ") },
                if settings.bool("showsAnnotations") { "，修正的讀音以灰字標出。" } else { "。" }
            )
        }
        PreviewState::Composing => "打完一個注音就轉成中文，整句一起轉換；要改的字可以之後再選。".into(),
        PreviewState::Paging => {
            let next: Vec<String> = keys.all("nextPage").into_iter().take(3).collect();
            format!(
                "{}{}",
                if next.is_empty() {
                    "清單中按空白翻頁".to_owned()
                } else {
                    format!("按 {} 翻到下一頁", next.join("、"))
                },
                if settings.bool("panelAnimation") { "，舊的一頁移出、新的一頁移入。" } else { "。" }
            )
        }
        PreviewState::Phrase => "選取 1 到 6 個字，再按 Return 加入自訂詞。".into(),
        PreviewState::Mode => match keys.mode_key() {
            Some(key) => format!(
                "按 {key} 切換中英{}",
                if settings.bool("showModeToast") { "，游標旁顯示「中」或「英」。" } else { "。" }
            ),
            None => "沒有設定切換中英的按鍵。".into(),
        },
    }
}

/// The flows at one page limit, with every scene's layout at the available width.
struct Flows {
    states: Vec<PreviewState>,
    frames: Vec<(PreviewState, Vec<Scene>)>,
}

impl Flows {
    fn new(settings: &Settings, keys: &CurrentKeys, page_limit: usize) -> Self {
        let scenes = Scenes::new(settings, keys, page_limit);
        let frames: Vec<(PreviewState, Vec<Scene>)> = PreviewState::ALL
            .into_iter()
            .map(|state| (state, scenes.frames(state, settings, keys)))
            .filter(|(_, frames)| !frames.is_empty())
            .collect();
        Self { states: frames.iter().map(|(state, _)| *state).collect(), frames }
    }

    fn frames(&self, state: PreviewState) -> &[Scene] {
        self.frames.iter().find(|(s, _)| *s == state).map_or(&[], |(_, frames)| frames.as_slice())
    }

    fn all(&self) -> impl Iterator<Item = &Scene> {
        self.frames.iter().flat_map(|(_, frames)| frames.iter())
    }
}

#[derive(Clone, Debug)]
struct Geometry {
    limit: usize,
    scene_width: f32,
    heights: Vec<(PreviewState, f32)>,
}

/// The candidates a page the preview lists: fewer than the window does, so the preview stays short; all of them for
/// a while after 每頁的候選字 changes.
const PAGE_LIMIT: usize = 4;
const MAX_HEIGHT: f32 = 300.;
/// The carousel's step, and how long a pick or a change holds it.
const STEP: Duration = Duration::from_millis(1400);
const HOLD: Duration = Duration::from_secs(2);
/// The note beside the scene.
const NOTE_MAX: f32 = 260.;
const NOTE_GAP: f32 = 18.;
/// The floating preview after a change.
const FLASH_WIDTH: f32 = 300.;
const FLASH_SHOWN: Duration = Duration::from_secs(2);
const FLASH_FADE: f32 = 300.;

pub struct UnifiedPreview {
    state: PreviewState,
    /// The moment of the state's flow (`usize::MAX`: the last, which shows the window).
    frame: usize,
    held_until: Instant,
    paused_until: Instant,
    full_page_until: Instant,
    /// A state the preview stays on, playing its flow (a wizard lesson's demo).
    pinned: Option<PreviewState>,
    shown: Option<Snapshot>,
    shown_revision: Option<u64>,
    panel: CandidatePanel,
    /// What the panel shows: the state, the frame and the settings' revision.
    panel_key: Option<(PreviewState, usize, u64, usize)>,
    /// Bumped when the state changes (the scene cross-fades) and when the frame changes (the key caps press).
    generation: usize,
    frame_generation: usize,
    width: Rc<Cell<f32>>,
    in_view: Rc<Cell<bool>>,
    flash: Option<(Instant, Point<Pixels>)>,
    flash_panel: CandidatePanel,
    last_render: Cell<Option<Instant>>,
    geometry: Option<((u64, i32, usize, bool), Geometry)>,
    theme_dark: bool,
    /// The state tabs above the scene (the wizard's lessons show one state, without them).
    pub tabs: bool,
    /// Holds still on one moment (screenshots: `ZT_PREVIEW=STATE[:FRAME]`).
    still: bool,
    /// The floating preview pinned at a point (screenshots: `ZT_PREVIEW_FLASH=X,Y`).
    pinned_flash: Option<Point<Pixels>>,
    /// Settings to show instead of the user's (the setup wizard's draft); the preview is then always in view.
    source: Option<Settings>,
    _carousel: Task<()>,
}

/// `ZT_PREVIEW=paging:0` → the state and the frame to hold still on.
fn still_from_env() -> Option<(PreviewState, usize)> {
    let value = std::env::var("ZT_PREVIEW").ok()?;
    let (name, frame) =
        value.split_once(':').map_or((value.as_str(), None), |(name, frame)| (name, frame.parse().ok()));
    let state = PreviewState::ALL.into_iter().find(|state| format!("{state:?}").eq_ignore_ascii_case(name))?;
    Some((state, frame.unwrap_or(usize::MAX)))
}

impl UnifiedPreview {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let carousel = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(STEP).await;
                if this.update(cx, |preview, cx| preview.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut panel = CandidatePanel::new();
        panel.hover = false;
        let mut flash_panel = CandidatePanel::new();
        flash_panel.hover = false;
        let past = Instant::now() - Duration::from_secs(60);
        Self {
            state: PreviewState::List,
            frame: usize::MAX,
            held_until: past,
            paused_until: past,
            full_page_until: past,
            pinned: None,
            shown: None,
            shown_revision: None,
            panel,
            panel_key: None,
            generation: 0,
            frame_generation: 0,
            width: Rc::new(Cell::new(300.)),
            in_view: Rc::new(Cell::new(true)),
            flash: None,
            flash_panel,
            last_render: Cell::new(None),
            geometry: None,
            theme_dark: false,
            tabs: true,
            still: false,
            pinned_flash: None,
            source: None,
            _carousel: carousel,
        }
        .with_environment()
    }

    fn with_environment(mut self) -> Self {
        if let Some((state, frame)) = still_from_env() {
            self.state = state;
            self.frame = frame;
            self.still = true;
        }
        self.pinned_flash = std::env::var("ZT_PREVIEW_FLASH").ok().and_then(|value| {
            let (x, y) = value.split_once(',')?;
            Some(point(px(x.parse().ok()?), px(y.parse().ok()?)))
        });
        if let Some(at) = self.pinned_flash {
            self.flash = Some((Instant::now(), at));
        }
        self
    }

    /// Shows `settings` (the setup wizard's draft) instead of the user's, with that draft's keys.
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.source = Some(settings);
        self
    }

    /// The settings shown and their keys.
    fn shown_settings(&self, cx: &App) -> (Settings, CurrentKeys) {
        match &self.source {
            Some(settings) => (settings.clone(), CurrentKeys::sandboxed(settings)),
            None => {
                let settings = app::settings(cx).clone();
                let keys = CurrentKeys::new(&settings, app::service(cx));
                (settings, keys)
            }
        }
    }

    /// Shows `state` from its first moment (or its last, which shows the setting); the carousel goes on after a hold.
    pub fn show(&mut self, state: PreviewState, last: bool) {
        self.held_until = Instant::now() + HOLD;
        if state != self.state {
            self.generation += 1;
        }
        self.state = state;
        self.frame = if last { usize::MAX } else { 0 };
        self.frame_generation += 1;
    }

    /// Keeps the preview on `state`, playing its flow over and over.
    pub fn pin(&mut self, state: PreviewState) {
        self.pinned = Some(state);
        self.state = state;
        self.frame = usize::MAX;
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        // Not drawn lately (another page is shown): nothing to move.
        if self.still
            || self.last_render.get().is_none_or(|at| at.elapsed() > STEP * 2)
            || Instant::now() < self.paused_until
        {
            return;
        }
        let (settings, keys) = self.shown_settings(cx);
        let flows = Flows::new(&settings, &keys, self.page_limit(&settings));
        let states = flows.states.clone();
        if states.is_empty() {
            return;
        }
        let count = flows.frames(self.state).len();
        let current = self.frame.min(count.saturating_sub(1));
        if current + 1 < count {
            self.frame = current + 1;
        } else if self.pinned.is_some() || Instant::now() < self.held_until || !states.iter().any(|s| *s != self.state)
        {
            self.frame = 0;
        } else {
            let index = states.iter().position(|s| *s == self.state).map_or(0, |i| (i + 1) % states.len());
            self.state = states[index];
            self.frame = 0;
            self.generation += 1;
        }
        self.frame_generation += 1;
        if app::nav(cx).page == Page::Candidates || self.pinned.is_some() || self.source.is_some() {
            cx.notify();
        }
    }

    fn page_limit(&self, settings: &Settings) -> usize {
        if Instant::now() < self.full_page_until { selection_keys(settings).len() } else { PAGE_LIMIT }
    }

    /// The candidates a page that fit the width and the height (at least two), how wide the scenes are and how tall
    /// each state's tallest moment is (so the box does not move while a flow plays).  Measured once per settings
    /// revision, width and page limit.
    fn geometry(
        &mut self,
        settings: &Settings,
        keys: &CurrentKeys,
        style: &PanelStyle,
        above: bool,
        available: f32,
        window: &Window,
    ) -> Geometry {
        let most = self.page_limit(settings).max(2);
        let key = (settings.revision(), available.round() as i32, most, self.theme_dark);
        if let Some((cached, geometry)) = &self.geometry
            && *cached == key
        {
            return geometry.clone();
        }
        let mut measure = WindowMeasure(window);
        let mut limit = most;
        let flows = loop {
            let flows = Flows::new(settings, keys, limit);
            let fits = flows.all().all(|scene| {
                let layout = SceneLayout::new(scene, available, style, above, &mut measure);
                !layout.overflows() && layout.natural_height(scene) <= MAX_HEIGHT
            });
            if limit <= 2 || fits {
                break flows;
            }
            limit -= 1;
        };
        // The scenes as wide as the window needs (a horizontal list takes the whole width).
        let scene_width = if style.layout == Layout::Vertical {
            let needed = flows
                .all()
                .filter_map(|scene| {
                    SceneLayout::new(scene, available, style, above, &mut measure).panel.map(|(x, _, w, _)| x + w + 16.)
                })
                .fold(300., f32::max);
            available.min(needed)
        } else {
            available
        };
        let heights = flows
            .frames
            .iter()
            .map(|(state, frames)| {
                let height = frames
                    .iter()
                    .map(|scene| SceneLayout::new(scene, scene_width, style, above, &mut measure).natural_height(scene))
                    .fold(0., f32::max);
                (*state, height)
            })
            .collect();
        let geometry = Geometry { limit, scene_width, heights };
        self.geometry = Some((key, geometry.clone()));
        geometry
    }

    /// After a setting changed: switch to the state that shows it and, if the preview is out of view, float it
    /// beside the pointer.
    fn notice_changes(&mut self, settings: &Settings, keys: &CurrentKeys, window: &mut Window, cx: &mut Context<Self>) {
        let revision = settings.revision();
        if self.shown_revision == Some(revision) {
            return;
        }
        self.shown_revision = Some(revision);
        let now = Snapshot::of(settings, keys);
        let Some(before) = self.shown.replace(now.clone()) else { return };
        if before == now {
            return;
        }
        if before.per_page != now.per_page {
            self.full_page_until = Instant::now() + HOLD;
        }
        if let Some(state) = before.state_for(&now) {
            self.show(state, true);
        }
        if !self.in_view.get() && self.pinned.is_none() {
            self.flash = Some((Instant::now(), window.mouse_position()));
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(FLASH_SHOWN).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
    }
}

impl Render for UnifiedPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.last_render.set(Some(Instant::now()));
        let p = theme::palette(cx);
        self.theme_dark = p.dark;
        let (settings, keys) = self.shown_settings(cx);
        self.notice_changes(&settings, &keys, window, cx);
        let style = PanelStyle::from_settings(&settings, &keys);
        let above = settings.string("candidatePlacement") == "above";
        let panel_palette = PanelAppearance::from_settings(&settings).palette(p.dark);
        let animates = settings.bool("panelAnimation");
        self.panel.animates = animates;
        let available = self.width.get();
        let geometry = self.geometry(&settings, &keys, &style, above, available, window);
        let flows = Flows::new(&settings, &keys, geometry.limit);
        let state = if flows.states.contains(&self.state) { self.state } else { PreviewState::List };
        let frames = flows.frames(state);
        let frame = self.frame.min(frames.len().saturating_sub(1));
        let scene = frames[frame].clone();
        let scene_width = geometry.scene_width;
        let scene_height = geometry.heights.iter().find(|(s, _)| *s == state).map_or(120., |(_, h)| *h);
        let mut layout = SceneLayout::new(&scene, scene_width, &style, above, &mut WindowMeasure(window));
        layout.fit(scene_height);

        // The window shows this moment; a page of the same list turns with the real motion.
        let key = (state, frame, settings.revision(), scene.list.as_ref().map_or(0, |list| list.page_size));
        if self.panel_key != Some(key) {
            let same_state = self.panel_key.is_some_and(|(s, f, ..)| s == state && f + 1 == frame);
            if !same_state {
                self.panel.reset();
            }
            if let Some(presentation) = scene.presentation() {
                self.panel.update(
                    &presentation,
                    Extras::default(),
                    &style,
                    4000.,
                    same_state,
                    &mut WindowMeasure(window),
                );
            }
            self.panel_key = Some(key);
        }
        let card = scene_card(
            Card {
                scene: &scene,
                layout: &layout,
                panel: &self.panel,
                panel_palette: &panel_palette,
                key_generation: SharedString::from(format!("{}-{}", self.generation, self.frame_generation)),
                animated: true,
            },
            window,
            cx,
        );
        let note_room = available - scene_width;
        let note = (note_room - NOTE_GAP > 150.).then(|| {
            v().w(px(NOTE_MAX.min(note_room - NOTE_GAP)))
                .pt(px(10.))
                .gap(px(6.))
                .child(
                    div()
                        .text_size(text::BODY)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child(state.title()),
                )
                .child(
                    div()
                        .text_size(text::SUBTITLE)
                        .text_color(p.text_secondary)
                        .child(SharedString::from(note_text(state, &settings, &keys))),
                )
                .with_animation(
                    SharedString::from(format!("note-{}", self.generation)),
                    Animation::new(Duration::from_millis(340)),
                    |this, t| this.opacity(((t - 0.35) / 0.65).clamp(0., 1.)),
                )
        });
        let tabs = self.tabs.then(|| {
            div().flex().flex_row().flex_wrap().gap(px(6.)).children(flows.states.iter().map(|&tab| {
                let selected = tab == state;
                div()
                    .id(SharedString::from(format!("preview-tab-{tab:?}")))
                    .h(px(24.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .cursor_pointer()
                    .text_size(text::SUBTITLE)
                    .when(selected, |this| {
                        this.bg(p.accent_faint).text_color(p.accent).font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!selected, |this| {
                        this.bg(p.hover).text_color(p.text_secondary).hover(|style| style.text_color(p.text))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show(tab, false);
                        cx.notify();
                    }))
                    .child(tab.title())
            }))
        });
        let width_probe = {
            let width = self.width.clone();
            let in_view = self.in_view.clone();
            let embedded = self.source.is_some();
            canvas(
                move |bounds: Bounds<Pixels>, window, cx| {
                    let w = f32::from(bounds.size.width);
                    if (w - width.get()).abs() > 0.5 {
                        width.set(w);
                        window.refresh();
                    }
                    if embedded {
                        in_view.set(true);
                        return;
                    }
                    let viewport = app::nav(cx).scroll.bounds();
                    in_view.set(bounds.bottom() > viewport.origin.y + px(60.));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
        };
        let flash = self.flash_overlay(&settings, &keys, &style, above, window, cx);
        v().id("candidate-preview")
            .relative()
            .w_full()
            .min_w(px(0.))
            .gap(px(10.))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, _| this.paused_until = Instant::now() + HOLD))
            .children(tabs)
            .child(
                h().relative()
                    .w_full()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .items_start()
                    .gap(px(NOTE_GAP))
                    .child(width_probe)
                    .child(div().flex_shrink_0().child(card).with_animation(
                        SharedString::from(format!("scene-{}", self.generation)),
                        Animation::new(Duration::from_millis(340)),
                        |this, t| this.opacity(((t - 0.35) / 0.65).clamp(0., 1.)),
                    ))
                    .children(note),
            )
            .children(flash)
    }
}

impl UnifiedPreview {
    /// The floating preview after a change: beside the pointer, above it when there is room.
    fn flash_overlay(
        &mut self,
        settings: &Settings,
        keys: &CurrentKeys,
        style: &PanelStyle,
        above: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (started, at) = self.flash?;
        let elapsed = if self.pinned_flash.is_some() { Duration::from_millis(500) } else { started.elapsed() };
        let fade = if elapsed < FLASH_SHOWN {
            1.
        } else {
            let t = (elapsed - FLASH_SHOWN).as_secs_f32() * 1000. / FLASH_FADE;
            if t >= 1. {
                self.flash = None;
                return None;
            }
            window.request_animation_frame();
            1. - t
        };
        let p = theme::palette(cx);
        let flows = Flows::new(settings, keys, selection_keys(settings).len());
        let state = if flows.states.contains(&self.state) { self.state } else { PreviewState::List };
        let scene = flows.frames(state).last()?.clone();
        let mut layout = SceneLayout::new(&scene, FLASH_WIDTH, style, above, &mut WindowMeasure(window));
        let height = layout.natural_height(&scene);
        layout.fit(height);
        self.flash_panel.reset();
        if let Some(presentation) = scene.presentation() {
            self.flash_panel.update(&presentation, Extras::default(), style, 4000., false, &mut WindowMeasure(window));
        }
        let panel_palette = PanelAppearance::from_settings(settings).palette(p.dark);
        let card = scene_card(
            Card {
                scene: &scene,
                layout: &layout,
                panel: &self.flash_panel,
                panel_palette: &panel_palette,
                key_generation: "flash".into(),
                animated: false,
            },
            window,
            cx,
        );
        let total = (FLASH_WIDTH + 20., height + 22. + 20.);
        let viewport = window.viewport_size();
        let x = (at.x - px(total.0 / 2.)).max(px(12.)).min(viewport.width - px(total.0 + 12.));
        let y = if at.y - px(total.1 + 24.) > px(8.) { at.y - px(total.1 + 24.) } else { at.y + px(24.) };
        let appear = (elapsed.as_secs_f32() * 1000. / 250.).min(1.);
        if appear < 1. {
            window.request_animation_frame();
        }
        Some(
            deferred(
                anchored().position(point(x, y)).child(
                    v().p(px(10.))
                        .gap(px(6.))
                        .rounded(size::RADIUS_CARD)
                        .bg(p.card)
                        .border_1()
                        .border_color(p.separator)
                        .shadow_lg()
                        .opacity(fade * appear)
                        .child(
                            div()
                                .text_size(text::CAPTION)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.text_secondary)
                                .child(scene.title),
                        )
                        .child(card),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

//! The setup wizard (macOS `WizardModel`, `WizardView`, `WizardWindowController`; docs/product/SETUP_WIZARD.md): a
//! window of its own that goes welcome → habit preset → the lessons (each with its choices and a live practice
//! field) → 原本 → 改成 → the keys you will use.
//!
//! Every choice goes to a draft of the settings in memory (the practice field's own copy, so the field follows each
//! choice at once); the user's settings change only when 「套用」 writes the list of changes.  The steps, presets and
//! lessons are data (`data`); the window draws them (`steps`) with the app's tokens and components.
//!
//! The window has no system frame: the bar at its top is the header (上一步, the progress, 全部跳過) and the window
//! controls, drawn by the app like the settings window's.  Closing it leaves the settings window as it was.
//!
//! First use: the version of the lessons the user has seen is kept in the settings file
//! (`setupWizardSeenVersion`, beside the settings, as macOS keeps it in the app's defaults).  When it is missing the
//! settings window opens the wizard once and records the version; a later version with new lessons shows
//! 「新功能導覽」 on 介紹 instead of opening by itself.

mod data;
mod practice;
mod steps;

use std::collections::HashSet;
use std::time::Instant;

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Global, KeyDownEvent, Subscription,
    Window, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use serde_json::Value;
use zt_core::settings::Settings;

use self::data::{LESSONS, Level, PRESETS};
use self::practice::{FieldEvent, PracticeField};
use crate::app::{self, Navigation};
use crate::candidate::preview::{PreviewState, UnifiedPreview};
use crate::pages::Page;
use crate::theme;

/// Which wizard to show (macOS `WizardKind`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardKind {
    /// 開始設定嚮導: choose a way of typing and try it.
    Quick,
    /// 新功能導覽: only the lessons new since this version.
    NewSince(String),
    /// 你會用到的按鍵: the keys for the current settings.
    KeysOnly,
}

/// Which steps the wizard goes through (macOS `WizardModel.Mode`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    /// The basic lessons (快速設定).
    Quick,
    /// Every lesson (完整導覽).
    Full,
    /// Only lessons added after this version (新功能導覽).
    NewSince(String),
    /// Only the key summary.
    KeysOnly,
}

/// One page of the wizard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Welcome,
    Preset,
    Lesson(&'static str),
    Finish,
    Keys,
}

impl Step {
    fn name(self) -> &'static str {
        match self {
            Self::Welcome => "歡迎",
            Self::Preset => "習慣",
            Self::Lesson(id) => data::lesson(id).map_or("", |lesson| lesson.title),
            Self::Finish => "確認",
            Self::Keys => "按鍵",
        }
    }

    fn id(self) -> String {
        match self {
            Self::Welcome => "welcome".into(),
            Self::Preset => "preset".into(),
            Self::Lesson(id) => format!("lesson-{id}"),
            Self::Finish => "finish".into(),
            Self::Keys => "keys".into(),
        }
    }
}

impl Mode {
    fn lessons(&self) -> Vec<&'static data::Lesson> {
        match self {
            Self::Quick => LESSONS.iter().filter(|lesson| lesson.level == Level::Basic).collect(),
            Self::Full => LESSONS.iter().collect(),
            Self::NewSince(version) => {
                LESSONS.iter().filter(|lesson| data::version_after(lesson.since, version)).collect()
            }
            Self::KeysOnly => Vec::new(),
        }
    }

    fn steps(&self) -> Vec<Step> {
        let lessons = self.lessons().into_iter().map(|lesson| Step::Lesson(lesson.id));
        match self {
            Self::KeysOnly => vec![Step::Keys],
            Self::NewSince(_) => lessons.chain([Step::Finish, Step::Keys]).collect(),
            Self::Quick | Self::Full => {
                [Step::Welcome, Step::Preset].into_iter().chain(lessons).chain([Step::Finish, Step::Keys]).collect()
            }
        }
    }

    fn title(&self) -> &'static str {
        match self {
            Self::KeysOnly => "你會用到的按鍵",
            Self::NewSince(_) => "新功能導覽",
            Self::Quick | Self::Full => "設定嚮導",
        }
    }
}

impl From<WizardKind> for Mode {
    fn from(kind: WizardKind) -> Self {
        match kind {
            WizardKind::Quick => Self::Quick,
            WizardKind::NewSince(version) => Self::NewSince(version),
            WizardKind::KeysOnly => Self::KeysOnly,
        }
    }
}

/// A popover open on the page.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Popover {
    /// 查看: what the chosen preset changes.
    PresetChanges,
    /// 其他 N 項…: the changes the finish page has no room for.
    Rest,
    /// A change's setting, to adjust in place.
    Edit(String),
}

/// A step on its way out (the step transition).
#[derive(Clone, Copy, Debug)]
struct Leaving {
    step: Step,
    at: Instant,
    direction: i32,
}

/// The window's sizes (macOS `WizardLayout`): laid out to fit the smallest without scrolling.
mod layout {
    pub const MIN: (f32, f32) = (640., 640.);
    pub const DEFAULT: (f32, f32) = (680., 700.);
    /// The content column never grows past the largest size (macOS caps the window there).
    pub const MAX_W: f32 = 680.;
    pub const MARGIN: f32 = 32.;
    pub const HEADER: f32 = 52.;
    /// The footer's `H_LG` buttons sit in its middle: 24 px above the window's bottom edge.
    pub const FOOTER: f32 = 80.;
    pub const TOP_INSET: f32 = 8.;
}

pub struct WizardWindow {
    mode: Mode,
    index: usize,
    preset: &'static str,
    completed: HashSet<&'static str>,
    phrases: Vec<(String, String)>,
    keep_phrases: bool,
    /// The user's settings: written only by 套用.
    original: Settings,
    field: Entity<PracticeField>,
    /// The lesson whose task the field holds.
    field_task: Option<&'static str>,
    welcome_preview: Entity<UnifiedPreview>,
    look_preview: Entity<UnifiedPreview>,
    /// When the current step arrived (its parts come in one after another).
    arrived: Instant,
    leaving: Option<Leaving>,
    popover: Option<Popover>,
    error: Option<String>,
    focus: FocusHandle,
    pending_appearance: Option<Option<bool>>,
    /// Screenshots: everything at rest, nothing recorded.
    still: bool,
    /// Screenshots: the transition from the step before, held at this progress.
    pinned_transition: Option<f32>,
    _subscriptions: Vec<Subscription>,
}

impl WizardWindow {
    fn new(mode: Mode, dark: Option<bool>, still: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let original = app::settings(cx).clone();
        let session = app::service(cx).map(zt_core::service::Service::practice);
        let field = cx.new(|cx| PracticeField::new(session, &original, still, cx));
        let draft = field.read(cx).settings().clone();
        let welcome_preview = cx.new(|cx| {
            let mut preview = UnifiedPreview::new(cx).with_settings(draft.clone());
            preview.tabs = false;
            preview
        });
        let look_preview = cx.new(|cx| {
            let mut preview = UnifiedPreview::new(cx).with_settings(draft.clone());
            preview.tabs = false;
            preview.pin(PreviewState::List);
            preview
        });
        let subscription = cx.subscribe(&field, |this: &mut Self, _, event: &FieldEvent, cx| match event {
            FieldEvent::Completed => {
                if let Step::Lesson(id) = this.step() {
                    this.completed.insert(id);
                }
                cx.notify();
            }
            FieldEvent::Phrases(list) => {
                for (word, reading) in list {
                    if !this.phrases.iter().any(|(known, _)| known == word) {
                        this.phrases.push((word.clone(), reading.clone()));
                    }
                }
                cx.notify();
            }
        });
        // Like switching apps: when the window loses the keyboard, the composition is sent and the list closes.
        let activation = cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            if !window.is_window_active() {
                this.field.update(cx, |field, cx| field.end_session(cx));
            }
        });
        let mut wizard = Self {
            mode,
            index: 0,
            preset: "custom",
            completed: HashSet::new(),
            phrases: Vec::new(),
            keep_phrases: false,
            original,
            field,
            field_task: None,
            welcome_preview,
            look_preview,
            arrived: Instant::now(),
            leaving: None,
            popover: None,
            error: None,
            focus: cx.focus_handle(),
            pending_appearance: dark.map(Some),
            still,
            pinned_transition: None,
            _subscriptions: vec![subscription, activation],
        };
        wizard.enter(window, cx);
        wizard
    }

    /// Starts over in `mode` (the wizard was asked for again while open).
    fn restart(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        data::PRESETS[3].apply(&self.original, &draft);
        self.field.update(cx, |field, cx| field.settings_changed(cx));
        self.mode = mode;
        self.index = 0;
        self.preset = "custom";
        self.completed.clear();
        self.phrases.clear();
        self.keep_phrases = false;
        self.field_task = None;
        self.leaving = None;
        self.popover = None;
        self.arrived = Instant::now();
        window.set_window_title(self.mode.title());
        self.enter(window, cx);
        cx.notify();
    }

    /// The draft settings every choice changes.
    fn draft(&self, cx: &App) -> Settings {
        self.field.read(cx).settings().clone()
    }

    fn steps(&self) -> Vec<Step> {
        self.mode.steps()
    }

    fn step(&self) -> Step {
        let steps = self.steps();
        steps[self.index.min(steps.len() - 1)]
    }

    /// Where the user is among the steps the progress counts (not the welcome).
    fn progress(&self) -> (usize, usize) {
        let counted: Vec<Step> = self.steps().into_iter().filter(|step| *step != Step::Welcome).collect();
        let step = self.step();
        (counted.iter().position(|s| *s == step).map_or(0, |i| i + 1), counted.len())
    }

    fn lesson_done(&self, lesson: &data::Lesson) -> bool {
        lesson.task.is_none() || self.completed.contains(lesson.id)
    }

    // MARK: Moving

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go(self.index as isize + 1, window, cx);
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go(self.index as isize - 1, window, cx);
    }

    fn go(&mut self, target: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.steps().len() as isize;
        if target < 0 || target >= count {
            self.close(window, cx);
            return;
        }
        let from = self.step();
        let direction = if target as usize > self.index { 1 } else { -1 };
        self.index = target as usize;
        self.leaving = Some(Leaving { step: from, at: Instant::now(), direction });
        self.arrived = Instant::now();
        self.popover = None;
        self.enter(window, cx);
        cx.notify();
    }

    /// The step arrived: its practice starts and takes the keyboard.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.step() {
            Step::Lesson(id) if data::lesson(id).is_some_and(|lesson| lesson.task.is_some()) => {
                let task = data::lesson(id).and_then(|lesson| lesson.task);
                if self.field_task != Some(id) {
                    self.field_task = Some(id);
                    self.field.update(cx, |field, cx| field.start(task, cx));
                }
                self.field.update(cx, |field, cx| field.focus(window, cx));
            }
            _ => {
                self.field.update(cx, |field, cx| field.end_session(cx));
                window.focus(&self.focus, cx);
            }
        }
    }

    /// Switching between 快速設定 and 完整導覽 on the welcome keeps the place.
    fn choose_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.index = self.index.min(self.steps().len() - 1);
            cx.notify();
        }
    }

    /// A preset: the draft starts again from the user's settings with the preset's values.
    fn choose_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        let preset = &PRESETS[index];
        self.preset = preset.id;
        let draft = self.draft(cx);
        preset.apply(&self.original, &draft);
        self.field.update(cx, |field, cx| field.settings_changed(cx));
        cx.notify();
    }

    /// A choice changed the draft: the field and the demos follow at once.
    fn draft_changed(&mut self, cx: &mut Context<Self>) {
        self.field.update(cx, |field, cx| field.settings_changed(cx));
        cx.notify();
    }

    fn revert(&mut self, change: &data::Change, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        data::revert(change, &self.original, &draft);
        if self.popover == Some(Popover::Edit(change.id.clone())) {
            self.popover = None;
        }
        self.draft_changed(cx);
    }

    /// 套用: the changes go to the user's settings; the practice phrases only if kept.
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        let changes = data::changes(&self.original, &draft);
        if let Err(error) = data::apply(&changes, &self.original, &draft) {
            self.error = Some(format!("{error:#}"));
            cx.notify();
            return;
        }
        if self.keep_phrases
            && let Some(service) = app::service(cx)
        {
            for (word, reading) in &self.phrases {
                if let Err(error) = service.add_phrase(word, reading) {
                    log::warn!("keeping the practice phrase {word}: {error:#}");
                }
            }
        }
        self.mark_seen(cx);
        cx.refresh_windows();
        let keys = self.steps().iter().position(|step| *step == Step::Keys).unwrap_or(self.index + 1);
        self.go(keys as isize, window, cx);
    }

    /// 全部捨棄 and 全部跳過: nothing is written.
    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mark_seen(cx);
        self.close(window, cx);
    }

    fn mark_seen(&self, cx: &mut App) {
        if !self.still {
            mark_seen(cx);
        }
    }

    /// Closes the wizard only: the settings window, if open, comes back to the front.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, |field, cx| field.end_session(cx));
        cx.set_global(OpenWizard(None));
        window.remove_window();
        cx.defer(front_settings);
    }

    /// The step's main button (Return when the practice field does not take it).
    fn primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.step() {
            Step::Welcome | Step::Preset => self.next(window, cx),
            Step::Lesson(id) => {
                if data::lesson(id).is_some_and(|lesson| self.lesson_done(lesson)) {
                    self.next(window, cx);
                }
            }
            Step::Finish => self.apply(window, cx),
            Step::Keys => self.close(window, cx),
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.key == "enter" && !keystroke.modifiers.modified() {
            if self.popover.take().is_some() {
                cx.notify();
                return;
            }
            self.primary(window, cx);
            cx.stop_propagation();
        } else if keystroke.key == "escape" && self.popover.take().is_some() {
            cx.notify();
            cx.stop_propagation();
        }
    }
}

/// Brings the settings window forward if it is open.
fn front_settings(cx: &mut App) {
    crate::shell::bring_forward(cx);
}

// MARK: - Opening

/// The wizard window while it is open, and its view.
struct OpenWizard(Option<(AnyWindowHandle, Entity<WizardWindow>)>);

impl Global for OpenWizard {}

/// How the window opens (screenshots force an appearance and a size and hold still).
#[derive(Default)]
struct Opening {
    dark: Option<bool>,
    size: Option<(f32, f32)>,
    still: bool,
}

/// Opens (or brings forward) the wizard window showing `kind`.
pub fn open(kind: WizardKind, cx: &mut App) {
    if let Err(error) = open_window(kind.into(), Opening::default(), cx) {
        log::error!("opening the setup wizard: {error:#}");
    }
}

fn open_window(mode: Mode, opening: Opening, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<WizardWindow>)> {
    if !cx.has_global::<Navigation>() {
        cx.set_global(Navigation::new(Page::Intro));
    }
    if !cx.has_global::<OpenWizard>() {
        practice::bind_keys(cx);
        cx.set_global(OpenWizard(None));
    }
    if let Some((handle, view)) = cx.try_global::<OpenWizard>().and_then(|open| open.0.clone()) {
        let reopened = handle.update(cx, |_, window, cx| {
            view.update(cx, |wizard, cx| wizard.restart(mode.clone(), window, cx));
            window.activate_window();
        });
        if reopened.is_ok() {
            return Ok((handle, view));
        }
    }
    let (width, height) = opening.size.unwrap_or(layout::DEFAULT);
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let title = mode.title();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(layout::MIN.0), px(layout::MIN.1))),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("zhuyintype".into()),
        titlebar: Some(gpui_kit::TitlebarOptions { title: Some(title.into()), ..TitleBar::title_bar_options() }),
        ..TitleBar::window_options()
    };
    let (dark, still) = (opening.dark, opening.still);
    let (handle, view) = gpui_kit::open_window(options, cx, move |window, cx| {
        if !cx.has_global::<theme::Tokens>() {
            theme::init(window.appearance(), cx);
        }
        #[cfg(windows)]
        crate::platform::windows::native_frame(window);
        cx.new(|cx| WizardWindow::new(mode, dark, still, window, cx))
    })?;
    cx.set_global(OpenWizard(Some((handle, view.clone()))));
    Ok((handle, view))
}

// MARK: - First use and new lessons

/// The settings key that records the lessons' version the user has seen (macOS `setupWizardSeenVersion`).
const SEEN_KEY: &str = "setupWizardSeenVersion";

fn seen_version(cx: &App) -> Option<String> {
    app::settings(cx).get(SEEN_KEY).as_str().map(str::to_owned)
}

fn mark_seen(cx: &mut App) {
    if let Err(error) = app::settings(cx).set(SEEN_KEY, Value::from(data::current_version())) {
        log::warn!("recording the wizard's version: {error:#}");
    }
    cx.refresh_windows();
}

/// The version whose lessons the user saw last, when this version has lessons they have not seen (macOS
/// `WizardProgress.hasNewLessons` / `seenVersion`); None hides 新功能導覽.
pub fn new_lessons_since(cx: &App) -> Option<String> {
    let seen = seen_version(cx)?;
    data::version_after(data::current_version(), &seen).then_some(seen)
}

/// The first opening of the settings after install also opens the wizard, once (macOS `showOnFirstUse`).
pub fn show_on_first_use(cx: &mut App) {
    if seen_version(cx).is_some() {
        return;
    }
    mark_seen(cx);
    cx.defer(|cx| open(WizardKind::Quick, cx));
}

// MARK: - Screenshots

/// `--wizard SPEC` / `ZT_WIZARD=SPEC`: the wizard alone, still, for screenshots and the UI checks.
///
/// `SPEC` is `MODE[@STEP][;OPTION…]`: MODE `quick`, `full`, `new:VERSION` or `keys`; STEP `welcome`, `preset`,
/// `finish`, `keys` or a lesson id; OPTIONs `set=KEY:VALUE` (a draft setting; VALUE is JSON or text),
/// `preset=ID`, `type=KEYS` (typed into the practice field, see `PracticeField::feed`), `done` (the lesson's
/// practice is complete), `transition=PERCENT` (the transition from the step before, held there), `edit=ID`,
/// `changes` and `rest` (a popover open), `live` (animations play).  A lesson only the full tour has (`quick@look`)
/// opens the full tour.
pub fn open_preview(spec: &str, dark: Option<bool>, window_size: Option<(f32, f32)>, cx: &mut App) {
    let mut parts = spec.split(';');
    let head = parts.next().unwrap_or_default();
    let (mode, step) = head.split_once('@').map_or((head, None), |(mode, step)| (mode, Some(step)));
    let mode = match mode {
        "full" => Mode::Full,
        "keys" => Mode::KeysOnly,
        other => match other.strip_prefix("new:") {
            Some(version) => Mode::NewSince(version.to_owned()),
            None => Mode::Quick,
        },
    };
    let options: Vec<(String, String)> = parts
        .map(|part| {
            part.split_once('=').map_or((part.to_owned(), String::new()), |(k, v)| (k.to_owned(), v.to_owned()))
        })
        .collect();
    let live = options.iter().any(|(key, _)| key == "live");
    let opening = Opening { dark, size: window_size, still: !live };
    let (handle, view) = match open_window(mode, opening, cx) {
        Ok(opened) => opened,
        Err(error) => {
            log::error!("opening the setup wizard: {error:#}");
            return;
        }
    };
    let step = step.map(str::to_owned);
    let _ = handle.update(cx, move |_, window, cx| {
        view.update(cx, |wizard, cx| {
            for (key, value) in &options {
                match key.as_str() {
                    "preset" => {
                        if let Some(index) = PRESETS.iter().position(|preset| preset.id == value) {
                            wizard.choose_preset(index, cx);
                        }
                    }
                    "set" => {
                        if let Some((setting, text)) = value.split_once(':') {
                            let parsed = serde_json::from_str(text).unwrap_or_else(|_| Value::from(text));
                            let draft = wizard.draft(cx);
                            if let Err(error) = draft.set(setting, parsed) {
                                log::warn!("--wizard set {setting}: {error:#}");
                            }
                            wizard.draft_changed(cx);
                        }
                    }
                    _ => {}
                }
            }
            // A lesson only the full tour has (`quick@look`) switches to it rather than staying on the welcome.
            let position = |wizard: &WizardWindow, step: &str| {
                wizard.steps().iter().position(|s| s.id() == step || s.id() == format!("lesson-{step}"))
            };
            if let Some(step) = &step
                && wizard.mode == Mode::Quick
                && position(wizard, step).is_none()
                && LESSONS.iter().any(|lesson| lesson.id == step)
            {
                wizard.mode = Mode::Full;
            }
            if let Some(step) = &step
                && let Some(index) = position(wizard, step)
            {
                wizard.index = index;
                wizard.enter(window, cx);
            }
            for (key, value) in &options {
                match key.as_str() {
                    "type" => wizard.field.update(cx, |field, cx| field.feed(value, cx)),
                    "done" => {
                        if let Step::Lesson(id) = wizard.step() {
                            wizard.completed.insert(id);
                        }
                    }
                    "transition" => {
                        wizard.pinned_transition = value.parse::<f32>().ok().map(|percent| percent / 100.);
                    }
                    "edit" => wizard.popover = Some(Popover::Edit(value.clone())),
                    "changes" => wizard.popover = Some(Popover::PresetChanges),
                    "rest" => wizard.popover = Some(Popover::Rest),
                    _ => {}
                }
            }
            window.activate_window();
            cx.notify();
        })
    });
}

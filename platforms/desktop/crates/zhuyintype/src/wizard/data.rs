//! What the setup wizard teaches and changes, as data (macOS `Habits.swift`, the snapshots and changes of
//! `SettingsRegistry.swift`): the ways of using each feature (styles, in exclusive or combinable groups), the habit
//! presets, the lessons with their practice tasks, the 原本 → 改成 list between two sets of settings, and the key
//! summary.  Everything reads and writes through `zt_core::settings::Settings` (the user's, or the wizard's draft in
//! memory) and `zt_core::shortcuts::Shortcuts` on it; nothing here draws.

use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use serde_json::{Map, Value};
use zt_core::keys::{code, modifier};
use zt_core::practice::{PracticeGoal, PracticeOutcome};
use zt_core::settings::{self, Descriptor, Kind, Settings};
use zt_core::shortcuts::{self, Binding, Shortcuts};

use crate::candidate::preview::PreviewState;
use crate::candidate::style::CurrentKeys;

// MARK: - Styles

/// How a style reads and writes the settings.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleKind {
    /// One value of a setting (an exclusive choice; for a switch, `true` is the style on).
    Value(&'static str, Value),
    /// A switch that can be on with others (a combinable style).
    Switch(&'static str),
    /// One key of a command, alongside its other keys.
    Key(&'static str, Binding),
}

/// One way of using a feature (macOS `HabitStyle`).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub title: String,
    pub detail: Option<String>,
    pub kind: StyleKind,
}

impl Style {
    fn value(key: &'static str, value: impl Into<Value>, title: impl Into<String>, detail: Option<String>) -> Self {
        Self { title: title.into(), detail, kind: StyleKind::Value(key, value.into()) }
    }

    fn switch(key: &'static str, title: &str, detail: Option<&str>) -> Self {
        Self { title: title.to_owned(), detail: detail.map(str::to_owned), kind: StyleKind::Switch(key) }
    }

    fn key(command: &'static str, key: i32, modifiers: u32, title: &str) -> Self {
        let binding = Binding { key, modifiers };
        let spoken = binding.spoken();
        let detail = (spoken != title && binding.label() != title).then_some(spoken);
        Self { title: title.to_owned(), detail, kind: StyleKind::Key(command, binding) }
    }

    /// The settings use this style now.
    pub fn is_active(&self, settings: &Settings) -> bool {
        match &self.kind {
            StyleKind::Value(key, value) => settings.get(key) == *value,
            StyleKind::Switch(key) => settings.bool(key),
            StyleKind::Key(command, binding) => {
                Shortcuts::new(settings.clone()).bindings(command).iter().any(|b| b.binding == *binding)
            }
        }
    }

    /// Turns the style on (an exclusive style replaces the others of its group) or off (combinable styles only).  A
    /// key goes through the key conflict check: a key another command has is not taken.
    pub fn set(&self, settings: &Settings, on: bool) {
        let result = match &self.kind {
            StyleKind::Value(key, value) if on => settings.set(key, value.clone()),
            StyleKind::Value(key, Value::Bool(true)) => settings.set(key, Value::Bool(false)),
            StyleKind::Value(..) => Ok(()),
            StyleKind::Switch(key) => settings.set(key, Value::Bool(on)),
            StyleKind::Key(command, binding) => {
                let keys = Shortcuts::new(settings.clone());
                let list = keys.bindings(command);
                let index = list.iter().position(|b| b.binding == *binding);
                match (on, index) {
                    (true, None) if list.len() < shortcuts::MAX_BINDINGS => {
                        keys.record(command, binding.key, binding.modifiers, None)
                    }
                    (false, Some(index)) => keys.remove(command, index),
                    _ => Ok(()),
                }
            }
        };
        if let Err(error) = result {
            log::info!("wizard style {}: {error:#}", self.title);
        }
    }

    /// A style that is a key of a command: the lesson offers 「自行設定…」.
    pub fn is_key(&self) -> bool {
        matches!(self.kind, StyleKind::Key(..))
    }
}

/// A group of styles: one at a time (a choice), or any number at once (macOS `HabitGroup`).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: &'static str,
    pub title: &'static str,
    pub exclusive: bool,
    pub styles: Vec<Style>,
}

/// The options of a choice setting as styles: the descriptor's labels, and its per-value details.
fn choice_styles(key: &'static str, with_details: bool) -> Vec<Style> {
    let Some(descriptor) = settings::descriptor(key) else { return Vec::new() };
    descriptor
        .options
        .iter()
        .map(|choice| {
            let detail = with_details.then(|| descriptor.subtitle(&choice.value).map(str::to_owned)).flatten();
            Style::value(key, choice.value.clone(), choice.label.clone(), detail)
        })
        .collect()
}

/// The group `id` (macOS `Habits`).
pub fn group(id: &str) -> Option<Group> {
    let (title, exclusive, styles) = match id {
        "space" => ("空白鍵", true, choice_styles("spaceAction", true)),
        "escape" => ("Esc", true, choice_styles("escape", false)),
        "punctuation" => (
            "標點按法",
            true,
            vec![
                Style::value(
                    "punctuationKeys",
                    "windows",
                    "Windows",
                    Some("Ctrl 加符號鍵打中文標點，⇧ 加符號鍵打鍵上的符號".into()),
                ),
                Style::value("punctuationKeys", "mac", "macOS", Some("⇧ 加符號鍵打中文標點".into())),
            ],
        ),
        "modeSwitch" => (
            "中英切換",
            false,
            vec![
                Style::switch("shiftSwitch", "單按 Shift", Some("在可以打字的地方單按一下")),
                Style::switch("feature.capsLockEnglish", "Caps Lock 打英文", Some("開著時打英文")),
            ],
        ),
        "layout" => (
            "排列",
            true,
            vec![
                Style::value("candidateLayout", "vertical", "直式", Some("↑ ↓ 移動反白".into())),
                Style::value("candidateLayout", "horizontal", "橫式", Some("← → 移動反白，↑ ↓ 翻頁".into())),
            ],
        ),
        "paging" => (
            "翻頁",
            false,
            vec![
                Style::key("nextPage", code::TAB, 0, "Tab"),
                Style::key("nextPage", code::PAGE_DOWN, 0, "Page Down"),
                Style::key("nextPage", code::DOWN, modifier::ALT, "Alt+↓"),
                Style::switch("pageAtListEdge", "在頁底按 ↓ 翻到下一頁", None),
            ],
        ),
        "selection" => (
            "選字鍵",
            true,
            vec![
                Style::value("selectionKeys", "123456789", "1–9", None),
                Style::value("selectionKeys", "asdfghjkl", "A S D F G H J K L", None),
            ],
        ),
        "anchor" => ("游標在句中時改", true, choice_styles("candidateAnchor", false)),
        "typo" => ("按錯鍵修正", true, choice_styles("typoTolerance", true)),
        "context" => (
            "前文",
            true,
            vec![
                Style::value("usesPrecedingText", true, "參考游標前的文字", None),
                Style::value("usesPrecedingText", false, "不參考", None),
            ],
        ),
        "correctionLog" => (
            "記錄改字",
            true,
            vec![
                Style::value("recordsCorrections", false, "不記錄", None),
                Style::value("recordsCorrections", true, "記錄改字", None),
            ],
        ),
        "fuzzy" => (
            "模糊音",
            false,
            ["fuzzy.zZh", "fuzzy.cCh", "fuzzy.sSh", "fuzzy.enEng", "fuzzy.anAng"]
                .iter()
                .filter_map(|key| {
                    let descriptor = settings::descriptor(key)?;
                    Some(Style::switch(&descriptor.key, &descriptor.title, None))
                })
                .collect(),
        ),
        "density" => ("間距", true, choice_styles("candidateDensity", false)),
        "placement" => ("位置", true, choice_styles("candidatePlacement", false)),
        _ => return None,
    };
    let id = GROUP_IDS.iter().find(|known| **known == id)?;
    Some(Group { id, title, exclusive, styles })
}

const GROUP_IDS: &[&str] = &[
    "space",
    "escape",
    "punctuation",
    "modeSwitch",
    "layout",
    "paging",
    "selection",
    "anchor",
    "typo",
    "context",
    "correctionLog",
    "fuzzy",
    "density",
    "placement",
];

// MARK: - Presets

/// A named set of settings, applied as a list of changes (macOS `SettingsPreset`).  Only behaviour with a source is in
/// a preset (docs/product/PRESETS.md); the settings it leaves out keep their values.
pub struct Preset {
    pub id: &'static str,
    pub title: &'static str,
    /// What it sets, a line each: the keys it is about (drawn as key caps) and the words.
    pub lines: &'static [(&'static [&'static str], &'static str)],
    values: fn() -> Vec<(String, Value)>,
}

fn windows_values() -> Vec<(String, Value)> {
    vec![
        ("shiftSwitch".into(), true.into()),
        ("shiftToggle".into(), "both".into()),
        ("startInEnglish".into(), false.into()),
        ("feature.controlPunctuation".into(), true.into()),
        ("selectionKeys".into(), "123456789".into()),
    ]
}

fn mac_values() -> Vec<(String, Value)> {
    vec![
        ("spaceAction".into(), "candidates".into()),
        ("shiftSwitch".into(), false.into()),
        ("feature.capsLockEnglish".into(), true.into()),
        ("feature.optionSymbols".into(), true.into()),
        ("selectionKeys".into(), "123456789".into()),
    ]
}

/// Every setting's default except the keys and the feature switches, with the Shift tap on.
fn default_values() -> Vec<(String, Value)> {
    let mut values: Vec<(String, Value)> = settings::descriptors()
        .iter()
        .filter(|d| !d.key.starts_with("feature."))
        .map(|d| (d.key.clone(), d.default.clone()))
        .collect();
    values.retain(|(key, _)| key != "shiftSwitch");
    values.push(("shiftSwitch".into(), true.into()));
    values
}

fn no_values() -> Vec<(String, Value)> {
    Vec::new()
}

pub const PRESETS: [Preset; 4] = [
    Preset {
        id: "windows",
        title: "Windows 注音的習慣",
        lines: &[
            (&["Shift"], "單按切換中英"),
            (&["Ctrl", "，"], "打中文標點"),
            (&["1–9"], "選字"),
            (&[], "和目前的預設相同"),
        ],
        values: windows_values,
    },
    Preset {
        id: "mac",
        title: "macOS 注音的習慣",
        lines: &[
            (&["空白"], "開啟候選字視窗"),
            (&["Caps Lock"], "切換中英"),
            (&["Shift"], "不切換中英"),
            (&["1–9"], "選字"),
        ],
        values: mac_values,
    },
    Preset {
        id: "default",
        title: "ZhuyinType 預設",
        lines: &[(&["空白"], "送出並輸入空白"), (&["Ctrl", "，"], "打中文標點"), (&["Shift"], "單按切換中英")],
        values: default_values,
    },
    Preset {
        id: "custom",
        title: "自訂",
        lines: &[(&[], "從目前的設定開始"), (&[], "每一步自己選")],
        values: no_values,
    },
];

/// Every setting and the keys of `settings`, to copy into another store.
pub fn copy_of(settings: &Settings) -> BTreeMap<String, Value> {
    let mut values: BTreeMap<String, Value> =
        settings::descriptors().iter().map(|d| (d.key.clone(), settings.get(&d.key))).collect();
    values.insert(shortcuts::STORAGE_KEY.to_owned(), settings.get(shortcuts::STORAGE_KEY));
    values
}

impl Preset {
    /// Makes `draft` the user's settings with this preset's values.
    pub fn apply(&self, original: &Settings, draft: &Settings) {
        let mut values = copy_of(original);
        values.extend((self.values)());
        if let Err(error) = draft.assign(&values) {
            log::warn!("wizard preset {}: {error:#}", self.id);
        }
    }
}

// MARK: - Lessons

/// Which tours a lesson is in: 快速設定 takes the basic ones, 完整導覽 all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Basic,
    Full,
    Optional,
    Advanced,
}

/// One thing a task asks for (`zt_core::practice::PracticeGoal`, written as data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Contains(&'static str),
    Pick(&'static str),
    TurnPage,
    SwitchMode(usize),
    PickCorrection,
    /// Any user phrase added.
    AnyPhrase,
}

/// A goal checked against what the field reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Core(PracticeGoal),
    AnyPhrase,
}

impl Check {
    pub fn is_met(&self, outcome: &PracticeOutcome) -> bool {
        match self {
            Self::Core(goal) => goal.is_met(outcome),
            Self::AnyPhrase => !outcome.phrases.is_empty(),
        }
    }
}

impl Goal {
    pub fn check(self) -> Check {
        match self {
            Self::Contains(text) => Check::Core(PracticeGoal::Contains(text.to_owned())),
            Self::Pick(word) => Check::Core(PracticeGoal::Pick(Some(word.to_owned()))),
            Self::TurnPage => Check::Core(PracticeGoal::TurnPage),
            Self::SwitchMode(times) => Check::Core(PracticeGoal::SwitchMode(times)),
            Self::PickCorrection => Check::Core(PracticeGoal::PickCorrection),
            Self::AnyPhrase => Check::AnyPhrase,
        }
    }
}

/// A lesson's practice: the prompt, a hint, the text the field starts with, and what completes it.
#[derive(Clone, Copy, Debug)]
pub struct Task {
    pub prompt: &'static str,
    pub hint: Option<&'static str>,
    pub initial: &'static str,
    pub goals: &'static [Goal],
}

/// What a lesson without practice shows beside its choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Preview(PreviewState),
    PunctuationWidth,
    CorrectionLog,
}

/// One step of the tour (macOS `Lesson`).
#[derive(Clone, Copy, Debug)]
pub struct Lesson {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub level: Level,
    pub groups: &'static [&'static str],
    pub task: Option<Task>,
    pub demo: Option<Demo>,
    /// The version that added it (新功能導覽).
    pub since: &'static str,
}

const BASE: &str = "0.4.2";

pub const LESSONS: [Lesson; 13] = [
    Lesson {
        id: "typing",
        title: "打字與空白鍵",
        detail: "打完字以後，空白鍵做什麼由你決定。",
        level: Level::Basic,
        groups: &["space"],
        task: Some(Task {
            prompt: "打出「今天天氣很好」",
            hint: Some("ㄐㄧㄣ ㄊㄧㄢ ㄊㄧㄢ ㄑㄧˋ ㄏㄣˇ ㄏㄠˇ"),
            initial: "",
            goals: &[Goal::Contains("今天天氣很好")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "punctuation",
        title: "標點",
        detail: "按住修飾鍵，再按符號鍵打出標點。",
        level: Level::Basic,
        groups: &["punctuation"],
        task: Some(Task {
            prompt: "打出「好，謝謝。」",
            hint: None,
            initial: "",
            goals: &[Goal::Contains("，"), Goal::Contains("。")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "punctuationWidth",
        title: "標點全半形",
        detail: "中文預設全形標點，英數與空白維持半形。切換只影響之後輸入的標點。",
        level: Level::Full,
        groups: &[],
        task: None,
        demo: Some(Demo::PunctuationWidth),
        since: "0.5.1",
    },
    Lesson {
        id: "modeSwitch",
        title: "中英切換",
        detail: "切換時，正在組的字會先送出。",
        level: Level::Basic,
        groups: &["modeSwitch"],
        task: Some(Task {
            prompt: "切換到英文打「OK」，再切回中文打「好」",
            hint: None,
            initial: "",
            goals: &[Goal::Contains("OK好"), Goal::SwitchMode(2)],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "selecting",
        title: "選字",
        detail: "候選字的排列，和用來選字的鍵。",
        level: Level::Basic,
        groups: &["layout", "selection"],
        task: Some(Task {
            prompt: "打 ㄇㄧㄥˊ，用選字鍵選「名」",
            hint: None,
            initial: "",
            goals: &[Goal::Pick("名")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "paging",
        title: "翻頁",
        detail: "這些翻頁方式同時有效，不用的可以關掉。",
        level: Level::Basic,
        groups: &["paging"],
        task: Some(Task {
            prompt: "打 ㄇㄧㄥˊ，翻到下一頁，選「暝」",
            hint: None,
            initial: "",
            goals: &[Goal::Pick("暝"), Goal::TurnPage],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "editing",
        title: "改字",
        detail: "按 ↓ 列出其他的字，選一個換掉。",
        level: Level::Basic,
        groups: &["anchor"],
        task: Some(Task {
            prompt: "打「謝謝你」，再把「你」改成「妳」",
            hint: None,
            initial: "",
            goals: &[Goal::Contains("謝謝妳"), Goal::Pick("妳")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "typo",
        title: "按錯鍵與修正清單",
        detail: "按到相鄰的鍵、多按或漏按時，試著修正。",
        level: Level::Full,
        groups: &["typo"],
        task: Some(Task {
            prompt: "照提示按錯一個鍵，再用修正清單選到正確的字",
            hint: None,
            initial: "",
            goals: &[Goal::PickCorrection],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "fuzzy",
        title: "模糊音",
        detail: "容易混淆的兩個音互相通用。",
        level: Level::Optional,
        groups: &["fuzzy"],
        task: Some(Task {
            prompt: "開啟「ㄙ 與 ㄕ」，用 ㄙㄨ ㄈㄨˊ 打出「舒服」",
            hint: None,
            initial: "",
            goals: &[Goal::Contains("舒服")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "context",
        title: "前文",
        detail: "參考游標前的文字選字，不會存下。",
        level: Level::Full,
        groups: &["context"],
        task: Some(Task {
            prompt: "在「天氣」後面接著打出「很好」",
            hint: None,
            initial: "天氣",
            goals: &[Goal::Contains("天氣很好")],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "phrase",
        title: "加入自訂詞",
        detail: "選取 1 到 6 個字，按 Return 加入。",
        level: Level::Full,
        groups: &[],
        task: Some(Task {
            prompt: "選取「王小明」，按 Return 加入",
            hint: None,
            initial: "",
            goals: &[Goal::AnyPhrase],
        }),
        demo: None,
        since: BASE,
    },
    Lesson {
        id: "look",
        title: "候選字視窗的樣子",
        detail: "預覽就是真正的候選字視窗。",
        level: Level::Full,
        groups: &["density", "placement", "selection"],
        task: None,
        demo: Some(Demo::Preview(PreviewState::List)),
        since: BASE,
    },
    Lesson {
        id: "correctionLog",
        title: "改字記錄",
        detail: "記下改字的過程來改進選字，只存在這台電腦。",
        level: Level::Advanced,
        groups: &["correctionLog"],
        task: None,
        demo: Some(Demo::CorrectionLog),
        since: BASE,
    },
];

pub fn lesson(id: &str) -> Option<&'static Lesson> {
    LESSONS.iter().find(|lesson| lesson.id == id)
}

impl Lesson {
    /// The lesson's groups.
    pub fn groups(&self) -> Vec<Group> {
        self.groups.iter().filter_map(|id| group(id)).collect()
    }
}

/// Compares two dotted versions numerically ("0.5.1" > "0.4.10" > "0.4.2").
pub fn version_after(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (parts(a), parts(b));
    for index in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(index).copied().unwrap_or(0), b.get(index).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// The newest lesson's version: what 「已看過」 records.
pub fn current_version() -> &'static str {
    LESSONS.iter().map(|lesson| lesson.since).fold(BASE, |a, b| if version_after(b, a) { b } else { a })
}

// MARK: - Changes

/// What a change is to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A setting: its key, the value before and after.
    Setting(&'static str, Value, Value),
    /// The keys of a command.
    Keys(&'static str),
}

/// One setting changed: 原本 → 改成 (macOS `SettingChange`).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub id: String,
    pub title: String,
    pub old: String,
    pub new: String,
    pub target: Target,
}

/// A value's label: an option's title, 「開啟」「關閉」, a number with its unit.
pub fn value_label(descriptor: &Descriptor, value: &Value) -> String {
    match descriptor.kind {
        Kind::Toggle => if value.as_bool().unwrap_or(false) { "開啟" } else { "關閉" }.to_owned(),
        Kind::Choice => descriptor.label(value).unwrap_or("—").to_owned(),
        Kind::Number => {
            let unit = descriptor.extra.get("unit").and_then(Value::as_str).unwrap_or_default();
            match value.as_f64() {
                Some(n) if n.fract() == 0.0 => format!("{}{unit}", n as i64),
                Some(n) => format!("{n:.1}{unit}"),
                None => "—".to_owned(),
            }
        }
        Kind::Text => value.as_str().unwrap_or_default().to_owned(),
    }
}

fn keys_label(list: Option<&Vec<shortcuts::BindingLabel>>) -> String {
    match list {
        Some(list) if !list.is_empty() => list.iter().map(|b| b.label.as_str()).collect::<Vec<_>>().join("、"),
        _ => "沒有按鍵".to_owned(),
    }
}

/// What differs from `original` to `draft`, in the settings' order, then the keys.
pub fn changes(original: &Settings, draft: &Settings) -> Vec<Change> {
    let mut list: Vec<Change> = settings::descriptors()
        .iter()
        .filter_map(|descriptor| {
            let (old, new) = (original.get(&descriptor.key), draft.get(&descriptor.key));
            (old != new).then(|| Change {
                id: descriptor.key.clone(),
                title: descriptor.title.clone(),
                old: value_label(descriptor, &old),
                new: value_label(descriptor, &new),
                target: Target::Setting(descriptor.key.as_str(), old, new),
            })
        })
        .collect();
    let (before, after) = (Shortcuts::new(original.clone()).all(), Shortcuts::new(draft.clone()).all());
    for command in Shortcuts::commands() {
        let (old, new) = (keys_label(before.get(command.id)), keys_label(after.get(command.id)));
        if old != new {
            list.push(Change {
                id: format!("keys.{}", command.id),
                title: command.label.to_owned(),
                old,
                new,
                target: Target::Keys(command.id),
            });
        }
    }
    list
}

fn stored_keys(settings: &Settings) -> Map<String, Value> {
    match settings.get(shortcuts::STORAGE_KEY) {
        Value::Object(object) => object,
        _ => Map::new(),
    }
}

/// 改回 one change in the draft.
pub fn revert(change: &Change, original: &Settings, draft: &Settings) {
    let result = match &change.target {
        Target::Setting(key, old, _) => draft.set(key, old.clone()),
        Target::Keys(command) => {
            let mut keys = stored_keys(draft);
            match stored_keys(original).get(*command) {
                Some(entry) => keys.insert((*command).to_owned(), entry.clone()),
                None => keys.remove(*command),
            };
            draft.set(shortcuts::STORAGE_KEY, Value::Object(keys))
        }
    };
    if let Err(error) = result {
        log::warn!("wizard revert {}: {error:#}", change.id);
    }
}

/// 套用: the changes go to the user's settings, all or none.
pub fn apply(changes: &[Change], original: &Settings, draft: &Settings) -> anyhow::Result<()> {
    let mut values = BTreeMap::new();
    for change in changes {
        match &change.target {
            Target::Setting(key, _, new) => {
                values.insert((*key).to_owned(), new.clone());
            }
            Target::Keys(_) => {
                values.insert(shortcuts::STORAGE_KEY.to_owned(), draft.get(shortcuts::STORAGE_KEY));
            }
        }
    }
    original.assign(&values)
}

// MARK: - Key summary

/// A card of 你會用到的按鍵: its title, icon and keys (each a list of chips) with what they do.
pub struct KeyGroup {
    pub title: &'static str,
    pub icon: IconName,
    pub items: Vec<(Vec<String>, String)>,
}

/// The first keys of each card.
pub const KEYS_PER_CARD: usize = 4;

/// The keys of `settings`, by what they are for, only what is on (macOS `KeySummary.groups`).
pub fn key_groups(settings: &Settings) -> Vec<KeyGroup> {
    let keys = Shortcuts::new(settings.clone());
    let current = CurrentKeys::sandboxed(settings);
    let commands = |section: &[&str]| -> Vec<(Vec<String>, String)> {
        Shortcuts::commands()
            .iter()
            .filter(|command| section.contains(&command.id) && keys.is_enabled(command.id))
            .filter_map(|command| {
                let list: Vec<String> = keys.bindings(command.id).into_iter().take(2).map(|b| b.label).collect();
                (!list.is_empty()).then(|| (list, command.label.to_owned()))
            })
            .collect()
    };
    let space =
        settings::descriptor("spaceAction").map(|d| value_label(d, &settings.get("spaceAction"))).unwrap_or_default();
    let escape = if settings.string("escape") == "composition" {
        "清除注音，再按一次清除組字"
    } else {
        "清除注音"
    };
    let mut typing = vec![(vec!["空白".to_owned()], space), (vec!["Esc".to_owned()], escape.to_owned())];
    if let Some(mode) = current.mode_key() {
        typing.push((vec![mode], "切換中英".to_owned()));
    }
    typing.extend(commands(&["commitWithNewline", "deleteWord"]));
    let mut list =
        vec![(vec!["↓".to_owned()], "開啟候選字清單".to_owned()), (vec![current.selection()], "選字".to_owned())];
    list.extend(commands(&[
        "nextPage",
        "previousPage",
        "singleCharacterList",
        "correctionsList",
        "previousCharacter",
        "nextCharacter",
    ]));
    let editing = commands(&["typeZhuyin", "selectLeft", "selectRight"]);
    [
        KeyGroup { title: "打字", icon: IconName::Keyboard, items: typing },
        KeyGroup { title: "候選字清單", icon: IconName::List, items: list },
        KeyGroup { title: "改字與選取", icon: IconName::TextCursorInput, items: editing },
    ]
    .into_iter()
    .filter(|group| !group.items.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(version_after("0.5.1", "0.4.2"));
        assert!(version_after("0.4.10", "0.4.2"));
        assert!(!version_after("0.4.2", "0.4.2"));
        assert_eq!(current_version(), "0.5.1");
    }

    #[test]
    fn every_lesson_group_exists() {
        for lesson in &LESSONS {
            assert_eq!(lesson.groups().len(), lesson.groups.len(), "{}", lesson.id);
        }
        assert_eq!(LESSONS.iter().filter(|lesson| lesson.level == Level::Basic).count(), 6);
    }

    #[test]
    fn presets_become_changes_that_revert_and_apply() {
        let original = Settings::open(None);
        let draft = original.sandbox();
        PRESETS[1].apply(&original, &draft);
        let list = changes(&original, &draft);
        assert!(list.iter().any(|c| c.id == "spaceAction" && c.new == "開啟候選字視窗"));
        assert!(list.iter().any(|c| c.id == "shiftSwitch"));
        let space = list.iter().find(|c| c.id == "spaceAction").unwrap().clone();
        revert(&space, &original, &draft);
        assert!(!changes(&original, &draft).iter().any(|c| c.id == "spaceAction"));
        apply(&changes(&original, &draft), &original, &draft).unwrap();
        assert!(!original.bool("shiftSwitch"));
        assert!(changes(&original, &draft).is_empty());
    }

    #[test]
    fn key_styles_change_and_revert_the_draft_keys() {
        let original = Settings::open(None);
        let draft = original.sandbox();
        let paging = group("paging").unwrap();
        let alt_down = &paging.styles[2];
        assert!(alt_down.is_active(&draft));
        alt_down.set(&draft, false);
        assert!(!alt_down.is_active(&draft));
        let list = changes(&original, &draft);
        let change = list.iter().find(|c| c.id == "keys.nextPage").unwrap();
        assert_eq!(change.old, "Tab、PgDn、Alt+↓");
        assert_eq!(change.new, "Tab、PgDn");
        revert(change, &original, &draft);
        assert!(alt_down.is_active(&draft));
        assert!(changes(&original, &draft).is_empty());
        assert!(!original.bool("feature.pageKeys") || alt_down.is_active(&original));
    }
}

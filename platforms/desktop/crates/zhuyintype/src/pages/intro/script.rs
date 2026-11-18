//! The 介紹 page's story (macOS `IntroScript`): Zhuyin typed, the sentence converts, ↓ opens the list, the highlight
//! moves, the corrections list, a pick, a phrase selected and added, the text sent, the 中/英 badge.  Built from the
//! current settings and keys: a feature that is off leaves its steps out, and every key shown is the user's.

use zt_core::presentation::{CandidateRow, Presentation};
use zt_core::settings::Settings;

use crate::ui::shortcuts::store::KeyStore;

/// A piece of the composition: underlined, or the focused clause (the one the list changes).
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub text: String,
    pub focus: bool,
}

fn seg(text: &str) -> Segment {
    Segment { text: text.to_owned(), focus: false }
}

fn focus(text: &str) -> Segment {
    Segment { text: text.to_owned(), focus: true }
}

/// One moment of the story.
#[derive(Clone, Debug)]
pub struct Step {
    /// Text already in the document.
    pub committed: String,
    pub marked: Vec<Segment>,
    /// The candidate window, if open.
    pub panel: Option<Presentation>,
    /// The 中/英 badge.
    pub toast: Option<&'static str>,
    /// The key just pressed, beside the caret.
    pub keys: Vec<String>,
    pub caption: String,
    /// Seconds before the next step.
    pub hold: f32,
}

/// The labels the candidate window shows for the current keys.
#[derive(Clone, Debug, Default)]
pub struct PanelKeys {
    /// The key that types the Zhuyin itself (「`」), when on.
    pub zhuyin: Option<String>,
    pub single: Option<String>,
    pub corrections: Option<String>,
    pub vertical: bool,
    /// The 單字 and 修正 switches in the footer.
    pub switches: bool,
}

/// The scenario for the current settings.
pub struct Script {
    pub steps: Vec<Step>,
    pub keys: PanelKeys,
    /// The window goes above the line.
    pub above: bool,
    /// The window's own appearance: None follows the app.
    pub dark: Option<bool>,
    pub animated: bool,
}

const START: &str = "週末約了";
const HINT: &str = "ㄒㄧㄠˇ ㄇㄧㄥˊ";
const WORDS: [&str; 14] = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"];

/// Where the story rests while it does not play (Reduce Motion): the list open, the highlight moved.
pub const RESTING_STEP: usize = 13;

impl Script {
    pub fn new(settings: &Settings, store: &KeyStore) -> Self {
        let label = |command: &str| Some(store.active_label(command)).filter(|label| !label.is_empty());
        let per_page = settings.number("candidatesPerPage").max(1.) as usize;
        let selection: Vec<char> = settings.string("selectionKeys").chars().take(per_page).collect();
        let keys = PanelKeys {
            zhuyin: label("typeZhuyin"),
            single: label("singleCharacterList"),
            corrections: label("correctionsList"),
            vertical: settings.string("candidateLayout") != "horizontal",
            switches: settings.bool("showsListSwitches"),
        };
        let mode_key = mode_key(settings);
        let pick_key = selection.get(1).map_or("2".to_owned(), |c| c.to_uppercase().to_string());
        let opens_with_space = settings.string("spaceAction") == "candidates";
        let shows_toast = settings.bool("showModeToast");
        let select_left = label("selectLeft");
        let corrections = keys.corrections.clone();
        let list = |selected: usize, tolerant: bool| list(&selection, selected, tolerant, &keys);
        let typing = "打注音：ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ";
        let s = START;
        let w = || seg("王");
        let step = |marked: Vec<Segment>, key: &str, caption: &str, hold: f32| Step {
            committed: s.to_owned(),
            marked,
            panel: None,
            toast: None,
            keys: if key.is_empty() { Vec::new() } else { vec![key.to_owned()] },
            caption: caption.to_owned(),
            hold,
        };
        let mut steps = vec![
            step(Vec::new(), "", "在文件中打字", 1.0),
            step(vec![seg("ㄨ")], "ㄨ", typing, 0.18),
            step(vec![seg("ㄨㄤ")], "ㄤ", typing, 0.18),
            step(vec![w()], "ˊ", typing, 0.3),
            step(vec![w(), seg("ㄒ")], "ㄒ", typing, 0.16),
            step(vec![w(), seg("ㄒㄧ")], "ㄧ", typing, 0.16),
            step(vec![w(), seg("ㄒㄧㄠ")], "ㄠ", typing, 0.18),
            step(vec![w(), seg("小")], "ˇ", typing, 0.3),
            step(vec![w(), seg("小"), seg("ㄇ")], "ㄇ", typing, 0.16),
            step(vec![w(), seg("小"), seg("ㄇㄧ")], "ㄧ", typing, 0.16),
            step(vec![w(), seg("小"), seg("ㄇㄧㄥ")], "ㄥ", typing, 0.18),
            step(vec![w(), seg("曉明")], "ˊ", "整句隨打隨選", 1.3),
            Step {
                panel: Some(list(0, false)),
                ..step(
                    vec![w(), focus("曉明")],
                    "↓",
                    if opens_with_space {
                        "按 ↓ 或空白，開啟候選字視窗"
                    } else {
                        "按 ↓，開啟候選字視窗"
                    },
                    1.4,
                )
            },
            Step {
                panel: Some(list(1, false)),
                ..step(vec![w(), focus("曉明")], if keys.vertical { "↓" } else { "→" }, "移動反白", 1.0)
            },
        ];
        if let Some(corrections) = &corrections {
            steps.push(Step {
                panel: Some(list(0, true)),
                ..step(vec![w(), focus("曉明")], corrections, "列出按錯修正的字", 1.8)
            });
            steps.push(Step {
                panel: Some(list(1, false)),
                ..step(vec![w(), focus("曉明")], corrections, "再按一次，回到一般清單", 1.0)
            });
        }
        steps.push(step(vec![w(), seg("小明")], &pick_key, "選字", 1.2));
        if let Some(select) = &select_left {
            let notice = |text: &str| Some(Presentation { notice: Some(text.to_owned()), ..Presentation::default() });
            steps.push(Step {
                panel: notice("Enter 加入自訂詞：明"),
                ..step(vec![w(), seg("小"), focus("明")], select, "選取字詞", 0.45)
            });
            steps.push(Step {
                panel: notice("Enter 加入自訂詞：小明"),
                ..step(vec![w(), focus("小明")], select, "選取字詞", 0.45)
            });
            steps.push(Step {
                panel: notice("Enter 加入自訂詞：王小明"),
                ..step(vec![focus("王小明")], select, "選取字詞", 1.4)
            });
            steps.push(Step {
                panel: notice("已加入自訂詞：王小明"),
                ..step(vec![seg("王小明")], "Enter", "加入自訂詞，下次直接打得出", 1.6)
            });
        }
        let sent = format!("{s}王小明");
        steps.push(Step { committed: sent.clone(), ..step(Vec::new(), "Enter", "送出", 1.0) });
        if shows_toast && let Some(mode_key) = &mode_key {
            steps.push(Step {
                committed: sent.clone(),
                toast: Some("英"),
                ..step(Vec::new(), mode_key, &format!("按 {mode_key} 切換中英"), 1.6)
            });
        }
        steps.push(Step { committed: sent, ..step(Vec::new(), "", "", 0.8) });
        let dark = match settings.string("panelAppearance").as_str() {
            "light" => Some(false),
            "dark" => Some(true),
            _ => None,
        };
        Script {
            steps,
            keys,
            above: settings.string("candidatePlacement") == "above",
            dark,
            animated: settings.bool("panelAnimation"),
        }
    }

    /// The step at `index` (clamped: the script may have shrunk when a feature was turned off).
    pub fn step(&self, index: usize) -> &Step {
        &self.steps[index.min(self.steps.len() - 1)]
    }
}

/// The list as the candidate window shows it: page 1 of the words (with 校名 first in the corrections list).
fn list(selection_keys: &[char], selected: usize, tolerant: bool, keys: &PanelKeys) -> Presentation {
    let mut items: Vec<(String, Option<String>)> = WORDS.iter().map(|w| ((*w).to_owned(), None)).collect();
    if tolerant {
        items.insert(0, ("校名".to_owned(), Some("ㄒㄧㄠˋ ㄇㄧㄥˊ".to_owned())));
    }
    let page_size = selection_keys.len().max(1);
    let rows = items
        .iter()
        .take(page_size)
        .enumerate()
        .map(|(index, (text, annotation))| CandidateRow {
            index,
            text: text.clone(),
            annotation: annotation.clone(),
            key: selection_keys.get(index).copied(),
        })
        .collect();
    let _ = keys;
    Presentation {
        composition: "王曉明".to_owned(),
        reading: Some(HINT.to_owned()),
        cursor: 3,
        clauses: vec![0..1, 1..3],
        focused_clause: Some(1),
        rows,
        count: items.len(),
        selection: Some(selected),
        selected: Some(selected),
        page: 0,
        page_count: items.len().div_ceil(page_size),
        page_size,
        corrections: tolerant,
        tolerant: Some(tolerant),
        ..Presentation::default()
    }
}

/// The key that shows the 中/英 badge: the Shift tap (「Shift」「左 Shift」「右 Shift」), or Caps Lock; None when
/// neither switches (macOS `CurrentKeys.modeKey`).
pub fn mode_key(settings: &Settings) -> Option<String> {
    if settings.bool("shiftSwitch") {
        return Some(
            match settings.string("shiftToggle").as_str() {
                "left" => "左 Shift",
                "right" => "右 Shift",
                _ => "Shift",
            }
            .to_owned(),
        );
    }
    settings.bool("feature.capsLockEnglish").then(|| "Caps Lock".to_owned())
}

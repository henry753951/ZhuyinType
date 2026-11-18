//! The rows of the 快捷鍵 page (macOS `KeysPane.items`): every key grouped by the page where it is set (SETTINGS_IA.md
//! §2: 打字, 標點與符號, 中英切換, 候選字視窗, 修正, 字詞), in the words of what it does.  Rebindable commands are
//! edited in place; fixed keys are read-only, with where their behaviour is set.

use zt_core::settings::Settings;

use crate::app::Place;
use crate::pages::{Page, links};
use crate::ui::shortcuts::store::{self, KeyStore};

/// The page's sections: the pages with keys, under their names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySection(pub Page);

impl KeySection {
    pub fn all() -> impl Iterator<Item = KeySection> {
        links::KEY_PAGES.into_iter().map(KeySection)
    }

    /// The section's title, also its anchor (the feature pages' 「在快捷鍵總覽中查看全部按鍵」 lead to it).
    pub fn title(self) -> &'static str {
        self.0.title()
    }
}

/// What a row's keys are.
#[derive(Clone, Debug)]
pub enum Keys {
    /// Read-only keys; a second group is an alternative.
    Fixed(Vec<String>, Vec<String>),
    /// A rebindable command.
    Command(&'static str),
    /// The Shift tap that switches 中/英, with its side.
    ShiftTap,
}

/// One row.
#[derive(Clone, Debug)]
pub struct KeyItem {
    pub id: &'static str,
    pub title: String,
    pub subtitle: Option<String>,
    pub keys: Keys,
    /// The `feature.*` switch that turns it off.
    pub feature: Option<&'static str>,
    /// Where the fixed keys or their behaviour are set.
    pub link: Option<Place>,
}

/// Why a feature is not in use although its switch is on: another setting turns it off (macOS `KeyAvailability`).
pub struct Availability {
    pub note: &'static str,
}

impl KeyItem {
    fn new(id: &'static str, title: impl Into<String>, keys: Keys) -> Self {
        Self { id, title: title.into(), subtitle: None, keys, feature: None, link: None }
    }

    fn sub(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    fn feature(mut self, feature: &'static str) -> Self {
        self.feature = Some(feature);
        self
    }

    fn link(mut self, place: Place) -> Self {
        self.link = Some(place);
        self
    }

    /// The page and row where this action is set, for the row's ›: the command's 「快捷鍵」 row on its feature's
    /// page; while the feature is off, its switch (「已關閉 ›」).
    pub fn home(&self, settings: &Settings) -> Option<Place> {
        if let Some(link) = &self.link {
            return Some(link.clone());
        }
        if let Some(feature) = self.feature
            && (!self.is_on(settings) || matches!(self.keys, Keys::Fixed(..)))
        {
            return Some(links::feature_place(feature));
        }
        match self.keys {
            Keys::Command(command) => Some(links::command_place(command)),
            Keys::ShiftTap => Some(links::shift_tap_place()),
            Keys::Fixed(..) => None,
        }
    }

    /// The row's anchor: `keys-<command>` for a command (as on its feature's page), else `row-<id>`.
    pub fn anchor(&self) -> String {
        match self.keys {
            Keys::Command(command) => links::keys_anchor(command),
            _ => format!("row-{}", self.id),
        }
    }

    /// A key that cannot change and has no switch: listed under 固定的按鍵.
    pub fn is_fixed(&self) -> bool {
        matches!(self.keys, Keys::Fixed(..)) && self.feature.is_none()
    }

    /// The row is in use (its feature is on).
    pub fn is_on(&self, settings: &Settings) -> bool {
        match self.keys {
            Keys::ShiftTap => settings.bool("shiftSwitch"),
            _ => self.feature.is_none_or(|feature| settings.bool(feature)),
        }
    }

    /// Another page's setting turns the feature off or changes it.
    pub fn availability(&self, settings: &Settings) -> Option<Availability> {
        match self.feature? {
            "feature.zhuyinRowClick" if !settings.bool("showsZhuyinHint") => {
                Some(Availability { note: "「顯示注音」已關閉，候選字視窗沒有注音列" })
            }
            "feature.correctionsList" if settings.string("typoTolerance") == "off" && !fuzzy_on(settings) => {
                Some(Availability {
                    note: "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單"
                })
            }
            _ => None,
        }
    }

    /// Whether the row matches the search: every word of the query in its title, subtitle or keys.
    pub fn matches(&self, query: &str, keys: &KeyStore, settings: &Settings) -> bool {
        let query = query
            .trim()
            .to_lowercase()
            .replace("全型", "全形")
            .replace("半型", "半形")
            .replace("全形半形", "全半形")
            .replace(['+', '＋'], " ");
        if query.is_empty() {
            return true;
        }
        let mut text = format!("{} {}", self.title, self.subtitle.as_deref().unwrap_or_default())
            .replace("全半形", "全形 半形 全半形");
        match &self.keys {
            Keys::Fixed(a, b) => {
                text.push(' ');
                text.push_str(&a.iter().chain(b).cloned().collect::<Vec<_>>().join(" "));
            }
            Keys::Command(command) => {
                for binding in keys.bindings(command) {
                    let caps = store::keycaps(binding.binding);
                    let words: Vec<String> = caps
                        .iter()
                        .map(|cap| match cap.as_str() {
                            "⇧" => "shift".to_owned(),
                            "空白" => "space".to_owned(),
                            other => other.to_owned(),
                        })
                        .collect();
                    text.push_str(&format!(" {} {} {}", caps.join(" "), words.join(" "), binding.label));
                }
            }
            Keys::ShiftTap => text.push_str(" shift"),
        }
        let _ = settings;
        let text = text.to_lowercase();
        query.split_whitespace().all(|word| text.contains(word))
    }
}

fn fuzzy_on(settings: &Settings) -> bool {
    ["fuzzy.zZh", "fuzzy.cCh", "fuzzy.sSh", "fuzzy.enEng", "fuzzy.anAng"].iter().any(|key| settings.bool(key))
}

/// The keys that pick on a page: 「1–9」, or 「A S D F G」 for the home row.
pub fn selection_keys(settings: &Settings) -> String {
    let count = settings.number("candidatesPerPage").max(1.) as usize;
    let keys: Vec<String> =
        settings.string("selectionKeys").chars().take(count).map(|c| c.to_uppercase().to_string()).collect();
    if settings.string("selectionKeys").starts_with('1') {
        format!("{}–{}", keys.first().cloned().unwrap_or_default(), keys.last().cloned().unwrap_or_default())
    } else {
        keys.join(" ")
    }
}

/// The Shift that switches 中/英 (「Shift」「左 Shift」「右 Shift」), or None when Shift does not switch.
pub fn shift_tap(settings: &Settings) -> Option<&'static str> {
    if !settings.bool("shiftSwitch") {
        return None;
    }
    Some(match settings.string("shiftToggle").as_str() {
        "left" => "左 Shift",
        "right" => "右 Shift",
        _ => "Shift",
    })
}

/// The space key's behaviour as the 空白 row says it.
fn space_subtitle(settings: &Settings) -> String {
    let detail = match settings.string("spaceAction").as_str() {
        "candidates" => "打完注音後，再按空白開啟候選字視窗",
        "insert" => "打完注音後，再按空白在組字中輸入空白，不送出",
        _ => "打完注音後，再按空白送出組字並輸入空白",
    };
    format!("{}；打注音時是一聲", detail.replace("，再按空白", ""))
}

fn fixed(keys: &[&str]) -> Keys {
    Keys::Fixed(keys.iter().map(|k| (*k).to_owned()).collect(), Vec::new())
}

/// The rows of `section` for the current settings.
pub fn items(section: KeySection, settings: &Settings) -> Vec<KeyItem> {
    let vertical = settings.string("candidateLayout") != "horizontal";
    let opens_with_space = settings.string("spaceAction") == "candidates";
    match section.0 {
        Page::Typing => vec![
            KeyItem::new("zhuyin", "拼注音", fixed(&["注音鍵"])).sub("固定：注音鍵的位置依標準注音鍵盤"),
            KeyItem::new("tones", "聲調：二聲、三聲、四聲、輕聲", fixed(&["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"])).sub("固定"),
            KeyItem::new("space", "空白", fixed(&["空白"])).sub(space_subtitle(settings)).link(links::space_place()),
            KeyItem::new("commit", "送出", fixed(&["Enter"])).sub("固定"),
            KeyItem::new("newline", "送出並換行", Keys::Command("commitWithNewline")),
            KeyItem::new("backspace", "刪掉注音符號，或游標前的字", fixed(&["⌫"])).sub("固定"),
            KeyItem::new("deleteWord", "刪掉一個詞", Keys::Command("deleteWord"))
                .sub("有未完成的注音時清除注音")
                .feature("feature.deleteWord"),
            KeyItem::new("forwardDelete", "刪掉游標後的字", fixed(&["Del"])).sub("固定"),
            KeyItem::new(
                "escape",
                if settings.string("escape") == "composition" {
                    "清除注音，再按一次清除組字"
                } else {
                    "清除注音"
                },
                fixed(&["Esc"]),
            )
            .link(links::escape_place()),
            KeyItem::new(
                "cursor",
                "在組字中移動游標",
                Keys::Fixed(vec!["←".into(), "→".into()], vec!["Home".into(), "End".into()]),
            )
            .sub("固定"),
            KeyItem::new("insertSpace", "在組字中放一個空白", Keys::Command("insertSpace")),
        ],
        Page::Punctuation => {
            let windows = settings.string("punctuationKeys") != "mac";
            vec![
                KeyItem::new("punctuationWidth", "切換標點全半形", Keys::Command("togglePunctuationWidth"))
                    .sub("中文預設全形，英數與空白維持半形"),
                KeyItem::new("symbolPicker", "開啟符號與 Emoji 面板", Keys::Command("symbolPicker"))
                    .sub("方向鍵移動，Tab 換類別，打注音或英文搜尋")
                    .feature("feature.symbolPicker"),
                KeyItem::new("brackets", "「 」 、", fixed(&["[", "]", "'"])).sub("固定"),
                KeyItem::new(
                    "shiftSymbols",
                    if windows {
                        "鍵上的符號：＜ ＞ ＿ ？ ！"
                    } else {
                        "中文標點：，。？！：『』——"
                    },
                    fixed(&["⇧符號鍵"]),
                )
                .link(links::punctuation_keys_place()),
                KeyItem::new("controlPunctuation", "中文標點：，。、？！：；『』", fixed(&["Ctrl+符號鍵"]))
                    .sub("兩種標點按法都能用")
                    .feature("feature.controlPunctuation"),
                KeyItem::new("optionSymbols", "鍵上的符號：. / ; < >", fixed(&["Alt+數字或符號鍵"]))
                    .sub("兩種標點按法都能用")
                    .feature("feature.optionSymbols"),
                KeyItem::new("punctuationList", "列出同類的標點", fixed(&["↓"])).sub("在標點上按"),
            ]
        }
        Page::Mode => vec![
            KeyItem::new("shiftTap", "單按 Shift 切換中英", Keys::ShiftTap)
                .sub("在可以打字的地方")
                .link(links::shift_tap_place()),
            KeyItem::new("capsLock", "Caps Lock 打英文", fixed(&["Caps Lock"]))
                .sub("開著時打英文，正在組的字先送出")
                .feature("feature.capsLockEnglish"),
            KeyItem::new("quickSettings", "開啟快速設定", Keys::Command("quickSettings"))
                .sub("英文、半形標點、簡體與暫停學習；預設沒有按鍵"),
        ],
        Page::Candidates => vec![
            KeyItem::new(
                "open",
                "開啟候選字視窗",
                Keys::Fixed(vec!["↓".into()], if opens_with_space { vec!["空白".into()] } else { Vec::new() }),
            )
            .sub("固定"),
            KeyItem::new("pick", "選字", Keys::Fixed(vec![selection_keys(settings)], Vec::new()))
                .sub(format!("目前每頁 {} 個；Enter 選取反白的字", settings.number("candidatesPerPage")))
                .link(links::selection_place()),
            KeyItem::new("move", "移動反白", fixed(if vertical { &["↑", "↓"] } else { &["←", "→"] }))
                .sub(if vertical { "直式清單" } else { "橫式清單；↑ ↓ 翻頁" })
                .link(links::layout_place()),
            KeyItem::new("previousCharacter", "改前一個字", Keys::Command("previousCharacter")).sub(if vertical {
                "清單開著時；直式清單也可以按 ←"
            } else {
                "清單開著時"
            }),
            KeyItem::new("nextCharacter", "改後一個字", Keys::Command("nextCharacter")).sub(if vertical {
                "清單開著時；直式清單也可以按 →"
            } else {
                "清單開著時"
            }),
            KeyItem::new("nextPage", "下一頁", Keys::Command("nextPage"))
                .sub("空白也翻到下一頁")
                .feature("feature.pageKeys"),
            KeyItem::new("previousPage", "上一頁", Keys::Command("previousPage")).feature("feature.pageKeys"),
            KeyItem::new("zhuyinRow", "點注音列打出注音", fixed(&["點一下"]))
                .sub("候選字視窗上方的注音")
                .feature("feature.zhuyinRowClick"),
            KeyItem::new("single", "只列一個字", Keys::Command("singleCharacterList"))
                .sub("再按一次還原")
                .feature("feature.singleCharacterList"),
            KeyItem::new("close", "關閉候選字視窗", fixed(&["Esc"])).sub("固定"),
        ],
        Page::Corrections => vec![
            KeyItem::new("corrections", "列出修正的字", Keys::Command("correctionsList"))
                .sub("按錯修正與模糊音的字排在前面；再按一次還原")
                .feature("feature.correctionsList"),
            KeyItem::new("typeZhuyin", "打出注音本身", Keys::Command("typeZhuyin"))
                .sub("緊接著再按一次或按 Esc 恢復")
                .feature("feature.typeZhuyin"),
            KeyItem::new("deleteAfterCommit", "送出後刪掉重打", fixed(&["⌫"]))
                .sub("送出後 30 秒內刪掉剛送出的字再打，算作改字並接著前文選字")
                .feature("feature.deleteAfterCommit"),
        ],
        Page::Words => vec![
            KeyItem::new("selectLeft", "往左選取", Keys::Command("selectLeft"))
                .sub("再按 Enter 加入自訂詞")
                .feature("feature.selectPhrase"),
            KeyItem::new("selectRight", "往右選取", Keys::Command("selectRight"))
                .sub("再按 Enter 加入自訂詞")
                .feature("feature.selectPhrase"),
            KeyItem::new("addSelectedText", "選取的字加入自訂詞", Keys::Command("addSelectedText"))
                .sub("在 App 裡選取 1 到 6 個字再按；預設沒有按鍵"),
            KeyItem::new("blockCandidate", "不要再推薦", Keys::Command("blockCandidate"))
                .sub("反白的字詞之後不再自動選用；也可以在候選字上按右鍵"),
            KeyItem::new("forgetCandidate", "忘記這個選字", Keys::Command("forgetCandidate")).sub("忘記選過反白的字詞"),
            KeyItem::new("addCandidatePhrase", "加入自訂詞", Keys::Command("addCandidatePhrase"))
                .sub("清單開著時，反白的字詞；要加入打的字，用選取"),
            KeyItem::new(
                "undoWordAction",
                "復原上一個動作",
                Keys::Fixed(vec![zt_core::shortcuts::UNDO_WORD_ACTION.label()], Vec::new()),
            )
            .sub("動作後 5 秒內，組字還在時"),
        ],
        _ => Vec::new(),
    }
}

/// One chip for a group of fixed caps: symbols run together (「←→」), words get a space (「Home End」).
pub fn fixed_chip(caps: &[String]) -> String {
    if caps.iter().all(|cap| cap.chars().count() == 1) { caps.concat() } else { caps.join(" ") }
}

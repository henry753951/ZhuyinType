//! Every place a link in the settings window leads to, in one table (SETTINGS_IA.md §4.2, §4.3), and the old page,
//! sub-page and row ids read as their new places (§4.4).
//!
//! A place is a page, optionally one of its sub-pages, optionally a row (its anchor) or a whole section (its title).
//! Pages link through the functions here rather than spelling places themselves; `targets` lists every one, and the
//! test below draws each page and sub-page headlessly and checks that every target is a row or section actually drawn
//! there, and that every rebindable command has its row on its feature's page and on 快捷鍵.

use super::Page;
use crate::app::Place;

// MARK: - Commands and features

/// The page where `command`'s keys are set beside its feature (SETTINGS_IA.md §2): the page whose name the
/// 快捷鍵 overview files it under.
pub fn command_page(command: &str) -> Page {
    match command {
        "commitWithNewline" | "deleteWord" | "insertSpace" => Page::Typing,
        "togglePunctuationWidth" | "symbolPicker" => Page::Punctuation,
        "quickSettings" => Page::Mode,
        "correctionsList" | "typeZhuyin" => Page::Corrections,
        "selectLeft" | "selectRight" | "addSelectedText" | "blockCandidate" | "forgetCandidate"
        | "addCandidatePhrase" => Page::Words,
        // singleCharacterList, nextPage, previousPage, previousCharacter, nextCharacter
        _ => Page::Candidates,
    }
}

/// The anchor of `command`'s key row, the same on its feature's page and on 快捷鍵.
pub fn keys_anchor(command: &str) -> String {
    format!("keys-{command}")
}

/// `command`'s 「快捷鍵」 row on its feature's page (the 快捷鍵 overview's ›).
pub fn command_place(command: &str) -> Place {
    Place::row(command_page(command), keys_anchor(command))
}

/// `command`'s row on the 快捷鍵 overview (a key field's 「前往」 to another command, the wizard's keys).
pub fn overview_place(command: &str) -> Place {
    Place::row(Page::Keys, keys_anchor(command))
}

/// The overview's section named like `page` (the feature pages' 「在快捷鍵總覽中查看全部按鍵」).
pub fn keys_section_place(page: Page) -> Place {
    Place::row(Page::Keys, page.title())
}

/// The pages with keys, in the order of the 快捷鍵 overview's sections.
pub const KEY_PAGES: [Page; 6] =
    [Page::Typing, Page::Punctuation, Page::Mode, Page::Candidates, Page::Corrections, Page::Words];

/// The switch of the feature `feature` (`feature.…`), where 快捷鍵's 「已關閉 ›」 leads.
pub fn feature_place(feature: &str) -> Place {
    match feature {
        "feature.deleteWord" => Place::row(Page::Typing, "deleteWord"),
        "feature.controlPunctuation" => Place::row(Page::Punctuation, "controlPunctuation"),
        "feature.optionSymbols" => Place::row(Page::Punctuation, "optionSymbols"),
        "feature.symbolPicker" => Place::row(Page::Punctuation, "symbolPicker"),
        "feature.capsLockEnglish" => Place::row(Page::Mode, "capsLock"),
        "feature.typeZhuyin" => Place::row(Page::Corrections, "typeZhuyin"),
        "feature.correctionsList" => Place::row(Page::Corrections, "corrections"),
        "feature.deleteAfterCommit" => Place::row(Page::Corrections, "deleteAfterCommit"),
        "feature.selectPhrase" => Place::row(Page::Words, "selectPhrase"),
        "feature.pageKeys" => Place::row(Page::Candidates, "pageKeys"),
        "feature.zhuyinRowClick" => Place::row(Page::Candidates, "zhuyinRow"),
        "feature.singleCharacterList" => Place::row(Page::Candidates, "single"),
        _ => Place::page(Page::Keys),
    }
}

#[cfg(test)]
/// Every `feature.…` switch with keys (the 快捷鍵 overview's 「已關閉 ›」).
pub const FEATURES: [&str; 12] = [
    "feature.deleteWord",
    "feature.controlPunctuation",
    "feature.optionSymbols",
    "feature.symbolPicker",
    "feature.capsLockEnglish",
    "feature.typeZhuyin",
    "feature.correctionsList",
    "feature.deleteAfterCommit",
    "feature.selectPhrase",
    "feature.pageKeys",
    "feature.zhuyinRowClick",
    "feature.singleCharacterList",
];

// MARK: - Rows other pages link to

/// 候選字視窗 › 顯示注音 (「點注音列打出注音」 without the 注音列).
pub fn shows_zhuyin_place() -> Place {
    Place::row(Page::Candidates, "showsZhuyin")
}

/// 修正 › 按錯鍵修正 (「列出修正的字」 with nothing to list, 介紹's summary and card).
pub fn typo_place() -> Place {
    Place::row(Page::Corrections, "typo")
}

/// 打字 › 空白鍵.
pub fn space_place() -> Place {
    Place::row(Page::Typing, "space")
}

/// 打字 › Esc.
pub fn escape_place() -> Place {
    Place::row(Page::Typing, "escape")
}

/// 標點與符號 › 標點按法.
pub fn punctuation_keys_place() -> Place {
    Place::row(Page::Punctuation, "punctuationKeys")
}

/// 中英切換 › 單按 Shift 切換中英.
pub fn shift_tap_place() -> Place {
    Place::row(Page::Mode, "shiftTap")
}

/// 選字 › 繁／簡.
pub fn output_script_place() -> Place {
    Place::row(Page::Choosing, "outputScript")
}

/// 字詞 › 暫停學習.
pub fn pause_learning_place() -> Place {
    Place::row(Page::Words, PAUSE_LEARNING)
}

/// The anchor of 字詞's 暫停學習 switch.
pub const PAUSE_LEARNING: &str = "pauseLearning";

/// 候選字視窗 › 每頁的候選字 (where 選字's keys are set).
pub fn selection_place() -> Place {
    Place::row(Page::Candidates, "selection")
}

/// 候選字視窗 › 排列 (where the keys that move the highlight are decided).
pub fn layout_place() -> Place {
    Place::row(Page::Candidates, "layout")
}

/// One feature card of 介紹 (§2.2): its title and where it leads.
pub fn intro_cards() -> [(&'static str, Place); 13] {
    [
        ("整句輸入", Place::row(Page::Choosing, "轉換")),
        ("按錯鍵也能打對", typo_place()),
        ("依前文選字", Place::row(Page::Choosing, "context")),
        ("聯想詞", Place::row(Page::Choosing, "nextWordSuggestions")),
        ("記住你的用法", Place::row(Page::Words, "learningEnabled")),
        ("在候選字視窗管理字詞", Place::row(Page::Words, WORD_ACTIONS)),
        ("標點與全半形", command_place("togglePunctuationWidth")),
        ("符號與 Emoji", Place::row(Page::Punctuation, "symbolPicker")),
        ("快速設定", command_place("quickSettings")),
        ("色彩與無障礙", Place::page(Page::Colors)),
        ("檢查輸入法", Place::page(Page::Health)),
        ("改字記錄", Place::row(Page::Advanced, "改字記錄")),
        ("使用分析", Place::page(Page::Analytics)),
    ]
}

/// The title of 字詞's section of the candidate window's word actions.
pub const WORD_ACTIONS: &str = "在候選字視窗中";

/// 中英切換 › 快速設定's three links: the other switches the quick settings flip.
pub fn quick_settings_links() -> [(&'static str, Place); 3] {
    [
        ("切換標點全半形", command_place("togglePunctuationWidth")),
        ("繁／簡", output_script_place()),
        ("暫停學習", pause_learning_place()),
    ]
}

// MARK: - Old ids

/// The place named by ids from the command line (`--page`, `--subpage`, `--row`), the input method's menu
/// (`show-settings`) or an older version, with old ids read as their new places (SETTINGS_IA.md §4.4); None for an
/// unknown page.
pub fn resolve(page: &str, subpage: Option<&str>, row: Option<&str>) -> Option<Place> {
    let (page, implied) = match page {
        // The input method's menu (`show-settings`).
        "feedback" => return Some(super::about::feedback_place()),
        "general" => (Page::Typing, None),
        "context" => (Page::Choosing, Some("context")),
        "phrases" => (Page::Words, Some("phrases")),
        "learned" => (Page::Words, Some("learnedWords")),
        id => (Page::from_current_id(id)?, None),
    };
    let (mut page, subpage) = moved_subpage(page, subpage.or(implied));
    let mut row = row.map(|anchor| renamed_row(page, anchor));
    if subpage.is_none()
        && let Some(anchor) = row.as_deref()
    {
        page = moved_row(page, anchor).unwrap_or(page);
        // 關於's row that opened 檢查輸入法 is the page itself now.
        if page == Page::Health && anchor == "health" {
            row = None;
        }
    }
    let mut place = Place::page(page);
    place.subpage = subpage.map(|id| id.to_owned().into());
    place.row = row.map(Into::into);
    Some(place)
}

/// Sub-pages that became pages or moved to another page.
fn moved_subpage(page: Page, subpage: Option<&str>) -> (Page, Option<&str>) {
    match (page, subpage) {
        (Page::Typing, Some("punctuation")) => (Page::Punctuation, None),
        (Page::Choosing, Some("fuzzy")) => (Page::Corrections, Some("fuzzy")),
        (Page::About, Some("health")) => (Page::Health, None),
        other => other,
    }
}

/// Anchors that were spelled differently: 0.7.0's 候選字視窗 key rows (`keysrow-…`) and 快捷鍵's command rows
/// (`row-<item>`) are `keys-<command>` now.
fn renamed_row(page: Page, anchor: &str) -> String {
    if let Some(command) = anchor.strip_prefix("keysrow-") {
        return keys_anchor(command);
    }
    if page == Page::Keys
        && let Some(item) = anchor.strip_prefix("row-")
    {
        let command = match item {
            "newline" => Some("commitWithNewline"),
            "single" => Some("singleCharacterList"),
            "corrections" => Some("correctionsList"),
            "punctuationWidth" => Some("togglePunctuationWidth"),
            other => crate::ui::shortcuts::store::command(other).map(|command| command.id),
        };
        if let Some(command) = command {
            return keys_anchor(command);
        }
    }
    anchor.to_owned()
}

/// The page a row of an older page is on now.
fn moved_row(page: Page, anchor: &str) -> Option<Page> {
    Some(match (page, anchor) {
        (
            Page::Typing,
            "punctuationKeys"
            | "keys-togglePunctuationWidth"
            | "controlPunctuation"
            | "optionSymbols"
            | "symbolPicker"
            | "keys-symbolPicker",
        ) => Page::Punctuation,
        (Page::Typing, "shiftTap" | "shiftSide" | "capsLock" | "keys-quickSettings" | "startInEnglish")
        | (Page::Candidates, "modeToast") => Page::Mode,
        (
            Page::Choosing,
            "typo"
            | "otherReadings"
            | "corrections"
            | "showCorrections"
            | "undoCorrection"
            | "fuzzy"
            | "typeZhuyin"
            | "deleteAfterCommit"
            | "keys-correctionsList"
            | "keys-typeZhuyin",
        ) => Page::Corrections,
        (
            Page::Choosing,
            "selectPhrase"
            | "keys-selectLeft"
            | "keys-selectRight"
            | "keys-addSelectedText"
            | "keys-blockCandidate"
            | "keys-forgetCandidate"
            | "keys-addCandidatePhrase",
        ) => Page::Words,
        (Page::Candidates, "appearance" | "accentColor" | "theme" | "transparency") => Page::Colors,
        (Page::About, "health") => Page::Health,
        _ => return None,
    })
}

// MARK: - The table

#[cfg(test)]
/// Every place a link leads to, with where the link is (for the test's messages).
pub fn targets() -> Vec<(String, Place)> {
    let mut targets: Vec<(String, Place)> = Vec::new();
    let mut add = |from: &str, place: Place| targets.push((from.to_owned(), place));
    for page in Page::ALL {
        add("側欄", Place::page(page));
    }
    for (page, subpage) in SUBPAGES {
        add("「›」列", Place::subpage(page, subpage));
        add("「›」列所在的列", Place::row(page, subpage));
    }
    add("介紹 › 空白鍵", space_place());
    add("介紹 › 標點按法", punctuation_keys_place());
    add("介紹 › 按錯鍵修正", typo_place());
    add("介紹 › 中英切換", shift_tap_place());
    for (title, place) in intro_cards() {
        add(&format!("介紹 › 功能 › {title}"), place);
    }
    for page in KEY_PAGES {
        add(&format!("{} › 在快捷鍵總覽中查看全部按鍵", page.title()), keys_section_place(page));
    }
    for command in crate::ui::shortcuts::store::commands() {
        add(&format!("快捷鍵 › {} ›", command.label), command_place(command.id));
        add(&format!("按鍵欄位的「前往」 › {}", command.label), overview_place(command.id));
    }
    for feature in FEATURES {
        add(&format!("快捷鍵 › 已關閉 › {feature}"), feature_place(feature));
    }
    for (title, place) in quick_settings_links() {
        add(&format!("中英切換 › 快速設定 › {title}"), place);
    }
    add("點注音列打出注音 › 在「候選字視窗」設定", shows_zhuyin_place());
    add("快捷鍵 › 空白", space_place());
    add("快捷鍵 › Esc", escape_place());
    add("快捷鍵 › 選字", selection_place());
    add("快捷鍵 › 移動反白", layout_place());
    add("快捷鍵 › 單按 Shift", shift_tap_place());
    add("快捷鍵 › 鍵上的符號", punctuation_keys_place());
    add("設定嚮導 › 自行設定…", Place::page(Page::Keys));
    add("輸入法選單 › 回報問題…", super::about::feedback_place());
    // A row under 快捷鍵's 固定的按鍵 (`--row`): the disclosure opens by itself.
    add("快捷鍵 › 固定的按鍵 › 拼注音", Place::row(Page::Keys, "row-zhuyin"));
    for (page, subpage, row) in LEGACY {
        if let Some(place) = resolve(page, subpage, row) {
            add(&format!("舊的 id {page} {subpage:?} {row:?}"), place);
        }
    }
    targets
}

#[cfg(test)]
/// The sub-pages, each opened by a row on its page anchored with its id.
pub const SUBPAGES: [(Page, &str); 10] = [
    (Page::Analytics, "performance"),
    (Page::Typing, "typingKeys"),
    (Page::Choosing, "context"),
    (Page::Corrections, "fuzzy"),
    (Page::Words, "learnedWords"),
    (Page::Words, "phrases"),
    (Page::Advanced, "apps"),
    (Page::About, "sources"),
    (Page::About, "notes"),
    (Page::About, "removal"),
];

/// The 快捷鍵 overview's section of conflicts (shown while there are some).
pub const KEY_CONFLICTS: &str = "按鍵衝突";

/// The anchor of the 快捷鍵 overview's 固定的按鍵 disclosure.
pub const FIXED_KEYS: &str = "fixedKeys";

#[cfg(test)]
/// Old ids that must keep working, as the command line, an older version's links or scripts spell them.
pub const LEGACY: [(&str, Option<&str>, Option<&str>); 22] = [
    ("general", None, None),
    ("context", None, None),
    ("phrases", None, None),
    ("learned", None, None),
    ("typing", Some("punctuation"), None),
    ("choosing", Some("fuzzy"), None),
    ("about", Some("health"), None),
    ("about", None, Some("health")),
    ("typing", None, Some("punctuationKeys")),
    ("typing", None, Some("keys-togglePunctuationWidth")),
    ("typing", None, Some("shiftTap")),
    ("typing", None, Some("capsLock")),
    ("typing", None, Some("keys-quickSettings")),
    ("candidates", None, Some("modeToast")),
    ("choosing", None, Some("typo")),
    ("choosing", None, Some("keys-correctionsList")),
    ("choosing", None, Some("undoCorrection")),
    ("choosing", None, Some("selectPhrase")),
    ("choosing", None, Some("keys-blockCandidate")),
    ("candidates", None, Some("theme")),
    ("candidates", None, Some("keysrow-nextPage")),
    ("keys", None, Some("row-corrections")),
];

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use gpui_kit::{
        AnyView, Bounds, FocusHandle, IntoElement as _, VisualTestContext, WindowAppearance, point, px, size,
    };
    use zt_core::settings::Settings;

    use super::*;
    use crate::app::{self, AppState, Navigation, Target};
    use crate::pages;
    use crate::ui::row::DrawnAnchors;

    /// The settings window's state with default settings in a temporary file and no service, as the screenshots run.
    fn app() -> gpui::TestAppContext {
        let dir =
            std::env::temp_dir().join(format!("zt-links-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).expect("a temporary folder");
        let cx = gpui::TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::theme::init(WindowAppearance::Light, cx);
            cx.set_global(AppState {
                settings: Settings::open(Some(dir.join("settings.json"))),
                service: None,
                service_error: None,
            });
            cx.set_global(Navigation::new(Page::Intro));
            cx.set_global(DrawnAnchors::default());
        });
        cx
    }

    /// Follows a link to `place` and draws its page: the anchors of the rows and sections drawn, with their focus.
    fn follow(
        place: &Place,
        views: &mut HashMap<Page, AnyView>,
        vcx: &mut VisualTestContext,
    ) -> BTreeMap<String, FocusHandle> {
        vcx.update(|_, cx| {
            cx.global_mut::<Navigation>().go(place.clone());
            cx.global_mut::<DrawnAnchors>().0.clear();
        });
        let view = match views.get(&place.page) {
            Some(view) => view.clone(),
            None => {
                let view = vcx.update(|window, cx| pages::make_view(place.page, window, cx));
                views.insert(place.page, view.clone());
                view
            }
        };
        vcx.draw(point(px(0.), px(0.)), size(px(860.), px(12000.)), |_, _| view.into_any_element());
        vcx.update(|_, cx| std::mem::take(&mut cx.global_mut::<DrawnAnchors>().0))
    }

    #[test]
    fn every_link_leads_to_a_drawn_row() {
        let mut cx = app();
        let vcx = cx.add_empty_window();
        let mut views = HashMap::new();
        let mut missing = Vec::new();
        for (from, place) in targets() {
            let drawn = follow(&place, &mut views, vcx);
            if let Some(subpage) = &place.subpage
                && pages::subpage_title(place.page, subpage).is_none()
            {
                missing.push(format!("{from}: {} has no sub-page {subpage}", place.page.title()));
            }
            let Some(row) = &place.row else { continue };
            let at = place.subpage.as_deref().map_or_else(String::new, |subpage| format!(" › {subpage}"));
            match drawn.get(row.as_ref()) {
                None => missing.push(format!("{from}: no 「{row}」 on {}{at}", place.page.title())),
                // The arrival gives the row (or section) the keyboard focus.
                Some(focus) if !vcx.update(|window, _| focus.is_focused(window)) => {
                    missing.push(format!("{from}: 「{row}」 on {}{at} is drawn but not focused", place.page.title()));
                }
                Some(_) => {}
            }
        }
        assert!(missing.is_empty(), "links without their row:\n{}", missing.join("\n"));
    }

    #[test]
    fn every_command_has_its_rows() {
        let mut cx = app();
        let vcx = cx.add_empty_window();
        let mut views = HashMap::new();
        for command in crate::ui::shortcuts::store::commands() {
            for place in [command_place(command.id), overview_place(command.id)] {
                let drawn = follow(&place, &mut views, vcx);
                let row = place.row.clone().unwrap_or_default();
                assert!(drawn.contains_key(row.as_ref()), "{}: no 「{row}」 on {}", command.label, place.page.title());
            }
        }
    }

    #[test]
    fn old_ids_lead_to_the_new_places() {
        let place = |page: Page, subpage: Option<&str>, row: Option<&str>| {
            let mut place = Place::page(page);
            place.subpage = subpage.map(|id| id.to_owned().into());
            place.row = row.map(|id| id.to_owned().into());
            Some(place)
        };
        assert_eq!(resolve("general", None, None), place(Page::Typing, None, None));
        assert_eq!(resolve("context", None, None), place(Page::Choosing, Some("context"), None));
        assert_eq!(resolve("phrases", None, None), place(Page::Words, Some("phrases"), None));
        assert_eq!(resolve("learned", None, None), place(Page::Words, Some("learnedWords"), None));
        assert_eq!(resolve("typing", Some("punctuation"), None), place(Page::Punctuation, None, None));
        assert_eq!(
            resolve("typing", Some("punctuation"), Some("symbolPicker")),
            place(Page::Punctuation, None, Some("symbolPicker"))
        );
        assert_eq!(resolve("choosing", Some("fuzzy"), None), place(Page::Corrections, Some("fuzzy"), None));
        assert_eq!(resolve("about", Some("health"), None), place(Page::Health, None, None));
        assert_eq!(resolve("about", None, Some("health")), place(Page::Health, None, None));
        assert_eq!(
            resolve("typing", None, Some("keys-togglePunctuationWidth")),
            place(Page::Punctuation, None, Some("keys-togglePunctuationWidth"))
        );
        assert_eq!(resolve("typing", None, Some("shiftSide")), place(Page::Mode, None, Some("shiftSide")));
        assert_eq!(resolve("candidates", None, Some("modeToast")), place(Page::Mode, None, Some("modeToast")));
        assert_eq!(resolve("choosing", None, Some("typo")), place(Page::Corrections, None, Some("typo")));
        assert_eq!(
            resolve("choosing", None, Some("keys-addSelectedText")),
            place(Page::Words, None, Some("keys-addSelectedText"))
        );
        assert_eq!(resolve("candidates", None, Some("accentColor")), place(Page::Colors, None, Some("accentColor")));
        assert_eq!(
            resolve("candidates", None, Some("keysrow-nextPage")),
            place(Page::Candidates, None, Some("keys-nextPage"))
        );
        assert_eq!(
            resolve("keys", None, Some("row-corrections")),
            place(Page::Keys, None, Some("keys-correctionsList"))
        );
        assert_eq!(resolve("keys", None, Some("row-zhuyin")), place(Page::Keys, None, Some("row-zhuyin")));
        // What stays where it was.
        assert_eq!(resolve("choosing", None, Some("outputScript")), place(Page::Choosing, None, Some("outputScript")));
        assert_eq!(resolve("typing", None, Some("space")), place(Page::Typing, None, Some("space")));
        // The input method's menu (`show-settings`).
        assert_eq!(resolve("words", None, None), place(Page::Words, None, None));
        assert_eq!(resolve("about", None, None), place(Page::About, None, None));
        assert_eq!(resolve("feedback", None, None), place(Page::About, None, Some("意見與問題")));
        assert_eq!(resolve("nowhere", None, None), None);
        for page in Page::ALL {
            assert_eq!(resolve(page.id(), None, None), place(page, None, None));
        }
    }

    #[test]
    fn an_arrival_shows_its_section_heading() {
        let viewport = Bounds::new(point(px(0.), px(100.)), size(px(600.), px(600.)));
        let row = |top: f32| Bounds::new(point(px(0.), px(top)), size(px(600.), px(44.)));
        // A row near its heading: about a third down the page.
        let section = Bounds::new(point(px(0.), px(1000.)), size(px(600.), px(300.)));
        let y = app::arrival_offset(Target::Row, row(1050.), Some(section), viewport, px(0.), px(5000.));
        assert_eq!(y, -(px(1050.) - px(300.)));
        // Far below its heading: lower, so the heading stays in view.
        let tall = Bounds::new(point(px(0.), px(1000.)), size(px(600.), px(900.)));
        let y = app::arrival_offset(Target::Row, row(1400.), Some(tall), viewport, px(0.), px(5000.));
        assert_eq!(px(1000.) + y, viewport.top() + px(24.));
        // A section: its heading near the top; never past either end.
        let y = app::arrival_offset(Target::Section, section, None, viewport, px(0.), px(5000.));
        assert_eq!(px(1000.) + y, viewport.top() + px(24.));
        assert_eq!(app::arrival_offset(Target::Row, row(150.), None, viewport, px(0.), px(5000.)), px(0.));
        assert_eq!(app::arrival_offset(Target::Row, row(9000.), None, viewport, px(0.), px(500.)), px(-500.));
    }
}

//! 符號與 Emoji 面板 (the symbol picker): its state and keys, without UI.  The categories and the search come from
//! `zhuyin-symbols` (through [`Source`], so tests can use a small catalog); the recent list is the platform's, a
//! local file in the user's data directory ([`RECENT_FILE`]).
//!
//! While the picker is open the input controller routes every key here ([`Picker::key`]); the client app keeps
//! focus.  The views draw [`Picker::view`] (the GPUI grid) or [`Picker::presentation`] (the highlighted row as a
//! candidate list, for the candidate windows the native adapters draw: the Windows text service, Fcitx on Wayland).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::keys::{Key, code, modifier};
use crate::presentation::{CandidateRow, Presentation};

/// Items in one row of the grid (digits 1–9 pick from the highlighted row).
pub const COLUMNS: usize = 9;
/// Rows the grid shows at once (PageUp / PageDown move this many).
pub const VISIBLE_ROWS: usize = 6;
/// Search results asked of the core.
pub const SEARCH_LIMIT: usize = 180;
/// The recent list's file in the user's data directory.
pub const RECENT_FILE: &str = "recent-symbols.txt";
/// The recent category's id.
pub const RECENT_ID: &str = "recent";

/// One symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub text: String,
    /// Its name (CLDR zh_Hant), for tooltips and screen readers.
    pub name: String,
}

/// One category of the core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub title: String,
    pub items: Vec<Item>,
}

/// Where the symbols come from.
pub trait Source: Send + Sync {
    /// Every category of the core, in order (not 最近使用).
    fn categories(&self) -> Arc<Vec<Category>>;
    /// The best matches for raw keys (Zhuyin on the standard layout or English); empty query: none.
    fn search(&self, query: &str, limit: usize) -> Vec<Item>;
    /// The query as the search field shows it (Zhuyin when the keys decode).
    fn display_query(&self, query: &str) -> String;
    /// A symbol's name, for recent items.
    fn name_of(&self, text: &str) -> Option<String>;
    /// The recent list stored as `text`, newest first.
    fn recent_from_text(&self, text: &str) -> Vec<String>;
    /// `recent` with `symbol` pushed to the front, as stored.
    fn recent_pushed(&self, recent: &[String], symbol: &str) -> (Vec<String>, String);
}

/// A small built-in catalog, until the core's symbols (`zhuyin-symbols`) are linked in.
pub struct CoreSource;

fn builtin() -> Vec<Category> {
    let category = |id: &str, title: &str, texts: &str| Category {
        id: id.to_owned(),
        title: title.to_owned(),
        items: texts.split(' ').map(|text| Item { text: text.to_owned(), name: String::new() }).collect(),
    };
    vec![
        category("punctuation", "常用標點", "， 。 、 ； ： ？ ！ … — 「 」 『 』 （ ） 《 》 〈 〉 ・ ～"),
        category("math", "數學", "＋ － × ÷ ＝ ≠ ≈ ≤ ≥ ± √ ∞ ∑ ∏ ∫ ∂ ∈ ∉ ⊂ ⊃ ∩ ∪ ∀ ∃ ∅ ∠ ⊥ ∥ ％ ‰"),
        category("arrows", "箭頭", "← → ↑ ↓ ↔ ↕ ↖ ↗ ↘ ↙ ⇐ ⇒ ⇑ ⇓ ⇔ ➜ ➤ ⤴ ⤵"),
        category(
            "smileys",
            "笑臉與情緒",
            "😀 😃 😄 😁 😆 😅 🤣 😂 🙂 🙃 😉 😊 😇 🥰 😍 🤩 😘 😗 😚 😙 😋 😛 😜 🤪 😝 🤑 🤗 🤭 🤫 🤔",
        ),
    ]
}

impl Source for CoreSource {
    fn categories(&self) -> Arc<Vec<Category>> {
        static CATEGORIES: OnceLock<Arc<Vec<Category>>> = OnceLock::new();
        CATEGORIES.get_or_init(|| Arc::new(builtin())).clone()
    }

    fn search(&self, query: &str, limit: usize) -> Vec<Item> {
        if query.is_empty() {
            return Vec::new();
        }
        self.categories()
            .iter()
            .flat_map(|category| category.items.iter())
            .filter(|item| item.text.contains(query))
            .take(limit)
            .cloned()
            .collect()
    }

    fn display_query(&self, query: &str) -> String {
        query.to_owned()
    }

    fn name_of(&self, _text: &str) -> Option<String> {
        None
    }

    fn recent_from_text(&self, text: &str) -> Vec<String> {
        text.lines().filter(|line| !line.is_empty()).take(32).map(str::to_owned).collect()
    }

    fn recent_pushed(&self, recent: &[String], symbol: &str) -> (Vec<String>, String) {
        let mut list = vec![symbol.to_owned()];
        list.extend(recent.iter().filter(|s| *s != symbol).take(31).cloned());
        let text = list.join("\n") + "\n";
        (list, text)
    }
}

/// The recent list and where it is kept (`None`: in memory only, for renders and tests).
pub struct Recent {
    file: Option<PathBuf>,
    items: Vec<String>,
}

impl Recent {
    /// The list stored in `dir`/[`RECENT_FILE`] (missing or unreadable: empty).
    #[must_use]
    pub fn open(source: &dyn Source, dir: Option<&Path>) -> Self {
        let file = dir.map(|dir| dir.join(RECENT_FILE));
        let items = file
            .as_deref()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .map(|text| source.recent_from_text(&text))
            .unwrap_or_default();
        Self { file, items }
    }

    /// Newest first.
    #[must_use]
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// `symbol` was inserted: it goes first, and the file is written.
    pub fn push(&mut self, source: &dyn Source, symbol: &str) {
        let (items, text) = source.recent_pushed(&self.items, symbol);
        self.items = items;
        if let Some(file) = &self.file {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(error) = crate::settings::write_atomically(file, text.as_bytes()) {
                log::warn!("recent symbols: {error:#}");
            }
        }
    }

    /// Forgets the list and removes the file (回復預設設定, deleting personal data).
    pub fn clear(&mut self) {
        self.items.clear();
        if let Some(file) = &self.file {
            match std::fs::remove_file(file) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => log::warn!("recent symbols: {error}"),
            }
        }
    }
}

/// Removes the recent list in `dir` (when no service holds it).
pub fn remove_recent_file(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(RECENT_FILE));
}

/// What a key did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The picker changed (or ignored the key); it stays open.
    Handled,
    /// Insert this symbol; the picker closes.
    Insert(String),
    /// The picker closes without inserting.
    Close,
}

/// One tab of the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
    pub title: String,
}

/// What the grid view draws: a window of rows around the highlight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerView {
    pub tabs: Vec<Tab>,
    /// The active tab; `None` while searching.
    pub tab: Option<usize>,
    /// The search field (as `display_query` shows it); empty: no search.
    pub query: String,
    /// The query came from the composition (the first key replaces it).
    pub prefilled: bool,
    /// The first row shown.
    pub first_row: usize,
    /// How many rows the whole list has.
    pub row_count: usize,
    /// The items of the rows shown, from `first_row`; item `i` is index `first_row * COLUMNS + i` of the list.
    pub items: Vec<Item>,
    /// The highlighted index in the whole list.
    pub highlight: Option<usize>,
}

/// The picker's state.
pub struct Picker {
    source: Arc<dyn Source>,
    base: Arc<Vec<Category>>,
    recent: Vec<Item>,
    tab: usize,
    query: String,
    prefilled: bool,
    results: Vec<Item>,
    highlight: usize,
    first_row: usize,
}

impl Picker {
    /// The picker on its first category (最近使用 when there are recent symbols), with `prefill` (the composition's
    /// converted text) as the search when not empty.
    #[must_use]
    pub fn open(source: Arc<dyn Source>, recent: &[String], prefill: &str) -> Self {
        let base = source.categories();
        let recent = recent
            .iter()
            .map(|text| Item { text: text.clone(), name: source.name_of(text).unwrap_or_default() })
            .collect();
        let mut picker = Self {
            source,
            base,
            recent,
            tab: 0,
            query: String::new(),
            prefilled: false,
            results: Vec::new(),
            highlight: 0,
            first_row: 0,
        };
        if !prefill.is_empty() {
            prefill.clone_into(&mut picker.query);
            picker.prefilled = true;
            picker.search();
        }
        picker
    }

    /// The tabs: 最近使用 (when not empty), then the core's categories.
    #[must_use]
    pub fn tabs(&self) -> Vec<Tab> {
        let recent =
            (!self.recent.is_empty()).then(|| Tab { id: RECENT_ID.to_owned(), title: "最近使用".to_owned() });
        recent
            .into_iter()
            .chain(self.base.iter().map(|category| Tab { id: category.id.clone(), title: category.title.clone() }))
            .collect()
    }

    fn tab_count(&self) -> usize {
        self.base.len() + usize::from(!self.recent.is_empty())
    }

    /// The items shown now: the search results, or the active category's.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        if !self.query.is_empty() {
            return &self.results;
        }
        if !self.recent.is_empty() {
            if self.tab == 0 {
                return &self.recent;
            }
            return self.base.get(self.tab - 1).map_or(&[], |category| &category.items);
        }
        self.base.get(self.tab).map_or(&[], |category| &category.items)
    }

    /// The active tab (`None` while searching).
    #[must_use]
    pub fn tab(&self) -> Option<usize> {
        self.query.is_empty().then_some(self.tab)
    }

    /// The raw query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The highlighted index, if the list has items.
    #[must_use]
    pub fn highlight(&self) -> Option<usize> {
        (self.highlight < self.items().len()).then_some(self.highlight)
    }

    fn row_count(&self) -> usize {
        self.items().len().div_ceil(COLUMNS)
    }

    fn search(&mut self) {
        self.results = if self.query.is_empty() { Vec::new() } else { self.source.search(&self.query, SEARCH_LIMIT) };
        self.highlight = 0;
        self.first_row = 0;
    }

    fn set_query(&mut self, query: String) {
        self.query = query;
        self.prefilled = false;
        self.search();
    }

    fn move_to(&mut self, index: usize) {
        let count = self.items().len();
        if count == 0 {
            return;
        }
        self.highlight = index.min(count - 1);
        let row = self.highlight / COLUMNS;
        if row < self.first_row {
            self.first_row = row;
        } else if row >= self.first_row + VISIBLE_ROWS {
            self.first_row = row + 1 - VISIBLE_ROWS;
        }
    }

    /// Moves the highlight by `delta` items (clamped to the list).
    pub fn move_by(&mut self, delta: isize) {
        let target = self.highlight.saturating_add_signed(delta);
        self.move_to(if delta < 0 && self.highlight < delta.unsigned_abs() { 0 } else { target });
    }

    /// Switches to tab `index` (a click), leaving any search.
    pub fn select_tab(&mut self, index: usize) {
        if index >= self.tab_count() {
            return;
        }
        self.query.clear();
        self.prefilled = false;
        self.results.clear();
        self.tab = index;
        self.highlight = 0;
        self.first_row = 0;
    }

    fn step_tab(&mut self, delta: isize) {
        let count = self.tab_count() as isize;
        if count == 0 {
            return;
        }
        let next = (self.tab as isize + delta).rem_euclid(count) as usize;
        self.select_tab(next);
    }

    /// Item `index` of the list (a click, or a digit), if it exists.
    #[must_use]
    pub fn item(&self, index: usize) -> Option<&Item> {
        self.items().get(index)
    }

    /// One key (contract: arrows, PageUp/PageDown, Tab, digits, Return/Space, Esc, ⌫, query keys).
    pub fn key(&mut self, key: &Key) -> Outcome {
        let (code, modifiers) = crate::keys::canonical(key.code, key.modifiers);
        let chord = modifiers & (modifier::CONTROL | modifier::ALT | modifier::META);
        let shift = modifiers & modifier::SHIFT != 0;
        match code {
            code::LEFT => self.move_by(-1),
            code::RIGHT => self.move_by(1),
            code::UP => self.move_by(-(COLUMNS as isize)),
            code::DOWN => self.move_by(COLUMNS as isize),
            code::PAGE_UP => self.move_by(-((COLUMNS * VISIBLE_ROWS) as isize)),
            code::PAGE_DOWN => self.move_by((COLUMNS * VISIBLE_ROWS) as isize),
            code::HOME => self.move_to(0),
            code::END => self.move_to(usize::MAX),
            code::TAB => {
                if self.query.is_empty() {
                    self.step_tab(if shift { -1 } else { 1 });
                } else {
                    let tab = self.tab;
                    self.select_tab(tab);
                }
            }
            code::RETURN => return self.insert_highlighted(),
            code::ESCAPE => {
                if self.query.is_empty() {
                    return Outcome::Close;
                }
                self.set_query(String::new());
            }
            code::BACKSPACE => {
                if self.prefilled {
                    self.set_query(String::new());
                } else if !self.query.is_empty() {
                    let mut query = std::mem::take(&mut self.query);
                    query.pop();
                    self.set_query(query);
                }
            }
            code::SPACE if chord == 0 && self.query.is_empty() => return self.insert_highlighted(),
            _ if chord == 0 && (i32::from(b'1')..=i32::from(b'9')).contains(&code) && !shift => {
                let column = (code - i32::from(b'1')) as usize;
                let start = self.highlight / COLUMNS * COLUMNS;
                return match self.item(start + column) {
                    Some(item) => Outcome::Insert(item.text.clone()),
                    None => Outcome::Handled,
                };
            }
            _ if chord == 0 => {
                if let Some(character) = query_character(key, code, shift) {
                    let mut query = if self.prefilled { String::new() } else { self.query.clone() };
                    query.push(character);
                    self.set_query(query);
                }
            }
            _ => {}
        }
        Outcome::Handled
    }

    fn insert_highlighted(&self) -> Outcome {
        match self.highlight().and_then(|index| self.item(index)) {
            Some(item) => Outcome::Insert(item.text.clone()),
            None => Outcome::Handled,
        }
    }

    /// The search field's text: the prefilled text as is, typed keys as `display_query` reads them.
    #[must_use]
    pub fn display_query(&self) -> String {
        if self.prefilled || self.query.is_empty() {
            return self.query.clone();
        }
        self.source.display_query(&self.query)
    }

    /// What the grid view draws.
    #[must_use]
    pub fn view(&self) -> PickerView {
        let items = self.items();
        let start = (self.first_row * COLUMNS).min(items.len());
        let end = ((self.first_row + VISIBLE_ROWS) * COLUMNS).min(items.len());
        PickerView {
            tabs: self.tabs(),
            tab: self.tab(),
            query: self.display_query(),
            prefilled: self.prefilled,
            first_row: self.first_row,
            row_count: self.row_count(),
            items: items[start..end].to_vec(),
            highlight: self.highlight(),
        }
    }

    /// The highlighted row as a candidate list (keys 1–9), for candidate windows the native adapters draw: the
    /// reading row shows the category (or the search), the notice how to move on.
    #[must_use]
    pub fn presentation(&self) -> Presentation {
        let items = self.items();
        let row = self.highlight / COLUMNS;
        let start = (row * COLUMNS).min(items.len());
        let end = (start + COLUMNS).min(items.len());
        let header = if self.query.is_empty() {
            self.tabs().get(self.tab).map(|tab| tab.title.clone()).unwrap_or_default()
        } else {
            format!("搜尋：{}", self.display_query())
        };
        let notice = if items.is_empty() {
            "找不到符合的符號".to_owned()
        } else {
            format!("{header}　{}/{}　↑↓ 換列　Tab 換類別", row + 1, self.row_count())
        };
        Presentation {
            rows: items[start..end]
                .iter()
                .enumerate()
                .map(|(column, item)| CandidateRow {
                    index: start + column,
                    text: item.text.clone(),
                    annotation: None,
                    key: char::from_digit(column as u32 + 1, 10),
                })
                .collect(),
            count: items.len(),
            selection: self.highlight(),
            selected: self.highlight().map(|index| index - start),
            page: row,
            page_count: self.row_count(),
            page_size: COLUMNS,
            notice: Some(notice),
            ..Presentation::default()
        }
    }
}

/// The raw ASCII key a query takes: the platform's text when it is one printable ASCII character, else the US
/// layout's character for the code.
fn query_character(key: &Key, code: i32, shift: bool) -> Option<char> {
    if code == code::SPACE {
        return Some(' ');
    }
    let mut chars = key.text.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_graphic()
    {
        return Some(c);
    }
    let c = char::from_u32(u32::try_from(code).ok()?)?;
    if !c.is_ascii_graphic() {
        return None;
    }
    Some(if c.is_ascii_alphabetic() && !shift { c.to_ascii_lowercase() } else { c })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    fn items(texts: &[&str]) -> Vec<Item> {
        texts.iter().map(|text| Item { text: (*text).to_owned(), name: format!("名{text}") }).collect()
    }

    impl Source for Fake {
        fn categories(&self) -> Arc<Vec<Category>> {
            let many: Vec<String> = (0..30).map(|n| format!("s{n}")).collect();
            let many: Vec<&str> = many.iter().map(String::as_str).collect();
            Arc::new(vec![
                Category { id: "punctuation".into(), title: "常用標點".into(), items: items(&many) },
                Category { id: "arrows".into(), title: "箭頭".into(), items: items(&["←", "→"]) },
            ])
        }
        fn search(&self, query: &str, _limit: usize) -> Vec<Item> {
            match query {
                "笑" | "vul" => items(&["😀", "😂"]),
                "v" => items(&["ㄒ"]),
                _ => Vec::new(),
            }
        }
        fn display_query(&self, query: &str) -> String {
            if query == "vul" { "ㄒㄧㄠ".to_owned() } else { query.to_owned() }
        }
        fn name_of(&self, _text: &str) -> Option<String> {
            None
        }
        fn recent_from_text(&self, text: &str) -> Vec<String> {
            text.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect()
        }
        fn recent_pushed(&self, recent: &[String], symbol: &str) -> (Vec<String>, String) {
            let mut list = vec![symbol.to_owned()];
            list.extend(recent.iter().filter(|s| *s != symbol).cloned());
            let text = list.join("\n");
            (list, text)
        }
    }

    fn picker(recent: &[&str], prefill: &str) -> Picker {
        let recent: Vec<String> = recent.iter().map(|s| (*s).to_owned()).collect();
        Picker::open(Arc::new(Fake), &recent, prefill)
    }

    fn press(picker: &mut Picker, code: i32, modifiers: u32) -> Outcome {
        picker.key(&Key::new(code, modifiers))
    }

    fn typed(picker: &mut Picker, c: char) -> Outcome {
        let mut key = Key::typing(c).expect("ascii");
        key.text = c.to_string();
        picker.key(&key)
    }

    #[test]
    fn recent_comes_first_only_when_not_empty() {
        assert_eq!(picker(&[], "").tabs()[0].id, "punctuation");
        let p = picker(&["★"], "");
        assert_eq!(p.tabs()[0].id, RECENT_ID);
        assert_eq!(p.items()[0].text, "★");
    }

    #[test]
    fn arrows_wrap_rows_and_clamp_at_the_ends() {
        let mut p = picker(&[], "");
        press(&mut p, code::LEFT, 0);
        assert_eq!(p.highlight(), Some(0));
        for _ in 0..9 {
            press(&mut p, code::RIGHT, 0);
        }
        assert_eq!(p.highlight(), Some(9), "→ at a row's end goes to the next row");
        press(&mut p, code::DOWN, 0);
        assert_eq!(p.highlight(), Some(18));
        press(&mut p, code::DOWN, 0);
        press(&mut p, code::DOWN, 0);
        assert_eq!(p.highlight(), Some(29), "↓ on the last row stops at the last item");
        press(&mut p, code::PAGE_UP, 0);
        assert_eq!(p.highlight(), Some(0));
        press(&mut p, code::PAGE_DOWN, 0);
        assert_eq!(p.highlight(), Some(29));
    }

    #[test]
    fn digits_insert_from_the_highlighted_row() {
        let mut p = picker(&[], "");
        press(&mut p, code::DOWN, 0);
        assert_eq!(press(&mut p, i32::from(b'2'), 0), Outcome::Insert("s10".into()));
        press(&mut p, code::DOWN, 0);
        press(&mut p, code::DOWN, 0);
        assert_eq!(press(&mut p, i32::from(b'9'), 0), Outcome::Handled, "the last row has 3 items");
        assert_eq!(press(&mut p, i32::from(b'3'), 0), Outcome::Insert("s29".into()));
    }

    #[test]
    fn tab_switches_categories_and_wraps() {
        let mut p = picker(&["★"], "");
        press(&mut p, code::TAB, 0);
        assert_eq!(p.tab(), Some(1));
        press(&mut p, code::TAB, modifier::SHIFT);
        press(&mut p, code::TAB, modifier::SHIFT);
        assert_eq!(p.tab(), Some(2), "Shift+Tab before the first goes to the last");
        press(&mut p, code::BACKTAB, modifier::SHIFT);
        assert_eq!(p.tab(), Some(1));
    }

    #[test]
    fn letters_search_and_tab_or_esc_leave_it() {
        let mut p = picker(&[], "");
        for c in "vul".chars() {
            typed(&mut p, c);
        }
        assert_eq!(p.query(), "vul");
        assert_eq!(p.display_query(), "ㄒㄧㄠ");
        assert_eq!(p.tab(), None);
        assert_eq!(p.items().len(), 2);
        press(&mut p, code::BACKSPACE, 0);
        assert_eq!(p.query(), "vu");
        assert!(p.items().is_empty());
        assert_eq!(press(&mut p, code::ESCAPE, 0), Outcome::Handled);
        assert_eq!(p.query(), "");
        assert_eq!(p.tab(), Some(0));
        typed(&mut p, 'v');
        press(&mut p, code::TAB, 0);
        assert_eq!(p.query(), "", "Tab leaves the search");
        assert_eq!(press(&mut p, code::ESCAPE, 0), Outcome::Close);
    }

    #[test]
    fn space_inserts_only_without_a_query() {
        let mut p = picker(&[], "");
        assert_eq!(press(&mut p, code::SPACE, 0), Outcome::Insert("s0".into()));
        typed(&mut p, 'v');
        assert_eq!(press(&mut p, code::SPACE, 0), Outcome::Handled);
        assert_eq!(p.query(), "v ", "space is a tone key in the search");
        assert_eq!(press(&mut p, code::RETURN, 0), Outcome::Handled, "no results: nothing to insert");
    }

    #[test]
    fn digits_are_never_query_keys() {
        let mut p = picker(&[], "");
        typed(&mut p, 'v');
        assert_eq!(press(&mut p, i32::from(b'1'), 0), Outcome::Insert("ㄒ".into()));
        assert_eq!(p.query(), "v");
    }

    #[test]
    fn a_prefilled_query_is_replaced_or_cleared() {
        let mut p = picker(&[], "笑");
        assert_eq!(p.display_query(), "笑");
        assert_eq!(p.items().len(), 2);
        typed(&mut p, 'v');
        assert_eq!(p.query(), "v", "the first key replaces the prefill");
        let mut p = picker(&[], "笑");
        press(&mut p, code::BACKSPACE, 0);
        assert_eq!(p.query(), "", "⌫ clears the prefill");
        assert_eq!(press(&mut p, code::RETURN, 0), Outcome::Insert("s0".into()));
    }

    #[test]
    fn chords_are_ignored() {
        let mut p = picker(&[], "");
        assert_eq!(press(&mut p, 'C' as i32, modifier::CONTROL), Outcome::Handled);
        assert_eq!(p.query(), "");
    }

    #[test]
    fn the_native_list_is_the_highlighted_row() {
        let mut p = picker(&[], "");
        press(&mut p, code::DOWN, 0);
        press(&mut p, code::RIGHT, 0);
        let list = p.presentation();
        assert_eq!(list.rows.len(), 9);
        assert_eq!(list.rows[0].text, "s9");
        assert_eq!(list.rows[0].index, 9);
        assert_eq!(list.rows[0].key, Some('1'));
        assert_eq!(list.selected, Some(1));
        assert_eq!(list.page, 1);
        assert_eq!(list.page_count, 4);
        assert!(list.has_list());
    }

    #[test]
    fn the_view_scrolls_with_the_highlight() {
        let mut p = picker(&[], "");
        let view = p.view();
        assert_eq!(view.row_count, 4);
        assert_eq!(view.items.len(), 30);
        press(&mut p, code::END, 0);
        assert_eq!(p.view().highlight, Some(29));
    }

    #[test]
    fn recent_file_round_trip_and_clear() {
        let dir = std::env::temp_dir().join(format!("zt-recent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut recent = Recent::open(&Fake, Some(&dir));
        assert!(recent.items().is_empty());
        recent.push(&Fake, "★");
        recent.push(&Fake, "→");
        recent.push(&Fake, "★");
        assert_eq!(recent.items(), ["★", "→"]);
        let again = Recent::open(&Fake, Some(&dir));
        assert_eq!(again.items(), ["★", "→"]);
        recent.clear();
        assert!(!dir.join(RECENT_FILE).exists());
        assert!(Recent::open(&Fake, Some(&dir)).items().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

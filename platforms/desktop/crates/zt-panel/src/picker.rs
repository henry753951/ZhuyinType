//! 符號與 Emoji: what the picker's grid shows ([`PickerView`], made by `zt_core::symbols::Picker::view`) and where
//! its parts go ([`PickerLayout`]).  The GPUI app draws the view in its candidate window (Linux); the broker sends
//! it in the panel (`Panel::picker`) to the Windows text service, which draws the same grid with Direct2D.

use serde::{Deserialize, Serialize};

use crate::layout::Frame;

/// Items in one row of the grid (digits 1–9 pick from the highlighted row).
pub const COLUMNS: usize = 9;
/// Rows the grid shows at once (PageUp / PageDown move this many).
pub const VISIBLE_ROWS: usize = 6;

/// One symbol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub text: String,
    /// Its name (CLDR zh_Hant), for tooltips and screen readers.
    pub name: String,
}

/// One tab of the picker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub title: String,
}

/// What the grid view draws: a window of rows around the highlight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl PickerView {
    /// The highlighted symbol's name (the footer).
    #[must_use]
    pub fn highlighted_name(&self) -> &str {
        self.highlight
            .and_then(|index| index.checked_sub(self.first_row * COLUMNS))
            .and_then(|offset| self.items.get(offset))
            .map_or("", |item| item.name.as_str())
    }

    /// 「3/12」: the highlighted row of all rows (the footer's right end).
    #[must_use]
    pub fn position(&self) -> String {
        self.highlight.map(|index| format!("{}/{}", index / COLUMNS + 1, self.row_count.max(1))).unwrap_or_default()
    }
}

/// One cell of the grid.
pub const CELL: f32 = 36.;
pub const PAD: f32 = 8.;
pub const SEARCH_H: f32 = 30.;
pub const TABS_H: f32 = 28.;
pub const FOOTER_H: f32 = 26.;
pub const RADIUS: f32 = 10.;
pub const SYMBOL_FONT: f32 = 20.;
pub const TAB_FONT: f32 = 12.;
/// Where the grid's text starts, for lining the window up with the caret.
pub const TEXT_INSET: f32 = PAD + 10.;
/// The search line's text when nothing is searched.
pub const SEARCH_PLACEHOLDER: &str = "打注音或英文搜尋";
/// The search line's hint while searching.
pub const SEARCH_HINT: &str = "Esc 清除・Tab 回類別";
/// The grid when nothing matches.
pub const NO_MATCH: &str = "找不到符合的符號";

/// The surface's size (fixed, so the window does not jump between categories).
#[must_use]
pub fn size() -> (f32, f32) {
    (PAD * 2. + CELL * COLUMNS as f32, PAD * 2. + SEARCH_H + TABS_H + CELL * VISIBLE_ROWS as f32 + FOOTER_H)
}

/// A tab title's width: measured roughly, CJK at the font size, plus padding (the GPUI grid measures the same way).
fn tab_width(title: &str) -> f32 {
    title.chars().count() as f32 * TAB_FONT + 14.
}

/// The tabs shown: a run that fits the width and holds the active one.
#[must_use]
pub fn visible_tabs(view: &PickerView, width: f32) -> std::ops::Range<usize> {
    let tab_w = |index: usize| tab_width(&view.tabs[index].title);
    let active = view.tab.unwrap_or(0).min(view.tabs.len().saturating_sub(1));
    let (mut start, mut end, mut used) = (active, active, 0.);
    while end < view.tabs.len() && used + tab_w(end) <= width {
        used += tab_w(end);
        end += 1;
    }
    while start > 0 && used + tab_w(start - 1) <= width {
        start -= 1;
        used += tab_w(start);
    }
    start..end.max(start)
}

/// One cell of the grid shown.
#[derive(Clone, Debug, PartialEq)]
pub struct CellLayout {
    /// The item's index in the whole list.
    pub index: usize,
    /// Its offset in `PickerView::items`.
    pub offset: usize,
    pub frame: Frame,
    /// In the highlighted row: shows its key (`column + 1`).
    pub numbered: bool,
    pub selected: bool,
}

/// Where everything of the grid goes, in logical pixels from the surface's top left.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerLayout {
    pub width: f32,
    pub height: f32,
    pub search: Frame,
    /// The tabs shown and their frames.
    pub tabs: Vec<(usize, Frame)>,
    /// The hairline under the tabs, and the one over the footer.
    pub rules: [f32; 2],
    pub grid: Frame,
    pub cells: Vec<CellLayout>,
    pub footer: Frame,
}

/// What a point of the grid is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerHit {
    /// Item `n` of the whole list.
    Symbol(usize),
    /// Tab `n`.
    Tab(usize),
}

impl PickerLayout {
    /// The geometry of `view` (the GPUI grid's: same sizes, tabs and cells).
    #[must_use]
    pub fn compute(view: &PickerView) -> Self {
        let (width, height) = size();
        let inner = width - PAD * 2.;
        let tabs_top = PAD + SEARCH_H;
        let mut x = PAD;
        let tabs = visible_tabs(view, inner)
            .map(|index| {
                let w = tab_width(&view.tabs[index].title);
                let frame = Frame::new(x, tabs_top + 3., w, TABS_H - 6.);
                x += w;
                (index, frame)
            })
            .collect();
        let rule_y = tabs_top + TABS_H + 1.;
        let grid = Frame::new(PAD, rule_y + 2., inner, CELL * VISIBLE_ROWS as f32);
        let highlighted_row = view.highlight.map(|index| index / COLUMNS);
        let cells = (0..view.items.len().min(COLUMNS * VISIBLE_ROWS))
            .map(|offset| {
                let index = view.first_row * COLUMNS + offset;
                let (row, column) = (offset / COLUMNS, offset % COLUMNS);
                CellLayout {
                    index,
                    offset,
                    frame: Frame::new(grid.x + column as f32 * CELL, grid.y + row as f32 * CELL, CELL, CELL),
                    numbered: highlighted_row == Some(index / COLUMNS),
                    selected: view.highlight == Some(index),
                }
            })
            .collect();
        let footer_top = height - PAD - FOOTER_H;
        Self {
            width,
            height,
            search: Frame::new(PAD + 4., PAD, inner - 8., SEARCH_H),
            tabs,
            rules: [rule_y, footer_top],
            grid,
            cells,
            footer: Frame::new(PAD + 4., footer_top + 1., inner - 8., FOOTER_H),
        }
    }

    /// What is at `(x, y)`.
    #[must_use]
    pub fn hit(&self, x: f32, y: f32) -> Option<PickerHit> {
        if let Some((index, _)) = self.tabs.iter().find(|(_, frame)| frame.contains(x, y)) {
            return Some(PickerHit::Tab(*index));
        }
        self.cells.iter().find(|cell| cell.frame.contains(x, y)).map(|cell| PickerHit::Symbol(cell.index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(items: usize, highlight: usize, first_row: usize) -> PickerView {
        PickerView {
            tabs: ["最近使用", "表情", "符號", "箭頭"]
                .iter()
                .map(|title| Tab { id: (*title).to_owned(), title: (*title).to_owned() })
                .collect(),
            tab: Some(1),
            query: String::new(),
            prefilled: false,
            first_row,
            row_count: items.div_ceil(COLUMNS) + first_row,
            items: (0..items).map(|n| Item { text: n.to_string(), name: format!("第{n}個") }).collect(),
            highlight: Some(highlight),
        }
    }

    #[test]
    fn the_grid_is_nine_across_and_its_cells_hit_their_index() {
        let v = view(20, 10, 2);
        let layout = PickerLayout::compute(&v);
        assert_eq!(layout.cells.len(), 20);
        assert_eq!((layout.width, layout.height), size());
        let cell = &layout.cells[10];
        assert_eq!(cell.index, 2 * COLUMNS + 10);
        assert!((cell.frame.x - (PAD + CELL)).abs() < 0.01, "second column");
        assert!((cell.frame.y - (layout.grid.y + CELL)).abs() < 0.01, "second row");
        let (cx, cy) = (cell.frame.x + CELL / 2., cell.frame.y + CELL / 2.);
        assert_eq!(layout.hit(cx, cy), Some(PickerHit::Symbol(28)));
        assert!(!layout.cells[10].selected && !layout.cells[10].numbered, "highlight is index 10, the first row");
        assert!(layout.cells.iter().filter(|c| c.numbered).count() == 0, "the highlighted row is not shown");
    }

    #[test]
    fn the_highlighted_row_is_numbered_and_tabs_hit_their_index() {
        let v = view(20, 12, 1);
        let layout = PickerLayout::compute(&v);
        assert!(layout.cells[3].selected);
        assert_eq!(layout.cells.iter().filter(|c| c.numbered).count(), COLUMNS);
        let (index, frame) = layout.tabs[2];
        assert_eq!(index, 2);
        assert_eq!(layout.hit(frame.x + 2., frame.y + 2.), Some(PickerHit::Tab(2)));
        assert_eq!(layout.hit(1., 1.), None);
        assert_eq!(v.highlighted_name(), "第3個");
        assert_eq!(v.position(), "2/4");
    }

    #[test]
    fn the_view_round_trips_through_json() {
        let v = view(3, 0, 0);
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(serde_json::from_value::<PickerView>(json).unwrap(), v);
    }
}

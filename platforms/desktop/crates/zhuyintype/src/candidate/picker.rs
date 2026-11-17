//! 符號與 Emoji: the picker's grid in the candidate window (`zt_core::symbols::PickerView`).  The window is the
//! candidate window's (same surface, edge, placement and appearance motion; never focused): a search line, the
//! category tabs, a grid of rows with the highlighted row's 1–9, and the highlighted symbol's name.  Clicks insert
//! a symbol or switch the tab (`PopupAction::PickSymbol`, `PopupAction::SymbolTab`).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString, Styled, div, px,
};
use zt_core::service::PopupAction;
use zt_core::symbols::{COLUMNS, PickerView, VISIBLE_ROWS};

use super::layout::SMALL_FONT;
use super::panel::{ActionHandler, Chrome, edge, shell};
use crate::theme::Palette;
use crate::theme::candidate::{self as tokens};

/// One cell of the grid.
const CELL: f32 = 36.;
const PAD: f32 = 8.;
const SEARCH_H: f32 = 30.;
const TABS_H: f32 = 28.;
const FOOTER_H: f32 = 26.;
const RADIUS: f32 = 10.;
const SYMBOL_FONT: f32 = 20.;
const TAB_FONT: f32 = 12.;

/// The surface's size (fixed, so the window does not jump between categories).
#[must_use]
pub fn size() -> (f32, f32) {
    (PAD * 2. + CELL * COLUMNS as f32, PAD * 2. + SEARCH_H + TABS_H + CELL * VISIBLE_ROWS as f32 + FOOTER_H)
}

/// Where the grid's text starts, for lining the window up with the caret.
pub const TEXT_INSET: f32 = PAD + 10.;

/// The tabs shown: a run that fits the width and holds the active one (titles are measured roughly: CJK at the font
/// size, plus padding).
fn visible_tabs(view: &PickerView, width: f32) -> std::ops::Range<usize> {
    let tab_w = |index: usize| view.tabs[index].title.chars().count() as f32 * TAB_FONT + 14.;
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

/// The picker's surface.
pub fn render(view: &PickerView, p: &Palette, chrome: Chrome, actions: Option<ActionHandler>) -> AnyElement {
    let c = tokens::colors(p);
    let (width, height) = size();
    let inner = width - PAD * 2.;
    let mut surface = shell(chrome, &c, RADIUS).w(px(width)).h(px(height)).text_color(p.text);

    // The search line: the query, or what it takes.
    let searching = !view.query.is_empty();
    surface = surface.child(
        div()
            .absolute()
            .left(px(PAD + 4.))
            .top(px(PAD))
            .w(px(inner - 8.))
            .h(px(SEARCH_H))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(Icon::new(IconName::Search).size(px(14.)).text_color(p.text_tertiary))
            .child(
                div().text_size(px(13.)).truncate().text_color(if searching { p.text } else { p.text_tertiary }).child(
                    SharedString::from(if searching {
                        view.query.clone()
                    } else {
                        "打注音或英文搜尋".to_owned()
                    }),
                ),
            )
            .when(searching, |this| {
                this.child(div().flex_1())
                    .child(div().text_size(px(SMALL_FONT)).text_color(p.text_tertiary).child("Esc 清除・Tab 回類別"))
            }),
    );

    // The tabs.
    let tabs_top = PAD + SEARCH_H;
    let mut tabs = div()
        .absolute()
        .left(px(PAD))
        .top(px(tabs_top))
        .w(px(inner))
        .h(px(TABS_H))
        .flex()
        .items_center()
        .overflow_hidden();
    for index in visible_tabs(view, inner) {
        let active = view.tab == Some(index);
        let title = view.tabs[index].title.clone();
        let actions = actions.clone();
        tabs = tabs.child(
            div()
                .id(("symbol-tab", index))
                .flex_none()
                .px(px(7.))
                .h(px(TABS_H - 6.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_size(px(TAB_FONT))
                .text_color(if active { p.text } else { p.text_secondary })
                .when(active, |this| this.bg(c.selection).font_weight(FontWeight::MEDIUM))
                .when_some(actions, |this, actions| {
                    this.cursor_pointer()
                        .hover(|style| style.bg(c.hover))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            actions(PopupAction::SymbolTab(index), window, cx)
                        })
                })
                .child(SharedString::from(title)),
        );
    }
    surface = surface.child(tabs);
    let rule_y = tabs_top + TABS_H + 1.;
    surface = surface.child(div().absolute().left_0().top(px(rule_y)).w(px(width)).h(px(1.)).bg(c.hairline));

    // The grid.
    let grid_top = rule_y + 2.;
    let highlighted_row = view.highlight.map(|index| index / COLUMNS);
    let mut grid = div().absolute().left(px(PAD)).top(px(grid_top)).w(px(inner)).h(px(CELL * VISIBLE_ROWS as f32));
    if view.items.is_empty() {
        grid = grid
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_size(px(13.)).text_color(p.text_tertiary).child("找不到符合的符號"));
    }
    for (offset, item) in view.items.iter().enumerate() {
        let index = view.first_row * COLUMNS + offset;
        let (row, column) = (offset / COLUMNS, offset % COLUMNS);
        let selected = view.highlight == Some(index);
        let numbered = highlighted_row == Some(index / COLUMNS);
        let actions = actions.clone();
        grid = grid.child(
            div()
                .id(("symbol", index))
                .absolute()
                .left(px(column as f32 * CELL))
                .top(px(row as f32 * CELL))
                .size(px(CELL))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .when(selected, |this| this.bg(c.selection))
                .when_some(actions, |this, actions| {
                    this.cursor_pointer()
                        .hover(|style| style.bg(c.hover))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            actions(PopupAction::PickSymbol(index), window, cx)
                        })
                })
                .when(numbered, |this| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(3.))
                            .top(px(1.))
                            .text_size(px(9.))
                            .line_height(px(11.))
                            .font_features(tokens::tabular())
                            .text_color(p.text_tertiary)
                            .child(SharedString::from((column + 1).to_string())),
                    )
                })
                .child(
                    div()
                        .text_size(px(SYMBOL_FONT))
                        .line_height(px(SYMBOL_FONT * 1.2))
                        .child(SharedString::from(item.text.clone())),
                ),
        );
    }
    surface = surface.child(grid);

    // The footer: the highlighted symbol's name, and where the row is.
    let footer_top = height - PAD - FOOTER_H;
    let name = view
        .highlight
        .and_then(|index| index.checked_sub(view.first_row * COLUMNS))
        .and_then(|offset| view.items.get(offset))
        .map(|item| item.name.clone())
        .unwrap_or_default();
    let position = highlighted_row.map(|row| format!("{}/{}", row + 1, view.row_count.max(1))).unwrap_or_default();
    surface = surface.child(div().absolute().left_0().top(px(footer_top)).w(px(width)).h(px(1.)).bg(c.hairline)).child(
        div()
            .absolute()
            .left(px(PAD + 4.))
            .top(px(footer_top + 1.))
            .w(px(inner - 8.))
            .h(px(FOOTER_H))
            .flex()
            .items_center()
            .text_size(px(SMALL_FONT))
            .text_color(p.text_secondary)
            .child(div().flex_1().truncate().child(SharedString::from(name)))
            .child(
                div().font_features(tokens::tabular()).text_color(p.text_tertiary).child(SharedString::from(position)),
            ),
    );
    surface.children(edge(chrome, &c, RADIUS)).into_any_element()
}

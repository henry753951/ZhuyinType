//! 符號與 Emoji's grid in the candidate window, drawn with Direct2D: the broker's `Panel::picker` (the view the GPUI
//! grid draws on Linux) at the geometry of `zt_panel::picker::PickerLayout` — the search line, the category tabs,
//! nine symbols a row with the highlighted row's 1–9, the highlighted symbol's name.  Emoji are in colour
//! (`Painter::text` draws colour fonts; DirectWrite falls back to Segoe UI Emoji).  Nothing moves, so Windows'
//! 「動畫效果」 has nothing to turn off here.

use zt_panel::layout::{Frame, SMALL_FONT};
use zt_panel::picker::{NO_MATCH, PickerHit, PickerLayout, PickerView, SEARCH_PLACEHOLDER, SYMBOL_FONT, TAB_FONT};

use crate::panel_draw::{Family, Painter};

/// The grid's content over the surface (`PanelWindow` draws the surface first).
pub(crate) fn content(p: &Painter<'_>, view: &PickerView, layout: &PickerLayout, hover: Option<PickerHit>) {
    let c = p.c;

    // The search line: a magnifier, then the query or what it takes.
    let search = layout.search;
    let cy = search.y + search.h / 2.;
    p.outline(Frame::new(search.x + 1., cy - 6., 10., 10.), 5., c.text_tertiary, 1.4);
    p.line(&[(search.x + 9.5, cy + 2.5), (search.x + 13., cy + 6.)], 1.4, c.text_tertiary);
    let searching = !view.query.is_empty();
    let text_x = search.x + 20.;
    let (query, ink) = if searching { (view.query.as_str(), c.text) } else { (SEARCH_PLACEHOLDER, c.text_tertiary) };
    p.text(query, 13., false, Family::Ui, Frame::new(text_x, search.y, search.w - 20., search.h), 0., ink);

    // The tabs: the active one on the selection colour.
    for &(index, frame) in &layout.tabs {
        let active = view.tab == Some(index);
        if active {
            p.fill(frame, 6., c.selection);
            if let Some(outline) = c.selection_outline {
                p.outline(frame, 6., outline, c.stroke);
            }
        } else if hover == Some(PickerHit::Tab(index)) {
            p.fill(frame, 6., c.hover);
        }
        let ink = if active { c.selection_text } else { c.text_secondary };
        let title = view.tabs.get(index).map_or("", |tab| tab.title.as_str());
        p.text(title, TAB_FONT, active, Family::Ui, frame, 0.5, ink);
    }
    let rule = |y: f32| p.fill(Frame::new(0., y, layout.width, 1.), 0., c.hairline);
    rule(layout.rules[0]);

    // The grid.
    if layout.cells.is_empty() {
        p.text(NO_MATCH, 13., false, Family::Ui, layout.grid, 0.5, c.text_tertiary);
    }
    for cell in &layout.cells {
        let Some(item) = view.items.get(cell.offset) else { continue };
        let f = cell.frame;
        if cell.selected {
            p.fill(f, 7., c.selection);
            if let Some(outline) = c.selection_outline {
                p.outline(f, 7., outline, c.stroke);
            }
        } else if hover == Some(PickerHit::Symbol(cell.index)) {
            p.fill(f, 7., c.hover);
        }
        if cell.numbered {
            let key = (cell.offset % zt_panel::picker::COLUMNS + 1).to_string();
            let ink = if cell.selected { c.selection_secondary } else { c.text_tertiary };
            p.text(&key, 9., false, Family::Ui, Frame::new(f.x + 3., f.y + 1., 10., 11.), 0., ink);
        }
        let ink = if cell.selected { c.selection_text } else { c.text };
        p.text(&item.text, SYMBOL_FONT, false, Family::Ui, f, 0.5, ink);
    }

    // The footer: the highlighted symbol's name; the keys for where the keys are now, and which row of how many.
    rule(layout.rules[1]);
    let footer = layout.footer;
    let position = view.position();
    let right = if position.is_empty() { view.hint.clone() } else { format!("{}　{position}", view.hint) };
    let right_w = if right.trim().is_empty() { 0. } else { 170. };
    let name = Frame::new(footer.x, footer.y, footer.w - right_w, footer.h);
    p.text(view.highlighted_name(), SMALL_FONT, false, Family::Ui, name, 0., c.text_secondary);
    if right_w > 0. {
        let at = Frame::new(footer.x + footer.w - right_w, footer.y, right_w, footer.h);
        p.text(right.trim(), SMALL_FONT, false, Family::Ui, at, 1., c.text_tertiary);
    }
}

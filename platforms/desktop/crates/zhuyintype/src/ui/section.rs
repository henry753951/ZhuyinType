//! A section (macOS `SettingsSection`, in this app's own look): a heading, then its rows as a list with hairlines
//! between them, open on the page surface rather than boxed in a filled card.  When a link leads to the section (its
//! title as the place's row), the heading turns accent and the rows tint once.
//!
//! An untitled section continues the one before it: it sits `SECTION_GAP_UNTITLED` under it with a hairline above its
//! first row, so the space reads as a break in one group rather than a missing heading.  The first section of a page
//! says so with `first()` (no pull-up); `bare()` draws content that is not a list (tiles, a grid, a stage) without
//! hairlines.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    canvas, div,
};

use super::row::{anchor_probe, arrival_tint, arrived};
use super::v;
use crate::app;
use crate::theme::{self, size, text, weight};

#[derive(IntoElement)]
pub struct Section {
    title: Option<SharedString>,
    footer: Option<AnyElement>,
    rows: Vec<AnyElement>,
    card: bool,
    first: bool,
}

/// A section titled `title` (empty: no header).
pub fn section(title: impl Into<SharedString>) -> Section {
    let title = title.into();
    Section { title: (!title.is_empty()).then_some(title), footer: None, rows: Vec::new(), card: true, first: false }
}

impl Section {
    /// A row of the card.
    pub fn row(mut self, row: impl IntoElement) -> Self {
        self.rows.push(row.into_any_element());
        self
    }

    /// Several rows.
    pub fn rows(mut self, rows: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.rows.extend(rows.into_iter().map(IntoElement::into_any_element));
        self
    }

    /// No card: the rows stand on the window under the header (tiles, feature cards; macOS `card: false`).
    pub fn bare(mut self) -> Self {
        self.card = false;
        self
    }

    /// The page's first section (an untitled one is not pulled up to the section before it).
    pub fn first(mut self) -> Self {
        self.first = true;
        self
    }

    /// Small text under the card (a link to another page, a note).
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl RenderOnce for Section {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let count = self.rows.len();
        let arrival = self.title.as_ref().and_then(|title| arrived(title, cx).map(|g| (title.clone(), g)));
        #[cfg(test)]
        if let Some(title) = &self.title {
            super::row::record_anchor(title, cx);
        }
        let untitled = self.title.is_none();
        let continues = untitled && !self.first;
        let hairline = || div().w_full().h(size::HAIRLINE).bg(p.separator);
        // A link to the whole section focuses it (not a Tab stop: Tab goes on to its first control); while a link
        // waits, a titled section reports where it is, so the row's heading can stay in view.
        let focus = self.title.as_ref().map(|title| {
            let key = SharedString::from(format!("section-focus-{title}"));
            window.use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(false)).read(cx).clone()
        });
        if let (Some((title, _)), Some(focus)) = (&arrival, &focus) {
            app::focus_on_arrival(title, focus, window, cx);
        }
        let reports = self.title.is_some() && app::nav(cx).arriving().is_some();
        v().w_full()
            .relative()
            .when_some(focus, |this, focus| this.track_focus(&focus))
            .when(reports, |this| {
                this.child(
                    canvas(|bounds, _, cx| app::section_laid_out(bounds, cx), |_, _, _, _| {}).absolute().size_full(),
                )
            })
            .when_some(arrival.clone(), |this, (title, _)| this.child(anchor_probe(title, app::Target::Section)))
            .when(continues, |this| this.mt(size::SECTION_GAP_UNTITLED - size::SECTION_GAP))
            .when_some(self.title, |this, title| {
                this.child(
                    div()
                        .px(size::ROW_PADDING_X)
                        .pb(size::SECTION_HEADING_PB)
                        .border_b_1()
                        .border_color(p.separator)
                        .text_size(text::HEADLINE)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(if arrival.is_some() { p.accent } else { p.text })
                        .child(title),
                )
            })
            .when(count > 0 && !self.card, |this| {
                this.child(
                    v().relative()
                        .w_full()
                        .when(!untitled, |this| this.pt(size::SECTION_BARE_PT))
                        .children(self.rows.drain(..)),
                )
            })
            .when(count > 0 && self.card, |this| {
                this.child(
                    v().w_full()
                        .relative()
                        .when(untitled, |this| this.child(hairline()))
                        .when(arrival.is_some(), |this| this.child(arrival_tint(window, cx)))
                        .children(self.rows.into_iter().enumerate().map(|(index, row)| {
                            div().w_full().when(index > 0, |this| this.child(hairline())).child(row)
                        })),
                )
            })
            .when_some(self.footer, |this, footer| {
                this.child(
                    div()
                        .px(size::ROW_PADDING_X)
                        .pt(size::FOOTER_PT)
                        .text_size(text::SUBTITLE)
                        .text_color(p.text_secondary)
                        .child(footer),
                )
            })
    }
}

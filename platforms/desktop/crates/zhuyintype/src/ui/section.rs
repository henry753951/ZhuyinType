//! A section: a small grey header over a card of rows separated by hairlines (macOS `SettingsSection`).  When a link
//! leads to the section (its title as the place's row), the header turns accent and the card tints once.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use super::row::{anchor_probe, arrival_tint, arrived};
use super::v;
use crate::theme::{self, size, text};

#[derive(IntoElement)]
pub struct Section {
    title: Option<SharedString>,
    footer: Option<AnyElement>,
    rows: Vec<AnyElement>,
    card: bool,
}

/// A section titled `title` (empty: no header).
pub fn section(title: impl Into<SharedString>) -> Section {
    let title = title.into();
    Section { title: (!title.is_empty()).then_some(title), footer: None, rows: Vec::new(), card: true }
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

    /// Small text under the card (a link to another page, a note).
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl RenderOnce for Section {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let count = self.rows.len();
        let arrival = self.title.as_ref().and_then(|title| arrived(title, cx).map(|g| (title.clone(), g)));
        v().w_full()
            .gap(px(7.))
            .when_some(self.title, |this, title| {
                this.child(
                    div()
                        .px(size::ROW_PADDING_X)
                        .text_size(text::SECTION)
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(if arrival.is_some() { p.accent } else { p.text_secondary })
                        .child(title),
                )
            })
            .when(count > 0 && !self.card, |this| {
                let probe = arrival.clone().map(|(title, _)| anchor_probe(title));
                this.child(v().relative().w_full().children(self.rows.drain(..)).children(probe))
            })
            .when(count > 0 && self.card, |this| {
                this.child(
                    v().w_full()
                        .relative()
                        .rounded(size::RADIUS_CARD)
                        .bg(p.card)
                        .border_1()
                        .border_color(p.separator)
                        .overflow_hidden()
                        .when_some(arrival.clone(), |this, (title, generation)| {
                            this.child(arrival_tint(format!("section-{title}"), generation, cx)).child(anchor_probe(title))
                        })
                        .children(self.rows.into_iter().enumerate().map(|(index, row)| {
                            div()
                                .w_full()
                                .when(index > 0, |this| {
                                    this.child(div().mx(size::ROW_PADDING_X).h(px(1.)).bg(p.separator))
                                })
                                .child(row)
                        })),
                )
            })
            .when_some(self.footer, |this, footer| {
                this.child(div().px(size::ROW_PADDING_X).text_size(text::SUBTITLE).text_color(p.text_secondary).child(footer))
            })
    }
}

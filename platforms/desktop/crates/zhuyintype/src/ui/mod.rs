//! The design system's components.  Pages are composed only from these and the tokens in `theme`; a page that needs
//! something new adds a component here rather than styling raw elements.

pub mod actions;
pub mod alert;
pub mod content;
pub mod controls;
pub mod keycap;
pub mod list;
pub mod row;
pub mod search;
pub mod section;
pub mod slider;
pub mod stepper;

pub use controls::*;
pub use keycap::{key_combo, keycap};
pub use row::{Row, row, setting_row};
pub use section::section;

use gpui_kit::{Div, Styled, div};

/// A horizontal flex row, items centred.
pub fn h() -> Div {
    div().flex().flex_row().items_center()
}

/// Whether the window is narrow: the page's column has no room for decorations beside a row's text (the minimum
/// window width and a little more).
pub fn narrow(window: &gpui_kit::Window) -> bool {
    window.viewport_size().width < crate::theme::size::NARROW_W
}

/// A vertical flex column.
pub fn v() -> Div {
    div().flex().flex_col()
}
pub mod charts;
pub mod current_keys;
pub mod demo;
pub mod feature;
pub mod feature_card;
pub mod shortcuts;

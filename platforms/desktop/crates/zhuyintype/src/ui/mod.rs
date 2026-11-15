//! The design system's components.  Pages are composed only from these and the tokens in `theme`; a page that needs
//! something new adds a component here rather than styling raw elements.

pub mod controls;
pub mod row;
pub mod section;
pub mod actions;
pub mod alert;
pub mod list;
pub mod content;

pub use controls::*;
pub use row::{Accessory, Row, row, setting_row};
pub use section::{Section, section};

use gpui_kit::{Div, Styled, div};

/// A horizontal flex row, items centred.
pub fn h() -> Div {
    div().flex().flex_row().items_center()
}

/// A vertical flex column.
pub fn v() -> Div {
    div().flex().flex_col()
}

//! The one search field (the list pages' toolbar, 快捷鍵): `H_MD` tall, radius `R_SM`, a control's border and fill, a
//! small search icon in tertiary text and a clear button once something is typed.  Boxed, it starts at the rows' outer
//! edge.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{App, Div, Entity, ParentElement, SharedString, Styled, div, px};

use super::h;
use crate::theme::{self, size, space};

/// A search field over `state`, its text read aloud as `label`.
pub fn search_field(state: &Entity<InputState>, label: impl Into<SharedString>, cx: &App) -> Div {
    let p = theme::palette(cx);
    h().h(size::H_MD)
        .min_w(px(0.))
        .pl(space::S8)
        .pr(space::S4)
        .gap(space::S4)
        .rounded(size::R_SM)
        .bg(p.control)
        .border_1()
        .border_color(p.control_border)
        .child(Icon::new(IconName::Search).size(size::ICON_SM).text_color(p.text_tertiary))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(Input::new(state).appearance(false).small().cleanable(true).aria_label(label.into())),
        )
}

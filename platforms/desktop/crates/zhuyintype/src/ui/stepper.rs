//! A stepper (macOS `Stepper`), in this app's look: − and ＋ side by side in one bordered control `H_MD` tall, for a
//! number setting the row shows as its value (「9 個」).  Each half is off at its end of the range.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Window, div,
};

use super::h;
use crate::theme::{self, size};

/// A stepper from `min` to `max` at `value`; `on_change` gets the new value.
pub fn stepper(
    id: impl Into<SharedString>,
    value: i64,
    min: i64,
    max: i64,
    on_change: impl Fn(i64, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let id: SharedString = id.into();
    let on_change = std::rc::Rc::new(on_change);
    let half = |delta: i64, icon: IconName| {
        let enabled = (min..=max).contains(&(value + delta));
        let on_change = on_change.clone();
        h().id(ElementId::Name(SharedString::from(format!("{id}-{delta}"))))
            .w(size::H_MD)
            .h_full()
            .justify_center()
            .when(enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(p.hover))
                    .active(|style| style.bg(p.pressed))
                    .on_click(move |_, window, cx| on_change(value + delta, window, cx))
            })
            .child(Icon::new(icon).size(size::ICON_SM).text_color(if enabled { p.text } else { p.text_tertiary }))
    };
    h().flex_shrink_0()
        .h(size::H_MD)
        .rounded(size::R_SM)
        .overflow_hidden()
        .bg(p.control)
        .border_1()
        .border_color(p.control_border)
        .child(half(-1, IconName::Minus))
        .child(div().w(size::HAIRLINE).h_full().bg(p.separator))
        .child(half(1, IconName::Plus))
}

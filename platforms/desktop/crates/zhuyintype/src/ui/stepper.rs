//! A stepper (macOS `Stepper`): ⌃ and ⌄ stacked in one small control, for a number setting the row shows as its value
//! (「9 個」).  Each arrow is off at its end of the range.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::v;
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
    let arrow = |delta: i64, icon: IconName| {
        let enabled = (min..=max).contains(&(value + delta));
        let on_change = on_change.clone();
        div()
            .id(ElementId::Name(SharedString::from(format!("{id}-{delta}"))))
            .w(px(18.))
            .h(px(12.))
            .flex()
            .items_center()
            .justify_center()
            .when(enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(p.hover))
                    .active(|style| style.bg(p.pressed))
                    .on_click(move |_, window, cx| on_change(value + delta, window, cx))
            })
            .child(Icon::new(icon).size(px(11.)).text_color(if enabled { p.text } else { p.text_tertiary }))
    };
    v().rounded(size::RADIUS_SMALL)
        .overflow_hidden()
        .bg(p.control)
        .border_1()
        .border_color(p.control_border)
        .shadow_xs()
        .child(arrow(1, IconName::ChevronUp))
        .child(div().h(px(1.)).bg(p.separator))
        .child(arrow(-1, IconName::ChevronDown))
}

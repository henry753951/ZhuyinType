//! Buttons a row or a dialog ends with (macOS push buttons 「匯出…」「回復…」, link buttons 「AGPL 3.0」), and a
//! setting switch that can be turned off by another setting.

use gpui_kit::component::Disableable as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div,
};
use serde_json::Value;

use crate::app;
use crate::theme::{self, size, space, text};

/// How a push button looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The quiet control fill (most row buttons).
    Normal,
    /// The inverted neutral (dark on light, light on dark): a dialog's default button.
    Default,
    /// Red text: 「清除」「回復」 in a confirmation.
    Destructive,
}

/// A push button `H_MD` tall: 「匯出…」, 「清除…」, 「完成」 (drawn here rather than by the library so the default one is
/// the inverted neutral).  Dialogs make theirs `H_LG` with `.h(size::H_LG)`.
pub fn push_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let p = theme::palette(cx);
    let (fill, border, foreground) = match (kind, enabled) {
        (ButtonKind::Default, true) => (p.primary, p.primary, p.on_primary),
        (ButtonKind::Default, false) => (p.subtle, p.subtle, p.text_tertiary),
        (_, false) => (p.control, p.control_border, p.text_tertiary),
        (ButtonKind::Normal, true) => (p.control, p.control_border, p.text),
        (ButtonKind::Destructive, true) => (p.control, p.control_border, p.danger),
    };
    let (hover, pressed) = if kind == ButtonKind::Default {
        (p.primary.opacity(0.88), p.primary.opacity(0.8))
    } else {
        (p.hover, p.pressed)
    };
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(size::H_MD)
        .px(space::S12)
        .rounded(size::R_SM)
        .bg(fill)
        .border_1()
        .border_color(border)
        .text_size(text::BODY)
        .text_color(foreground)
        .whitespace_nowrap()
        .child(label.into())
        .when(enabled, |this| {
            this.cursor_pointer()
                .when(kind == ButtonKind::Default, |this| this.hover(move |style| style.bg(hover)))
                .when(kind != ButtonKind::Default, |this| {
                    // The control fill is opaque: hovering layers the hover tint over it.
                    this.hover(move |style| style.bg(gpui_kit::Hsla::blend(p.control, hover)))
                })
                .active(move |style| {
                    style.bg(if kind == ButtonKind::Default { pressed } else { p.control.blend(pressed) })
                })
                .on_click(on_click)
        })
}

/// Text in the accent that acts as a link (「AGPL 3.0」, 「查看此版本」, a licence file).
pub fn link_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let p = theme::palette(cx);
    div()
        .id(id)
        .flex_shrink_0()
        .text_size(text::BODY)
        .text_color(p.accent)
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|style| style.underline())
        .active(|style| style.opacity(0.7))
        .on_click(on_click)
        .child(label.into())
}

/// A switch bound to a toggle setting that only works while `enabled` (a dependent row whose feature is off).
pub fn setting_switch_when(key: &'static str, enabled: bool, cx: &App) -> impl IntoElement {
    let on = app::settings(cx).bool(key);
    Switch::new(SharedString::from(format!("switch-{key}")))
        .color(theme::palette(cx).accent_fill)
        .checked(on)
        .disabled(!enabled)
        .accessibility_label(zt_core::settings::descriptor(key).map(|d| d.title.clone()).unwrap_or_default())
        .on_click(move |checked, _, cx| app::set_setting(key, Value::Bool(*checked), cx))
}

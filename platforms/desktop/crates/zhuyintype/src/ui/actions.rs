//! Buttons a row or a dialog ends with (macOS push buttons 「匯出…」「回復…」, link buttons 「AGPL 3.0」), and a
//! setting switch that can be turned off by another setting.

use gpui_kit::component::Disableable as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use serde_json::Value;

use crate::app;
use crate::theme::{self, size, text};

/// How a push button looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The quiet control fill (most row buttons).
    Normal,
    /// The accent: a dialog's default button.
    Default,
    /// Red text: 「清除」「回復」 in a confirmation.
    Destructive,
}

/// A push button: 「匯出…」, 「清除…」, 「完成」 (drawn here rather than by the library so the default one is a solid
/// accent).
pub fn push_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let p = theme::palette(cx);
    let (fill, border, foreground) = match kind {
        ButtonKind::Normal => (p.control, p.control_border, p.text),
        ButtonKind::Default => (p.accent, p.accent, p.on_accent),
        ButtonKind::Destructive => (p.control, p.control_border, p.danger),
    };
    let pressed = if kind == ButtonKind::Default { p.accent.opacity(0.8) } else { p.pressed };
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(px(26.))
        .px(px(12.))
        .rounded(size::RADIUS_CONTROL)
        .bg(fill)
        .border_1()
        .border_color(border)
        .text_size(text::BODY)
        .text_color(foreground)
        .whitespace_nowrap()
        .child(label.into())
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(move |style| style.opacity(0.92))
                .active(move |style| style.bg(pressed))
                .on_click(on_click)
        })
        .when(!enabled, |this| this.opacity(0.45))
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
        .checked(on)
        .disabled(!enabled)
        .accessibility_label(zt_core::settings::descriptor(key).map(|d| d.title.clone()).unwrap_or_default())
        .on_click(move |checked, _, cx| app::set_setting(key, Value::Bool(*checked), cx))
}

//! The controls a row ends with: switch, picker (pop-up menu or segments), slider, key caps, the ⓘ info button, the
//! › chevron, and small text links.  Each control bound to a setting reads and writes it through `app::set_setting`.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use serde_json::Value;
use zt_core::settings::{self, Kind};

use super::{h, v};
use crate::app;
use crate::theme::{self, size, text};

/// A switch bound to a toggle setting.
pub fn setting_switch(key: &'static str, cx: &App) -> impl IntoElement {
    let on = app::settings(cx).bool(key);
    Switch::new(SharedString::from(format!("switch-{key}")))
        .checked(on)
        .accessibility_label(settings::descriptor(key).map(|d| d.title.clone()).unwrap_or_default())
        .on_click(move |checked, _, cx| app::set_setting(key, Value::Bool(*checked), cx))
}

/// A switch with its own state (not a stored setting).
pub fn switch(
    id: impl Into<ElementId>,
    on: bool,
    on_change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Switch::new(id).checked(on).on_click(move |checked, window, cx| on_change(*checked, window, cx))
}

/// The picker of a choice setting: segments for two or three short labels, otherwise a pop-up menu.
pub fn setting_picker(key: &'static str, cx: &App) -> AnyElement {
    let Some(descriptor) = settings::descriptor(key) else { return div().into_any_element() };
    let current = app::settings(cx).get(key);
    let options: Vec<(SharedString, Value)> =
        descriptor.options.iter().map(|choice| (choice.label.clone().into(), choice.value.clone())).collect();
    let short = options.len() <= 3 && options.iter().all(|(label, _)| label.chars().count() <= 4);
    let on_pick = move |value: Value, _: &mut Window, cx: &mut App| app::set_setting(key, value, cx);
    if short && descriptor.extra.get("control").and_then(Value::as_str) == Some("segmented") {
        segmented(SharedString::from(format!("seg-{key}")), options, current, on_pick, cx).into_any_element()
    } else {
        popup_button(SharedString::from(format!("pick-{key}")), options, current, on_pick, cx).into_any_element()
    }
}

/// A pop-up button: the current choice's label with ⌃⌄, a menu of the choices with a check on the current one.
pub fn popup_button<T: Clone + PartialEq + 'static>(
    id: impl Into<ElementId>,
    options: Vec<(SharedString, T)>,
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let label = options.iter().find(|(_, value)| *value == current).map(|(label, _)| label.clone()).unwrap_or_default();
    let on_pick = Rc::new(on_pick);
    Button::new(id)
        .small()
        .custom(
            gpui_kit::component::button::ButtonCustomVariant::new(cx)
                .color(p.control)
                .foreground(p.text)
                .hover(p.control)
                .active(p.pressed)
                .shadow(true),
        )
        .border_1()
        .border_color(p.control_border)
        .child(
            h().gap(px(6.))
                .child(div().text_size(text::BODY).child(label))
                .child(Icon::new(IconName::ChevronsUpDown).size(px(12.)).text_color(p.text_secondary)),
        )
        .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
            for (label, value) in options.iter().cloned() {
                let on_pick = on_pick.clone();
                let checked = value == current;
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .checked(checked)
                        .on_click(move |_, window, cx| on_pick(value.clone(), window, cx)),
                );
            }
            menu
        })
}

/// Segments for two or three short choices.
pub fn segmented<T: Clone + PartialEq + 'static>(
    id: impl Into<SharedString>,
    options: Vec<(SharedString, T)>,
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let id = id.into();
    let on_pick = Rc::new(on_pick);
    h().p(px(2.)).gap(px(2.)).rounded(size::RADIUS_CONTROL).bg(p.pressed).children(options.into_iter().enumerate().map(
        |(index, (label, value))| {
            let selected = value == current;
            let on_pick = on_pick.clone();
            div()
                .id(SharedString::from(format!("{id}-{index}")))
                .px(px(10.))
                .py(px(3.))
                .rounded(size::RADIUS_SMALL)
                .text_size(text::SUBTITLE)
                .cursor_pointer()
                .when(selected, |this| this.bg(p.control).shadow_sm().text_color(p.text))
                .when(!selected, |this| this.text_color(p.text_secondary).hover(|style| style.text_color(p.text)))
                .on_click(move |_, window, cx| on_pick(value.clone(), window, cx))
                .child(label)
        },
    ))
}

/// The › at the end of a row that goes somewhere.
pub fn chevron(cx: &App) -> impl IntoElement {
    Icon::new(IconName::ChevronRight).size(px(13.)).text_color(theme::palette(cx).text_tertiary)
}

/// A key drawn as a key cap ("⇧", "空白", "Return").  `tint`: a conflict (red) or a highlight.
#[derive(IntoElement)]
pub struct Keycap {
    label: SharedString,
    tint: Option<gpui_kit::Hsla>,
    large: bool,
}

pub fn keycap(label: impl Into<SharedString>) -> Keycap {
    Keycap { label: label.into(), tint: None, large: false }
}

impl Keycap {
    pub fn tint(mut self, tint: gpui_kit::Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }
}

impl RenderOnce for Keycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let tint = self.tint.unwrap_or(p.text_secondary);
        div()
            .h(if self.large { px(26.) } else { px(21.) })
            .min_w(if self.large { px(26.) } else { px(21.) })
            .px(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(size::RADIUS_SMALL)
            .bg(p.control)
            .border_1()
            .border_color(if self.tint.is_some() { tint } else { p.control_border })
            .border_b_2()
            .text_size(if self.large { text::BODY } else { text::SUBTITLE })
            .font_weight(FontWeight::MEDIUM)
            .text_color(if self.tint.is_some() { tint } else { p.text })
            .child(self.label)
    }
}

/// Several key caps in a row: one combination ("⇧", "空白") or alternatives joined by 「或」.
pub fn key_combo(keys: impl IntoIterator<Item = impl Into<SharedString>>) -> impl IntoElement {
    h().gap(px(3.)).children(keys.into_iter().map(|key| keycap(key)))
}

/// The ⓘ button: a popover card titled `title` with `content`, explaining how an option behaves.
pub fn info_button(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let title: SharedString = title.into();
    Popover::new(id)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new("info-trigger")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::Info).size(px(16.)).text_color(p.text_tertiary))
                .tooltip(title.clone()),
        )
        .content(move |_, window, cx| info_card(title.clone(), content(window, cx), cx))
}

/// What an info popover shows; also drawn on its own (screenshots).
pub fn info_card(title: SharedString, content: AnyElement, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    v().w(px(380.))
        .p(px(16.))
        .gap(px(10.))
        .child(div().text_size(text::HEADLINE).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(title))
        .child(content)
        .into_any_element()
}

/// Small text that goes somewhere: 「在快捷鍵總覽中查看全部按鍵 ›」.
pub fn text_link(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    h().id(id)
        .gap(px(2.))
        .cursor_pointer()
        .text_size(text::SUBTITLE)
        .text_color(p.accent)
        .hover(|style| style.opacity(0.8))
        .on_click(on_click)
        .child(label.into())
        .child(Icon::new(IconName::ChevronRight).size(px(12.)))
}

/// The value of a setting as the row's trailing text (drill rows show where they lead).
pub fn value_text(value: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().text_size(text::BODY).text_color(theme::palette(cx).text_secondary).child(value.into())
}

/// The control for any registry setting by its kind.
pub fn setting_control(key: &'static str, cx: &App) -> AnyElement {
    match settings::descriptor(key).map(|d| d.kind) {
        Some(Kind::Toggle) => setting_switch(key, cx).into_any_element(),
        Some(Kind::Choice) => setting_picker(key, cx),
        Some(Kind::Number) => {
            let value = app::settings(cx).number(key);
            let unit = settings::descriptor(key)
                .and_then(|d| d.extra.get("unit"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            value_text(format!("{value}{unit}"), cx).into_any_element()
        }
        _ => div().into_any_element(),
    }
}

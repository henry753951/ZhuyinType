//! The controls a row ends with: switch, picker (pop-up menu or segments), icon buttons, the ⓘ info button, the ›
//! chevron, and small text links.  Every in-row control is `size::H_MD` tall.  Each control bound to a setting reads and writes it through `app::set_setting`.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use serde_json::Value;
use zt_core::settings::{self, Kind};

use super::{h, v};
use crate::app;
use crate::theme::{self, size, space, text, weight};

/// A switch bound to a toggle setting.
pub fn setting_switch(key: &'static str, cx: &App) -> impl IntoElement {
    let on = app::settings(cx).bool(key);
    Switch::new(SharedString::from(format!("switch-{key}")))
        .color(theme::palette(cx).accent_fill)
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
    AccentSwitch(Switch::new(id).checked(on).on_click(move |checked, window, cx| on_change(*checked, window, cx)))
}

/// A switch whose on track is `accent_fill` (the library's default is `primary`, an inverted neutral here).
#[derive(IntoElement)]
struct AccentSwitch(Switch);

impl gpui_kit::RenderOnce for AccentSwitch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.color(theme::palette(cx).accent_fill)
    }
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
                .hover(p.subtle)
                .active(p.pressed)
                .shadow(false),
        )
        .h(size::H_MD)
        .pl(space::S12)
        .pr(space::S8)
        .rounded(size::R_SM)
        .border_1()
        .border_color(p.control_border)
        .child(
            h().gap(space::S6)
                .child(div().text_size(text::BODY).child(label))
                .child(Icon::new(IconName::ChevronDown).size(size::ICON_SM).text_color(p.text_tertiary)),
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

/// Segments for two or three short choices: a track `H_MD` tall, segments `H_SM` inside it (radii concentric).  The
/// raised fill fades from the old choice to the new one; each segment takes the keyboard (Tab, then Space or Enter).
pub fn segmented<T: Clone + PartialEq + 'static>(
    id: impl Into<SharedString>,
    options: Vec<(SharedString, T)>,
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    _cx: &App,
) -> impl IntoElement {
    let on_pick = Rc::new(on_pick);
    let segments = options
        .into_iter()
        .map(|(label, value)| {
            let selected = value == current;
            let on_pick = on_pick.clone();
            let pick: PickHandler = Rc::new(move |window, cx| on_pick(value.clone(), window, cx));
            (label, selected, pick)
        })
        .collect();
    Segmented { id: id.into(), segments }
}

type PickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
struct Segmented {
    id: SharedString,
    segments: Vec<(SharedString, bool, PickHandler)>,
}

impl gpui_kit::RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let id = self.id;
        let fills: Vec<f32> = self
            .segments
            .iter()
            .enumerate()
            .map(|(index, (_, selected, _))| {
                let key = SharedString::from(format!("{id}-fill-{index}"));
                crate::motion::fade(key, *selected, crate::motion::SELECT, window, cx)
            })
            .collect();
        h().flex_shrink_0().h(size::H_MD).p(space::S2).gap(space::S2).rounded(size::R_SM).bg(p.subtle).children(
            self.segments.into_iter().zip(fills).enumerate().map(|(index, ((label, selected, pick), fill))| {
                h().id(SharedString::from(format!("{id}-{index}")))
                    .relative()
                    .h(size::H_SM)
                    .px(space::S12)
                    .justify_center()
                    .rounded(size::R_XS)
                    .text_size(text::SUBTITLE)
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|style| crate::motion::focus_ring(style, &p, p.surface.blend(p.subtle)))
                    .when(fill > 0., |this| {
                        this.child(div().absolute().inset_0().rounded(size::R_XS).bg(p.card).shadow_xs().opacity(fill))
                    })
                    .text_color(if selected { p.text } else { p.text_secondary })
                    .when(!selected, |this| {
                        this.hover(|style| style.bg(p.hover).text_color(p.text)).active(|style| style.bg(p.pressed))
                    })
                    .on_click(move |_, window, cx| pick(window, cx))
                    .child(div().relative().child(label))
            }),
        )
    }
}

/// The › at the end of a row that goes somewhere.
pub fn chevron(cx: &App) -> impl IntoElement {
    Icon::new(IconName::ChevronRight).size(size::ICON_SM).text_color(theme::palette(cx).text_tertiary)
}

/// How an icon button looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconButtonKind {
    /// A control's border and fill (＋ beside a list).
    Bordered,
    /// No border or fill until hovered (⋯).
    Ghost,
}

/// A square icon button `H_MD` tall (the list toolbar's ＋ and ⋯).
pub fn icon_button(id: impl Into<ElementId>, icon: IconName, kind: IconButtonKind, cx: &App) -> Button {
    let p = theme::palette(cx);
    let button = Button::new(id)
        .custom(
            gpui_kit::component::button::ButtonCustomVariant::new(cx)
                .color(if kind == IconButtonKind::Bordered { p.control } else { gpui_kit::transparent_black() })
                .foreground(p.text)
                .hover(p.hover)
                .active(p.pressed)
                .shadow(false),
        )
        .with_size(size::H_MD)
        .rounded(size::R_SM)
        .icon(
            Icon::new(icon)
                .size(if kind == IconButtonKind::Bordered { size::ICON_SM } else { size::ICON_MD })
                .text_color(p.text),
        );
    match kind {
        IconButtonKind::Bordered => button.border_1().border_color(p.control_border),
        IconButtonKind::Ghost => button,
    }
}

/// The ⓘ after a row's title: a popover card titled `title` with `content`, explaining how an option behaves.
pub fn info_button(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let title: SharedString = title.into();
    Popover::new(id)
        .anchor(Anchor::TopLeft)
        .trigger(
            Button::new("info-trigger")
                .custom(
                    gpui_kit::component::button::ButtonCustomVariant::new(cx)
                        .color(gpui_kit::transparent_black())
                        .foreground(p.text_tertiary)
                        .hover(p.hover)
                        .active(p.pressed)
                        .shadow(false),
                )
                .with_size(size::H_XS)
                .rounded(size::R_XS)
                .icon(Icon::new(IconName::Info).size(size::ICON_SM).text_color(p.text_tertiary))
                .tooltip(title.clone()),
        )
        .content(move |_, window, cx| info_card(title.clone(), content(window, cx), cx))
}

/// The width of an ⓘ card.
pub const INFO_CARD_W: gpui_kit::Pixels = gpui_kit::px(380.);

/// What an info popover shows; also drawn on its own (screenshots).
pub fn info_card(title: SharedString, content: AnyElement, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    v().w(INFO_CARD_W)
        .p(space::S16)
        .gap(space::S12)
        .child(div().text_size(text::HEADLINE).font_weight(weight::SEMIBOLD).text_color(p.text).child(title))
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
        .gap(space::S2)
        .cursor_pointer()
        .text_size(text::SUBTITLE)
        .text_color(p.accent)
        .rounded(size::R_XS)
        .tab_index(0)
        .focus_visible(|style| crate::motion::focus_ring(style, &p, p.surface))
        .hover(|style| style.opacity(0.8))
        .active(|style| style.opacity(0.6))
        .on_click(on_click)
        .child(label.into())
        .child(Icon::new(IconName::ChevronRight).size(size::ICON_XS))
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

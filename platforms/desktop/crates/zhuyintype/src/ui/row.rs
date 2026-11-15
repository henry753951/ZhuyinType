//! A settings row (macOS `SettingRow`): title, subtitle and an optional link on the left; value, control and
//! accessory (› or ⓘ) on the right, centred on the left block.  A dot in the margin marks a customised setting; a link
//! that leads to the row tints it once (the arrival cue).

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};

use super::{controls, h, v};
use crate::app;
use crate::theme::{self, size, text};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// What ends a row on the right.
pub enum Accessory {
    None,
    /// The row goes somewhere (the whole row is the button).
    Chevron,
    /// A ⓘ with an explanation card.
    Info { title: SharedString, content: Box<dyn Fn(&mut Window, &mut App) -> AnyElement> },
}

#[derive(IntoElement)]
pub struct Row {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    value: Option<SharedString>,
    link: Option<AnyElement>,
    trailing: Option<AnyElement>,
    accessory: Accessory,
    anchor: Option<SharedString>,
    marked: bool,
    indent: bool,
    dimmed: bool,
    on_click: Option<ClickHandler>,
    hover_group: Option<SharedString>,
}

/// A row titled `title`; `id` must be unique within the page.
pub fn row(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Row {
    Row {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        value: None,
        link: None,
        trailing: None,
        accessory: Accessory::None,
        anchor: None,
        marked: false,
        indent: false,
        dimmed: false,
        on_click: None,
        hover_group: None,
    }
}

impl Row {
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        let subtitle = subtitle.into();
        self.subtitle = (!subtitle.is_empty()).then_some(subtitle);
        self
    }

    pub fn subtitle_opt(mut self, subtitle: Option<impl Into<SharedString>>) -> Self {
        self.subtitle = subtitle.map(Into::into);
        self
    }

    /// Grey text before the control: a drill row's current value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// A small link under the subtitle (to another page).
    pub fn link(mut self, link: impl IntoElement) -> Self {
        self.link = Some(link.into_any_element());
        self
    }

    /// The control on the right.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// The ⓘ explanation.
    pub fn info(mut self, title: impl Into<SharedString>, content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.accessory = Accessory::Info { title: title.into(), content: Box::new(content) };
        self
    }

    /// The row's id for links: they scroll to it and it shows the arrival tint.
    pub fn anchor(mut self, anchor: impl Into<SharedString>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }

    /// The dot that says 「已自訂」.
    pub fn marked(mut self, marked: bool) -> Self {
        self.marked = marked;
        self
    }

    /// A dependent row: indented, secondary title.
    pub fn indent(mut self, indent: bool) -> Self {
        self.indent = indent;
        self
    }

    /// Greyed out: its feature is off.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Names the row as a hover group, so controls inside can show only while the pointer is on the row.
    pub fn hover_group(mut self, group: impl Into<SharedString>) -> Self {
        self.hover_group = Some(group.into());
        self
    }

    /// The whole row is a button with a ›.
    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        if matches!(self.accessory, Accessory::None) {
            self.accessory = Accessory::Chevron;
        }
        self
    }
}

/// Whether a link just led to `anchor` (the row or its section), and the navigation generation to key its animation.
pub(crate) fn arrived(anchor: &str, cx: &App) -> Option<usize> {
    let nav = app::nav(cx);
    nav.arrival
        .as_ref()
        .filter(|(row, at)| row.as_ref() == anchor && at.elapsed() < Duration::from_secs(3))
        .map(|_| nav.generation)
}

/// An invisible layer that reports where `anchor` is once laid out, so a waiting link can scroll to it.
pub(crate) fn anchor_probe(anchor: SharedString) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| app::scroll_into_view(&anchor, bounds, window, cx),
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// The arrival tint: rises, holds, fades (macOS: peak opacity 0.17).
pub(crate) fn arrival_tint(key: impl Into<SharedString>, generation: usize, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let key: SharedString = key.into();
    div()
        .absolute()
        .inset(px(4.))
        .rounded(size::RADIUS_CARD - px(4.))
        .bg(p.accent)
        .with_animation(
            SharedString::from(format!("arrival-{key}-{generation}")),
            Animation::new(Duration::from_millis(1600)),
            |this, t| {
                let opacity = if t < 0.15 { t / 0.15 } else if t < 0.45 { 1.0 } else { 1.0 - (t - 0.45) / 0.55 };
                this.opacity(0.17 * opacity)
            },
        )
}

impl RenderOnce for Row {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let clickable = self.on_click.is_some();
        let arrival = self.anchor.as_ref().and_then(|anchor| arrived(anchor, cx).map(|g| (anchor.clone(), g)));
        let accessory: Option<AnyElement> = match self.accessory {
            Accessory::None => None,
            Accessory::Chevron => Some(controls::chevron(cx).into_any_element()),
            Accessory::Info { title, content } => {
                let id = SharedString::from(format!("info-{}", self.title));
                Some(controls::info_button(id, title, content, cx).into_any_element())
            }
        };
        let _ = window;
        h().id(self.id)
            .when_some(self.hover_group, |this, group| this.group(group))
            .relative()
            .w_full()
            .min_h(size::ROW_MIN)
            .px(size::ROW_PADDING_X)
            .py(size::ROW_PADDING_Y)
            .gap(px(12.))
            .when_some(arrival, |this, (anchor, generation)| {
                this.child(arrival_tint(anchor.clone(), generation, cx)).child(anchor_probe(anchor))
            })
            .when(self.dimmed, |this| this.opacity(0.5))
            .when(clickable, |this| {
                this.cursor_pointer().hover(|style| style.bg(p.hover)).active(|style| style.bg(p.pressed))
            })
            .when_some(self.on_click, |this, handler| this.on_click(handler))
            .child(
                v().flex_1()
                    .min_w(px(0.))
                    .gap(px(3.))
                    .when(self.indent, |this| this.pl(px(18.)))
                    .child(
                        div()
                            .relative()
                            .text_size(text::BODY)
                            .text_color(if self.indent || self.dimmed { p.text_secondary } else { p.text })
                            .when(self.marked, |this| {
                                this.child(
                                    div().absolute().left(px(-11.)).top(px(7.)).size(px(6.)).rounded_full().bg(p.accent),
                                )
                            })
                            .child(self.title),
                    )
                    .when_some(self.subtitle, |this, subtitle| {
                        this.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(subtitle))
                    })
                    .when_some(self.link, |this, link| this.child(link)),
            )
            .child(
                h().flex_shrink_0()
                    .gap(px(8.))
                    .when_some(self.value, |this, value| this.child(controls::value_text(value, cx)))
                    .when_some(self.trailing, |this, trailing| this.child(trailing))
                    .when_some(accessory, |this, accessory| this.child(accessory)),
            )
    }
}

/// A row for a registry setting: its title, the subtitle for the current value, its control, the 已自訂 dot and its
/// anchor.  Add `.info(…)` or other builders as the macOS row has them.
pub fn setting_row(key: &'static str, cx: &App) -> Row {
    let settings = app::settings(cx);
    let descriptor = zt_core::settings::descriptor(key);
    let value = settings.get(key);
    let title = descriptor.map(|d| d.title.clone()).unwrap_or_else(|| key.to_owned());
    let subtitle = descriptor.and_then(|d| d.subtitle(&value)).map(str::to_owned);
    let anchor = descriptor.and_then(|d| d.anchor.clone()).unwrap_or_else(|| key.to_owned());
    row(SharedString::from(format!("row-{key}")), title)
        .subtitle_opt(subtitle)
        .anchor(anchor)
        .marked(settings.is_customised(key))
        .trailing(controls::setting_control(key, cx))
}

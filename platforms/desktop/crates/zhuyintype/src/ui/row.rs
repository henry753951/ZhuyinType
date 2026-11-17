//! A settings row (macOS `SettingRow`): title (with its ⓘ right after it), subtitle and an optional link on the left;
//! value, control and › on the right, centred on the left block.  The trailing column holds only the control, so every
//! row's control ends at the same edge.  A dot in the margin marks a customised setting; a link
//! that leads to the row tints it once (the arrival cue).

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div,
};

use super::{controls, h, v};
use crate::app;
use crate::theme::{self, size, space, text};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type CardContent = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// What ends a row on the right.
pub enum Accessory {
    None,
    /// The row goes somewhere (the whole row is the button).
    Chevron,
    /// The row opens and closes what is under it: a chevron that turns down while open.
    Disclosure(bool),
    /// A ⓘ with an explanation card.
    Info {
        title: SharedString,
        content: CardContent,
    },
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
    wrap_value: bool,
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
        wrap_value: false,
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
    pub fn info(
        mut self,
        title: impl Into<SharedString>,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
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

    /// A long value (a fact, not a control): the title keeps its width and the value wraps, right-aligned, instead
    /// of pushing the row wider than a narrow window.
    pub fn wrap_value(mut self, wrap: bool) -> Self {
        self.wrap_value = wrap;
        self
    }

    /// Names the row as a hover group, so controls inside can show only while the pointer is on the row.
    pub fn hover_group(mut self, group: impl Into<SharedString>) -> Self {
        self.hover_group = Some(group.into());
        self
    }

    /// The row opens and closes the rows under it (set before `on_click`).
    pub fn disclosure(mut self, open: bool) -> Self {
        self.accessory = Accessory::Disclosure(open);
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
    canvas(move |bounds, window, cx| app::scroll_into_view(&anchor, bounds, window, cx), |_, _, _, _| {})
        .absolute()
        .size_full()
}

/// How long the arrival tint takes: rises 0.24 s, holds, fades out by 1.6 s.
const ARRIVAL: Duration = Duration::from_millis(1600);

/// The arrival tint: rises, holds, fades (macOS: peak opacity 0.17), from the moment the link was followed.  It is a
/// fade, so it plays under reduced motion too (only the scroll there is immediate).
pub(crate) fn arrival_tint(window: &mut Window, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let t = app::nav(cx)
        .arrival
        .as_ref()
        .map_or(1., |(_, at)| (at.elapsed().as_secs_f32() / ARRIVAL.as_secs_f32()).min(1.));
    if t < 1. {
        window.request_animation_frame();
    }
    let opacity = if t < 0.15 {
        t / 0.15
    } else if t < 0.45 {
        1.0
    } else {
        1.0 - (t - 0.45) / 0.55
    };
    div().absolute().inset(space::S2).rounded(size::R_SM).bg(p.accent).opacity(0.17 * opacity)
}

/// A row's focus handle, and whether the focus was in it when last drawn (the page scrolls to it once on arrival).
struct Focus {
    handle: gpui_kit::FocusHandle,
    shown: bool,
}

/// The 已自訂 dot's size.
const DOT: gpui_kit::Pixels = space::S6;

impl RenderOnce for Row {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let clickable = self.on_click.is_some();
        // The row holds the keyboard focus (a clickable row) or contains it (its switch, its pop-up button): when the
        // focus arrives, the page scrolls to show the row.
        let focus = window.use_keyed_state(SharedString::from(format!("focus-{:?}", self.id)), cx, |_, cx| {
            let handle = cx.focus_handle();
            Focus {
                handle: if clickable { handle.tab_index(0).tab_stop(true) } else { handle.tab_stop(false) },
                shown: false,
            }
        });
        let focused = focus.read(cx).handle.contains_focused(window, cx);
        let reveal = focused && !focus.read(cx).shown;
        if focused != focus.read(cx).shown {
            focus.update(cx, |focus, _| focus.shown = focused);
        }
        let focus_handle = focus.read(cx).handle.clone();
        let arrival = self.anchor.as_ref().and_then(|anchor| arrived(anchor, cx).map(|g| (anchor.clone(), g)));
        let chevron = match self.accessory {
            Accessory::Chevron => Some(controls::chevron(cx).into_any_element()),
            Accessory::Disclosure(open) => {
                // The chevron turns down as the rows open (at once under reduced motion).
                let key = SharedString::from(format!("disclosure-{:?}", self.id));
                let target = if open { std::f32::consts::FRAC_PI_2 } else { 0. };
                let angle = crate::motion::glide(key, target, crate::motion::DISCLOSE, window, cx);
                Some(
                    gpui_kit::component::Icon::new(gpui_kit::assets::IconName::ChevronRight)
                        .size(size::ICON_SM)
                        .text_color(p.text_tertiary)
                        .rotate(gpui_kit::radians(angle))
                        .into_any_element(),
                )
            }
            _ => None,
        };
        let info: Option<AnyElement> = match self.accessory {
            Accessory::Info { title, content } => {
                let id = SharedString::from(format!("info-{}", self.title));
                Some(controls::info_button(id, title, content, cx).into_any_element())
            }
            _ => None,
        };
        let group: SharedString = self.hover_group.clone().unwrap_or_else(|| "row-hover".into());
        let line = text::BODY * 1.5;
        h().id(self.id)
            .group(group.clone())
            .relative()
            .w_full()
            .min_h(size::ROW_MIN)
            .px(size::ROW_PADDING_X)
            // The text column adds `S4` above and below: `ROW_PADDING_Y` around the text, while a control as tall
            // as `H_MD` still fits a single-line row of `ROW_MIN`.
            .py(size::ROW_PADDING_Y - space::S4)
            .gap(size::ROW_GAP)
            // The hover fill stays clear of the hairlines above and below.
            .when(clickable, |this| {
                this.child(
                    div()
                        .id("row-fill")
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(space::S2)
                        .bottom(space::S2)
                        .rounded(size::R_SM)
                        .group_hover(group.clone(), |style| style.bg(p.hover))
                        .group_active(group.clone(), |style| style.bg(p.pressed)),
                )
            })
            .when_some(arrival, |this, (anchor, _)| this.child(arrival_tint(window, cx)).child(anchor_probe(anchor)))
            .when(self.dimmed, |this| this.opacity(0.5))
            .track_focus(&focus_handle)
            .when(reveal, |this| this.child(canvas(app::reveal, |_, _, _, _| {}).absolute().size_full()))
            .when(clickable, |this| {
                this.cursor_pointer()
                    .rounded(size::R_SM)
                    .focus_visible(|style| crate::motion::focus_ring(style, &p, p.surface))
            })
            .when_some(self.on_click, |this, handler| this.on_click(handler))
            .child(
                v().flex_1()
                    .min_w(gpui_kit::px(0.))
                    .py(space::S4)
                    .when(self.wrap_value, |this| this.flex_none())
                    .gap(space::S2)
                    .when(self.indent, |this| this.pl(size::ROW_INDENT))
                    .child(
                        h().relative()
                            .gap(space::S4)
                            .text_size(text::BODY)
                            .line_height(line)
                            .text_color(if self.indent || self.dimmed { p.text_secondary } else { p.text })
                            .when(self.marked, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .left(-(size::ROW_PADDING_X + DOT) / 2.)
                                        .top((line - DOT) / 2.)
                                        .size(DOT)
                                        .rounded_full()
                                        .bg(p.accent),
                                )
                            })
                            .child(div().min_w(gpui_kit::px(0.)).child(self.title))
                            .children(info),
                    )
                    .when_some(self.subtitle, |this, subtitle| {
                        this.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(subtitle))
                    })
                    .when_some(self.link, |this, link| this.child(link)),
            )
            .child(
                h().flex_shrink_0()
                    .gap(space::S8)
                    .when(self.wrap_value, |this| this.flex_1().min_w(gpui_kit::px(0.)).justify_end())
                    .when_some(self.value, |this, value| {
                        if self.wrap_value {
                            this.child(
                                div()
                                    .flex_1()
                                    .min_w(gpui_kit::px(0.))
                                    .text_right()
                                    .text_size(text::BODY)
                                    .text_color(p.text_secondary)
                                    .child(value),
                            )
                        } else {
                            this.child(controls::value_text(value, cx))
                        }
                    })
                    .when_some(self.trailing, |this, trailing| this.child(trailing))
                    .when_some(chevron, |this, chevron| this.child(chevron)),
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

//! A settings row: title and subtitle on the left, a trailing control on the right, vertically centred on the text
//! block (macOS `SettingRow`).  Narrow windows stack the control under the text.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{h, v};
use crate::theme::{self, size, text};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Row {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    trailing: Option<AnyElement>,
    indent: bool,
    dimmed: bool,
    on_click: Option<ClickHandler>,
}

/// A row titled `title`; `id` must be unique within the page (the setting key, usually).
pub fn row(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Row {
    Row {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        trailing: None,
        indent: false,
        dimmed: false,
        on_click: None,
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

    /// The control on the right.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// A dependent row: indented, lighter title (macOS `indent: true`).
    pub fn indent(mut self, indent: bool) -> Self {
        self.indent = indent;
        self
    }

    /// Greyed out: its feature is off.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// The whole row is a button (drill-in rows).
    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Row {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let clickable = self.on_click.is_some();
        h().id(self.id)
            .w_full()
            .min_h(size::ROW_MIN)
            .px(size::ROW_PADDING_X)
            .py(size::ROW_PADDING_Y)
            .gap(px(16.))
            .when(self.dimmed, |this| this.opacity(0.45))
            .when(clickable, |this| this.cursor_pointer().hover(|style| style.bg(p.hover)).active(|style| style.bg(p.pressed)))
            .when_some(self.on_click, |this, handler| this.on_click(handler))
            .child(
                v().flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .when(self.indent, |this| this.pl(px(18.)))
                    .child(
                        div()
                            .text_size(text::BODY)
                            .text_color(if self.indent { p.text_secondary } else { p.text })
                            .child(self.title),
                    )
                    .when_some(self.subtitle, |this, subtitle| {
                        this.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(subtitle))
                    }),
            )
            .when_some(self.trailing, |this, trailing| this.child(h().flex_shrink_0().gap(px(8.)).child(trailing)))
    }
}

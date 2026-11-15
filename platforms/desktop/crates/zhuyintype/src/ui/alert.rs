//! Alerts and sheets over the settings window (macOS `NSAlert` sheets and `.alert`): a title, a message, optional
//! content (fields), and buttons on the trailing side, the default one last.  Escape cancels; Return presses the
//! default button.

use std::rc::Rc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::{AnyElement, App, FontWeight, ParentElement, SharedString, Styled, Window, div, px};

use super::actions::{ButtonKind, push_button};
use super::{h, v};
use crate::theme::{self, text};

type Action = Rc<dyn Fn(&mut Window, &mut App)>;
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One button of an alert.
#[derive(Clone)]
pub struct AlertButton {
    label: SharedString,
    kind: ButtonKind,
    action: Option<Action>,
}

impl AlertButton {
    /// A button that closes the alert and then runs `action`.
    pub fn new(label: impl Into<SharedString>, action: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self { label: label.into(), kind: ButtonKind::Normal, action: Some(Rc::new(action)) }
    }

    /// 取消: closes and does nothing.
    pub fn cancel() -> Self {
        Self { label: "取消".into(), kind: ButtonKind::Normal, action: None }
    }

    /// 好: closes and does nothing (an alert that only informs).
    pub fn ok() -> Self {
        Self { label: "好".into(), kind: ButtonKind::Default, action: None }
    }

    /// The default button (accent; Return presses it).
    pub fn default(mut self) -> Self {
        self.kind = ButtonKind::Default;
        self
    }

    /// A destructive action (red label).
    pub fn destructive(mut self) -> Self {
        self.kind = ButtonKind::Destructive;
        self
    }
}

/// What an alert shows.
pub struct Alert {
    title: SharedString,
    message: Option<SharedString>,
    content: Option<Content>,
    buttons: Vec<AlertButton>,
    width: f32,
}

/// An alert titled `title`.
pub fn alert(title: impl Into<SharedString>) -> Alert {
    Alert { title: title.into(), message: None, content: None, buttons: Vec::new(), width: 400. }
}

impl Alert {
    /// The informative text under the title.
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Fields or other content under the message; rebuilt on every frame the alert is open.
    pub fn content(mut self, content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    /// The buttons, leading to trailing (macOS: the default button is the trailing one).
    pub fn buttons(mut self, buttons: impl IntoIterator<Item = AlertButton>) -> Self {
        self.buttons = buttons.into_iter().collect();
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows the alert over `window`.
    pub fn show(self, window: &mut Window, cx: &mut App) {
        let alert = Rc::new(self);
        let default = alert.buttons.iter().rposition(|button| button.kind == ButtonKind::Default);
        window.open_dialog(cx, move |dialog, window, cx| {
            let p = theme::palette(cx);
            let alert = alert.clone();
            let mut body = v().gap(px(6.)).child(
                div().text_size(text::HEADLINE).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(alert.title.clone()),
            );
            if let Some(message) = &alert.message {
                body = body.child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(message.clone()));
            }
            if let Some(content) = &alert.content {
                body = body.child(div().pt(px(8.)).child(content(window, cx)));
            }
            let buttons = h().w_full().justify_end().gap(px(8.)).children(alert.buttons.iter().enumerate().map(|(index, button)| {
                let button = button.clone();
                let label = button.label.clone();
                push_button(
                    SharedString::from(format!("alert-button-{index}")),
                    label,
                    button.kind,
                    true,
                    move |_, window, cx| press(&button, window, cx),
                    cx,
                )
            }));
            let on_ok = default.map(|index| alert.buttons[index].clone());
            dialog
                .w(px(alert.width))
                .close_button(false)
                .overlay_closable(false)
                .on_ok(move |_, window, cx| {
                    if let Some(button) = &on_ok {
                        press(button, window, cx);
                    }
                    false
                })
                .child(body.pb(px(4.)))
                .footer(buttons)
        });
    }
}

fn press(button: &AlertButton, window: &mut Window, cx: &mut App) {
    window.close_dialog(cx);
    if let Some(action) = &button.action {
        action(window, cx);
    }
}

/// An alert that only informs (「無法匯出」).
pub fn notice(title: impl Into<SharedString>, message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
    alert(title).message(message).buttons([AlertButton::ok()]).show(window, cx);
}

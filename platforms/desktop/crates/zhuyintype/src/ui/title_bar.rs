//! The app's own title bar for the settings and wizard windows: a drag area holding the caller's content, then the
//! window controls.  The window has no system caption (Discord, Claude), so the bar moves the window, double-clicking
//! it maximizes, and on Linux a right click opens the window manager's menu.
//!
//! The controls are drawn the way the system's own caption buttons are, not as small icon buttons: each fills the
//! bar's height and is `CONTROL_W` wide, its glyph a 10 px line drawing (`assets/window/*.svg`); hovering fills the
//! button, and the close button turns red with a white glyph.  On Windows each button is a window control area, so
//! the system handles the click (and Snap Layouts on the maximize button); on Linux the clicks call the window.

use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Decorations, Hsla, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Render, RenderOnce, StatefulInteractiveElement as _, Styled, Window, WindowControlArea, div, px, rgb, svg,
};

use super::h;
use crate::theme;

/// A caption button's width (the system's own: 46).
const CONTROL_W: Pixels = px(46.);
/// The glyphs' box.
const GLYPH: Pixels = px(10.);
/// The close button under the pointer (the system's red) and pressed.
const CLOSE_HOVER: u32 = 0xc42b1c;
const CLOSE_PRESSED: u32 = 0xb22a1d;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The title bar, `height` tall on `background`.
#[derive(IntoElement)]
pub struct TitleBar {
    height: Pixels,
    background: Hsla,
    children: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
}

pub fn title_bar(height: Pixels, background: Hsla) -> TitleBar {
    TitleBar { height, background, children: Vec::new(), on_close: None }
}

impl TitleBar {
    /// Content of the drag area (from the left edge; buttons in it stay clickable).
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// What the close button does (default: close the window).  Linux only: on Windows the system closes it, which
    /// the window's own close handling sees.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

/// Whether a press in the bar may start moving the window (Linux: a press then a move drags; a double click
/// maximizes instead).
struct Drag {
    armed: bool,
}

impl Render for Drag {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Control {
    fn id(self) -> &'static str {
        match self {
            Self::Minimize => "window-minimize",
            Self::Maximize => "window-maximize",
            Self::Restore => "window-restore",
            Self::Close => "window-close",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Minimize => "window/minimize.svg",
            Self::Maximize => "window/maximize.svg",
            Self::Restore => "window/restore.svg",
            Self::Close => "window/close.svg",
        }
    }

    fn area(self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Maximize | Self::Restore => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }
}

fn control(kind: Control, on_close: Option<CloseHandler>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let close = kind == Control::Close;
    let group = kind.id();
    let (hover_bg, pressed_bg, hover_ink) = if close {
        (Hsla::from(rgb(CLOSE_HOVER)), Hsla::from(rgb(CLOSE_PRESSED)), gpui_kit::white())
    } else {
        (p.subtle, p.pressed, p.text)
    };
    div()
        .id(kind.id())
        .group(group)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(CONTROL_W)
        .h_full()
        .hover(move |style| style.bg(hover_bg))
        .active(move |style| style.bg(pressed_bg))
        .when(cfg!(target_os = "windows"), |this| this.window_control_area(kind.area()))
        .when(cfg!(target_os = "linux"), |this| {
            this.on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                match kind {
                    Control::Minimize => window.minimize_window(),
                    Control::Maximize | Control::Restore => window.zoom_window(),
                    Control::Close => match &on_close {
                        Some(handler) => handler(window, cx),
                        None => window.remove_window(),
                    },
                }
            })
        })
        .child(
            svg()
                .path(kind.glyph())
                .size(GLYPH)
                .flex_shrink_0()
                .text_color(p.text_secondary)
                .group_hover(group, move |style| style.text_color(hover_ink)),
        )
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let linux = cfg!(target_os = "linux");
        let client = matches!(window.window_decorations(), Decorations::Client { .. });
        // Under the window manager's own decorations (X11 without a compositor) it draws the controls.
        let controls = !cfg!(target_os = "macos") && (!linux || client);
        let supported = window.window_controls();
        let radii = gpui_kit::component::window_corner_radii(window, cx);
        let drag = window.use_state(cx, |_, _| Drag { armed: false });
        let on_close = self.on_close.clone();
        h().id("title-bar")
            .flex_shrink_0()
            .w_full()
            .h(self.height)
            .rounded_tl(radii.top_left)
            .rounded_tr(radii.top_right)
            .bg(self.background)
            .child(
                h().id("title-bar-drag")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .when(linux, |this| {
                        this.on_mouse_down(
                            MouseButton::Left,
                            window.listener_for(&drag, |drag, event: &MouseDownEvent, window, _| {
                                drag.armed = event.click_count < 2;
                                if event.click_count == 2 {
                                    window.zoom_window();
                                }
                            }),
                        )
                        .on_mouse_up(MouseButton::Left, window.listener_for(&drag, |drag, _, _, _| drag.armed = false))
                        .on_mouse_move(window.listener_for(&drag, |drag, _, window, _| {
                            if drag.armed {
                                drag.armed = false;
                                window.start_window_move();
                            }
                        }))
                        .when(client, |this| {
                            this.on_mouse_down(MouseButton::Right, |event, window, _| {
                                window.show_window_menu(event.position)
                            })
                        })
                    })
                    .children(self.children),
            )
            .when(controls, |this| {
                this.child(
                    h().h_full()
                        .flex_shrink_0()
                        .when(supported.minimize, |this| this.child(control(Control::Minimize, None, cx)))
                        .when(supported.maximize, |this| {
                            let kind = if window.is_maximized() { Control::Restore } else { Control::Maximize };
                            this.child(control(kind, None, cx))
                        })
                        .child(control(Control::Close, on_close, cx)),
                )
            })
    }
}

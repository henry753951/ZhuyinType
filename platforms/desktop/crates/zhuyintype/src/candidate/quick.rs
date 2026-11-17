//! 快速設定: the small panel the 開啟快速設定 shortcut opens by the caret (`UiEvent::QuickPanel`), with the same items as
//! the input method's menu (IPC.md §快速設定): 英文, 半形標點, 簡體 and 暫停學習 as check items, then 設定… and
//! 自訂詞與學到的詞….  Like the candidate window it is a floating window that never takes focus: the keys stay with
//! the app, and the next one closes it (`UiEvent::HideQuickPanel`), as does a click in it.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, FontWeight, Global, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, Styled, Window, WindowHandle, div, point, px, size,
};
use zt_core::presentation::Rect;
use zt_core::service::{QuickSwitch, QuickSwitches};

use super::panel::{Chrome, SHADOW_MARGIN, edge, shell};
use super::platform;
use super::style::PanelAppearance;
use crate::app;
use crate::theme::Palette;
use crate::theme::candidate as tokens;

/// The panel's width and row height, in logical pixels.
pub const WIDTH: f32 = 208.;
const ROW: f32 = 28.;
const TITLE: f32 = 26.;
const PADDING: f32 = 5.;

/// What a click in the panel does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickItem {
    Switch(QuickSwitch),
    Settings,
    Words,
}

const SWITCHES: [(QuickSwitch, &str); 4] = [
    (QuickSwitch::English, "英文"),
    (QuickSwitch::HalfWidth, "半形標點"),
    (QuickSwitch::Simplified, "簡體"),
    (QuickSwitch::PauseLearning, "暫停學習"),
];

fn checked(switches: QuickSwitches, switch: QuickSwitch) -> bool {
    match switch {
        QuickSwitch::English => switches.english,
        QuickSwitch::HalfWidth => !switches.full_width,
        QuickSwitch::Simplified => switches.simplified,
        QuickSwitch::PauseLearning => switches.learning_paused,
    }
}

/// The panel's height: the title, four check items, a separator and two links.
#[must_use]
pub fn height() -> f32 {
    PADDING * 2. + TITLE + ROW * 6. + 9.
}

/// What a click does (`None`: a picture, as in screenshots).
pub type OnItem = std::rc::Rc<dyn Fn(QuickItem, &mut Window, &mut App)>;

/// The panel's surface with `switches` checked.
pub fn quick_panel(switches: QuickSwitches, p: &Palette, chrome: Chrome, on_item: Option<OnItem>) -> AnyElement {
    let c = tokens::colors(p);
    let hover = c.hover;
    let row = |item: QuickItem, label: &'static str, check: Option<bool>| {
        let on_item = on_item.clone();
        div()
            .id(SharedString::from(format!("quick-{label}")))
            .h(px(ROW))
            .mx(px(PADDING))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(6.))
            .text_size(px(14.))
            .text_color(p.text)
            .child(
                div()
                    .w(px(16.))
                    .flex()
                    .justify_center()
                    .when(check == Some(true), |this| this.child(Icon::new(IconName::Check).size(px(14.)))),
            )
            .child(SharedString::from(label))
            .when_some(on_item, |this, on_item| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| on_item(item, window, cx))
            })
    };
    let mut body = div().flex().flex_col().py(px(PADDING)).child(
        div()
            .h(px(TITLE))
            .px(px(PADDING + 8.))
            .flex()
            .items_center()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_tertiary)
            .child("快速設定"),
    );
    for (switch, label) in SWITCHES {
        body = body.child(row(QuickItem::Switch(switch), label, Some(checked(switches, switch))));
    }
    body = body
        .child(div().my(px(4.)).mx(px(PADDING + 8.)).h(px(1.)).bg(c.hairline))
        .child(row(QuickItem::Settings, "設定…", None))
        .child(row(QuickItem::Words, "自訂詞與學到的詞…", None));
    shell(chrome, &c, tokens::RADIUS)
        .w(px(WIDTH))
        .child(body)
        .children(edge(chrome, &c, tokens::RADIUS))
        .into_any_element()
}

// MARK: - The floating window

pub struct QuickWindow {
    switches: QuickSwitches,
    margin: f32,
    appearance: PanelAppearance,
}

struct QuickPopup {
    window: Option<WindowHandle<QuickWindow>>,
}

impl Global for QuickPopup {}

impl Render for QuickWindow {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.appearance.palette(crate::theme::system_dark(window));
        let on_item: OnItem = std::rc::Rc::new(|item, _, cx| {
            // After the event: the switch may update other windows (the badge), which cannot happen inside this one.
            cx.defer(move |cx| choose(item, cx));
        });
        let chrome = super::popup::popup_chrome(self.margin);
        div().size_full().font(crate::theme::font()).p(px(self.margin)).child(quick_panel(
            self.switches,
            &palette,
            chrome,
            Some(on_item),
        ))
    }
}

fn choose(item: QuickItem, cx: &mut App) {
    hide(cx);
    let Some(service) = app::service(cx).cloned() else { return };
    service.quick_panel_closed();
    match item {
        QuickItem::Switch(switch) => service.toggle_quick(switch),
        QuickItem::Settings => super::popup::handle(zt_core::service::UiEvent::ShowSettings(None), cx),
        QuickItem::Words => super::popup::handle(zt_core::service::UiEvent::ShowSettings(Some("words")), cx),
    }
}

/// Shows the panel by `anchor` (the caret, logical pixels) with the switches as they are now.
pub fn show(anchor: Rect, cx: &mut App) {
    let unknown = anchor.x == 0. && anchor.y == 0. && anchor.width == 0. && anchor.height == 0.;
    let Some(service) = app::service(cx).cloned() else { return };
    if unknown {
        return;
    }
    let switches = service.quick_switches();
    let settings = app::settings(cx).clone();
    let appearance = PanelAppearance::from_settings(&settings);
    let above = settings.string("candidatePlacement") == "above";
    let transparent = super::popup::transparent(cx);
    let margin = if transparent { SHADOW_MARGIN } else { 0. };
    let screen = super::popup::screen_at(anchor, cx);
    let (w, h) = (WIDTH, height());
    let (x, y) = zt_core::presentation::popup_position(anchor, f64::from(w), f64::from(h), above, 0., screen);
    let frame = (x as f32 - margin, y as f32 - margin, w + margin * 2., h + margin * 2.);
    if let Some(handle) = existing(cx) {
        let placed = handle
            .update(cx, |view, window, cx| {
                view.switches = switches;
                view.appearance = appearance;
                let scale = window.scale_factor();
                let placed = platform::place(window, super::popup::device(frame, scale));
                if placed {
                    platform::set_input_region(window, Some(super::popup::device((margin, margin, w, h), scale)));
                    platform::set_visible(window, true);
                }
                cx.notify();
                placed
            })
            .unwrap_or(false);
        if placed {
            return;
        }
        // This platform cannot move a window: open a new one where it goes.
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    let bounds = Bounds::new(point(px(frame.0), px(frame.1)), size(px(frame.2), px(frame.3)));
    let opened = cx.open_window(super::popup::options(bounds, transparent), |window, cx| {
        platform::make_non_activating(window);
        let scale = window.scale_factor();
        platform::set_input_region(window, Some(super::popup::device((margin, margin, w, h), scale)));
        cx.new(|_| QuickWindow { switches, margin, appearance })
    });
    match opened {
        Ok(handle) => cx.set_global(QuickPopup { window: Some(handle) }),
        Err(error) => log::warn!("opening 快速設定: {error:#}"),
    }
}

fn existing(cx: &App) -> Option<WindowHandle<QuickWindow>> {
    cx.try_global::<QuickPopup>().and_then(|popup| popup.window)
}

/// Hides the panel (the next key, a click in it).
pub fn hide(cx: &mut App) {
    if let Some(handle) = existing(cx) {
        let _ = handle.update(cx, |_, window, _| {
            if !platform::set_visible(window, false) {
                let scale = window.scale_factor();
                platform::place(window, super::popup::device((-10000., -10000., 8., 8.), scale));
            }
        });
    }
}

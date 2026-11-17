//! The floating windows: the candidate window and the 中/英 badge (macOS `CandidatePanelController`, `ModeToast`).
//!
//! Each is one borderless GPUI `WindowKind::PopUp` window, opened once and kept: shown, moved and hidden as the
//! service's `UiEvent`s arrive, never activated and never given focus (see `platform`).  The window is the surface
//! plus a transparent margin for its shadow when the desktop composites (the margin lets clicks through); without a
//! compositor it is the surface alone, square.
//!
//! Placement: below the caret's line (or above it, 「位置」) with a gap of 4, the candidates' text lined up with the
//! composition, flipped to the other side when the screen has no room and always inside the visible area of the
//! caret's screen (`zt_core::presentation::popup_position`).  While the caret stays on the same line the window keeps
//! its x, so it does not wander as the composition grows.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    App, AppContext as _, Bounds, Context, Global, IntoElement, ParentElement, Pixels, Render, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowHandle, WindowKind, WindowOptions, div, point,
    px, size,
};
use zt_core::presentation::{Rect, popup_position};
use zt_core::service::{PopupAction, PopupState, UiEvent};

use super::layout::{BADGE_H, BADGE_W, Extras, WindowMeasure};
use super::panel::{ActionHandler, CandidatePanel, Chrome, SHADOW_MARGIN, ease_out, mode_badge_in, motion};
use super::platform::{self, DeviceRect};
use super::style::{self, CurrentKeys, PanelAppearance};
use crate::app;

/// Where clicks in the candidate window go: the service, or (screenshots, demos) a stand-in.
pub type Sink = Rc<dyn Fn(PopupAction, &mut App)>;

/// The floating windows, created on first use.
struct Popups {
    candidate: Option<WindowHandle<CandidateWindow>>,
    toast: Option<WindowHandle<ToastWindow>>,
    toast_generation: u64,
    sink: Option<Sink>,
    /// Transparent windows show what is behind them (decided once).
    transparent: bool,
}

impl Global for Popups {}

fn popups(cx: &mut App) -> &mut Popups {
    if !cx.has_global::<Popups>() {
        let transparent = platform::transparent_windows();
        cx.set_global(Popups { candidate: None, toast: None, toast_generation: 0, sink: None, transparent });
    }
    cx.global_mut::<Popups>()
}

/// Drains the service's events on the foreground executor for as long as the app runs.  Call once.
#[allow(dead_code, reason = "main.rs calls it once Service::start is ported")]
pub fn run(receiver: async_channel::Receiver<UiEvent>, cx: &mut App) {
    cx.spawn(async move |cx| {
        while let Ok(event) = receiver.recv().await {
            cx.update(|cx| {
                let event = in_logical_pixels(event, cx);
                handle(event, cx);
            });
        }
    })
    .detach();
}

/// The service's caret comes from the Fcitx5 addon in the X server's pixels (Fcitx knows no other), which are GPUI's
/// logical pixels times its scale (Xft.dpi, GPUI_X11_SCALE_FACTOR): on a HiDPI X11 desktop the window would land at
/// twice the caret's place.  The Windows text service sends logical pixels already.
fn in_logical_pixels(event: UiEvent, cx: &mut App) -> UiEvent {
    let caret = match &event {
        UiEvent::Popup(Some(state)) => state.caret,
        UiEvent::Toast { anchor, .. } => *anchor,
        _ => return event,
    };
    // GPUI's X11 scale is one for every window: the candidate window's (opened once and kept anyway).
    let scale = candidate_window(cx)
        .and_then(|handle| {
            handle.update(cx, |_, window, _| platform::x11_pixels(window).then(|| window.scale_factor())).ok()
        })
        .flatten();
    let Some(scale) = scale.filter(|scale| *scale > 0. && (*scale - 1.).abs() > f32::EPSILON) else { return event };
    let scale = f64::from(scale);
    let logical = Rect::new(caret.x / scale, caret.y / scale, caret.width / scale, caret.height / scale);
    match event {
        UiEvent::Popup(Some(state)) => UiEvent::Popup(Some(PopupState { caret: logical, ..state })),
        UiEvent::Toast { text, .. } => UiEvent::Toast { text, anchor: logical },
        other => other,
    }
}

/// Opens the floating windows (hidden) and shapes some CJK text, which loads the system's fallback fonts, soon after
/// start rather than at the first key: otherwise the session's first candidate window waits ~100 ms and its first
/// badge ~55 ms.  For the copy that serves the input method on Linux (on Windows the text service draws both), on X11
/// only: in a Wayland session the Fcitx5 addon has Fcitx show the list and the badge (IPC.md, `panel`), and GPUI's
/// floating windows there are toplevels this app can neither hide nor move off screen, so opening them early would
/// only put two empty windows on the desktop.
#[cfg(target_os = "linux")]
pub fn prewarm(cx: &mut App) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty()) {
        return;
    }
    cx.spawn(async move |cx| cx.update(open_hidden)).detach();
}

#[cfg(target_os = "linux")]
fn open_hidden(cx: &mut App) {
    if let Some(handle) = candidate_window(cx) {
        let _ = handle.update(cx, |_, window, _| {
            let mut measure = WindowMeasure(window);
            for medium in [false, true] {
                super::layout::Measure::width(&mut measure, "候選字注音ㄓㄨˋㄧㄣ 中英 1234 Shift", 16., medium);
            }
        });
    }
    toast_window(cx);
}

/// Sends clicks somewhere else than the service (demos and screenshots).
pub fn set_sink(sink: Sink, cx: &mut App) {
    popups(cx).sink = Some(sink);
}

/// Handles one event from the service.
pub fn handle(event: UiEvent, cx: &mut App) {
    match event {
        UiEvent::Popup(Some(state)) => show_candidates(state, cx),
        UiEvent::Popup(None) => hide_candidates(cx),
        UiEvent::Toast { text, anchor } => show_toast(text, anchor, cx),
        UiEvent::Boundary(direction) => {
            if let Some(handle) = popups(cx).candidate {
                let _ = handle.update(cx, |view, _, cx| {
                    view.panel.bounce(direction);
                    cx.notify();
                });
            }
        }
        UiEvent::ShowSettings(place) => show_settings(place, cx),
        UiEvent::QuickPanel { .. } => {}
    }
}

/// Another start of the app asked for the settings: bring the open settings window forward, or open one.
/// The settings window, or the place the input method's menu asked for (IPC `show-settings`).
fn show_settings(place: Option<&'static str>, cx: &mut App) {
    use crate::app::{self, Place};
    use crate::pages::Page;
    use crate::wizard::{self, WizardKind};
    match place {
        Some("wizard") => wizard::open(WizardKind::Quick, cx),
        Some("tour") => match wizard::new_lessons_since(cx) {
            Some(since) => wizard::open(WizardKind::NewSince(since), cx),
            None => wizard::open(WizardKind::Quick, cx),
        },
        Some(page) => {
            crate::shell::show(cx);
            app::go(Place::page(if page == "words" { Page::Words } else { Page::About }), cx);
        }
        None => crate::shell::show(cx),
    }
}

/// The visible area of the screen with `caret` on it (logical pixels).
fn screen_at(caret: Rect, cx: &App) -> Rect {
    let to_rect = |bounds: Bounds<Pixels>| {
        Rect::new(
            f64::from(f32::from(bounds.origin.x)),
            f64::from(f32::from(bounds.origin.y)),
            f64::from(f32::from(bounds.size.width)),
            f64::from(f32::from(bounds.size.height)),
        )
    };
    let (cx_, cy) = (caret.x + caret.width / 2., caret.y + caret.height / 2.);
    let displays = cx.displays();
    displays
        .iter()
        .map(|display| (to_rect(display.bounds()), to_rect(display.visible_bounds())))
        .find(|(full, _)| cx_ >= full.x && cx_ < full.right() && cy >= full.y && cy < full.bottom())
        .map(|(_, visible)| visible)
        .or_else(|| cx.primary_display().map(|display| to_rect(display.visible_bounds())))
        .unwrap_or(Rect::new(0., 0., 1440., 900.))
}

fn options(bounds: Bounds<Pixels>, transparent: bool) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: if transparent {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        },
        window_decorations: Some(WindowDecorations::Client),
        // The popups are never active; without this GPUI would draw their motion at 30 fps.
        inactive_frame_interval: None,
        app_id: Some("zhuyintype".into()),
        ..WindowOptions::default()
    }
}

/// Far outside every screen: where a window waits before its first show on platforms that cannot hide it.
const OFFSCREEN: f32 = -10000.;

fn device(rect: (f32, f32, f32, f32), scale: f32) -> DeviceRect {
    DeviceRect {
        x: (rect.0 * scale).round() as i32,
        y: (rect.1 * scale).round() as i32,
        width: (rect.2 * scale).round().max(1.) as u32,
        height: (rect.3 * scale).round().max(1.) as u32,
    }
}

/// The floating windows' edge: their own (rounded, shadowed) with a transparent margin around them, the system's
/// (Windows), or square with a hairline (no compositor).
fn popup_chrome(margin: f32) -> Chrome {
    if platform::system_frames_popups() {
        Chrome::System
    } else if margin > 0. {
        Chrome::Floating
    } else {
        Chrome::Square
    }
}

// MARK: - The candidate window

pub struct CandidateWindow {
    panel: CandidatePanel,
    margin: f32,
    visible: bool,
    /// Where the window is now (logical: x, y, width, height), to keep x on the same line and skip no-op moves.
    frame: Option<(f32, f32, f32, f32)>,
    last_caret: Option<Rect>,
    /// The window appearing: when, and how far it starts from its place.
    appeared: Option<(Instant, f32)>,
    appearance: PanelAppearance,
}

impl CandidateWindow {
    fn new(margin: f32) -> Self {
        let mut panel = CandidatePanel::new();
        panel.chrome = popup_chrome(margin);
        Self {
            panel,
            margin,
            visible: false,
            frame: None,
            last_caret: None,
            appeared: None,
            appearance: PanelAppearance::System,
        }
    }
}

impl Render for CandidateWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let system_dark = crate::theme::system_dark(window);
        let palette = self.appearance.palette(system_dark);
        // After the event: the answer may update this window again, which cannot happen inside its own update.
        let actions: ActionHandler = Rc::new(|action, _, cx| {
            cx.defer(move |cx| {
                let sink = popups(cx).sink.clone();
                match sink {
                    Some(sink) => sink(action, cx),
                    None => {
                        if let Some(service) = app::service(cx) {
                            service.popup_action(action);
                        }
                    }
                }
            });
        });
        let (mut alpha, mut dy) = (1., 0.);
        if let Some((started, from)) = self.appeared {
            let t = started.elapsed().as_secs_f32() * 1000. / motion::APPEAR;
            if t < 1. {
                alpha = ease_out(t);
                dy = from * (1. - ease_out(t));
                window.request_animation_frame();
            } else {
                self.appeared = None;
            }
        }
        let _ = cx;
        div().size_full().font(crate::theme::font()).p(px(self.margin)).child(
            div().relative().top(px(dy)).opacity(alpha).child(self.panel.render(&palette, Some(actions), window)),
        )
    }
}

fn candidate_window(cx: &mut App) -> Option<WindowHandle<CandidateWindow>> {
    if let Some(handle) = popups(cx).candidate {
        return Some(handle);
    }
    let transparent = popups(cx).transparent;
    let margin = if transparent { SHADOW_MARGIN } else { 0. };
    let bounds = Bounds::new(point(px(OFFSCREEN), px(OFFSCREEN)), size(px(8.), px(8.)));
    let handle = cx
        .open_window(options(bounds, transparent), |window, cx| {
            platform::make_non_activating(window);
            platform::set_visible(window, false);
            cx.new(|_| CandidateWindow::new(margin))
        })
        .map_err(|error| log::warn!("opening the candidate window: {error:#}"))
        .ok()?;
    popups(cx).candidate = Some(handle);
    Some(handle)
}

fn show_candidates(state: PopupState, cx: &mut App) {
    let Some(handle) = candidate_window(cx) else { return };
    let screen = screen_at(state.caret, cx);
    let settings = app::settings(cx).clone();
    let keys = CurrentKeys::new(&settings, app::service(cx));
    let style = style::style_from_settings(&settings, &keys);
    let animates = settings.bool("panelAnimation");
    let moves = crate::motion::Panel::new(animates, cx);
    let above = settings.string("candidatePlacement") == "above";
    let appearance = PanelAppearance::from_settings(&settings);
    let reopen = handle
        .update(cx, |view, window, cx| {
            let p = &state.presentation;
            let preview = (!state.inline_preedit).then_some(p.composition.as_str());
            if !p.has_list() && preview.is_none_or(str::is_empty) && p.notice.is_none() {
                hide(view, window);
                cx.notify();
                return None;
            }
            view.appearance = appearance;
            view.panel.animates = animates;
            view.panel.reduced = moves.reduced;
            let margin = view.margin;
            let max_width = (screen.width as f32 - margin * 2.).max(100.);
            let (width, height) =
                view.panel.update(p, Extras { preview }, &style, max_width, view.visible, &mut WindowMeasure(window));
            let text_inset = view.panel.layout().map_or(14., |layout| layout.text_inset);
            let caret = state.caret;
            let (mut x, y) =
                popup_position(caret, f64::from(width), f64::from(height), above, f64::from(text_inset), screen);
            let same_line = view.visible
                && view
                    .last_caret
                    .is_some_and(|last| (last.y - caret.y).abs() < 2. && (last.bottom() - caret.bottom()).abs() < 2.);
            if same_line && let Some((previous_x, ..)) = view.frame {
                let right = screen.right() - f64::from(width);
                x = (f64::from(previous_x + margin)).clamp(screen.x, screen.x.max(right));
            }
            let frame = (x as f32 - margin, y as f32 - margin, width + margin * 2., height + margin * 2.);
            let appearing = !view.visible;
            let scale = window.scale_factor();
            let moved = view.frame != Some(frame);
            let mut reopen = false;
            if moved {
                if platform::place(window, device(frame, scale)) {
                    platform::set_input_region(window, Some(device((margin, margin, width, height), scale)));
                } else if view.frame.is_some_and(|(fx, fy, ..)| (fx, fy) == (frame.0, frame.1)) {
                    window.resize(size(px(frame.2), px(frame.3)));
                } else {
                    reopen = true;
                }
            }
            if appearing {
                platform::set_visible(window, true);
                if animates {
                    // A few pixels away from the caret, towards its place (only a fade under reduced motion).
                    let below = y + f64::from(height) / 2. > caret.y + caret.height / 2.;
                    let slide = if !moves.slides() {
                        0.
                    } else if below {
                        -motion::APPEAR_SLIDE
                    } else {
                        motion::APPEAR_SLIDE
                    };
                    view.appeared = Some((Instant::now(), slide));
                }
            } else {
                // Still appearing when the next key arrives: finish at once, so the window shows every key in full
                // (macOS `PanelMotion.present`).
                view.appeared = None;
            }
            view.frame = Some(frame);
            view.last_caret = Some(caret);
            view.visible = true;
            cx.notify();
            reopen.then_some(frame)
        })
        .ok()
        .flatten();
    if let Some(frame) = reopen {
        // This platform cannot move a window: open it where it goes (the state is drawn again into the new one).
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        let transparent = popups(cx).transparent;
        let margin = if transparent { SHADOW_MARGIN } else { 0. };
        let bounds = Bounds::new(point(px(frame.0), px(frame.1)), size(px(frame.2), px(frame.3)));
        match cx.open_window(options(bounds, transparent), |window, cx| {
            platform::make_non_activating(window);
            cx.new(|_| CandidateWindow::new(margin))
        }) {
            Ok(new) => {
                popups(cx).candidate = Some(new);
                let _ = new.update(cx, |view, _, _| view.frame = Some(frame));
                show_candidates(state, cx);
            }
            Err(error) => {
                popups(cx).candidate = None;
                log::warn!("reopening the candidate window: {error:#}");
            }
        }
    }
}

fn hide(view: &mut CandidateWindow, window: &mut Window) {
    if view.visible && !platform::set_visible(window, false) {
        let scale = window.scale_factor();
        let (w, h) = view.frame.map_or((8., 8.), |frame| (frame.2, frame.3));
        if platform::place(window, device((OFFSCREEN, OFFSCREEN, w, h), scale)) {
            view.frame = None;
        }
    }
    view.visible = false;
    view.last_caret = None;
    view.appeared = None;
    view.panel.reset();
}

fn hide_candidates(cx: &mut App) {
    if let Some(handle) = popups(cx).candidate {
        let _ = handle.update(cx, |view, window, cx| {
            hide(view, window);
            cx.notify();
        });
    }
}

// MARK: - The 中/英 badge

pub struct ToastWindow {
    text: &'static str,
    margin: f32,
    shown_at: Option<Instant>,
    /// Appearing: when, and how far it starts from its place (towards the caret).
    appeared: Option<(Instant, f32)>,
    animates: bool,
    appearance: PanelAppearance,
    visible: bool,
}

impl Render for ToastWindow {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let system_dark = crate::theme::system_dark(window);
        let palette = self.appearance.palette(system_dark);
        let (mut alpha, mut dy) = (1., 0.);
        if let Some((started, from)) = self.appeared {
            let t = started.elapsed().as_secs_f32() * 1000. / crate::motion::panel::BADGE_APPEAR;
            if t < 1. {
                let from_alpha = crate::motion::panel::BADGE_FROM;
                alpha = from_alpha + (1. - from_alpha) * ease_out(t);
                dy = from * (1. - ease_out(t));
                window.request_animation_frame();
            } else {
                self.appeared = None;
            }
        }
        if let Some(shown) = self.shown_at
            && self.animates
        {
            let elapsed = shown.elapsed().as_secs_f32() * 1000.;
            if elapsed >= motion::TOAST_HOLD {
                alpha *= (1. - (elapsed - motion::TOAST_HOLD) / motion::TOAST_FADE).max(0.);
                if alpha > 0. {
                    window.request_animation_frame();
                }
            }
        }
        div().size_full().font(crate::theme::font()).p(px(self.margin)).child(
            div().relative().top(px(dy)).opacity(alpha).child(mode_badge_in(
                self.text,
                &palette,
                popup_chrome(self.margin),
            )),
        )
    }
}

/// The badge's window, opened (hidden, off screen) on first use.
fn toast_window(cx: &mut App) -> Option<WindowHandle<ToastWindow>> {
    if let Some(handle) = popups(cx).toast {
        return Some(handle);
    }
    let transparent = popups(cx).transparent;
    let margin = if transparent { SHADOW_MARGIN } else { 0. };
    let bounds =
        Bounds::new(point(px(OFFSCREEN), px(OFFSCREEN)), size(px(BADGE_W + margin * 2.), px(BADGE_H + margin * 2.)));
    let opened = cx.open_window(options(bounds, transparent), |window, cx| {
        platform::make_non_activating(window);
        platform::set_input_region(window, None);
        platform::set_visible(window, false);
        cx.new(|_| ToastWindow {
            text: "中",
            margin,
            shown_at: None,
            appeared: None,
            animates: true,
            appearance: PanelAppearance::System,
            visible: false,
        })
    });
    match opened {
        Ok(handle) => {
            popups(cx).toast = Some(handle);
            Some(handle)
        }
        Err(error) => {
            log::warn!("opening the 中/英 badge: {error:#}");
            None
        }
    }
}

fn show_toast(text: &'static str, anchor: Rect, cx: &mut App) {
    let settings = app::settings(cx).clone();
    // No caret reported (nothing to type into): no badge, rather than one in a corner of the screen.
    let unknown = anchor.x == 0. && anchor.y == 0. && anchor.width == 0. && anchor.height == 0.;
    if !settings.bool("showModeToast") || unknown {
        return;
    }
    let Some(handle) = toast_window(cx) else { return };
    let margin = if popups(cx).transparent { SHADOW_MARGIN } else { 0. };
    let screen = screen_at(anchor, cx);
    let above = settings.string("candidatePlacement") == "above";
    let animates = settings.bool("panelAnimation");
    let moves = crate::motion::Panel::new(animates, cx);
    let appearance = PanelAppearance::from_settings(&settings);
    let generation = {
        let popups = popups(cx);
        popups.toast_generation += 1;
        popups.toast_generation
    };
    let _ = handle.update(cx, |view, window, cx| {
        let (x, y) = popup_position(anchor, f64::from(BADGE_W), f64::from(BADGE_H), above, 0., screen);
        let frame = (x as f32 - margin, y as f32 - margin, BADGE_W + margin * 2., BADGE_H + margin * 2.);
        platform::place(window, device(frame, window.scale_factor()));
        platform::set_visible(window, true);
        platform::set_input_region(window, None);
        // Appearing (not when it is already up: a second toggle only changes the text and restarts the hold).
        if !view.visible && animates {
            let below = y + f64::from(BADGE_H) / 2. > anchor.y + anchor.height / 2.;
            let slide = crate::motion::panel::BADGE_SLIDE;
            view.appeared = Some((
                Instant::now(),
                if !moves.slides() {
                    0.
                } else if below {
                    -slide
                } else {
                    slide
                },
            ));
        } else if !animates {
            view.appeared = None;
        }
        view.text = text;
        view.shown_at = Some(Instant::now());
        view.animates = animates;
        view.appearance = appearance;
        view.visible = true;
        cx.notify();
    });
    // Shown for 0.8 s, then faded over 0.2 s (or removed at once at 1 s with animation off).
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(motion::TOAST_HOLD as u64)).await;
        cx.update(|cx| {
            if popups(cx).toast_generation == generation {
                let _ = handle.update(cx, |_, _, cx| cx.notify());
            }
        });
        cx.background_executor().timer(Duration::from_millis(motion::TOAST_FADE as u64)).await;
        cx.update(|cx| {
            if popups(cx).toast_generation == generation {
                let _ = handle.update(cx, |view, window, cx| {
                    view.visible = false;
                    view.shown_at = None;
                    if !platform::set_visible(window, false) {
                        platform::place(window, device((OFFSCREEN, OFFSCREEN, 8., 8.), window.scale_factor()));
                    }
                    cx.notify();
                });
            }
        });
    })
    .detach();
}

/// Hides the badge at once (the app deactivated).
#[allow(dead_code, reason = "for the service's deactivation once ported")]
pub fn hide_toast(cx: &mut App) {
    let popups = popups(cx);
    popups.toast_generation += 1;
    if let Some(handle) = popups.toast {
        let _ = handle.update(cx, |view, window, cx| {
            view.visible = false;
            view.shown_at = None;
            platform::set_visible(window, false);
            cx.notify();
        });
    }
}

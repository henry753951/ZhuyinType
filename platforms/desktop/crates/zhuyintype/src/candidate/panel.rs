//! The candidate window's content (macOS `CandidateContentView` on its `InputSurface`): the notice, the composition
//! when the app cannot show it, the reading row, the rows, and the footer with ⇧↓ 單字, ⇧↑ 修正 and the pager.  The same
//! element draws the floating window, the 「視窗與提示」 preview and every other place that shows the window.
//!
//! The panel keeps what it showed last, so a new page of the same list is a page turn (the old rows move out against
//! the direction and fade, the new ones come in from the other side, the highlight stays put; macOS `PageMotion`), and
//! a page turn that stopped at an end of the list nudges the page (the bounce).  The rows are always the new ones at
//! once: keys never wait for motion.

use std::rc::Rc;
use std::time::Instant;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, BoxShadow, Div, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Window, div, point, px,
};
use zt_core::presentation::Presentation;
use zt_core::service::PopupAction;

use super::layout::{
    self, BADGE_FONT, BADGE_H, BADGE_W, Extras, Frame, HintLayout, KEYCAP_HEIGHT, Measure, NoticeLayout, PanelLayout,
    RowLayout, SMALL_FONT, ToggleLayout,
};
use super::style::PanelStyle;
use crate::theme::{self, Palette, size, text};

/// A click in the window.
pub type ActionHandler = Rc<dyn Fn(PopupAction, &mut Window, &mut App)>;

/// How a page change and the end of the list move (macOS `PageMotion`): short, eased out, a few pixels along the list.
pub mod motion {
    pub const DURATION: f32 = crate::theme::motion::PAGE_TURN.as_millis() as f32;
    /// The old rows are gone after the first 35 % and the new ones start at 30 %, so two pages of text are never
    /// legible at once.
    pub const OUT_DURATION: f32 = 50.;
    pub const IN_DELAY: f32 = 42.;
    /// How far the pages travel.
    pub const OFFSET: f32 = 10.;
    /// The nudge at the first or last page.
    pub const BOUNCE: f32 = 3.;
    pub const BOUNCE_OUT: f32 = 60.;
    pub const BOUNCE_BACK: f32 = 120.;
    /// The window appearing: a fade and a few pixels towards its place.
    pub const APPEAR: f32 = 120.;
    pub const APPEAR_SLIDE: f32 = 4.;
    /// The 中/英 badge stays, then fades (docs/product/DESIGN.md §4.2).
    pub const TOAST_HOLD: f32 = 800.;
    pub const TOAST_FADE: f32 = 200.;
}

/// Ease-out (CoreAnimation's `easeOut` drawn as macOS's pinned frames draw it).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t) * (1. - t)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.5 { 2. * t * t } else { 1. - (-2. * t + 2.).powi(2) / 2. }
}

/// What makes two lists the same list, so that a new page of it is a page change (and not a new list).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    count: usize,
    page_size: usize,
    tolerant: Option<bool>,
    single: bool,
    reading: Option<String>,
    focused: Option<usize>,
}

impl Identity {
    fn of(p: &Presentation) -> Option<Self> {
        p.has_list().then(|| Self {
            count: p.count,
            page_size: p.page_size,
            tolerant: p.tolerant,
            single: p.single,
            reading: p.reading.clone(),
            focused: p.focused_clause,
        })
    }
}

/// A motion frozen at `progress` (0…1), for screenshots of the paging frames (macOS `pinnedMotion`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pinned {
    Page { direction: i32, progress: f32 },
    Bounce { direction: i32, progress: f32 },
}

#[derive(Clone, Copy, Debug)]
struct Turn {
    direction: i32,
    started: Instant,
}

/// The candidate window's content and what it showed last.
pub struct CandidatePanel {
    shown: Option<(Identity, usize)>,
    shown_style: Option<PanelStyle>,
    steady: (f32, f32),
    layout: Option<PanelLayout>,
    /// The page moving out during a turn.
    outgoing: Option<Vec<RowLayout>>,
    /// The page label before the turn, fading out.
    old_label: Option<String>,
    turn: Option<Turn>,
    bounce: Option<Turn>,
    /// Rows show the pointer (false for illustrations).
    pub hover: bool,
    /// The 動畫 setting (the wizard's practice field follows its own draft settings).
    pub animates: bool,
    /// The surface draws its own shadow (inside another window, or a floating window with room around it).
    pub shadow: bool,
    /// Rounded corners (a floating window without a compositor is square: its corners would show black).
    pub rounded: bool,
    pub pinned: Option<Pinned>,
}

impl Default for CandidatePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl CandidatePanel {
    pub fn new() -> Self {
        Self {
            shown: None,
            shown_style: None,
            steady: (0., 0.),
            layout: None,
            outgoing: None,
            old_label: None,
            turn: None,
            bounce: None,
            hover: true,
            animates: true,
            shadow: true,
            rounded: true,
            pinned: None,
        }
    }

    /// Lays the window out for `p` and returns its size.  `visible`: the window is on screen now, so a page change
    /// animates (a window appearing does not turn pages).
    pub fn update(
        &mut self,
        p: &Presentation,
        extras: Extras<'_>,
        style: &PanelStyle,
        max_width: f32,
        visible: bool,
        measure: &mut dyn Measure,
    ) -> (f32, f32) {
        let identity = Identity::of(p);
        let mut direction = 0;
        if let (Some(identity), Some((shown, page))) = (&identity, &self.shown)
            && identity == shown
            && *page != p.page
        {
            direction = if *page + 1 == p.page_count && p.page == 0 || p.page > *page { 1 } else { -1 };
        }
        if identity.is_none()
            || identity.as_ref() != self.shown.as_ref().map(|(identity, _)| identity)
            || self.shown_style.as_ref() != Some(style)
        {
            self.steady = (0., 0.);
        }
        self.shown_style = Some(style.clone());
        self.shown = identity.map(|identity| (identity, p.page));
        let layout = layout::compute(p, extras, style, max_width, self.steady, measure);
        if layout.list_open {
            self.steady = (layout.width, layout.height);
        }
        let previous = self.layout.take();
        if direction != 0 && self.animates && visible {
            self.outgoing = previous.as_ref().map(|previous| previous.rows.clone());
            self.old_label = previous.and_then(|previous| previous.pager.map(|pager| pager.label));
            self.turn = Some(Turn { direction, started: Instant::now() });
            self.bounce = None;
        } else if direction != 0 || !layout.list_open {
            self.stop_motion();
        }
        let size = (layout.width, layout.height);
        self.layout = Some(layout);
        size
    }

    /// The layout of the last update.
    pub fn layout(&self) -> Option<&PanelLayout> {
        self.layout.as_ref()
    }

    /// Paging stopped at the first or last page: the page nudges that way and settles back.
    pub fn bounce(&mut self, direction: i32) {
        if !self.animates || !self.layout.as_ref().is_some_and(|layout| layout.list_open) {
            return;
        }
        self.stop_motion();
        self.bounce = Some(Turn { direction: direction.signum(), started: Instant::now() });
    }

    /// Forgets the list shown (the window closed): the next list starts at its own size.
    pub fn reset(&mut self) {
        self.shown = None;
        self.steady = (0., 0.);
        self.stop_motion();
    }

    fn stop_motion(&mut self) {
        self.turn = None;
        self.bounce = None;
        self.outgoing = None;
        self.old_label = None;
    }

    /// The content drawn now, on its surface.  `actions` makes the rows and buttons clickable.
    pub fn render(&self, palette: &Palette, actions: Option<ActionHandler>, window: &mut Window) -> AnyElement {
        let Some(layout) = &self.layout else { return div().into_any_element() };
        let p = palette;
        let along = |amount: f32| if layout.vertical { (0., amount) } else { (amount, 0.) };

        // The page turn and the bounce, from the clock (or pinned for screenshots).
        let mut incoming = (0., 0., 1.);
        let mut outgoing = None;
        let mut highlight_shift = (0., 0.);
        let mut label_fade = 1.;
        let (turn_ms, turn_dir) = match self.pinned {
            Some(Pinned::Page { direction, progress }) => (Some(progress * motion::DURATION), direction),
            _ => (
                self.turn.map(|turn| turn.started.elapsed().as_secs_f32() * 1000.),
                self.turn.map_or(0, |t| t.direction),
            ),
        };
        if let Some(elapsed) = turn_ms.filter(|elapsed| *elapsed < motion::DURATION) {
            let travel = motion::OFFSET * turn_dir as f32;
            let t = elapsed / motion::DURATION;
            let (x, y) = along(travel * (1. - ease_out(t)));
            let fade = (elapsed - motion::IN_DELAY) / (motion::DURATION - motion::IN_DELAY);
            incoming = (x, y, ease_out(fade));
            let out = ease_out(elapsed / motion::OUT_DURATION);
            if elapsed < motion::OUT_DURATION {
                let (x, y) = along(-travel * out);
                outgoing = Some((x, y, 1. - out));
            }
            label_fade = ease_out(t);
            window.request_animation_frame();
        }
        let bounce = match self.pinned {
            Some(Pinned::Bounce { direction, progress }) => {
                let amount = if progress < 1. / 3. { progress * 3. } else { (1. - (progress - 1. / 3.) * 1.5).max(0.) };
                Some(motion::BOUNCE * direction as f32 * amount)
            }
            _ => self.bounce.and_then(|bounce| {
                let elapsed = bounce.started.elapsed().as_secs_f32() * 1000.;
                (elapsed < motion::BOUNCE_OUT + motion::BOUNCE_BACK).then(|| {
                    window.request_animation_frame();
                    let amount = if elapsed < motion::BOUNCE_OUT {
                        ease_out(elapsed / motion::BOUNCE_OUT)
                    } else {
                        1. - ease_in_out((elapsed - motion::BOUNCE_OUT) / motion::BOUNCE_BACK)
                    };
                    motion::BOUNCE * bounce.direction as f32 * amount
                })
            }),
        };
        if let Some(amount) = bounce {
            let (x, y) = along(amount);
            incoming = (x, y, 1.);
            highlight_shift = (x, y);
        }

        let fs = layout.font_size;
        let mut surface = div()
            .relative()
            .w(px(layout.width))
            .h(px(layout.height))
            .when(self.rounded, |this| this.rounded(size::R_XL))
            .bg(p.panel)
            .border_1()
            .border_color(p.panel_border)
            .font_family(theme::ui_font())
            .text_color(p.text)
            .when(self.shadow, |this| this.shadow(surface_shadow(p)));
        if let Some(notice) = &layout.notice {
            surface = surface.child(notice_element(notice, p));
        }
        if let Some((frame, text, text_w)) = &layout.preview {
            surface = surface.child(
                at(*frame)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .relative()
                            .max_w(px(frame.w))
                            .text_size(px(fs))
                            .line_height(px(fs * 1.3))
                            .truncate()
                            .child(SharedString::from(text.clone())),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .bottom(px(((frame.h - fs * 1.3) / 2.).max(0.) + 1.))
                            .w(px(text_w.min(frame.w)))
                            .h(px(1.))
                            .bg(p.text),
                    ),
            );
        }
        if let Some(hint) = &layout.hint {
            surface = surface.child(hint_element(hint, p, actions.clone()));
        }
        if layout.list_open {
            let area = layout.rows_area;
            let mut clip = at(area).overflow_hidden();
            if let Some(selected) = layout.selected_frame() {
                clip = clip.child(
                    div()
                        .absolute()
                        .left(px(selected.x + highlight_shift.0))
                        .top(px(selected.y + 1. + highlight_shift.1))
                        .w(px(selected.w))
                        .h(px(selected.h - 2.))
                        .rounded(px(layout.row_radius))
                        .bg(p.selection),
                );
            }
            if let (Some(rows), Some((x, y, alpha))) = (&self.outgoing, outgoing) {
                clip = clip.child(page_element(rows, layout, p, (x, y, alpha), false, None, "out"));
            }
            clip = clip.child(page_element(&layout.rows, layout, p, incoming, self.hover, actions.clone(), "in"));
            surface = surface.child(clip);
        }
        for toggle in [&layout.single, &layout.corrections].into_iter().flatten() {
            let action =
                if toggle.label == "單字" { PopupAction::ToggleSingle } else { PopupAction::ToggleCorrections };
            surface = surface.child(toggle_element(toggle, p, action, actions.clone()));
        }
        if let Some(pager) = &layout.pager {
            let label = |text: &str, alpha: f32| {
                at(pager.label_frame)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(text::CAPTION)
                    .line_height(px(16.))
                    .text_color(p.text_secondary)
                    .opacity(alpha)
                    .child(SharedString::from(text.to_owned()))
            };
            if label_fade < 1.
                && let Some(old) = &self.old_label
            {
                surface = surface.child(label(old, 1. - label_fade));
            }
            surface = surface.child(label(&pager.label, label_fade));
            for (frame, enabled, icon, delta, id) in [
                (pager.previous, pager.previous_enabled, IconName::ChevronLeft, -1, "page-previous"),
                (pager.next, pager.next_enabled, IconName::ChevronRight, 1, "page-next"),
            ] {
                let actions = actions.clone().filter(|_| enabled);
                surface = surface.child(
                    at(frame)
                        .id(id)
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(size::R_SM)
                        .when(actions.is_some(), |this| this.cursor_pointer().hover(|style| style.bg(p.hover)))
                        .when_some(actions, |this, actions| {
                            this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                actions(PopupAction::TurnPage(delta), window, cx)
                            })
                        })
                        .child(Icon::new(icon).size(px(14.)).text_color(if enabled {
                            p.text_secondary
                        } else {
                            p.text_tertiary
                        })),
                );
            }
        }
        surface.into_any_element()
    }
}

/// An absolutely placed box at `frame`.
fn at(frame: Frame) -> Div {
    div().absolute().left(px(frame.x)).top(px(frame.y)).w(px(frame.w)).h(px(frame.h))
}

/// The floating surface's shadow under its hairline border: a tight contact shadow and a wide soft one, the way
/// modern menus and command palettes float.
pub fn surface_shadow(p: &Palette) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: p.shadow.opacity(0.6),
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: p.shadow,
            offset: point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(-4.),
            inset: false,
        },
    ]
}

/// The room a floating window keeps around its surface for the shadow.
pub const SHADOW_MARGIN: f32 = 18.;

fn page_element(
    rows: &[RowLayout],
    layout: &PanelLayout,
    p: &Palette,
    (x, y, alpha): (f32, f32, f32),
    hover: bool,
    actions: Option<ActionHandler>,
    id: &'static str,
) -> impl IntoElement {
    let fs = layout.font_size;
    let radius = layout.row_radius;
    div().absolute().left(px(x)).top(px(y)).size_full().opacity(alpha).children(rows.iter().map(|row| {
        let selected = row.selected;
        let frame = row.frame;
        let index = row.index;
        let small = fs * 0.68;
        // The selection is a neutral fill (drawn under the pages); its number and reading come forward.
        let text_color = p.text;
        let key_color = if selected { p.text_secondary } else { p.text_tertiary };
        let annotation_color = if selected { p.text_secondary } else { p.text_tertiary };
        let actions = actions.clone();
        let cell = |left: f32, width: f32| {
            div().absolute().left(px(left)).top_0().bottom_0().w(px(width.max(0.))).flex().items_center()
        };
        div()
            .id((id, index))
            .absolute()
            .left(px(frame.x))
            .top(px(frame.y + 1.))
            .w(px(frame.w))
            .h(px(frame.h - 2.))
            .rounded(px(radius))
            .when(hover && !selected, |this| this.hover(|style| style.bg(p.hover)))
            .when_some(actions, |this, actions| {
                this.cursor_pointer().on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    actions(PopupAction::Pick(index), window, cx)
                })
            })
            .child(
                cell(row.key_inset, row.text_inset - row.key_inset - 4.)
                    .text_size(px(small))
                    .line_height(px(small * 1.3))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(key_color)
                    .child(div().truncate().child(SharedString::from(row.key.clone()))),
            )
            .child(
                cell(row.text_inset, row.main_width + 1.)
                    .text_size(px(fs))
                    .line_height(px(fs * 1.3))
                    .text_color(text_color)
                    .child(div().truncate().child(SharedString::from(row.text.clone()))),
            )
            .when_some(row.annotation.clone(), |this, annotation| {
                this.child(
                    cell(row.text_inset + row.main_width + 10., row.annotation_width + 1.)
                        .text_size(px(small))
                        .line_height(px(small * 1.3))
                        .text_color(annotation_color)
                        .child(div().truncate().child(SharedString::from(annotation))),
                )
            })
    }))
}

/// A key cap of the reading row or the footer: a rounded outline (filled, while on).
fn keycap(width: f32, height: f32, key: &str, on: bool, p: &Palette) -> Div {
    div()
        .absolute()
        .left(px(0.5))
        .top(px(((22. - height) / 2.).max(0.)))
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .rounded(size::R_SM)
        .when(on, |this| this.bg(p.selection).border_1().border_color(p.control_border).text_color(p.text))
        .when(!on, |this| this.border_1().border_color(p.control_border).text_color(p.text_secondary))
        .child(key_glyphs(key, SMALL_FONT, if on { p.text } else { p.text_secondary }))
}

/// The key glyphs live with the shared key cap (`ui::keycap`).
pub use crate::ui::keycap::key_glyphs;

fn hint_element(hint: &HintLayout, p: &Palette, actions: Option<ActionHandler>) -> impl IntoElement {
    let actions = actions.filter(|_| hint.clickable);
    let text_x = 0.5 + hint.keycap_w + 8.;
    at(hint.frame)
        .id("zhuyin-hint")
        .when(!hint.key.is_empty(), |this| this.child(keycap(hint.keycap_w, KEYCAP_HEIGHT, &hint.key, false, p)))
        .child(
            div()
                .absolute()
                .left(px(text_x))
                .top_0()
                .bottom_0()
                .w(px((hint.frame.w - text_x).max(0.)))
                .flex()
                .items_center()
                .text_size(px(SMALL_FONT))
                .line_height(px(SMALL_FONT * 1.3))
                .text_color(p.text_secondary)
                .child(div().truncate().child(SharedString::from(hint.hint.clone()))),
        )
        .when_some(actions, |this, actions| {
            this.cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| actions(PopupAction::TypeReading, window, cx))
        })
}

fn toggle_element(
    toggle: &ToggleLayout,
    p: &Palette,
    action: PopupAction,
    actions: Option<ActionHandler>,
) -> impl IntoElement {
    let color = if toggle.on { p.text } else { p.text_secondary };
    at(toggle.frame)
        .id(toggle.label)
        .child(keycap(toggle.keycap_w, KEYCAP_HEIGHT, &toggle.key, toggle.on, p))
        .child(
            div()
                .absolute()
                .left(px(0.5 + toggle.keycap_w + 7.))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .text_size(px(SMALL_FONT))
                .line_height(px(SMALL_FONT * 1.3))
                .text_color(color)
                .whitespace_nowrap()
                .child(toggle.label),
        )
        .when_some(actions, |this, actions| {
            this.cursor_pointer().on_mouse_down(MouseButton::Left, move |_, window, cx| actions(action, window, cx))
        })
}

fn notice_element(notice: &NoticeLayout, p: &Palette) -> impl IntoElement {
    let fs = notice.font_size;
    let line = px(fs * 1.3);
    at(notice.frame)
        .when_some(notice.key.clone(), |this, key| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.5))
                    .top(px(((notice.frame.h - notice.keycap_h) / 2.).round() + 0.5))
                    .w(px(notice.keycap_w - 1.))
                    .h(px(notice.keycap_h - 1.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(size::R_SM)
                    .border_1()
                    .border_color(p.control_border)
                    .child(key_glyphs(&key, fs, p.text_secondary)),
            )
        })
        .when(notice.key.is_none() && notice.done, |this| {
            this.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px((notice.frame.h - fs) / 2.))
                    .size(px(fs))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Check).size(px(fs)).text_color(p.success)),
            )
        })
        .child(
            div()
                .absolute()
                .left(px(notice.lead))
                .top_0()
                .bottom_0()
                .w(px((notice.frame.w - notice.lead).max(0.)))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(fs))
                .line_height(line)
                .child(
                    div().flex_shrink_0().text_color(p.text_secondary).child(SharedString::from(notice.label.clone())),
                )
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text)
                        .child(SharedString::from(notice.value.clone())),
                ),
        )
}

/// The 中/英 badge (macOS `ModeBadgeView` on its surface); `shadow` also rounds it (see `CandidatePanel::rounded`).
pub fn mode_badge(text: &str, p: &Palette, shadow: bool) -> AnyElement {
    div()
        .w(px(BADGE_W))
        .h(px(BADGE_H))
        .flex()
        .items_center()
        .justify_center()
        .when(shadow, |this| this.rounded(size::R_LG))
        .bg(p.panel)
        .border_1()
        .border_color(p.panel_border)
        .when(shadow, |this| this.shadow(surface_shadow(p)))
        .font_family(theme::ui_font())
        .text_size(px(BADGE_FONT))
        .line_height(px(30.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text)
        .child(SharedString::from(text.to_owned()))
        .into_any_element()
}

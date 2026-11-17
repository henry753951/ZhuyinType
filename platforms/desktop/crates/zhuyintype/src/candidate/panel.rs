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
    AnyElement, App, Div, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, Window, div, px,
};
use zt_core::presentation::Presentation;
use zt_core::service::PopupAction;

use super::layout::{
    self, BADGE_FONT, BADGE_H, BADGE_W, CONTROL_H, Extras, Frame, HintLayout, KEYCAP_HEIGHT, Measure, NoticeLayout,
    PanelLayout, RowLayout, SMALL_FONT, ToggleLayout,
};
use super::style::PanelStyle;
use crate::theme::candidate::{self as tokens, Colors};
use crate::theme::{self, Palette};

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

use crate::motion::ease_in_out;
/// Ease-out (CoreAnimation's `easeOut` drawn as macOS's pinned frames draw it) and ease-in-out.
pub use crate::motion::ease_out;

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
    /// The system asks for less motion: a page turn only cross-fades, and the bounce dims the page instead of
    /// nudging it.
    pub reduced: bool,
    /// How the surface's edge is drawn: by itself (rounded, hairline, shadow), square, or by the system.
    pub chrome: Chrome,
    pub pinned: Option<Pinned>,
}

/// Who draws the surface's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    /// Rounded, with its hairline border and shadows: inside another window (previews, the 介紹 stage, the wizard),
    /// or a floating window with a transparent margin around it (Linux with a compositor).
    Floating,
    /// Square with its hairline border: a floating window that cannot be transparent (X11 without a compositor,
    /// where rounded corners would show black).
    Square,
    /// Square and edge to edge, no border or shadow of its own: the system rounds the window and draws its border
    /// and shadow (Windows: DWM, as for the system's own menus).
    System,
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
            reduced: false,
            chrome: Chrome::Floating,
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
            let travel = if self.reduced { 0. } else { motion::OFFSET * turn_dir as f32 };
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
            if self.reduced {
                // No movement: the page dims for a moment and comes back.
                let dim = (1. - zt_panel::tokens::motion::BOUNCE_DIM) * (amount / motion::BOUNCE).abs();
                incoming = (0., 0., 1. - dim);
            } else {
                let (x, y) = along(amount);
                incoming = (x, y, 1.);
                highlight_shift = (x, y);
            }
        }

        let fs = layout.font_size;
        let colors = tokens::colors(p);
        let c = &colors;
        let mut surface =
            shell(self.chrome, c, layout.radius).relative().w(px(layout.width)).h(px(layout.height)).text_color(p.text);
        if let Some(notice) = &layout.notice {
            surface = surface.child(notice_element(notice, p, c));
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
            surface = surface.child(hint_element(hint, layout.row_radius, p, c, actions.clone()));
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
                        .bg(c.selection),
                );
            }
            if let (Some(rows), Some((x, y, alpha))) = (&self.outgoing, outgoing) {
                clip = clip.child(page_element(rows, layout, p, c, (x, y, alpha), false, None, "out"));
            }
            clip = clip.child(page_element(&layout.rows, layout, p, c, incoming, self.hover, actions.clone(), "in"));
            surface = surface.child(clip);
        }
        if let Some(rule) = layout.footer_rule {
            surface = surface.child(at(rule).bg(c.hairline));
        }
        for toggle in [&layout.single, &layout.corrections].into_iter().flatten() {
            let action =
                if toggle.label == "單字" { PopupAction::ToggleSingle } else { PopupAction::ToggleCorrections };
            surface = surface.child(toggle_element(toggle, p, c, action, actions.clone()));
        }
        if let Some(pager) = &layout.pager {
            let label = |text: &str, alpha: f32| {
                at(pager.label_frame)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(SMALL_FONT))
                    .line_height(px(16.))
                    .font_features(tokens::tabular())
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
                        .rounded(px(tokens::KEYCAP_RADIUS))
                        .text_color(if enabled { p.text_tertiary } else { p.text_tertiary.opacity(0.3) })
                        .when(actions.is_some(), |this| {
                            this.cursor_pointer().hover(|style| style.bg(c.hover).text_color(p.text))
                        })
                        .when_some(actions, |this, actions| {
                            this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                actions(PopupAction::TurnPage(delta), window, cx)
                            })
                        })
                        .child(Icon::new(icon).size(px(tokens::CHEVRON))),
                );
            }
        }
        surface.children(edge(self.chrome, c, layout.radius)).into_any_element()
    }
}

/// An absolutely placed box at `frame`.
fn at(frame: Frame) -> Div {
    div().absolute().left(px(frame.x)).top(px(frame.y)).w(px(frame.w)).h(px(frame.h))
}

/// The window's surface for `chrome`: its own colour and, unless the system frames it, a hairline border; when it
/// floats on its own, rounded with a tight contact shadow and a wide soft one (and a dark outer ring in dark), the way
/// modern menus and command palettes float.
///
/// The hairline is drawn by `edge` on top of the content rather than as the box's border, so the content's
/// coordinates start at the surface's edge whether or not it has one (the layout's insets are exact, and the text
/// lines up with the composition on every platform).
pub fn shell(chrome: Chrome, c: &Colors, radius: f32) -> Div {
    div()
        .relative()
        .bg(c.surface)
        .font(theme::font())
        .when(chrome == Chrome::Floating, |this| this.rounded(px(radius)).shadow(c.shadow_list()))
}

/// The surface's hairline border, laid over its content (see `shell`); none when the system draws the edge.
pub fn edge(chrome: Chrome, c: &Colors, radius: f32) -> Option<Div> {
    (chrome != Chrome::System).then(|| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .border_1()
            .border_color(c.border)
            .when(chrome == Chrome::Floating, |this| this.rounded(px(radius)))
    })
}

/// The room a floating window keeps around its surface for the shadow.
pub const SHADOW_MARGIN: f32 = 18.;

#[allow(clippy::too_many_arguments, reason = "one page of rows and everything it is drawn with")]
fn page_element(
    rows: &[RowLayout],
    layout: &PanelLayout,
    p: &Palette,
    c: &Colors,
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
        let small = layout::annotation_font(fs);
        let number = layout.number_font;
        // The selection is a neutral fill (drawn under the pages); its number and reading come forward.
        let text_color = p.text;
        let key_color = if selected { p.text_secondary } else { p.text_tertiary };
        let annotation_color = if selected { p.text_secondary } else { p.text_tertiary };
        let hover_fill = c.hover;
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
            .when(hover && !selected, |this| this.hover(|style| style.bg(hover_fill)))
            .when_some(actions, |this, actions| {
                this.cursor_pointer().on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    actions(PopupAction::Pick(index), window, cx)
                })
            })
            // The selection key, right-aligned in its fixed column, centred on the row like the candidate.
            .child(
                cell(row.key_inset, layout.number_width)
                    .justify_end()
                    .text_size(px(number))
                    .line_height(px(number * 1.3))
                    .font_weight(FontWeight::MEDIUM)
                    .font_features(tokens::tabular())
                    .text_color(key_color)
                    .whitespace_nowrap()
                    .child(SharedString::from(row.key.clone())),
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
                    cell(row.text_inset + row.main_width + 8., row.annotation_width + 1.)
                        .font(theme::zhuyin_font())
                        .text_size(px(small))
                        .line_height(px(small * 1.3))
                        .text_color(annotation_color)
                        .child(div().truncate().child(SharedString::from(annotation))),
                )
            })
    }))
}

/// A key cap of the reading row, the footer or a notice: borderless on a faint fill (stronger, with the label in the
/// text colour, while its switch is on), vertically centred in a control box `box_h` tall.
fn keycap(width: f32, box_h: f32, key: &str, on: bool, p: &Palette, c: &Colors) -> Div {
    let color = if on { p.text } else { p.text_secondary };
    // A lone punctuation key is drawn larger by `key_glyphs`, in the text colour so the mark is clearly there.
    let glyphs = key_glyphs(key, tokens::KEYCAP_FONT, if crate::ui::keycap::is_speck(key) { p.text } else { color });
    div()
        .absolute()
        .left_0()
        .top(px(((box_h - KEYCAP_HEIGHT) / 2.).max(0.).round()))
        .w(px(width))
        .h(px(KEYCAP_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(tokens::KEYCAP_RADIUS))
        .bg(if on { c.keycap_on } else { c.keycap })
        .font_weight(FontWeight::MEDIUM)
        .child(glyphs)
}

/// The key glyphs live with the shared key cap (`ui::keycap`).
pub use crate::ui::keycap::key_glyphs;

fn hint_element(
    hint: &HintLayout,
    radius: f32,
    p: &Palette,
    c: &Colors,
    actions: Option<ActionHandler>,
) -> impl IntoElement {
    let actions = actions.filter(|_| hint.clickable);
    let group_w = hint.keycap_w + 6. + hint.key_label_w;
    let key_room = if hint.keycap_w > 0. { group_w + hint.keycap_right + 12. } else { 0. };
    let hover_fill = c.hover;
    at(hint.frame)
        .id("zhuyin-hint")
        .rounded(px(radius))
        .child(
            div()
                .absolute()
                .left(px(hint.text_x))
                .top_0()
                .bottom_0()
                .w(px((hint.frame.w - hint.text_x - key_room).max(0.)))
                .flex()
                .items_center()
                .font(theme::zhuyin_font())
                .text_size(px(SMALL_FONT))
                .line_height(px(SMALL_FONT * 1.3))
                .text_color(p.text_secondary)
                .child(div().truncate().child(SharedString::from(hint.hint.clone()))),
        )
        .when(hint.keycap_w > 0., |this| {
            this.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(hint.keycap_right))
                    .w(px(group_w))
                    .child(keycap(hint.keycap_w, hint.frame.h, &hint.key, false, p, c))
                    .child(
                        div()
                            .absolute()
                            .left(px(hint.keycap_w + 6.))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .text_size(px(SMALL_FONT))
                            .line_height(px(SMALL_FONT * 1.3))
                            .text_color(p.text_tertiary)
                            .whitespace_nowrap()
                            .child(hint.key_label),
                    ),
            )
        })
        .when_some(actions, |this, actions| {
            this.cursor_pointer()
                .hover(|style| style.bg(hover_fill))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| actions(PopupAction::TypeReading, window, cx))
        })
}

fn toggle_element(
    toggle: &ToggleLayout,
    p: &Palette,
    c: &Colors,
    action: PopupAction,
    actions: Option<ActionHandler>,
) -> impl IntoElement {
    let color = if toggle.on { p.text } else { p.text_secondary };
    let hover_color = p.text;
    at(toggle.frame)
        .id(toggle.label)
        .text_color(color)
        .child(keycap(toggle.keycap_w, CONTROL_H, &toggle.key, toggle.on, p, c))
        .child(
            div()
                .absolute()
                .left(px(toggle.keycap_w + 6.))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .text_size(px(SMALL_FONT))
                .line_height(px(SMALL_FONT * 1.3))
                .whitespace_nowrap()
                .child(toggle.label),
        )
        .when_some(actions, |this, actions| {
            this.cursor_pointer()
                .hover(|style| style.text_color(hover_color))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| actions(action, window, cx))
        })
}

fn notice_element(notice: &NoticeLayout, p: &Palette, c: &Colors) -> impl IntoElement {
    let fs = notice.font_size;
    let line = px(fs * 1.3);
    let icon = tokens::NOTICE_ICON;
    at(notice.frame)
        .when_some(notice.key.clone(), |this, key| {
            this.child(keycap(notice.keycap_w, notice.frame.h, &key, false, p, c))
        })
        .when(notice.key.is_none() && notice.done, |this| {
            this.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(((notice.frame.h - icon) / 2.).round()))
                    .size(px(icon))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Check).size(px(icon)).text_color(p.success)),
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
                    div().min_w(px(0.)).truncate().text_color(p.text).child(SharedString::from(notice.value.clone())),
                ),
        )
}

/// The 中/英 badge on its floating surface (`floating`: rounded with its shadow; otherwise square, see `Chrome`).
pub fn mode_badge(text: &str, p: &Palette, floating: bool) -> AnyElement {
    mode_badge_in(text, p, if floating { Chrome::Floating } else { Chrome::Square })
}

/// The 中/英 badge with the edge `chrome` asks for.
pub fn mode_badge_in(text: &str, p: &Palette, chrome: Chrome) -> AnyElement {
    let c = tokens::colors(p);
    shell(chrome, &c, tokens::RADIUS)
        .w(px(BADGE_W))
        .h(px(BADGE_H))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(BADGE_FONT))
        .line_height(px(BADGE_FONT * 1.3))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text)
        .child(SharedString::from(text.to_owned()))
        .children(edge(chrome, &c, tokens::RADIUS))
        .into_any_element()
}

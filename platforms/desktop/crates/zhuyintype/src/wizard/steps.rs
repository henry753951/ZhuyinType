//! The wizard's pages (macOS `WizardView`): the header in the window's bar, each step with its footer, and the step
//! transition.  Quiet as a setup screen should be: the plain page surface, open hairline lists, a raised stage only
//! for a live demo or the practice field, choices selected with the neutral selection fill, the inverted neutral for
//! the main button, the accent only for links and the progress.
//!
//! Motion keeps macOS's rhythm: a step slides in 24 px from the side it comes from while the old one moves 12 px the
//! other way and fades sooner (0.32 s); its parts arrive one after another (10 px, 40 ms apart); the welcome arrives
//! slower (the icon, then the title, then the rest); the finish page draws its check.  Under Reduce Motion everything
//! only fades.

use std::f32::consts::PI;
use std::time::Instant;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, TitleBar, window_border};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnchoredPositionMode, AnyElement, App, ClickEvent, Context, Decorations, Div, ElementId,
    InteractiveElement, IntoElement, MouseButton, ParentElement, PathBuilder, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, TextRun, Window, anchored, canvas, deferred, div, img, point, px,
};
use serde_json::Value;
use zt_core::settings::{self, Kind};
use zt_core::shortcuts::Shortcuts;

use super::data::{self, Change, Demo, Group, LESSONS, Level, PRESETS, Style, Target};
use super::practice;
use super::{Leaving, Mode, Popover, Step, WizardWindow, layout};
use crate::app::{self, Place};
use crate::candidate::style::CurrentKeys;
use crate::pages::Page;
use crate::theme::{self, Palette, elevation, size, space, text, weight};
use crate::ui::actions::{ButtonKind, link_button, push_button};
use crate::ui::demo::{DemoFrame, Segment, fact_row, mini_demo};
use crate::ui::keycap::bindings;
use crate::ui::stepper::stepper;
use crate::ui::{h, keycap, switch, v};

// MARK: - Motion

/// The step transition (macOS `WizardMotion.step`, `WizardSlide`).
const STEP_SECONDS: f32 = 0.32;
const STEP_IN: f32 = 24.;
const STEP_OUT: f32 = 12.;
const FADE_SECONDS: f32 = 0.2;

/// CSS-style cubic bezier (0.2, 0.8, 0.2, 1): fast out of the gate, a long gentle settle.
fn step_curve(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    let (x1, y1, x2, y2) = (0.2_f32, 0.8_f32, 0.2_f32, 1.0_f32);
    let bezier = |a: f32, b: f32, s: f32| 3. * a * s * (1. - s).powi(2) + 3. * b * s * s * (1. - s) + s.powi(3);
    let mut s = t;
    for _ in 0..8 {
        let x = bezier(x1, x2, s) - t;
        let dx = 3. * x1 * (1. - s).powi(2) + 6. * (x2 - x1) * s * (1. - s) + 3. * (1. - x2) * s * s;
        if dx.abs() < 1e-5 {
            break;
        }
        s = (s - x / dx).clamp(0., 1.);
    }
    bezier(y1, y2, s)
}

fn ease_out_cubic(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.5 { 4. * t * t * t } else { 1. - (-2. * t + 2.).powi(3) / 2. }
}

/// A step's parts arriving (macOS `Entrance`): each `delay` after the step, rising 10 px and fading in.
#[derive(Clone, Copy)]
struct Arrival {
    started: Instant,
    reduced: bool,
    /// At rest (screenshots, or a step leaving).
    rest: bool,
}

impl Arrival {
    fn progress(&self, delay: f32, duration: f32, window: &mut Window) -> f32 {
        if self.rest {
            return 1.;
        }
        let (delay, duration) = if self.reduced { (delay * 0.5, FADE_SECONDS) } else { (delay, duration) };
        let t = (self.started.elapsed().as_secs_f32() - delay) / duration;
        if t < 1. {
            window.request_animation_frame();
        }
        t.clamp(0., 1.)
    }

    /// Part `order` of a step: 40 ms after the one before.
    fn part(&self, order: usize, element: impl IntoElement, window: &mut Window) -> Div {
        self.at(0.05 + order as f32 * 0.04, 10., element, window)
    }

    /// A part arriving at a moment of its own (the welcome's slower ceremony).
    fn at(&self, delay: f32, rise: f32, element: impl IntoElement, window: &mut Window) -> Div {
        let t = ease_out_cubic(self.progress(delay, 0.5, window));
        let rise = if self.reduced { 0. } else { rise * (1. - t) };
        div().relative().top(px(rise)).opacity(t).child(element)
    }
}

// MARK: - Pieces

/// A stage: the one raised surface of a step, for a live demo or the practice field (elevation 1: the card colour and
/// a hairline, no shadow).
fn stage(p: &Palette) -> Div {
    v().rounded(size::R_LG).bg(p.card).border_1().border_color(p.separator)
}

/// A step's title and the sentence under it: `text::STEP_TITLE`, or `text::DISPLAY` on the welcome.
fn step_title(title: &'static str, detail: impl Into<SharedString>, cx: &App) -> Div {
    let p = theme::palette(cx);
    v().w_full()
        .gap(space::S8)
        .child(div().text_size(text::STEP_TITLE).font_weight(weight::SEMIBOLD).text_color(p.text).child(title))
        .child(div().text_size(text::HEADLINE).text_color(p.text_secondary).child(detail.into()))
}

/// A step title centred over what follows (the finish).
fn step_title_centred(title: &'static str, detail: impl Into<SharedString>, cx: &App) -> Div {
    step_title(title, detail, cx).items_center()
}

/// The footer's buttons are this wide at least, so 下一步 and 套用 do not change size from step to step.
const BUTTON_MIN_W: f32 = 96.;

/// The step's main button: the inverted neutral, `H_LG` (a quiet `subtle` fill while it waits for the practice).
fn primary_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    push_button(id, label, ButtonKind::Default, enabled, on_click, cx)
        .h(size::H_LG)
        .min_w(px(BUTTON_MIN_W))
        .px(space::S16)
}

fn secondary_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    push_button(id, label, ButtonKind::Normal, true, on_click, cx).h(size::H_LG).px(space::S16)
}

/// The colour of a selected choice's outline: the text colour, faint (never the accent).
fn selected_border(p: &Palette) -> gpui_kit::Hsla {
    p.text.opacity(0.3)
}

/// A radio mark: a ring, with a dot in the text colour when on.
fn radio(on: bool, p: &Palette) -> impl IntoElement {
    div()
        .size(size::ICON_MD)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(if on { p.text } else { p.control_border })
        .bg(p.control)
        .when(on, |this| this.child(div().size(space::S6).rounded_full().bg(p.text)))
}

/// A check box mark: filled with the text colour (the inverted neutral) when on.
fn check_box(on: bool, p: &Palette) -> impl IntoElement {
    div()
        .size(size::ICON_MD)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(size::R_XS)
        .border_1()
        .border_color(if on { p.text } else { p.control_border })
        .bg(if on { p.text } else { p.control })
        .when(on, |this| this.child(Icon::new(IconName::Check).size(size::ICON_XS).text_color(p.on_primary)))
}

/// A selection mark for a choice card: a check in the text colour when chosen, an empty ring otherwise.
fn card_mark(selected: bool, p: &Palette) -> impl IntoElement {
    let mark = div().size(size::ICON_LG).flex_shrink_0().flex().items_center().justify_center().rounded_full();
    if selected {
        mark.bg(p.text).child(Icon::new(IconName::Check).size(size::ICON_XS).text_color(p.on_primary))
    } else {
        mark.border_1().border_color(p.control_border)
    }
}

/// A big choice: a title, lines of keys and words, the selection mark (macOS `ChoiceCard`).  Not a card at rest (a
/// hairline outline on the page); chosen, the neutral selection fill and a firmer outline.
fn choice_card(
    id: impl Into<ElementId>,
    title: &'static str,
    icon: Option<IconName>,
    lines: Vec<(Vec<SharedString>, SharedString)>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let p = theme::palette(cx);
    v().id(id)
        .flex_1()
        .min_w(px(0.))
        .p(space::S16)
        .gap(space::S12)
        .rounded(size::R_LG)
        .border_1()
        .border_color(if selected { selected_border(&p) } else { p.separator })
        .when(selected, |this| this.bg(p.selection))
        .cursor_pointer()
        .when(!selected, |this| this.hover(|style| style.bg(p.hover)))
        .on_click(on_click)
        .child(
            h().gap(space::S8)
                .when_some(icon, |this, icon| {
                    this.child(Icon::new(icon).size(size::ICON_MD).text_color(p.text_secondary))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(text::HEADLINE)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(p.text)
                        .child(title),
                )
                .child(card_mark(selected, &p)),
        )
        .children(lines.into_iter().map(move |(keys, words)| {
            h().gap(space::S8)
                .when(!keys.is_empty(), |this| {
                    this.child(h().flex_shrink_0().gap(space::S4).children(keys.into_iter().map(keycap)))
                })
                .child(
                    div()
                        .min_w(px(0.))
                        .text_size(text::BODY)
                        .text_color(p.text_secondary)
                        .whitespace_nowrap()
                        .truncate()
                        .child(words),
                )
        }))
}

/// Where a popover opens from its trigger.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Opens {
    Below,
    /// Over the trigger (a trigger in the footer).
    Above,
}

/// A floating card by its trigger (elevation 2), closed by a click elsewhere.
fn popover_card(
    content: impl IntoElement,
    opens: Opens,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let p = theme::palette(cx);
    let (anchor, at) = match opens {
        Opens::Below => (Anchor::TopLeft, point(-space::S16, size::H_SM)),
        Opens::Above => (Anchor::BottomLeft, point(-space::S16, -space::S8)),
    };
    deferred(
        anchored()
            .position_mode(AnchoredPositionMode::Local)
            .anchor(anchor)
            .position(at)
            .snap_to_window_with_margin(space::S8)
            .child(
                div()
                    .p(space::S16)
                    .rounded(size::R_LG)
                    .bg(p.card)
                    .border_1()
                    .border_color(p.separator)
                    .shadow(elevation::floating(&p))
                    .occlude()
                    .on_mouse_down_out(move |_, window, cx| on_close(window, cx))
                    .child(content),
            ),
    )
    .with_priority(2)
    .into_any_element()
}

/// 原本 → 改成, a line each (the preset's 查看 and 其他 N 項…).
fn change_list(changes: &[Change], cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    v().gap(space::S8).children(changes.iter().map(|change| {
        h().gap(space::S8)
            .text_size(text::BODY)
            .child(div().w(px(CHANGE_TITLE_W)).text_color(p.text).child(SharedString::from(change.title.clone())))
            .child(div().text_color(p.text_secondary).child(SharedString::from(change.old.clone())))
            .child(Icon::new(IconName::ArrowRight).size(size::ICON_XS).text_color(p.text_tertiary))
            .child(div().font_weight(weight::SEMIBOLD).text_color(p.text).child(SharedString::from(change.new.clone())))
    }))
}

/// The setting's name column of a change.
const CHANGE_TITLE_W: f32 = 160.;

/// The finish moment's size.
const CHECK_SIDE: f32 = 48.;

/// The finish moment (macOS `DrawnCheck`): a green circle drawn round, then its check.
fn drawn_check(arrival: Arrival, window: &mut Window, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let (circle, check, fill) = if arrival.rest {
        (1., 1., 1.)
    } else if arrival.reduced {
        let t = arrival.progress(0., FADE_SECONDS, window);
        (t, t, t)
    } else {
        let e = arrival.started.elapsed().as_secs_f32();
        if e < 0.9 {
            window.request_animation_frame();
        }
        (ease_in_out((e - 0.1) / 0.5), ease_out_cubic((e - 0.5) / 0.28), ease_out_cubic((e - 0.3) / 0.3))
    };
    let green = p.success;
    let opacity = if arrival.reduced { circle } else { 1. };
    div()
        .relative()
        .size(px(CHECK_SIDE))
        .flex_shrink_0()
        .opacity(opacity)
        .child(div().absolute().size_full().rounded_full().bg(green.opacity(0.12 * fill)))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let side = f32::from(bounds.size.width);
                    let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                    let (cx_, cy_) = (ox + side / 2., oy + side / 2.);
                    let r = side / 2. - 1.25;
                    if circle > 0. {
                        let mut path = PathBuilder::stroke(px(2.5));
                        let steps = (72. * circle).ceil().max(2.) as usize;
                        for i in 0..=steps {
                            let a = -PI / 2. + 2. * PI * circle * i as f32 / steps as f32;
                            let at = point(px(cx_ + r * a.cos()), px(cy_ + r * a.sin()));
                            if i == 0 { path.move_to(at) } else { path.line_to(at) }
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, green);
                        }
                    }
                    if check > 0. {
                        let inset = 14.;
                        let (x0, y0, w, hgt) = (ox + inset, oy + inset, side - inset * 2., side - inset * 2.);
                        let points =
                            [(x0, y0 + hgt * 0.54), (x0 + w * 0.36, y0 + hgt * 0.88), (x0 + w, y0 + hgt * 0.12)];
                        let lengths = [
                            ((points[1].0 - points[0].0).hypot(points[1].1 - points[0].1)),
                            ((points[2].0 - points[1].0).hypot(points[2].1 - points[1].1)),
                        ];
                        let mut remaining = (lengths[0] + lengths[1]) * check;
                        let mut path = PathBuilder::stroke(px(3.));
                        path.move_to(point(px(points[0].0), px(points[0].1)));
                        for segment in 0..2 {
                            let (a, b) = (points[segment], points[segment + 1]);
                            let part = (remaining / lengths[segment]).min(1.);
                            path.line_to(point(px(a.0 + (b.0 - a.0) * part), px(a.1 + (b.1 - a.1) * part)));
                            remaining -= lengths[segment];
                            if remaining <= 0. {
                                break;
                            }
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, green);
                        }
                    }
                },
            )
            .absolute()
            .size_full(),
        )
}

// MARK: - The window

/// A lesson's column of choices beside its practice stage.
const LESSON_CHOICES_W: f32 = 236.;
/// The room the 候選字視窗的樣子 preview keeps while the step is leaving.
const LOOK_PREVIEW_H: f32 = 200.;
/// A preset's card grows with the window up to this tall.
const PRESET_CARD_MAX_H: f32 = 200.;
/// The welcome's app icon.
const APP_ICON: f32 = 64.;
/// The welcome's column: the preview's stage and a note that breaks into two even lines beside it.
const WELCOME_W: f32 = 484.;
/// The room the welcome's preview keeps while the step is leaving (the live preview has moved on).
const WELCOME_PREVIEW_H: f32 = 190.;

impl Render for WizardWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.pending_appearance.take() {
            Some(forced) => theme::force(forced, window, cx),
            None => theme::sync(window, cx),
        }
        let p = theme::palette(cx);
        let reduced = cx.reduce_motion();
        let step = self.step();

        // The step, and the one leaving while the transition plays.
        let pinned = self.pinned_transition;
        let leaving = match pinned {
            Some(_) if self.index > 0 => {
                Some(Leaving { step: self.steps()[self.index - 1], at: Instant::now(), direction: 1 })
            }
            _ => self.leaving,
        };
        let progress = leaving.map(|leaving| match pinned {
            Some(progress) => progress,
            None => {
                let duration = if reduced { FADE_SECONDS } else { STEP_SECONDS };
                (leaving.at.elapsed().as_secs_f32() / duration).min(1.)
            }
        });
        if progress.is_some_and(|t| t >= 1.) && pinned.is_none() {
            self.leaving = None;
        } else if progress.is_some() && pinned.is_none() {
            window.request_animation_frame();
        }
        // The practice field takes the keyboard back when nothing else has it (a click on a choice, on the page).
        if let Step::Lesson(id) = step
            && data::lesson(id).is_some_and(|lesson| lesson.task.is_some())
            && self.focus.is_focused(window)
        {
            self.field.update(cx, |field, cx| field.focus(window, cx));
        }
        let arrival = Arrival { started: self.arrived, reduced, rest: self.still };
        let current = self.page(step, true, arrival, window, cx);
        let mut layers: Vec<AnyElement> = Vec::new();
        if let (Some(leaving), Some(t)) = (leaving.filter(|_| self.leaving.is_some() || pinned.is_some()), progress) {
            let rest = Arrival { started: self.arrived, reduced, rest: true };
            let old = self.page(leaving.step, false, rest, window, cx);
            let (eased, dir) = (if reduced { t } else { step_curve(t) }, leaving.direction as f32);
            let (old_x, new_x) =
                if reduced { (0., 0.) } else { (-dir * STEP_OUT * eased, dir * STEP_IN * (1. - eased)) };
            let old_opacity = if reduced { 1. - t } else { (1. - t * 1.6).max(0.) };
            layers.push(div().absolute().inset_0().left(px(old_x)).opacity(old_opacity).child(old).into_any_element());
            layers.push(div().absolute().inset_0().left(px(new_x)).opacity(eased).child(current).into_any_element());
        } else {
            layers.push(div().absolute().inset_0().child(current).into_any_element());
        }

        window_border().child(
            v().id("wizard")
                .size_full()
                .relative()
                .bg(p.surface)
                .text_color(p.text)
                .font(theme::font())
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|this, event, window, cx| this.key_down(event, window, cx)))
                .child(self.header(step, window, cx))
                .child(div().relative().flex_1().min_h(px(0.)).overflow_hidden().children(layers)),
        )
    }
}

impl WizardWindow {
    /// The bar: 上一步, the progress on the window's centre line, 全部跳過, the window controls.  The two buttons'
    /// text lines up with the content's edges; with window controls in the corner, 全部跳過 sits beside them.
    fn header(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let bar_button = |id: &'static str| {
            h().id(id)
                .h(size::H_MD)
                .px(space::S8)
                .gap(space::S4)
                .rounded(size::R_SM)
                .cursor_pointer()
                .text_size(text::BODY)
                .text_color(p.text_secondary)
                .hover(|style| style.bg(p.hover).text_color(p.text))
                .active(|style| style.bg(p.pressed))
                // The bar moves the window on a drag: a press on a button is the button's.
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
        };
        let controls = cfg!(target_os = "windows")
            || (cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Client { .. }));
        let back = (step != Step::Welcome && self.index > 0 && step != Step::Keys).then(|| {
            bar_button("wizard-back")
                .ml(-space::S8)
                .on_click(cx.listener(|this, _, window, cx| this.back(window, cx)))
                .child(Icon::new(IconName::ChevronLeft).size(size::ICON_MD))
                .child("上一步")
        });
        let skip = (step != Step::Finish && step != Step::Keys).then(|| {
            bar_button("wizard-skip")
                .when(!controls, |this| this.mr(-space::S8))
                .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx)))
                .child("全部跳過")
        });
        let (index, count) = self.progress();
        let progress = (step != Step::Welcome && count > 1).then(|| {
            let segment = (220. / count as f32 - 4.).clamp(8., 16.);
            v().absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(layout::HEADER))
                .items_center()
                .justify_center()
                .gap(space::S6)
                .child(h().gap(space::S4).children((1..=count).map(|number| {
                    let current = number == index;
                    div().h(space::S4).w(px(if current { segment * 2. } else { segment })).rounded_full().bg(
                        if number <= index {
                            p.accent_fill.opacity(if current { 1. } else { 0.45 })
                        } else {
                            p.text.opacity(0.1)
                        },
                    )
                })))
                .child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(p.text_secondary)
                        .child(format!("{} · {index} / {count}", step.name())),
                )
        });
        div()
            .relative()
            .flex_shrink_0()
            .child(
                TitleBar::new()
                    .h(px(layout::HEADER))
                    .pl(px(0.))
                    .bg(gpui_kit::transparent_black())
                    .border_b_0()
                    .on_close_window(cx.listener(|this, _, window, cx| this.close(window, cx)))
                    .child(
                        div().flex_1().h_full().child(
                            h().h_full()
                                .w_full()
                                .max_w(px(layout::MAX_W))
                                .mx_auto()
                                .pl(px(layout::MARGIN))
                                .pr(if controls { space::S8 } else { px(layout::MARGIN) })
                                .children(back)
                                .child(div().flex_1())
                                .children(skip),
                        ),
                    ),
            )
            .children(progress)
    }

    /// A step's page: its content from the top, never scrolled, and its footer.
    fn page(
        &mut self,
        step: Step,
        live: bool,
        arrival: Arrival,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (content, footer) = match step {
            Step::Welcome => self.welcome(live, arrival, window, cx),
            Step::Preset => self.preset_step(arrival, window, cx),
            Step::Lesson(id) => match data::lesson(id) {
                Some(lesson) => self.lesson_step(lesson, live, arrival, window, cx),
                None => (div().into_any_element(), div().into_any_element()),
            },
            Step::Finish => self.finish(arrival, window, cx),
            Step::Keys => self.keys_step(arrival, window, cx),
        };
        v().id(SharedString::from(format!("page-{}", step.id())))
            .size_full()
            .child(
                v().flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .max_w(px(layout::MAX_W))
                    .mx_auto()
                    .px(px(layout::MARGIN))
                    .pt(px(layout::TOP_INSET))
                    .child(content),
            )
            .child(
                h().w_full()
                    .max_w(px(layout::MAX_W))
                    .mx_auto()
                    .flex_shrink_0()
                    .h(px(layout::FOOTER))
                    .px(px(layout::MARGIN))
                    .child(footer),
            )
            .into_any_element()
    }

    /// The height a page's content has: the window less the bar, the footer and the top inset.
    fn room(window: &Window) -> f32 {
        f32::from(window.viewport_size().height) - layout::HEADER - layout::FOOTER - layout::TOP_INSET
    }

    // MARK: 歡迎

    fn welcome(
        &mut self,
        live: bool,
        arrival: Arrival,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let p = theme::palette(cx);
        let icon_t = ease_out_cubic(arrival.progress(0.08, 0.6, window));
        let icon_side = if arrival.reduced { APP_ICON } else { APP_ICON * (0.86 + 0.14 * icon_t) };
        let basic = LESSONS.iter().filter(|lesson| lesson.level == Level::Basic).count();
        let quick = self.mode == Mode::Quick;
        let full = self.mode == Mode::Full;
        // The preview is its own stage (the real window over a line of text) with its note beside it: no card around
        // it.  The column is as wide as the stage and a two-line note, so the note breaks evenly.
        let preview: AnyElement = if live {
            self.welcome_preview.clone().into_any_element()
        } else {
            div().h(px(WELCOME_PREVIEW_H)).into_any_element()
        };
        let column = || v().w_full().max_w(px(WELCOME_W));
        let content = v()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(APP_ICON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(icon_t)
                    .child(img("app-icon.png").size(px(icon_side)).rounded(px(icon_side * 0.22))),
            )
            .child(
                arrival.at(
                    0.42,
                    8.,
                    div()
                        .pt(space::S16)
                        .text_size(text::DISPLAY)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(p.text)
                        .child("歡迎使用 ZhuyinType"),
                    window,
                ),
            )
            .child(
                arrival.at(
                    0.56,
                    8.,
                    div()
                        .pt(space::S8)
                        .text_size(text::HEADLINE)
                        .text_color(p.text_secondary)
                        .child("選好你的打字習慣，再實際試打。"),
                    window,
                ),
            )
            .child(
                arrival.at(0.72, 8., column().pt(space::S32).child(preview), window).w_full().flex().justify_center(),
            )
            .child(
                arrival
                    .at(
                        0.84,
                        8.,
                        column().pt(space::S24).child(
                            h().items_stretch()
                                .gap(space::S12)
                                .child(choice_card(
                                    "mode-quick",
                                    "快速設定",
                                    Some(IconName::Zap),
                                    vec![(vec![], format!("{basic} 個步驟，約 2 分鐘").into())],
                                    quick,
                                    cx.listener(|this, _, _, cx| this.choose_mode(Mode::Quick, cx)),
                                    cx,
                                ))
                                .child(choice_card(
                                    "mode-full",
                                    "完整導覽",
                                    Some(IconName::Map),
                                    vec![(vec![], format!("{} 個步驟，每個功能", LESSONS.len()).into())],
                                    full,
                                    cx.listener(|this, _, _, cx| this.choose_mode(Mode::Full, cx)),
                                    cx,
                                )),
                        ),
                        window,
                    )
                    .w_full()
                    .flex()
                    .justify_center(),
            )
            .into_any_element();
        let footer = h()
            .w_full()
            .child(div().flex_1())
            .child(arrival.at(
                0.84,
                0.,
                primary_button(
                    "wizard-start",
                    "開始",
                    true,
                    cx.listener(|this, _, window, cx| this.next(window, cx)),
                    cx,
                ),
                window,
            ))
            .into_any_element();
        (content, footer)
    }

    // MARK: 習慣組合

    fn preset_step(
        &mut self,
        arrival: Arrival,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let p = theme::palette(cx);
        let draft = self.draft(cx);
        let changes = data::changes(&self.original, &draft);
        let cards: Vec<AnyElement> = PRESETS
            .iter()
            .enumerate()
            .map(|(index, preset)| {
                let lines = preset
                    .lines
                    .iter()
                    .map(|(keys, words)| {
                        (keys.iter().map(|k| SharedString::from(*k)).collect(), SharedString::from(*words))
                    })
                    .collect();
                arrival
                    .part(
                        1 + index,
                        choice_card(
                            SharedString::from(format!("preset-{}", preset.id)),
                            preset.title,
                            None,
                            lines,
                            self.preset == preset.id,
                            cx.listener(move |this, _, _, cx| this.choose_preset(index, cx)),
                            cx,
                        )
                        .size_full(),
                        window,
                    )
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .into_any_element()
            })
            .collect();
        // Two rows of two that share the page's height, so the choices fill it rather than leaving a hole under them.
        let mut cards = cards.into_iter();
        let mut rows = Vec::new();
        while let Some(first) = cards.next() {
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .max_h(px(PRESET_CARD_MAX_H))
                    .gap(space::S16)
                    .child(first)
                    .children(cards.next())
                    .into_any_element(),
            );
        }
        let count = changes.len();
        let showing = self.popover == Some(Popover::PresetChanges);
        // What the choice changes, by the footer's button.
        let summary = h()
            .gap(space::S8)
            .child(div().text_size(text::BODY).text_color(p.text_secondary).child(if count == 0 {
                "和目前的設定相同".to_owned()
            } else {
                format!("會改變 {count} 項設定")
            }))
            .when(count > 0, |this| {
                this.child(
                    v().child(link_button(
                        "preset-show-changes",
                        "查看",
                        cx.listener(|this, _, _, cx| {
                            this.popover = Some(Popover::PresetChanges);
                            cx.notify();
                        }),
                        cx,
                    ))
                    .when(showing, |this| {
                        let weak = cx.weak_entity();
                        this.child(popover_card(
                            change_list(&changes, cx),
                            Opens::Above,
                            move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.popover = None;
                                    cx.notify();
                                });
                            },
                            cx,
                        ))
                    }),
                )
            });
        let content = v()
            .size_full()
            .gap(space::S24)
            .child(arrival.part(
                0,
                step_title("你原本習慣哪一種注音輸入？", "選最接近的一種，之後每一步都能再改。", cx),
                window,
            ))
            .child(v().flex_1().min_h(px(0.)).gap(space::S16).children(rows))
            .into_any_element();
        let footer = h()
            .w_full()
            .child(arrival.part(6, summary, window))
            .child(div().flex_1())
            .child(primary_button(
                "wizard-next",
                "下一步",
                true,
                cx.listener(|this, _, window, cx| this.next(window, cx)),
                cx,
            ))
            .into_any_element();
        (content, footer)
    }

    // MARK: Lessons

    fn lesson_step(
        &mut self,
        lesson: &'static data::Lesson,
        live: bool,
        arrival: Arrival,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let draft = self.draft(cx);
        let done = self.lesson_done(lesson);
        let groups = lesson.groups();
        // A horizontal list is wide and short: the practice field takes the whole width under the choices.  A
        // vertical one is narrow and tall: the field takes the right column.  Either way the practice stage fills
        // the height left, so the candidate window opens inside it.
        let wide = draft.string("candidateLayout") == "horizontal" && lesson.task.is_some();
        let demo = self.lesson_demo(lesson, live, window, cx);
        let fills = lesson.task.is_some();
        let demo = arrival.part(2, demo, window).flex().flex_col().when(fills, |this| this.flex_1().min_h(px(0.)));
        let content = if wide {
            let across = groups.len() == 1;
            v().size_full()
                .gap(space::S24)
                .child(arrival.part(0, step_title(lesson.title, lesson.detail, cx), window))
                .child(
                    arrival.part(
                        1,
                        div().flex().flex_row().items_start().gap(space::S24).children(
                            groups
                                .iter()
                                .map(|group| div().flex_1().min_w(px(0.)).child(self.group_view(group, across, cx))),
                        ),
                        window,
                    ),
                )
                .child(demo)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .size_full()
                .gap(space::S24)
                .child(
                    v().w(px(LESSON_CHOICES_W))
                        .flex_shrink_0()
                        .gap(space::S24)
                        .child(arrival.part(0, step_title(lesson.title, lesson.detail, cx), window))
                        .child(arrival.part(
                            1,
                            v().gap(space::S20).children(groups.iter().map(|group| self.group_view(group, false, cx))),
                            window,
                        )),
                )
                .child(v().flex_1().min_w(px(0.)).child(demo))
                .into_any_element()
        };
        let keys = groups.iter().any(|group| group.styles.iter().any(Style::is_key));
        let footer = h()
            .w_full()
            .gap(space::S8)
            .when(keys, |this| {
                this.child(link_button(
                    "wizard-custom-keys",
                    "自行設定…",
                    |_, _, cx| {
                        crate::shell::show(cx);
                        app::go(Place::page(Page::Keys), cx);
                    },
                    cx,
                ))
            })
            .child(div().flex_1())
            .when(!done, |this| {
                this.child(secondary_button(
                    "wizard-skip-step",
                    "略過這一步",
                    cx.listener(|this, _, window, cx| this.next(window, cx)),
                    cx,
                ))
            })
            .child(primary_button(
                "wizard-next",
                "下一步",
                done,
                cx.listener(|this, _, window, cx| this.next(window, cx)),
                cx,
            ))
            .into_any_element();
        (content, footer)
    }

    /// The lesson's one live demo: the practice field (with the real window) on a stage that fills the column, or
    /// for a lesson without practice its preview (a stage of its own) or ⓘ demo.
    fn lesson_demo(
        &mut self,
        lesson: &data::Lesson,
        live: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = theme::palette(cx);
        if let Some(task) = &lesson.task {
            let field = if live && self.field_task == Some(lesson.id) {
                self.field.clone().into_any_element()
            } else {
                practice::placeholder(task, cx)
            };
            return stage(&p).flex_1().min_h(px(0.)).p(space::S16).child(field).into_any_element();
        }
        let draft = self.draft(cx);
        match lesson.demo {
            Some(Demo::Preview(_)) if live => self.look_preview.clone().into_any_element(),
            Some(Demo::Preview(_)) => div().h(px(LOOK_PREVIEW_H)).into_any_element(),
            Some(Demo::PunctuationWidth) => {
                let toggle = CurrentKeys::sandboxed(&draft).caps("togglePunctuationWidth");
                let full = DemoFrame::new([Segment::marked("＃")], "中文預設全形標點", 1.5).keys(["⇧", "3"]);
                let frames = match toggle {
                    Some(toggle) => vec![
                        full,
                        DemoFrame::new([Segment::marked("＃")], "切換為半形，保留已打的字", 1.5).keys(toggle.clone()),
                        DemoFrame::new([Segment::marked("＃#")], "之後輸入半形標點", 1.5).keys(["⇧", "3"]),
                        DemoFrame::new([Segment::marked("＃#")], "再按一次切回全形", 1.5).keys(toggle),
                    ],
                    None => vec![full],
                };
                v().gap(space::S12)
                    .child(mini_demo("wizard-punctuation-width", frames))
                    .child(
                        div()
                            .text_size(text::SUBTITLE)
                            .text_color(p.text_secondary)
                            .child("只切換標點；英文字母、數字與空白維持半形。英文模式使用半形。"),
                    )
                    .into_any_element()
            }
            Some(Demo::CorrectionLog) => correction_log_facts(&draft),
            None => div().into_any_element(),
        }
    }

    /// A group of styles: exclusive ones as options, only the chosen one's detail shown; combinable ones as check
    /// boxes, their detail on hover (macOS `StyleGroupView`).  A chosen option has the neutral selection fill.
    fn group_view(&self, group: &Group, across: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let draft = self.draft(cx);
        let title = div().text_size(text::HEADLINE).font_weight(weight::SEMIBOLD).text_color(p.text).child(group.title);
        if !group.exclusive {
            let boxes = group.styles.iter().enumerate().map(|(index, style)| {
                let on = style.is_active(&draft);
                let style = style.clone();
                let detail = style.detail.clone();
                h().id(SharedString::from(format!("style-{}-{index}", group.id)))
                    .h(size::H_MD)
                    .gap(space::S8)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        style.set(&this.draft(cx), !on);
                        this.draft_changed(cx);
                    }))
                    .when_some(detail, |this, detail| {
                        this.tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
                    })
                    .child(check_box(on, &p))
                    .child(
                        div()
                            .text_size(text::BODY)
                            .text_color(p.text)
                            .whitespace_nowrap()
                            .child(SharedString::from(group.styles[index].title.clone())),
                    )
            });
            return v()
                .gap(space::S8)
                .child(title)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_x(if across { space::S16 } else { space::S12 })
                        .children(boxes),
                )
                .into_any_element();
        }
        let options = group.styles.iter().enumerate().map(|(index, style)| {
            let on = style.is_active(&draft);
            let chosen = style.clone();
            h().id(SharedString::from(format!("style-{}-{index}", group.id)))
                .when(across, |this| this.flex_1().min_w(px(0.)))
                .h(size::H_FIELD)
                .px(space::S12)
                .gap(space::S8)
                .rounded(size::R_SM)
                .border_1()
                .border_color(if on { selected_border(&p) } else { p.separator })
                .when(on, |this| this.bg(p.selection))
                .cursor_pointer()
                .when(!on, |this| this.hover(|s| s.bg(p.hover)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    chosen.set(&this.draft(cx), true);
                    this.draft_changed(cx);
                }))
                .child(radio(on, &p))
                .child(
                    div()
                        .min_w(px(0.))
                        .text_size(text::BODY)
                        .text_color(p.text)
                        .whitespace_nowrap()
                        .truncate()
                        .child(SharedString::from(style.title.clone())),
                )
        });
        let detail = group.styles.iter().find(|style| style.is_active(&draft)).and_then(|style| style.detail.clone());
        v().gap(space::S8)
            .child(title)
            .child(if across {
                h().gap(space::S8).children(options).into_any_element()
            } else {
                v().gap(space::S6).children(options).into_any_element()
            })
            .when_some(detail, |this, detail| {
                this.child(
                    div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(SharedString::from(detail)),
                )
            })
            .into_any_element()
    }

    // MARK: 確認

    fn finish(&mut self, arrival: Arrival, window: &mut Window, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let p = theme::palette(cx);
        let draft = self.draft(cx);
        let changes = data::changes(&self.original, &draft);
        // As many rows as the window has room for; the rest behind 「其他 N 項」.
        let row = f32::from(size::ROW_MIN_SINGLE) + f32::from(size::HAIRLINE);
        let hero = CHECK_SIDE + 16. + 30. + 8. + 22.;
        let fixed = hero + 24. + if self.phrases.is_empty() { 0. } else { 16. + 28. };
        let fit = (((Self::room(window) - fixed - 2.) / row).floor() as usize).max(2);
        let shown = if changes.len() <= fit { changes.len() } else { fit - 1 };
        let rest: Vec<Change> = changes[shown..].to_vec();
        let detail = if changes.is_empty() {
            "沒有要改變的設定。"
        } else {
            "按「套用」之前，還可以改回任何一項。"
        };
        let hairline = || div().mx(size::ROW_PADDING_X).h(size::HAIRLINE).bg(p.separator);
        let mut list = v();
        for (index, change) in changes.iter().take(shown).enumerate() {
            if index > 0 {
                list = list.child(hairline());
            }
            let row = self.change_row(change, &draft, cx);
            list = list.child(arrival.part(2 + index, row, window));
        }
        if !rest.is_empty() {
            let showing = self.popover == Some(Popover::Rest);
            let weak = cx.weak_entity();
            list = list.child(hairline()).child(
                h().h(size::ROW_MIN_SINGLE).px(size::ROW_PADDING_X).child(
                    v().child(link_button(
                        "finish-rest",
                        format!("其他 {} 項…", rest.len()),
                        cx.listener(|this, _, _, cx| {
                            this.popover = Some(Popover::Rest);
                            cx.notify();
                        }),
                        cx,
                    ))
                    .when(showing, |this| {
                        this.child(popover_card(
                            change_list(&rest, cx),
                            Opens::Above,
                            move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.popover = None;
                                    cx.notify();
                                });
                            },
                            cx,
                        ))
                    }),
                ),
            );
        }
        let phrases = (!self.phrases.is_empty()).then(|| {
            let words = self.phrases.iter().map(|(word, _)| word.as_str()).collect::<Vec<_>>().join("、");
            let keep = self.keep_phrases;
            h().id("keep-phrases")
                .h(size::H_MD)
                .gap(space::S8)
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.keep_phrases = !this.keep_phrases;
                    cx.notify();
                }))
                .child(check_box(keep, &p))
                .child(
                    div()
                        .text_size(text::BODY)
                        .text_color(p.text)
                        .truncate()
                        .child(format!("保留練習時加入的自訂詞（{words}）")),
                )
        });
        // The moment, centred over the list; the whole block in the middle of the page.
        let heading = v().items_center().gap(space::S16).child(drawn_check(arrival, window, cx)).child(arrival.at(
            0.3,
            10.,
            step_title_centred("設定好了", detail, cx),
            window,
        ));
        let content = v()
            .size_full()
            .justify_center()
            .gap(space::S24)
            .child(heading)
            .when(!changes.is_empty(), |this| this.child(arrival.part(1, stage(&p).w_full().child(list), window)))
            .when_some(phrases, |this, phrases| this.child(arrival.part(3 + shown, phrases, window)))
            .when_some(self.error.clone(), |this, error| {
                this.child(div().text_size(text::SUBTITLE).text_color(p.danger).child(error))
            })
            .into_any_element();
        let footer = h()
            .w_full()
            .gap(space::S8)
            .child(div().flex_1())
            .child(secondary_button(
                "wizard-discard",
                "全部捨棄",
                cx.listener(|this, _, window, cx| this.discard(window, cx)),
                cx,
            ))
            .child(primary_button(
                "wizard-apply",
                "套用",
                true,
                cx.listener(|this, _, window, cx| this.apply(window, cx)),
                cx,
            ))
            .into_any_element();
        (content, footer)
    }

    /// 原本 → 改成, with 改回; the setting's name opens its control in a popover (macOS `ChangeRow`).
    fn change_row(&self, change: &Change, draft: &zt_core::settings::Settings, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let id = change.id.clone();
        let editing = self.popover == Some(Popover::Edit(id.clone()));
        let weak = cx.weak_entity();
        let editor = editing.then(|| {
            popover_card(
                self.change_editor(change, draft, cx),
                Opens::Below,
                move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.popover = None;
                        cx.notify();
                    });
                },
                cx,
            )
        });
        let open_id = id.clone();
        let revert = change.clone();
        h().h(size::ROW_MIN_SINGLE)
            .px(size::ROW_PADDING_X)
            .gap(space::S12)
            .text_size(text::BODY)
            .child(
                v().w(px(CHANGE_TITLE_W))
                    .flex_shrink_0()
                    .child(
                        h().id(SharedString::from(format!("change-{id}")))
                            .gap(space::S4)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.popover = Some(Popover::Edit(open_id.clone()));
                                cx.notify();
                            }))
                            .child(div().text_color(p.text).truncate().child(SharedString::from(change.title.clone())))
                            .child(Icon::new(IconName::ChevronDown).size(size::ICON_XS).text_color(p.text_tertiary)),
                    )
                    .children(editor),
            )
            .child(
                h().flex_1()
                    .min_w(px(0.))
                    .gap(space::S8)
                    .child(
                        div()
                            .min_w(px(0.))
                            .text_color(p.text_secondary)
                            .line_through()
                            .truncate()
                            .child(SharedString::from(change.old.clone())),
                    )
                    .child(Icon::new(IconName::ArrowRight).size(size::ICON_XS).text_color(p.text_tertiary))
                    .child(
                        div()
                            .min_w(px(0.))
                            .font_weight(weight::SEMIBOLD)
                            .text_color(p.text)
                            .truncate()
                            .child(SharedString::from(change.new.clone())),
                    ),
            )
            .child(link_button(
                SharedString::from(format!("revert-{id}")),
                "改回",
                cx.listener(move |this, _, _, cx| this.revert(&revert, cx)),
                cx,
            ))
            .into_any_element()
    }

    /// The control of a change's setting, on the draft (macOS `SettingControl`).
    fn change_editor(
        &self,
        change: &Change,
        draft: &zt_core::settings::Settings,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = theme::palette(cx);
        let title = div()
            .text_size(text::HEADLINE)
            .font_weight(weight::SEMIBOLD)
            .text_color(p.text)
            .child(SharedString::from(change.title.clone()));
        let control: AnyElement = match &change.target {
            Target::Setting(key, _, _) => {
                let key: &'static str = key;
                let Some(descriptor) = settings::descriptor(key) else { return div().into_any_element() };
                let current = draft.get(key);
                match descriptor.kind {
                    Kind::Toggle => {
                        let on = current.as_bool().unwrap_or(false);
                        h().gap(space::S12)
                            .child(switch(SharedString::from(format!("edit-{key}")), on, {
                                let weak = cx.weak_entity();
                                move |on, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        let _ = this.draft(cx).set(key, Value::Bool(on));
                                        this.draft_changed(cx);
                                    });
                                }
                            }))
                            .child(div().text_size(text::BODY).text_color(p.text_secondary).child(if on {
                                "開啟"
                            } else {
                                "關閉"
                            }))
                            .into_any_element()
                    }
                    Kind::Choice => v()
                        .gap(space::S2)
                        .min_w(px(EDITOR_MIN_W))
                        .mx(-space::S8)
                        .children(descriptor.options.iter().enumerate().map(|(index, choice)| {
                            let on = choice.value == current;
                            let value = choice.value.clone();
                            h().id(SharedString::from(format!("edit-{key}-{index}")))
                                .h(size::H_MD)
                                .gap(space::S8)
                                .px(space::S8)
                                .rounded(size::R_SM)
                                .cursor_pointer()
                                .when(on, |this| this.bg(p.selection))
                                .hover(|s| s.bg(p.hover))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let _ = this.draft(cx).set(key, value.clone());
                                    this.draft_changed(cx);
                                }))
                                .child(radio(on, &p))
                                .child(
                                    div()
                                        .text_size(text::BODY)
                                        .text_color(p.text)
                                        .child(SharedString::from(choice.label.clone())),
                                )
                        }))
                        .into_any_element(),
                    Kind::Number => {
                        let value = current.as_f64().unwrap_or(0.) as i64;
                        let (min, max) = (descriptor.min.unwrap_or(0.) as i64, descriptor.max.unwrap_or(100.) as i64);
                        let weak = cx.weak_entity();
                        h().gap(space::S12)
                            .child(
                                div()
                                    .text_size(text::BODY)
                                    .text_color(p.text)
                                    .child(SharedString::from(data::value_label(descriptor, &current))),
                            )
                            .child(stepper(
                                format!("edit-{key}"),
                                value,
                                min,
                                max,
                                move |next, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        let _ = this.draft(cx).set(key, Value::from(next));
                                        this.draft_changed(cx);
                                    });
                                },
                                cx,
                            ))
                            .into_any_element()
                    }
                    Kind::Text => div().into_any_element(),
                }
            }
            Target::Keys(command) => {
                let command: &'static str = command;
                let keys = Shortcuts::new(draft.clone()).bindings(command);
                v().gap(space::S12)
                    .child(bindings(keys.into_iter().map(|b| b.label)))
                    .child(link_button(
                        SharedString::from(format!("edit-keys-{command}")),
                        "在「快捷鍵」頁設定",
                        move |_, _, cx| {
                            crate::shell::show(cx);
                            app::go(Place::row(Page::Keys, format!("keys-{command}")), cx);
                        },
                        cx,
                    ))
                    .into_any_element()
            }
        };
        v().gap(space::S12).child(title).child(control).into_any_element()
    }

    // MARK: 你會用到的按鍵

    fn keys_step(&mut self, arrival: Arrival, window: &mut Window, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let p = theme::palette(cx);
        let groups = data::key_groups(&self.original);
        // Two columns of equal width, the groups shared out so their lengths come out alike; each group an open
        // list with hairlines, its keys in a column as wide as its widest keys.
        let mut columns: [Vec<(usize, &data::KeyGroup)>; 2] = [Vec::new(), Vec::new()];
        let mut lengths = [0usize; 2];
        for (index, group) in groups.iter().enumerate() {
            let rows = group.items.len().min(KEYS_PER_GROUP) + 1;
            let column = if lengths[0] <= lengths[1] { 0 } else { 1 };
            lengths[column] += rows;
            columns[column].push((index, group));
        }
        let column_view = |list: &[(usize, &data::KeyGroup)], window: &mut Window| {
            let key_w = list
                .iter()
                .flat_map(|(_, group)| group.items.iter().take(KEYS_PER_GROUP))
                .map(|(keys, _)| bindings_width(keys, window))
                .fold(KEY_COLUMN_MIN, f32::max);
            v().flex_1().min_w(px(0.)).gap(space::S24).children(list.iter().map(|(index, group)| {
                let heading = h()
                    .gap(space::S8)
                    .pb(size::SECTION_HEADING_PB)
                    .border_b_1()
                    .border_color(p.separator)
                    .child(Icon::new(group.icon).size(size::ICON_SM).text_color(p.text_secondary))
                    .child(
                        div()
                            .text_size(text::HEADLINE)
                            .font_weight(weight::SEMIBOLD)
                            .text_color(p.text)
                            .child(group.title),
                    );
                let shown = group.items.len().min(KEYS_PER_GROUP);
                let rows = group.items.iter().take(KEYS_PER_GROUP).enumerate().map(|(row, (keys, action))| {
                    h().min_h(px(KEY_ROW_H))
                        .py(space::S8)
                        .gap(space::S12)
                        .when(row + 1 < shown, |this| this.border_b_1().border_color(p.separator))
                        .child(div().w(px(key_w)).flex_shrink_0().child(bindings(keys.iter())))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(text::BODY)
                                .text_color(p.text)
                                .child(SharedString::from(action.clone())),
                        )
                });
                arrival.part(1 + index, v().child(heading).children(rows), window)
            }))
        };
        let [left, right] = &columns;
        let left = column_view(left, window);
        let right = column_view(right, window);
        let content = v()
            .size_full()
            .gap(space::S24)
            .child(arrival.part(
                0,
                step_title("你會用到的按鍵", "依你的設定產生；之後可以在「介紹」再看一次。", cx),
                window,
            ))
            .child(div().flex().flex_row().items_start().gap(space::S24).child(left).child(right))
            .into_any_element();
        let footer = h()
            .w_full()
            .child(div().flex_1())
            .child(primary_button(
                "wizard-done",
                "完成",
                true,
                cx.listener(|this, _, window, cx| this.close(window, cx)),
                cx,
            ))
            .into_any_element();
        (content, footer)
    }
}

/// The keys step's rows: a key cap and the line of text beside it.
const KEY_ROW_H: f32 = 36.;
/// The keys column is at least this wide (and as wide as its widest keys).
const KEY_COLUMN_MIN: f32 = 64.;
/// The keys a group of the keys step lists.
const KEYS_PER_GROUP: usize = 6;
/// The width of one choice's popover control.
const EDITOR_MIN_W: f32 = 200.;

/// How wide `labels` draw as key caps side by side (`keycap::bindings`): each cap its text in the caption size, its
/// symbols as icons, `S6` either side, at least `H_XS`; caps `S4` apart, alternatives `S12`.
fn bindings_width(labels: &[String], window: &Window) -> f32 {
    let font_size = f32::from(text::CAPTION);
    let icon = (font_size * 0.95).round();
    let measure = |text: &str| -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let run = TextRun {
            len: text.len(),
            font: theme::font(),
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(window.text_system().layout_line(text, px(font_size), &[run], None).width)
    };
    let cap = |label: &str| -> f32 {
        let symbols = label.chars().filter(|c| "⇧↑↓←→↩⌫".contains(*c)).count() as f32;
        let rest: String = label.chars().filter(|c| !"⇧↑↓←→↩⌫".contains(*c)).collect();
        (measure(&rest) + symbols * icon + f32::from(space::S6) * 2.).max(f32::from(size::H_XS))
    };
    let one = |label: &str| -> f32 {
        let caps = keycap::split(label);
        caps.iter().map(|c| cap(c)).sum::<f32>() + caps.len().saturating_sub(1) as f32 * f32::from(space::S4)
    };
    labels.iter().map(|label| one(label)).sum::<f32>() + labels.len().saturating_sub(1) as f32 * f32::from(space::S12)
}

/// What the correction log records, with the draft's keys (the 改字記錄 lesson; macOS `CorrectionLogInfo`).
fn correction_log_facts(draft: &zt_core::settings::Settings) -> AnyElement {
    let keys = CurrentKeys::sandboxed(draft);
    let key = |command: &str| Some(keys.text(command)).filter(|text| !text.is_empty());
    let lists: Vec<String> = std::iter::once("從清單選字".to_owned())
        .chain(key("correctionsList").map(|k| format!("{k} 修正")))
        .chain(key("singleCharacterList").map(|k| format!("{k} 單字")))
        .collect();
    let editing: Vec<String> = key("typeZhuyin")
        .map(|k| format!("{k} 打出注音"))
        .into_iter()
        .chain(key("selectLeft").zip(key("selectRight")).map(|(left, right)| format!("{left} {right} 加入自訂詞")))
        .chain(std::iter::once("在句中修改".to_owned()))
        .collect();
    v().gap(space::S8)
        .children([
            fact_row(IconName::CircleCheck, "只記錄有改字的組字").into_any_element(),
            fact_row(IconName::Keyboard, "打的鍵與按鍵間隔，以及每一步畫面上的字").into_any_element(),
            fact_row(IconName::ListChecks, lists.join("、")).into_any_element(),
            fact_row(IconName::Delete, "Backspace 或 Esc 刪掉後重打；送出後 30 秒內刪掉重打").into_any_element(),
            fact_row(IconName::TextCursorInput, editing.join("、")).into_any_element(),
            fact_row(IconName::TextAlignStart, "游標前的文字只在開啟第二個選項時記錄").into_any_element(),
            fact_row(IconName::Lock, "密碼欄位中不記錄；記錄中沒有 App 的名稱").into_any_element(),
            fact_row(IconName::HardDrive, "存在這台電腦的 corrections.jsonl，可以匯出").into_any_element(),
        ])
        .into_any_element()
}

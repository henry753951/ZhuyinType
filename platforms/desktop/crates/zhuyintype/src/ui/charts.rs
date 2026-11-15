//! Statistics components (the 分析 page; any page that shows numbers): tiles, the recap card with its chips, bar
//! charts with a value axis, a weekday × hour heat map, a two-part bar and a single meter.  All drawn with plain
//! elements in the accent colour (one series, one colour, no legend), as the macOS charts.

use std::rc::Rc;

use gpui_kit::component::Icon;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, linear_color_stop, linear_gradient, px, relative,
};

use super::{h, v};
use crate::theme::{self, size, text};

/// Padding inside tiles and cards (macOS `RowMetrics.tilePadding`).
const TILE_PADDING: f32 = 14.;
/// Space between tiles (macOS `RowMetrics.tileSpacing`).
const TILE_SPACING: f32 = 12.;

/// 38126 → 「38,126」.
pub fn grouped(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// A tile: a small icon and title over a large number.
pub fn stat_tile(icon: IconName, title: impl Into<SharedString>, value: impl Into<SharedString>, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    v().gap(px(4.))
        .p(px(TILE_PADDING))
        .rounded(size::RADIUS_CARD)
        .bg(p.card)
        .border_1()
        .border_color(p.separator)
        .child(
            h().gap(px(5.))
                .text_size(text::CAPTION)
                .text_color(p.text_secondary)
                .child(Icon::new(icon).size(px(12.)))
                .child(div().truncate().child(title.into())),
        )
        .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(value.into()))
        .into_any_element()
}

/// Tiles in `columns` equal columns, wrapping to fewer when a tile would be narrower than `min_width`.
pub fn tile_grid(columns: usize, min_width: f32, tiles: impl IntoIterator<Item = AnyElement>) -> impl IntoElement {
    // Each tile starts at `min_width` and grows; it is never narrower than a share that would let one more column
    // in, so a row holds at most `columns`.
    let share = 1. / (columns as f32 + 1.) + 0.01;
    // The layout engine sizes a wrapping row's intrinsic width as if it did not wrap; a zero width with a full
    // minimum keeps the grid out of the page's intrinsic width while it still fills the page.
    div().w(px(0.)).min_w(relative(1.)).child(
        h().w_full()
            .flex_wrap()
            .items_stretch()
            .gap(px(TILE_SPACING))
            .children(tiles.into_iter().map(move |tile| {
                div()
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .flex_basis(px(min_width))
                    .min_w(relative(share))
                    .flex()
                    .flex_col()
                    .child(tile)
            })),
    )
}

/// A small capsule with an icon: 「連續 3 天」.
pub fn info_chip(icon: IconName, label: impl Into<SharedString>, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    h().gap(px(4.))
        .px(px(8.))
        .py(px(3.))
        .rounded_full()
        .bg(p.card.opacity(0.7))
        .text_size(text::CAPTION)
        .text_color(p.text_secondary)
        .whitespace_nowrap()
        .child(Icon::new(icon).size(px(11.)))
        .child(label.into())
        .into_any_element()
}

/// The recap at the top of a statistics page: a caption, a large number with its unit, and chips, on an accent wash.
pub fn recap_card(
    caption: impl Into<SharedString>,
    number: impl Into<SharedString>,
    unit: impl Into<SharedString>,
    chips: Vec<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    v().w_full()
        .gap(px(10.))
        .p(px(TILE_PADDING + 2.))
        .rounded(size::RADIUS_CARD)
        .bg(linear_gradient(
            135.,
            linear_color_stop(p.accent.opacity(if p.dark { 0.24 } else { 0.16 }), 0.),
            linear_color_stop(p.accent.opacity(0.04), 1.),
        ))
        .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(caption.into()))
        .child(
            h().items_baseline()
                .gap(px(6.))
                .child(div().text_size(px(40.)).line_height(px(44.)).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(number.into()))
                .child(div().text_size(text::HEADLINE).text_color(p.text_secondary).child(unit.into())),
        )
        .when(!chips.is_empty(), |this| {
            this.child(div().w(px(0.)).min_w(relative(1.)).child(h().w_full().flex_wrap().gap(px(8.)).children(chips)))
        })
}

/// Round `value` up to 1, 2, 2.5 or 5 × 10ⁿ.
fn nice_ceil(value: f64) -> f64 {
    if value <= 0. {
        return 1.;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let fraction = value / magnitude;
    let nice = [1., 2., 2.5, 5., 10.].into_iter().find(|n| fraction <= *n).unwrap_or(10.);
    nice * magnitude
}

type HoverHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// Bars over a value axis (0, one step, two steps), with labels under some bars.
#[derive(IntoElement)]
pub struct BarChart {
    id: SharedString,
    values: Vec<f64>,
    labels: Vec<(usize, SharedString)>,
    height: f32,
    bar_ratio: f32,
    /// A number shown over one bar (the best day) while nothing is hovered.
    annotate: Option<(usize, SharedString)>,
    hovered: Option<usize>,
    tooltip: Option<Rc<dyn Fn(usize) -> SharedString>>,
    on_hover: Option<HoverHandler>,
    caption: Option<SharedString>,
}

/// A bar chart of `values` (one bar each), `height` tall in all.
pub fn bar_chart(id: impl Into<SharedString>, values: Vec<f64>, height: f32) -> BarChart {
    BarChart {
        id: id.into(),
        values,
        labels: Vec::new(),
        height,
        bar_ratio: 0.62,
        annotate: None,
        hovered: None,
        tooltip: None,
        on_hover: None,
        caption: None,
    }
}

impl BarChart {
    /// Labels under bars, by index.
    pub fn labels(mut self, labels: Vec<(usize, SharedString)>) -> Self {
        self.labels = labels;
        self
    }

    /// The share of each slot the bar takes.
    pub fn bar_ratio(mut self, ratio: f32) -> Self {
        self.bar_ratio = ratio;
        self
    }

    pub fn annotate(mut self, index: usize, label: impl Into<SharedString>) -> Self {
        self.annotate = Some((index, label.into()));
        self
    }

    /// Hovering a bar dims the others and shows `tooltip(index)` over it; `on_hover` keeps the hovered bar.
    pub fn hover(
        mut self,
        hovered: Option<usize>,
        tooltip: impl Fn(usize) -> SharedString + 'static,
        on_hover: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.hovered = hovered;
        self.tooltip = Some(Rc::new(tooltip));
        self.on_hover = Some(Rc::new(on_hover));
        self
    }

    /// Small text under the chart's trailing edge (「毫秒以內」).
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }
}

impl RenderOnce for BarChart {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let max = self.values.iter().copied().fold(0., f64::max);
        let step = nice_ceil(max / 2.);
        let top = step * 2.;
        let label_h = 18.;
        let annotation_h = 16.;
        let plot_h = (self.height - label_h - annotation_h).max(40.);
        let axis_w = 40.;
        let ticks = [0., step, top];
        let grid = div().absolute().left_0().right_0().top(px(annotation_h)).h(px(plot_h)).children(ticks.iter().map(|tick| {
            let y = plot_h * (1. - (*tick / top) as f32);
            div().absolute().left_0().right_0().top(px(y)).h(px(1.)).bg(p.separator)
        }));
        let axis = div().w(px(axis_w)).flex_shrink_0().relative().h(px(annotation_h + plot_h)).children(ticks.iter().map(|tick| {
            let y = annotation_h + plot_h * (1. - (*tick / top) as f32);
            div()
                .absolute()
                .left_0()
                .w(px(axis_w - 6.))
                .top(px(y - 7.))
                .text_right()
                .text_size(text::CAPTION)
                .text_color(p.text_tertiary)
                .child(grouped(*tick as i64))
        }));
        let count = self.values.len().max(1);
        let hovered = self.hovered;
        let bars = h().absolute().left_0().right_0().top(px(annotation_h)).h(px(plot_h)).items_end().children(
            self.values.iter().enumerate().map(|(index, value)| {
                let bar_h = (plot_h * (*value / top) as f32).max(if *value > 0. { 2. } else { 0. });
                let dim = hovered.is_some_and(|h| h != index);
                let on_hover = self.on_hover.clone();
                let tip = (hovered == Some(index)).then(|| self.tooltip.as_ref().map(|f| f(index))).flatten();
                let note = (hovered.is_none() && self.annotate.as_ref().is_some_and(|(i, _)| *i == index))
                    .then(|| self.annotate.as_ref().map(|(_, label)| label.clone()))
                    .flatten();
                div()
                    .id(SharedString::from(format!("{}-bar-{index}", self.id)))
                    .flex_1()
                    .h_full()
                    .relative()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .when_some(on_hover, |this, on_hover| {
                        this.on_hover(move |inside, window, cx| on_hover(inside.then_some(index), window, cx))
                    })
                    .child(
                        div()
                            .w(relative(self.bar_ratio))
                            .h(px(bar_h))
                            .rounded_t(px(3.))
                            .bg(p.accent.opacity(if dim { 0.35 } else { 0.85 })),
                    )
                    .when_some(note, |this, note| {
                        this.child(
                            div()
                                .absolute()
                                .bottom(px(bar_h + 2.))
                                .left(px(-30.))
                                .right(px(-30.))
                                .text_center()
                                .text_size(text::CAPTION)
                                .text_color(p.text_secondary)
                                .child(note),
                        )
                    })
                    .when_some(tip, |this, tip| {
                        this.child(
                            div().absolute().bottom(px(bar_h + 4.)).left(px(-80.)).right(px(-80.)).flex().justify_center().child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(5.))
                                    .bg(p.card)
                                    .border_1()
                                    .border_color(p.separator)
                                    .shadow_sm()
                                    .text_size(text::CAPTION)
                                    .text_color(p.text)
                                    .whitespace_nowrap()
                                    .child(tip),
                            ),
                        )
                    })
            }),
        );
        let labels = h().h(px(label_h)).children((0..count).map(|index| {
            let label = self.labels.iter().find(|(i, _)| *i == index).map(|(_, label)| label.clone());
            div().flex_1().h_full().relative().when_some(label, |this, label| {
                this.child(
                    div()
                        .absolute()
                        .top(px(3.))
                        .left(px(-24.))
                        .right(px(-24.))
                        .text_center()
                        .text_size(text::CAPTION)
                        .text_color(p.text_tertiary)
                        .whitespace_nowrap()
                        .child(label),
                )
            })
        }));
        v().w_full()
            .id(self.id.clone())
            .when_some(self.on_hover.clone(), |this, on_hover| {
                this.on_hover(move |inside, window, cx| {
                    if !inside {
                        on_hover(None, window, cx);
                    }
                })
            })
            .child(
                h().items_start()
                    .child(axis)
                    .child(v().flex_1().child(div().relative().h(px(annotation_h + plot_h)).child(grid).child(bars)).child(labels)),
            )
            .when_some(self.caption, |this, caption| {
                this.child(div().w_full().text_right().text_size(text::CAPTION).text_color(p.text_secondary).child(caption))
            })
    }
}

/// Characters by weekday (Sunday first) and hour: one colour, darker for more.
pub fn heat_map(hours: &[i64], cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    const WEEKDAYS: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];
    let top = hours.iter().copied().max().unwrap_or(0).max(1) as f32;
    v().w_full()
        .gap(px(2.))
        .children((0..7).map(|day| {
            h().gap(px(2.))
                .child(div().w(px(14.)).flex_shrink_0().text_size(px(10.)).text_color(p.text_secondary).child(WEEKDAYS[day]))
                .children((0..24).map(|hour| {
                    let value = hours.get(day * 24 + hour).copied().unwrap_or(0);
                    let fill = if value == 0 { p.pressed } else { p.accent.opacity(0.15 + 0.85 * value as f32 / top) };
                    div().flex_1().h(px(11.)).rounded(px(2.)).bg(fill)
                }))
        }))
        .child(h().child(div().w(px(16.)).flex_shrink_0()).children([0, 6, 12, 18].map(|hour| {
            div().flex_1().text_size(px(10.)).text_color(p.text_tertiary).child(hour.to_string())
        })))
}

/// Two parts of a whole as one bar: the first in the accent, the second grey.
pub fn split_bar(first: i64, second: i64, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let total = (first + second).max(1) as f32;
    let share = first as f32 / total;
    h().w(px(160.)).h(px(6.)).gap(px(2.))
        .child(div().h_full().rounded_full().w(relative(share * 0.98)).min_w(px(if first > 0 { 3. } else { 0. })).bg(p.accent.opacity(0.8)))
        .child(div().h_full().flex_1().rounded_full().bg(p.pressed))
}

/// A part of the largest as a short bar (the breakdown rows).
pub fn meter(value: i64, largest: i64, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let share = value as f32 / largest.max(1) as f32;
    h().w(px(140.)).h(px(14.)).child(
        div().h(px(6.)).rounded_full().w(relative(share)).min_w(px(if value > 0 { 3. } else { 0. })).bg(p.accent.opacity(0.75)),
    )
}

/// A list of facts, each with an icon (the ⓘ cards).
pub fn fact_list(facts: &[(IconName, &str)], cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    v().gap(px(9.))
        .children(facts.iter().map(|(icon, fact)| {
            h().items_start()
                .gap(px(9.))
                .child(div().pt(px(2.)).child(Icon::new(*icon).size(px(14.)).text_color(p.accent)))
                .child(div().flex_1().text_size(text::SUBTITLE).text_color(p.text).child(SharedString::from(fact.to_string())))
        }))
        .into_any_element()
}

/// A column that fills its parent's width but adds nothing to its parent's intrinsic width.  The layout engine sizes
/// wrapping rows and some absolute children into the scroll view's intrinsic width, which then grows past the window;
/// page content wrapped in this keeps to the window.
pub fn fill_column() -> gpui_kit::Div {
    div().w(px(0.)).min_w(relative(1.)).flex().flex_col()
}

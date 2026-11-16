//! Statistics components (the 分析 page; any page that shows numbers): open number grids, the recap with its notes,
//! bar charts with a value axis, a weekday × hour heat map, a two-part bar and a single meter.  Numbers stand open on
//! the page between hairlines (no tiles); charts are drawn with plain elements in `accent_fill` (one series, one
//! colour, no legend), as the macOS charts.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use super::{h, v};
use crate::theme::{self, size, space, text, weight};

/// 38126 → 「38,126」.
pub fn grouped(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// One number of a statistics grid: a small icon and title over the number.
pub struct Stat {
    pub icon: IconName,
    pub title: SharedString,
    pub value: SharedString,
}

/// A number with its title for `stat_grid`.
pub fn stat(icon: IconName, title: impl Into<SharedString>, value: impl Into<SharedString>) -> Stat {
    Stat { icon, title: title.into(), value: value.into() }
}

/// Numbers in rows of `columns`, open on the page: hairlines between the columns and between the rows, no fills.  The
/// text lines up with the rows' text.
pub fn stat_grid(columns: usize, stats: Vec<Stat>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let columns = columns.max(1);
    let mut rows: Vec<AnyElement> = Vec::new();
    let mut stats = stats.into_iter().peekable();
    let mut index = 0;
    while stats.peek().is_some() {
        let row: Vec<Stat> = stats.by_ref().take(columns).collect();
        let fillers = columns - row.len();
        rows.push(
            h().w_full()
                .items_stretch()
                .when(index > 0, |this| this.border_t_1().border_color(p.separator))
                .children(
                    row.into_iter().map(Some).chain(std::iter::repeat_with(|| None).take(fillers)).enumerate().map(
                        |(column, stat)| {
                            v().flex_1()
                                .min_w(px(0.))
                                .justify_between()
                                .gap(space::S4)
                                .px(size::ROW_PADDING_X)
                                .py(size::ROW_PADDING_Y)
                                .when(column > 0, |this| this.border_l_1().border_color(p.separator))
                                .when_some(stat, |this, stat| {
                                    // The title wraps in a narrow window rather than losing its unit.
                                    this.child(
                                        h().items_start()
                                            .gap(space::S4)
                                            .min_w(px(0.))
                                            .text_size(text::CAPTION)
                                            .text_color(p.text_secondary)
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .h(text::CAPTION * 1.5)
                                                    .flex()
                                                    .items_center()
                                                    .child(Icon::new(stat.icon).size(size::ICON_XS)),
                                            )
                                            .child(div().min_w(px(0.)).child(stat.title)),
                                    )
                                    .child(
                                        div()
                                            .text_size(text::TITLE)
                                            .font_weight(weight::SEMIBOLD)
                                            .text_color(p.text)
                                            .child(stat.value),
                                    )
                                })
                        },
                    ),
                )
                .into_any_element(),
        );
        index += 1;
    }
    fill_column().children(rows)
}

/// A note beside the recap's number: 「連續 3 天」 with its icon, in secondary text.
pub fn info_chip(icon: IconName, label: impl Into<SharedString>, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    h().gap(space::S4)
        .text_size(text::SUBTITLE)
        .text_color(p.text_secondary)
        .whitespace_nowrap()
        .child(Icon::new(icon).size(size::ICON_XS))
        .child(label.into())
        .into_any_element()
}

/// The recap at the top of a statistics page: a caption, a large number with its unit, and notes; open on the page.
pub fn recap_card(
    caption: impl Into<SharedString>,
    number: impl Into<SharedString>,
    unit: impl Into<SharedString>,
    chips: Vec<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    v().w_full()
        .gap(space::S4)
        .px(size::ROW_PADDING_X)
        .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(caption.into()))
        .child(
            h().items_baseline()
                .gap(space::S6)
                .child(
                    div()
                        .text_size(text::DISPLAY)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(p.text)
                        .child(number.into()),
                )
                .child(div().text_size(text::HEADLINE).text_color(p.text_secondary).child(unit.into())),
        )
        .when(!chips.is_empty(), |this| {
            this.child(
                div()
                    .pt(space::S4)
                    .w(px(0.))
                    .min_w(relative(1.))
                    .child(h().w_full().flex_wrap().gap_x(space::S16).gap_y(space::S4).children(chips)),
            )
        })
}

/// The bar chart's own metrics: the labels under the bars, the room for a number over the best bar, the value axis.
const LABEL_H: f32 = 18.;
const ANNOTATION_H: f32 = 16.;
const PLOT_MIN_H: f32 = 40.;
const AXIS_W: f32 = 40.;

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
        let label_h = LABEL_H;
        let annotation_h = ANNOTATION_H;
        let plot_h = (self.height - label_h - annotation_h).max(PLOT_MIN_H);
        let axis_w = AXIS_W;
        let ticks = [0., step, top];
        let grid = div().absolute().left_0().right_0().top(px(annotation_h)).h(px(plot_h)).children(ticks.iter().map(
            |tick| {
                let y = plot_h * (1. - (*tick / top) as f32);
                div().absolute().left_0().right_0().top(px(y)).h(size::HAIRLINE).bg(p.separator)
            },
        ));
        let axis = div().w(px(axis_w)).flex_shrink_0().relative().h(px(annotation_h + plot_h)).children(
            ticks.iter().map(|tick| {
                let y = annotation_h + plot_h * (1. - (*tick / top) as f32);
                div()
                    .absolute()
                    .left_0()
                    .w(px(axis_w) - space::S6)
                    .top(px(y) - text::CAPTION * 0.6)
                    .text_right()
                    .text_size(text::CAPTION)
                    .text_color(p.text_tertiary)
                    .child(grouped(*tick as i64))
            }),
        );
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
                            .rounded_t(space::S2)
                            .bg(p.accent_fill.opacity(if dim { 0.35 } else { 0.85 })),
                    )
                    .when_some(note, |this, note| {
                        this.child(
                            div()
                                .absolute()
                                .bottom(px(bar_h) + space::S2)
                                .left(-space::S32)
                                .right(-space::S32)
                                .text_center()
                                .text_size(text::CAPTION)
                                .text_color(p.text_secondary)
                                .child(note),
                        )
                    })
                    .when_some(tip, |this, tip| {
                        this.child(
                            div()
                                .absolute()
                                .bottom(px(bar_h) + space::S4)
                                .left(-space::S40 * 2.)
                                .right(-space::S40 * 2.)
                                .flex()
                                .justify_center()
                                .child(
                                    div()
                                        .px(space::S6)
                                        .py(space::S2)
                                        .rounded(size::R_SM)
                                        .bg(p.panel)
                                        .border_1()
                                        .border_color(p.panel_border)
                                        .shadow(theme::elevation::floating(&p))
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
                        .top(space::S4)
                        .left(-space::S24)
                        .right(-space::S24)
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
            .child(h().items_start().child(axis).child(
                v().flex_1().child(div().relative().h(px(annotation_h + plot_h)).child(grid).child(bars)).child(labels),
            ))
            .when_some(self.caption, |this, caption| {
                this.child(
                    div().w_full().text_right().text_size(text::CAPTION).text_color(p.text_secondary).child(caption),
                )
            })
    }
}

/// The heat map's weekday column and cell height.
const HEAT_LABEL_W: gpui_kit::Pixels = px(16.);
const HEAT_CELL_H: gpui_kit::Pixels = px(14.);

/// Characters by weekday (Sunday first) and hour: one colour, darker for more.
pub fn heat_map(hours: &[i64], cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    const WEEKDAYS: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];
    let top = hours.iter().copied().max().unwrap_or(0).max(1) as f32;
    v().w_full()
        .gap(space::S2)
        .children((0..7).map(|day| {
            h().gap(space::S2)
                .child(
                    div()
                        .w(HEAT_LABEL_W)
                        .flex_shrink_0()
                        .text_size(text::CAPTION)
                        .line_height(HEAT_CELL_H)
                        .text_color(p.text_secondary)
                        .child(WEEKDAYS[day]),
                )
                .children((0..24).map(|hour| {
                    let value = hours.get(day * 24 + hour).copied().unwrap_or(0);
                    let fill =
                        if value == 0 { p.pressed } else { p.accent_fill.opacity(0.15 + 0.85 * value as f32 / top) };
                    div().flex_1().h(HEAT_CELL_H).rounded(space::S2).bg(fill)
                }))
        }))
        .child(h().pt(space::S2).child(div().w(HEAT_LABEL_W + space::S2).flex_shrink_0()).children(
            [0, 6, 12, 18].map(|hour| {
                div().flex_1().text_size(text::CAPTION).text_color(p.text_tertiary).child(hour.to_string())
            }),
        ))
}

/// The width and thickness of the small bars in rows.
const BAR_W: gpui_kit::Pixels = px(140.);
const BAR_H: gpui_kit::Pixels = px(6.);

/// Two parts of a whole as one bar: the first in the accent, the second grey.
pub fn split_bar(first: i64, second: i64, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let total = (first + second).max(1) as f32;
    let share = first as f32 / total;
    h().w(BAR_W)
        .h(BAR_H)
        .gap(space::S2)
        .child(
            div()
                .h_full()
                .rounded_full()
                .w(relative(share * 0.98))
                .min_w(if first > 0 { BAR_H } else { px(0.) })
                .bg(p.accent_fill.opacity(0.8)),
        )
        .child(div().h_full().flex_1().rounded_full().bg(p.pressed))
}

/// A part of the largest as a short bar (the breakdown rows).
pub fn meter(value: i64, largest: i64, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let share = value as f32 / largest.max(1) as f32;
    h().w(BAR_W).h(size::ICON_SM).child(
        div()
            .h(BAR_H)
            .rounded_full()
            .w(relative(share))
            .min_w(if value > 0 { BAR_H } else { px(0.) })
            .bg(p.accent_fill.opacity(0.75)),
    )
}

/// A list of facts, each with an icon (the ⓘ cards).
pub fn fact_list(facts: &[(IconName, &str)], cx: &App) -> AnyElement {
    super::content::facts(facts.iter().map(|(icon, fact)| {
        super::content::fact_row(*icon, SharedString::from(fact.to_string()), cx).into_any_element()
    }))
    .into_any_element()
}

/// A column that fills its parent's width but adds nothing to its parent's intrinsic width.  The layout engine sizes
/// wrapping rows and some absolute children into the scroll view's intrinsic width, which then grows past the window;
/// page content wrapped in this keeps to the window.
pub fn fill_column() -> gpui_kit::Div {
    div().w(px(0.)).min_w(relative(1.)).flex().flex_col()
}

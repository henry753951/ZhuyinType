//! Small content blocks of the 字詞, 進階 and 關於 pages: the facts of an ⓘ card, labelled fields and suggestion
//! chips of a sheet, a row's group of buttons, the app's header on 關於, and an expandable row (macOS
//! `DisclosureGroup`).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, ElementId, Entity, InteractiveElement, IntoElement, ListState, ParentElement,
    Pixels, SharedString, StatefulInteractiveElement, Styled, Window, div, img, px,
};

use super::{h, v};
use crate::theme::{self, size, space, text, weight};

/// The space between the facts of an ⓘ card, and between a sheet's fields.
pub const FACT_GAP: Pixels = space::S8;

/// One fact of an ⓘ card: a monochrome line icon and a sentence (macOS `FactRow`).  Every list of facts uses this.
pub fn fact_row(icon: IconName, line: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let line_h = text::SUBTITLE * 1.5;
    h().items_start()
        .gap(space::S8)
        .child(
            div()
                .flex_shrink_0()
                .h(line_h)
                .flex()
                .items_center()
                .child(Icon::new(icon).size(size::ICON_SM).text_color(p.text_secondary)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(text::SUBTITLE)
                .line_height(line_h)
                .text_color(p.text)
                .child(line.into()),
        )
}

/// Facts in a column.
pub fn facts(rows: impl IntoIterator<Item = AnyElement>) -> Div {
    v().gap(FACT_GAP).children(rows)
}

/// A sheet's text field, `H_FIELD` tall.
pub fn text_input(state: &Entity<InputState>) -> Input {
    Input::new(state).h(size::H_FIELD)
}

/// A row of a section that is a title and buttons (「3 筆記錄」 顯示 匯出 清除): the buttons wrap under each other's
/// line when the window is narrow instead of pushing the card wider.
pub fn action_row(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    buttons: impl IntoIterator<Item = AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    h().id(id)
        .w_full()
        .min_h(size::ROW_MIN)
        .px(size::ROW_PADDING_X)
        .py(size::ROW_PADDING_Y - space::S4)
        .gap(size::ROW_GAP)
        .child(div().flex_shrink_0().text_size(text::BODY).text_color(p.text).child(title.into()))
        .child(h().flex_1().min_w(px(0.)).flex_wrap().justify_end().gap(space::S8).children(buttons))
}

/// A sheet's field: a trailing-aligned label, then the control.
pub fn labelled(label: &'static str, control: impl IntoElement, cx: &App) -> Div {
    let p = theme::palette(cx);
    h().gap(space::S8)
        .child(
            div()
                .w(LABEL_W)
                .flex_shrink_0()
                .text_right()
                .text_size(text::BODY)
                .text_color(p.text_secondary)
                .child(label),
        )
        .child(div().flex_1().min_w(px(0.)).child(control))
}

const LABEL_W: Pixels = px(52.);

/// Content under the fields of `labelled` (aligned with the controls).
pub fn under_fields(content: impl IntoElement) -> Div {
    div().pl(LABEL_W + space::S8).child(content)
}

/// A suggestion to pick (a reading of the word being added): a quiet chip, the selected one in the neutral selection.
pub fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    h().id(id)
        .h(size::H_SM)
        .px(space::S8)
        .rounded(size::R_SM)
        .border_1()
        .border_color(if selected { p.text.opacity(0.3) } else { p.control_border })
        .bg(if selected { p.selection } else { p.control })
        .text_size(text::SUBTITLE)
        .text_color(p.text)
        .cursor_pointer()
        .hover(|style| style.bg(p.hover))
        .on_click(on_click)
        .child(label.into())
}

/// Chips that wrap.
pub fn chips(children: impl IntoIterator<Item = AnyElement>) -> Div {
    h().gap(space::S6).flex_wrap().children(children)
}

/// The app's icon, name and version, centred (關於).
pub fn app_header(name: &'static str, version: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    v().w_full()
        .items_center()
        .gap(space::S4)
        .py(space::S16)
        .child(img("app-icon.png").size(APP_ICON).rounded(APP_ICON_RADIUS))
        .child(div().pt(space::S8).text_size(text::TITLE).font_weight(weight::SEMIBOLD).text_color(p.text).child(name))
        .child(div().text_size(text::BODY).text_color(p.text_secondary).child(version.into()))
}

/// The app icon on 關於 and its corner (the icon's own shape, the one rounded thing outside the radius scale).
const APP_ICON: Pixels = px(80.);
const APP_ICON_RADIUS: Pixels = px(18.);

/// A row that opens to show more (macOS `DisclosureGroup`): a title and subtitle with a turning chevron; `body` under
/// it while open.
pub fn disclosure(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    open: bool,
    body: Option<AnyElement>,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    v().w_full()
        .min_w(px(0.))
        .child(
            h().id(id)
                .group("disclosure")
                .relative()
                .w_full()
                .min_w(px(0.))
                .min_h(size::ROW_MIN)
                .px(size::ROW_PADDING_X)
                .py(size::ROW_PADDING_Y)
                .gap(space::S8)
                .cursor_pointer()
                .on_click(on_toggle)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(space::S2)
                        .bottom(space::S2)
                        .rounded(size::R_SM)
                        .group_hover("disclosure", |style| style.bg(p.hover)),
                )
                .child(
                    Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight })
                        .size(size::ICON_SM)
                        .text_color(p.text_tertiary),
                )
                .child(
                    v().flex_1()
                        .min_w(px(0.))
                        .gap(space::S2)
                        .child(div().w_full().text_size(text::BODY).text_color(p.text).child(title.into()))
                        .child(
                            div()
                                .w_full()
                                .text_size(text::SUBTITLE)
                                .text_color(p.text_secondary)
                                .child(subtitle.into()),
                        ),
                ),
        )
        .when_some(body.filter(|_| open), |this, body| {
            this.child(
                v().w_full()
                    .pl(size::ROW_PADDING_X + size::ICON_SM + space::S8)
                    .pr(size::ROW_PADDING_X)
                    .pb(space::S12)
                    .gap(space::S6)
                    .text_size(text::SUBTITLE)
                    .text_color(p.text_secondary)
                    .child(body),
            )
        })
}

/// The monospaced font of licence texts, per platform.
pub fn mono_font() -> &'static str {
    if cfg!(target_os = "windows") { "Consolas" } else { "DejaVu Sans Mono" }
}

/// A document sheet laid out for a dialog: its width and the element.
pub struct DocumentFrame {
    pub width: Pixels,
    pub element: AnyElement,
}

const DOCUMENT_W: Pixels = px(680.);
const DOCUMENT_H: Pixels = px(620.);
const DOCUMENT_HEADER_H: Pixels = px(62.);
const DOCUMENT_FOOTER_H: Pixels = px(56.);
const DOCUMENT_MIN_H: Pixels = px(300.);

/// The document viewer's frame (macOS `DocumentSheet`): name and owner over a hairline, the scrolling `body`, a
/// hairline, then `buttons` on the leading side and `done` on the trailing one.
#[allow(clippy::too_many_arguments)]
pub fn document_frame(
    name: SharedString,
    entry: SharedString,
    markdown: bool,
    body: AnyElement,
    scroll: &ListState,
    buttons: Vec<AnyElement>,
    done: AnyElement,
    window: &Window,
    cx: &App,
) -> DocumentFrame {
    let p = theme::palette(cx);
    let viewport = window.viewport_size();
    let height = DOCUMENT_H.min(viewport.height * 0.86).max(DOCUMENT_MIN_H);
    let body_h = height - DOCUMENT_HEADER_H - DOCUMENT_FOOTER_H - size::HAIRLINE * 2.;
    let element = v()
        .w_full()
        .h(height)
        .child(
            v().h(DOCUMENT_HEADER_H)
                .justify_center()
                .px(space::S20)
                .gap(space::S2)
                .child(
                    div()
                        .text_size(text::HEADLINE)
                        .font_weight(weight::SEMIBOLD)
                        .text_color(p.text)
                        .truncate()
                        .child(name),
                )
                .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(entry)),
        )
        .child(div().h(size::HAIRLINE).w_full().bg(p.separator))
        .child(
            div()
                .relative()
                .h(body_h)
                .w_full()
                .bg(p.card)
                .child(
                    div()
                        .size_full()
                        .px(space::S16)
                        .py(space::S12)
                        .when(!markdown, |this| this.font_family(mono_font()).text_size(text::CAPTION))
                        .when(markdown, |this| this.text_size(text::SUBTITLE))
                        .text_color(p.text)
                        .child(body),
                )
                .child(Scrollbar::vertical(scroll)),
        )
        .child(div().h(size::HAIRLINE).w_full().bg(p.separator))
        .child(
            h().h(DOCUMENT_FOOTER_H).px(space::S20).gap(space::S8).children(buttons).child(div().flex_1()).child(done),
        )
        .into_any_element();
    DocumentFrame { width: DOCUMENT_W, element }
}

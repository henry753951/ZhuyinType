//! Small content blocks of the 字詞, 進階 and 關於 pages: the facts of an ⓘ card, labelled fields and suggestion
//! chips of a sheet, a row's group of buttons, the app's header on 關於, and an expandable row (macOS
//! `DisclosureGroup`).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, ElementId, FontWeight, InteractiveElement, IntoElement, ListState, ParentElement,
    Pixels, SharedString, StatefulInteractiveElement, Styled, Window, div, img, px,
};

use super::{h, v};
use crate::theme::{self, size, text};

/// The space between the facts of an ⓘ card.
pub const FACT_GAP: Pixels = px(9.);

/// One fact of an ⓘ card: an accent glyph and a line of text (macOS `FactRow`).
pub fn fact_row(icon: IconName, line: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    h().items_start()
        .gap(px(12.))
        .child(div().pt(px(2.)).child(Icon::new(icon).size(px(16.)).text_color(p.accent)))
        .child(div().flex_1().text_size(text::BODY).text_color(p.text).child(line.into()))
}

/// Facts in a column.
pub fn facts(rows: impl IntoIterator<Item = AnyElement>) -> Div {
    v().gap(FACT_GAP).children(rows)
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
        .py(size::ROW_PADDING_Y)
        .gap(px(12.))
        .child(div().flex_shrink_0().text_size(text::BODY).text_color(p.text).child(title.into()))
        .child(h().flex_1().min_w(px(0.)).flex_wrap().justify_end().gap(px(8.)).children(buttons))
}

/// A sheet's field: a trailing-aligned label, then the control.
pub fn labelled(label: &'static str, control: impl IntoElement, cx: &App) -> Div {
    let p = theme::palette(cx);
    h().gap(px(8.))
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
    div().pl(LABEL_W + px(8.)).child(content)
}

/// A suggestion to pick (a reading of the word being added).
pub fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    div()
        .id(id)
        .px(px(8.))
        .py(px(2.))
        .rounded(size::RADIUS_SMALL)
        .border_1()
        .border_color(if selected { p.accent } else { p.control_border })
        .bg(if selected { p.accent_faint } else { p.control })
        .text_size(text::SUBTITLE)
        .text_color(if selected { p.accent } else { p.text })
        .cursor_pointer()
        .hover(|style| style.opacity(0.85))
        .on_click(on_click)
        .child(label.into())
}

/// Chips that wrap.
pub fn chips(children: impl IntoIterator<Item = AnyElement>) -> Div {
    h().gap(px(6.)).flex_wrap().children(children)
}

/// The app's icon, name and version, centred (關於).
pub fn app_header(name: &'static str, version: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    v().w_full()
        .items_center()
        .gap(px(6.))
        .py(px(14.))
        .child(img("app-icon.png").size(px(80.)).rounded(px(18.)))
        .child(div().pt(px(4.)).text_size(text::TITLE).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(name))
        .child(div().text_size(text::BODY).text_color(p.text_secondary).child(version.into()))
}

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
                .w_full()
                .min_w(px(0.))
                .min_h(size::ROW_MIN)
                .px(size::ROW_PADDING_X)
                .py(size::ROW_PADDING_Y)
                .gap(px(10.))
                .cursor_pointer()
                .hover(|style| style.bg(p.hover))
                .active(|style| style.bg(p.pressed))
                .on_click(on_toggle)
                .child(
                    Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight })
                        .size(px(13.))
                        .text_color(p.text_tertiary),
                )
                .child(
                    v().flex_1()
                        .min_w(px(0.))
                        .gap(px(3.))
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
                    .pl(size::ROW_PADDING_X + px(23.))
                    .pr(size::ROW_PADDING_X)
                    .pb(px(12.))
                    .gap(px(6.))
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
    let height = DOCUMENT_H.min(viewport.height * 0.86).max(px(300.));
    let body_h = height - DOCUMENT_HEADER_H - DOCUMENT_FOOTER_H - px(2.);
    let element = v()
        .w_full()
        .h(height)
        .child(
            v().h(DOCUMENT_HEADER_H)
                .justify_center()
                .px(px(20.))
                .gap(px(3.))
                .child(
                    div()
                        .text_size(text::HEADLINE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .truncate()
                        .child(name),
                )
                .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(entry)),
        )
        .child(div().h(px(1.)).w_full().bg(p.separator))
        .child(
            div()
                .relative()
                .h(body_h)
                .w_full()
                .bg(p.card)
                .child(
                    div()
                        .size_full()
                        .px(px(16.))
                        .py(px(10.))
                        .when(!markdown, |this| this.font_family(mono_font()).text_size(text::CAPTION))
                        .when(markdown, |this| this.text_size(text::SUBTITLE))
                        .text_color(p.text)
                        .child(body),
                )
                .child(Scrollbar::vertical(scroll)),
        )
        .child(div().h(px(1.)).w_full().bg(p.separator))
        .child(h().h(DOCUMENT_FOOTER_H).px(px(20.)).gap(px(8.)).children(buttons).child(div().flex_1()).child(done))
        .into_any_element();
    DocumentFrame { width: DOCUMENT_W, element }
}

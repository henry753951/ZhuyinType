//! A feature card (macOS `FeatureCard`): a tinted icon tile and title, a short explanation, and a link at the bottom to
//! where the feature is set.  Laid out by `charts::tile_grid` (two columns, one when narrow).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

use super::{controls, h, v};
use crate::theme::{self, size, text};

/// A card for one feature.
#[allow(clippy::too_many_arguments)]
pub fn feature_card(
    id: impl Into<SharedString>,
    icon: IconName,
    tint: Hsla,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    link: impl Into<SharedString>,
    on_link: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let p = theme::palette(cx);
    let id: SharedString = id.into();
    v().h_full()
        .min_h(px(100.))
        .gap(px(8.))
        .p(px(14.))
        .rounded(size::RADIUS_CARD)
        .bg(p.card)
        .border_1()
        .border_color(p.separator)
        .child(
            h().gap(px(8.))
                .child(
                    div()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.5))
                        .bg(tint)
                        .child(Icon::new(icon).size(px(13.)).text_color(gpui_kit::white())),
                )
                .child(div().text_size(text::HEADLINE).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(title.into())),
        )
        .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(body.into()))
        .child(div().flex_1())
        .child(controls::text_link(SharedString::from(format!("{id}-link")), link, on_link, cx))
        .into_any_element()
}

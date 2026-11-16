//! A feature (macOS `FeatureCard`), open on the page rather than boxed: a monochrome line icon and title, a short
//! explanation, and a link at the bottom to where the feature is set.  Laid out by `feature_grid` (two columns, one
//! when narrow); the text lines up with the rows' text.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{
    AnyElement, App, ClickEvent, IntoElement, ParentElement, SharedString, Styled, Window, div, px, relative,
};

use super::{controls, h, v};
use crate::theme::{self, size, space, text, weight};

/// One feature.
pub fn feature_card(
    id: impl Into<SharedString>,
    icon: IconName,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    link: impl Into<SharedString>,
    on_link: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let p = theme::palette(cx);
    let id: SharedString = id.into();
    v().h_full()
        .gap(space::S6)
        .px(size::ROW_PADDING_X)
        .py(size::ROW_PADDING_Y)
        .child(h().gap(space::S8).child(Icon::new(icon).size(size::ICON_MD).text_color(p.text_secondary)).child(
            div().text_size(text::HEADLINE).font_weight(weight::SEMIBOLD).text_color(p.text).child(title.into()),
        ))
        .child(div().text_size(text::SUBTITLE).text_color(p.text_secondary).child(body.into()))
        .child(div().flex_1())
        .child(controls::text_link(SharedString::from(format!("{id}-link")), link, on_link, cx))
        .into_any_element()
}

/// Features in two columns (one when a column would be narrower than `min_width`), open on the page.
pub fn feature_grid(min_width: f32, features: Vec<AnyElement>) -> impl IntoElement {
    let fillers = features.len() % 2;
    // A zero width with a full minimum keeps the wrapping row out of the page's intrinsic width.
    div().w(px(0.)).min_w(relative(1.)).child(h().w_full().flex_wrap().items_stretch().children(
        features.into_iter().map(Some).chain(std::iter::repeat_with(|| None).take(fillers)).map(move |feature| {
            let cell = div().flex_grow(1.).flex_shrink(1.).flex_basis(px(min_width)).min_w(relative(0.34));
            match feature {
                Some(feature) => cell.flex().flex_col().child(feature),
                // Keeps a last lone feature in its column; takes no room when it wraps alone.
                None => cell.h(px(0.)),
            }
        }),
    ))
}

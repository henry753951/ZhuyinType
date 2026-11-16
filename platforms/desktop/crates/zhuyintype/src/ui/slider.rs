//! A slider between a small and a large sample of what it changes (macOS `Slider` with minimum and maximum value
//! labels): 「字」 small, the track, 「字」 large, for the candidate window's 字級.

use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::{App, Entity, IntoElement, ParentElement, Pixels, SharedString, Styled, div, px};

use super::h;
use crate::theme::{self, space, text};

/// The track's width.
const TRACK_W: Pixels = px(160.);

/// A slider over `state` with `sample` drawn small before it and large after it.
pub fn sample_slider(
    state: &Entity<SliderState>,
    sample: impl Into<SharedString>,
    label: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    let p = theme::palette(cx);
    let sample = sample.into();
    h().gap(space::S8)
        .child(div().text_size(text::CAPTION).text_color(p.text_secondary).child(sample.clone()))
        .child(div().w(TRACK_W).child(Slider::new(state).accessibility_label(label)))
        .child(div().text_size(text::TITLE).text_color(p.text_secondary).child(sample))
}

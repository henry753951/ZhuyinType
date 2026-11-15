//! A slider between a small and a large sample of what it changes (macOS `Slider` with minimum and maximum value
//! labels): 「字」 small, the track, 「字」 large, for the candidate window's 字級.

use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::{App, Entity, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::h;
use crate::theme::{self, text};

/// A slider over `state` with `sample` drawn small before it and large after it.
pub fn sample_slider(state: &Entity<SliderState>, sample: impl Into<SharedString>, label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let p = theme::palette(cx);
    let sample = sample.into();
    h().gap(px(8.))
        .child(div().text_size(text::CAPTION).text_color(p.text_secondary).child(sample.clone()))
        .child(div().w(px(160.)).child(Slider::new(state).accessibility_label(label)))
        .child(div().text_size(text::TITLE).text_color(p.text_secondary).child(sample))
}

//! A minimal candidate window for the 介紹 stage.
//!
//! TODO(candidate panel): the shared candidate window element is being built on another branch; when it lands,
//! `candidate_panel` and `panel_height` call it and this file's drawing goes.  Everything else draws through these
//! two functions only.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, FontWeight, IntoElement, ParentElement, Styled, div, px};
use zt_core::presentation::Presentation;

use super::script::PanelKeys;
use crate::theme::{Palette, text};
use crate::ui::{h, v};

const PADDING: f32 = 6.;
const ROW_H: f32 = 32.;
const HEADER_H: f32 = 30.;
const FOOTER_H: f32 = 32.;
const KEY_W: f32 = 14.;
const CANDIDATE_TEXT: f32 = 18.;

/// From the window's left edge to its candidates' text: the window is placed so that text lines up with the
/// composition, as the real one.
pub const TEXT_INSET: f32 = PADDING + 8. + KEY_W + 10.;

/// The window's natural height for `p` (the stage keeps room for the tallest).
pub fn panel_height(p: &Presentation, keys: &PanelKeys) -> f32 {
    if p.rows.is_empty() {
        return if p.notice.is_some() { 36. } else { 0. };
    }
    let rows = if keys.vertical { p.rows.len() as f32 * ROW_H } else { ROW_H };
    PADDING * 2. + rows + if p.reading.is_some() { HEADER_H } else { 0. } + FOOTER_H
}

fn keycap(label: &str, palette: &Palette) -> impl IntoElement {
    div()
        .h(px(20.))
        .min_w(px(22.))
        .px(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .border_1()
        .border_color(palette.control_border)
        .text_size(text::CAPTION)
        .text_color(palette.text_secondary)
        .child(label.to_owned())
}

/// The candidate window showing `p`, in `palette` (the window's own appearance).
pub fn candidate_panel(p: &Presentation, keys: &PanelKeys, palette: &Palette, _cx: &App) -> AnyElement {
    let surface = v()
        .bg(palette.panel)
        .border_1()
        .border_color(palette.panel_border)
        .rounded(px(12.))
        .shadow_lg()
        .text_color(palette.text);
    if p.rows.is_empty() {
        return surface
            .px(px(12.))
            .h(px(36.))
            .justify_center()
            .text_size(text::BODY)
            .child(p.notice.clone().unwrap_or_default())
            .into_any_element();
    }
    let rows = p.rows.iter().enumerate().map(|(index, row)| {
        let selected = p.selected == Some(index);
        h().h(px(ROW_H))
            .px(px(8.))
            .gap(px(10.))
            .rounded(px(8.))
            .when(selected, |this| this.bg(palette.accent))
            .child(
                div()
                    .w(px(KEY_W))
                    .text_size(text::SUBTITLE)
                    .text_color(if selected { palette.on_accent } else { palette.text_secondary })
                    .child(row.key.map(|k| k.to_uppercase().to_string()).unwrap_or_default()),
            )
            .child(
                div()
                    .text_size(px(CANDIDATE_TEXT))
                    .text_color(if selected { palette.on_accent } else { palette.text })
                    .child(row.text.clone()),
            )
            .when_some(row.annotation.clone(), |this, annotation| {
                this.child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(if selected { palette.on_accent } else { palette.text_tertiary })
                        .child(annotation),
                )
            })
            .when(!selected, |this| this.pr(px(24.)))
    });
    let list = if keys.vertical { v().children(rows) } else { h().children(rows) };
    let switch = |label: &Option<String>, title: &'static str, on: bool| {
        label.clone().map(|label| {
            h().gap(px(5.)).child(keycap(&label, palette)).child(
                div()
                    .text_size(text::SUBTITLE)
                    .text_color(if on { palette.accent } else { palette.text_secondary })
                    .child(title),
            )
        })
    };
    let footer = h()
        .h(px(FOOTER_H))
        .px(px(4.))
        .gap(px(10.))
        .when(keys.switches, |this| {
            this.children(switch(&keys.single, "單字", p.single)).children(switch(
                &keys.corrections,
                "修正",
                p.corrections,
            ))
        })
        .child(div().flex_1())
        .when(p.page_count > 0, |this| {
            this.child(
                h().gap(px(12.))
                    .text_size(text::SUBTITLE)
                    .text_color(palette.text_secondary)
                    .child("‹")
                    .child(div().font_weight(FontWeight::MEDIUM).child(format!("{} / {}", p.page + 1, p.page_count)))
                    .child("›"),
            )
        });
    surface
        .when(keys.vertical, |this| this.min_w(px(236.)))
        .p(px(PADDING))
        .when_some(p.reading.clone(), |this, reading| {
            this.child(
                h().h(px(HEADER_H))
                    .px(px(4.))
                    .gap(px(10.))
                    .when_some(keys.zhuyin.clone(), |this, key| this.child(keycap(&key, palette)))
                    .child(div().text_size(text::SUBTITLE).text_color(palette.text_secondary).child(reading)),
            )
        })
        .child(list)
        .child(footer)
        .into_any_element()
}

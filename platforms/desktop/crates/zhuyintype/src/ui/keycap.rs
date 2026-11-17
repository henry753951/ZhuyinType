//! The one key cap of the app: settings rows, the shortcut fields, demos, the 介紹 stage and the candidate window's
//! footer all draw keys with it.  A cap is a quiet `subtle` fill without a border, radius `R_XS`; a combination is one
//! cap per key (「Ctrl」「Shift」「U」, no 「+」).  Arrows, ↩ and ⌫ are icons, so the caps look the same whatever fonts
//! are installed; ⇧ is written 「Shift」 as Windows and Linux keyboards and menus name it (the ⇧ glyph is the Mac's).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;

use gpui_kit::{App, Div, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use super::h;
use crate::theme::{self, size, space, text, weight};

pub use zt_panel::keys::{Glyph, Piece, gap, is_speck, pieces, prose};

/// The icon that draws `glyph`.
pub fn glyph_icon(glyph: Glyph) -> IconName {
    match glyph {
        Glyph::ArrowUp => IconName::ArrowUp,
        Glyph::ArrowDown => IconName::ArrowDown,
        Glyph::ArrowLeft => IconName::ArrowLeft,
        Glyph::ArrowRight => IconName::ArrowRight,
        Glyph::Return => IconName::CornerDownLeft,
        Glyph::Backspace => IconName::Delete,
    }
}

/// A key's label (`pieces`), `font_size` large in `color`.  A lone punctuation key is drawn larger and the marks
/// that sit high in their line (「`」「'」「^」) are moved down to the line's centre, so the cap reads as that key and
/// not as an empty box.
pub fn key_glyphs(label: &str, font_size: f32, color: Hsla) -> Div {
    if is_speck(label) {
        let size = font_size + zt_panel::keys::SPECK_GROWTH;
        return div()
            .relative()
            .top(px(zt_panel::keys::speck_drop(label, size)))
            .text_size(px(size))
            .line_height(px((font_size * 1.3).round()))
            .text_color(color)
            .child(SharedString::from(label.to_owned()));
    }
    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .text_size(px(font_size))
        .line_height(px((font_size * 1.3).round()))
        .text_color(color);
    pieces(label).into_iter().fold(row, |row, piece| match piece {
        Piece::Text(text) => row.child(SharedString::from(text)),
        Piece::Glyph(glyph) => {
            row.child(Icon::new(glyph_icon(glyph)).size(px(zt_panel::keys::glyph_size(font_size))).text_color(color))
        }
        Piece::Gap => row.child(div().w(px(gap(font_size)))),
    })
}

/// A shortcut's label split into caps: 「Alt+⌫」 → 「Alt」「⌫」, 「⇧↓」 → 「⇧」「↓」, 「Ctrl+⇧U」 → 「Ctrl」「⇧」「U」 (each ⇧
/// cap is drawn 「Shift」).
pub fn split(label: &str) -> Vec<String> {
    let parts: Vec<&str> = label.split('+').collect();
    // A lone 「+」 key (「Ctrl++」) splits into empty parts.
    if parts.iter().all(|part| part.is_empty()) {
        return vec![label.to_owned()];
    }
    let (last, modifiers) = parts.split_last().map_or(("", &[][..]), |(last, rest)| (*last, rest));
    let mut caps: Vec<String> =
        modifiers.iter().filter(|part| !part.is_empty()).map(|part| (*part).to_owned()).collect();
    let mut rest = if last.is_empty() { "+" } else { last };
    while let Some(tail) = rest.strip_prefix('⇧').filter(|tail| !tail.is_empty()) {
        caps.push("⇧".to_owned());
        rest = tail;
    }
    caps.push(rest.to_owned());
    caps
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Height {
    Small,
    Medium,
}

/// One key cap.
#[derive(IntoElement)]
pub struct Keycap {
    label: SharedString,
    height: Height,
    tint: Option<Hsla>,
    /// How much of the tint shows (0…1): a just-recorded key's accent fades back to the plain cap.
    tint_amount: f32,
}

/// A key cap `H_XS` tall.
pub fn keycap(label: impl Into<SharedString>) -> Keycap {
    Keycap { label: label.into(), height: Height::Medium, tint: None, tint_amount: 1. }
}

impl Keycap {
    /// `KEYCAP_SM` tall (the candidate window's footer).
    #[allow(dead_code, reason = "for the candidate window's footer")]
    pub fn small(mut self) -> Self {
        self.height = Height::Small;
        self
    }

    /// Coloured: a conflict (danger) or a key just recorded (accent).
    pub fn tint(mut self, tint: Option<Hsla>) -> Self {
        self.tint = tint;
        self
    }

    /// Only part of the tint (0…1), on its way back to the plain cap.
    pub fn tint_amount(mut self, amount: f32) -> Self {
        self.tint_amount = amount.clamp(0., 1.);
        self
    }
}

impl RenderOnce for Keycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let height = if self.height == Height::Small { size::KEYCAP_SM } else { size::H_XS };
        let amount = self.tint_amount;
        let ink = self.tint.map_or(p.text, |tint| if amount >= 1. { tint } else { p.text.blend(tint.opacity(amount)) });
        let fill = self.tint.map_or(p.subtle, |tint| {
            if amount >= 1. { tint.opacity(0.12) } else { p.subtle.blend(tint.opacity(0.12 * amount)) }
        });
        h().flex_shrink_0()
            .h(height)
            .min_w(height)
            .px(space::S6)
            .justify_center()
            .rounded(size::R_XS)
            .bg(fill)
            .font_weight(weight::REGULAR)
            .whitespace_nowrap()
            .child(key_glyphs(&self.label, f32::from(text::CAPTION), ink))
    }
}

/// The caps of one combination side by side (「⇧」「↓」).
pub fn key_combo(keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Div {
    h().flex_shrink_0().gap(space::S4).children(keys.into_iter().map(keycap))
}

/// One shortcut written as a label (「Alt+⌫」) as its caps, optionally tinted.
pub fn binding(label: &str, tint: Option<Hsla>) -> Div {
    h().flex_shrink_0().gap(space::S4).children(split(label).into_iter().map(move |cap| keycap(cap).tint(tint)))
}

/// A binding whose tint shows only `amount` (0…1) of the way.
pub fn binding_faded(label: &str, tint: Option<Hsla>, amount: f32) -> Div {
    h().flex_shrink_0()
        .gap(space::S4)
        .children(split(label).into_iter().map(move |cap| keycap(cap).tint(tint).tint_amount(amount)))
}

/// Several shortcuts (alternatives) side by side, further apart than the caps of one.
pub fn bindings(labels: impl IntoIterator<Item = impl AsRef<str>>) -> Div {
    h().flex_shrink_0().gap(space::S12).children(labels.into_iter().map(|label| binding(label.as_ref(), None)))
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn splits_modifiers_into_caps() {
        assert_eq!(split("Alt+⌫"), ["Alt", "⌫"]);
        assert_eq!(split("⇧↓"), ["⇧", "↓"]);
        assert_eq!(split("Ctrl+⇧U"), ["Ctrl", "⇧", "U"]);
        assert_eq!(split("PgDn"), ["PgDn"]);
        assert_eq!(split("⇧"), ["⇧"]);
        assert_eq!(split("Ctrl++"), ["Ctrl", "+"]);
    }
}

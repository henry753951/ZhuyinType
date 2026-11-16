//! The one key cap of the app: settings rows, the shortcut fields, demos, the 介紹 stage and the candidate window's
//! footer all draw keys with it.  A cap is a quiet `subtle` fill without a border, radius `R_XS`; a combination is one
//! cap per key (「Ctrl」「Shift」「U」, no 「+」).  Arrows, ↩ and ⌫ are icons, so the caps look the same whatever fonts
//! are installed; ⇧ is written 「Shift」 as Windows and Linux keyboards and menus name it (the ⇧ glyph is the Mac's).

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;

use gpui_kit::{App, Div, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use super::h;
use crate::theme::{self, size, space, text, weight};

/// One piece of a key's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Icon(IconName),
    /// The room between 「Shift」 and the key after it in one cap (「Shift ↓」).
    Gap,
}

/// The room a `Piece::Gap` takes at `font_size`.
pub fn gap(font_size: f32) -> f32 {
    (font_size * 0.3).round()
}

/// A key's label as text and icons: 「⇧↓」 → 「Shift」, a gap, the ↓ icon.
pub fn pieces(label: &str) -> Vec<Piece> {
    let icon = |c: char| match c {
        '↑' => Some(IconName::ArrowUp),
        '↓' => Some(IconName::ArrowDown),
        '←' => Some(IconName::ArrowLeft),
        '→' => Some(IconName::ArrowRight),
        '↩' => Some(IconName::CornerDownLeft),
        '⌫' => Some(IconName::Delete),
        _ => None,
    };
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '⇧' {
            text.push_str("Shift");
            if chars.peek().is_some() {
                pieces.push(Piece::Text(std::mem::take(&mut text)));
                pieces.push(Piece::Gap);
            }
        } else if let Some(name) = icon(c) {
            if !text.is_empty() {
                pieces.push(Piece::Text(std::mem::take(&mut text)));
            }
            pieces.push(Piece::Icon(name));
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    pieces
}

/// A lone punctuation key (「`」, 「'」, 「;」) is a speck at a key cap's text size: it is drawn this much larger.
const SPECK_GROWTH: f32 = 7.;

/// Whether `label` is one punctuation key, drawn larger (`SPECK_GROWTH`).
pub fn is_speck(label: &str) -> bool {
    let mut chars = label.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_punctuation())
}

/// A key's label for running text, as Windows and Linux write shortcuts: 「⇧←」 → 「Shift+←」, 「Ctrl+⇧U」 →
/// 「Ctrl+Shift+U」.
pub fn prose(label: &str) -> String {
    let mut text = String::new();
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '⇧' {
            text.push_str(if chars.peek().is_some() { "Shift+" } else { "Shift" });
        } else {
            text.push(c);
        }
    }
    text
}

/// A key's label (`pieces`), `font_size` large in `color`.  A lone punctuation key is drawn larger and the marks
/// that sit high in their line (「`」「'」「^」) are moved down to the line's centre, so the cap reads as that key and
/// not as an empty box.
pub fn key_glyphs(label: &str, font_size: f32, color: Hsla) -> Div {
    if is_speck(label) {
        let size = font_size + SPECK_GROWTH;
        let high = matches!(label, "`" | "'" | "\"" | "^" | "~" | "*");
        return div()
            .relative()
            .top(px(if high { (size * 0.3).round() } else { 0. }))
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
        Piece::Icon(name) => row.child(Icon::new(name).size(px((font_size * 0.95).round())).text_color(color)),
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
}

/// A key cap `H_XS` tall.
pub fn keycap(label: impl Into<SharedString>) -> Keycap {
    Keycap { label: label.into(), height: Height::Medium, tint: None }
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
}

impl RenderOnce for Keycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(cx);
        let height = if self.height == Height::Small { size::KEYCAP_SM } else { size::H_XS };
        let ink = self.tint.unwrap_or(p.text);
        h().flex_shrink_0()
            .h(height)
            .min_w(height)
            .px(space::S6)
            .justify_center()
            .rounded(size::R_XS)
            .bg(self.tint.map_or(p.subtle, |tint| tint.opacity(0.12)))
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

/// Several shortcuts (alternatives) side by side, further apart than the caps of one.
pub fn bindings(labels: impl IntoIterator<Item = impl AsRef<str>>) -> Div {
    h().flex_shrink_0().gap(space::S12).children(labels.into_iter().map(|label| binding(label.as_ref(), None)))
}

#[cfg(test)]
mod tests {
    use super::{Piece, pieces, split};
    use gpui_kit::assets::IconName;

    #[test]
    fn prose_names_shift() {
        assert_eq!(super::prose("⇧←"), "Shift+←");
        assert_eq!(super::prose("Ctrl+⇧U"), "Ctrl+Shift+U");
        assert_eq!(super::prose("⇧"), "Shift");
        assert_eq!(super::prose("`"), "`");
    }

    #[test]
    fn shift_is_a_word() {
        assert_eq!(pieces("⇧"), [Piece::Text("Shift".into())]);
        assert_eq!(pieces("⇧↓"), [Piece::Text("Shift".into()), Piece::Gap, Piece::Icon(IconName::ArrowDown)]);
        assert_eq!(pieces("Ctrl"), [Piece::Text("Ctrl".into())]);
        assert_eq!(pieces("⇧U"), [Piece::Text("Shift".into()), Piece::Gap, Piece::Text("U".into())]);
    }

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

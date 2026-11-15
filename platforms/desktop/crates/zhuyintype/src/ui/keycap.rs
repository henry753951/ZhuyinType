//! The one key cap of the app: settings rows, the shortcut fields, demos, the 介紹 stage and the candidate window's
//! footer all draw keys with it.  A cap is a quiet `subtle` fill without a border, radius `R_XS`; a combination is one
//! cap per key (「Ctrl」「⇧」「U」, no 「+」).  Symbols that every system draws differently (⇧ ↑ ↓ ← → ↩ ⌫) are icons,
//! so the caps look the same whatever fonts are installed.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;

use gpui_kit::{App, Div, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use super::h;
use crate::theme::{self, size, space, text, weight};

/// A key's label with its symbols as icons, the rest as text, `font_size` large in `color`.
pub fn key_glyphs(label: &str, font_size: f32, color: Hsla) -> Div {
    let icon = |c: char| match c {
        '⇧' => Some(IconName::ArrowBigUp),
        '↑' => Some(IconName::ArrowUp),
        '↓' => Some(IconName::ArrowDown),
        '←' => Some(IconName::ArrowLeft),
        '→' => Some(IconName::ArrowRight),
        '↩' => Some(IconName::CornerDownLeft),
        '⌫' => Some(IconName::Delete),
        _ => None,
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .text_size(px(font_size))
        .line_height(px((font_size * 1.3).round()))
        .text_color(color);
    let mut text = String::new();
    for c in label.chars() {
        match icon(c) {
            Some(name) => {
                if !text.is_empty() {
                    row = row.child(SharedString::from(std::mem::take(&mut text)));
                }
                row = row.child(Icon::new(name).size(px((font_size * 0.95).round())).text_color(color));
            }
            None => text.push(c),
        }
    }
    if !text.is_empty() {
        row = row.child(SharedString::from(text));
    }
    row
}

/// A shortcut's label split into caps: 「Alt+⌫」 → 「Alt」「⌫」, 「⇧↓」 → 「⇧」「↓」, 「Ctrl+⇧U」 → 「Ctrl」「⇧」「U」.
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

//! How the window writes keys: arrows, ↩ and ⌫ as glyphs drawn the same everywhere, ⇧ as the word 「Shift」 (the
//! ⇧ symbol is the Mac's), a lone punctuation key larger so its cap reads as that key.

/// A key symbol drawn as a line glyph rather than text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Return,
    Backspace,
}

/// One piece of a key's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Glyph(Glyph),
    /// The room between 「Shift」 and the key after it in one cap (「Shift ↓」).
    Gap,
}

/// The room a [`Piece::Gap`] takes at `font_size`.
#[must_use]
pub fn gap(font_size: f32) -> f32 {
    (font_size * 0.3).round()
}

/// A glyph's side at `font_size`.
#[must_use]
pub fn glyph_size(font_size: f32) -> f32 {
    (font_size * 0.95).round()
}

/// A key's label as text and glyphs: 「⇧↓」 → 「Shift」, a gap, the ↓ glyph.
#[must_use]
pub fn pieces(label: &str) -> Vec<Piece> {
    let glyph = |c: char| match c {
        '↑' => Some(Glyph::ArrowUp),
        '↓' => Some(Glyph::ArrowDown),
        '←' => Some(Glyph::ArrowLeft),
        '→' => Some(Glyph::ArrowRight),
        '↩' => Some(Glyph::Return),
        '⌫' => Some(Glyph::Backspace),
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
        } else if let Some(glyph) = glyph(c) {
            if !text.is_empty() {
                pieces.push(Piece::Text(std::mem::take(&mut text)));
            }
            pieces.push(Piece::Glyph(glyph));
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    pieces
}

/// A key's label for running text, as Windows and Linux write shortcuts: 「⇧←」 → 「Shift+←」, 「Ctrl+⇧U」 →
/// 「Ctrl+Shift+U」.
#[must_use]
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

/// How much larger a lone punctuation key is drawn.
pub const SPECK_GROWTH: f32 = 7.;

/// Whether `label` is one punctuation key (a speck at a cap's text size: drawn [`SPECK_GROWTH`] larger).
#[must_use]
pub fn is_speck(label: &str) -> bool {
    let mut chars = label.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_punctuation())
}

/// How far a lone punctuation mark that sits high in its line (「`」「'」「^」) moves down to the line's centre, at
/// its drawn size; 0 for the others.
#[must_use]
pub fn speck_drop(label: &str, size: f32) -> f32 {
    if matches!(label, "`" | "'" | "\"" | "^" | "~" | "*") { (size * 0.3).round() } else { 0. }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_is_a_word() {
        assert_eq!(pieces("⇧"), [Piece::Text("Shift".into())]);
        assert_eq!(pieces("⇧↓"), [Piece::Text("Shift".into()), Piece::Gap, Piece::Glyph(Glyph::ArrowDown)]);
        assert_eq!(pieces("Ctrl"), [Piece::Text("Ctrl".into())]);
        assert_eq!(pieces("⇧U"), [Piece::Text("Shift".into()), Piece::Gap, Piece::Text("U".into())]);
    }

    #[test]
    fn prose_names_shift() {
        assert_eq!(prose("⇧←"), "Shift+←");
        assert_eq!(prose("Ctrl+⇧U"), "Ctrl+Shift+U");
        assert_eq!(prose("⇧"), "Shift");
        assert_eq!(prose("`"), "`");
    }

    #[test]
    fn specks() {
        assert!(is_speck("`") && is_speck(";") && !is_speck("Tab") && !is_speck("中"));
        assert!(speck_drop("`", 18.) > 0. && speck_drop(";", 18.) == 0.);
    }
}

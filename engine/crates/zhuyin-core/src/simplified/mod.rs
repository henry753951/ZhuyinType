//! 簡體 output: the composition, the candidates and the committed text are shown in simplified characters while the
//! engine keeps converting, learning and matching in traditional ones (only what reaches the screen changes).
//!
//! The table is `OpenCC`'s `t2s` applied to one character at a time (`tools/simplified_table.py`, `table.txt`):
//! a character with a different simplified form is replaced by its first one, so 乾 becomes 干 even in 乾坤 —
//! there is no phrase table.  Every pair is one BMP character to one BMP character, so [`to_simplified`] never
//! changes a string's length in UTF-16 units or in characters: cursor positions and clause ranges computed on the
//! traditional text stay valid.

use std::sync::OnceLock;

/// The generated pairs, one line each: traditional, then simplified.  Lines starting with `#` are comments.
const TABLE: &str = include_str!("table.txt");

/// The pairs sorted by the traditional character.
fn pairs() -> &'static [(char, char)] {
    static PAIRS: OnceLock<Vec<(char, char)>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let mut pairs: Vec<(char, char)> = TABLE
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let mut chars = line.chars();
                Some((chars.next()?, chars.next()?))
            })
            .collect();
        pairs.sort_unstable_by_key(|pair| pair.0);
        pairs
    })
}

/// The simplified form of one character, or the character itself.
#[must_use]
pub fn simplified_char(c: char) -> char {
    // Everything before the first CJK block is never converted (Latin, Zhuyin, punctuation).
    if (c as u32) < 0x3400 {
        return c;
    }
    let table = pairs();
    table.binary_search_by_key(&c, |pair| pair.0).map_or(c, |i| table[i].1)
}

/// `text` in simplified characters, the same length in characters and in UTF-16 units.
#[must_use]
pub fn to_simplified(text: &str) -> String {
    text.chars().map(simplified_char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_characters() {
        assert_eq!(to_simplified("頭髮發財，臺灣的後來"), "头发发财，台湾的后来");
        assert_eq!(to_simplified("ㄓㄨˋ abc 123"), "ㄓㄨˋ abc 123");
        assert_eq!(to_simplified("中文"), "中文");
    }

    #[test]
    fn keeps_lengths() {
        for &(from, to) in pairs() {
            assert_eq!(from.len_utf16(), 1, "{from}");
            assert_eq!(to.len_utf16(), 1, "{to}");
            assert_ne!(from, to);
        }
        assert!(pairs().len() > 3000);
        assert!(pairs().windows(2).all(|w| w[0].0 < w[1].0), "one entry per character");
    }
}

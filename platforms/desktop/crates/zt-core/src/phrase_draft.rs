//! 選取的字加入自訂詞: the text selected in the focused field, as a custom word to add (the 加入自訂詞 form, filled
//! in), and what is wrong with a word and reading the form is about to add.

use zhuyin_core::engine::valid_phrase;

/// The form's text: the word, the engine's first reading of it, and what keeps it from being added (shown in the
/// form), if anything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhraseDraft {
    pub word: String,
    pub reading: String,
    pub problem: Option<String>,
}

/// Nothing was selected (or the field could not say).
pub const NO_SELECTION: &str = "沒有選取文字";
/// Not 1 to 6 Han characters.
pub const WRONG_LENGTH: &str = "自訂詞需為 1 到 6 個中文字";
/// No reading (the lexicon knows none, or the field was emptied).
pub const NO_READING: &str = "請輸入讀音";
/// A syllable is not valid Zhuyin.
pub const INVALID_READING: &str = "讀音需為有效的注音，以空白分隔";

/// The most of a selection an adapter sends (UTF-16 units, IPC `selection`, as macOS reads); longer is refused.
pub const MAX_SELECTION: usize = 32;

/// The reading has more or fewer syllables than the word has characters.
#[must_use]
pub fn wrong_syllables(characters: usize, syllables: usize) -> String {
    format!("讀音的音節數要和字數相同（{characters} 個字，{syllables} 個音節）")
}

/// The word is a custom word already.
#[must_use]
pub fn already_added(word: &str) -> String {
    format!("「{word}」已是自訂詞")
}

fn is_han(c: char) -> bool {
    matches!(u32::from(c),
        0x3007 | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2EBEF | 0x30000..=0x323AF)
}

/// What keeps `word` with `reading` from being a custom word, if anything (the same words as macOS).
#[must_use]
pub fn check(word: &str, reading: &str) -> Option<String> {
    let characters = word.chars().count();
    if !(1..=6).contains(&characters) || !word.chars().all(is_han) {
        return Some(WRONG_LENGTH.to_owned());
    }
    let syllables: Vec<&str> = reading.split_whitespace().collect();
    if syllables.is_empty() {
        return Some(NO_READING.to_owned());
    }
    if syllables.len() != characters {
        return Some(wrong_syllables(characters, syllables.len()));
    }
    (!valid_phrase(word, &syllables.join(" "))).then(|| INVALID_READING.to_owned())
}

/// The form for `selection`: the word trimmed, the first of `readings(word)` as its reading, and its problem;
/// `added(word, reading)`: it is a custom word already.
#[must_use]
pub fn draft(
    selection: &str,
    readings: impl FnOnce(&str) -> Vec<String>,
    added: impl FnOnce(&str, &str) -> bool,
) -> PhraseDraft {
    let word = selection.trim();
    if word.is_empty() {
        return PhraseDraft { problem: Some(NO_SELECTION.to_owned()), ..PhraseDraft::default() };
    }
    let reading = if check(word, "").is_some_and(|problem| problem != WRONG_LENGTH) {
        readings(word).into_iter().next().unwrap_or_default()
    } else {
        String::new()
    };
    let problem = check(word, &reading).or_else(|| added(word, &reading).then(|| already_added(word)));
    PhraseDraft { word: word.to_owned(), problem, reading }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_and_problems() {
        let readings = |word: &str| if word == "台南" { vec!["ㄊㄞˊ ㄋㄢˊ".to_owned()] } else { Vec::new() };
        let new = |_: &str, _: &str| false;
        let problem = |text: &str| draft(text, readings, new).problem;
        assert_eq!(
            draft(" 台南\n", readings, new),
            PhraseDraft { word: "台南".into(), reading: "ㄊㄞˊ ㄋㄢˊ".into(), problem: None }
        );
        assert_eq!(problem("").as_deref(), Some(NO_SELECTION));
        assert_eq!(problem("台南市政府大樓").as_deref(), Some(WRONG_LENGTH));
        assert_eq!(problem("hello").as_deref(), Some(WRONG_LENGTH));
        assert_eq!(problem("曉明").as_deref(), Some(NO_READING));
        assert_eq!(draft("台南", readings, |_, _| true).problem.as_deref(), Some("「台南」已是自訂詞"));
        assert_eq!(check("台南", "ㄊㄞˊ").as_deref(), Some("讀音的音節數要和字數相同（2 個字，1 個音節）"));
        assert_eq!(check("台南", "ㄊㄞˊ abc").as_deref(), Some(INVALID_READING));
        assert_eq!(check("台南", "ㄊㄞˊ  ㄋㄢˊ"), None);
    }
}

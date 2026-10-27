//! The standard Zhuyin keyboard layout and the punctuation keys (port of `Keyboard.swift`, `ZhuyinKeyboard`).
//!
//! Lookups are tables indexed by ASCII byte instead of hash maps: every key the composer sees is ASCII.

use crate::api::{KeyInput, PunctuationKeys};

/// Keys of the standard layout, in the order of [`SYMBOL_ORDER`].
const SYMBOL_KEYS: &str = "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-";
/// The Zhuyin symbol on each key of [`SYMBOL_KEYS`].
const SYMBOL_ORDER: &str = "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ";

const fn symbol_table() -> [char; 128] {
    let keys = SYMBOL_KEYS.as_bytes();
    // The symbols are all three-byte UTF-8 in U+3105..=U+3129.
    let symbols = SYMBOL_ORDER.as_bytes();
    let mut table = ['\0'; 128];
    let mut i = 0;
    while i < keys.len() {
        let b = &symbols;
        let scalar =
            ((b[i * 3] as u32 & 0x0f) << 12) | ((b[i * 3 + 1] as u32 & 0x3f) << 6) | (b[i * 3 + 2] as u32 & 0x3f);
        table[keys[i] as usize] = match char::from_u32(scalar) {
            Some(c) => c,
            None => panic!("bad symbol"),
        };
        i += 1;
    }
    table
}

static SYMBOLS: [char; 128] = symbol_table();

/// The Zhuyin symbol a key types, or `None` for a key that is not a symbol key.
#[must_use]
#[inline]
pub fn symbol(key: char) -> Option<char> {
    let index = key as usize;
    if index < 128 && SYMBOLS[index] != '\0' { Some(SYMBOLS[index]) } else { None }
}

/// Every (key, symbol) pair of the layout, in layout order.
pub fn symbol_pairs() -> impl Iterator<Item = (char, char)> {
    SYMBOL_KEYS.chars().zip(SYMBOL_ORDER.chars())
}

/// The tone mark a tone key types (`6` ˊ, `3` ˇ, `4` ˋ, `7` ˙).
#[must_use]
#[inline]
pub fn tone(key: char) -> Option<&'static str> {
    match key {
        '6' => Some("ˊ"),
        '3' => Some("ˇ"),
        '4' => Some("ˋ"),
        '7' => Some("˙"),
        _ => None,
    }
}

/// The key that types a tone mark.
#[must_use]
#[inline]
pub fn key_for_tone(mark: char) -> Option<char> {
    match mark {
        'ˊ' => Some('6'),
        'ˇ' => Some('3'),
        'ˋ' => Some('4'),
        '˙' => Some('7'),
        _ => None,
    }
}

/// The key that types a Zhuyin symbol.
#[must_use]
pub fn key_for_symbol(symbol: char) -> Option<char> {
    symbol_pairs().find(|&(_, s)| s == symbol).map(|(key, _)| key)
}

/// A tone mark (ˊ ˇ ˋ ˙).
#[must_use]
#[inline]
pub fn is_tone_mark(c: char) -> bool {
    matches!(c, 'ˊ' | 'ˇ' | 'ˋ' | '˙')
}

/// Punctuation typed by an unshifted key.
#[must_use]
pub fn punctuation(key: char) -> Option<&'static str> {
    match key {
        '[' => Some("「"),
        ']' => Some("」"),
        '\'' => Some("、"),
        '\\' => Some("＼"),
        '=' => Some("＝"),
        _ => None,
    }
}

/// Chinese punctuation typed by Shift with a key (the macOS punctuation keys).
#[must_use]
pub fn shifted_punctuation(key: char) -> Option<&'static str> {
    Some(match key {
        ',' => "，",
        '.' => "。",
        '/' => "？",
        ';' => "：",
        '\'' => "；",
        '1' => "！",
        '2' => "＠",
        '3' => "＃",
        '4' => "＄",
        '5' => "％",
        '6' => "…",
        '7' => "＆",
        '8' => "＊",
        '9' => "（",
        '0' => "）",
        '-' => "——",
        '=' => "＋",
        '[' => "『",
        ']' => "』",
        '\\' => "｜",
        '`' => "～",
        _ => return None,
    })
}

/// The upper character printed on each symbol key of the US layout.
#[must_use]
pub fn shifted_symbol(key: char) -> Option<char> {
    const KEYS: &str = "1234567890-=[]\\;',./`";
    const SHIFTED: &str = "!@#$%^&*()_+{}|:\"<>?~";
    KEYS.chars().zip(SHIFTED.chars()).find(|&(k, _)| k == key).map(|(_, s)| s)
}

/// Control with a symbol key: Chinese punctuation for the marks Chinese text uses, otherwise the full-width form of
/// the printed symbol.  Control+`[` and Control+`]` stay the application's (Escape in terminals); `[` and `]` type 「」.
/// Control+/ is the application's too (Control+? types ？).
pub fn control_punctuation(key: char) -> Option<String> {
    let chinese = match key {
        ',' => Some("，"),
        '.' => Some("。"),
        '\'' => Some("、"),
        '?' => Some("？"),
        '!' => Some("！"),
        ':' => Some("："),
        ';' => Some("；"),
        '{' => Some("『"),
        '}' => Some("』"),
        '<' => Some("《"),
        '>' => Some("》"),
        '_' => Some("——"),
        '^' => Some("…"),
        _ => None,
    };
    if let Some(text) = chinese {
        return Some(text.to_owned());
    }
    let scalar = key as u32;
    if !(0x21..=0x7e).contains(&scalar) || key.is_ascii_alphanumeric() || matches!(key, '[' | ']' | '/') {
        return None;
    }
    char::from_u32(scalar - 0x21 + 0xff01).map(String::from)
}

/// The slot of a Zhuyin symbol: 0 initial, 1 medial, 2 final.
#[must_use]
#[inline]
pub fn slot(symbol: char) -> Option<usize> {
    match symbol {
        'ㄅ'..='ㄙ' => Some(0),
        'ㄧ' | 'ㄨ' | 'ㄩ' => Some(1),
        'ㄚ'..='ㄦ' => Some(2),
        _ => None,
    }
}

/// Standard plain Zhuyin: the neutral-tone mark before the syllable instead of after it.  Syllables are separated
/// by single spaces (empty parts are dropped, as Swift's `split` does).
#[must_use]
pub fn plain(reading: &str) -> String {
    let mut result = String::with_capacity(reading.len());
    for part in reading.split(' ').filter(|part| !part.is_empty()) {
        if !result.is_empty() {
            result.push(' ');
        }
        match part.strip_suffix('˙') {
            Some(rest) => {
                result.push('˙');
                result.push_str(rest);
            }
            None => result.push_str(part),
        }
    }
    result
}

/// The literal text a key types in Chinese mode, or `None` when the key is not a literal key.
pub fn literal(key: KeyInput, keys: PunctuationKeys) -> Option<String> {
    match key {
        KeyInput::Char(c) => punctuation(c).map(str::to_owned),
        KeyInput::ShiftedChar(c) => {
            if c.is_ascii_alphabetic() {
                return Some(c.to_ascii_uppercase().to_string());
            }
            match keys {
                PunctuationKeys::Windows => shifted_symbol(c).map(String::from),
                PunctuationKeys::Mac => shifted_punctuation(c).map(str::to_owned),
            }
        }
        KeyInput::Printed(c) => Some(c.to_string()),
        KeyInput::ControlChar(c) => control_punctuation(c),
        _ => None,
    }
}

/// Other forms of a punctuation mark, for the candidate window: the mark itself first.
#[must_use]
pub fn alternatives(text: &str) -> Vec<String> {
    const GROUPS: &[&[&str]] = &[
        &["，", ",", "、", "；", "：", "…"],
        &["。", ".", "．", "…"],
        &["？", "?", "！", "!"],
        &["「", "『", "（", "〈", "《", "【"],
        &["」", "』", "）", "〉", "》", "】"],
        &["——", "—", "－", "-", "_", "＿"],
        &["《", "〈", "＜", "<"],
        &["》", "〉", "＞", ">"],
        &[" ", "　"],
        &["＼", "\\", "／", "/"],
        &["＝", "=", "＋", "+"],
        &["‵", "～", "~", "`"],
        &["｜", "|", "／"],
    ];
    let mut result = vec![text.to_owned()];
    if let Some(group) = GROUPS.iter().find(|group| group.contains(&text)) {
        result.extend(group.iter().filter(|&&mark| mark != text).map(|&mark| mark.to_owned()));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_symbol() {
        // ZhuyinCoreChecks: "keyboard \(key) = \(symbol)".
        let pairs: Vec<_> = symbol_pairs().collect();
        assert_eq!(pairs.len(), 37);
        for (key, s) in pairs {
            assert_eq!(symbol(key), Some(s), "keyboard {key} = {s}");
            assert_eq!(key_for_symbol(s), Some(key));
            assert!(slot(s).is_some());
        }
        assert_eq!(symbol('6'), None);
        assert_eq!(symbol('ㄅ'), None);
    }

    #[test]
    fn slots() {
        for s in "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ".chars() {
            assert_eq!(slot(s), Some(0));
        }
        for s in "ㄧㄨㄩ".chars() {
            assert_eq!(slot(s), Some(1));
        }
        for s in "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ".chars() {
            assert_eq!(slot(s), Some(2));
        }
        assert_eq!(slot('ˊ'), None);
        assert_eq!(slot('a'), None);
    }

    #[test]
    fn literal_mappings() {
        // ZhuyinCoreChecks: "literal mapping \(key), shift=\(shifted)" (with the macOS punctuation keys).
        let plain_marks = [('[', "「"), (']', "」"), ('\'', "、"), ('\\', "＼"), ('=', "＝")];
        let shifted_marks = [
            (',', "，"),
            ('.', "。"),
            ('/', "？"),
            (';', "："),
            ('\'', "；"),
            ('1', "！"),
            ('2', "＠"),
            ('3', "＃"),
            ('4', "＄"),
            ('5', "％"),
            ('6', "…"),
            ('7', "＆"),
            ('8', "＊"),
            ('9', "（"),
            ('0', "）"),
            ('-', "——"),
            ('=', "＋"),
            ('[', "『"),
            (']', "』"),
            ('\\', "｜"),
            ('`', "～"),
        ];
        for (key, text) in plain_marks {
            assert_eq!(literal(KeyInput::Char(key), PunctuationKeys::Mac).as_deref(), Some(text));
        }
        for (key, text) in shifted_marks {
            assert_eq!(literal(KeyInput::ShiftedChar(key), PunctuationKeys::Mac).as_deref(), Some(text));
        }
    }

    #[test]
    fn windows_keys() {
        // ZhuyinCoreChecks: "Windows Shift+\(key) types \(text)".
        let printed = [
            (',', "<"),
            ('.', ">"),
            ('/', "?"),
            (';', ":"),
            ('\'', "\""),
            ('1', "!"),
            ('6', "^"),
            ('-', "_"),
            ('=', "+"),
            ('[', "{"),
            (']', "}"),
            ('\\', "|"),
            ('`', "~"),
        ];
        for (key, text) in printed {
            assert_eq!(literal(KeyInput::ShiftedChar(key), PunctuationKeys::Windows).as_deref(), Some(text));
        }
        // ZhuyinCoreChecks: "Control+\(key) types \(text)".
        let control = [
            (',', "，"),
            ('.', "。"),
            ('\'', "、"),
            (';', "；"),
            (':', "："),
            ('?', "？"),
            ('!', "！"),
            ('{', "『"),
            ('}', "』"),
            ('(', "（"),
            (')', "）"),
            ('<', "《"),
            ('>', "》"),
            ('-', "－"),
            ('_', "——"),
            ('=', "＝"),
            ('~', "～"),
        ];
        for (key, text) in control {
            assert_eq!(literal(KeyInput::ControlChar(key), PunctuationKeys::Windows).as_deref(), Some(text));
        }
        // Control+[ and Control+] are the application's; Control+/ too.
        for key in ['[', ']', '/', 'a', 'Z', '5'] {
            assert_eq!(control_punctuation(key), None, "control {key}");
        }
        // T_T with Windows keys.
        let typed: String = [KeyInput::ShiftedChar('t'), KeyInput::ShiftedChar('-'), KeyInput::ShiftedChar('t')]
            .into_iter()
            .map(|key| literal(key, PunctuationKeys::Windows).unwrap())
            .collect();
        assert_eq!(typed, "T_T");
    }

    #[test]
    fn uppercase_and_printed() {
        // ZhuyinCoreChecks: "uppercase literal \(key)".
        for key in 'a'..='z' {
            for keys in PunctuationKeys::ALL {
                assert_eq!(literal(KeyInput::ShiftedChar(key), keys), Some(key.to_ascii_uppercase().to_string()));
            }
        }
        assert_eq!(literal(KeyInput::Printed('.'), PunctuationKeys::Windows).as_deref(), Some("."));
        assert_eq!(literal(KeyInput::Char('a'), PunctuationKeys::Windows), None);
        assert_eq!(literal(KeyInput::Space, PunctuationKeys::Windows), None);
    }

    #[test]
    fn plain_and_alternatives() {
        assert_eq!(plain("ㄅㄚ˙ ㄆㄚ"), "˙ㄅㄚ ㄆㄚ");
        assert_eq!(plain("ㄅㄚ  ㄆㄚˊ"), "ㄅㄚ ㄆㄚˊ");
        assert_eq!(alternatives("，"), ["，", ",", "、", "；", "：", "…"]);
        assert_eq!(alternatives("《"), ["《", "「", "『", "（", "〈", "【"]);
        assert_eq!(alternatives("x"), ["x"]);
        assert_eq!(tone('6'), Some("ˊ"));
        assert_eq!(tone('5'), None);
    }
}

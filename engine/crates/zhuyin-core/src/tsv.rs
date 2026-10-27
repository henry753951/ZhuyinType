//! Tab-separated text: the scanner every TSV reader shares (`scanTSV` in `LanguageModel.swift`) and number parsing
//! with the syntax Swift's `Double(String)` and `Int(String)` accept.

use std::borrow::Cow;

/// Scans UTF-8 text once and calls `row` with the fields of every line that has at least two fields.
///
/// Fields are separated by tabs (and by spaces when `separating_spaces`); a line ending in CR LF loses the CR; a line
/// whose first byte (after leading separators that produced no field) is `#` is a comment.  With
/// `separating_spaces`, empty fields are dropped; without it they are kept.  Invalid UTF-8 is replaced with U+FFFD,
/// as Swift's `String(decoding:as:)` does.
pub fn scan_tsv(data: &[u8], separating_spaces: bool, mut row: impl FnMut(&[Cow<'_, str>])) {
    let mut start = 0usize;
    let mut fields: Vec<Cow<'_, str>> = Vec::with_capacity(4);
    let mut comment = false;
    for i in 0..=data.len() {
        let byte = if i == data.len() { b'\n' } else { data[i] };
        if i == start && fields.is_empty() && byte == b'#' {
            comment = true;
        }
        if byte == b'\t' || byte == b'\n' || (separating_spaces && byte == b' ') {
            if !comment {
                let end = if byte == b'\n' && i > start && data[i - 1] == b'\r' { i - 1 } else { i };
                if end > start || !separating_spaces {
                    fields.push(String::from_utf8_lossy(&data[start..end]));
                }
            }
            start = i + 1;
            if byte == b'\n' {
                if !comment && fields.len() > 1 {
                    row(&fields);
                }
                fields.clear();
                comment = false;
            }
        }
    }
}

/// Parses a number the way Swift's `Double(String)` does: the whole string must be a decimal or hexadecimal
/// floating-point literal, `inf`/`infinity` or `nan` (any case, optional sign); no surrounding whitespace.
/// Out-of-range values become ±infinity or zero.
#[must_use]
pub fn parse_double(text: &str) -> Option<f64> {
    if let Ok(value) = text.parse::<f64>() {
        return Some(value);
    }
    let (negative, rest) = match text.as_bytes().first() {
        Some(b'+') => (false, &text[1..]),
        Some(b'-') => (true, &text[1..]),
        _ => (false, text),
    };
    let lower = rest.to_ascii_lowercase();
    // strtod's NaN forms Rust does not accept: `snan` and `nan(chars)`.
    if lower == "snan" || (lower.starts_with("nan(") && lower.ends_with(')')) {
        return Some(if negative { -f64::NAN } else { f64::NAN });
    }
    let hex = lower.strip_prefix("0x")?;
    parse_hex(hex).map(|value| if negative { -value } else { value })
}

/// `digits[.digits][p[+-]exponent]` in hexadecimal, at least one digit.
fn parse_hex(text: &str) -> Option<f64> {
    let (mantissa, exponent) = match text.find('p') {
        Some(index) => {
            let exponent = &text[index + 1..];
            let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            // Saturate: anything this large overflows or underflows anyway.
            let magnitude: i64 = digits.parse().unwrap_or(i64::MAX / 4).min(1 << 40);
            (&text[..index], if exponent.starts_with('-') { -magnitude } else { magnitude })
        }
        None => (text, 0),
    };
    let (whole, fraction) = match mantissa.find('.') {
        Some(index) => (&mantissa[..index], &mantissa[index + 1..]),
        None => (mantissa, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    let mut significand: u128 = 0;
    let mut shift: i64 = 0;
    let mut sticky = false;
    for (index, byte) in whole.bytes().chain(fraction.bytes()).enumerate() {
        let digit = u128::from((byte as char).to_digit(16)?);
        let fractional = index >= whole.len();
        if significand >> 120 == 0 {
            significand = significand << 4 | digit;
            if fractional {
                shift -= 4;
            }
        } else {
            sticky |= digit != 0;
            if !fractional {
                shift += 4;
            }
        }
    }
    if sticky {
        significand |= 1;
    }
    let mut value = significand as f64;
    let mut exponent = exponent + shift;
    // Scale in steps that stay exact while the result is normal.
    while exponent > 0 && value.is_finite() {
        let step = exponent.min(1000);
        value *= 2f64.powi(step as i32);
        exponent -= step;
    }
    while exponent < 0 && value != 0.0 {
        let step = (-exponent).min(1000);
        value /= 2f64.powi(step as i32);
        exponent += step;
    }
    Some(value)
}

/// Parses an integer the way Swift's `Int(String)` does: optional sign, decimal digits, nothing else.
#[must_use]
pub fn parse_int(text: &str) -> Option<i64> {
    text.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str, spaces: bool) -> Vec<Vec<String>> {
        let mut result = vec![];
        scan_tsv(text.as_bytes(), spaces, |fields| {
            result.push(fields.iter().map(std::string::ToString::to_string).collect());
        });
        result
    }

    #[test]
    fn scanner() {
        assert_eq!(rows("a\tb\tc\n# x\ty\nd\te\r\n\nf\n", false), [vec!["a", "b", "c"], vec!["d", "e"]]);
        assert_eq!(rows("a  b\n  # c d\n\t#e\tf\n", true), [vec!["a", "b"]]);
        assert_eq!(rows("\t#e\tf", false), [vec!["", "#e", "f"]]);
        assert_eq!(rows("a\t\tb", false), [vec!["a", "", "b"]]);
        assert_eq!(rows("a\t\tb", true), [vec!["a", "b"]]);
        let mut invalid = vec![];
        scan_tsv(b"x\t\xff", false, |fields| invalid.push(fields[1].to_string()));
        assert_eq!(invalid, ["\u{FFFD}"]);
    }

    #[test]
    fn swift_double_syntax() {
        // Values probed from Swift 6.4's Double(String).
        let cases: &[(&str, Option<u64>)] = &[
            (" 1", None),
            ("1 ", None),
            ("+1", Some(0x3ff0_0000_0000_0000)),
            ("0x10", Some(0x4030_0000_0000_0000)),
            ("0x1p3", Some(0x4020_0000_0000_0000)),
            ("1_000", None),
            ("", None),
            (".5", Some(0x3fe0_0000_0000_0000)),
            ("5.", Some(0x4014_0000_0000_0000)),
            ("1e400", Some(0x7ff0_0000_0000_0000)),
            ("-1e400", Some(0xfff0_0000_0000_0000)),
            ("Infinity", Some(0x7ff0_0000_0000_0000)),
            ("1e-400", Some(0)),
            ("- 1", None),
            ("1.5e", None),
            ("e5", None),
            ("-.5e-3", Some(0xbf40_624d_d2f1_a9fc)),
            ("\t1", None),
            ("-0", Some(0x8000_0000_0000_0000)),
            ("00012", Some(0x4028_0000_0000_0000)),
            ("1.7976931348623159e308", Some(0x7ff0_0000_0000_0000)),
        ];
        for &(text, bits) in cases {
            assert_eq!(parse_double(text).map(f64::to_bits), bits, "{text:?}");
        }
        for text in ["nan", "NaN", "-nan", "snan"] {
            assert!(parse_double(text).unwrap().is_nan(), "{text}");
        }
        assert_eq!(parse_double("0x1.8p1"), Some(3.0));
        assert_eq!(parse_double("-0X.8"), Some(-0.5));
        assert_eq!(parse_int("+5"), Some(5));
        assert_eq!(parse_int(" 5"), None);
        assert_eq!(parse_int("5.0"), None);
    }
}

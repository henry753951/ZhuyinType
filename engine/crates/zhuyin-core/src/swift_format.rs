//! Text forms the Swift engine writes into personal data files: `"\(double)"` (Swift's `Double.description`) and the
//! grapheme clusters Swift's `Character` iterates over.

use crate::api::extends_grapheme;

/// Formats a `f64` exactly as Swift's `Double.description`: the shortest digits that round-trip, as a decimal
/// (`1761234567.123`, `0.0001`, `1.0`) when 1e-4 ≤ |x| ≤ 2⁵³, otherwise in exponent form (`1e+16`, `9.999e-05`,
/// `5e-324`); `inf`, `-inf`, `nan`.
///
/// # Panics
/// Never: Rust's exponent form of a finite number always has a mantissa and an integer exponent.
#[must_use]
pub fn format_double(value: f64) -> String {
    if value.is_nan() {
        return if value.is_sign_negative() { "-nan".to_owned() } else { "nan".to_owned() };
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf".to_owned() } else { "inf".to_owned() };
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return format!("{sign}0.0");
    }
    // Rust's `{:e}` without a precision gives the number of shortest round-trip digits.  The digits themselves are
    // the value rounded to that many digits (ties to even), as Swift chooses them: in an exact tie the shortest
    // mode may round up instead.
    let shortest = format!("{magnitude:e}");
    let count = shortest.split_once('e').map_or(1, |(m, _)| m.chars().filter(char::is_ascii_digit).count());
    let scientific = format!("{magnitude:.*e}", count - 1);
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("exponent");
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    if !(1e-4..=9_007_199_254_740_992.0).contains(&magnitude) {
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() { String::new() } else { format!(".{rest}") };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!("{sign}{first}{fraction}e{exponent_sign}{:02}", exponent.unsigned_abs());
    }
    if exponent >= 0 {
        let point = exponent.unsigned_abs() as usize + 1;
        if digits.len() <= point {
            format!("{sign}{digits}{}.0", "0".repeat(point - digits.len()))
        } else {
            format!("{sign}{}.{}", &digits[..point], &digits[point..])
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat(exponent.unsigned_abs() as usize - 1))
    }
}

/// The text's extended grapheme clusters, as far as the engine's text needs them: a scalar followed by combining
/// marks, variation selectors, joiners or emoji modifiers is one cluster, and CR LF is one.  (Swift iterates
/// `Character`s; for Han text, Zhuyin and ASCII each scalar is its own cluster.)
#[must_use]
pub fn graphemes(text: &str) -> Vec<&str> {
    let mut result = Vec::with_capacity(text.len() / 3 + 1);
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (index, c) in text.char_indices() {
        let joins = match previous {
            None => false,
            Some('\r') => c == '\n',
            Some('\u{200D}') => true,
            Some(_) => extends_grapheme(c),
        };
        if !joins && index > start {
            result.push(&text[start..index]);
            start = index;
        }
        previous = Some(c);
    }
    if start < text.len() {
        result.push(&text[start..]);
    }
    result
}

/// The number of grapheme clusters (Swift's `String.count`).
#[must_use]
pub fn grapheme_count(text: &str) -> usize {
    graphemes(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_descriptions() {
        // Probed from Swift 6.4.
        let cases: &[(f64, &str)] = &[
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.000_099_99, "9.999e-05"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (9_007_199_254_740_991.0, "9007199254740991.0"),
            (9_007_199_254_740_992.0, "9007199254740992.0"),
            (9_007_199_254_740_994.0, "9.007199254740994e+15"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (2.5e15, "2500000000000000.0"),
            (9.1e15, "9.1e+15"),
            (123.456, "123.456"),
            (1_761_234_567.123, "1761234567.123"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
            (0.300_000_000_000_000_04, "0.30000000000000004"),
            (1e100, "1e+100"),
            (1e-100, "1e-100"),
            (4.35e-5, "4.35e-05"),
            (0.05, "0.05"),
            (99_999_999_999_999.98, "99999999999999.98"),
            (f64::from_bits(0x414f_c7b0_af90_0000), "4165473.3715820312"),
        ];
        for &(value, text) in cases {
            assert_eq!(format_double(value), text, "{value:e}");
        }
    }

    #[test]
    fn clusters() {
        assert_eq!(graphemes("你好"), ["你", "好"]);
        assert_eq!(graphemes("臺\u{FE00}灣"), ["臺\u{FE00}", "灣"]);
        assert_eq!(graphemes("a\r\nb"), ["a", "\r\n", "b"]);
        assert_eq!(grapheme_count(""), 0);
    }
}

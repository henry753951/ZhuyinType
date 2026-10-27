//! Physical US ANSI neighbours and the keyboard error channel, in natural-log costs (port of `KeyChannel.swift`).

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::keyboard;
use crate::lexicon::swift_min;

/// The four key rows.  q straddles 1/2, a straddles q/w, and z straddles a/s.
const ROWS: [&[u8]; 4] = [b"1234567890-=", b"qwertyuiop[]", b"asdfghjkl;'", b"zxcvbnm,./"];

/// Cost of typing the 3 tone key for 4 or the other way round.
pub const TONE34: f64 = 5.0;
/// Cost of typing the 6 tone key for 7 or the other way round.
pub const TONE67: f64 = 4.5;
/// Cost of a neighbouring key typed instead of the intended one.
pub const SUBSTITUTION: f64 = 11.0;
/// Cost of two neighbouring keys typed in the wrong order.
pub const TRANSPOSITION: f64 = 9.0;
/// Cost of an extra key.
pub const DELETION: f64 = 12.5;
/// Cost of a missing key.
pub const INSERTION: f64 = 12.5;
/// Hit-area costs: the constant and the Gaussian's widths across a row and between rows, in key widths.
pub const HOTSPOT_BASE: f64 = 9.0;
/// See [`HOTSPOT_BASE`].
pub const HOTSPOT_SIGMA_X: f64 = 0.5;
/// See [`HOTSPOT_BASE`].
pub const HOTSPOT_SIGMA_Y: f64 = 0.4;

/// Every key an insertion may add: the symbol keys, tone keys and Space.
const INPUT_KEYS: &[u8] = b"1234567890qwertyuiopasdfghjkl;zxcvbnm,./- ";

/// One valid reading near the keys typed.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingAlternative {
    /// The reading.
    pub reading: String,
    /// Its channel cost.
    pub cost: f64,
    /// A fuzzy-sound match: ranked by its cost but not counted as a typing error.
    pub fuzzy: bool,
}

struct Tables {
    /// Sorted neighbours of every ASCII key.
    neighbours: Vec<Vec<char>>,
    /// Neighbours that are symbol or tone keys, as bytes.
    neighbour_bytes: Vec<Vec<u8>>,
    /// Key centres in key widths (x, y), NaN for keys not on the rows.
    centres: Vec<(f64, f64)>,
    /// For each ASCII key: the bits a key sets in a reading code and their mask.
    key_codes: [(u32, u32); 128],
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let mut sets: Vec<Vec<char>> = vec![Vec::new(); 128];
    let mut connect = |a: u8, b: u8| {
        for (x, y) in [(a, b), (b, a)] {
            let list = &mut sets[x as usize];
            if !list.contains(&(y as char)) {
                list.push(y as char);
            }
        }
    };
    for (row, keys) in ROWS.iter().enumerate() {
        for (index, &key) in keys.iter().enumerate() {
            if index + 1 < keys.len() {
                connect(key, keys[index + 1]);
            }
            if row > 0 {
                for above in [index, index + 1] {
                    if above < ROWS[row - 1].len() {
                        connect(key, ROWS[row - 1][above]);
                    }
                }
            }
        }
    }
    for list in &mut sets {
        list.sort_unstable();
    }
    let neighbour_bytes = sets
        .iter()
        .map(|list| {
            list.iter()
                .filter(|&&key| keyboard::symbol(key).is_some() || keyboard::tone(key).is_some())
                .map(|&key| key as u8)
                .collect()
        })
        .collect();
    let offsets = [0.0, 0.5, 0.75, 1.25];
    let mut centres = vec![(f64::NAN, f64::NAN); 128];
    for (row, keys) in ROWS.iter().enumerate() {
        for (index, &key) in keys.iter().enumerate() {
            centres[key as usize] = (index as f64 + offsets[row], row as f64);
        }
    }
    let mut key_codes = [(0u32, 0u32); 128];
    for (byte, item) in key_codes.iter_mut().enumerate() {
        let key = byte as u8 as char;
        if let Some(symbol) = keyboard::symbol(key)
            && let Some(slot) = keyboard::slot(symbol)
        {
            *item = (63 << (slot * 6), symbol_code(symbol) << (slot * 6));
        } else if let Some(tone) = keyboard::tone(key) {
            *item = (7 << 18, code_for_reading(tone));
        }
    }
    Tables { neighbours: sets, neighbour_bytes, centres, key_codes }
});

/// A Zhuyin symbol's number: its position among the layout's symbols sorted by scalar value, from 1.
#[inline]
fn symbol_code(symbol: char) -> u32 {
    // The 37 symbols of the layout are exactly U+3105..=U+3129, so the sorted position is the offset from U+3104.
    debug_assert!(keyboard::symbol_pairs().any(|(_, s)| s == symbol));
    symbol as u32 - 0x3104
}

/// The keys next to `key` on a US ANSI keyboard, sorted.
#[must_use]
pub fn neighbours(key: char) -> &'static [char] {
    let index = key as usize;
    if index < 128 { &TABLES.neighbours[index] } else { &[] }
}

/// Whether `other` is next to `key`.
#[inline]
pub(crate) fn are_neighbours(key: char, other: char) -> bool {
    neighbours(key).contains(&other)
}

/// −log of how likely a press meant for `intended` lands on `typed`, up to a constant: an anisotropic Gaussian
/// around the intended key's centre, wider across a row than between rows.
#[must_use]
pub fn hotspot_cost(intended: char, typed: char) -> f64 {
    let centre = |key: char| {
        let index = key as usize;
        (index < 128).then(|| TABLES.centres[index]).filter(|c| !c.0.is_nan())
    };
    let (Some(a), Some(b)) = (centre(intended), centre(typed)) else { return SUBSTITUTION };
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    HOTSPOT_BASE
        + dx * dx / (2.0 * HOTSPOT_SIGMA_X * HOTSPOT_SIGMA_X)
        + dy * dy / (2.0 * HOTSPOT_SIGMA_Y * HOTSPOT_SIGMA_Y)
}

/// A reading packed into an integer: six bits per slot, plus three tone bits, so parsing edits needs no strings.
#[must_use]
pub fn code_for_reading(reading: &str) -> u32 {
    let mut code = 0u32;
    for symbol in reading.chars() {
        if let Some(slot) = keyboard::slot(symbol) {
            code = (code & !(63 << (slot * 6))) | (symbol_code(symbol) << (slot * 6));
        } else if let Some(tone) = "ˊˇˋ˙".chars().position(|mark| mark == symbol) {
            code |= (tone as u32 + 1) << 18;
        }
    }
    code
}

/// The code of a key sequence: symbol keys, then at most one tone key or Space, last.  `None` when the keys type
/// no symbol, or a tone key or Space is not last, or a key types nothing.
#[inline]
fn code_for_keys(keys: &[u8], key_codes: &[(u32, u32); 128]) -> Option<u32> {
    let mut code = 0u32;
    let mut has_symbols = false;
    for (index, &key) in keys.iter().enumerate() {
        if key >= 128 {
            return None;
        }
        let (mask, value) = key_codes[key as usize];
        if mask == 7 << 18 || key == b' ' {
            if index != keys.len() - 1 {
                return None;
            }
        } else if mask == 0 {
            return None;
        } else {
            has_symbols = true;
        }
        code = (code & !mask) | value;
    }
    has_symbols.then_some(code)
}

/// The keys that type `reading` on the standard layout: symbols, then the tone key or Space.
#[must_use]
pub fn keys_for_reading(reading: &str) -> Option<String> {
    let mut keys = String::new();
    let mut tone = ' ';
    for symbol in reading.chars() {
        if let Some(key) = keyboard::key_for_symbol(symbol) {
            keys.push(key);
        } else {
            tone = keyboard::key_for_tone(symbol)?;
        }
    }
    keys.push(tone);
    Some(keys)
}

/// Keys typed in place of others, by a minimal edit alignment of what was typed with what was meant.
#[must_use]
pub fn substitutions(typed: &str, intended: &str) -> Vec<(char, char)> {
    let a: Vec<char> = typed.chars().collect();
    let b: Vec<char> = intended.chars().collect();
    let mut cost = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in cost.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, value) in cost[0].iter_mut().enumerate() {
        *value = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let replace = cost[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cost[i][j] = (cost[i - 1][j] + 1).min(cost[i][j - 1] + 1).min(replace);
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (a.len(), b.len());
    while i > 0 && j > 0 {
        if cost[i][j] == cost[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]) {
            if a[i - 1] != b[j - 1] {
                pairs.push((a[i - 1], b[j - 1]));
            }
            i -= 1;
            j -= 1;
        } else if cost[i][j] == cost[i - 1][j] + 1 {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

/// Visits every single edit of `keys` with its cost: neighbour substitutions (tone pairs cheaper), adjacent
/// transpositions, deletions and insertions, in the Swift engine's order.
fn edits(
    keys: &[u8],
    hotspot: bool,
    discount: &impl Fn(u8, u8) -> f64,
    neighbour_bytes: &[Vec<u8>],
    mut visit: impl FnMut(&[u8], f64),
) {
    let mut edited = keys.to_vec();
    for index in 0..keys.len() {
        let old = keys[index];
        if old < 128 {
            for &key in &neighbour_bytes[old as usize] {
                edited[index] = key;
                let cost = if (old == b'3' && key == b'4') || (old == b'4' && key == b'3') {
                    TONE34
                } else if (old == b'6' && key == b'7') || (old == b'7' && key == b'6') {
                    TONE67
                } else if hotspot {
                    hotspot_cost(key as char, old as char)
                } else {
                    SUBSTITUTION
                };
                visit(&edited, cost - discount(old, key));
            }
            edited[index] = old;
        }
        if index + 1 < keys.len() && old != keys[index + 1] {
            edited.swap(index, index + 1);
            visit(&edited, TRANSPOSITION);
            edited.swap(index, index + 1);
        }
        edited.remove(index);
        visit(&edited, DELETION);
        edited.insert(index, old);
    }
    for index in 0..=keys.len() {
        edited.insert(index, b' ');
        for &key in INPUT_KEYS {
            edited[index] = key;
            visit(&edited, INSERTION);
        }
        edited.remove(index);
    }
}

/// The eighth-cheapest cost so far (∞ with fewer than eight).
fn cutoff(best: &HashMap<u32, f64>) -> f64 {
    if best.len() < 8 {
        return f64::INFINITY;
    }
    let mut costs: Vec<f64> = best.values().copied().collect();
    let (_, eighth, _) = costs.select_nth_unstable_by(7, f64::total_cmp);
    *eighth
}

/// The eight cheapest valid readings within one edit of `keys` (two for four or more keys), cheapest first, ties by
/// reading.  `discount(typed, intended)` makes this user's own slips cheaper; `valid` maps a reading code to its
/// reading when the syllable exists; `hotspot` uses hit-area substitution costs (an evaluation setting).
pub fn alternatives<S: AsRef<str>>(
    keys: &str,
    hotspot: bool,
    discount: impl Fn(u8, u8) -> f64,
    valid: impl Fn(u32) -> Option<S>,
) -> Vec<ReadingAlternative> {
    let tables = &*TABLES;
    let keys = keys.as_bytes();
    let exact = code_for_keys(keys, &tables.key_codes);
    let mut best: HashMap<u32, f64> = HashMap::new();
    let mut first: HashMap<Vec<u8>, f64> = HashMap::new();
    let accept = |best: &mut HashMap<u32, f64>, edited: &[u8], cost: f64| {
        let Some(code) = code_for_keys(edited, &tables.key_codes) else { return };
        if Some(code) == exact || valid(code).is_none() {
            return;
        }
        let entry = best.entry(code).or_insert(f64::INFINITY);
        *entry = swift_min(*entry, cost);
    };
    edits(keys, hotspot, &discount, &tables.neighbour_bytes, |edited, cost| {
        match first.get_mut(edited) {
            Some(old) => *old = swift_min(*old, cost),
            None => {
                first.insert(edited.to_vec(), cost);
            }
        }
        accept(&mut best, edited, cost);
    });
    if keys.len() >= 4 {
        let cheapest_edit = [TONE34, TONE67, SUBSTITUTION, TRANSPOSITION, DELETION, INSERTION]
            .into_iter()
            .fold(f64::INFINITY, swift_min);
        let mut limit = cutoff(&best);
        let mut ordered: Vec<(Vec<u8>, f64)> = first.into_iter().collect();
        ordered.sort_unstable_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        for (edited, cost) in &ordered {
            if cost + cheapest_edit > limit {
                break;
            }
            edits(edited, hotspot, &discount, &tables.neighbour_bytes, |twice, extra| {
                if cost + extra <= limit {
                    accept(&mut best, twice, cost + extra);
                }
            });
            limit = cutoff(&best);
        }
    }
    let mut result: Vec<ReadingAlternative> = best
        .into_iter()
        .filter_map(|(code, cost)| {
            valid(code).map(|reading| ReadingAlternative { reading: reading.as_ref().to_owned(), cost, fuzzy: false })
        })
        .collect();
    result.sort_unstable_by(|a, b| a.cost.total_cmp(&b.cost).then_with(|| a.reading.cmp(&b.reading)));
    result.truncate(8);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_neighbours() {
        // ZhuyinCoreChecks: "symmetric neighbours".
        for row in ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"] {
            for key in row.chars() {
                for &neighbour in neighbours(key) {
                    assert!(neighbours(neighbour).contains(&key), "symmetric neighbours {key}/{neighbour}");
                }
            }
        }
        let w = neighbours('w');
        assert!("2qeas".chars().all(|c| w.contains(&c)), "staggered w neighbours");
        assert!("23as".chars().all(|c| w.contains(&c)), "w straddles 2/3 above and a/s below");
        assert!(neighbours('3').contains(&'4') && neighbours('6').contains(&'7'), "adjacent tone pairs");
        assert!(!neighbours('4').contains(&'6'), "4 and 6 are separated by 5");
        assert!(neighbours('e').contains(&'w') && neighbours('a').contains(&'z'), "ordinary and staggered neighbours");
        assert!(neighbours('ㄅ').is_empty());
        let mut sorted = w.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, w);
    }

    #[test]
    fn reading_codes() {
        assert_eq!(code_for_reading("ㄅㄧㄢ"), 1 | (code_for_reading("ㄧ")) | (code_for_reading("ㄢ")));
        assert_eq!(code_for_reading("ㄅ"), 1);
        assert_eq!(code_for_reading("ˊ"), 1 << 18);
        assert_eq!(code_for_reading("ㄚㄅ"), code_for_reading("ㄅㄚ"));
        let tables = &*TABLES;
        assert_eq!(code_for_keys(b"1u0 ", &tables.key_codes), Some(code_for_reading("ㄅㄧㄢ")));
        assert_eq!(code_for_keys(b"183", &tables.key_codes), Some(code_for_reading("ㄅㄚˇ")));
        assert_eq!(code_for_keys(b"3", &tables.key_codes), None);
        assert_eq!(code_for_keys(b"13u", &tables.key_codes), None);
        assert_eq!(code_for_keys(b"1 8", &tables.key_codes), None);
        assert_eq!(code_for_keys(b"1A", &tables.key_codes), None);
        assert_eq!(keys_for_reading("ㄍㄜˋ").as_deref(), Some("ek4"));
        assert_eq!(keys_for_reading("ㄅㄧㄢ").as_deref(), Some("1u0 "));
        assert_eq!(keys_for_reading("x"), None);
        assert_eq!(substitutions("rk4", "ek4"), [('r', 'e')]);
        assert_eq!(substitutions("abc", "abc"), []);
    }

    #[test]
    fn hotspot() {
        assert_eq!(hotspot_cost('a', 'a'), HOTSPOT_BASE);
        assert_eq!(hotspot_cost('a', 's'), HOTSPOT_BASE + 2.0);
        assert_eq!(hotspot_cost('ㄅ', 's'), SUBSTITUTION);
    }
}

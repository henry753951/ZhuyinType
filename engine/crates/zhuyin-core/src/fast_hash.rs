//! A fast hasher for the decoders' internal tables, whose keys are small (language-model histories, word IDs, key
//! decoder signatures) and never come from outside, so the standard library's `SipHash` (built to resist crafted
//! keys) only costs time there.  Only lookups use these tables, never their iteration order, so results do not
//! depend on the hash.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A `HashMap` with [`FastHasher`].
pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// The multiply-rotate hash of rustc (`FxHasher`): one rotation, xor and multiplication per word.
#[derive(Clone, Copy, Default)]
pub(crate) struct FastHasher(u64);

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FastHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.add(u64::from_le_bytes(*chunk));
        }
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::FastMap;
    use crate::language_model::LanguageModelHistory;

    #[test]
    fn maps_find_what_they_hold() {
        let mut map: FastMap<LanguageModelHistory, u32> = FastMap::default();
        for previous in 0..1000 {
            map.insert(LanguageModelHistory::new(Some(previous / 3), previous), previous);
        }
        assert_eq!(map.len(), 1000);
        assert_eq!(map.get(&LanguageModelHistory::new(Some(7), 21)), Some(&21));
        assert_eq!(map.get(&LanguageModelHistory::new(None, 21)), None);
        let mut strings: FastMap<String, usize> = FastMap::default();
        for word in ["", "a", "ㄋㄧˇ", "0123456789abcdef", "0123456789abcdefg"] {
            strings.insert(word.to_owned(), word.len());
        }
        assert_eq!(strings.len(), 5);
        assert_eq!(strings.get("0123456789abcdefg"), Some(&17));
    }
}

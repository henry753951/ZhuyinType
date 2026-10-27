//! ZTLX/1: the memory-mapped lexicon (port of `MappedLexicon.swift`).  Tables and UTF-8 stay mapped; only requested
//! entries become owned values.  Format: `resources/formats/lexicon.md`.

use std::path::Path;

use super::{DataError, Mapping, f64_at, u32_at, u64_at};
use crate::lexicon::WordEntry;

/// Table numbers: the order of the section offsets in the header.
const WORDS: usize = 0;
const READINGS: usize = 1;
const ENTRIES: usize = 2;
const REVERSE: usize = 3;
const BASES: usize = 4;
const BASE_REFS: usize = 5;
const PREFIXES: usize = 6;
const SYLLABLES: usize = 7;
const BLOB: usize = 8;

/// One lexicon entry, borrowed from the mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MappedEntry<'a> {
    /// The word.
    pub word: &'a str,
    /// log P of the word under its most common reading.
    pub logp: f64,
    /// log P(this reading | word) against the word's most common reading.
    pub reading_prior: f64,
}

/// A ZTLX/1 lexicon kept memory-mapped.
pub struct MappedLexicon {
    map: Mapping,
    word_count: usize,
    reading_count: usize,
    base_count: usize,
    prefix_count: usize,
    syllable_count: usize,
    sections: [usize; 10],
}

impl MappedLexicon {
    /// Maps and validates a lexicon file: header, section bounds, every string reference, UTF-8 and order, every
    /// range and index, and finite scores.
    ///
    /// # Errors
    /// [`DataError::Io`] when the file cannot be read, [`DataError::InvalidLexicon`] when any check fails.
    pub fn open(path: impl AsRef<Path>) -> Result<MappedLexicon, DataError> {
        let map = Mapping::open(path.as_ref())?;
        let data = map.bytes();
        let size = data.len();
        if size < 128 || u32_at(data, 0) != 0x584c_545a || u32_at(data, 4) != 1 {
            return Err(DataError::InvalidLexicon);
        }
        let counts: [usize; 6] = std::array::from_fn(|i| u32_at(data, (i + 2) * 4) as usize);
        let offsets: [u64; 12] = std::array::from_fn(|i| u64_at(data, 32 + i * 8));
        if offsets[9] != size as u64
            || offsets[10] != 0
            || offsets[11] != 0
            || offsets[0] != 128
            || !offsets[..9].iter().all(|&offset| offset <= size as u64 && offset % 8 == 0)
            || !offsets[..9].windows(2).all(|pair| pair[0] <= pair[1])
        {
            return Err(DataError::InvalidLexicon);
        }
        let s: [usize; 10] = std::array::from_fn(|i| offsets[i] as usize);
        let lengths = [
            counts[0] * 24,
            counts[1] * 16,
            counts[2] * 24,
            counts[2] * 4,
            counts[3] * 16,
            counts[1] * 4,
            counts[4] * 8,
            counts[5] * 8,
        ];
        if !(0..8).all(|i| lengths[i] <= s[i + 1] - s[i]) {
            return Err(DataError::InvalidLexicon);
        }
        if !tables_valid(data, &s, &counts) {
            return Err(DataError::InvalidLexicon);
        }
        Ok(MappedLexicon {
            word_count: counts[0],
            reading_count: counts[1],
            base_count: counts[3],
            prefix_count: counts[4],
            syllable_count: counts[5],
            sections: s,
            map,
        })
    }

    /// Number of distinct words.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.word_count
    }

    #[inline]
    fn data(&self) -> &[u8] {
        self.map.bytes()
    }

    #[inline]
    fn uint(&self, offset: usize) -> usize {
        u32_at(self.data(), offset) as usize
    }

    /// The bytes a string reference of a word, reading, base, prefix or syllable record points to.
    #[inline]
    fn string_bytes(&self, record: usize) -> &[u8] {
        let data = self.data();
        debug_assert!(record + 8 <= self.sections[BLOB]);
        // SAFETY: `record` is a record of the word, reading, base, prefix or syllable table: callers compute it from
        // a table's section offset and an index below that table's count, and `open` checked that each table fits
        // in its section (so the eight bytes are inside the mapping) and that every string reference of those
        // tables lies inside the blob.  The mapping is immutable for the lifetime of `self`.
        #[allow(unsafe_code)]
        unsafe {
            let reference = data.get_unchecked(record..record + 8);
            let start = self.sections[BLOB] + u32::from_le_bytes(reference[..4].try_into().unwrap_unchecked()) as usize;
            let length = u32::from_le_bytes(reference[4..].try_into().unwrap_unchecked()) as usize;
            data.get_unchecked(start..start + length)
        }
    }

    /// The string a record's `(offset, length)` reference points to.
    #[inline]
    fn spelling(&self, record: usize) -> &str {
        let bytes = self.string_bytes(record);
        // SAFETY: `open` checked that every string of the tables `string_bytes` reads is valid UTF-8.
        #[allow(unsafe_code)]
        unsafe {
            std::str::from_utf8_unchecked(bytes)
        }
    }

    /// Binary search of a sorted string table.
    #[inline]
    fn find(&self, key: &[u8], table: usize, count: usize, stride: usize) -> Option<usize> {
        let base = self.sections[table];
        let (mut low, mut high) = (0usize, count);
        while low < high {
            let mid = usize::midpoint(low, high);
            match self.string_bytes(base + mid * stride).cmp(key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    /// The word's index in the sorted word table.
    #[must_use]
    pub fn word_index(&self, word: &str) -> Option<usize> {
        self.find(word.as_bytes(), WORDS, self.word_count, 24)
    }

    /// Whether `base` (toneless syllables separated by spaces) starts some lexicon reading.
    #[must_use]
    pub fn contains_prefix(&self, base: &str) -> bool {
        self.find(base.as_bytes(), PREFIXES, self.prefix_count, 8).is_some()
    }

    /// Every distinct toned syllable, in table order.
    #[must_use]
    pub fn syllable_strs(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        (0..self.syllable_count).map(|i| self.spelling(self.sections[SYLLABLES] + i * 8))
    }

    /// Every distinct toned syllable, in table order.
    pub fn syllables(&self) -> Vec<String> {
        self.syllable_strs().map(str::to_owned).collect()
    }

    /// Every reading, in table order.
    #[must_use]
    pub fn reading_strs(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        (0..self.reading_count).map(|i| self.spelling(self.sections[READINGS] + i * 16))
    }

    /// The readings whose toneless form is `base`, borrowed.
    pub fn reading_strs_for_base(&self, base: &str) -> impl Iterator<Item = &str> + '_ {
        let range = match self.find(base.as_bytes(), BASES, self.base_count, 16) {
            Some(i) => {
                let p = self.sections[BASES] + i * 16;
                let start = self.uint(p + 8);
                start..start + self.uint(p + 12)
            }
            None => 0..0,
        };
        range.map(move |i| self.spelling(self.sections[READINGS] + self.uint(self.sections[BASE_REFS] + i * 4) * 16))
    }

    /// The readings whose toneless form is `base`.
    pub fn readings_for_base(&self, base: &str) -> Vec<String> {
        self.reading_strs_for_base(base).map(str::to_owned).collect()
    }

    /// The entries of `reading`, in lexicon order, borrowed from the mapping.
    #[must_use]
    pub fn entry_refs(&self, reading: &str) -> impl ExactSizeIterator<Item = MappedEntry<'_>> + '_ {
        let range = match self.find(reading.as_bytes(), READINGS, self.reading_count, 16) {
            Some(i) => {
                let p = self.sections[READINGS] + i * 16;
                let start = self.uint(p + 8);
                start..start + self.uint(p + 12)
            }
            None => 0..0,
        };
        range.map(move |i| {
            let p = self.sections[ENTRIES] + i * 24;
            let data = self.data();
            MappedEntry {
                word: self.spelling(self.sections[WORDS] + u32_at(data, p) as usize * 24),
                logp: f64_at(data, p + 8),
                reading_prior: f64_at(data, p + 16),
            }
        })
    }

    /// The entries of `reading`, in lexicon order, as engine word entries (no reading, no word ID yet).
    #[must_use]
    pub fn entries(&self, reading: &str) -> Vec<WordEntry> {
        self.entry_refs(reading)
            .map(|entry| WordEntry::new(entry.word, entry.logp).with_reading_prior(entry.reading_prior))
            .collect()
    }

    /// The reading and original score (`logp + prior`) of entry `index`.
    fn reading_entry(&self, index: usize) -> (&str, f64) {
        let p = self.sections[ENTRIES] + index * 24;
        let data = self.data();
        (
            self.spelling(self.sections[READINGS] + u32_at(data, p + 4) as usize * 16),
            f64_at(data, p + 8) + f64_at(data, p + 16),
        )
    }

    /// Every reading of `word` with its original lexicon score, in lexicon order, borrowed.
    pub fn reading_strs_of(&self, word: &str) -> impl Iterator<Item = (&str, f64)> + '_ {
        let range = match self.word_index(word) {
            Some(i) => {
                let p = self.sections[WORDS] + i * 24;
                let start = self.uint(p + 8);
                start..start + self.uint(p + 12)
            }
            None => 0..0,
        };
        range.map(move |i| self.reading_entry(self.uint(self.sections[REVERSE] + i * 4)))
    }

    /// Every reading of `word` with its original lexicon score, in lexicon order.
    #[must_use]
    pub fn readings_of(&self, word: &str) -> Vec<(String, f64)> {
        self.reading_strs_of(word).map(|(reading, score)| (reading.to_owned(), score)).collect()
    }

    /// The word's most common reading (the first of the highest-scoring ones) and its score.
    #[must_use]
    pub fn best_reading(&self, word: &str) -> Option<(String, f64)> {
        let i = self.word_index(word)?;
        let (reading, score) = self.reading_entry(self.uint(self.sections[WORDS] + i * 24 + 16));
        Some((reading.to_owned(), score))
    }
}

/// Checks every table the way `MappedLexicon.init` does.
fn tables_valid(data: &[u8], s: &[usize; 10], counts: &[usize; 6]) -> bool {
    let u = |offset: usize| u32_at(data, offset) as usize;
    let blob_size = data.len() - s[BLOB];
    for (table, count, stride) in [
        (WORDS, counts[0], 24),
        (READINGS, counts[1], 16),
        (BASES, counts[3], 16),
        (PREFIXES, counts[4], 8),
        (SYLLABLES, counts[5], 8),
    ] {
        let mut previous: Option<&[u8]> = None;
        for i in 0..count {
            let p = s[table] + i * stride;
            let (start, length) = (u(p), u(p + 4));
            if length == 0 || start > blob_size || length > blob_size - start {
                return false;
            }
            let bytes = &data[s[BLOB] + start..s[BLOB] + start + length];
            if std::str::from_utf8(bytes).is_err() {
                return false;
            }
            if previous.is_some_and(|previous| previous >= bytes) {
                return false;
            }
            previous = Some(bytes);
            if table == WORDS || table == READINGS || table == BASES {
                let limit = if table == BASES { counts[1] } else { counts[2] };
                if u(p + 8) > limit || u(p + 12) > limit - u(p + 8) {
                    return false;
                }
            }
            if table == WORDS && u(p + 16) >= counts[2] {
                return false;
            }
        }
    }
    for i in 0..counts[2] {
        let p = s[ENTRIES] + i * 24;
        if u(p) >= counts[0]
            || u(p + 4) >= counts[1]
            || !f64_at(data, p + 8).is_finite()
            || !f64_at(data, p + 16).is_finite()
            || u(s[REVERSE] + i * 4) >= counts[2]
        {
            return false;
        }
    }
    (0..counts[1]).all(|i| u(s[BASE_REFS] + i * 4) < counts[1])
}

//! Lexicon entries and the text lexicon (`lexicon.tsv`), as the Swift `Engine` loads it when no `lexicon.bin` is
//! next to it.  The memory-mapped lexicon is [`crate::binary::MappedLexicon`].

use std::collections::HashMap;
use std::sync::Arc;

use crate::tsv::{parse_double, scan_tsv};

/// One word for one reading, with what decoding adds to it (port of Swift's `WordEntry`).
#[derive(Clone, Debug, PartialEq)]
pub struct WordEntry {
    /// The word.
    pub word: Arc<str>,
    /// log P of the word under its most common reading.
    pub logp: f64,
    /// The lexicon reading this entry was found under, once matched.
    pub reading: Option<Arc<str>>,
    /// Cost subtracted for tone tolerance, typing corrections and fuzzy sounds.
    pub penalty: f64,
    /// A user-confirmed correction: an exact typed-reading recall on later input.
    pub recalled: bool,
    /// The language model's ID for the word, or a personal one.
    pub word_id: Option<u32>,
    /// A recall of a whole earlier composition: offered only when it covers the whole current one.
    pub whole_only: bool,
    /// log P(this reading | word) against the word's most common reading (0 for it, below 0 for rarer ones), so the
    /// language model's word probability is not spent on a reading the word rarely has.
    pub reading_prior: f64,
}

impl WordEntry {
    /// An entry with every other field at its default.
    #[must_use]
    pub fn new(word: impl Into<Arc<str>>, logp: f64) -> Self {
        Self {
            word: word.into(),
            logp,
            reading: None,
            penalty: 0.0,
            recalled: false,
            word_id: None,
            whole_only: false,
            reading_prior: 0.0,
        }
    }

    /// The entry with a word ID.
    #[must_use]
    pub fn with_word_id(mut self, word_id: Option<u32>) -> Self {
        self.word_id = word_id;
        self
    }

    /// The entry with a reading prior.
    #[must_use]
    pub fn with_reading_prior(mut self, reading_prior: f64) -> Self {
        self.reading_prior = reading_prior;
        self
    }
}

type EntryIndex = HashMap<String, Vec<WordEntry>>;
type ReverseIndex = HashMap<String, Vec<(String, f64)>>;

/// `lexicon.tsv` read into memory: `reading<TAB>word<TAB>log P(word, reading)` per line.
#[derive(Clone, Debug, Default)]
pub struct TextLexicon {
    /// Entries by reading, in file order.  A word with several readings has `logp` set to its best reading's score
    /// and `reading_prior` to the difference.
    entries: HashMap<String, Vec<WordEntry>>,
    /// Every (reading, original score) of each word, in file order.
    reverse: HashMap<String, Vec<(String, f64)>>,
}

impl TextLexicon {
    /// The readings in the lexicon.
    pub fn readings(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub(crate) fn into_indexes(self) -> (EntryIndex, ReverseIndex) {
        (self.entries, self.reverse)
    }

    /// Parses the text.  `resolve` gives each row's word its ID, called once per accepted row in file order (the
    /// engine hands out personal IDs in that order).
    ///
    /// Rows need three fields, a non-empty reading and word, and a finite score; others are skipped.
    pub fn parse(data: &[u8], mut resolve: impl FnMut(&str) -> Option<u32>) -> TextLexicon {
        let mut entries: HashMap<String, Vec<WordEntry>> = HashMap::with_capacity(50_000);
        let mut reverse: HashMap<String, Vec<(String, f64)>> = HashMap::with_capacity(125_000);
        scan_tsv(data, false, |fields| {
            if fields.len() != 3 || fields[0].is_empty() || fields[1].is_empty() {
                return;
            }
            let Some(score) = parse_double(&fields[2]).filter(|score| score.is_finite()) else { return };
            let word = fields[1].as_ref();
            let id = resolve(word);
            entries.entry(fields[0].to_string()).or_default().push(WordEntry::new(word, score).with_word_id(id));
            reverse.entry(word.to_owned()).or_default().push((fields[0].to_string(), score));
        });
        // The lexicon gives log P(word, reading); split it into the word's probability and the reading's share.
        let best: HashMap<&str, f64> = reverse
            .iter()
            .filter(|(_, readings)| readings.len() > 1)
            .map(|(word, readings)| (word.as_str(), readings.iter().map(|r| r.1).fold(f64::NEG_INFINITY, swift_max)))
            .collect();
        for list in entries.values_mut() {
            for entry in list {
                if let Some(&top) = best.get(&*entry.word) {
                    entry.reading_prior = entry.logp - top;
                    entry.logp = top;
                }
            }
        }
        TextLexicon { entries, reverse }
    }

    /// The entries of `reading`, in file order.
    pub fn entries(&self, reading: &str) -> &[WordEntry] {
        self.entries.get(reading).map_or(&[][..], Vec::as_slice)
    }

    /// Every reading of `word` with its original score, in file order.
    pub fn readings_of(&self, word: &str) -> &[(String, f64)] {
        self.reverse.get(word).map_or(&[][..], Vec::as_slice)
    }

    /// The word's most common reading: the first of the highest-scoring ones (Swift's `max(by:)`).
    #[must_use]
    pub fn best_reading(&self, word: &str) -> Option<(String, f64)> {
        let readings = self.reverse.get(word)?;
        let mut best = readings.first()?;
        for candidate in &readings[1..] {
            if best.1 < candidate.1 {
                best = candidate;
            }
        }
        Some(best.clone())
    }

    /// Whether the lexicon has the word.
    #[must_use]
    pub fn knows_word(&self, word: &str) -> bool {
        self.reverse.contains_key(word)
    }
}

/// Swift's `max(_:_:)` for floating point: `y >= x ? y : x`.
#[inline]
pub(crate) fn swift_max(x: f64, y: f64) -> f64 {
    if y >= x { y } else { x }
}

/// Swift's `min(_:_:)` for floating point: `y < x ? y : x`.
#[inline]
pub(crate) fn swift_min(x: f64, y: f64) -> f64 {
    if y < x { y } else { x }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_priors() {
        let text = "# comment\nㄅㄚ\t甲\t-5\nㄅㄚ˙\t甲\t0\nㄅㄚ\t乙\t-4\nbad\nㄅㄚ\t\t-1\nㄅㄚ\t丙\tnan\n";
        let mut order = vec![];
        let lexicon = TextLexicon::parse(text.as_bytes(), |word| {
            order.push(word.to_owned());
            Some(order.len() as u32)
        });
        assert_eq!(order, ["甲", "甲", "乙"]);
        let entries = lexicon.entries("ㄅㄚ");
        assert_eq!(entries.len(), 2);
        assert_eq!((&*entries[0].word, entries[0].logp, entries[0].reading_prior), ("甲", 0.0, -5.0));
        assert_eq!((&*entries[1].word, entries[1].logp, entries[1].reading_prior), ("乙", -4.0, 0.0));
        assert_eq!(lexicon.entries("ㄅㄚ˙")[0].reading_prior, 0.0);
        assert_eq!(lexicon.best_reading("甲"), Some(("ㄅㄚ˙".to_owned(), 0.0)));
        assert_eq!(lexicon.readings_of("甲").len(), 2);
        assert!(!lexicon.knows_word("丙"));
    }

    #[test]
    fn best_reading_keeps_the_first_maximum() {
        let lexicon = TextLexicon::parse("ㄅㄚ\t甲\t-1\nㄅㄚ˙\t甲\t-1\n".as_bytes(), |_| None);
        assert_eq!(lexicon.best_reading("甲").unwrap().0, "ㄅㄚ");
        assert_eq!(swift_max(0.0, f64::NAN), 0.0);
        assert_eq!(swift_min(1.0, 0.0), 0.0);
    }
}

//! ZTLM/1: the memory-mapped back-off n-gram model (port of `NGramLanguageModel.swift`).  Sections stay mapped; only
//! the header and array bounds are inspected at startup.  Format: `resources/formats/lm.md`.

use std::borrow::Cow;
use std::path::Path;

use super::{DataError, Mapping, f32_at, u32_at, u64_at};
use crate::language_model::{LanguageModel, LanguageModelHistory, NO_CONTEXT, VocabularyLanguageModel};

/// A ZTLM/1 model (`lm.bin`, with its `lm.json` metadata) kept memory-mapped.
pub struct NGramLanguageModel {
    map: Mapping,
    tsv_compatibility: bool,
    /// Sorted: IDs without a TSV unigram row (compatibility mode only).
    missing_unigrams: Vec<u32>,
    order: usize,
    unknown_word_shift: f64,
    vocabulary_count: usize,
    bigram_count: usize,
    trigram_count: usize,
    vocabulary: usize,
    blob: usize,
    unigrams: usize,
    bigrams: usize,
    trigrams: usize,
}

/// Metadata read from `lm.json`, with Swift `JSONDecoder` semantics: a missing file, malformed JSON or a field of
/// the wrong type leaves every field at its default.
struct Metadata {
    unknown_word_shift: Option<f64>,
    tsv_compatibility: Option<bool>,
    missing_unigrams: Option<Vec<u32>>,
}

impl Metadata {
    fn read(path: &Path) -> Option<Metadata> {
        let text = std::fs::read(path).ok()?;
        let value: serde_json::Value = serde_json::from_slice(&text).ok()?;
        let object = value.as_object()?;
        let field = |name: &str| object.get(name).filter(|value| !value.is_null());
        let unknown_word_shift = match field("unknown_word_shift") {
            Some(value) => Some(value.as_f64()?),
            None => None,
        };
        let tsv_compatibility = match field("tsv_compatibility") {
            Some(value) => Some(value.as_bool()?),
            None => None,
        };
        let missing_unigrams = match field("missing_unigrams") {
            Some(value) => Some(
                value
                    .as_array()?
                    .iter()
                    .map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok()))
                    .collect::<Option<Vec<u32>>>()?,
            ),
            None => None,
        };
        Some(Metadata { unknown_word_shift, tsv_compatibility, missing_unigrams })
    }
}

impl NGramLanguageModel {
    /// Maps `directory/lm.bin` and reads `directory/lm.json` (a missing or malformed `lm.json` leaves the defaults).
    ///
    /// # Errors
    /// [`DataError::Io`] when `lm.bin` cannot be read, [`DataError::InvalidLanguageModel`] when its header or
    /// arrays are invalid.
    pub fn open(directory: impl AsRef<Path>) -> Result<NGramLanguageModel, DataError> {
        let directory = directory.as_ref();
        let map = Mapping::open(&directory.join("lm.bin"))?;
        let data = map.bytes();
        let size = data.len();
        if size < 64 {
            return Err(DataError::InvalidLanguageModel);
        }
        let header: [u64; 8] = std::array::from_fn(|i| u64_at(data, i * 8));
        if header[0] & 0xffff_ffff != 0x4d4c_545a || header[0] >> 32 != 1 || header[7] != size as u64 {
            return Err(DataError::InvalidLanguageModel);
        }
        let order = (header[1] & 0xffff_ffff) as usize;
        let v = (header[1] >> 32) as usize;
        let b = (header[2] & 0xffff_ffff) as usize;
        let t = (header[2] >> 32) as usize;
        if !(order == 2 || order == 3) || v < 3 || !(order == 3 || (t == 0 && header[6] == 0)) {
            return Err(DataError::InvalidLanguageModel);
        }
        let section_offsets = if order == 3 { &header[3..=6] } else { &header[3..=5] };
        if !section_offsets.iter().all(|&offset| offset >= 64 && offset <= size as u64 && offset % 8 == 0)
            || !section_offsets.windows(2).all(|pair| pair[0] <= pair[1])
        {
            return Err(DataError::InvalidLanguageModel);
        }
        let vocab = header[3] as usize;
        let uni = header[4] as usize;
        let bi = header[5] as usize;
        let tri = if order == 3 { header[6] as usize } else { size };
        if (v + 1) * 4 > uni - vocab
            || v * 8 > bi - uni
            || (v + 1) * 4 + b * 12 > tri - bi
            || !(order == 2 || (b + 1) * 4 + t * 8 <= size - tri)
        {
            return Err(DataError::InvalidLanguageModel);
        }
        let blob = vocab + (v + 1) * 4;
        let uint = |offset: usize| u32_at(data, offset) as usize;
        let bounds = |offset: usize, count: usize, limit: usize| -> bool {
            if uint(offset) != 0 || uint(offset + count * 4) > limit {
                return false;
            }
            let mut previous = 0;
            for i in 0..count {
                let current = uint(offset + (i + 1) * 4);
                if current < previous || current > limit {
                    return false;
                }
                previous = current;
            }
            true
        };
        let valid = bounds(vocab, v, uni - blob)
            && bounds(bi, v, b)
            && uint(bi + v * 4) == b
            && (order == 2 || (bounds(tri, b, t) && uint(tri + b * 4) == t))
            && ["</s>", "<s>", "<unk>"].iter().enumerate().all(|(i, spelling)| {
                let start = uint(vocab + i * 4);
                let end = uint(vocab + (i + 1) * 4);
                start <= end && data[blob + start..blob + end] == *spelling.as_bytes()
            });
        if !valid {
            return Err(DataError::InvalidLanguageModel);
        }
        let metadata = Metadata::read(&directory.join("lm.json"));
        let shift = metadata.as_ref().and_then(|m| m.unknown_word_shift).unwrap_or(0.0);
        let mut missing_unigrams = metadata.as_ref().and_then(|m| m.missing_unigrams.clone()).unwrap_or_default();
        missing_unigrams.sort_unstable();
        missing_unigrams.dedup();
        Ok(NGramLanguageModel {
            tsv_compatibility: metadata.as_ref().and_then(|m| m.tsv_compatibility).unwrap_or(false),
            missing_unigrams,
            order,
            unknown_word_shift: if shift.is_finite() { shift } else { 0.0 },
            vocabulary_count: v,
            bigram_count: b,
            trigram_count: t,
            vocabulary: vocab,
            blob,
            unigrams: uni,
            bigrams: bi,
            trigrams: tri,
            map,
        })
    }

    /// The calibration added to a lexicon probability for a word the model does not know.
    #[must_use]
    pub fn unknown_word_shift(&self) -> f64 {
        self.unknown_word_shift
    }

    /// Whether the model keeps the TSV bigram model's fallback semantics (`lm.json` `tsv_compatibility`).
    #[must_use]
    pub fn tsv_compatibility(&self) -> bool {
        self.tsv_compatibility
    }

    #[inline]
    fn data(&self) -> &[u8] {
        self.map.bytes()
    }

    /// A stored log probability; anything non-finite reads as −∞.
    #[inline]
    fn float(&self, offset: usize) -> f64 {
        let value = f32_at(self.data(), offset);
        if value.is_finite() { value } else { f64::NEG_INFINITY }
    }

    #[inline]
    fn uint(&self, offset: usize) -> u32 {
        u32_at(self.data(), offset)
    }

    /// The UTF-8 bytes of word `id`.
    #[inline]
    fn spelling_bytes(&self, id: usize) -> &[u8] {
        let data = self.data();
        let start = self.blob + u32_at(data, self.vocabulary + id * 4) as usize;
        let end = self.blob + u32_at(data, self.vocabulary + (id + 1) * 4) as usize;
        &data[start..end]
    }

    /// The index of `word` in `context`'s group of `section`, which has `groups` groups.
    #[inline]
    fn find(&self, section: usize, groups: usize, context: u32, word: u32) -> Option<usize> {
        if context as usize >= groups {
            return None;
        }
        let mut low = self.uint(section + context as usize * 4) as usize;
        let end = self.uint(section + (context as usize + 1) * 4) as usize;
        let mut high = end;
        let next = section + (groups + 1) * 4;
        while low < high {
            let mid = usize::midpoint(low, high);
            if self.uint(next + mid * 4) < word { low = mid + 1 } else { high = mid }
        }
        (low < end && self.uint(next + low * 4) == word).then_some(low)
    }

    /// The BigramLanguageModel-compatible score (`tsv_compatibility`).
    fn compatible_score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        let v = self.vocabulary_count;
        let previous = history.previous;
        let backoff =
            if (previous as usize) < v { self.float(self.unigrams + (v + previous as usize) * 4) } else { 0.0 };
        let Some(word_id) = word_id.filter(|&id| (id as usize) < v) else {
            return backoff + unigram + self.unknown_word_shift;
        };
        let group = if (previous as usize) < v { previous as usize } else { 0 };
        let start = self.uint(self.bigrams + group * 4) as usize;
        let end = if (previous as usize) < v { self.uint(self.bigrams + (group + 1) * 4) as usize } else { start };
        let (mut low, mut high) = (start, end);
        let next = self.bigrams + (v + 1) * 4;
        while low < high {
            let mid = usize::midpoint(low, high);
            if self.uint(next + mid * 4) < word_id { low = mid + 1 } else { high = mid }
        }
        if low < end && self.uint(next + low * 4) == word_id {
            return self.float(next + (self.bigram_count + low) * 4);
        }
        backoff
            + if self.missing_unigrams.binary_search(&word_id).is_ok() {
                unigram
            } else {
                self.float(self.unigrams + word_id as usize * 4)
            }
    }
}

impl LanguageModel for NGramLanguageModel {
    fn order(&self) -> usize {
        self.order
    }

    fn word_id(&self, word: &str) -> Option<u32> {
        match word {
            "</s>" => return Some(0),
            "<s>" => return Some(1),
            "<unk>" => return Some(2),
            _ => {}
        }
        let key = word.as_bytes();
        let (mut low, mut high) = (3usize, self.vocabulary_count);
        while low < high {
            let mid = usize::midpoint(low, high);
            match self.spelling_bytes(mid).cmp(key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Some(mid as u32),
            }
        }
        None
    }

    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        if self.tsv_compatibility {
            return self.compatible_score(word_id, history, unigram);
        }
        let v = self.vocabulary_count;
        let Some(word_id) = word_id.filter(|&id| (id as usize) < v) else {
            return unigram + self.unknown_word_shift;
        };
        if history.previous == NO_CONTEXT {
            return self.float(self.unigrams + word_id as usize * 4);
        }
        let mut backoff = 0.0;
        if self.order == 3
            && let Some(older) = history.older
            && let Some(context) = self.find(self.bigrams, v, older, history.previous)
        {
            if let Some(index) = self.find(self.trigrams, self.bigram_count, context as u32, word_id) {
                return self.float(self.trigrams + (self.bigram_count + 1 + self.trigram_count + index) * 4);
            }
            backoff = self.float(self.bigrams + (v + 1 + self.bigram_count * 2 + context) * 4);
        }
        if let Some(index) = self.find(self.bigrams, v, history.previous, word_id) {
            return backoff + self.float(self.bigrams + (v + 1 + self.bigram_count + index) * 4);
        }
        let previous = if (history.previous as usize) < v { history.previous } else { 2 };
        backoff
            + self.float(self.unigrams + (v + previous as usize) * 4)
            + self.float(self.unigrams + word_id as usize * 4)
    }
}

impl VocabularyLanguageModel for NGramLanguageModel {
    fn vocabulary_count(&self) -> usize {
        self.vocabulary_count
    }

    /// The spelling of word `id`; invalid UTF-8 is replaced with U+FFFD, as Swift's `String(decoding:as:)` does.
    ///
    /// # Panics
    /// When `id` is not below [`vocabulary_count`](Self::vocabulary_count).
    fn word(&self, id: u32) -> Cow<'_, str> {
        assert!((id as usize) < self.vocabulary_count, "word ID {id} outside the vocabulary");
        String::from_utf8_lossy(self.spelling_bytes(id as usize))
    }
}

//! Language models (port of `LanguageModel.swift`): the history type, the model traits, the unigram fallback, the
//! TSV bigram model, the two-domain mixture and the boundary-free wrapper.  The memory-mapped n-gram model is
//! [`crate::binary::NGramLanguageModel`].
//!
//! Scores are natural logarithms.  Word IDs 0, 1 and 2 are `</s>`, `<s>` and `<unk>`; a `None` word ID scores with
//! the lexicon probability passed as `unigram`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::binary::{DataError, NGramLanguageModel};
use crate::lexicon::{swift_max, swift_min};
use crate::tsv::{parse_double, scan_tsv};

/// The history value meaning "no context": the word is scored by its unigram probability.
pub const NO_CONTEXT: u32 = u32::MAX;

/// The words before the one being scored: the previous word and, for trigram models, the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LanguageModelHistory {
    /// The word before `previous` (trigram models only).
    pub older: Option<u32>,
    /// The previous word; 1 (`<s>`) at the sentence start, [`NO_CONTEXT`] for none.
    pub previous: u32,
}

impl Default for LanguageModelHistory {
    /// The sentence start.
    fn default() -> Self {
        Self { older: None, previous: 1 }
    }
}

impl LanguageModelHistory {
    /// A history with the given words.
    #[must_use]
    pub fn new(older: Option<u32>, previous: u32) -> Self {
        Self { older, previous }
    }

    /// The history after `word_id` (`None` is scored as `<unk>`), for a model of `order`.
    #[must_use]
    #[inline]
    pub fn appending(self, word_id: Option<u32>, order: usize) -> Self {
        Self {
            older: if order == 3 { Some(self.previous) } else { None },
            previous: if order == 1 { 1 } else { word_id.unwrap_or(2) },
        }
    }
}

/// A language model.  Resolve strings to IDs once, before decoding.
pub trait LanguageModel: Send + Sync {
    /// 1, 2 or 3.
    fn order(&self) -> usize;

    /// The model's ID for `word`.
    fn word_id(&self, word: &str) -> Option<u32>;

    /// log P(word | history).  `unigram` is the lexicon's log probability, used for words the model lacks.
    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64;

    /// Where a composition stops: the sentence ends there with probability `share`, otherwise the user goes on
    /// typing (a continuation every next word shares, so it costs nothing):
    /// `log(share × P(</s> | history) + (1 − share))`.  `share` is the engine's sentence-end share (1 in the app).
    fn end_score(&self, history: LanguageModelHistory, share: f64) -> f64 {
        let end = self.score(Some(0), history, 0.0);
        // Swift: `guard share < 1 else { return end }`, so NaN also returns `end`.
        if share.partial_cmp(&1.0) != Some(std::cmp::Ordering::Less) {
            return end;
        }
        (share * end.exp() + (1.0 - share)).ln()
    }

    /// log P(`word` | `previous_word`), resolving both strings; unknown words are `<unk>` as history.
    fn score_after(&self, word: &str, previous_word: &str, unigram: f64) -> f64 {
        let history = LanguageModelHistory::new(None, self.word_id(previous_word).unwrap_or(2));
        self.score(self.word_id(word), history, unigram)
    }

    /// [`end_score`](Self::end_score) after `previous_word`.
    fn end_score_after(&self, previous_word: &str, share: f64) -> f64 {
        self.end_score(LanguageModelHistory::new(None, self.word_id(previous_word).unwrap_or(2)), share)
    }
}

/// A model with an enumerable vocabulary (IDs `0..vocabulary_count`).
pub trait VocabularyLanguageModel: LanguageModel {
    /// Number of word IDs.
    fn vocabulary_count(&self) -> usize;

    /// The spelling of word `id`.
    fn word(&self, id: u32) -> Cow<'_, str>;
}

/// No language model: every word scores its lexicon probability.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnigramLanguageModel;

impl LanguageModel for UnigramLanguageModel {
    fn order(&self) -> usize {
        1
    }
    fn word_id(&self, _word: &str) -> Option<u32> {
        None
    }
    fn score(&self, _word_id: Option<u32>, _history: LanguageModelHistory, unigram: f64) -> f64 {
        unigram
    }
}

/// The bigram model read from `unigram.tsv`, `bigram.tsv` and `backoff.tsv` (fields separated by tabs or spaces).
pub struct BigramLanguageModel {
    ids: HashMap<String, u32>,
    words: Vec<String>,
    unigrams: HashMap<u32, f64>,
    bigrams: HashMap<u32, HashMap<u32, f64>>,
    backoffs: HashMap<u32, f64>,
}

impl BigramLanguageModel {
    /// Reads the three files of `directory`; each must exist.
    ///
    /// # Errors
    /// [`DataError::Io`] when a file cannot be read.  Malformed rows are skipped, not errors.
    pub fn open(directory: impl AsRef<Path>) -> Result<BigramLanguageModel, DataError> {
        let directory = directory.as_ref();
        let unigram = std::fs::read(directory.join("unigram.tsv"))?;
        let bigram = std::fs::read(directory.join("bigram.tsv"))?;
        let backoff = std::fs::read(directory.join("backoff.tsv"))?;
        Ok(Self::from_tsv(&unigram, &bigram, &backoff))
    }

    /// Builds the model from the three files' contents.
    #[must_use]
    pub fn from_tsv(unigram: &[u8], bigram: &[u8], backoff: &[u8]) -> BigramLanguageModel {
        let mut model = BigramLanguageModel {
            ids: HashMap::from([("</s>".to_owned(), 0), ("<s>".to_owned(), 1), ("<unk>".to_owned(), 2)]),
            words: vec!["</s>".to_owned(), "<s>".to_owned(), "<unk>".to_owned()],
            unigrams: HashMap::new(),
            bigrams: HashMap::new(),
            backoffs: HashMap::new(),
        };
        scan_tsv(unigram, true, |fields| {
            if fields.len() != 2 {
                return;
            }
            if let Some(score) = parse_double(&fields[1]).filter(|s| s.is_finite()) {
                let id = model.intern(&fields[0]);
                model.unigrams.insert(id, score);
            }
        });
        scan_tsv(bigram, true, |fields| {
            if fields.len() != 3 {
                return;
            }
            if let Some(score) = parse_double(&fields[2]).filter(|s| s.is_finite()) {
                let first = model.intern(&fields[0]);
                let second = model.intern(&fields[1]);
                model.bigrams.entry(first).or_default().insert(second, score);
            }
        });
        scan_tsv(backoff, true, |fields| {
            if fields.len() != 2 {
                return;
            }
            if let Some(score) = parse_double(&fields[1]).filter(|s| s.is_finite()) {
                let id = model.intern(&fields[0]);
                model.backoffs.insert(id, score);
            }
        });
        model
    }

    fn intern(&mut self, word: &str) -> u32 {
        if let Some(&id) = self.ids.get(word) {
            return id;
        }
        let id = self.ids.len() as u32;
        self.ids.insert(word.to_owned(), id);
        self.words.push(word.to_owned());
        id
    }

    /// Every word the model knows, with its ID.
    #[must_use]
    pub fn vocabulary(&self) -> &HashMap<String, u32> {
        &self.ids
    }
}

impl LanguageModel for BigramLanguageModel {
    fn order(&self) -> usize {
        2
    }
    fn word_id(&self, word: &str) -> Option<u32> {
        self.ids.get(word).copied()
    }
    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        let word = word_id.unwrap_or(u32::MAX);
        if let Some(&score) = self.bigrams.get(&history.previous).and_then(|next| next.get(&word)) {
            return score;
        }
        self.backoffs.get(&history.previous).copied().unwrap_or(0.0)
            + self.unigrams.get(&word).copied().unwrap_or(unigram)
    }
}

impl VocabularyLanguageModel for BigramLanguageModel {
    fn vocabulary_count(&self) -> usize {
        self.words.len()
    }
    fn word(&self, id: u32) -> Cow<'_, str> {
        Cow::Borrowed(&self.words[id as usize])
    }
}

/// One loaded model directory: the memory-mapped binary when it loads, else the TSV files.  An enum rather than a
/// trait object so the mixture's per-word calls dispatch statically.
pub enum VocabularyModel {
    /// `lm.bin` (+ `lm.json`).
    NGram(NGramLanguageModel),
    /// `unigram.tsv`, `bigram.tsv`, `backoff.tsv`.
    Bigram(BigramLanguageModel),
}

impl VocabularyModel {
    /// Loads `directory` as the Swift engine does: `lm.bin` first, the TSV files when the binary is missing or
    /// invalid, `None` when neither loads.
    pub fn load(directory: impl AsRef<Path>) -> Option<VocabularyModel> {
        let directory = directory.as_ref();
        if let Ok(model) = NGramLanguageModel::open(directory) {
            return Some(VocabularyModel::NGram(model));
        }
        BigramLanguageModel::open(directory).ok().map(VocabularyModel::Bigram)
    }
}

macro_rules! dispatch {
    ($self:ident, $model:ident => $body:expr) => {
        match $self {
            VocabularyModel::NGram($model) => $body,
            VocabularyModel::Bigram($model) => $body,
        }
    };
}

impl LanguageModel for VocabularyModel {
    #[inline]
    fn order(&self) -> usize {
        dispatch!(self, m => m.order())
    }
    #[inline]
    fn word_id(&self, word: &str) -> Option<u32> {
        dispatch!(self, m => m.word_id(word))
    }
    #[inline]
    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        dispatch!(self, m => m.score(word_id, history, unigram))
    }
}

impl VocabularyLanguageModel for VocabularyModel {
    #[inline]
    fn vocabulary_count(&self) -> usize {
        dispatch!(self, m => m.vocabulary_count())
    }
    #[inline]
    fn word(&self, id: u32) -> Cow<'_, str> {
        dispatch!(self, m => m.word(id))
    }
}

/// Two bigram models of different domains, mixed: P = λ·P(chat) + (1 − λ)·P(encyclopedia).  λ follows what the
/// user is writing ([`set_chat_weight`](Self::set_chat_weight)).  Word IDs are the encyclopedic model's, then new
/// ones for words only the chat model knows.
pub struct MixtureLanguageModel<C = VocabularyModel, E = VocabularyModel> {
    chat: C,
    encyclopedia: E,
    /// Per mixture ID, the chat model's ID (`u32::MAX` when it lacks the word).
    chat_ids: Vec<u32>,
    encyclopedia_count: u32,
    /// The chat share, as `f64` bits: read on every score, set from another thread when the context changes.
    weight: AtomicU64,
}

impl<C: VocabularyLanguageModel, E: VocabularyLanguageModel> MixtureLanguageModel<C, E> {
    /// Mixes the two models with the given chat share (not clamped, as in Swift's initializer).
    pub fn new(chat: C, encyclopedia: E, chat_weight: f64) -> Self {
        let encyclopedia_count = encyclopedia.vocabulary_count() as u32;
        let chat_ids =
            (0..encyclopedia_count).map(|id| chat.word_id(&encyclopedia.word(id)).unwrap_or(u32::MAX)).collect();
        Self { chat, encyclopedia, chat_ids, encyclopedia_count, weight: AtomicU64::new(chat_weight.to_bits()) }
    }

    /// The chat model's share.
    pub fn chat_weight(&self) -> f64 {
        f64::from_bits(self.weight.load(Ordering::Relaxed))
    }

    /// Sets the chat model's share, clamped to 0…1.
    pub fn set_chat_weight(&self, weight: f64) {
        self.weight.store(swift_min(1.0, swift_max(0.0, weight)).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn chat_id(&self, id: Option<u32>) -> Option<u32> {
        let id = id?;
        if id == NO_CONTEXT {
            return Some(id);
        }
        if id >= self.encyclopedia_count {
            let chat_id = id - self.encyclopedia_count;
            return ((chat_id as usize) < self.chat.vocabulary_count()).then_some(chat_id);
        }
        let mapped = self.chat_ids[id as usize];
        (mapped != u32::MAX).then_some(mapped)
    }

    #[inline]
    fn encyclopedia_id(&self, id: Option<u32>) -> Option<u32> {
        let id = id?;
        (id < self.encyclopedia_count || id == NO_CONTEXT).then_some(id)
    }

    /// log P under each model (chat, encyclopedia), for the domain posterior.
    pub fn scores(&self, word_id: Option<u32>, previous: u32, unigram: f64) -> (f64, f64) {
        let c = self.chat.score(
            self.chat_id(word_id),
            LanguageModelHistory::new(None, self.chat_id(Some(previous)).unwrap_or(2)),
            unigram,
        );
        let e = self.encyclopedia.score(
            self.encyclopedia_id(word_id),
            LanguageModelHistory::new(None, self.encyclopedia_id(Some(previous)).unwrap_or(2)),
            unigram,
        );
        (c, e)
    }
}

impl<C: VocabularyLanguageModel, E: VocabularyLanguageModel> LanguageModel for MixtureLanguageModel<C, E> {
    fn order(&self) -> usize {
        2
    }

    fn word_id(&self, word: &str) -> Option<u32> {
        self.encyclopedia.word_id(word).or_else(|| self.chat.word_id(word).map(|id| self.encyclopedia_count + id))
    }

    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        let (chat, encyclopedia) = self.scores(word_id, history.previous, unigram);
        let weight = self.chat_weight();
        if weight <= 0.0 {
            return encyclopedia;
        }
        if weight >= 1.0 {
            return chat;
        }
        let chat_term = weight.ln() + chat;
        let encyclopedia_term = (1.0 - weight).ln() + encyclopedia;
        let top = swift_max(chat_term, encyclopedia_term);
        top + ((chat_term - top).exp() + (encyclopedia_term - top).exp()).ln()
    }
}

/// Treats a composition as a stretch of a sentence rather than a whole one: what would be scored after `<s>` is
/// scored with no context, and nothing is added for `</s>`.
pub struct BoundaryFreeLanguageModel<M> {
    /// The model being wrapped.
    pub base: M,
}

impl<M: LanguageModel> LanguageModel for BoundaryFreeLanguageModel<M> {
    fn order(&self) -> usize {
        self.base.order()
    }
    fn word_id(&self, word: &str) -> Option<u32> {
        self.base.word_id(word)
    }
    fn score(&self, word_id: Option<u32>, mut history: LanguageModelHistory, unigram: f64) -> f64 {
        if word_id == Some(0) {
            return 0.0;
        }
        if history.previous == 1 {
            history.previous = NO_CONTEXT;
            history.older = None;
        }
        if history.older == Some(1) {
            history.older = None;
        }
        self.base.score(word_id, history, unigram)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_appending() {
        let start = LanguageModelHistory::default();
        assert_eq!(start.appending(Some(7), 3), LanguageModelHistory::new(Some(1), 7));
        assert_eq!(start.appending(None, 2), LanguageModelHistory::new(None, 2));
        assert_eq!(start.appending(Some(7), 1), LanguageModelHistory::new(None, 1));
    }

    #[test]
    fn end_share() {
        let model = UnigramLanguageModel;
        assert_eq!(model.end_score(LanguageModelHistory::default(), 1.0), 0.0);
        assert_eq!(model.end_score(LanguageModelHistory::default(), 0.5), 0.0);
    }

    #[test]
    fn boundary_free() {
        let model = BoundaryFreeLanguageModel { base: UnigramLanguageModel };
        assert_eq!(model.score(Some(0), LanguageModelHistory::default(), -3.0), 0.0);
        assert_eq!(model.score(Some(5), LanguageModelHistory::default(), -3.0), -3.0);
    }
}

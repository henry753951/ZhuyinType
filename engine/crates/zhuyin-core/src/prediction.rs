//! 聯想詞: the words likely to come right after committed text, from the shipped language models and the user's
//! own n-grams.  Nothing is trained or stored here; the composer shows the list after a commit
//! ([`crate::composer::Composer::set_predicts_next_words`]).
//!
//! Candidates are the words the n-gram models store after the last word (and the one before it, for trigrams),
//! plus the words the user wrote after it.  Each is kept only when it is all Han characters and in the lexicon (a
//! word the user could type), and when it is likely enough: a single character needs a higher probability than a
//! longer word, so rare characters do not fill the list.  What the user wrote there comes first, strongest first,
//! then the model's words by P(word | history).

use std::collections::HashSet;

use super::{Engine, State, is_han};
use crate::language_model::{LanguageModel, LanguageModelHistory, NO_CONTEXT};
use crate::swift_format::graphemes;

/// How many followers each source contributes before filtering and ranking.
const POOL: usize = 64;
/// Least log P(word | history) for a single character (2%).
const MIN_CHARACTER_SCORE: f64 = -3.912;
/// Least log P(word | history) for a word of two or more characters (0.1%).
const MIN_WORD_SCORE: f64 = -6.908;
/// Least personal strength (decaying counts; one pick is 1) for a word the user wrote there to come first.
const MIN_PERSONAL_STRENGTH: f64 = 0.25;
/// Longest suggestion, in characters (the lexicon's longest words).
const MAX_CHARACTERS: usize = 6;

/// One suggested next word.
#[derive(Clone, Debug, PartialEq)]
pub struct Prediction {
    /// The word.
    pub text: String,
    /// Its most common reading.
    pub reading: String,
    /// Its language-model (or personal) ID.
    pub word_id: u32,
    /// The user wrote it after the same words before (it comes from their n-grams).
    pub personal: bool,
}

impl Engine {
    /// Up to `count` words likely to follow `context` (text before the insertion point), best first.  `personal`:
    /// use the user's n-grams; off when learning is off (「記住選過的字詞」), so only the shipped models suggest.
    #[must_use]
    pub fn predict_next(&self, context: &str, count: usize, personal: bool) -> Vec<Prediction> {
        let words = self.trailing_words(context);
        let Some(history) = words.last().and_then(|w| w.history) else { return vec![] };
        self.predict_after(history, count, personal)
    }

    /// Up to `count` words likely to follow `history` (the history after the last committed word), best first.
    /// Empty at a sentence start or after an unknown word: there is nothing to predict from.
    #[must_use]
    pub fn predict_after(&self, history: LanguageModelHistory, count: usize, personal: bool) -> Vec<Prediction> {
        if count == 0 || history.previous <= 2 || history.previous == NO_CONTEXT {
            return vec![];
        }
        let state = self.lock();
        let mut seen: HashSet<String> = HashSet::new();
        let mut predictions: Vec<Prediction> = Vec::new();
        if personal {
            for (id, text, strength) in self.personal.followers(history, POOL) {
                if predictions.len() >= count {
                    break;
                }
                if strength < MIN_PERSONAL_STRENGTH || seen.contains(&text) {
                    continue;
                }
                if let Some(reading) = self.suggestible(&state, &text) {
                    seen.insert(text.clone());
                    predictions.push(Prediction { text, reading, word_id: id, personal: true });
                }
            }
        }
        let mut scored: Vec<(f64, u32, String)> = Vec::new();
        for (id, text) in self.model.followers(history, POOL) {
            if seen.contains(&text) {
                continue;
            }
            let score = if personal {
                self.model.score(Some(id), history, f64::NEG_INFINITY)
            } else {
                self.model.base_score(Some(id), history, f64::NEG_INFINITY)
            };
            let least = if graphemes(&text).len() == 1 { MIN_CHARACTER_SCORE } else { MIN_WORD_SCORE };
            if score.is_finite() && score >= least {
                scored.push((score, id, text));
            }
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, id, text) in scored {
            if predictions.len() >= count {
                break;
            }
            if seen.contains(&text) {
                continue;
            }
            if let Some(reading) = self.suggestible(&state, &text) {
                seen.insert(text.clone());
                predictions.push(Prediction { text, reading, word_id: id, personal: false });
            }
        }
        predictions
    }

    /// The reading of `text` when it can be suggested: Han characters only, a lexicon word, not set to 不記住.
    fn suggestible(&self, state: &State, text: &str) -> Option<String> {
        let characters = graphemes(text);
        if characters.is_empty() || characters.len() > MAX_CHARACTERS || !characters.iter().all(|c| is_han(c)) {
            return None;
        }
        if !self.knows_word(state, text) || state.excluded.iter().any(|key| key.word == text) {
            return None;
        }
        self.best_reading(state, text).map(|(reading, _)| reading)
    }
}

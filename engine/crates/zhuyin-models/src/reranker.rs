//! The sentence reranker (Swift's `NeuralReranker`): chooses the displayed conversion among the engine's N best with
//! a character language model, and orders candidate lists.

use std::path::Path;
use std::sync::Mutex;

use zhuyin_core::api::ScoredSentence;
use zhuyin_core::swift_format::graphemes;

use crate::tree::RerankerTree;
use crate::vocabulary::{CharacterIndex, character_index, read_strings};
use crate::{ModelError, RerankerModel};

/// The texts last scored by [`NeuralReranker::choose`]: a key that adds only an unfinished Zhuyin symbol leaves the
/// sentences as they were, and is answered without running the model.
struct LastChoice {
    context: String,
    texts: Vec<String>,
    neural: Vec<f64>,
}

/// Each candidate sentence is scored as log P(candidate | text before it) by one model call; the choice is the
/// candidate with the highest engine score + `weight` × that log probability.  The model directory holds
/// `vocab.json` (and `config.json` for [`configured`](Self::configured)); the numeric model is a [`RerankerModel`].
pub struct NeuralReranker {
    /// How much the model's log probability counts against the engine's score.
    pub weight: f64,
    /// How many of the engine's best conversions to choose among.
    pub candidate_count: usize,
    /// Characters of preceding text given to the model.
    pub context_length: usize,
    /// log of how many characters share the model's `<unk>` token (5,465 lexicon characters are outside its
    /// 12,000): each gets its share of `<unk>`'s probability, not all of it, so rare characters do not outrank common
    /// ones.
    pub unknown_penalty: f64,
    /// How much better a corrected sentence must score than the best one matching the keys as typed to be shown.  8
    /// under the app setup (reranker weight 1.0): 4 let a corrected reading win where the keys were right (ㄜˇ became
    /// 與); 8 keeps standard typing untouched on the dev sets and costs 0.5–1.7 points with 10% mistyped keys.
    pub exact_margin: f64,
    /// The same for a sentence corrected only by the reading channel (a phonetic-series misreading): 4 to start
    /// (`resources/formats/reading-channel.md`), tuned on the development sets.
    pub reading_margin: f64,
    /// τ: the model is consulted only when the engine's two best sentences are this close (`docs/engine/MODEL.md`
    /// §7).
    pub gate_margin: f64,
    /// δ: the model's choice replaces the engine's first sentence only when it wins by at least this much.
    pub switch_margin: f64,
    index: CharacterIndex,
    max_length: usize,
    /// Swift's `lock`: one model call at a time.
    model: Mutex<Box<dyn RerankerModel>>,
    last_choice: Mutex<Option<LastChoice>>,
}

impl std::fmt::Debug for NeuralReranker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralReranker").field("weight", &self.weight).finish_non_exhaustive()
    }
}

impl NeuralReranker {
    /// A reranker reading `vocab.json` from `directory`, running `model`.
    ///
    /// # Errors
    /// When `vocab.json` cannot be read.
    pub fn new(directory: &Path, weight: f64, model: Box<dyn RerankerModel>) -> Result<Self, ModelError> {
        let vocabulary = read_strings(&directory.join("vocab.json"))?;
        Ok(NeuralReranker {
            weight,
            candidate_count: 16,
            context_length: 24,
            unknown_penalty: 5465f64.ln(),
            exact_margin: 8.0,
            reading_margin: 4.0,
            gate_margin: 8.0,
            switch_margin: 0.0,
            index: character_index(&vocabulary, 3),
            max_length: 64,
            model: Mutex::new(model),
            last_choice: Mutex::new(None),
        })
    }

    /// The reranker as the app loads it: weight and sentence count from the directory's `config.json` (weight 0.5
    /// and the default count without one).
    ///
    /// # Errors
    /// When `vocab.json` cannot be read.
    pub fn configured(directory: &Path, model: Box<dyn RerankerModel>) -> Result<Self, ModelError> {
        let config = std::fs::read(directory.join("config.json"))
            .ok()
            .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok());
        let weight = config.as_ref().and_then(|c| c.get("weight")).and_then(serde_json::Value::as_f64).unwrap_or(0.5);
        let mut reranker = NeuralReranker::new(directory, weight, model)?;
        if let Some(count) = config.as_ref().and_then(|c| c.get("count")).and_then(json_count) {
            reranker.candidate_count = count;
        }
        Ok(reranker)
    }

    fn encode(&self, text: &str) -> Vec<i32> {
        graphemes(text).into_iter().map(|c| self.index.get(c).unwrap_or(2)).collect()
    }

    /// log P(candidate | context) for each candidate; `None` when the model cannot score them.
    #[must_use]
    pub fn scores<S: AsRef<str>>(&self, context: &str, candidates: &[S]) -> Option<Vec<f64>> {
        if candidates.is_empty() {
            return Some(vec![]);
        }
        let encoded: Vec<Vec<i32>> = candidates.iter().map(|c| self.encode(c.as_ref())).collect();
        let longest = encoded.iter().map(Vec::len).max().unwrap_or(0);
        if longest == 0 || longest >= self.max_length {
            return None;
        }
        let context_characters = graphemes(context);
        let keep = self.context_length.min(self.max_length - 1 - longest);
        let tail = &context_characters[context_characters.len().saturating_sub(keep)..];
        let mut prefix = Vec::with_capacity(tail.len() + 1);
        prefix.push(1);
        prefix.extend(tail.iter().map(|c| self.index.get(c).unwrap_or(2)));
        let mut model = self.model.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if model.has_tree() && candidates.len() <= 32 {
            let tree = RerankerTree::new(&prefix, &encoded);
            if let Ok(logprob) = model.tree_logprob(tree.packed())
                && let Some(result) = tree.scores(&logprob, self.unknown_penalty)
            {
                return Some(result);
            }
        }
        // The model accepts sequences of 2 or more; a lone character after <bos> is padded.
        let length = 2.max(prefix.len() - 1 + longest);
        let mut ids = vec![0; candidates.len() * length];
        let mut targets = ids.clone();
        for (row, candidate) in encoded.iter().enumerate() {
            let tokens: Vec<i32> = prefix.iter().chain(candidate).copied().collect();
            for column in 0..length {
                ids[row * length + column] = if column + 1 < tokens.len() { tokens[column] } else { 0 };
                targets[row * length + column] = if column + 1 < tokens.len() { tokens[column + 1] } else { 0 };
            }
        }
        let logprob = model.logprob(&ids, &targets, candidates.len(), length).ok()?;
        drop(model);
        if logprob.len() != ids.len() || !logprob.iter().all(|v| v.is_finite()) {
            return None;
        }
        let start = prefix.len() - 1;
        Some(
            encoded
                .iter()
                .enumerate()
                .map(|(row, candidate)| {
                    let sum =
                        (start..start + candidate.len()).fold(0.0, |sum, column| sum + logprob[row * length + column]);
                    sum - self.unknown_penalty * candidate.iter().filter(|&&id| id == 2).count() as f64
                })
                .collect(),
        )
    }

    /// `weight` × log P(candidate | context) for candidate-list ordering, scored in batches the model accepts.
    #[must_use]
    pub fn candidate_scores<S: AsRef<str>>(&self, context: &str, candidates: &[S]) -> Option<Vec<f64>> {
        let mut result = Vec::with_capacity(candidates.len());
        for part in candidates.chunks(32) {
            result.extend(self.scores(context, part)?.into_iter().map(|score| self.weight * score));
        }
        Some(result)
    }

    /// The index of the sentence to display.  The keys as typed win unless a correction scores clearly better.
    #[must_use]
    pub fn choose(&self, sentences: &[ScoredSentence], context: &str) -> usize {
        // The engine is sure (its two best are far apart): keep its choice without running the model.
        if sentences.len() > 1 && sentences[0].score - sentences[1].score >= self.gate_margin {
            return 0;
        }
        let texts: Vec<String> = sentences.iter().map(ScoredSentence::text).collect();
        let cached = {
            let last = self.last_choice.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            last.as_ref().filter(|l| l.context == context && l.texts == texts).map(|l| l.neural.clone())
        };
        if sentences.len() <= 1 {
            return 0;
        }
        let was_cached = cached.is_some();
        let Some(neural) = cached.or_else(|| self.scores(context, &texts)) else { return 0 };
        if !was_cached {
            *self.last_choice.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(LastChoice { context: context.to_owned(), texts, neural: neural.clone() });
        }
        let totals: Vec<f64> = sentences.iter().zip(&neural).map(|(s, n)| s.score + self.weight * n).collect();
        let mut best = first_max(totals.iter().copied().enumerate()).unwrap_or(0);
        if best != 0 && totals[best] - totals[0] < self.switch_margin {
            best = 0;
        }
        if !sentences[best].corrected() {
            return best;
        }
        let exact = first_max(totals.iter().copied().enumerate().filter(|&(i, _)| !sentences[i].corrected()));
        let margin = if sentences[best].reading_corrected_only() { self.reading_margin } else { self.exact_margin };
        match exact {
            Some(exact) if totals[best] - totals[exact] < margin => exact,
            _ => best,
        }
    }
}

/// Swift's `max(by: <)`: the first of the largest values.
fn first_max(values: impl Iterator<Item = (usize, f64)>) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, value) in values {
        if best.is_none_or(|(_, b)| b < value) {
            best = Some((index, value));
        }
    }
    best.map(|(index, _)| index)
}

/// A JSON number Swift reads `as? Int` with a positive value.
fn json_count(value: &serde_json::Value) -> Option<usize> {
    let count = match value.as_u64() {
        Some(count) => count,
        None => format!("{}", value.as_f64().filter(|v| v.fract() == 0.0)?).parse().ok()?,
    };
    usize::try_from(count).ok().filter(|&c| c > 0)
}

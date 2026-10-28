//! The v3 character model (Swift's `SyllableScorer`): a causal context encoder run once per composition, and a
//! bidirectional syllable encoder that, for every syllable of the composition, gives log P(character | all
//! syllables, the text before) over the characters it may be.  The composer adds these scores to its word lattice.
//! Model files and vocabularies come from `ml/src/zhuyintype_ml/training/v3` (`data/build/v3`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use zhuyin_core::api::CharacterModelSyllable;
use zhuyin_core::swift_format::graphemes;

use crate::vocabulary::{CharacterIndex, character_index, read_strings};
use crate::{ModelError, SyllableInput, SyllableModel};

/// A recent request and its answer: a key that leaves the syllables and candidates as they were is answered without
/// running the model.  Two are kept: with the reading channel the composer asks a second question per key.
struct LastRequest {
    context: String,
    syllables: Vec<CharacterModelSyllable>,
    candidates: Vec<Vec<String>>,
    scores: Vec<Vec<f64>>,
}

/// Scores each syllable's candidate characters with a [`SyllableModel`].
pub struct SyllableScorer {
    character_ids: CharacterIndex,
    syllable_ids: HashMap<Box<str>, i32>,
    /// Swift's `lock`: one model call at a time.
    model: Mutex<Box<dyn SyllableModel>>,
    last_requests: Mutex<Vec<LastRequest>>,
}

impl std::fmt::Debug for SyllableScorer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyllableScorer").finish_non_exhaustive()
    }
}

impl SyllableScorer {
    /// At most this many candidates per syllable reach the model; the caller prunes.
    pub const MAX_CANDIDATES: usize = 64;
    /// Characters of preceding text given to the context encoder.
    pub const MAX_CONTEXT: usize = 255;

    /// A scorer reading `chars.json` and `syllables.json` from `directory`, running `model`.
    ///
    /// # Errors
    /// When the vocabularies cannot be read.
    pub fn new(directory: &Path, model: Box<dyn SyllableModel>) -> Result<Self, ModelError> {
        let characters = read_strings(&directory.join("chars.json"))?;
        let syllables = read_strings(&directory.join("syllables.json"))?;
        let mut syllable_ids = HashMap::with_capacity(syllables.len());
        for (id, syllable) in syllables.iter().enumerate().skip(2) {
            if syllable_ids.insert(syllable.as_str().into(), i32::try_from(id).unwrap_or(i32::MAX)).is_some() {
                return Err(ModelError(format!("syllables.json: duplicate syllable {syllable}")));
            }
        }
        Ok(SyllableScorer {
            character_ids: character_index(&characters, 4),
            syllable_ids,
            model: Mutex::new(model),
            last_requests: Mutex::new(Vec::new()),
        })
    }

    /// The model's ID of a character (one grapheme); `<unk>` (2) when it has none.
    #[must_use]
    pub fn character_id(&self, character: &str) -> i32 {
        self.character_ids.get(character).unwrap_or(2)
    }

    /// The context encoder's input for the text before the composition: `<bos>`, then the last 255 characters with
    /// line breaks as `<sep>`.
    #[must_use]
    pub fn context_ids(&self, text: &str) -> Vec<i32> {
        let characters = graphemes(text);
        let tail = &characters[characters.len().saturating_sub(Self::MAX_CONTEXT)..];
        let mut ids = Vec::with_capacity(tail.len() + 1);
        ids.push(1);
        ids.extend(tail.iter().map(|&c| if c == "\n" { 3 } else { self.character_id(c) }));
        ids
    }

    /// For each syllable, log P over its candidate characters (same order; at most [`MAX_CANDIDATES`](
    /// Self::MAX_CANDIDATES) each, the caller prunes).  `None` when the model cannot run or the input is empty or too
    /// long.
    #[must_use]
    pub fn scores(
        &self,
        context: &str,
        syllables: &[CharacterModelSyllable],
        candidates: &[Vec<String>],
    ) -> Option<Vec<Vec<f64>>> {
        if syllables.is_empty() || syllables.len() != candidates.len() || syllables.len() > 64 {
            return None;
        }
        {
            let last = self.last_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(last) =
                last.iter().find(|l| l.context == context && l.syllables == syllables && l.candidates == candidates)
            {
                return Some(last.scores.clone());
            }
        }
        let result = self.compute_scores(context, syllables, candidates)?;
        let mut last = self.last_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.len() >= 2 {
            last.remove(0);
        }
        last.push(LastRequest {
            context: context.to_owned(),
            syllables: syllables.to_vec(),
            candidates: candidates.to_vec(),
            scores: result.clone(),
        });
        Some(result)
    }

    fn compute_scores(
        &self,
        context: &str,
        syllables: &[CharacterModelSyllable],
        candidates: &[Vec<String>],
    ) -> Option<Vec<Vec<f64>>> {
        let width = Self::MAX_CANDIDATES.min(candidates.iter().map(Vec::len).max().unwrap_or(1)).max(1);
        let ids: Vec<Vec<i32>> = candidates
            .iter()
            .map(|list| {
                let mut row = vec![0; width];
                if list.is_empty() {
                    row[0] = 2;
                }
                for (slot, character) in row.iter_mut().zip(list) {
                    *slot = self.character_id(character);
                }
                row
            })
            .collect();
        let bases: Vec<i32> = syllables
            .iter()
            .map(|s| s.base.as_deref().and_then(|b| self.syllable_ids.get(b).copied()).unwrap_or(1))
            .collect();
        let tones: Vec<i32> = syllables.iter().map(|s| s.tone).collect();
        let costs: Vec<i32> = syllables.iter().map(|s| s.cost_bucket).collect();
        let boundaries: Vec<i32> = syllables.iter().map(|s| i32::from(s.implicit)).collect();
        let context_ids = self.context_ids(context);
        let input = SyllableInput {
            context_ids: &context_ids,
            syllable_ids: &bases,
            tones: &tones,
            costs: &costs,
            boundaries: &boundaries,
            candidates: &ids,
        };
        let output = self.model.lock().unwrap_or_else(std::sync::PoisonError::into_inner).predict(&input).ok()?;
        if output.len() != candidates.len()
            || !output.iter().all(|row| row.len() == width && row.iter().all(|v| v.is_finite()))
        {
            return None;
        }
        Some(output.into_iter().zip(candidates).map(|(row, list)| row[..list.len().min(width)].to_vec()).collect())
    }

    /// Runs the model on raw IDs (for checking against the v3 test vectors).
    ///
    /// # Errors
    /// When the model cannot run.
    pub fn raw_scores(&self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        self.model.lock().unwrap_or_else(std::sync::PoisonError::into_inner).predict(input)
    }
}

/// The result of checking a model against `test_vectors.json` (`zhuyin-cli --check-v3`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorCheck {
    /// Vectors whose every scored value is within tolerance.
    pub passed: usize,
    /// Vectors checked.
    pub total: usize,
    /// The largest absolute difference seen.
    pub worst: f64,
}

/// Compares `scorer`'s model with the v3 golden outputs in `directory/test_vectors.json`: every expected value
/// above −9000 (padding) must be within 0.002 + 0.002 × |value|.
///
/// # Errors
/// When the file cannot be read or has an unexpected shape, or the model cannot run.
pub fn check_vectors(scorer: &SyllableScorer, directory: &Path) -> Result<VectorCheck, ModelError> {
    let data = std::fs::read(directory.join("test_vectors.json"))?;
    let file: serde_json::Value = serde_json::from_slice(&data).map_err(|e| ModelError(e.to_string()))?;
    let invalid = || ModelError("test_vectors.json: unexpected shape".to_owned());
    let vectors = file.get("vectors").and_then(serde_json::Value::as_array).ok_or_else(invalid)?;
    let mut check = VectorCheck { passed: 0, total: vectors.len(), worst: 0.0 };
    for vector in vectors {
        let inputs = vector.get("inputs").ok_or_else(invalid)?;
        let row = |key: &str| -> Result<Vec<i32>, ModelError> {
            let values =
                inputs.get(key).and_then(|v| v.get(0)).and_then(serde_json::Value::as_array).ok_or_else(invalid)?;
            values.iter().map(|v| v.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or_else(invalid)).collect()
        };
        let candidates: Vec<Vec<i32>> = inputs
            .get("candidate_ids")
            .and_then(|v| v.get(0))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(invalid)?
            .iter()
            .map(|list| {
                list.as_array()
                    .ok_or_else(invalid)?
                    .iter()
                    .map(|v| v.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or_else(invalid))
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        let (context_ids, syllable_ids, tones, costs, boundaries) =
            (row("context_ids")?, row("syllable_ids")?, row("tone_ids")?, row("cost_bucket")?, row("boundary")?);
        let got = scorer.raw_scores(&SyllableInput {
            context_ids: &context_ids,
            syllable_ids: &syllable_ids,
            tones: &tones,
            costs: &costs,
            boundaries: &boundaries,
            candidates: &candidates,
        })?;
        let expected = vector
            .get("expected")
            .and_then(|e| e.get("logprob"))
            .and_then(|v| v.get(0))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(invalid)?;
        let mut ok = true;
        for (r, list) in expected.iter().enumerate() {
            for (c, value) in list.as_array().ok_or_else(invalid)?.iter().enumerate() {
                let value = value.as_f64().ok_or_else(invalid)?;
                if value <= -9000.0 {
                    continue;
                }
                let got = got.get(r).and_then(|row| row.get(c)).copied().ok_or_else(invalid)?;
                if !got.is_finite() {
                    ok = false;
                    continue;
                }
                let difference = (got - value).abs();
                check.worst = check.worst.max(difference);
                if difference > 0.002 + 0.002 * value.abs() {
                    ok = false;
                }
            }
        }
        if ok {
            check.passed += 1;
        }
    }
    Ok(check)
}

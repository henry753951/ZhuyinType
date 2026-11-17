//! 聯想詞: after a commit, the words likely to come next, in the candidate window ([`Engine::predict_after`]).
//!
//! The list opens only when the setting is on, learning is not suspended (secure input, private windows), the
//! commit ended in a Han character and nothing is left composed.  While it is open:
//!
//! | Key | Effect |
//! |---|---|
//! | ⇧ + a selection key (⇧1…⇧9) | sends that word and opens the list for what follows it |
//! | Esc | closes the list; the app does not get the key |
//! | any other key (Zhuyin keys, Space, Enter, ⌫, arrows, punctuation) | closes the list, then does what it does with nothing composed |
//!
//! The plain selection keys are Zhuyin keys on the standard layout (1 is ㄅ, 3 is ˇ), so they always start a
//! composition; Space is a tone or a space, never a pick.
//!
//! [`Engine::predict_after`]: crate::engine::Engine::predict_after

use std::sync::Arc;

use super::{Composer, trailing_words};
use crate::api::{Candidate, CandidateList, KeyInput, KeyResult, Presentation};
use crate::conversion::ConvertedWord;
use crate::engine::{CommittedWord, is_han};
use crate::language_model::LanguageModel;
use crate::swift_format::graphemes;

impl Composer {
    /// Shows next-word suggestions (聯想詞) after each commit.  Off by default.
    pub fn set_associated_phrases(&mut self, on: bool) {
        self.associated_phrases = on;
        if !on {
            self.close_predictions();
        }
    }

    /// Whether next-word suggestions are on.
    #[must_use]
    pub fn associated_phrases(&self) -> bool {
        self.associated_phrases
    }

    /// While suggestions are shown: ⇧ + a selection key picks one, Esc closes the list; any other key closes it and
    /// is handled as usual (`None`).
    pub(super) fn prediction_key(&mut self, key: KeyInput) -> Option<KeyResult> {
        if self.predictions.is_empty() {
            return None;
        }
        if let KeyInput::ShiftedChar(c) = key
            && let Some(index) = self.options.selection_keys.iter().position(|&k| k == c)
            && index < self.predictions.len()
        {
            return Some(self.commit_prediction(index));
        }
        self.close_predictions();
        (key == KeyInput::Escape).then_some(KeyResult::CONSUMED)
    }

    /// A click on suggestion `index`; `None` when no suggestions are shown.
    pub(super) fn select_prediction(&mut self, index: usize) -> Option<KeyResult> {
        if self.predictions.is_empty() {
            return None;
        }
        if index >= self.predictions.len() {
            return Some(KeyResult::CONSUMED);
        }
        Some(self.commit_prediction(index))
    }

    /// After a key that committed `result.commit` and left nothing composed: suggest what comes next.
    pub(super) fn offer_predictions(&mut self, result: &KeyResult) {
        if !self.associated_phrases
            || self.learning_suspended
            || self.has_composition()
            || !self.presentation.is_empty()
            || !result.commit.as_deref().and_then(|text| graphemes(text).last().copied()).is_some_and(is_han)
        {
            return;
        }
        let count = self.options.selection_keys.len();
        let predictions = self.engine.predict_after(self.leading_history, count, self.options.learning_enabled);
        if predictions.is_empty() {
            return;
        }
        let forms = self.options.character_forms;
        let items: Vec<Candidate> =
            predictions.iter().map(|p| Candidate { text: forms.apply(&p.text), annotation: None }).collect();
        self.presentation = Presentation {
            candidates: Some(CandidateList::new(items, 0, count, None, false).as_associated()),
            ..Presentation::default()
        };
        self.predictions = predictions;
    }

    pub(super) fn close_predictions(&mut self) {
        if self.predictions.is_empty() {
            return;
        }
        self.predictions.clear();
        if !self.has_composition() {
            self.presentation = Presentation::default();
        }
    }

    /// Sends suggestion `index` as if it had been typed and committed, and suggests what follows it.
    fn commit_prediction(&mut self, index: usize) -> KeyResult {
        let prediction = self.predictions.swap_remove(index);
        self.predictions.clear();
        self.presentation = Presentation::default();
        let text = self.options.character_forms.apply(&prediction.text);
        if self.learns() {
            let picked = CommittedWord {
                range: 0..0,
                text: prediction.text.clone(),
                reading: prediction.reading.clone(),
                word_id: Some(prediction.word_id),
                picked: true,
            };
            let context: Vec<CommittedWord> = self
                .leading_words
                .iter()
                .map(|w| CommittedWord {
                    range: 0..0,
                    text: w.text.to_string(),
                    reading: w.reading.to_string(),
                    word_id: w.word_id,
                    picked: false,
                })
                .collect();
            // No syllable count: a suggestion is not a typed reading, so no whole-composition recall is kept.
            self.engine.learn(&[picked], 0, &context);
        }
        let history = self.leading_history.appending(Some(prediction.word_id), self.engine.language_model().order());
        let word = ConvertedWord {
            range: 0..0,
            text: Arc::from(prediction.text.as_str()),
            reading: Arc::from(prediction.reading.as_str()),
            word_id: Some(prediction.word_id),
            history: Some(history),
            corrected: false,
            channel: None,
            reading_corrected: false,
        };
        self.leading_words = trailing_words(std::slice::from_ref(&word), &self.leading_words);
        self.leading_history = history;
        // ⌫ right after this deletes from the app's text, not from a composition this composer remembers.
        self.last_commit = None;
        self.deleted_characters = 0;
        self.deleted_words.clear();
        self.conversion_dirty = true;
        let result = KeyResult::new(true, Some(text));
        self.offer_predictions(&result);
        result
    }
}

//! 聯想詞: after a commit, the words likely to come next, in the candidate window ([`Engine::predict_after`]).
//!
//! The list opens only when the setting is on, learning is not suspended (secure input, private windows), the
//! commit ended in a Han character and nothing is left composed.  It opens passively: nothing is highlighted and
//! every key does what it does with no list, so ⇧1 still types ！ and a Zhuyin key starts a composition.  Only ↓
//! enters it.
//!
//! | State | Key | Effect |
//! |---|---|---|
//! | passive | ↓ | enters the list: highlights the first word |
//! | passive | a click | sends that word |
//! | passive | any other key | closes the list, then does what it does with no list |
//! | entered | ↑ ↓ | move the highlight (↑ on the first word goes back to passive) |
//! | entered | Return, a selection key, ⇧ + a selection key, a click | sends that word |
//! | entered | Esc | closes the list; the app does not get the key |
//! | entered | any other key | closes the list, then does what it does with no list |
//!
//! Sending a word lists, passively again, the words that follow it.  ↓ is the only key a passive list takes from
//! the app: Tab stays the app's (moving between fields after a commit is common), and so do Space, Return and ⇧
//! with symbol keys (！ ？ after a word).
//!
//! [`Engine::predict_after`]: crate::engine::Engine::predict_after

use std::collections::HashSet;
use std::sync::Arc;

use super::{Composer, trailing_words};
use crate::api::{Candidate, CandidateList, KeyInput, KeyResult, Presentation};
use crate::conversion::ConvertedWord;
use crate::engine::{CommittedWord, Prediction, is_han};
use crate::language_model::LanguageModel;
use crate::swift_format::graphemes;

/// The 聯想詞 shown after a commit.  `words` is never empty; `highlight`, when set, is an index into it.
#[derive(Clone, Debug)]
pub(super) struct Suggestions {
    words: Vec<Prediction>,
    /// `None`: passive (shown, not entered); keys go on as if there were no list.
    highlight: Option<usize>,
}

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

    /// While suggestions are shown: the keys of the table above that act on the list (`Some`); any other key closes
    /// it and is handled as usual (`None`).
    pub(super) fn prediction_key(&mut self, key: KeyInput) -> Option<KeyResult> {
        let list = self.predictions.as_ref()?;
        let count = list.words.len();
        let Some(highlight) = list.highlight else {
            if key == KeyInput::Down {
                return Some(self.highlight_prediction(Some(0)));
            }
            self.close_predictions();
            return None;
        };
        let keys = &self.options.selection_keys;
        let picked = match key {
            KeyInput::Enter => Some(highlight),
            KeyInput::Char(c) | KeyInput::ShiftedChar(c) => keys.iter().position(|&k| k == c),
            _ => None,
        };
        if let Some(index) = picked.filter(|&index| index < count) {
            return Some(self.commit_prediction(index));
        }
        match key {
            KeyInput::Down => Some(self.highlight_prediction(Some((highlight + 1).min(count - 1)))),
            KeyInput::Up => Some(self.highlight_prediction(highlight.checked_sub(1))),
            KeyInput::Escape => {
                self.close_predictions();
                Some(KeyResult::CONSUMED)
            }
            _ => {
                self.close_predictions();
                None
            }
        }
    }

    /// A click on suggestion `index`; `None` when no suggestions are shown.
    pub(super) fn select_prediction(&mut self, index: usize) -> Option<KeyResult> {
        let count = self.predictions.as_ref()?.words.len();
        Some(if index < count { self.commit_prediction(index) } else { KeyResult::CONSUMED })
    }

    /// After a key that committed `result.commit` and left nothing composed: suggest what comes next, passively.
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
        let mut words = self.engine.predict_after(self.leading_history, count, self.options.learning_enabled);
        // 台灣 and 臺灣 are one suggestion once written in the user's forms.
        let forms = self.options.character_forms;
        let mut shown: HashSet<String> = HashSet::new();
        words.retain(|p| shown.insert(forms.apply(&p.text)));
        if words.is_empty() {
            return;
        }
        self.predictions = Some(Suggestions { words, highlight: None });
        self.present_predictions();
    }

    /// The presentation of the suggestions: no marked text, the list (passive or with its highlight).
    fn present_predictions(&mut self) {
        let Some(list) = &self.predictions else { return };
        let forms = self.options.character_forms;
        let items: Vec<Candidate> =
            list.words.iter().map(|p| Candidate { text: forms.apply(&p.text), annotation: None }).collect();
        let page = self.options.selection_keys.len().max(items.len());
        let candidates = CandidateList::new(items, list.highlight.unwrap_or(0), page, None, false)
            .as_associated(list.highlight.is_none());
        self.presentation = Presentation { candidates: Some(candidates), ..Presentation::default() };
    }

    fn highlight_prediction(&mut self, highlight: Option<usize>) -> KeyResult {
        if let Some(list) = &mut self.predictions {
            list.highlight = highlight;
        }
        self.present_predictions();
        KeyResult::CONSUMED
    }

    pub(super) fn close_predictions(&mut self) {
        if self.predictions.take().is_some() && !self.has_composition() {
            self.presentation = Presentation::default();
        }
    }

    /// Sends suggestion `index` as if it had been typed and committed, and suggests what follows it.
    fn commit_prediction(&mut self, index: usize) -> KeyResult {
        let Some(mut list) = self.predictions.take() else { return KeyResult::CONSUMED };
        let prediction = list.words.swap_remove(index);
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

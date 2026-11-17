//! Word actions from the candidate window: never suggest a word for a reading (「不要再推薦」), forget a learned
//! choice (「忘記這個選字」) and add a custom word (「加入自訂詞」), each returning a [`WordUndo`] that puts the
//! personal data back exactly as it was (「復原」).
//!
//! A blocked word stays in the lexicon and in the candidate list, but scores [`BLOCKED_COST`] lower wherever it
//! appears under that reading, so conversion never picks it while any other word fits.  With nothing blocked the
//! scores are unchanged.

use super::{Engine, EngineError, PhraseKey, RecallValue, State, valid_phrase};
use crate::personal::{Entry, Key};

/// What blocking subtracts from a word's score (natural log): far below any word the lexicon has.
pub const BLOCKED_COST: f64 = 30.0;

/// Everything one word action changed, to undo it.  Only [`Engine::undo_word_action`] reads it.
#[derive(Clone, Debug)]
pub struct WordUndo {
    key: PhraseKey,
    /// The recall entry removed, if any.
    learned: Option<RecallValue>,
    /// The personal n-grams removed.
    ngrams: Vec<(Key, Entry)>,
    /// Whether the word was blocked before.
    blocked: bool,
    /// Whether the word was a custom word before.
    phrase: bool,
}

impl WordUndo {
    /// The word acted on.
    #[must_use]
    pub fn word(&self) -> &str {
        &self.key.word
    }
    /// Its reading.
    #[must_use]
    pub fn reading(&self) -> &str {
        &self.key.reading
    }
}

impl Engine {
    /// The cost [`scoring`](Self::scoring) subtracts for a blocked word, under its lexicon or its typed reading.
    pub(super) fn blocked_cost(state: &State, word: &str, source: &str, typed: &str) -> f64 {
        let blocked = |reading: &str| state.blocked.contains(&PhraseKey::new(reading, word));
        if blocked(source) || (typed != source && blocked(typed)) { BLOCKED_COST } else { 0.0 }
    }

    /// Whether `word` is never suggested for `reading`.
    #[must_use]
    pub fn is_blocked(&self, word: &str, reading: &str) -> bool {
        self.lock().blocked.contains(&PhraseKey::new(reading, word))
    }

    /// The words never suggested, sorted.
    #[must_use]
    pub fn blocked_entries(&self) -> Vec<PhraseKey> {
        let mut keys: Vec<PhraseKey> = self.lock().blocked.iter().cloned().collect();
        keys.sort();
        keys
    }

    /// Suggests `word` for `reading` again.
    pub fn unblock(&self, word: &str, reading: &str) {
        let mut state = self.lock();
        if state.blocked.remove(&PhraseKey::new(reading, word)) {
            Self::blocked_changed(&mut state);
            self.schedule_write(&state);
        }
    }

    /// Whether `word` is a custom word with `reading`.
    #[must_use]
    pub fn is_user_phrase(&self, word: &str, reading: &str) -> bool {
        self.lock().phrases.contains_key(&PhraseKey::new(reading, word))
    }

    /// Whether the engine remembers the user choosing `word` for `reading`: a recall, or picks of the word.
    #[must_use]
    pub fn has_learned_choice(&self, word: &str, reading: &str) -> bool {
        let state = self.lock();
        state.learned.contains_key(&PhraseKey::new(reading, word))
            || self.known_word_id(&state, word).is_some_and(|id| self.personal.picked(id))
    }

    /// 「不要再推薦」: `word` is never suggested for `reading` while another word fits, what was learned about it is
    /// forgotten and a custom word with that reading is removed.
    pub fn block_suggestion(&self, word: &str, reading: &str) -> WordUndo {
        let mut state = self.lock();
        let undo = self.take_learned(&mut state, word, reading);
        if undo.phrase {
            self.remove_phrase_locked(&mut state, word, reading);
        }
        state.blocked.insert(undo.key.clone());
        Self::blocked_changed(&mut state);
        self.learned_changed(&mut state);
        undo
    }

    /// 「忘記這個選字」: forgets the choice of `word` for `reading`; `None` when nothing is remembered.
    pub fn forget_choice(&self, word: &str, reading: &str) -> Option<WordUndo> {
        if !self.has_learned_choice(word, reading) {
            return None;
        }
        let mut state = self.lock();
        let undo = self.take_learned(&mut state, word, reading);
        self.learned_changed(&mut state);
        Some(undo)
    }

    /// 「加入自訂詞」: adds `word` with `reading` as a custom word, suggesting it again if it was blocked.
    ///
    /// # Errors
    /// [`EngineError::InvalidPhrase`] unless the phrase has 1–6 characters and one valid reading per character, or
    /// when it already is a custom word.
    pub fn add_phrase_undoable(&self, word: &str, reading: &str) -> Result<WordUndo, EngineError> {
        if !valid_phrase(word, reading) || self.is_user_phrase(word, reading) {
            return Err(EngineError::InvalidPhrase);
        }
        let mut state = self.lock();
        let key = PhraseKey::new(reading, word);
        let blocked = state.blocked.remove(&key);
        let undo = WordUndo { key, learned: None, ngrams: vec![], blocked, phrase: false };
        self.add_phrase(&mut state, word, reading);
        state.revision = state.revision.wrapping_add(1);
        Self::blocked_changed(&mut state);
        self.rebuild_reading_index(&mut state);
        self.schedule_write(&state);
        Ok(undo)
    }

    /// Puts the personal data a word action changed back as it was.
    pub fn undo_word_action(&self, undo: WordUndo) {
        let mut state = self.lock();
        let WordUndo { key, learned, ngrams, blocked, phrase } = undo;
        let is_phrase = state.phrases.contains_key(&key);
        if phrase && !is_phrase {
            self.add_phrase(&mut state, &key.word, &key.reading);
        } else if !phrase && is_phrase {
            self.remove_phrase_locked(&mut state, &key.word, &key.reading);
        }
        if blocked {
            state.blocked.insert(key.clone());
        } else {
            state.blocked.remove(&key);
        }
        if let Some(value) = learned {
            state.learned_words.entry(key.reading.clone()).or_default().insert(key.word.clone());
            state.learned.insert(key, value);
        }
        self.personal.restore(ngrams);
        Self::blocked_changed(&mut state);
        self.rebuild_reading_index(&mut state);
        self.learned_changed(&mut state);
    }

    /// Removes and returns what is learned about `word` for `reading`, with the custom-word and blocked state.
    fn take_learned(&self, state: &mut State, word: &str, reading: &str) -> WordUndo {
        let key = PhraseKey::new(reading, word);
        let learned = state.learned.remove(&key);
        if let Some(words) = state.learned_words.get_mut(reading) {
            words.remove(word);
        }
        let id = self.resolve_id(state, word);
        let ngrams = self.personal.take(id);
        WordUndo {
            blocked: state.blocked.contains(&key),
            phrase: state.phrases.contains_key(&key),
            key,
            learned,
            ngrams,
        }
    }

    fn remove_phrase_locked(&self, state: &mut State, word: &str, reading: &str) {
        state.phrases.remove(&PhraseKey::new(reading, word));
        if let Some(words) = state.phrase_index.get_mut(reading) {
            words.remove(word);
        }
        state.revision = state.revision.wrapping_add(1);
        self.rebuild_reading_index(state);
    }

    /// Scores of blocked words are cached in matches and entries.
    fn blocked_changed(state: &mut State) {
        state.entry_cache.clear();
        state.match_cache.clear();
        state.revision = state.revision.wrapping_add(1);
    }
}

//! The composition state machine (port of `Composer.swift`): keys in, text to commit and a [`Presentation`] out.
//!
//! Typing at the end of the composition goes through the key decoder (a run of keys split again on every key);
//! the conversion is the engine's lattice decoding, optionally reranked by a sentence model and adjusted by a
//! character model, both supplied by the client as callbacks.

mod candidates;
mod scoring;

use candidates::CandidateWindow;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::Arc;

use crate::api::{
    Candidate, CandidateAnchor, CandidateList, CharacterModelSyllable, ComposerOptions, KeyDecoderMethod, KeyInput,
    KeyResult, Presentation, ScoredSentence, SpaceAction, TypoTolerance,
};
use crate::conversion::{CharacterScores, CompositionToken, ConversionCache, ConvertedWord, FixedWord, Request};
use crate::engine::{CommittedWord, Engine, Match};
use crate::key_decoder::{Costs, DecodeResult, Key, KeyDecoder, Syllable};
use crate::keyboard;
use crate::language_model::{LanguageModel, LanguageModelHistory};
use crate::lexicon::swift_max;
use crate::swift_format::graphemes;

/// Chooses the sentence to show among the engine's N best: an index into them (others keep the engine's first).
pub type Reranker = Box<dyn FnMut(&[ScoredSentence]) -> Option<usize> + Send>;
/// Scores to add to a span's best candidates, given the composed text before it; `None` leaves the order.
pub type CandidateScorer = Box<dyn FnMut(&str, &[String]) -> Option<Vec<f64>> + Send>;
/// A character model: for each syllable, log P over the candidate characters given; `None` when it cannot answer.
pub type CharacterScorer = Box<dyn FnMut(&[CharacterModelSyllable], &[Vec<String>]) -> Option<Vec<Vec<f64>>> + Send>;
/// Told the text a full composition commits by itself, before the rest is converted again: the models' context.
pub type CommitListener = Box<dyn FnMut(&str) + Send>;

/// Seconds after a commit in which deleting and retyping it counts as a correction.
pub const CORRECTION_WINDOW: f64 = 30.0;
/// How many characters per syllable the character model is asked about.
pub const CHARACTER_CANDIDATES: usize = 64;
/// How much costlier than the cheapest a split may be and still be judged by the language model.
pub const SPLIT_MARGIN: f64 = 6.0;
/// Keys in the decoder's run past which its head is settled.
const RUN_KEYS: usize = 48;
/// Finished syllables a settled run keeps, so the keys after them can still change how they were split.
const RUN_OPEN_SYLLABLES: usize = 4;

/// The run's head to settle, as (syllables, keys): the most syllables that leave `RUN_OPEN_SYLLABLES` in the run and
/// were taken from exactly the run's first keys.  A head whose last syllable took a key from after it (two keys
/// swapped across the cut) or that skipped a key is no place to cut: the cut would fix that repair for good.
fn settled_head(run: &[Key], syllables: &[Syllable]) -> Option<(usize, usize)> {
    let mut taken: Vec<char> = Vec::new();
    let mut best = None;
    for (count, syllable) in syllables.iter().take(syllables.len().saturating_sub(RUN_OPEN_SYLLABLES)).enumerate() {
        // An implicit syllable records the Space no one typed.
        let keys =
            if syllable.implicit { syllable.keys.strip_suffix(' ').unwrap_or(&syllable.keys) } else { &syllable.keys };
        taken.extend(keys.chars());
        let Some(typed) = run.get(..taken.len()) else { break };
        let mut typed: Vec<char> = typed.iter().map(|k| k.character).collect();
        let mut sorted = taken.clone();
        typed.sort_unstable();
        sorted.sort_unstable();
        if typed == sorted {
            best = Some((count + 1, taken.len()));
        }
    }
    best
}

/// The character model's cost bucket for a correction of this cost: the keyboard channel's and the reading
/// channel's corrections share the bounds (`resources/formats/reading-channel.md`, `v3_noise.tsv`).
#[must_use]
pub fn cost_bucket(cost: f64) -> i32 {
    if cost <= 6.0 {
        1
    } else if cost <= 11.0 {
        2
    } else {
        3
    }
}

/// How the character model scores a character that only a reading-channel variant put at a position.  The model
/// was trained on standard readings, so it reads 熟 typed ㄕㄡˊ as unlikely; until it is retrained on the channel's
/// reading noise one of the first three stands in (`resources/formats/reading-channel.md`, rule 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VariantScores {
    /// (a) The score the character gets under its lexicon reading: one more model call with that syllable.
    Standard,
    /// (a) without the extra call: the position's best score.
    Best,
    /// (b) No character-model score (weight 0): the default; (a) scored the same on every set and its extra call
    /// costs the Mac about 10% per-key p95 (`docs/research/log.md`, 2026-10-28).
    #[default]
    Zero,
    /// As typed, for a model trained on the channel's noise: accepted readings with cost bucket 0, corrections
    /// bucketed by their cost.
    Typed,
}

impl VariantScores {
    /// Every option.
    pub const ALL: [VariantScores; 4] =
        [VariantScores::Standard, VariantScores::Best, VariantScores::Zero, VariantScores::Typed];

    /// The command-line name.
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            VariantScores::Standard => "standard",
            VariantScores::Best => "best",
            VariantScores::Zero => "zero",
            VariantScores::Typed => "typed",
        }
    }

    /// The option named `raw`.
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.raw_value() == raw)
    }
}

/// The default [`Tuning::variant_window`].
pub const VARIANT_WINDOW: f64 = 3.0;

/// A character only a reading-channel variant put at a position: its lexicon syllable, and the variant's state and
/// cost, from the best-scoring such match.
#[derive(Clone, Debug)]
struct VariantCharacter {
    prior: f64,
    syllable: String,
    correction: bool,
    cost: f64,
}

/// The last character of a literal that ends a clause or a sentence: an automatic commit prefers to end after it.
/// ASCII `.`, `,` and `:` are left out, as they also stand inside numbers, times and addresses.
const CLAUSE_ENDS: &str = "。！？，、；：…．!?;";
/// The last character of a literal that ends a sentence (the early commit).
const SENTENCE_ENDS: &str = "。！？…．!?";
/// Closing brackets and quotes that stay with the punctuation before them.
const CLOSERS: &str = "」』）〉》】〕”’\"')]";

/// Settings that affect conversion and candidate ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Character model weight.
    pub character_weight: f64,
    /// Include character scores in candidate ordering.
    pub character_scores_in_list: bool,
    /// Number of candidates scored.
    pub candidate_scoring_count: usize,
    /// Keyboard decoder costs.
    pub decoder_costs: Costs,
    /// Maximum split cost difference.
    pub split_margin: f64,
    /// How variant characters are scored.
    pub variant_scores: VariantScores,
    /// Variant scoring window.
    pub variant_window: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            character_weight: 1.0,
            character_scores_in_list: true,
            candidate_scoring_count: 24,
            decoder_costs: Costs::default(),
            split_margin: SPLIT_MARGIN,
            variant_scores: VariantScores::default(),
            variant_window: VARIANT_WINDOW,
        }
    }
}

struct LastCommit {
    words: Vec<ConvertedWord>,
    leading: LanguageModelHistory,
    leading_words: Vec<ConvertedWord>,
    time: f64,
}

/// One composition: tokens, the unfinished syllable, picks, the candidate window and the conversion.
// The flags are the Swift composer's independent state and settings, one for one.
#[allow(clippy::struct_excessive_bools)]
pub struct Composer {
    options: ComposerOptions,
    tuning: Tuning,
    engine: Arc<Engine>,
    tokens: Vec<CompositionToken>,
    cursor: usize,
    buffer: [Option<char>; 3],
    buffer_keys: String,
    fixed: Vec<FixedWord>,
    conversion: Vec<ConvertedWord>,
    conversion_cache: Option<ConversionCache>,
    conversion_dirty: bool,
    conversion_revision: u64,
    candidate_window: CandidateWindow,
    selection_anchor: Option<usize>,
    notice: Option<String>,
    key_time: Option<f64>,
    run: Vec<Key>,
    run_tokens: usize,
    /// The composition before and right after the last backtick: pressing it again or Esc, with nothing changed in
    /// between, puts the syllables back.
    literal_undo: Option<(Snapshot, Snapshot)>,
    leading_history: LanguageModelHistory,
    leading_words: Vec<ConvertedWord>,
    last_commit: Option<LastCommit>,
    deleted_characters: usize,
    deleted_words: Vec<(ConvertedWord, LanguageModelHistory)>,
    reranker: Option<Reranker>,
    reranking_count: usize,
    /// Scores added to the best candidates of a span when the window opens.
    candidate_scorer: Option<CandidateScorer>,
    character_scorer: Option<CharacterScorer>,
    /// After a pick from the candidate list, keep every other shown word as it is.
    freezes_on_pick: bool,
    /// Learning paused for now (secure input); the setting itself is unchanged.
    learning_suspended: bool,
    character_scores: Option<CharacterScores>,
    presentation: Presentation,
    /// Text committed by the key being handled before what it composes (a full composition's head).
    sent: String,
    commit_listener: Option<CommitListener>,
}

/// What a composition is made of, to put it back after the backtick key (Zhuyin as typed).
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    tokens: Vec<CompositionToken>,
    cursor: usize,
    buffer: [Option<char>; 3],
    buffer_keys: String,
    fixed: Vec<FixedWord>,
    run: Vec<Key>,
    run_tokens: usize,
}

/// The history each word was scored after (a converted word carries the history that follows it).
fn histories_before(words: &[ConvertedWord], start: LanguageModelHistory) -> Vec<LanguageModelHistory> {
    (0..words.len()).map(|i| if i == 0 { start } else { words[i - 1].history.unwrap_or_default() }).collect()
}

/// The last two syllable words of `words` after its last literal, continuing `earlier` when there is no literal.
fn trailing_words(words: &[ConvertedWord], earlier: &[ConvertedWord]) -> Vec<ConvertedWord> {
    let tail = words.iter().rev().take_while(|w| !w.reading.is_empty()).count();
    let tail = &words[words.len() - tail..];
    let all: Vec<ConvertedWord> =
        if tail.len() == words.len() { earlier.iter().chain(tail).cloned().collect() } else { tail.to_vec() };
    all[all.len().saturating_sub(2)..].to_vec()
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    !a.is_empty() && !b.is_empty() && a.start < b.end && b.start < a.end
}

/// `count` graphemes of `text` after skipping `skip`.
fn grapheme_slice(text: &str, skip: usize, count: usize) -> String {
    graphemes(text).into_iter().skip(skip).take(count).collect()
}

fn is_zhuyin_key(c: char) -> bool {
    keyboard::symbol(c).is_some() || keyboard::tone(c).is_some()
}

/// A literal whose last character is one of `marks`.
fn ends_with(token: &CompositionToken, marks: &str) -> bool {
    matches!(token, CompositionToken::Literal(text) if text.chars().last().is_some_and(|c| marks.contains(c)))
}

/// A literal of closing brackets and quotes only.
fn is_closer(token: &CompositionToken) -> bool {
    matches!(token, CompositionToken::Literal(text) if !text.is_empty() && text.chars().all(|c| CLOSERS.contains(c)))
}

impl Composer {
    /// A composer on `engine`.
    #[must_use]
    pub fn new(engine: Arc<Engine>, options: ComposerOptions) -> Self {
        Self {
            options,
            engine,
            tokens: vec![],
            cursor: 0,
            buffer: [None; 3],
            buffer_keys: String::new(),
            fixed: vec![],
            conversion: vec![],
            conversion_cache: None,
            conversion_dirty: true,
            conversion_revision: 0,
            candidate_window: CandidateWindow::default(),
            selection_anchor: None,
            notice: None,
            key_time: None,
            run: vec![],
            run_tokens: 0,
            literal_undo: None,
            leading_history: LanguageModelHistory::default(),
            leading_words: vec![],
            last_commit: None,
            deleted_characters: 0,
            deleted_words: vec![],
            reranker: None,
            reranking_count: 8,
            tuning: Tuning::default(),
            candidate_scorer: None,
            character_scorer: None,
            freezes_on_pick: true,
            learning_suspended: false,
            character_scores: None,
            presentation: Presentation::default(),
            sent: String::new(),
            commit_listener: None,
        }
    }

    /// Every match through the reading channel at every syllable of the composition (`zhuyin-cli --channel-report`).
    #[must_use]
    pub fn channel_matches(&self) -> Vec<Match> {
        (0..self.tokens.len())
            .filter(|&start| self.tokens[start].reading().is_some())
            .flat_map(|start| self.engine.matches(&self.tokens, start, self.options.typo_tolerance))
            .filter(|m| m.channel.is_some())
            .collect()
    }

    /// The words of the last commit, with how each was matched.
    #[must_use]
    pub fn last_committed(&self) -> &[ConvertedWord] {
        self.last_commit.as_ref().map_or(&[], |c| c.words.as_slice())
    }

    /// Whether a pick came through the reading channel (its annotation is not a keyboard slip to learn).
    fn picked_through_channel(&self, choice: &FixedWord) -> bool {
        self.engine.reading_channel().is_some()
            && self
                .engine
                .matches(&self.tokens, choice.range.start, self.options.typo_tolerance)
                .iter()
                .any(|m| m.range == choice.range && m.entry.word == choice.text && m.channel.is_some())
    }

    /// The options.
    #[must_use]
    pub fn options(&self) -> &ComposerOptions {
        &self.options
    }

    /// Changes the options: a change closes the candidate window and redraws.
    pub fn set_options(&mut self, options: ComposerOptions) {
        let old = std::mem::replace(&mut self.options, options);
        if self.options.typo_tolerance != old.typo_tolerance {
            self.conversion_dirty = true;
        }
        if self.options != old {
            self.close_candidates();
            self.refresh();
        }
    }

    /// Returns the conversion settings.
    #[must_use]
    pub fn tuning(&self) -> Tuning {
        self.tuning
    }

    /// Replaces conversion settings and invalidates derived state when they change.
    pub fn set_tuning(&mut self, tuning: Tuning) {
        if self.tuning != tuning {
            self.tuning = tuning;
            self.conversion_dirty = true;
            self.close_candidates();
            self.refresh();
        }
    }

    /// Sets whether a pick freezes other words.
    pub fn set_freezes_on_pick(&mut self, value: bool) {
        self.freezes_on_pick = value;
    }

    /// Suspends or resumes learning.
    pub fn set_learning_suspended(&mut self, value: bool) {
        self.learning_suspended = value;
    }

    /// Returns whether learning is suspended.
    #[must_use]
    pub fn learning_suspended(&self) -> bool {
        self.learning_suspended
    }

    /// Sets the candidate scorer.
    pub fn set_candidate_scorer(&mut self, scorer: Option<CandidateScorer>) {
        self.candidate_scorer = scorer;
    }

    /// Sets the sentence reranker.
    pub fn set_reranker(&mut self, reranker: Option<Reranker>) {
        self.reranker = reranker;
        self.conversion_dirty = true;
        self.close_candidates();
        self.refresh();
    }

    /// How many sentences the reranker chooses among.
    #[must_use]
    pub fn reranking_count(&self) -> usize {
        self.reranking_count
    }

    /// Sets how many sentences the reranker chooses among.
    pub fn set_reranking_count(&mut self, count: usize) {
        self.reranking_count = count;
        self.conversion_dirty = true;
        self.close_candidates();
        self.refresh();
    }

    /// Sets the character model.
    pub fn set_character_scorer(&mut self, scorer: Option<CharacterScorer>) {
        self.character_scorer = scorer;
        self.conversion_dirty = true;
        self.refresh();
    }

    /// Sets the listener told what a full composition commits by itself, before the rest is converted again (the
    /// host's models read the text before the composition).
    pub fn set_commit_listener(&mut self, listener: Option<CommitListener>) {
        self.commit_listener = listener;
    }

    /// What to draw.
    #[must_use]
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    /// Something is being composed (or a window is open).
    #[must_use]
    pub fn is_composing(&self) -> bool {
        !self.presentation.is_empty()
    }

    /// The engine.
    #[must_use]
    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }

    fn buffer_text(&self) -> String {
        self.buffer.iter().flatten().collect()
    }

    fn has_buffer(&self) -> bool {
        self.buffer.iter().any(Option::is_some)
    }

    fn has_composition(&self) -> bool {
        !self.tokens.is_empty() || self.has_buffer()
    }

    fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        (anchor != self.cursor).then(|| anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    fn learns(&self) -> bool {
        self.options.learning_enabled && !self.learning_suspended
    }

    fn request<'a>(&'a self, tokens: &'a [CompositionToken]) -> Request<'a> {
        let mut request = Request::new(tokens, &self.fixed, self.options.typo_tolerance);
        request.start = self.leading_history;
        request
    }

    /// Beam-bounded alternatives for completed tokens, excluding the unfinished syllable.
    #[must_use]
    pub fn n_best(&self, k: usize) -> Vec<ScoredSentence> {
        let mut request = self.request(&self.tokens);
        request.characters = self.character_scores.as_ref();
        self.engine.sentences(&request, k)
    }

    /// Handles a key that went down at `time` (seconds; the decoder reads how close keys were).
    pub fn handle_at(&mut self, key: KeyInput, time: f64) -> KeyResult {
        self.key_time = Some(time);
        let result = self.handle(key);
        self.key_time = None;
        result
    }

    // An open candidate window has a valid selection and target; fixed ranges stay within the composition.
    fn invariants_hold(&self) -> bool {
        self.cursor <= self.tokens.len()
            && self.fixed.iter().all(|word| word.range.start <= word.range.end && word.range.end <= self.tokens.len())
            && (self.candidate_window.is_closed() || (self.candidate_window.valid_for(self.tokens.len())))
    }

    /// Handles a key.
    pub fn handle(&mut self, key: KeyInput) -> KeyResult {
        let mut result = self.process(key);
        // A full composition sent its head first: that text goes before anything else the key commits.
        let sent = std::mem::take(&mut self.sent);
        if !sent.is_empty() {
            result.commit = Some(sent + result.commit.as_deref().unwrap_or(""));
        }
        // A key the app receives may move the insertion point or edit the text before it.
        if !result.handled && (key != KeyInput::Backspace || !self.follow_deletion()) {
            self.forget_context();
        }
        debug_assert!(self.invariants_hold());
        result
    }

    fn forget_context(&mut self) {
        self.leading_history = LanguageModelHistory::default();
        self.leading_words.clear();
        self.last_commit = None;
        self.deleted_words.clear();
    }

    /// Backspace passed to the app with nothing composed deletes the last commit from its end: the context becomes
    /// what is left of it.  False when it is not following a recent commit.
    fn follow_deletion(&mut self) -> bool {
        if self.has_composition() {
            return false;
        }
        let now = self.engine.now();
        let Some(last) = &self.last_commit else { return false };
        if (now - last.time).partial_cmp(&CORRECTION_WINDOW) != Some(std::cmp::Ordering::Less) {
            return false;
        }
        self.deleted_characters += 1;
        let mut remaining = self.deleted_characters;
        let mut kept = last.words.len();
        while kept > 0 && remaining >= graphemes(&last.words[kept - 1].text).len() {
            remaining -= graphemes(&last.words[kept - 1].text).len();
            kept -= 1;
        }
        if !(kept > 0 || remaining == 0) {
            return false;
        }
        let before = histories_before(&last.words, last.leading);
        let deleted: Vec<(ConvertedWord, LanguageModelHistory)> = (kept..last.words.len())
            .filter(|&i| !last.words[i].reading.is_empty())
            .map(|i| (last.words[i].clone(), before[i]))
            .collect();
        let (history, words) = if remaining > 0 {
            (LanguageModelHistory::default(), vec![])
        } else if kept == 0 {
            (last.leading, last.leading_words.clone())
        } else {
            let before = &last.words[..kept];
            (before.last().and_then(|w| w.history).unwrap_or_default(), trailing_words(before, &last.leading_words))
        };
        self.deleted_words = deleted;
        self.leading_history = history;
        self.leading_words = words;
        true
    }

    #[allow(clippy::too_many_lines)]
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tokens: self.tokens.clone(),
            cursor: self.cursor,
            buffer: self.buffer,
            buffer_keys: self.buffer_keys.clone(),
            fixed: self.fixed.clone(),
            run: self.run.clone(),
            run_tokens: self.run_tokens,
        }
    }

    fn restore(&mut self, state: Snapshot) {
        self.tokens = state.tokens;
        self.cursor = state.cursor;
        self.buffer = state.buffer;
        self.buffer_keys = state.buffer_keys;
        self.fixed = state.fixed;
        self.run = state.run;
        self.run_tokens = state.run_tokens;
        self.conversion_dirty = true;
    }

    fn process(&mut self, key: KeyInput) -> KeyResult {
        self.notice = None;
        if let Some((before, after)) = self.literal_undo.take()
            && (key == KeyInput::Char('`') || key == KeyInput::Escape)
            && self.candidate_window.is_closed()
            && self.snapshot() == after
        {
            self.restore(before);
            self.refresh();
            return KeyResult::CONSUMED;
        }
        let before_key = self.snapshot();
        if !self.continues_run(key) {
            self.settle_run();
        }
        if let Some(result) = self.selection_key(key) {
            return result;
        }
        if self.candidate_window.is_open()
            && let Some(result) = self.handle_candidate_key(key)
        {
            return result;
        }
        let result: KeyResult;
        if key == KeyInput::Char('`') {
            result = self.key_literal(before_key);
        } else if let Some(literal) = keyboard::literal(key, self.options.punctuation_keys) {
            result = self.key_punctuation(literal);
        } else {
            match key {
                KeyInput::Char(c) if self.decodes_run() && is_zhuyin_key(c) => result = self.type_into_run(c),
                KeyInput::Space if self.decodes_run() && self.has_buffer() => result = self.type_into_run(' '),
                KeyInput::Backspace if self.decodes_run() && self.has_buffer() && !self.run.is_empty() => {
                    result = self.erase_from_run();
                }
                KeyInput::Char(c) => result = self.key_character(c),
                KeyInput::Space => {
                    let (space_result, committed) = self.key_space();
                    if committed {
                        return space_result;
                    }
                    result = space_result;
                }
                KeyInput::ShiftSpace => result = self.key_shift_space(),
                KeyInput::Enter | KeyInput::ShiftEnter => return self.key_commit(key == KeyInput::Enter),
                KeyInput::Backspace => result = self.key_backspace(),
                KeyInput::OptionBackspace => result = self.key_option_backspace(),
                KeyInput::ForwardDelete => result = self.key_forward_delete(),
                KeyInput::Left | KeyInput::Right => result = self.key_horizontal(key),
                KeyInput::Home | KeyInput::End => result = self.key_boundary(key),
                KeyInput::Down | KeyInput::ShiftDown | KeyInput::ShiftUp => result = self.key_open_window(key),
                KeyInput::Up | KeyInput::PageUp | KeyInput::PageDown | KeyInput::Tab | KeyInput::ShiftTab => {
                    result = self.key_if_composing();
                }
                KeyInput::Escape => result = self.key_escape(),
                KeyInput::ControlChar(_) => return self.key_commit(false),
                KeyInput::ShiftedChar(_) | KeyInput::Printed(_) | KeyInput::ShiftLeft | KeyInput::ShiftRight => {
                    result = KeyResult::PASS_THROUGH;
                }
            }
        }
        self.refresh();
        result
    }

    fn key_punctuation(&mut self, literal: String) -> KeyResult {
        self.finish_buffer("", ' ', true, true);
        self.insert(CompositionToken::Literal(literal.into()));
        KeyResult::CONSUMED
    }

    fn key_space(&mut self) -> (KeyResult, bool) {
        if self.has_buffer() {
            self.finish_buffer("", ' ', false, true);
            (KeyResult::CONSUMED, false)
        } else if self.has_composition() {
            match self.options.space_action {
                SpaceAction::OpenCandidates => {
                    self.open_candidates(false, false);
                    (KeyResult::CONSUMED, false)
                }
                SpaceAction::Commit => (KeyResult::new(true, Some(self.commit_all())), true),
                SpaceAction::CommitAndSpace => (KeyResult::new(false, Some(self.commit_all())), true),
                SpaceAction::InsertSpace => {
                    self.insert_space();
                    (KeyResult::CONSUMED, false)
                }
            }
        } else {
            (KeyResult::PASS_THROUGH, false)
        }
    }

    fn key_commit(&mut self, handled: bool) -> KeyResult {
        if self.has_composition() { KeyResult::new(handled, Some(self.commit_all())) } else { KeyResult::PASS_THROUGH }
    }

    fn key_if_composing(&self) -> KeyResult {
        if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH }
    }

    fn key_literal(&mut self, before_key: Snapshot) -> KeyResult {
        let result: KeyResult;
        if self.has_buffer() {
            let text = self.buffer_text();
            self.clear_buffer();
            self.insert(CompositionToken::Literal(text.into()));
            self.literal_undo = Some((before_key, self.snapshot()));
            result = KeyResult::CONSUMED;
        } else if self.cursor > 0
            && let Some(reading) = self.tokens[self.cursor - 1].reading().map(str::to_owned)
        {
            let target = self.cursor - 1;
            self.delete(target..self.cursor);
            self.insert(CompositionToken::Literal(keyboard::plain(&reading).into()));
            self.literal_undo = Some((before_key, self.snapshot()));
            result = KeyResult::CONSUMED;
        } else {
            result = if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH };
        }
        result
    }

    fn selection_key(&mut self, key: KeyInput) -> Option<KeyResult> {
        if key == KeyInput::ShiftLeft || key == KeyInput::ShiftRight {
            if !self.has_composition() {
                return Some(KeyResult::PASS_THROUGH);
            }
            self.finish_buffer("", ' ', false, true);
            self.close_candidates();
            if !self.has_buffer() {
                if self.selection_anchor.is_none() {
                    self.selection_anchor = Some(self.cursor);
                }
                self.cursor = if key == KeyInput::ShiftLeft {
                    self.cursor.saturating_sub(1)
                } else {
                    (self.cursor + 1).min(self.tokens.len())
                };
            }
            self.refresh();
            return Some(KeyResult::CONSUMED);
        }
        if let Some(range) = self.selection()
            && key == KeyInput::Enter
        {
            let parts: Vec<String> =
                self.tokens[range.clone()].iter().filter_map(|t| t.reading().map(str::to_owned)).collect();
            let text = self.shown(&range);
            let reading = parts.join(" ");
            let added = (1..=6).contains(&range.len())
                && parts.len() == range.len()
                && self.engine.add_user_phrase(&text, &reading).is_ok();
            if added {
                let choice = self.choice(&range, &text, &reading, None);
                self.fix(choice);
                self.selection_anchor = None;
                self.notice = Some(format!("已加入自訂詞：{text}"));
            } else {
                self.notice = Some("自訂詞需為 1 到 6 個字".to_owned());
            }
            self.refresh();
            return Some(KeyResult::CONSUMED);
        }
        if self.selection_anchor.is_some() {
            self.selection_anchor = None;
            if key == KeyInput::Escape {
                self.refresh();
                return Some(KeyResult::CONSUMED);
            }
        }
        None
    }

    fn key_character(&mut self, c: char) -> KeyResult {
        let result: KeyResult;
        if let Some(symbol) = keyboard::symbol(c)
            && let Some(slot) = keyboard::slot(symbol)
        {
            if !self.has_buffer() {
                // A new syllable at the cursor.
                self.make_room(self.syllable_count() + 1);
            }
            self.buffer[slot] = Some(symbol);
            self.buffer_keys.push(c);
            result = KeyResult::CONSUMED;
        } else if let Some(tone) = keyboard::tone(c) {
            self.finish_buffer(tone, c, false, true);
            result = KeyResult::CONSUMED;
        } else {
            result = KeyResult::PASS_THROUGH;
        }

        result
    }

    fn key_shift_space(&mut self) -> KeyResult {
        let result: KeyResult = if self.has_composition() {
            self.insert_space();
            KeyResult::CONSUMED
        } else {
            KeyResult::PASS_THROUGH
        };

        result
    }

    fn key_backspace(&mut self) -> KeyResult {
        let result: KeyResult = if self.has_buffer() {
            if let Some(last) = self.buffer.iter().rposition(Option::is_some) {
                self.buffer[last] = None;
                self.buffer_keys.retain(|k| keyboard::symbol(k).and_then(keyboard::slot) != Some(last));
            }
            KeyResult::CONSUMED
        } else if self.cursor > 0 {
            self.delete(self.cursor - 1..self.cursor);
            KeyResult::CONSUMED
        } else if self.has_composition() {
            KeyResult::CONSUMED
        } else {
            KeyResult::PASS_THROUGH
        };

        result
    }

    fn key_option_backspace(&mut self) -> KeyResult {
        let result: KeyResult = if self.has_buffer() {
            self.clear_buffer();
            KeyResult::CONSUMED
        } else if self.cursor > 0 {
            let target = self.cursor - 1;
            let range = self
                .conversion
                .iter()
                .find(|w| w.range.contains(&target))
                .map_or(target..self.cursor, |w| w.range.clone());
            self.delete(range);
            KeyResult::CONSUMED
        } else if self.has_composition() {
            KeyResult::CONSUMED
        } else {
            KeyResult::PASS_THROUGH
        };

        result
    }

    fn key_forward_delete(&mut self) -> KeyResult {
        let result: KeyResult = if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH };
        if !self.has_buffer() && self.cursor < self.tokens.len() {
            self.delete(self.cursor..self.cursor + 1);
        }

        result
    }

    fn key_horizontal(&mut self, key: KeyInput) -> KeyResult {
        let result: KeyResult = if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH };
        if self.has_composition() && !self.has_buffer() {
            self.cursor = if key == KeyInput::Left {
                self.cursor.saturating_sub(1)
            } else {
                (self.cursor + 1).min(self.tokens.len())
            };
        }

        result
    }

    fn key_boundary(&mut self, key: KeyInput) -> KeyResult {
        let result: KeyResult = if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH };
        if self.has_composition() {
            self.cursor = if key == KeyInput::Home { 0 } else { self.tokens.len() };
        }

        result
    }

    fn key_open_window(&mut self, key: KeyInput) -> KeyResult {
        let result: KeyResult = if self.has_composition() { KeyResult::CONSUMED } else { KeyResult::PASS_THROUGH };
        if self.has_composition() {
            self.finish_buffer("", ' ', false, true);
            self.refresh();
            let tolerant = key == KeyInput::ShiftUp && self.corrections_available();
            self.open_candidates(key == KeyInput::ShiftDown, tolerant);
        }

        result
    }

    fn key_escape(&mut self) -> KeyResult {
        let result: KeyResult = if self.has_buffer() {
            self.clear_buffer();
            KeyResult::CONSUMED
        } else if self.has_composition() {
            if self.options.escape_clears_composition {
                self.reset();
            }
            KeyResult::CONSUMED
        } else {
            KeyResult::PASS_THROUGH
        };

        result
    }

    /// Picks candidate `index` of the open list.
    pub fn select_candidate(&mut self, index: usize) -> KeyResult {
        let Some(choice) = self.candidate_window.get(index).cloned() else {
            debug_assert!(self.invariants_hold());
            return KeyResult::CONSUMED;
        };
        if choice.reading.is_empty() {
            self.fixed.retain(|f| !overlaps(&f.range, &choice.range));
            self.tokens[choice.range.start] = CompositionToken::Literal(choice.text.clone());
        } else {
            if self.learns()
                && let Some(annotation) = &choice.annotation
                && !self.picked_through_channel(&choice)
            {
                // A picked correction: the keys typed against the reading meant teach this user's slips.
                let meant: Vec<String> = annotation
                    .split(' ')
                    .filter(|p| !p.is_empty())
                    .map(|part| match part.strip_prefix('˙') {
                        Some(rest) => format!("{rest}˙"),
                        None => part.to_owned(),
                    })
                    .collect();
                let typed: Vec<String> = self.tokens[choice.range.clone()]
                    .iter()
                    .filter_map(|t| match t {
                        CompositionToken::Syllable { keys, .. } => Some(keys.to_string()),
                        CompositionToken::Literal(_) => None,
                    })
                    .collect();
                if typed.len() == meant.len() {
                    let pairs: Vec<(String, String)> = typed.into_iter().zip(meant).collect();
                    self.engine.record_typos(&pairs);
                }
            }
            if self.learns() {
                // What was shown there loses some of the user's evidence in its context.
                let before = histories_before(&self.conversion, self.leading_history);
                for (index, word) in self.conversion.iter().enumerate() {
                    if overlaps(&word.range, &choice.range) && word.text != choice.text && !word.reading.is_empty() {
                        self.engine.weaken(&word.text, &word.reading, word.word_id, Some(before[index]));
                    }
                }
            }
            self.fix(choice.clone());
            if self.freezes_on_pick {
                // The rest of what is shown stays as it is.  A pick inside a longer word keeps that word's characters
                // before it; those after it may still change with the pick.
                let conversion = self.conversion.clone();
                for word in conversion.iter().filter(|w| !w.reading.is_empty()) {
                    let piece = if overlaps(&word.range, &choice.range) {
                        word.range.start..word.range.start.max(choice.range.start)
                    } else {
                        word.range.clone()
                    };
                    if piece.is_empty() || self.fixed.iter().any(|f| overlaps(&f.range, &piece)) {
                        continue;
                    }
                    if piece == word.range {
                        self.fixed.push(FixedWord {
                            range: word.range.clone(),
                            text: word.text.clone(),
                            reading: word.reading.clone(),
                            word_id: word.word_id,
                            annotation: None,
                            components: vec![],
                            frozen: true,
                        });
                        continue;
                    }
                    let text = grapheme_slice(&word.text, piece.start - word.range.start, piece.len());
                    let reading = self.readings(&piece).join(" ");
                    let word_id = self.engine.language_model().word_id(&text);
                    self.fixed.push(FixedWord {
                        range: piece,
                        text: text.into(),
                        reading: reading.into(),
                        word_id,
                        annotation: None,
                        components: vec![],
                        frozen: true,
                    });
                }
            }
        }
        self.conversion_dirty = true;
        self.close_candidates();
        self.refresh();
        debug_assert!(self.invariants_hold());
        KeyResult::CONSUMED
    }

    fn readings(&self, range: &Range<usize>) -> Vec<String> {
        self.tokens[range.clone()].iter().filter_map(|t| t.reading().map(str::to_owned)).collect()
    }

    fn fix(&mut self, choice: FixedWord) {
        self.fixed.retain(|f| !overlaps(&f.range, &choice.range));
        self.fixed.push(choice);
        self.conversion_dirty = true;
    }

    /// Commits everything: learns from it and returns the text to insert.
    pub fn commit_all(&mut self) -> String {
        // An unfinished syllable is sent as the symbols on screen: committing never guesses a tone or a correction.
        if self.has_buffer() {
            let text = self.buffer_text();
            self.insert(CompositionToken::Literal(text.into()));
            self.clear_buffer();
        }
        self.refresh();
        let text = self.presentation.text.clone();
        self.record_commit(self.tokens.len(), &text);
        self.reset();
        text
    }

    /// Learns from the words before token `end` as committed (`text`, what was shown), and makes them the text
    /// before what comes next.
    fn record_commit(&mut self, end: usize, text: &str) {
        let words: Vec<ConvertedWord> = self.conversion.iter().take_while(|w| w.range.end <= end).cloned().collect();
        if self.learns() && !text.is_empty() {
            let mut committed: Vec<CommittedWord> = words
                .iter()
                .map(|word| CommittedWord {
                    range: word.range.clone(),
                    text: word.text.to_string(),
                    reading: word.reading.to_string(),
                    word_id: word.word_id,
                    picked: self.fixed.iter().any(|f| f.range == word.range && f.text == word.text && !f.frozen),
                })
                .collect();
            let corrected = self.corrected_prefix();
            if corrected > 0 {
                // Deleted right after it was committed and typed again as other words: learn it like a pick, and
                // weaken what was deleted where it stood.
                for word in committed.iter_mut().filter(|w| w.range.end <= corrected) {
                    word.picked = true;
                }
                for (word, before) in &self.deleted_words {
                    if !self.conversion.iter().any(|c| c.text == word.text && c.reading == word.reading) {
                        self.engine.weaken(&word.text, &word.reading, word.word_id, Some(*before));
                    }
                }
            }
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
            let syllables = self.tokens[..end].iter().filter(|t| t.reading().is_some()).count();
            self.engine.learn(&committed, syllables, &context);
        }
        if !text.is_empty() {
            // A literal at the end (punctuation, symbols as typed) leaves the sentence-start history.
            let leading_words = trailing_words(&words, &self.leading_words);
            let leading_history = words.last().and_then(|w| w.history).unwrap_or_default();
            self.last_commit = Some(LastCommit {
                words,
                leading: self.leading_history,
                leading_words: std::mem::replace(&mut self.leading_words, leading_words),
                time: self.engine.now(),
            });
            self.leading_history = leading_history;
            self.deleted_characters = 0;
            self.deleted_words.clear();
        }
    }

    /// The number of leading tokens that retype the deleted words' readings as different words; 0 when they do not.
    fn corrected_prefix(&self) -> usize {
        if self.deleted_words.is_empty() {
            return 0;
        }
        let readings: Vec<&str> =
            self.deleted_words.iter().flat_map(|(w, _)| w.reading.split(' ').filter(|p| !p.is_empty())).collect();
        if self.tokens.len() < readings.len()
            || !self.tokens.iter().zip(&readings).all(|(t, r)| t.reading() == Some(*r))
            || !self.conversion.iter().any(|w| w.range.end == readings.len())
        {
            return 0;
        }
        let retyped: String =
            self.conversion.iter().filter(|w| w.range.end <= readings.len()).map(|w| &*w.text).collect();
        let deleted: String = self.deleted_words.iter().map(|(w, _)| &*w.text).collect();
        if retyped == deleted { 0 } else { readings.len() }
    }

    /// The text before the insertion point, read from the app: the next composition continues its last words.
    /// Ignored while composing.
    pub fn set_preceding_text(&mut self, text: &str) {
        if self.has_composition() {
            return;
        }
        self.engine.adapt_domain(text);
        let words = self.engine.trailing_words(text);
        self.leading_history = words.last().and_then(|w| w.history).unwrap_or_default();
        self.leading_words = words;
        self.conversion_dirty = true;
    }

    /// The next composition starts a sentence: call when the text before the insertion point is unknown.
    pub fn break_context(&mut self) {
        self.forget_context();
        if !self.tokens.is_empty() {
            self.conversion_dirty = true;
            self.refresh();
        }
    }

    /// Clears the composition without committing or learning.
    pub fn reset(&mut self) {
        self.settle_run();
        self.tokens.clear();
        self.cursor = 0;
        self.clear_buffer();
        self.fixed.clear();
        self.conversion.clear();
        self.conversion_cache = None;
        self.selection_anchor = None;
        self.notice = None;
        self.conversion_dirty = true;
        self.close_candidates();
        self.presentation = Presentation::default();
    }

    fn clear_buffer(&mut self) {
        self.buffer = [None; 3];
        self.buffer_keys.clear();
    }

    /// Typing at the end of the composition, with a decoder that splits key runs.
    fn decodes_run(&self) -> bool {
        self.options.key_decoder != KeyDecoderMethod::Slots
            && self.cursor == self.tokens.len()
            && self.selection_anchor.is_none()
            && self.candidate_window.is_closed()
    }

    /// Keys that extend the run: Zhuyin and tone keys, Space finishing a syllable, Backspace inside the unfinished
    /// one.
    fn continues_run(&self, key: KeyInput) -> bool {
        match key {
            KeyInput::Char(c) => is_zhuyin_key(c),
            KeyInput::Space | KeyInput::Backspace => self.has_buffer(),
            _ => false,
        }
    }

    /// The run's tokens stay as they are; typing on starts a new run from the unfinished syllable.
    fn settle_run(&mut self) {
        self.run.clear();
        self.run_tokens = 0;
    }

    fn type_into_run(&mut self, character: char) -> KeyResult {
        if self.run.is_empty() {
            self.run = self.buffer_keys.chars().map(|c| Key::new(c, None)).collect();
            self.run_tokens = 0;
        }
        self.run.push(Key::new(character, self.key_time));
        if !self.apply_run(true) {
            self.run.pop();
        }
        KeyResult::CONSUMED
    }

    /// Asks the character model about every syllable: the characters any lattice word puts there, the most frequent
    /// `CHARACTER_CANDIDATES` of them.  A reading-channel variant's character ranks without the variant's cost, so
    /// the model sees it ([`VariantScores`] says how it is scored).
    // One function, as in Swift, with the reading channel's additions in place.
    /// The run's split: the decoder's cheapest, unless another split of the same shape (as many syllables, the same
    /// unfinished part) close in cost reads better to the language model.
    fn best_split(&self) -> Option<DecodeResult> {
        let tolerance = self.options.typo_tolerance;
        let method = if tolerance == TypoTolerance::Off
            && matches!(self.options.key_decoder, KeyDecoderMethod::Reorder | KeyDecoderMethod::Timed)
        {
            KeyDecoderMethod::Boundaries
        } else {
            self.options.key_decoder
        };
        let engine = &self.engine;
        let decoder = KeyDecoder::new(method, self.tuning.decoder_costs, |reading: &str, keys: &str| {
            if engine.is_valid_syllable(reading) {
                return Some(0.0);
            }
            if tolerance == TypoTolerance::Off {
                return None;
            }
            engine.alternatives(keys).first().map(|a| a.cost * tolerance.cost_multiplier())
        });
        let splits = decoder.decode(&self.run, 6);
        let cheapest = splits.first()?;
        if !cheapest.cost.is_finite() {
            return None;
        }
        let rivals: Vec<&DecodeResult> = splits
            .iter()
            .filter(|s| {
                s.cost.is_finite()
                    && s.cost <= cheapest.cost + self.tuning.split_margin
                    && s.syllables.len() == cheapest.syllables.len()
                    && s.buffer_keys.is_empty() == cheapest.buffer_keys.is_empty()
            })
            .collect();
        if rivals.len() <= 1 || self.options.key_decoder == KeyDecoderMethod::Slots {
            return Some(cheapest.clone());
        }
        let settled = &self.tokens[..self.tokens.len() - self.run_tokens];
        let mut best = rivals[0];
        let mut best_score = self.split_score(best, settled);
        for &rival in &rivals[1..] {
            let score = self.split_score(rival, settled);
            if best_score < score {
                best = rival;
                best_score = score;
            }
        }
        Some(best.clone())
    }

    fn split_score(&self, split: &DecodeResult, settled: &[CompositionToken]) -> f64 {
        let mut candidate = settled.to_vec();
        candidate.extend(split.syllables.iter().map(|s| CompositionToken::syllable(&s.reading, &s.keys, s.implicit)));
        let request = self.request(&candidate);
        self.engine.sentences(&request, 1).first().map_or(f64::NEG_INFINITY, |s| s.score - split.cost)
    }

    fn erase_from_run(&mut self) -> KeyResult {
        self.run.pop();
        if !self.apply_run(false) {
            // The shorter run has no split: the run stays as it is, and Backspace takes the unfinished syllable's
            // last symbol as it does without the decoder.
            self.settle_run();
            if let Some(last) = self.buffer.iter().rposition(Option::is_some) {
                self.buffer[last] = None;
                self.buffer_keys.retain(|k| keyboard::symbol(k).and_then(keyboard::slot) != Some(last));
            }
        }
        KeyResult::CONSUMED
    }

    /// Splits the run again and replaces the tokens it produced.  False when no split exists, and nothing changes.
    /// `growing`: a key was added, and a composition that would grow past the limit first commits its head.
    fn apply_run(&mut self, growing: bool) -> bool {
        let Some(decoded) = self.best_split() else { return false };
        if !decoded.cost.is_finite() {
            return false;
        }
        let settled =
            self.tokens[..self.tokens.len() - self.run_tokens].iter().filter(|t| t.reading().is_some()).count();
        let unfinished = usize::from(!decoded.buffer_keys.is_empty());
        if growing && self.make_room(settled + decoded.syllables.len() + unfinished) {
            // The head is sent: split the run again after it.
            return self.apply_run(false);
        }
        if self.run_tokens > 0 {
            let count = self.tokens.len();
            self.delete(count - self.run_tokens..count);
        }
        for syllable in &decoded.syllables {
            self.insert(CompositionToken::syllable(&syllable.reading, &syllable.keys, syllable.implicit));
        }
        self.run_tokens = decoded.syllables.len();
        self.buffer = decoded.buffer;
        self.buffer_keys = decoded.buffer_keys;
        // A long run settles its head, which keeps the search short.  The last syllables stay in the run, so the
        // next keys can still change how they were split.
        if self.run.len() > RUN_KEYS {
            if let Some((syllables, keys)) = settled_head(&self.run, &decoded.syllables) {
                self.run.drain(..keys);
                self.run_tokens -= syllables;
            } else {
                self.run = self.buffer_keys.chars().map(|c| Key::new(c, None)).collect();
                self.run_tokens = 0;
            }
        }
        true
    }

    fn syllable_count(&self) -> usize {
        self.tokens.iter().filter(|t| t.reading().is_some()).count()
    }

    /// Before a key adds syllables at the cursor: a composition that would then have `total` syllables, more than the
    /// limit, first commits its head, so no key is ever refused.  With the early commit on and `total` past it, the
    /// finished sentences before the cursor are committed.  True when something was committed.
    fn make_room(&mut self, total: usize) -> bool {
        let max = self.options.max_syllables;
        let early = self.options.early_commit_syllables;
        if total <= max && (early == 0 || total <= early) {
            return false;
        }
        if self.conversion_dirty {
            self.refresh();
        }
        let mut total = total;
        let mut sent = false;
        loop {
            let edit = self.cursor;
            let cut = if total > max {
                self.commit_point(edit, total)
            } else if !sent && early > 0 && total > early {
                (0..edit)
                    .rev()
                    .find(|&i| ends_with(&self.tokens[i], SENTENCE_ENDS))
                    .map(|mark| self.past_closers(mark + 1).min(edit))
            } else {
                None
            };
            let Some(cut) = cut else { return sent };
            if cut > self.tokens.len() - self.run_tokens {
                // The head reaches into the run: its syllables stay as they are, and the run goes on from the
                // unfinished syllable and the key just typed.
                let key = self.run.last().copied();
                self.run = self.buffer_keys.chars().map(|c| Key::new(c, None)).chain(key).collect();
                self.run_tokens = 0;
            }
            total = total.saturating_sub(self.tokens[..cut].iter().filter(|t| t.reading().is_some()).count());
            self.commit_head(cut);
            sent = true;
        }
    }

    /// Where the head of a full composition ends, between two words of the conversion shown and at most at `limit`
    /// (the cursor): after the last clause or sentence punctuation when at most two thirds of the limit follow it
    /// (of `total`, the syllables with the key), otherwise about the first third of the limit.  `None` when no word
    /// ends before `limit`.
    fn commit_point(&self, limit: usize, total: usize) -> Option<usize> {
        let max = self.options.max_syllables;
        let head = (max / 3).max(1);
        let keep = max.saturating_sub(head);
        let mut before = Vec::with_capacity(self.tokens.len() + 1);
        before.push(0usize);
        for token in &self.tokens {
            before.push(before[before.len() - 1] + usize::from(token.reading().is_some()));
        }
        if let Some(mark) = (0..limit).rev().find(|&i| ends_with(&self.tokens[i], CLAUSE_ENDS)) {
            let cut = self.past_closers(mark + 1).min(limit);
            if total.saturating_sub(before[cut]) <= keep {
                return Some(cut);
            }
        }
        // Word ends with at least one syllable before them: the last with at most `head`, or else the first.
        let mut best = None;
        for word in &self.conversion {
            let end = word.range.end;
            if end > limit {
                break;
            }
            if before[end] == 0 || self.fixed.iter().any(|f| f.range.start < end && end < f.range.end) {
                continue;
            }
            if best.is_some() && before[end] > head {
                break;
            }
            best = Some(end);
        }
        // Punctuation right after the head goes with it.
        best.map(|mut cut| {
            while cut < limit && (ends_with(&self.tokens[cut], CLAUSE_ENDS) || is_closer(&self.tokens[cut])) {
                cut += 1;
            }
            cut
        })
    }

    /// `index`, moved past the closing brackets and quotes that start there.
    fn past_closers(&self, index: usize) -> usize {
        index + self.tokens[index..].iter().take_while(|t| is_closer(t)).count()
    }

    /// Commits the first `cut` tokens as shown, as Enter would commit them, and keeps composing the rest.
    fn commit_head(&mut self, cut: usize) {
        let text = self.shown(&(0..cut));
        self.record_commit(cut, &text);
        self.delete(0..cut);
        // The rest keeps its words until it is converted again after the head (with the key).
        self.conversion = self
            .conversion
            .iter()
            .filter(|w| w.range.start >= cut)
            .map(|w| ConvertedWord { range: w.range.start - cut..w.range.end - cut, ..w.clone() })
            .collect();
        self.literal_undo = None;
        if let Some(listener) = self.commit_listener.as_mut() {
            listener(&text);
        }
        self.sent.push_str(&text);
    }

    fn finish_buffer(&mut self, tone: &str, tone_key: char, dropping_invalid: bool, allowing_typo: bool) -> bool {
        if !self.has_buffer() {
            return false;
        }
        let reading = self.buffer_text() + tone;
        let keys = format!("{}{tone_key}", self.buffer_keys);
        if self.engine.is_valid_syllable(&reading)
            || (allowing_typo
                && self.options.typo_tolerance != TypoTolerance::Off
                && !self.engine.alternatives(&keys).is_empty())
        {
            self.insert(CompositionToken::syllable(&reading, &keys, false));
            self.clear_buffer();
            return true;
        }
        if dropping_invalid {
            self.clear_buffer();
        }
        false
    }

    /// A literal space in the composition: a sentence boundary that stays until the composition is sent.
    fn insert_space(&mut self) {
        self.finish_buffer("", ' ', true, true);
        self.insert(CompositionToken::Literal(" ".into()));
    }

    fn insert(&mut self, token: CompositionToken) {
        self.conversion_dirty = true;
        let cursor = self.cursor;
        self.fixed.retain(|w| !(w.range.start < cursor && cursor < w.range.end));
        for word in &mut self.fixed {
            if word.range.start >= cursor {
                word.range = word.range.start + 1..word.range.end + 1;
            }
        }
        self.tokens.insert(cursor, token);
        self.cursor += 1;
    }

    fn delete(&mut self, range: Range<usize>) {
        self.conversion_dirty = true;
        self.fixed.retain(|w| !overlaps(&w.range, &range));
        for word in &mut self.fixed {
            if word.range.start >= range.end {
                word.range = word.range.start - range.len()..word.range.end - range.len();
            }
        }
        self.tokens.drain(range.clone());
        if self.cursor >= range.end {
            self.cursor -= range.len();
        } else if self.cursor > range.start {
            self.cursor = range.start;
        }
    }

    fn close_candidates(&mut self) {
        self.candidate_window.close();
    }

    fn handle_candidate_key(&mut self, key: KeyInput) -> Option<KeyResult> {
        let page_size = self.options.selection_keys.len().max(1);
        let page = self.candidate_window.selection() / page_size;
        if let KeyInput::Char(c) = key
            && c != '`'
            && let Some(number) = self.options.selection_keys.iter().position(|&k| k == c)
        {
            return Some(self.select_candidate(page * page_size + number));
        }
        let vertical = self.options.vertical_list;
        match key {
            KeyInput::Char('`') if self.candidate_literal() => return Some(KeyResult::CONSUMED),
            KeyInput::Enter => return Some(self.select_candidate(self.candidate_window.selection())),
            KeyInput::ShiftDown => self.candidate_toggle_single(),
            KeyInput::ShiftUp if self.candidate_toggle_corrections() => return Some(KeyResult::CONSUMED),
            KeyInput::Char('`') | KeyInput::ShiftUp => {}
            KeyInput::Escape | KeyInput::Backspace => self.close_candidates(),
            KeyInput::Left if vertical => self.move_list_target(false),
            KeyInput::Right if vertical => self.move_list_target(true),
            KeyInput::PageUp | KeyInput::ShiftTab => self.candidate_window.move_page(page_size, false, false),
            KeyInput::Up if !vertical => self.candidate_window.move_page(page_size, false, false),
            KeyInput::PageDown | KeyInput::Tab => self.candidate_window.move_page(page_size, true, false),
            KeyInput::Down if !vertical => self.candidate_window.move_page(page_size, true, false),
            KeyInput::Up | KeyInput::Left => {
                self.candidate_window.move_selection(page_size, false, self.options.page_at_list_edge);
            }
            KeyInput::Down | KeyInput::Right => {
                self.candidate_window.move_selection(page_size, true, self.options.page_at_list_edge);
            }
            KeyInput::Space => self.candidate_window.move_page(page_size, true, true),
            _ => {
                self.close_candidates();
                return None;
            }
        }
        self.refresh();
        Some(KeyResult::CONSUMED)
    }

    fn candidate_toggle_single(&mut self) {
        self.open_candidates(!self.candidate_window.single_character(), self.candidate_window.tolerant());
    }

    // Returns true when the unavailable correction action must leave the display untouched.
    fn candidate_toggle_corrections(&mut self) -> bool {
        if self.candidate_window.literal() || !self.corrections_available() {
            return true;
        }
        self.open_candidates(self.candidate_window.single_character(), !self.candidate_window.tolerant());
        false
    }

    fn candidate_literal(&mut self) -> bool {
        let range = self.candidate_window.selected().expect("candidate window is open").range.clone();
        if !self.tokens[range.clone()].iter().all(|t| t.reading().is_some()) {
            self.close_candidates();
            self.refresh();
            return true;
        }
        let text = keyboard::plain(&self.readings(&range).join(" "));
        let before = self.snapshot();
        self.delete(range.clone());
        self.cursor = range.start;
        self.insert(CompositionToken::Literal(text.into()));
        self.close_candidates();
        self.literal_undo = Some((before, self.snapshot()));
        false
    }

    /// The open list moves to the previous or next character, in the same mode.
    fn move_list_target(&mut self, forward: bool) {
        let Some(target) = self.candidate_window.target(self.tokens.len(), forward) else { return };
        self.cursor = if self.options.candidate_anchor == CandidateAnchor::AfterCursor { target } else { target + 1 };
        self.open_candidates(self.candidate_window.single_character(), self.candidate_window.tolerant());
    }

    /// The converted text over `range`, in the lexicon's own forms.
    fn converted(&self, range: &Range<usize>) -> String {
        self.conversion
            .iter()
            .filter(|w| overlaps(&w.range, range))
            .map(|word| {
                if range.start <= word.range.start && range.end >= word.range.end {
                    return word.text.to_string();
                }
                let skip = range.start.saturating_sub(word.range.start);
                let count = word.range.end.min(range.end) - word.range.start.max(range.start);
                grapheme_slice(&word.text, skip, count)
            })
            .collect()
    }

    /// The text over `range` as shown, in the preferred character forms.
    fn shown(&self, range: &Range<usize>) -> String {
        self.options.character_forms.apply(&self.converted(range))
    }

    fn choice(&self, range: &Range<usize>, text: &str, reading: &str, words: Option<&[ConvertedWord]>) -> FixedWord {
        let words = words.unwrap_or(&self.conversion);
        let source: Vec<ConvertedWord> = words
            .iter()
            .flat_map(|word| {
                let existing = self.fixed.iter().find(|f| f.range == word.range && f.text == word.text);
                match existing.and_then(|e| e.components.first().map(|first| (e, first))) {
                    Some((existing, first)) => {
                        // Components are in order, so none starts before the first.
                        let (from, to) = (first.range.start, word.range.start);
                        existing
                            .components
                            .iter()
                            .map(|part| {
                                let mut part = part.clone();
                                part.range = part.range.start - from + to..part.range.end - from + to;
                                part
                            })
                            .collect::<Vec<_>>()
                    }
                    None => vec![word.clone()],
                }
            })
            .collect();
        let parts: Vec<ConvertedWord> = source
            .into_iter()
            .filter(|w| overlaps(&w.range, range))
            .map(|word| {
                let overlap = word.range.start.max(range.start)..word.range.end.min(range.end);
                if overlap == word.range {
                    return word;
                }
                let value = grapheme_slice(&word.text, overlap.start - word.range.start, overlap.len());
                ConvertedWord {
                    range: overlap.clone(),
                    reading: self.readings(&overlap).join(" ").into(),
                    word_id: self.engine.language_model().word_id(&value),
                    text: value.into(),
                    history: None,
                    corrected: false,
                    channel: None,
                    reading_corrected: false,
                }
            })
            .collect();
        FixedWord {
            range: range.clone(),
            text: text.into(),
            reading: reading.into(),
            word_id: parts.last().and_then(|p| p.word_id),
            annotation: None,
            components: parts,
            frozen: false,
        }
    }

    /// Switches the open candidate window between words for the keys as typed and typing corrections.
    pub fn toggle_corrections(&mut self) {
        if self.candidate_window.is_closed() || self.candidate_window.literal() || !self.corrections_available() {
            return;
        }
        self.open_candidates(self.candidate_window.single_character(), !self.candidate_window.tolerant());
        self.refresh();
    }

    fn corrections_available(&self) -> bool {
        self.options.typo_tolerance != TypoTolerance::Off || !self.engine.fuzzy_sounds().is_empty()
    }

    /// The list for the span at the cursor: words for the keys as typed, longest spans first, each span ranked in
    /// context.  `single_character`: only the character at the cursor.  `tolerant`: corrections and fuzzy matches
    /// first, then the rest.
    #[allow(clippy::too_many_lines)]
    fn open_candidates(&mut self, single_character: bool, tolerant: bool) {
        if self.tokens.is_empty() || self.has_buffer() {
            return;
        }
        let cursor = self.cursor;
        let count = self.tokens.len();
        let forward = cursor == 0 || (self.options.candidate_anchor == CandidateAnchor::AfterCursor && cursor < count);
        let target = if forward { cursor } else { cursor - 1 };
        if let CompositionToken::Literal(text) = &self.tokens[target] {
            let choices = keyboard::alternatives(text)
                .into_iter()
                .map(|text| FixedWord {
                    range: target..target + 1,
                    text: text.into(),
                    reading: "".into(),
                    word_id: None,
                    annotation: None,
                    components: vec![],
                    frozen: false,
                })
                .collect();
            self.candidate_window.open(choices, target, single_character, tolerant, true);
            return;
        }
        let mut ranges: Vec<Range<usize>> = vec![];
        let longest_length = if single_character { 1 } else { 6.min(if forward { count - cursor } else { cursor }) };
        for length in 1..=longest_length {
            let range = if forward { cursor..cursor + length } else { cursor - length..cursor };
            if !self.tokens[range.clone()].iter().all(|t| t.reading().is_some()) {
                break;
            }
            ranges.push(range);
        }
        let mut head: Option<FixedWord> = None;
        let mut exact: Vec<FixedWord> = vec![];
        let mut corrected: Vec<FixedWord> = vec![];
        let longest = ranges.last().cloned();
        let order = self.engine.language_model().order();
        for range in ranges.iter().rev() {
            let reading = self.readings(range).join(" ");
            let mut history = self.leading_history;
            for word in self.conversion.iter().filter(|w| w.range.end <= range.start) {
                history = if word.reading.is_empty() {
                    LanguageModelHistory::default()
                } else {
                    word.history.unwrap_or_else(|| history.appending(word.word_id, order))
                };
            }
            let all_matches: Vec<Match> = self
                .engine
                .matches(&self.tokens, range.start, self.options.typo_tolerance)
                .into_iter()
                .filter(|m| m.range == *range)
                .collect();
            let mut ranked: Vec<(&Match, f64)> = all_matches
                .iter()
                .filter(|m| tolerant || m.annotation.is_none())
                .map(|m| (m, self.engine.score(&m.entry, &reading, history)))
                .collect();
            let compare = |a: &(&Match, f64), b: &(&Match, f64)| {
                if a.1 == b.1 {
                    a.0.entry.word.cmp(&b.0.entry.word)
                } else if a.1 > b.1 {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            };
            if self.tuning.character_scores_in_list
                && let Some(scores) = &self.character_scores
            {
                for item in &mut ranked {
                    item.1 += scores.score(&item.0.entry.word, range.start);
                }
            }
            ranked.sort_by(compare);
            if ranked.len() > 1 && self.candidate_scorer.is_some() {
                let head_count = ranked.len().min(self.tuning.candidate_scoring_count);
                let before: String =
                    self.conversion.iter().filter(|w| w.range.end <= range.start).map(|w| &*w.text).collect();
                let words: Vec<String> = ranked[..head_count].iter().map(|r| r.0.entry.word.to_string()).collect();
                if let Some(scorer) = self.candidate_scorer.as_mut()
                    && let Some(extra) = scorer(&before, &words)
                    && scoring::valid_scores(&extra, head_count)
                {
                    for (item, extra) in ranked[..head_count].iter_mut().zip(extra) {
                        item.1 += extra;
                    }
                    ranked[..head_count].sort_by(compare);
                }
            }
            // What is shown there: first for the whole span; for a shorter span only when it is one word or a pick.
            let current = self.converted(range);
            if let Some(existing) = self.fixed.iter().find(|f| f.range == *range && *f.text == *current) {
                exact.push(existing.clone());
            } else if Some(range) == longest.as_ref() || self.conversion.iter().any(|w| w.range == *range) {
                let mut word = self.choice(range, &current, &reading, None);
                word.annotation = if all_matches.iter().any(|m| *m.entry.word == *current && m.annotation.is_none()) {
                    None
                } else {
                    all_matches.iter().find(|m| *m.entry.word == *current).and_then(|m| m.annotation.clone())
                };
                if Some(range) == longest.as_ref() {
                    head = Some(word);
                } else {
                    exact.push(word);
                }
            }
            for (m, _) in &ranked {
                let word = FixedWord {
                    range: range.clone(),
                    text: m.entry.word.clone(),
                    reading: reading.as_str().into(),
                    word_id: m.entry.word_id,
                    annotation: m.annotation.clone(),
                    components: vec![],
                    frozen: false,
                };
                if m.annotation.is_none() {
                    exact.push(word);
                } else {
                    corrected.push(word);
                }
            }
        }
        // Two forms of one word (臺北, 台北) are shown once, in the preferred form.
        let forms = self.options.character_forms;
        let mut choices: Vec<FixedWord> = vec![];
        let ordered = head.into_iter().chain(if tolerant {
            corrected.into_iter().chain(exact).collect::<Vec<_>>()
        } else {
            exact
        });
        for word in ordered {
            if choices.len() < 60
                && !choices.iter().any(|c| c.range == word.range && forms.apply(&c.text) == forms.apply(&word.text))
            {
                choices.push(word);
            }
        }
        self.candidate_window.open(choices, target, single_character, tolerant, false);
    }

    fn refresh(&mut self) {
        let revision = self.engine.conversion_revision();
        if self.conversion_dirty || self.conversion_revision != revision {
            self.character_scores = self.score_characters();
            let mut cache = self.conversion_cache.take();
            let mut request = self.request(&self.tokens);
            request.characters = self.character_scores.as_ref();
            self.conversion = self.engine.convert(&request, &mut cache);
            self.conversion_cache = cache;
            if self.reranker.is_some() && !self.tokens.is_empty() {
                self.rerank();
            }
            self.conversion_dirty = false;
            self.conversion_revision = revision;
        }
        if let Some(range) = self.selection()
            && self.notice.is_none()
        {
            self.notice = Some(format!("Enter 加入自訂詞：{}", self.shown(&range)));
        }
        self.presentation = self.present();
    }

    /// Words the user picked in this place before stay: the reranker chooses among the sentences that keep them.
    fn rerank(&mut self) {
        let before = histories_before(&self.conversion, self.leading_history);
        let remembered: Vec<ConvertedWord> = (0..self.conversion.len())
            .filter(|&i| {
                !self.conversion[i].reading.is_empty() && self.engine.remembers(&self.conversion[i], before[i])
            })
            .map(|i| self.conversion[i].clone())
            .collect();
        let sentences: Vec<ScoredSentence> = self
            .n_best(self.reranking_count.max(1))
            .into_iter()
            .filter(|sentence| {
                remembered
                    .iter()
                    .all(|word| sentence.words.iter().any(|w| w.range == word.range && *w.text == *word.text))
            })
            .collect();
        if sentences.is_empty() {
            return;
        }
        let Some(reranker) = self.reranker.as_mut() else { return };
        let Some(index) = reranker(&sentences).filter(|&i| i < sentences.len()) else { return };
        let model = self.engine.language_model();
        let order = model.order();
        let mut history = self.leading_history;
        self.conversion = sentences[index]
            .words
            .iter()
            .map(|word| {
                let constraint = self.fixed.iter().find(|f| f.range == word.range && *f.text == *word.text);
                let word_id = match constraint {
                    Some(c) => c.word_id.or_else(|| model.word_id(&word.text)),
                    None => model.word_id(&word.text),
                };
                if word.reading.is_empty() {
                    history = LanguageModelHistory::default();
                } else if let Some(parts) = constraint.map(|c| &c.components).filter(|p| !p.is_empty()) {
                    for part in parts {
                        history = history.appending(part.word_id, order);
                    }
                } else {
                    history = history.appending(word_id, order);
                }
                ConvertedWord {
                    range: word.range.clone(),
                    text: word.text.as_str().into(),
                    reading: word.reading.as_str().into(),
                    word_id,
                    history: Some(history),
                    corrected: false,
                    channel: None,
                    reading_corrected: false,
                }
            })
            .collect();
    }

    #[allow(clippy::too_many_lines)]
    fn present(&self) -> Presentation {
        let focus_range = self.selection().or_else(|| {
            (self.candidate_window.is_open())
                .then(|| self.candidate_window.selected().expect("candidate window is open").range.clone())
        });
        let forms = self.options.character_forms;
        let mut text = String::new();
        let mut length = 0usize; // UTF-16 units of `text`
        let mut offsets = vec![0usize; self.tokens.len() + 1];
        let mut boundaries: BTreeSet<usize> = BTreeSet::new();
        let mut caret = 0;
        let mut inserted_buffer = false;
        let buffer_text = self.buffer_text();
        let has_buffer = self.has_buffer();
        let mut append_buffer = |text: &mut String, length: &mut usize, boundaries: &mut BTreeSet<usize>| {
            if inserted_buffer {
                return;
            }
            if has_buffer {
                boundaries.insert(*length);
                text.push_str(&buffer_text);
                *length += buffer_text.encode_utf16().count();
                boundaries.insert(*length);
            }
            caret = *length;
            inserted_buffer = true;
        };
        for word in &self.conversion {
            boundaries.insert(length);
            let shown = forms.apply(&word.text);
            let clusters = graphemes(&shown);
            for token in word.range.clone() {
                if self.cursor == token {
                    append_buffer(&mut text, &mut length, &mut boundaries);
                }
                offsets[token] = length;
                let piece: &str = if word.range.len() == 1 {
                    &shown
                } else {
                    clusters.get(token - word.range.start).copied().unwrap_or("")
                };
                text.push_str(piece);
                length += piece.encode_utf16().count();
            }
            offsets[word.range.end] = length;
            boundaries.insert(length);
            if self.cursor == word.range.end {
                append_buffer(&mut text, &mut length, &mut boundaries);
            }
        }
        append_buffer(&mut text, &mut length, &mut boundaries);
        let mut focus: Option<Range<usize>> = None;
        if let Some(range) = &focus_range {
            let focused = offsets[range.start]..offsets[range.end];
            boundaries.retain(|b| !focused.contains(b));
            boundaries.insert(focused.start);
            boundaries.insert(focused.end);
            focus = Some(focused);
        }
        // With the cursor moved inside the composition, the character ↓ would change.
        let mut target_range: Option<Range<usize>> = None;
        if focus_range.is_none() && !has_buffer && self.cursor < self.tokens.len() {
            let forward = self.cursor == 0 || self.options.candidate_anchor == CandidateAnchor::AfterCursor;
            let target = if forward { self.cursor } else { self.cursor - 1 };
            let range = offsets[target]..offsets[target + 1];
            if !range.is_empty() {
                boundaries.insert(range.start);
                boundaries.insert(range.end);
                target_range = Some(range);
            }
        }
        let sorted: Vec<usize> = boundaries.into_iter().collect();
        let clauses: Vec<Range<usize>> = sorted.windows(2).filter(|w| w[0] < w[1]).map(|w| w[0]..w[1]).collect();
        let candidates = (self.candidate_window.is_open()).then(|| {
            CandidateList::new(
                self.candidate_window
                    .choices()
                    .iter()
                    .map(|c| Candidate {
                        text: forms.apply(&c.text),
                        annotation: c.annotation.as_deref().map(str::to_owned),
                    })
                    .collect(),
                self.candidate_window.selection(),
                self.options.selection_keys.len().max(1),
                if self.candidate_window.literal() || !self.corrections_available() {
                    None
                } else {
                    Some(self.candidate_window.tolerant())
                },
                self.candidate_window.single_character(),
            )
        });
        let zhuyin = (self.candidate_window.is_open())
            .then(|| {
                let range = &self.candidate_window.selected().expect("candidate window is open").range;
                let parts: Vec<Option<&str>> =
                    self.tokens[range.clone()].iter().map(CompositionToken::reading).collect();
                parts
                    .iter()
                    .all(Option::is_some)
                    .then(|| keyboard::plain(&parts.into_iter().flatten().collect::<Vec<_>>().join(" ")))
            })
            .flatten();
        Presentation {
            text,
            cursor: caret,
            focused_clause: focus.and_then(|f| clauses.iter().position(|c| *c == f)),
            target_clause: target_range.and_then(|t| clauses.iter().position(|c| *c == t)),
            clauses,
            candidates,
            notice: self.notice.clone(),
            zhuyin_hint: zhuyin,
        }
    }
}

/// A syllable as the character model reads it.
fn model_syllable(reading: &str, cost_bucket: i32, implicit: bool) -> CharacterModelSyllable {
    let tone = match reading.chars().last() {
        Some('ˊ') => 2,
        Some('ˇ') => 3,
        Some('ˋ') => 4,
        Some('˙') => 5,
        _ => 1,
    };
    let base: String = reading.chars().filter(|&c| !keyboard::is_tone_mark(c)).collect();
    CharacterModelSyllable { base: Some(base), tone, cost_bucket, implicit }
}

//! The engine (port of `Engine.swift`): lexicon lookup with tone tolerance, matching with typing corrections and
//! fuzzy sounds, scoring, user phrases, remembered words, typing habits, the language-model stack, the text before
//! the cursor, learning, and the personal data files.  Lattice decoding is in [`crate::conversion`].
//!
//! One engine is shared by every composer: its tables sit behind one lock, and personal data is written on the
//! engine's own thread.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::api::{FuzzySounds, TypoTolerance};
use crate::binary::{DataError, MappedLexicon};
use crate::conversion::{CompositionToken, ConvertedWord};
use crate::key_channel::{self, ReadingAlternative};
use crate::keyboard;
use crate::language_model::{
    LanguageModel, LanguageModelHistory, MixtureLanguageModel, NO_CONTEXT, UnigramLanguageModel, VocabularyModel,
};
use crate::lexicon::{TextLexicon, WordEntry, swift_max, swift_min};
use crate::personal::{DecayingCount, PersonalLanguageModel, Word};
use crate::swift_format::{format_double, grapheme_count, graphemes};
use crate::syllables::{SyllableSet, without_tones};
use crate::tsv::{parse_double, parse_int, scan_tsv};

/// Settings the Swift engine keeps in static variables, read when an engine is created.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineConfig {
    /// Score a composition as a whole sentence (`<s>` … `</s>`).
    pub sentence_boundaries: bool,
    /// The chat-domain bigram mixed with the encyclopedic one (`lm_chat/`); `None` keeps the encyclopedic alone.
    pub chat_model_directory: Option<PathBuf>,
    /// How often the end of a composition is the end of a sentence.
    pub sentence_end_share: f64,
    /// The chat share with no text before the cursor.
    pub chat_prior: f64,
    /// How fast the text before the cursor moves the chat share.
    pub domain_evidence_scale: f64,
    /// The per-word log-ratio at which the text is neutral.
    pub domain_evidence_centre: f64,
    /// Hit-area substitution costs in the keyboard channel (an evaluation setting).
    pub hotspot: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sentence_boundaries: true,
            chat_model_directory: None,
            sentence_end_share: 1.0,
            chat_prior: 0.3,
            domain_evidence_scale: 1.0,
            domain_evidence_centre: -2.5,
            hotspot: false,
        }
    }
}

/// Default beam width; N-best decoding widens it when K is larger.
pub const DECODING_BEAM_WIDTH: usize = 12;
/// The most a personal slip is discounted: 1.5 × ln(1 + times seen), up to this.
pub const TYPO_DISCOUNT_CAP: f64 = 4.0;
/// Cost of one fuzzy-sound substitution.
pub const FUZZY_COST: f64 = 3.0;
const FIRST_PERSONAL_ID: u32 = 0x4000_0000;

/// A reading and a word: the key of user phrases and remembered words.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhraseKey {
    /// The reading, syllables separated by spaces.
    pub reading: String,
    /// The word.
    pub word: String,
}

impl PhraseKey {
    /// A key.
    #[must_use]
    pub fn new(reading: impl Into<String>, word: impl Into<String>) -> Self {
        Self { reading: reading.into(), word: word.into() }
    }
}

/// What a recall entry remembers: exact typed readings mapped to what the user committed for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecallKind {
    /// A whole composition with a correction in it; offered only for an identical whole composition.
    Whole,
    /// A correction together with the syllables around it (3–6 syllables).
    Span,
    /// A word picked under a reading it does not have in the lexicon (a typo the user corrected).
    Typo,
    /// A multi-word phrase the user picked as one unit.
    Phrase,
}

impl RecallKind {
    /// The name in `learned.tsv`.
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            RecallKind::Whole => "whole",
            RecallKind::Span => "span",
            RecallKind::Typo => "typo",
            RecallKind::Phrase => "phrase",
        }
    }

    fn from_raw(raw: &str) -> Option<Self> {
        [RecallKind::Whole, RecallKind::Span, RecallKind::Typo, RecallKind::Phrase]
            .into_iter()
            .find(|k| k.raw_value() == raw)
    }
}

/// How strongly a recall is remembered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecallValue {
    /// What it remembers.
    pub kind: RecallKind,
    /// Short-term strength.
    pub short: DecayingCount,
    /// Long-term strength.
    pub long: DecayingCount,
    /// Times picked.
    pub picks: i64,
}

impl RecallValue {
    /// Half-life of the short-term strength.
    pub const SHORT_HALF_LIFE: f64 = 8.0 * 3600.0;
    /// Half-life of the long-term strength.
    pub const LONG_HALF_LIFE: f64 = 90.0 * 86400.0;
    /// Picked this many times, a recall keeps a floor and is never forgotten.
    pub const FLOOR_PICKS: i64 = 3;

    fn new(kind: RecallKind) -> Self {
        Self { kind, short: DecayingCount::default(), long: DecayingCount::default(), picks: 0 }
    }

    /// Its strength at `now`.
    #[must_use]
    pub fn value(&self, now: f64) -> f64 {
        let long = self.long.at(now, Self::LONG_HALF_LIFE);
        self.short.at(now, Self::SHORT_HALF_LIFE)
            + if self.picks >= Self::FLOOR_PICKS { swift_max(long, 1.0) } else { long }
    }

    /// One pick stays active for about two months unless used again.
    #[must_use]
    pub fn is_active(&self, now: f64) -> bool {
        self.value(now) >= 0.6
    }
}

/// One word of a committed composition, as the composer hands it to the engine for learning.
#[derive(Clone, Debug, PartialEq)]
pub struct CommittedWord {
    /// Composition tokens it covered.
    pub range: Range<usize>,
    /// The word.
    pub text: String,
    /// Its reading; empty for a literal.
    pub reading: String,
    /// Its language-model ID.
    pub word_id: Option<u32>,
    /// The user picked it.
    pub picked: bool,
}

/// One lexicon word matching a span of syllables.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// Composition tokens it covers.
    pub range: Range<usize>,
    /// The reading as typed (the tokens' readings).
    pub typed_reading: Arc<str>,
    /// The entry, with its lexicon reading and penalty.
    pub entry: WordEntry,
    /// The plain lexicon reading, when it differs from the keys typed (a correction or fuzzy sound).
    pub annotation: Option<Arc<str>>,
}

/// The language model the engine scores with: the base model (unigram, one n-gram model or the two-domain mixture),
/// optionally boundary-free, adapted by the user's statistics.
pub struct EngineModel {
    base: BaseModel,
    boundary_free: bool,
    personal: Arc<PersonalLanguageModel>,
    end_share: f64,
}

// One per engine, read on every score: inline variants avoid an indirection, and the size does not matter.
#[allow(clippy::large_enum_variant)]
enum BaseModel {
    Unigram(UnigramLanguageModel),
    Vocabulary(VocabularyModel),
    Mixture(MixtureLanguageModel),
}

impl BaseModel {
    #[inline]
    fn order(&self) -> usize {
        match self {
            BaseModel::Unigram(m) => m.order(),
            BaseModel::Vocabulary(m) => m.order(),
            BaseModel::Mixture(m) => m.order(),
        }
    }
    #[inline]
    fn word_id(&self, word: &str) -> Option<u32> {
        match self {
            BaseModel::Unigram(m) => m.word_id(word),
            BaseModel::Vocabulary(m) => m.word_id(word),
            BaseModel::Mixture(m) => m.word_id(word),
        }
    }
    #[inline]
    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        match self {
            BaseModel::Unigram(m) => m.score(word_id, history, unigram),
            BaseModel::Vocabulary(m) => m.score(word_id, history, unigram),
            BaseModel::Mixture(m) => m.score(word_id, history, unigram),
        }
    }
}

impl EngineModel {
    /// The engine's sentence-end share.
    #[must_use]
    pub fn end_share(&self) -> f64 {
        self.end_share
    }

    /// [`LanguageModel::end_score`] with the engine's sentence-end share.
    #[must_use]
    pub fn end(&self, history: LanguageModelHistory) -> f64 {
        self.end_score(history, self.end_share)
    }
}

impl LanguageModel for EngineModel {
    #[inline]
    fn order(&self) -> usize {
        if self.personal.has_data() { 3 } else { self.base.order() }
    }
    #[inline]
    fn word_id(&self, word: &str) -> Option<u32> {
        self.base.word_id(word)
    }
    #[inline]
    fn score(&self, word_id: Option<u32>, mut history: LanguageModelHistory, unigram: f64) -> f64 {
        let original = history;
        let base = if self.boundary_free {
            if word_id == Some(0) {
                0.0
            } else {
                if history.previous == 1 {
                    history.previous = NO_CONTEXT;
                    history.older = None;
                }
                if history.older == Some(1) {
                    history.older = None;
                }
                self.base.score(word_id, history, unigram)
            }
        } else {
            self.base.score(word_id, history, unigram)
        };
        self.personal.adapt(base, word_id, original)
    }
}

struct State {
    lexicon: HashMap<String, Vec<WordEntry>>,
    reverse: HashMap<String, Vec<(String, f64)>>,
    phrases: HashMap<PhraseKey, f64>,
    phrase_index: HashMap<String, HashMap<String, WordEntry>>,
    word_ids: HashMap<String, u32>,
    reading_index: HashMap<String, Vec<String>>,
    syllables: SyllableSet,
    match_cache: HashMap<String, Arc<Vec<Match>>>,
    reading_prefixes: HashSet<String>,
    alternative_cache: HashMap<String, Arc<Vec<ReadingAlternative>>>,
    entry_cache: HashMap<String, Arc<Vec<WordEntry>>>,
    learned: HashMap<PhraseKey, RecallValue>,
    excluded: HashSet<PhraseKey>,
    typo_counts: HashMap<String, f64>,
    next_personal_id: u32,
    cache_day: i64,
    learned_syllables: HashSet<String>,
    learned_words: HashMap<String, HashSet<String>>,
    revision: u64,
    fuzzy: FuzzySounds,
}

/// The texts of the personal data files.
struct Snapshot {
    phrases: String,
    learned: String,
    excluded: String,
    typos: String,
    personal: String,
}

enum Job {
    Write(PathBuf, Snapshot),
    Flush(mpsc::Sender<()>),
}

/// Writes personal data on its own thread, 150 ms after the last change.
struct Writer {
    sender: Option<mpsc::Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let thread = std::thread::Builder::new()
            .name("zhuyin-personal-data".into())
            .spawn(move || {
                let mut pending: Option<(Instant, PathBuf, Snapshot)> = None;
                loop {
                    let job = match &pending {
                        Some((deadline, _, _)) => {
                            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                                Ok(job) => Some(job),
                                Err(mpsc::RecvTimeoutError::Timeout) => None,
                                Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    if let Some((_, directory, snapshot)) = pending.take() {
                                        write_files(&directory, &snapshot);
                                    }
                                    return;
                                }
                            }
                        }
                        None => match receiver.recv() {
                            Ok(job) => Some(job),
                            Err(_) => return,
                        },
                    };
                    match job {
                        Some(Job::Write(directory, snapshot)) => {
                            pending = Some((Instant::now() + Duration::from_millis(150), directory, snapshot));
                        }
                        Some(Job::Flush(done)) => {
                            if let Some((_, directory, snapshot)) = pending.take() {
                                write_files(&directory, &snapshot);
                            }
                            let _ = done.send(());
                        }
                        None => {
                            if let Some((_, directory, snapshot)) = pending.take() {
                                write_files(&directory, &snapshot);
                            }
                        }
                    }
                }
            })
            .expect("personal data thread");
        Self { sender: Some(sender), thread: Some(thread) }
    }

    fn send(&self, job: Job) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(job);
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Replaces `path` with `contents` atomically (a temporary file renamed over it).
fn write_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)
}

fn write_files(directory: &Path, snapshot: &Snapshot) {
    let result = (|| {
        std::fs::create_dir_all(directory)?;
        write_atomically(&directory.join("user_phrases.tsv"), &snapshot.phrases)?;
        write_atomically(&directory.join("learned.tsv"), &snapshot.learned)?;
        write_atomically(&directory.join("excluded.tsv"), &snapshot.excluded)?;
        write_atomically(&directory.join("typos.tsv"), &snapshot.typos)?;
        write_atomically(&directory.join(PersonalLanguageModel::FILE_NAME), &snapshot.personal)
    })();
    if let Err(error) = result {
        eprintln!("ZhuyinCore: could not save personal data: {error}");
    }
}

/// Why an operation on personal data was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A phrase needs 1–6 characters and one valid Zhuyin reading per character.
    InvalidPhrase,
    /// The file is not a personal data export.
    NotAnExport,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EngineError::InvalidPhrase => "A phrase needs 1–6 characters and one valid Zhuyin reading per character.",
            EngineError::NotAnExport => "This file is not a ZhuyinType personal data export.",
        })
    }
}

impl std::error::Error for EngineError {}

/// Han characters (CJK unified ideographs, extension A and B–G, compatibility ideographs): every scalar of the
/// grapheme in those blocks.
#[must_use]
pub fn is_han(grapheme: &str) -> bool {
    grapheme.chars().all(|c| matches!(c as u32, 0x3400..=0x9FFF | 0xF900..=0xFAFF | 0x2_0000..=0x3_134F))
}

/// The lexicon a new engine loads.
pub enum LexiconSource<'a> {
    /// `lexicon.tsv` or `lexicon.bin` at this path: the `.bin` next to it is used when it exists.
    Path(&'a Path),
    /// TSV text.
    Text(&'a str),
}

/// `values` in an allocation of exactly their size, for the caches that keep them for the session (growing a vector
/// leaves up to half of it spare, and shrinking in place may not return it).
fn exact<T>(values: Vec<T>) -> Vec<T> {
    if values.capacity() == values.len() {
        return values;
    }
    let mut result = Vec::with_capacity(values.len());
    result.extend(values);
    result
}

/// The shared engine.
pub struct Engine {
    config: EngineConfig,
    mapped: Option<MappedLexicon>,
    model: EngineModel,
    personal: Arc<PersonalLanguageModel>,
    state: Mutex<State>,
    directory: Option<PathBuf>,
    writer: Writer,
}

impl Engine {
    /// An engine for `lexicon` with the language model in `lexicon`'s directory `lm/`.
    ///
    /// # Errors
    /// When the lexicon cannot be read or its `.bin` is invalid.
    pub fn open(lexicon: &Path, user_data: Option<&Path>, config: EngineConfig) -> Result<Engine, DataError> {
        let directory = lexicon.parent().unwrap_or(Path::new("")).join("lm");
        Self::open_with_model(lexicon, user_data, Some(&directory), config)
    }

    /// An engine for `lexicon` with an explicit language-model directory (`None`: no language model).
    ///
    /// # Errors
    /// When the lexicon cannot be read or its `.bin` is invalid.
    pub fn open_with_model(
        lexicon: &Path,
        user_data: Option<&Path>,
        model: Option<&Path>,
        config: EngineConfig,
    ) -> Result<Engine, DataError> {
        let binary = lexicon.with_extension("bin");
        if binary.exists() {
            let mapped = MappedLexicon::open(&binary)?;
            Ok(Self::build(&[], Some(mapped), user_data, model, config))
        } else {
            let data = std::fs::read(lexicon)?;
            Ok(Self::build(&data, None, user_data, model, config))
        }
    }

    /// An engine for TSV lexicon text.
    #[must_use]
    pub fn from_text(text: &str, user_data: Option<&Path>, model: Option<&Path>, config: EngineConfig) -> Engine {
        Self::build(text.as_bytes(), None, user_data, model, config)
    }

    fn build(
        data: &[u8],
        mapped: Option<MappedLexicon>,
        user_data: Option<&Path>,
        model_directory: Option<&Path>,
        config: EngineConfig,
    ) -> Engine {
        let personal = Arc::new(PersonalLanguageModel::new());
        let mut base = BaseModel::Unigram(UnigramLanguageModel);
        if let Some(encyclopedia) = model_directory.and_then(VocabularyModel::load) {
            let chat = (encyclopedia.order() == 2)
                .then(|| config.chat_model_directory.as_deref().and_then(VocabularyModel::load))
                .flatten()
                .filter(|chat| chat.order() == 2);
            base = match chat {
                Some(chat) => BaseModel::Mixture(MixtureLanguageModel::new(chat, encyclopedia, config.chat_prior)),
                None => BaseModel::Vocabulary(encyclopedia),
            };
        }
        let boundary_free = !(config.sentence_boundaries || base.order() == 1);
        let model =
            EngineModel { base, boundary_free, personal: personal.clone(), end_share: config.sentence_end_share };
        let mut state = State {
            lexicon: HashMap::new(),
            reverse: HashMap::new(),
            phrases: HashMap::new(),
            phrase_index: HashMap::new(),
            word_ids: HashMap::new(),
            reading_index: HashMap::new(),
            syllables: SyllableSet::default(),
            match_cache: HashMap::new(),
            reading_prefixes: HashSet::new(),
            alternative_cache: HashMap::new(),
            entry_cache: HashMap::new(),
            learned: HashMap::new(),
            excluded: HashSet::new(),
            typo_counts: HashMap::new(),
            next_personal_id: FIRST_PERSONAL_ID,
            cache_day: i64::MIN,
            learned_syllables: HashSet::new(),
            learned_words: HashMap::new(),
            revision: 0,
            fuzzy: FuzzySounds::NONE,
        };
        if let Some(mapped) = &mapped {
            state.next_personal_id += mapped.word_count() as u32;
        } else {
            let mut word_ids: HashMap<String, u32> = HashMap::new();
            let mut next = state.next_personal_id;
            let text = TextLexicon::parse(data, |word| {
                if let Some(&id) = word_ids.get(word) {
                    return Some(id);
                }
                let id = model.word_id(word).unwrap_or_else(|| {
                    next += 1;
                    next - 1
                });
                word_ids.insert(word.to_owned(), id);
                Some(id)
            });
            state.word_ids = word_ids;
            state.next_personal_id = next;
            state.lexicon = text.entries;
            state.reverse = text.reverse;
        }
        let engine = Engine {
            config,
            mapped,
            model,
            personal,
            state: Mutex::new(state),
            directory: user_data.map(Path::to_path_buf),
            writer: Writer::new(),
        };
        {
            let mut state = engine.lock();
            engine.load_personal_data(&mut state);
            engine.rebuild_reading_index(&mut state);
        }
        engine
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The engine's settings.
    #[must_use]
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// The language model the engine scores with.
    #[must_use]
    pub fn language_model(&self) -> &EngineModel {
        &self.model
    }

    /// The user's statistics.
    #[must_use]
    pub fn personal(&self) -> &PersonalLanguageModel {
        &self.personal
    }

    /// The time memories decay by (seconds since 1970).
    #[must_use]
    pub fn now(&self) -> f64 {
        self.personal.now()
    }

    /// Replaces the clock (tests and reproducible runs).
    pub fn set_clock(&self, clock: impl Fn() -> f64 + Send + Sync + 'static) {
        self.personal.set_clock(clock);
    }

    /// Sound pairs that match each other.
    #[must_use]
    pub fn fuzzy_sounds(&self) -> FuzzySounds {
        self.lock().fuzzy
    }

    /// Sets the fuzzy sound pairs.
    pub fn set_fuzzy_sounds(&self, fuzzy: FuzzySounds) {
        let mut state = self.lock();
        if fuzzy == state.fuzzy {
            return;
        }
        state.fuzzy = fuzzy;
        state.match_cache.clear();
        state.revision = state.revision.wrapping_add(1);
    }

    /// Changes whenever conversions may change: cached lattices of an older revision are discarded.
    #[must_use]
    pub fn conversion_revision(&self) -> u64 {
        self.lock().revision
    }

    /// Every word gets an ID: the language model's, or a personal one above its vocabulary.
    fn resolve_id(&self, state: &mut State, word: &str) -> u32 {
        if let Some(index) = self.mapped.as_ref().and_then(|m| m.word_index(word)) {
            return self.model.word_id(word).unwrap_or(FIRST_PERSONAL_ID + index as u32);
        }
        if let Some(&id) = state.word_ids.get(word) {
            return id;
        }
        let id = self.model.word_id(word).unwrap_or_else(|| {
            state.next_personal_id += 1;
            state.next_personal_id - 1
        });
        state.word_ids.insert(word.to_owned(), id);
        id
    }

    fn lexical_entries(&self, state: &mut State, reading: &str) -> Vec<WordEntry> {
        let Some(mapped) = &self.mapped else { return state.lexicon.get(reading).cloned().unwrap_or_default() };
        mapped
            .entry_refs(reading)
            .map(|entry| {
                WordEntry::new(entry.word, entry.logp)
                    .with_reading_prior(entry.reading_prior)
                    .with_word_id(Some(self.resolve_id(state, entry.word)))
            })
            .collect()
    }

    fn known_readings(&self, state: &State, word: &str) -> Vec<(String, f64)> {
        match &self.mapped {
            Some(mapped) => mapped.readings_of(word),
            None => state.reverse.get(word).cloned().unwrap_or_default(),
        }
    }

    fn best_reading(&self, state: &State, word: &str) -> Option<(String, f64)> {
        if let Some(mapped) = &self.mapped {
            return mapped.best_reading(word);
        }
        let readings = state.reverse.get(word)?;
        let mut best = readings.first()?;
        for candidate in &readings[1..] {
            if best.1 < candidate.1 {
                best = candidate;
            }
        }
        Some(best.clone())
    }

    fn knows_word(&self, state: &State, word: &str) -> bool {
        match &self.mapped {
            Some(mapped) => mapped.word_index(word).is_some(),
            None => state.reverse.contains_key(word),
        }
    }

    /// Memories decay, so cached lookups live for at most a day.
    fn expire_caches_on_new_day(&self, state: &mut State) {
        let today = (self.personal.now() / 86400.0) as i64;
        if today == state.cache_day {
            return;
        }
        state.cache_day = today;
        state.entry_cache.clear();
        state.match_cache.clear();
        state.revision = state.revision.wrapping_add(1);
    }

    /// Every word for `reading`: its own entries, those of the readings that differ only in neutral tones (with
    /// the tolerance penalty), user phrases, and active recalls.
    pub fn entries(&self, reading: &str) -> Arc<Vec<WordEntry>> {
        let mut state = self.lock();
        self.expire_caches_on_new_day(&mut state);
        if let Some(cached) = state.entry_cache.get(reading) {
            return cached.clone();
        }
        let typed: Vec<&str> = reading.split(' ').filter(|p| !p.is_empty()).collect();
        let base = without_tones(reading);
        let mut alternatives: HashSet<String> =
            self.mapped.as_ref().map(|m| m.readings_for_base(&base)).unwrap_or_default().into_iter().collect();
        alternatives.extend(state.reading_index.get(&base).cloned().unwrap_or_default());
        let mut others: Vec<String> = alternatives.into_iter().filter(|r| r != reading).collect();
        others.sort();
        let mut result: Vec<WordEntry> = Vec::new();
        let mut position: HashMap<Arc<str>, usize> = HashMap::new();
        for source in std::iter::once(reading.to_owned()).chain(others) {
            let mut penalty = 0.0;
            if source != reading {
                for (t, l) in typed.iter().zip(source.split(' ').filter(|p| !p.is_empty())) {
                    if *t == l {
                        continue;
                    }
                    if l.ends_with('˙') {
                        penalty += 0.4;
                    } else if t.ends_with('˙') {
                        penalty += 1.2;
                    } else {
                        penalty = f64::INFINITY;
                        break;
                    }
                }
                if !penalty.is_finite() {
                    continue;
                }
            }
            let mut entries = self.lexical_entries(&mut state, &source);
            if let Some(phrases) = state.phrase_index.get(&source) {
                let mut phrases: Vec<(&String, &WordEntry)> = phrases.iter().collect();
                phrases.sort_by(|a, b| a.0.cmp(b.0));
                for (word, phrase) in phrases {
                    if let Some(entry) = entries.iter_mut().find(|e| &*e.word == word.as_str()) {
                        entry.logp = phrase.logp;
                        entry.reading_prior = 0.0;
                        entry.recalled = source == reading;
                    } else {
                        let mut entry = phrase.clone();
                        entry.recalled = source == reading;
                        entries.push(entry);
                    }
                }
            }
            // One entry per word: the reading that costs least.
            let source: Arc<str> = source.into();
            for mut entry in entries {
                entry.reading = Some(source.clone());
                entry.penalty = penalty;
                if let Some(&index) = position.get(&entry.word) {
                    let old = &result[index];
                    let better = entry.reading_prior - penalty > old.reading_prior - old.penalty;
                    if old.recalled || !better {
                        continue;
                    }
                    result[index] = entry;
                } else {
                    position.insert(entry.word.clone(), result.len());
                    result.push(entry);
                }
            }
        }
        let now = self.personal.now();
        let mut learned: Vec<String> =
            state.learned_words.get(reading).map(|w| w.iter().cloned().collect()).unwrap_or_default();
        learned.sort();
        for word in learned {
            let Some(value) = state.learned.get(&PhraseKey::new(reading, word.as_str())).copied() else { continue };
            if !value.is_active(now) {
                continue;
            }
            let source = self.best_reading(&state, &word);
            let id = self.resolve_id(&mut state, &word);
            let mut entry = WordEntry::new(word.as_str(), source.map_or(-7.0, |s| s.1)).with_word_id(Some(id));
            entry.recalled = true;
            entry.whole_only = value.kind == RecallKind::Whole;
            result.push(entry);
        }
        if state.entry_cache.len() >= 4096 {
            state.entry_cache.clear();
            state.match_cache.clear();
        }
        let result = Arc::new(exact(result));
        state.entry_cache.insert(reading.to_owned(), result.clone());
        result
    }

    fn rebuild_reading_index(&self, state: &mut State) {
        state.reading_prefixes.clear();
        state.alternative_cache.clear();
        state.reading_index.clear();
        state.entry_cache.clear();
        state.match_cache.clear();
        let mut readings: HashSet<&String> = state.lexicon.keys().collect();
        readings.extend(state.phrase_index.iter().filter(|(_, words)| !words.is_empty()).map(|(r, _)| r));
        let mut syllables: HashSet<String> = HashSet::new();
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for reading in readings {
            index.entry(without_tones(reading)).or_default().push(reading.clone());
            for part in reading.split(' ').filter(|p| !p.is_empty()) {
                syllables.insert(part.to_owned());
            }
        }
        state.reading_index = index;
        let bases: Vec<String> = state.reading_index.keys().cloned().collect();
        for base in bases {
            add_base_prefixes(&mut state.reading_prefixes, &base);
        }
        let learned: Vec<String> = state.learned_words.keys().cloned().collect();
        for reading in learned {
            add_base_prefixes(&mut state.reading_prefixes, &without_tones(&reading));
        }
        if let Some(mapped) = &self.mapped {
            syllables.extend(mapped.syllable_strs().map(str::to_owned));
        }
        state.syllables = SyllableSet::new(syllables);
        state.syllables.set_learned(state.learned_syllables.iter().cloned());
    }

    /// Whether `reading` is a syllable the composer accepts.
    #[must_use]
    pub fn is_valid_syllable(&self, reading: &str) -> bool {
        self.lock().syllables.is_valid(reading)
    }

    /// The eight cheapest valid readings of actual typed keys, before strength scaling.
    #[must_use]
    pub fn typo_alternatives(&self, keys: &str) -> Vec<(String, f64)> {
        self.alternatives(keys).iter().map(|a| (a.reading.clone(), a.cost)).collect()
    }

    /// The keyboard channel's corrections of `keys`, with this user's slips cheaper; cached.
    pub fn alternatives(&self, keys: &str) -> Arc<Vec<ReadingAlternative>> {
        let mut state = self.lock();
        if let Some(cached) = state.alternative_cache.get(keys) {
            return cached.clone();
        }
        let mut discounts: HashMap<(u8, u8), f64> = HashMap::new();
        for (pair, &n) in &state.typo_counts {
            let mut chars = pair.chars();
            if let (Some(typed), Some(intended), None) = (chars.next(), chars.next(), chars.next())
                && typed.is_ascii()
                && intended.is_ascii()
            {
                discounts.insert((typed as u8, intended as u8), swift_min(TYPO_DISCOUNT_CAP, 1.5 * (1.0 + n).ln()));
            }
        }
        let syllables = &state.syllables;
        let result = key_channel::alternatives(
            keys,
            self.config.hotspot,
            |typed, intended| discounts.get(&(typed, intended)).copied().unwrap_or(0.0),
            |code| syllables.valid_reading(code),
        );
        let result = Arc::new(exact(result));
        if state.alternative_cache.len() >= 4096 {
            state.alternative_cache.clear();
        }
        state.alternative_cache.insert(keys.to_owned(), result.clone());
        result
    }

    /// The words matching the syllables from `start` (at most six), as typed, with tone tolerance, corrections and
    /// fuzzy sounds; cached per keys.
    pub fn matches(&self, tokens: &[CompositionToken], start: usize, typo: TypoTolerance) -> Vec<Match> {
        let fuzzy = self.fuzzy_sounds();
        let mut span: Vec<&CompositionToken> = Vec::new();
        let mut key = format!("{}{}", typo.raw_value(), fuzzy.raw_value());
        for token in &tokens[start..tokens.len().min(start + 6)] {
            let CompositionToken::Syllable { keys, .. } = token else { break };
            span.push(token);
            key.push('\t');
            key.push_str(keys);
        }
        let (cached, revision) = {
            let mut state = self.lock();
            self.expire_caches_on_new_day(&mut state);
            (state.match_cache.get(&key).cloned(), state.revision)
        };
        let matches = if let Some(cached) = cached {
            cached
        } else {
            let built = Arc::new(exact(self.build_matches(&span, typo, fuzzy)));
            let mut state = self.lock();
            if revision == state.revision {
                if state.match_cache.len() >= 4096 {
                    state.match_cache.clear();
                }
                state.match_cache.insert(key, built.clone());
            }
            built
        };
        matches
            .iter()
            .filter_map(|m| {
                let mut m = m.clone();
                m.range = (m.range.start + start)..(m.range.end + start);
                (!(m.entry.whole_only && m.range != (0..tokens.len()))).then_some(m)
            })
            .collect()
    }

    fn build_matches(&self, tokens: &[&CompositionToken], typo: TypoTolerance, fuzzy: FuzzySounds) -> Vec<Match> {
        let mut options: Vec<Vec<ReadingAlternative>> = Vec::new();
        let mut typed: Vec<&str> = Vec::new();
        for token in tokens {
            let CompositionToken::Syllable { reading, keys, .. } = token else { break };
            typed.push(reading);
            let mut cheapest: HashMap<String, ReadingAlternative> = HashMap::new();
            cheapest.insert(
                reading.to_string(),
                ReadingAlternative { reading: reading.to_string(), cost: 0.0, fuzzy: false },
            );
            let corrections = if typo == TypoTolerance::Off { Arc::new(vec![]) } else { self.alternatives(keys) };
            for option in fuzzy_alternatives(reading, fuzzy).into_iter().chain(corrections.iter().cloned()) {
                if option.cost < cheapest.get(&option.reading).map_or(f64::INFINITY, |o| o.cost) {
                    cheapest.insert(option.reading.clone(), option);
                }
            }
            let mut list: Vec<ReadingAlternative> = cheapest.into_values().collect();
            list.sort_by(|a, b| swift_order(a.cost, b.cost).then_with(|| a.reading.cmp(&b.reading)));
            options.push(list);
        }
        let mut result: Vec<Match> = Vec::new();
        self.visit(&options, &typed, typo, 0, "", "", 0.0, 0, &mut result);
        // The language model sees the same word ID on duplicate paths; keep its best channel/lexicon score once.
        let order = self.model.order();
        let mut best: HashMap<PhraseKey, (Match, f64)> = HashMap::new();
        let mut order_seen: Vec<PhraseKey> = Vec::new();
        for m in result {
            let key = PhraseKey::new(
                format!("{}{}", m.typed_reading, if m.entry.whole_only { "\u{1}" } else { "" }),
                &*m.entry.word,
            );
            let (unigram, adjustment) = self.scoring(&m.entry, &m.typed_reading);
            let value =
                adjustment + if !m.entry.recalled && (order == 1 || m.entry.word_id.is_none()) { unigram } else { 0.0 };
            if let Some(old) = best.get(&key)
                && (old.1 > value || (old.1 == value && old.0.annotation.is_none()))
            {
                continue;
            }
            if !best.contains_key(&key) {
                order_seen.push(key.clone());
            }
            best.insert(key, (m, value));
        }
        let mut matches: Vec<Match> = Vec::with_capacity(order_seen.len());
        matches.extend(order_seen.into_iter().filter_map(|k| best.remove(&k).map(|b| b.0)));
        // A word is kept once per span and recall kind, so these keys order every match.
        matches.sort_by(|a, b| {
            a.range
                .len()
                .cmp(&b.range.len())
                .then_with(|| swift_order(a.entry.penalty, b.entry.penalty))
                .then_with(|| a.entry.word.cmp(&b.entry.word))
                .then_with(|| a.entry.whole_only.cmp(&b.entry.whole_only))
        });
        matches
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &self,
        options: &[Vec<ReadingAlternative>],
        typed: &[&str],
        typo: TypoTolerance,
        index: usize,
        reading: &str,
        base: &str,
        cost: f64,
        edits: usize,
        result: &mut Vec<Match>,
    ) {
        if index >= options.len() {
            return;
        }
        for option in &options[index] {
            let count = edits + usize::from(option.cost > 0.0 && !option.fuzzy);
            if count > 2 {
                continue;
            }
            let next =
                if reading.is_empty() { option.reading.clone() } else { format!("{reading} {}", option.reading) };
            let option_base = without_tones(&option.reading);
            let next_base = if base.is_empty() { option_base } else { format!("{base} {option_base}") };
            let known = {
                let state = self.lock();
                state.reading_prefixes.contains(&next_base)
                    || self.mapped.as_ref().is_some_and(|m| m.contains_prefix(&next_base))
            };
            if !known {
                continue;
            }
            let total = cost + option.cost * typo.cost_multiplier();
            if count <= if index < 3 { 1 } else { 2 } {
                let typed_reading: Arc<str> = typed[..=index].join(" ").into();
                // Entries come grouped by the reading they were found under: one annotation per reading, shared.
                let mut shared: Option<(String, Arc<str>)> = None;
                for entry in self.entries(&next).iter() {
                    let mut entry = entry.clone();
                    let annotation = (total > 0.0).then(|| {
                        let source = entry.reading.as_deref().unwrap_or(&next);
                        match &shared {
                            Some((reading, annotation)) if reading == source => annotation.clone(),
                            _ => {
                                let annotation: Arc<str> = Arc::from(keyboard::plain(source).as_str());
                                shared = Some((source.to_owned(), annotation.clone()));
                                annotation
                            }
                        }
                    });
                    entry.recalled = entry.recalled && *typed_reading == *next;
                    entry.penalty += total;
                    result.push(Match { range: 0..index + 1, typed_reading: typed_reading.clone(), entry, annotation });
                }
            }
            self.visit(options, typed, typo, index + 1, &next, &next_base, total, count, result);
        }
    }

    /// The lexicon probability and the adjustment (phrase boost, penalty, reading prior) of an entry.
    #[must_use]
    pub fn scoring(&self, entry: &WordEntry, reading: &str) -> (f64, f64) {
        let mut state = self.lock();
        let source = entry.reading.as_deref().unwrap_or(reading);
        let phrase = state.phrases.get(&PhraseKey::new(source, &*entry.word)).copied();
        let original = match phrase {
            None => entry.logp,
            Some(_) => {
                self.lexical_entries(&mut state, source).iter().find(|e| e.word == entry.word).map_or(-7.0, |e| e.logp)
            }
        };
        (original, (phrase.map_or(0.0, |p| p - original) - entry.penalty) + entry.reading_prior)
    }

    /// An entry's score after `history`.
    #[must_use]
    pub fn score(&self, entry: &WordEntry, reading: &str, history: LanguageModelHistory) -> f64 {
        let (unigram, adjustment) = self.scoring(entry, reading);
        (if entry.recalled { 0.0 } else { self.model.score(entry.word_id, history, unigram) }) + adjustment
    }

    /// Every user phrase, sorted.
    #[must_use]
    pub fn user_phrases(&self) -> Vec<PhraseKey> {
        let mut keys: Vec<PhraseKey> = self.lock().phrases.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Adds a user phrase.
    ///
    /// # Errors
    /// [`EngineError::InvalidPhrase`] unless the phrase has 1–6 characters and one valid reading per character.
    pub fn add_user_phrase(&self, word: &str, reading: &str) -> Result<(), EngineError> {
        if !valid_phrase(word, reading) {
            return Err(EngineError::InvalidPhrase);
        }
        let mut state = self.lock();
        self.add_phrase(&mut state, word, reading);
        state.revision = state.revision.wrapping_add(1);
        self.rebuild_reading_index(&mut state);
        self.schedule_write(&state);
        Ok(())
    }

    fn add_phrase(&self, state: &mut State, word: &str, reading: &str) {
        let key = PhraseKey::new(reading, word);
        let lexical = self.lexical_entries(state, reading).iter().find(|e| &*e.word == word).map_or(-7.0, |e| e.logp);
        let logp = swift_max(lexical, -7.0) + 1.0;
        state.phrases.insert(key, logp);
        let id = self.resolve_id(state, word);
        state
            .phrase_index
            .entry(reading.to_owned())
            .or_default()
            .insert(word.to_owned(), WordEntry::new(word, logp).with_word_id(Some(id)));
    }

    /// Removes a user phrase.
    pub fn remove_user_phrase(&self, word: &str, reading: &str) {
        let mut state = self.lock();
        state.phrases.remove(&PhraseKey::new(reading, word));
        if let Some(words) = state.phrase_index.get_mut(reading) {
            words.remove(word);
        }
        state.revision = state.revision.wrapping_add(1);
        self.rebuild_reading_index(&mut state);
        self.schedule_write(&state);
    }

    /// The word's readings: the lexicon's by score, then user phrases'; for an unknown word, each character's most
    /// common reading.
    #[must_use]
    pub fn readings(&self, word: &str) -> Vec<String> {
        let state = self.lock();
        let mut known = self.known_readings(&state, word);
        known.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut personal: Vec<String> =
            state.phrases.keys().filter(|k| k.word == word).map(|k| k.reading.clone()).collect();
        personal.sort();
        let mut seen = HashSet::new();
        let result: Vec<String> =
            known.into_iter().map(|k| k.0).chain(personal).filter(|r| seen.insert(r.clone())).collect();
        if !result.is_empty() {
            return result;
        }
        let mut parts = Vec::new();
        for character in graphemes(word) {
            let Some(best) = self.best_reading(&state, character) else { return vec![] };
            parts.push(best.0);
        }
        if parts.is_empty() { vec![] } else { vec![parts.join(" ")] }
    }

    /// What the engine remembers, one row per word or phrase: recalls and picked words, strengths to one decimal.
    #[must_use]
    pub fn learned_entries(&self) -> Vec<(PhraseKey, f64)> {
        let state = self.lock();
        let now = self.personal.now();
        let mut rows: HashMap<PhraseKey, f64> = HashMap::new();
        for (key, value) in &state.learned {
            if value.is_active(now) {
                rows.insert(key.clone(), value.value(now));
            }
        }
        for row in self.personal.picked_words() {
            let key = PhraseKey::new(row.reading, row.word);
            let old = rows.get(&key).copied().unwrap_or(0.0);
            rows.insert(key, swift_max(old, row.count));
        }
        let mut result: Vec<(PhraseKey, f64)> = rows.into_iter().map(|(k, v)| (k, (v * 10.0).round() / 10.0)).collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Forgets a remembered word.
    pub fn forget_learned(&self, word: &str, reading: &str) {
        let mut state = self.lock();
        self.forget_learned_locked(&mut state, word, reading);
    }

    fn forget_learned_locked(&self, state: &mut State, word: &str, reading: &str) {
        state.learned.remove(&PhraseKey::new(reading, word));
        if let Some(words) = state.learned_words.get_mut(reading) {
            words.remove(word);
        }
        let id = self.resolve_id(state, word);
        self.personal.forget(id);
        self.learned_changed(state);
    }

    /// Sets the chat share of the mixture from the text before the cursor; returns it, or `None` without a mixture.
    pub fn adapt_domain(&self, text: &str) -> Option<f64> {
        let BaseModel::Mixture(mixture) = &self.model.base else { return None };
        let words = self.segment_words(text);
        let mut weight = self.config.chat_prior;
        if !words.is_empty() {
            let mut total = 0.0;
            let mut previous = 1;
            for word in &words {
                let id = mixture.word_id(word);
                let (c, e) = mixture.scores(id, previous, -14.0);
                total += c - e;
                previous = id.unwrap_or(2);
            }
            let count = words.len() as f64;
            let evidence = self.config.domain_evidence_scale * (total / count - self.config.domain_evidence_centre);
            let prior = (self.config.chat_prior / (1.0 - self.config.chat_prior)).ln();
            weight = 1.0 / (1.0 + (-(prior + evidence * swift_min(1.0, count / 4.0))).exp());
        }
        if (mixture.chat_weight() - weight).abs() > 0.01 {
            mixture.set_chat_weight(weight);
            let mut state = self.lock();
            state.entry_cache.clear();
            state.match_cache.clear();
            state.revision = state.revision.wrapping_add(1);
        }
        Some(weight)
    }

    /// Han runs of `text` split into dictionary words by longest match (for scoring, not display).
    fn segment_words(&self, text: &str) -> Vec<String> {
        let characters = graphemes(text);
        let mut words = Vec::new();
        let mut index = 0;
        while index < characters.len() {
            if !is_han(characters[index]) {
                index += 1;
                continue;
            }
            let mut length = 6.min(characters.len() - index);
            while length > 1 {
                let slice = &characters[index..index + length];
                let known = slice.iter().all(|c| is_han(c)) && {
                    let state = self.lock();
                    self.knows_word(&state, &slice.concat())
                };
                if known {
                    break;
                }
                length -= 1;
            }
            words.push(characters[index..index + length].concat());
            index += length;
        }
        words
    }

    /// The last words of `text` (at most two, within its final run of Han characters) by longest dictionary match
    /// from the end, each with its most common reading and the language-model history after it.
    #[must_use]
    pub fn trailing_words(&self, text: &str) -> Vec<ConvertedWord> {
        let characters = graphemes(text);
        let han = characters.iter().rev().take_while(|c| is_han(c)).count();
        let run = &characters[characters.len() - han.min(12)..];
        if run.is_empty() {
            return vec![];
        }
        let mut state = self.lock();
        let mut words: Vec<String> = Vec::new();
        let mut end = run.len();
        while end > 0 && words.len() < 2 {
            let mut length = 6.min(end);
            while length > 1 && !self.knows_word(&state, &run[end - length..end].concat()) {
                length -= 1;
            }
            words.insert(0, run[end - length..end].concat());
            end -= length;
        }
        let mut history = LanguageModelHistory::default();
        words
            .into_iter()
            .map(|word| {
                let id = self.resolve_id(&mut state, &word);
                history = history.appending(Some(id), self.model.order());
                let reading = self.best_reading(&state, &word).map(|b| b.0).unwrap_or_default();
                ConvertedWord {
                    range: 0..0,
                    text: word.into(),
                    reading: reading.into(),
                    word_id: Some(id),
                    history: Some(history),
                    corrected: false,
                }
            })
            .collect()
    }

    /// A correction was picked: each key typed for another makes that slip cheaper next time.
    pub fn record_typos(&self, syllables: &[(String, String)]) {
        let mut state = self.lock();
        let mut changed = false;
        for (typed, intended_reading) in syllables {
            let Some(intended) = key_channel::keys_for_reading(intended_reading) else { continue };
            for (t, i) in key_channel::substitutions(typed, &intended) {
                *state.typo_counts.entry(format!("{t}{i}")).or_insert(0.0) += 1.0;
                changed = true;
            }
        }
        if !changed {
            return;
        }
        state.alternative_cache.clear();
        state.entry_cache.clear();
        state.match_cache.clear();
        state.revision = state.revision.wrapping_add(1);
        self.schedule_write(&state);
    }

    /// Forgets `word` and never learns it again, until [`include_in_learning`](Self::include_in_learning).
    pub fn exclude_from_learning(&self, word: &str, reading: &str) {
        self.lock().excluded.insert(PhraseKey::new(reading, word));
        self.forget_learned(word, reading);
    }

    /// Learns `word` again.
    pub fn include_in_learning(&self, word: &str, reading: &str) {
        let mut state = self.lock();
        state.excluded.remove(&PhraseKey::new(reading, word));
        self.schedule_write(&state);
    }

    /// The words never to learn, sorted.
    #[must_use]
    pub fn excluded_entries(&self) -> Vec<PhraseKey> {
        let mut keys: Vec<PhraseKey> = self.lock().excluded.iter().cloned().collect();
        keys.sort();
        keys
    }

    /// Everything personal in one text file: user phrases, what was learned, and the words never to learn.
    #[must_use]
    pub fn export_personal_data(&self) -> String {
        let state = self.lock();
        let mut text = String::from("# ZhuyinType personal data 1\n[phrases]\n");
        text += &phrase_text(state.phrases.keys());
        text += "[learned]\n";
        text += &learned_text(&state.learned);
        text += "[ngrams]\n";
        text += &self.personal.serialized();
        text += "[excluded]\n";
        text += &phrase_text(state.excluded.iter());
        text += "[typos]\n";
        text += &typo_text(&state.typo_counts);
        text
    }

    /// Adds an export to what is here; for anything both have, the stronger memory stays.  Returns the rows read.
    ///
    /// # Errors
    /// [`EngineError::NotAnExport`] unless the data is UTF-8 starting with the export header.
    pub fn import_personal_data(&self, data: &[u8]) -> Result<usize, EngineError> {
        let text = std::str::from_utf8(data).map_err(|_| EngineError::NotAnExport)?;
        if !text.starts_with("# ZhuyinType personal data 1") {
            return Err(EngineError::NotAnExport);
        }
        let mut sections: HashMap<String, String> = HashMap::new();
        let mut current = String::new();
        for line in swift_lines(text) {
            if line.starts_with('[') && line.ends_with(']') && !line.is_empty() {
                let inner = line.strip_prefix('[').unwrap_or(line);
                inner.strip_suffix(']').unwrap_or(inner).clone_into(&mut current);
                continue;
            }
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let section = sections.entry(current.clone()).or_default();
            section.push_str(line);
            section.push('\n');
        }
        let section = |name: &str| sections.get(name).cloned().unwrap_or_default();
        let mut state = self.lock();
        let mut rows = 0;
        let phrases = section("phrases");
        scan_tsv(phrases.as_bytes(), false, |fields| {
            if fields.len() == 2 && valid_phrase(&fields[1], &fields[0]) {
                self.add_phrase(&mut state, &fields[1], &fields[0]);
                rows += 1;
            }
        });
        let learned = section("learned");
        scan_tsv(learned.as_bytes(), false, |fields| {
            let Some((key, value)) = learned_row(fields) else { return };
            let now = self.personal.now();
            if let Some(old) = state.learned.get(&key)
                && old.value(now) >= value.value(now)
            {
                return;
            }
            state.learned_words.entry(key.reading.clone()).or_default().insert(key.word.clone());
            state.learned.insert(key, value);
            rows += 1;
        });
        let ngrams = section("ngrams");
        rows += self.personal.load(ngrams.as_bytes(), true, |word| self.resolve_id(&mut state, word));
        let typos = section("typos");
        scan_tsv(typos.as_bytes(), false, |fields| {
            if fields.len() != 3 || grapheme_count(&fields[0]) != 1 || grapheme_count(&fields[1]) != 1 {
                return;
            }
            let Some(n) = parse_double(&fields[2]).filter(|n| *n > 0.0) else { return };
            let key = format!("{}{}", fields[0], fields[1]);
            let old = state.typo_counts.get(&key).copied().unwrap_or(0.0);
            state.typo_counts.insert(key, swift_max(old, n));
            rows += 1;
        });
        state.alternative_cache.clear();
        let excluded = section("excluded");
        scan_tsv(excluded.as_bytes(), false, |fields| {
            if fields.len() == 2 {
                state.excluded.insert(PhraseKey::new(fields[0].as_ref(), fields[1].as_ref()));
                rows += 1;
            }
        });
        state.revision = state.revision.wrapping_add(1);
        self.rebuild_reading_index(&mut state);
        let now = self.personal.now();
        trim_learned(&mut state, now);
        self.learned_changed(&mut state);
        Ok(rows)
    }

    /// Forgets everything learned (user phrases stay).
    pub fn forget_all_learned(&self) {
        let mut state = self.lock();
        state.learned.clear();
        state.learned_words.clear();
        state.typo_counts.clear();
        state.alternative_cache.clear();
        self.personal.forget_all();
        self.learned_changed(&mut state);
    }

    /// Learns from one committed composition.  Literal words split it into stretches for the personal n-grams;
    /// corrections also become recalls of their exact typed readings.  `context` holds up to two words committed
    /// just before, which the personal n-grams continue from.
    pub fn learn(&self, words: &[CommittedWord], syllable_count: usize, context: &[CommittedWord]) {
        let mut state = self.lock();
        let now = self.personal.now();
        let mut stretch: Vec<Word> = context
            .iter()
            .map(|word| {
                let resolved = self.resolve_id(&mut state, &word.text);
                Word {
                    id: word.word_id.unwrap_or(resolved),
                    text: word.text.clone(),
                    reading: word.reading.clone(),
                    picked: false,
                }
            })
            .collect();
        let mut leading = stretch.len();
        let mut syllables: Vec<(String, String, bool)> = Vec::new();
        let mut whole_is_plain = true;
        for word in words {
            if word.reading.is_empty()
                || state.excluded.contains(&PhraseKey::new(word.reading.as_str(), word.text.as_str()))
            {
                self.personal.learn(&stretch, leading);
                stretch.clear();
                leading = 0;
                whole_is_plain = false;
                syllables.push((String::new(), word.text.clone(), false));
                continue;
            }
            let id = if word.picked {
                self.resolve_id(&mut state, &word.text)
            } else {
                match word.word_id {
                    Some(id) => id,
                    None => self.resolve_id(&mut state, &word.text),
                }
            };
            stretch.push(Word { id, text: word.text.clone(), reading: word.reading.clone(), picked: word.picked });
            for (part, character) in word.reading.split(' ').filter(|p| !p.is_empty()).zip(graphemes(&word.text)) {
                syllables.push((part.to_owned(), character.to_owned(), word.picked));
            }
            if !word.picked {
                continue;
            }
            let key = PhraseKey::new(word.reading.as_str(), word.text.as_str());
            let lexical = self.lexical_entries(&mut state, &word.reading).iter().any(|e| *e.word == *word.text)
                || state.phrases.contains_key(&key);
            if !lexical {
                let kind = if self.knows_word(&state, &word.text) { RecallKind::Typo } else { RecallKind::Phrase };
                remember(&mut state, &key, kind, now);
            }
        }
        self.personal.learn(&stretch, leading);
        let picked: Vec<usize> = (0..syllables.len()).filter(|&i| syllables[i].2).collect();
        if picked.is_empty() {
            self.persist_learned(&mut state);
            return;
        }
        // The whole composition, when it is short enough to be looked up as one unit.
        if whole_is_plain && (1..=6).contains(&syllable_count) && syllables.len() == syllable_count {
            let reading = syllables.iter().map(|s| s.0.as_str()).collect::<Vec<_>>().join(" ");
            let text: String = syllables.iter().map(|s| s.1.as_str()).collect();
            remember(&mut state, &PhraseKey::new(reading, text), RecallKind::Whole, now);
        }
        // Each correction with up to two syllables of context on its left (its right at the start).
        for &index in &picked {
            let (mut lower, mut upper) = (index, index + 1);
            while lower > 0 && index - lower < 2 && !syllables[lower - 1].0.is_empty() {
                lower -= 1;
            }
            if lower == index {
                while upper < syllables.len() && upper - index < 3 && !syllables[upper].0.is_empty() {
                    upper += 1;
                }
            }
            if !(upper - lower >= 3 && (upper - lower < syllables.len() || !whole_is_plain || syllable_count > 6)) {
                continue;
            }
            let window = &syllables[lower..upper];
            let reading = window.iter().map(|s| s.0.as_str()).collect::<Vec<_>>().join(" ");
            let text: String = window.iter().map(|s| s.1.as_str()).collect();
            remember(&mut state, &PhraseKey::new(reading, text), RecallKind::Span, now);
        }
        self.persist_learned(&mut state);
    }

    /// The user picked this word in this place before (after the same word, or a phrase anywhere): the neural
    /// reranker must not replace it.
    #[must_use]
    pub fn remembers(&self, word: &ConvertedWord, history: LanguageModelHistory) -> bool {
        let state = self.lock();
        let phrase = grapheme_count(&word.text) > 1;
        if phrase && state.learned.get(&PhraseKey::new(&*word.reading, &*word.text)).is_some_and(|v| v.picks > 0) {
            return true;
        }
        let id = word
            .word_id
            .or_else(|| self.model.word_id(&word.text))
            .or_else(|| state.word_ids.get(&*word.text).copied())
            .or_else(|| {
                self.mapped.as_ref().and_then(|m| m.word_index(&word.text)).map(|i| FIRST_PERSONAL_ID + i as u32)
            });
        let Some(id) = id else { return false };
        self.personal.remembers(id, history, phrase)
    }

    /// The user picked something else where `text` was shown: weaken what made it the first choice.
    pub fn weaken(&self, text: &str, reading: &str, word_id: Option<u32>, history: Option<LanguageModelHistory>) {
        let mut state = self.lock();
        let now = self.personal.now();
        if let Some(value) = state.learned.get_mut(&PhraseKey::new(reading, text)) {
            value.short.scale(0.5, now, RecallValue::SHORT_HALF_LIFE);
            value.long.scale(0.5, now, RecallValue::LONG_HALF_LIFE);
            value.picks = 0;
        }
        let id = match word_id {
            Some(id) => id,
            None => self.resolve_id(&mut state, text),
        };
        if let Some(history) = history {
            self.personal.weaken(id, history);
        }
        self.learned_changed(&mut state);
    }

    fn persist_learned(&self, state: &mut State) {
        trim_learned(state, self.personal.now());
        self.learned_changed(state);
    }

    fn learned_changed(&self, state: &mut State) {
        state.learned_syllables = state
            .learned
            .keys()
            .flat_map(|k| k.reading.split(' ').filter(|p| !p.is_empty()).map(str::to_owned).collect::<Vec<_>>())
            .collect();
        state.syllables.set_learned(state.learned_syllables.iter().cloned());
        state.alternative_cache.clear();
        state.entry_cache.clear();
        state.match_cache.clear();
        state.revision = state.revision.wrapping_add(1);
        self.schedule_write(state);
    }

    fn load_personal_data(&self, state: &mut State) {
        let Some(directory) = &self.directory else { return };
        if let Ok(data) = std::fs::read(directory.join("user_phrases.tsv")) {
            scan_tsv(&data, false, |fields| {
                if fields.len() == 2 && valid_phrase(&fields[1], &fields[0]) {
                    self.add_phrase(state, &fields[1], &fields[0]);
                }
            });
        }
        if let Ok(data) = std::fs::read(directory.join("typos.tsv")) {
            scan_tsv(&data, false, |fields| {
                if fields.len() != 3 || grapheme_count(&fields[0]) != 1 || grapheme_count(&fields[1]) != 1 {
                    return;
                }
                if let Some(n) = parse_double(&fields[2]).filter(|n| *n > 0.0) {
                    state.typo_counts.insert(format!("{}{}", fields[0], fields[1]), n);
                }
            });
        }
        if let Ok(data) = std::fs::read(directory.join("excluded.tsv")) {
            scan_tsv(&data, false, |fields| {
                if fields.len() == 2 {
                    state.excluded.insert(PhraseKey::new(fields[0].as_ref(), fields[1].as_ref()));
                }
            });
        }
        if let Ok(data) = std::fs::read(directory.join("learned.tsv")) {
            scan_tsv(&data, false, |fields| {
                if fields.len() == 4 {
                    // Earlier format: reading, word, count, last used.  Kept as whole-composition recalls.
                    if !valid_phrase(&fields[1], &fields[0]) {
                        return;
                    }
                    let (Some(count), Some(date)) = (parse_double(&fields[2]), parse_double(&fields[3])) else {
                        return;
                    };
                    if !(count.is_finite() && count > 0.0 && date.is_finite() && date >= 0.0) {
                        return;
                    }
                    let mut value = RecallValue::new(RecallKind::Whole);
                    value.long = DecayingCount::new(count, date);
                    value.picks = count.floor() as i64;
                    state.learned.insert(PhraseKey::new(fields[0].as_ref(), fields[1].as_ref()), value);
                    state.learned_words.entry(fields[0].to_string()).or_default().insert(fields[1].to_string());
                    return;
                }
                let Some((key, value)) = learned_row(fields) else { return };
                state.learned_words.entry(key.reading.clone()).or_default().insert(key.word.clone());
                state.learned.insert(key, value);
            });
        }
        if let Ok(data) = std::fs::read(directory.join(PersonalLanguageModel::FILE_NAME)) {
            self.personal.load(&data, false, |word| self.resolve_id(state, word));
        }
        state.learned_syllables = state
            .learned
            .keys()
            .flat_map(|k| k.reading.split(' ').filter(|p| !p.is_empty()).map(str::to_owned).collect::<Vec<_>>())
            .collect();
        trim_learned(state, self.personal.now());
    }

    /// Writes pending personal data now: before another engine reads the same directory, or before exiting.
    pub fn flush_personal_data(&self) {
        let (done, wait) = mpsc::channel();
        self.writer.send(Job::Flush(done));
        let _ = wait.recv();
    }

    fn schedule_write(&self, state: &State) {
        let Some(directory) = &self.directory else { return };
        let snapshot = Snapshot {
            phrases: phrase_text(state.phrases.keys()),
            learned: learned_text(&state.learned),
            excluded: phrase_text(state.excluded.iter()),
            typos: typo_text(&state.typo_counts),
            personal: self.personal.serialized(),
        };
        self.writer.send(Job::Write(directory.clone(), snapshot));
    }
}

/// Swift's `<` and `==` on doubles as an ordering (−0 equals 0; NaN never occurs here).
fn swift_order(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Lines split at LF, except that CR LF is one character in Swift and does not split.
fn swift_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' && !(index > 0 && bytes[index - 1] == b'\r') {
            lines.push(&text[start..index]);
            start = index + 1;
        }
    }
    lines.push(&text[start..]);
    lines
}

fn add_base_prefixes(prefixes: &mut HashSet<String>, base: &str) {
    let mut prefix = String::new();
    for part in base.split(' ').filter(|p| !p.is_empty()) {
        if !prefix.is_empty() {
            prefix.push(' ');
        }
        prefix.push_str(part);
        prefixes.insert(prefix.clone());
    }
}

fn remember(state: &mut State, key: &PhraseKey, kind: RecallKind, now: f64) {
    let value = state.learned.entry(key.clone()).or_insert_with(|| RecallValue::new(kind));
    value.short.add(1.0, now, RecallValue::SHORT_HALF_LIFE);
    value.long.add(1.0, now, RecallValue::LONG_HALF_LIFE);
    value.picks += 1;
    state.learned_words.entry(key.reading.clone()).or_default().insert(key.word.clone());
    for part in key.reading.split(' ').filter(|p| !p.is_empty()) {
        if !state.syllables.contains(part) {
            state.learned_syllables.insert(part.to_owned());
        }
    }
    add_base_prefixes(&mut state.reading_prefixes, &without_tones(&key.reading));
}

fn trim_learned(state: &mut State, now: f64) {
    if state.learned.len() <= 20_000 {
        return;
    }
    let mut ranked: Vec<(PhraseKey, RecallValue, f64)> =
        state.learned.iter().map(|(k, v)| (k.clone(), *v, v.value(now))).collect();
    ranked.sort_by(|a, b| {
        if a.2 == b.2 {
            a.0.cmp(&b.0)
        } else if a.2 > b.2 {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
    ranked.truncate(20_000);
    state.learned = ranked.into_iter().map(|(k, v, _)| (k, v)).collect();
    state.learned_words.clear();
    for key in state.learned.keys() {
        state.learned_words.entry(key.reading.clone()).or_default().insert(key.word.clone());
    }
}

/// A user phrase needs 1–6 characters without whitespace and one valid Zhuyin syllable per character.
#[must_use]
pub fn valid_phrase(word: &str, reading: &str) -> bool {
    if word.is_empty() || word.chars().any(char::is_whitespace) {
        return false;
    }
    let syllables: Vec<&str> = reading.split(' ').collect();
    if !(1..=6).contains(&syllables.len()) || grapheme_count(word) != syllables.len() {
        return false;
    }
    syllables.iter().all(|syllable| {
        let mut slots = HashSet::new();
        let mut last_slot: Option<usize> = None;
        let mut saw_tone = false;
        let last = syllable.chars().last();
        for symbol in syllable.chars() {
            if keyboard::is_tone_mark(symbol) {
                if slots.is_empty() || saw_tone || Some(symbol) != last {
                    return false;
                }
                saw_tone = true;
            } else {
                let Some(slot) = keyboard::slot(symbol) else { return false };
                if saw_tone || last_slot.is_some_and(|last| slot <= last) || !slots.insert(slot) {
                    return false;
                }
                last_slot = Some(slot);
            }
        }
        !slots.is_empty()
    })
}

/// The readings Swift's fuzzy matching adds for `reading` under `fuzzy`; invalid ones are pruned by the prefixes.
#[must_use]
pub fn fuzzy_alternatives(reading: &str, fuzzy: FuzzySounds) -> Vec<ReadingAlternative> {
    if fuzzy.is_empty() {
        return vec![];
    }
    let mut variants: Vec<(Vec<char>, i32)> = vec![(reading.chars().collect(), 0)];
    for pair in FuzzySounds::PAIRS {
        if !fuzzy.contains(pair.set) {
            continue;
        }
        let snapshot = variants.clone();
        for (symbols, swaps) in snapshot {
            let index = if pair.initial {
                (!symbols.is_empty()).then_some(0)
            } else {
                symbols.iter().rposition(|&c| !keyboard::is_tone_mark(c))
            };
            let Some(index) = index else { continue };
            if symbols[index] != pair.a && symbols[index] != pair.b {
                continue;
            }
            let mut swapped = symbols.clone();
            swapped[index] = if symbols[index] == pair.a { pair.b } else { pair.a };
            variants.push((swapped, swaps + 1));
        }
    }
    variants
        .into_iter()
        .skip(1)
        .map(|(symbols, swaps)| ReadingAlternative {
            reading: symbols.into_iter().collect(),
            cost: f64::from(swaps) * FUZZY_COST,
            fuzzy: true,
        })
        .collect()
}

fn phrase_text<'a>(keys: impl Iterator<Item = &'a PhraseKey>) -> String {
    let mut keys: Vec<&PhraseKey> = keys.collect();
    keys.sort();
    keys.iter().fold(String::new(), |mut text, k| {
        let _ = writeln!(text, "{}\t{}", k.reading, k.word);
        text
    })
}

fn typo_text(counts: &HashMap<String, f64>) -> String {
    let mut rows: Vec<(&String, &f64)> = counts.iter().collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    rows.iter().fold(String::new(), |mut text, (key, n)| {
        let clusters = graphemes(key);
        let first = clusters.first().unwrap_or(&"");
        let last = clusters.last().unwrap_or(&"");
        let _ = writeln!(text, "{first}\t{last}\t{}", format_double(**n));
        text
    })
}

fn learned_text(learned: &HashMap<PhraseKey, RecallValue>) -> String {
    let mut rows: Vec<(&PhraseKey, &RecallValue)> = learned.iter().collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    rows.iter()
        .map(|(key, value)| {
            [
                value.kind.raw_value().to_owned(),
                key.reading.clone(),
                key.word.clone(),
                format_double(value.short.value),
                format_double(value.short.time),
                format_double(value.long.value),
                format_double(value.long.time),
                value.picks.to_string(),
            ]
            .join("\t")
                + "\n"
        })
        .collect()
}

/// One row of learned.tsv: kind, reading, word, short value and time, long value and time, picks.
fn learned_row(fields: &[std::borrow::Cow<'_, str>]) -> Option<(PhraseKey, RecallValue)> {
    if fields.len() != 8 {
        return None;
    }
    let kind = RecallKind::from_raw(&fields[0])?;
    if !valid_phrase(&fields[2], &fields[1]) {
        return None;
    }
    let short_value = parse_double(&fields[3])?;
    let short_time = parse_double(&fields[4])?;
    let long_value = parse_double(&fields[5])?;
    let long_time = parse_double(&fields[6])?;
    let picks = parse_int(&fields[7])?;
    if ![short_value, short_time, long_value, long_time].iter().all(|v| v.is_finite() && *v >= 0.0) {
        return None;
    }
    let mut value = RecallValue::new(kind);
    value.short = DecayingCount::new(short_value, short_time);
    value.long = DecayingCount::new(long_value, long_time);
    value.picks = picks.max(0);
    Some((PhraseKey::new(fields[1].as_ref(), fields[2].as_ref()), value))
}

//! The user's own word statistics (port of `PersonalLanguageModel.swift`).
//!
//! Every commit adds its word sequence to unigram, bigram and trigram counts in two layers: a short-term layer that
//! forgets within a day and a long-term layer that forgets over months (it grows at most once per day per n-gram
//! unless the user picked the words, and n-grams picked three times keep a floor).  Scoring shrinks the base
//! probability towards the personal estimate one order at a time:
//!
//! ```text
//! p ← (min(c(h, w), C(h)) + κₙ · p) / (C(h) + κₙ)        for n = 1, 2, 3
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::language_model::{LanguageModel, LanguageModelHistory, NO_CONTEXT};
use crate::lexicon::{swift_max, swift_min};
use crate::swift_format::format_double;
use crate::tsv::{parse_double, parse_int, scan_tsv};

/// A count that halves every `half_life` seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DecayingCount {
    /// The count at `time`.
    pub value: f64,
    /// When `value` was last set, seconds since 1970.
    pub time: f64,
}

impl DecayingCount {
    /// A count with the given value and time.
    #[must_use]
    pub fn new(value: f64, time: f64) -> Self {
        Self { value, time }
    }

    /// The count at `now`.
    #[must_use]
    pub fn at(&self, now: f64, half_life: f64) -> f64 {
        // Swift writes pow(0.5, t); its optimizer (LLVM) turns that into exp2(−t), which the shipped engine computes.
        // Calling exp2 here gives those results at every optimization level.
        if self.value == 0.0 { 0.0 } else { self.value * (-(swift_max(0.0, now - self.time) / half_life)).exp2() }
    }

    /// Decays to `now`, then adds `amount`.
    pub fn add(&mut self, amount: f64, now: f64, half_life: f64) {
        self.value = self.at(now, half_life) + amount;
        self.time = now;
    }

    /// Decays to `now`, then multiplies by `factor`.
    pub fn scale(&mut self, factor: f64, now: f64, half_life: f64) {
        self.value = self.at(now, half_life) * factor;
        self.time = now;
    }
}

/// The model's constants.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Weight of a picked word on the unigram, bigram and trigram that end in or contain it.
    pub pick_weights: [f64; 3],
    /// Weight of text accepted without picking, on every order.
    pub passive_weight: f64,
    /// Prior strength per order (unigram, bigram, trigram).
    pub kappa: [f64; 3],
    /// Half-life of the short-term layer, seconds.
    pub short_half_life: f64,
    /// Half-life of the long-term layer, seconds.
    pub long_half_life: f64,
    /// Picks after which an n-gram keeps `floor_value`.
    pub floor_picks: i64,
    /// The long-term floor of n-grams picked `floor_picks` times.
    pub floor_value: f64,
    /// Most n-grams kept.
    pub capacity: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            pick_weights: [0.1, 0.3, 1.0],
            passive_weight: 0.05,
            kappa: [1000.0, 100.0, 1.0],
            short_half_life: 8.0 * 3600.0,
            long_half_life: 90.0 * 86400.0,
            floor_picks: 3,
            floor_value: 1.0,
            capacity: 50_000,
        }
    }
}

/// An n-gram's word IDs.
pub type Key = Vec<u32>;

/// One n-gram's statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The words' texts, as first learned.
    pub texts: Vec<String>,
    /// The words' readings, as first learned.
    pub readings: Vec<String>,
    /// Short-term count.
    pub short: DecayingCount,
    /// Long-term count.
    pub long: DecayingCount,
    /// How often the last word was picked here.
    pub picks: i64,
    /// The first day it was learned (days since 1970), `i64::MAX` before.
    pub first_day: i64,
    /// The day of the last long-term growth from passive text.
    pub long_day: i64,
    /// How much the long-term count grew on `long_day`.
    pub long_day_amount: f64,
}

impl Entry {
    fn new(texts: Vec<String>, readings: Vec<String>) -> Self {
        Self {
            texts,
            readings,
            short: DecayingCount::default(),
            long: DecayingCount::default(),
            picks: 0,
            first_day: i64::MAX,
            long_day: -1,
            long_day_amount: 0.0,
        }
    }
}

/// One committed word, as learning sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    /// Its ID.
    pub id: u32,
    /// Its text.
    pub text: String,
    /// Its reading.
    pub reading: String,
    /// The user picked it.
    pub picked: bool,
}

/// A row of the remembered-words list.
#[derive(Clone, Debug, PartialEq)]
pub struct PickedWord {
    /// The word's reading.
    pub reading: String,
    /// The word.
    pub word: String,
    /// Its strength now.
    pub count: f64,
}

type Clock = Arc<dyn Fn() -> f64 + Send + Sync>;

struct State {
    options: Options,
    clock: Clock,
    entries: HashMap<Key, Entry>,
    totals: HashMap<Key, (DecayingCount, DecayingCount)>,
}

/// The user's word statistics.  Shared between threads; one lock guards everything.
pub struct PersonalLanguageModel {
    state: Mutex<State>,
    /// Whether anything has been learned; read without the lock on every decoding step.
    has_data: AtomicBool,
}

impl Default for PersonalLanguageModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Seconds since 1970, now.
fn system_time() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

fn day(time: f64) -> i64 {
    (time / 86400.0).floor() as i64
}

fn value(options: &Options, entry: &Entry, now: f64) -> f64 {
    let long = entry.long.at(now, options.long_half_life);
    entry.short.at(now, options.short_half_life)
        + if entry.picks >= options.floor_picks { swift_max(long, options.floor_value) } else { long }
}

/// `a` precedes `b` word by word, a prefix first: the tie-break wherever order matters.
fn key_precedes(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    a.cmp(b)
}

impl PersonalLanguageModel {
    /// An empty model on the system clock.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                options: Options::default(),
                clock: Arc::new(system_time),
                entries: HashMap::new(),
                totals: HashMap::new(),
            }),
            has_data: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The time memories decay by (seconds since 1970).
    #[must_use]
    pub fn now(&self) -> f64 {
        let clock = self.lock().clock.clone();
        clock()
    }

    /// Replaces the clock (for tests and reproducible runs).
    pub fn set_clock(&self, clock: impl Fn() -> f64 + Send + Sync + 'static) {
        self.lock().clock = Arc::new(clock);
    }

    /// The model's constants.
    #[must_use]
    pub fn options(&self) -> Options {
        self.lock().options.clone()
    }

    /// Replaces the model's constants.
    pub fn set_options(&self, options: Options) {
        self.lock().options = options;
    }

    /// Whether anything has been learned.
    #[must_use]
    pub fn has_data(&self) -> bool {
        self.has_data.load(Ordering::Relaxed)
    }

    /// Whether nothing has been learned (under the lock).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().entries.is_empty()
    }

    /// Number of n-grams.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    /// The base log probability of `word` after `history`, adapted to the user.
    #[must_use]
    pub fn adapt(&self, base: f64, word: Option<u32>, history: LanguageModelHistory) -> f64 {
        let Some(word) = word.filter(|&w| w > 2) else { return base };
        let state = self.lock();
        if state.entries.is_empty() {
            return base;
        }
        let now = (state.clock)();
        let options = &state.options;
        let mut p = base.exp();
        let mut changed = false;
        let usable = |id: u32| id > 2 && id != NO_CONTEXT;
        let mut contexts: Vec<Vec<u32>> = vec![vec![]];
        if usable(history.previous) {
            contexts.push(vec![history.previous]);
            if let Some(older) = history.older.filter(|&o| usable(o)) {
                contexts.push(vec![older, history.previous]);
            }
        }
        for (order, context) in contexts.into_iter().enumerate() {
            let count = match state.totals.get(&context) {
                Some((short, long)) => short.at(now, options.short_half_life) + long.at(now, options.long_half_life),
                None => 0.0,
            };
            if count.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                continue;
            }
            let mut gram = context;
            gram.push(word);
            let observed = state.entries.get(&gram).map_or(0.0, |entry| value(options, entry, now));
            let kappa = options.kappa[order];
            p = (swift_min(observed, count) + kappa * p) / (count + kappa);
            changed = true;
        }
        if changed { swift_max(p, 5e-324).ln() } else { base }
    }

    /// Learns one committed stretch of words (no literals inside).  The first `context` words were committed
    /// earlier: they are history only, so n-grams ending in them are not counted again.
    pub fn learn(&self, words: &[Word], context: usize) {
        if words.len() <= context {
            return;
        }
        let mut state = self.lock();
        let now = (state.clock)();
        for end in context..words.len() {
            for order in 1..=3usize {
                if end + 1 < order {
                    continue;
                }
                let gram = &words[end + 1 - order..=end];
                let picked = gram.iter().any(|w| w.picked);
                let weight = if picked { state.options.pick_weights[order - 1] } else { state.options.passive_weight };
                add(&mut state, gram, weight, picked && gram.last().is_some_and(|w| w.picked), now);
            }
        }
        trim(&mut state, now);
        self.has_data.store(!state.entries.is_empty(), Ordering::Relaxed);
    }

    /// The user picked `word` after the previous word of `history`, or, for a phrase, anywhere.
    #[must_use]
    pub fn remembers(&self, word: u32, history: LanguageModelHistory, phrase: bool) -> bool {
        let state = self.lock();
        if history.previous > 2
            && history.previous != NO_CONTEXT
            && state.entries.get(&vec![history.previous, word]).is_some_and(|e| e.picks > 0)
        {
            return true;
        }
        phrase && state.entries.get(&vec![word]).is_some_and(|e| e.picks > 0)
    }

    /// A pick replaced `word` after `history`: halve the user's evidence for it there.
    pub fn weaken(&self, word: u32, history: LanguageModelHistory) {
        let mut state = self.lock();
        let now = (state.clock)();
        let mut keys = vec![vec![history.previous, word]];
        if let Some(older) = history.older {
            keys.push(vec![older, history.previous, word]);
        }
        let (short, long) = (state.options.short_half_life, state.options.long_half_life);
        for key in keys {
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.short.scale(0.5, now, short);
                entry.long.scale(0.5, now, long);
                entry.picks = 0;
            }
        }
        rebuild_totals(&mut state);
    }

    /// Removes every n-gram containing `id`.
    pub fn forget(&self, id: u32) {
        let mut state = self.lock();
        state.entries.retain(|key, _| !key.contains(&id));
        rebuild_totals(&mut state);
        self.has_data.store(!state.entries.is_empty(), Ordering::Relaxed);
    }

    /// Forgets everything.
    pub fn forget_all(&self) {
        let mut state = self.lock();
        state.entries.clear();
        state.totals.clear();
        self.has_data.store(false, Ordering::Relaxed);
    }

    /// Picked single words, for the remembered-words list (in no particular order).
    #[must_use]
    pub fn picked_words(&self) -> Vec<PickedWord> {
        let state = self.lock();
        let now = (state.clock)();
        state
            .entries
            .iter()
            .filter(|(key, entry)| key.len() == 1 && entry.picks > 0)
            .map(|(_, entry)| PickedWord {
                reading: entry.readings[0].clone(),
                word: entry.texts[0].clone(),
                count: value(&state.options, entry, now),
            })
            .collect()
    }

    /// The file name of the serialized model.
    pub const FILE_NAME: &'static str = "personal_ngrams.tsv";

    /// One line per n-gram: texts and readings (joined by U+001F), short value and time, long value and time, picks,
    /// first day (-1 for none), long day, long-day amount.  Sorted by texts, then by key.
    #[must_use]
    pub fn serialized(&self) -> String {
        let state = self.lock();
        let mut rows: Vec<(&Key, &Entry)> = state.entries.iter().collect();
        rows.sort_by(|a, b| compare_texts(&a.1.texts, &b.1.texts).then_with(|| key_precedes(a.0, b.0)));
        let mut text = String::new();
        for (_, entry) in rows {
            let fields = [
                entry.texts.join("\u{1F}"),
                entry.readings.join("\u{1F}"),
                format_double(entry.short.value),
                format_double(entry.short.time),
                format_double(entry.long.value),
                format_double(entry.long.time),
                entry.picks.to_string(),
                (if entry.first_day == i64::MAX { -1 } else { entry.first_day }).to_string(),
                entry.long_day.to_string(),
                format_double(entry.long_day_amount),
            ];
            text.push_str(&fields.join("\t"));
            text.push('\n');
        }
        text
    }

    /// Loads rows written by [`serialized`](Self::serialized), resolving words to IDs.  Merging keeps an n-gram
    /// already here when its long-term count is the larger one.  Returns the rows taken.
    pub fn load(&self, data: &[u8], merging: bool, mut resolve: impl FnMut(&str) -> u32) -> usize {
        let mut state = self.lock();
        let mut taken = 0;
        let now = (state.clock)();
        let long_half_life = state.options.long_half_life;
        scan_tsv(data, false, |fields| {
            if fields.len() != 10 {
                return;
            }
            let texts: Vec<String> = fields[0].split('\u{1F}').map(str::to_owned).collect();
            let readings: Vec<String> = fields[1].split('\u{1F}').map(str::to_owned).collect();
            if !(1..=3).contains(&texts.len()) || texts.len() != readings.len() || texts.iter().any(String::is_empty) {
                return;
            }
            let (Some(short_value), Some(short_time), Some(long_value), Some(long_time)) = (
                parse_double(&fields[2]),
                parse_double(&fields[3]),
                parse_double(&fields[4]),
                parse_double(&fields[5]),
            ) else {
                return;
            };
            let (Some(picks), Some(first_day), Some(long_day), Some(long_day_amount)) =
                (parse_int(&fields[6]), parse_int(&fields[7]), parse_int(&fields[8]), parse_double(&fields[9]))
            else {
                return;
            };
            if ![short_value, short_time, long_value, long_time, long_day_amount]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
            {
                return;
            }
            let key: Key = texts.iter().map(|t| resolve(t)).collect();
            let mut entry = Entry::new(texts, readings);
            entry.short = DecayingCount::new(short_value, short_time);
            entry.long = DecayingCount::new(long_value, long_time);
            entry.picks = picks.max(0);
            entry.first_day = if first_day < 0 { i64::MAX } else { first_day };
            entry.long_day = long_day;
            entry.long_day_amount = long_day_amount;
            if merging
                && let Some(old) = state.entries.get(&key)
                && old.long.at(now, long_half_life) >= entry.long.at(now, long_half_life)
            {
                return;
            }
            state.entries.insert(key, entry);
            taken += 1;
        });
        trim(&mut state, now);
        rebuild_totals(&mut state);
        self.has_data.store(!state.entries.is_empty(), Ordering::Relaxed);
        taken
    }
}

/// Swift's `[String].lexicographicallyPrecedes` with String `<` (UTF-8 order for the engine's normalized text).
fn compare_texts(a: &[String], b: &[String]) -> std::cmp::Ordering {
    a.iter().map(String::as_str).cmp(b.iter().map(String::as_str))
}

fn add(state: &mut State, gram: &[Word], weight: f64, picked: bool, now: f64) {
    let key: Key = gram.iter().map(|w| w.id).collect();
    let options = state.options.clone();
    let entry = state.entries.entry(key.clone()).or_insert_with(|| {
        Entry::new(gram.iter().map(|w| w.text.clone()).collect(), gram.iter().map(|w| w.reading.clone()).collect())
    });
    let today = day(now);
    entry.short.add(weight, now, options.short_half_life);
    let mut long_amount = 0.0;
    if picked || weight > options.passive_weight {
        long_amount = weight;
        if picked {
            entry.picks += 1;
        }
    } else if entry.first_day < today {
        // Habits: text accepted on an earlier day as well, at most one unit per day.
        if entry.long_day != today {
            entry.long_day = today;
            entry.long_day_amount = 0.0;
        }
        long_amount = swift_min(weight, 1.0 - entry.long_day_amount);
        entry.long_day_amount += long_amount;
    }
    if long_amount > 0.0 {
        entry.long.add(long_amount, now, options.long_half_life);
    }
    entry.first_day = entry.first_day.min(today);
    let history: Key = key[..key.len() - 1].to_vec();
    let total = state.totals.entry(history).or_default();
    total.0.add(weight, now, options.short_half_life);
    if long_amount > 0.0 {
        total.1.add(long_amount, now, options.long_half_life);
    }
}

/// Keeps the strongest `capacity` n-grams; equal strengths by key.
fn trim(state: &mut State, now: f64) {
    if state.entries.len() <= state.options.capacity {
        return;
    }
    let mut ranked: Vec<(f64, Key)> =
        state.entries.iter().map(|(key, entry)| (value(&state.options, entry, now), key.clone())).collect();
    ranked.sort_by(|a, b| {
        if a.0 == b.0 {
            key_precedes(&a.1, &b.1)
        } else if a.0 > b.0 {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
    let keep: std::collections::HashSet<Key> = ranked.into_iter().take(state.options.capacity).map(|r| r.1).collect();
    state.entries.retain(|key, _| keep.contains(key));
    rebuild_totals(state);
}

/// Each history's total is summed in ascending order of its n-grams' values, so the sum does not depend on the
/// table's order (the Swift engine sums the same way).
fn rebuild_totals(state: &mut State) {
    let now = (state.clock)();
    let options = &state.options;
    let mut parts: HashMap<Key, (Vec<f64>, Vec<f64>)> = HashMap::with_capacity(state.entries.len());
    for (key, entry) in &state.entries {
        let part = parts.entry(key[..key.len() - 1].to_vec()).or_default();
        part.0.push(entry.short.at(now, options.short_half_life));
        part.1.push(entry.long.at(now, options.long_half_life));
    }
    state.totals.clear();
    for (history, (mut shorts, mut longs)) in parts {
        shorts.sort_by(f64::total_cmp);
        longs.sort_by(f64::total_cmp);
        let mut total = (DecayingCount::default(), DecayingCount::default());
        for value in shorts {
            total.0.add(value, now, options.short_half_life);
        }
        for value in longs {
            total.1.add(value, now, options.long_half_life);
        }
        state.totals.insert(history, total);
    }
}

/// The base model adapted by the user's statistics.  Once anything is learned it keeps two words of history, so
/// personal trigrams apply even over a unigram or bigram base; with nothing learned it is exactly the base model.
pub struct PersonalizedLanguageModel<M> {
    /// The model being adapted.
    pub base: M,
    /// The user's statistics.
    pub personal: Arc<PersonalLanguageModel>,
}

impl<M: LanguageModel> LanguageModel for PersonalizedLanguageModel<M> {
    fn order(&self) -> usize {
        if self.personal.has_data() { 3 } else { self.base.order() }
    }
    fn word_id(&self, word: &str) -> Option<u32> {
        self.base.word_id(word)
    }
    fn score(&self, word_id: Option<u32>, history: LanguageModelHistory, unigram: f64) -> f64 {
        self.personal.adapt(self.base.score(word_id, history, unigram), word_id, history)
    }
}

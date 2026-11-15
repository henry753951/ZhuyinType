//! 使用統計 (macOS `IMK/UsageStats.swift`, Qt `DesktopServices`): counts only — characters, compositions, keys, picks
//! and corrections, per-key latency as a histogram, the typing rhythm (key intervals, thinking pauses, the longest
//! runs, characters by weekday and hour) — by local day for 60 days and in total.  Never the text, the readings,
//! which keys or the app.  Kept in memory and written to `stats.json` (the macOS schema) at most every 10 minutes,
//! when the settings window asks and before the process exits.
//!
//! The input controller reports what happened as [`UsageEvent`]s (`IPC.md` §使用統計).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike as _, Days, Local, NaiveDate, Timelike as _, Utc};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};

/// Which list a pick came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickList {
    Normal,
    Corrections,
    Single,
    Punctuation,
}

/// One counted event (macOS `UsageStatsObserver`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UsageEvent {
    /// A key the input method saw; `composing` when it was handled while composing (counted, latency kept).
    Key {
        latency_ms: f64,
        composing: bool,
    },
    /// A key interval inside a burst of typing, measured by the controller.
    Interval {
        ms: f64,
    },
    /// A thinking pause (over the pause threshold, under a minute), measured by the controller.
    Pause {
        ms: f64,
    },
    /// Text sent to the app.
    Commit {
        characters: i64,
        composition_ended: bool,
        /// The composition needed no correction (`None`: the store decides from the events it saw).
        clean: Option<bool>,
        corrected_shown: bool,
        corrected_committed: bool,
    },
    Pick(PickList),
    Backspace,
    Escape,
    DeleteAfterCommit,
    LiteralZhuyin,
    UndoLiteral,
    PhraseAdded,
    /// Reserved for platforms with a reranker.
    CorrectionShown,
    /// Reserved for platforms with a reranker.
    CorrectionCommitted,
    /// Secure input or a switched-away input: forget the current burst.
    Reset,
}

/// The counters of `stats.json`, by their schema names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub characters: i64,
    pub compositions: i64,
    pub clean_compositions: i64,
    pub keys: i64,
    pub picks_normal: i64,
    pub picks_corrections: i64,
    pub picks_single: i64,
    pub picks_punctuation: i64,
    pub backspaces: i64,
    pub escapes: i64,
    pub delete_after_commit: i64,
    pub literal_zhuyin: i64,
    pub undo_literal: i64,
    pub corrected_shown: i64,
    pub corrected_committed: i64,
    pub phrases_added: i64,
    pub typing_milliseconds: i64,
    pub thinking_milliseconds: i64,
}

impl Counters {
    /// Every pick.
    #[must_use]
    pub fn picks(&self) -> i64 {
        self.picks_normal + self.picks_corrections + self.picks_single + self.picks_punctuation
    }

    /// Every correction: picks, backspaces, escapes, deletions after a commit and literal Zhuyin.
    #[must_use]
    pub fn corrections(&self) -> i64 {
        self.picks() + self.backspaces + self.escapes + self.delete_after_commit + self.literal_zhuyin
    }
}

/// The period the 分析 page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Period {
    /// The last 30 days.
    #[default]
    Month,
    /// Since the statistics started.
    All,
}

/// Characters typed on one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayCount {
    pub date: NaiveDate,
    pub characters: i64,
}

/// Per-key latency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LatencySummary {
    pub total: i64,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub maximum: Option<f64>,
    /// Histogram counts, one more than `bounds` (the last is above the last bound).
    pub counts: Vec<i64>,
    /// Upper bounds of the buckets, in ms.
    pub bounds: Vec<f64>,
}

/// The typing rhythm.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RhythmSummary {
    pub longest_run: i64,
    pub longest_clean_run: i64,
    /// Seconds: a gap longer than this is a thinking pause.
    pub pause_threshold: f64,
    pub pause_median: Option<f64>,
    /// Characters by weekday (Sunday first) and hour: 7 × 24 values.
    pub hours: Vec<i64>,
}

/// What the 分析 page shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Usage {
    /// The stored statistics could be read (false: counting waits for 清除統計).
    pub valid: bool,
    /// The day counting started.
    pub since: NaiveDate,
    /// The last 30 days, oldest first.
    pub days: Vec<DayCount>,
    /// The counters for the period.
    pub counters: Counters,
    /// Characters per minute of active typing (with at least a second of it).
    pub active_speed: Option<f64>,
    /// Compositions committed without a correction, as a share.
    pub first_time_right: Option<f64>,
    /// Consecutive days with typing up to today (or yesterday while today has none).
    pub streak: u32,
    pub best_day: Option<DayCount>,
    pub latency: LatencySummary,
    pub rhythm: RhythmSummary,
}

/// The process's own use of the machine (the 分析 page's 效能 rows).
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessMetrics {
    pub launched: DateTime<Local>,
    pub running_seconds: f64,
    pub cpu_seconds: Option<f64>,
    pub memory_bytes: Option<f64>,
    pub peak_memory_bytes: Option<f64>,
}

/// Days kept.
pub const KEPT_DAYS: usize = 60;
/// The longest time between writes, in ms.
pub const SAVE_INTERVAL_MS: i64 = 10 * 60 * 1000;
const SCHEMA: i64 = 2;
const MAX_STATS_BYTES: u64 = 4 * 1024 * 1024;
/// A gap longer than this is time away, not thinking (`UsageStatsObserver.awayAfter`).
const AWAY_AFTER_SECONDS: f64 = 60.0;
const LATENCY_BOUNDS: &[f64] =
    &[1., 2., 3., 4., 5., 6., 8., 10., 12., 15., 20., 25., 30., 40., 50., 75., 100., 150., 250., 500., 1000.];
const INTERVAL_BOUNDS: &[f64] = &[
    0.04, 0.06, 0.08, 0.1, 0.12, 0.15, 0.18, 0.22, 0.26, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.3, 1.6, 2.0, 3.0, 4.0,
    6.0,
];
const PAUSE_BOUNDS: &[f64] = &[2., 3., 4., 5., 6., 8., 10., 13., 16., 20., 25., 30., 40., 50., 60.];

/// The counters by their `stats.json` names.
macro_rules! fields {
    ($($name:literal => $field:ident),* $(,)?) => {
        impl Counters {
            fn fields_mut(&mut self) -> [(&'static str, &mut i64); 18] { [$(($name, &mut self.$field)),*] }
            fn fields(&self) -> [(&'static str, i64); 18] { [$(($name, self.$field)),*] }
        }
    };
}
fields! {
    "characters" => characters, "compositions" => compositions, "cleanCompositions" => clean_compositions,
    "keys" => keys, "picksNormal" => picks_normal, "picksCorrections" => picks_corrections,
    "picksSingle" => picks_single, "picksPunctuation" => picks_punctuation, "backspaces" => backspaces,
    "escapes" => escapes, "deleteAfterCommit" => delete_after_commit, "literalZhuyin" => literal_zhuyin,
    "undoLiteral" => undo_literal, "correctedShown" => corrected_shown, "correctedCommitted" => corrected_committed,
    "phrasesAdded" => phrases_added, "typingMilliseconds" => typing_milliseconds,
    "thinkingMilliseconds" => thinking_milliseconds,
}

impl Counters {
    fn add(&mut self, other: &Counters) {
        for ((_, mine), (_, theirs)) in self.fields_mut().into_iter().zip(other.fields()) {
            *mine += theirs;
        }
    }

    fn is_zero(&self) -> bool {
        self.fields().iter().all(|(_, value)| *value == 0)
    }

    /// A named counter (`characters`, `picksNormal`, …).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<i64> {
        self.fields().into_iter().find(|(field, _)| *field == name).map(|(_, value)| value)
    }

    fn decode(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let mut counters = Self::default();
        for (name, field) in counters.fields_mut() {
            *field = count(object.get(name))?;
        }
        Some(counters)
    }

    fn encode(&self) -> Value {
        Value::Object(self.fields().into_iter().map(|(name, value)| (name.to_owned(), json!(value))).collect())
    }
}

/// A stored count: a whole, non-negative number; a missing field (added later) is zero.
fn count(value: Option<&Value>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => Some(0),
        Some(value) => {
            let number = value.as_f64()?;
            (number.is_finite() && (0.0..=9_007_199_254_740_991.0).contains(&number) && number.fract() == 0.0)
                .then_some(number as i64)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Buckets {
    bounds: &'static [f64],
    counts: Vec<i64>,
    maximum: f64,
}

impl Buckets {
    fn new(bounds: &'static [f64]) -> Self {
        Self { bounds, counts: vec![0; bounds.len() + 1], maximum: 0.0 }
    }

    fn total(&self) -> i64 {
        self.counts.iter().sum()
    }

    fn add(&mut self, value: f64) {
        let index = self.bounds.iter().take_while(|bound| value > **bound).count();
        self.counts[index] += 1;
        if value > self.maximum {
            self.maximum = value;
        }
    }

    /// Interpolated inside its bucket; `None` when empty.
    fn percentile(&self, fraction: f64) -> Option<f64> {
        let all = self.total();
        if all <= 0 {
            return None;
        }
        let target = fraction * all as f64;
        let mut seen = 0.0;
        for (index, &value) in self.counts.iter().enumerate() {
            if value <= 0 {
                continue;
            }
            if seen + value as f64 >= target {
                let low = if index == 0 { 0.0 } else { self.bounds[index - 1] };
                let high = if index < self.bounds.len() { self.bounds[index] } else { self.maximum.max(low) };
                return Some(self.maximum.min(low + (high - low) * (target - seen) / value as f64));
            }
            seen += value as f64;
        }
        Some(self.maximum)
    }

    fn decode(value: Option<&Value>, bounds: &'static [f64]) -> Option<Self> {
        let mut buckets = Self::new(bounds);
        let Some(value) = value else { return Some(buckets) };
        let object = value.as_object()?;
        let counts = object.get("counts")?.as_array()?;
        if counts.len() != bounds.len() + 1 {
            return None;
        }
        for (slot, value) in buckets.counts.iter_mut().zip(counts) {
            *slot = count(Some(value))?;
        }
        let maximum = object.get("maximum").and_then(Value::as_f64).unwrap_or(0.0);
        buckets.maximum = if maximum.is_finite() && maximum >= 0.0 { maximum } else { 0.0 };
        Some(buckets)
    }

    fn encode(&self, with_bounds: bool) -> Value {
        let mut object = Map::new();
        object.insert("counts".into(), json!(self.counts));
        object.insert("maximum".into(), json!(self.maximum));
        if with_bounds {
            object.insert("bounds".into(), json!(self.bounds));
        }
        Value::Object(object)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Rhythm {
    intervals: Buckets,
    pauses: Buckets,
    longest_run: i64,
    longest_clean_run: i64,
    hours: Vec<i64>,
}

impl Rhythm {
    fn new() -> Self {
        Self {
            intervals: Buckets::new(INTERVAL_BOUNDS),
            pauses: Buckets::new(PAUSE_BOUNDS),
            longest_run: 0,
            longest_clean_run: 0,
            hours: vec![0; 7 * 24],
        }
    }

    /// Four times the usual key interval, at least 2 s.
    fn pause_threshold(&self) -> f64 {
        (4.0 * self.intervals.percentile(0.5).unwrap_or(0.25)).max(2.0)
    }
}

type Clock<T> = Box<dyn Fn() -> T + Send>;

struct Store {
    path: Option<PathBuf>,
    now: Clock<DateTime<Local>>,
    uptime: Clock<i64>,
    enabled: bool,
    readable: bool,
    period: Period,
    since: DateTime<Utc>,
    totals: Counters,
    days: BTreeMap<NaiveDate, Counters>,
    latency: Buckets,
    rhythm: Rhythm,
    error: Option<String>,
    dirty: bool,
    last_save: Option<i64>,
    writes: usize,
    revision: u64,
    // The observer: one input session's burst and corrections.
    session_threshold: f64,
    last_key: Option<i64>,
    burst: i64,
    clean_run: i64,
    corrected: bool,
    saw_corrected: bool,
    committed_corrected: bool,
    external_rhythm: bool,
}

impl Store {
    fn reset(&mut self) {
        self.since = (self.now)().with_timezone(&Utc);
        self.totals = Counters::default();
        self.days.clear();
        self.latency = Buckets::new(LATENCY_BOUNDS);
        self.rhythm = Rhythm::new();
        self.last_key = None;
        self.burst = 0;
        self.clean_run = 0;
        self.corrected = false;
        self.saw_corrected = false;
        self.committed_corrected = false;
    }

    fn load(&mut self) {
        let Some(path) = self.path.clone().filter(|path| path.exists()) else { return };
        let parsed = std::fs::metadata(&path)
            .ok()
            .filter(|meta| meta.len() <= MAX_STATS_BYTES)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|document| Self::decode(&document));
        match parsed {
            Some((since, totals, days, latency, rhythm)) => {
                self.since = since;
                self.totals = totals;
                self.days = days;
                self.latency = latency;
                self.rhythm = rhythm;
            }
            None => {
                // Never overwrite what cannot be read: counting waits until 清除統計.
                self.readable = false;
                self.error = Some("使用統計資料無法讀取，請先清除統計再開始記錄。".to_owned());
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn decode(document: &Value) -> Option<(DateTime<Utc>, Counters, BTreeMap<NaiveDate, Counters>, Buckets, Rhythm)> {
        let object = document.as_object()?;
        let since = DateTime::parse_from_rfc3339(object.get("since")?.as_str()?).ok()?.with_timezone(&Utc);
        let totals = Counters::decode(object.get("totals")?)?;
        let mut days = BTreeMap::new();
        for (date, value) in object.get("days")?.as_object()? {
            days.insert(NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?, Counters::decode(value)?);
        }
        let latency = Buckets::decode(object.get("latency"), LATENCY_BOUNDS)?;
        let mut rhythm = Rhythm::new();
        if let Some(stored) = object.get("rhythm").and_then(Value::as_object) {
            // As on macOS, a rhythm of an older shape starts empty rather than failing the whole file.
            rhythm.intervals = Buckets::decode(stored.get("intervals"), INTERVAL_BOUNDS)
                .unwrap_or_else(|| Buckets::new(INTERVAL_BOUNDS));
            rhythm.pauses =
                Buckets::decode(stored.get("pauses"), PAUSE_BOUNDS).unwrap_or_else(|| Buckets::new(PAUSE_BOUNDS));
            rhythm.longest_run = count(stored.get("longestRun")).unwrap_or(0);
            rhythm.longest_clean_run = count(stored.get("longestCleanRun")).unwrap_or(0);
            if let Some(hours) = stored.get("hours").and_then(Value::as_array).filter(|h| h.len() == 7 * 24) {
                for (slot, value) in rhythm.hours.iter_mut().zip(hours) {
                    *slot = count(Some(value)).unwrap_or(0);
                }
            }
        }
        Some((since, totals, days, latency, rhythm))
    }

    fn write(&mut self) -> Result<()> {
        let Some(path) = self.path.clone() else {
            self.dirty = false;
            return Ok(());
        };
        let days: Map<String, Value> =
            self.days.iter().map(|(date, counters)| (date.format("%Y-%m-%d").to_string(), counters.encode())).collect();
        let document = json!({
            "schema": SCHEMA,
            "since": self.since.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "totals": self.totals.encode(),
            "days": days,
            "latency": self.latency.encode(false),
            "rhythm": {
                "intervals": self.rhythm.intervals.encode(true),
                "pauses": self.rhythm.pauses.encode(true),
                "longestRun": self.rhythm.longest_run,
                "longestCleanRun": self.rhythm.longest_clean_run,
                "hours": self.rhythm.hours,
            },
        });
        self.writes += 1;
        self.last_save = Some((self.uptime)());
        let written = crate::settings::write_atomically(&path, document.to_string().as_bytes());
        if written.is_err() {
            self.error = Some("無法儲存使用統計，請檢查資料夾的寫入權限。".to_owned());
            self.revision += 1;
            return Err(anyhow!("無法儲存使用統計，請檢查資料夾的寫入權限。"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        self.dirty = false;
        Ok(())
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.revision += 1;
        let now = (self.uptime)();
        match self.last_save {
            None => self.last_save = Some(now),
            Some(last) if now - last >= SAVE_INTERVAL_MS => {
                let _ = self.write();
            }
            Some(_) => {}
        }
    }

    fn add(&mut self, delta: &Counters) {
        if delta.is_zero() {
            return;
        }
        let now = (self.now)();
        let day = now.date_naive();
        self.days.entry(day).or_default().add(delta);
        self.totals.add(delta);
        if delta.characters > 0 {
            // Sunday first, as Calendar's weekday on macOS.
            let weekday = now.weekday().num_days_from_sunday() as usize;
            self.rhythm.hours[weekday * 24 + now.hour() as usize] += delta.characters;
        }
        while self.days.len() > KEPT_DAYS {
            self.days.pop_first();
        }
    }

    fn note_key(&mut self, latency_ms: f64, composing: bool) {
        let mut delta = Counters::default();
        if !self.external_rhythm {
            let now = (self.uptime)();
            if let Some(last) = self.last_key {
                let gap = (now - last) as f64 / 1000.0;
                if (0.0..=self.session_threshold).contains(&gap) {
                    delta.typing_milliseconds = (gap * 1000.0) as i64;
                    self.rhythm.intervals.add(gap);
                } else {
                    if gap > self.session_threshold && gap <= AWAY_AFTER_SECONDS {
                        delta.thinking_milliseconds = (gap * 1000.0) as i64;
                        self.rhythm.pauses.add(gap);
                    }
                    self.burst = 0;
                    self.clean_run = 0;
                }
            }
            self.last_key = Some(now);
        }
        if composing {
            delta.keys = 1;
            if latency_ms.is_finite() && (0.0..=600_000.0).contains(&latency_ms) {
                self.latency.add(latency_ms);
            }
        }
        self.add(&delta);
    }

    fn record(&mut self, event: &UsageEvent) {
        if !self.enabled || !self.readable {
            return;
        }
        let mut delta = Counters::default();
        match *event {
            UsageEvent::Key { latency_ms, composing } => {
                self.note_key(latency_ms, composing);
                self.changed();
                return;
            }
            UsageEvent::Pause { ms } | UsageEvent::Interval { ms } => {
                // The caller measures the rhythm itself: key gaps are no longer measured here.
                self.external_rhythm = true;
                if !ms.is_finite() || ms < 0.0 || ms > AWAY_AFTER_SECONDS * 1000.0 {
                    return;
                }
                if matches!(event, UsageEvent::Pause { .. }) {
                    delta.thinking_milliseconds = ms as i64;
                    self.rhythm.pauses.add(ms / 1000.0);
                    self.burst = 0;
                    self.clean_run = 0;
                } else {
                    delta.typing_milliseconds = ms as i64;
                    self.rhythm.intervals.add(ms / 1000.0);
                }
            }
            UsageEvent::Commit { characters, composition_ended, clean, corrected_shown, corrected_committed } => {
                if !(1..=1_000_000).contains(&characters) {
                    return;
                }
                delta.characters = characters;
                self.burst += characters;
                self.clean_run += characters;
                self.rhythm.longest_run = self.rhythm.longest_run.max(self.burst);
                self.rhythm.longest_clean_run = self.rhythm.longest_clean_run.max(self.clean_run);
                if composition_ended {
                    delta.compositions = 1;
                    if clean.unwrap_or(!self.corrected) {
                        delta.clean_compositions = 1;
                    }
                    if self.saw_corrected || corrected_shown {
                        delta.corrected_shown = 1;
                    }
                    if self.committed_corrected || corrected_committed {
                        delta.corrected_committed = 1;
                    }
                    self.corrected = false;
                    self.saw_corrected = false;
                    self.committed_corrected = false;
                }
            }
            UsageEvent::Pick(list) => match list {
                PickList::Corrections => delta.picks_corrections = 1,
                PickList::Single => delta.picks_single = 1,
                PickList::Punctuation => delta.picks_punctuation = 1,
                PickList::Normal => delta.picks_normal = 1,
            },
            UsageEvent::Backspace => delta.backspaces = 1,
            UsageEvent::Escape => delta.escapes = 1,
            UsageEvent::DeleteAfterCommit => delta.delete_after_commit = 1,
            UsageEvent::LiteralZhuyin => delta.literal_zhuyin = 1,
            UsageEvent::UndoLiteral => delta.undo_literal = 1,
            UsageEvent::PhraseAdded => delta.phrases_added = 1,
            UsageEvent::CorrectionShown => {
                self.saw_corrected = true;
                return;
            }
            UsageEvent::CorrectionCommitted => {
                self.committed_corrected = true;
                return;
            }
            UsageEvent::Reset => {
                self.last_key = None;
                self.burst = 0;
                self.clean_run = 0;
                self.corrected = false;
                self.saw_corrected = false;
                self.committed_corrected = false;
                return;
            }
        }
        if delta.picks() > 0
            || delta.backspaces > 0
            || delta.escapes > 0
            || delta.literal_zhuyin > 0
            || delta.delete_after_commit > 0
        {
            self.corrected = true;
            self.clean_run = 0;
        }
        self.add(&delta);
        self.changed();
    }

    fn usage(&self) -> Usage {
        let today = (self.now)().date_naive();
        let day = |back: u64| today.checked_sub_days(Days::new(back)).unwrap_or(today);
        let mut month = Counters::default();
        let mut days = Vec::with_capacity(30);
        for back in (0..30).rev() {
            let date = day(back);
            let counters = self.days.get(&date).copied().unwrap_or_default();
            month.add(&counters);
            days.push(DayCount { date, characters: counters.characters });
        }
        let counters = if self.period == Period::All { self.totals } else { month };
        // Consecutive days with typing up to today, or yesterday when today has none yet.
        let mut streak = 0;
        for back in 0..KEPT_DAYS as u64 {
            let characters = self.days.get(&day(back)).map_or(0, |c| c.characters);
            if back == 0 && characters == 0 {
                continue;
            }
            if characters == 0 {
                break;
            }
            streak += 1;
        }
        let best_day =
            self.days.iter().filter(|(_, c)| c.characters > 0).fold(None::<DayCount>, |best, (date, c)| match best {
                Some(best) if best.characters >= c.characters => Some(best),
                _ => Some(DayCount { date: *date, characters: c.characters }),
            });
        Usage {
            valid: self.readable,
            since: self.since.with_timezone(&Local).date_naive(),
            days,
            counters,
            active_speed: (counters.typing_milliseconds >= 1000)
                .then(|| counters.characters as f64 * 60_000.0 / counters.typing_milliseconds as f64),
            first_time_right: (counters.compositions > 0)
                .then(|| counters.clean_compositions as f64 / counters.compositions as f64),
            streak,
            best_day,
            latency: LatencySummary {
                total: self.latency.total(),
                p50: self.latency.percentile(0.5),
                p95: self.latency.percentile(0.95),
                maximum: (self.latency.total() > 0).then_some(self.latency.maximum),
                counts: self.latency.counts.clone(),
                bounds: self.latency.bounds.to_vec(),
            },
            rhythm: RhythmSummary {
                longest_run: self.rhythm.longest_run,
                longest_clean_run: self.rhythm.longest_clean_run,
                pause_threshold: self.rhythm.pause_threshold(),
                pause_median: self.rhythm.pauses.percentile(0.5),
                hours: self.rhythm.hours.clone(),
            },
        }
    }
}

/// The statistics store.  Cheap to clone; clones share it.
#[derive(Clone)]
pub struct UsageStats {
    inner: Arc<Mutex<Store>>,
}

impl UsageStats {
    /// The statistics in `path` (`None`: in memory only), with the system clocks.  Off until
    /// [`UsageStats::set_enabled`].
    #[must_use]
    pub fn open(path: Option<PathBuf>) -> Self {
        let start = std::time::Instant::now();
        Self::with_clocks(path, Local::now, move || i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX))
    }

    /// The statistics in `path` with a wall clock (for days and hours) and a monotonic clock in ms (key gaps, the
    /// write interval): tests replace both.
    #[must_use]
    pub fn with_clocks(
        path: Option<PathBuf>,
        now: impl Fn() -> DateTime<Local> + Send + 'static,
        uptime: impl Fn() -> i64 + Send + 'static,
    ) -> Self {
        let since = now().with_timezone(&Utc);
        let mut store = Store {
            path,
            now: Box::new(now),
            uptime: Box::new(uptime),
            enabled: false,
            readable: true,
            period: Period::Month,
            since,
            totals: Counters::default(),
            days: BTreeMap::new(),
            latency: Buckets::new(LATENCY_BOUNDS),
            rhythm: Rhythm::new(),
            error: None,
            dirty: false,
            last_save: None,
            writes: 0,
            revision: 0,
            session_threshold: 2.0,
            last_key: None,
            burst: 0,
            clean_run: 0,
            corrected: false,
            saw_corrected: false,
            committed_corrected: false,
            external_rhythm: false,
        };
        store.reset();
        store.load();
        store.session_threshold = store.rhythm.pause_threshold();
        Self { inner: Arc::new(Mutex::new(store)) }
    }

    /// Counts `event` (nothing while off or unreadable).
    pub fn record(&self, event: &UsageEvent) {
        self.inner.lock().record(event);
    }

    /// The aggregates for the current period.
    #[must_use]
    pub fn usage(&self) -> Usage {
        self.inner.lock().usage()
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.inner.lock().enabled
    }

    /// 記錄使用統計 on or off; off writes what was counted.
    pub fn set_enabled(&self, enabled: bool) {
        let mut store = self.inner.lock();
        if store.enabled == enabled {
            return;
        }
        store.enabled = enabled;
        if !enabled {
            if store.dirty && store.readable {
                let _ = store.write();
            }
            store.last_key = None;
        }
        store.revision += 1;
    }

    #[must_use]
    pub fn period(&self) -> Period {
        self.inner.lock().period
    }

    pub fn set_period(&self, period: Period) {
        let mut store = self.inner.lock();
        if store.period != period {
            store.period = period;
            store.revision += 1;
        }
    }

    /// The pause threshold the input controller should use for this user's rhythm, in seconds.
    #[must_use]
    pub fn pause_threshold(&self) -> f64 {
        self.inner.lock().rhythm.pause_threshold()
    }

    /// 清除統計: deletes the file and starts over.
    ///
    /// # Errors
    /// When the file cannot be deleted.
    pub fn clear(&self) -> Result<()> {
        let mut store = self.inner.lock();
        if let Some(path) = store.path.clone()
            && path.exists()
            && std::fs::remove_file(&path).is_err()
        {
            store.error = Some("無法刪除使用統計檔案，請檢查資料夾的寫入權限。".to_owned());
            store.revision += 1;
            return Err(anyhow!("無法刪除使用統計檔案，請檢查資料夾的寫入權限。"));
        }
        store.reset();
        store.session_threshold = store.rhythm.pause_threshold();
        store.readable = true;
        store.error = None;
        store.dirty = false;
        store.revision += 1;
        Ok(())
    }

    /// Writes `stats.json` now if anything changed.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn save_if_needed(&self) -> Result<()> {
        let mut store = self.inner.lock();
        if !store.dirty || !store.readable {
            return Ok(());
        }
        store.write()
    }

    /// Why the statistics cannot be read or saved, if so.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.inner.lock().error.clone()
    }

    /// Increases with every change, so the page can refresh (at most every two seconds, as macOS).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.lock().revision
    }

    /// Writes since the store opened (tests).
    #[must_use]
    pub fn writes(&self) -> usize {
        self.inner.lock().writes
    }
}

/// CPU and memory of this process, sampled now.
#[must_use]
pub fn process_metrics() -> ProcessMetrics {
    static LAUNCHED: std::sync::OnceLock<(DateTime<Local>, std::time::Instant)> = std::sync::OnceLock::new();
    let (launched, started) = *LAUNCHED.get_or_init(|| (Local::now(), std::time::Instant::now()));
    let (cpu_seconds, memory_bytes, peak_memory_bytes) = platform::sample();
    ProcessMetrics {
        launched,
        running_seconds: started.elapsed().as_secs_f64(),
        cpu_seconds,
        memory_bytes,
        peak_memory_bytes,
    }
}

#[cfg(unix)]
mod platform {
    /// CPU seconds (user + system), resident memory now and at its highest, from the kernel's own accounting.
    pub(super) fn sample() -> (Option<f64>, Option<f64>, Option<f64>) {
        // utime and stime, in clock ticks: fields 14 and 15, counted after the command name's closing parenthesis.
        let ticks = rustix::param::clock_ticks_per_second() as f64;
        let cpu = std::fs::read_to_string("/proc/self/stat").ok().and_then(|stat| {
            let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split_whitespace().collect();
            let user: f64 = fields.get(11)?.parse().ok()?;
            let system: f64 = fields.get(12)?.parse().ok()?;
            Some((user + system) / ticks)
        });
        let mut memory = None;
        let mut peak = None;
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                let mut parts = line.split_whitespace();
                let (Some(name), Some(value)) = (parts.next(), parts.next()) else { continue };
                let bytes = value.parse::<f64>().ok().map(|kib| kib * 1024.0);
                match name {
                    "VmRSS:" => memory = bytes,
                    "VmHWM:" => peak = bytes,
                    _ => {}
                }
            }
        }
        (cpu, memory, peak)
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    /// CPU seconds (kernel + user), private bytes now and the peak page-file use.
    pub(super) fn sample() -> (Option<f64>, Option<f64>, Option<f64>) {
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let seconds = |t: FILETIME| ((u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)) as f64 / 1e7;
        // SAFETY: the pseudo-handle of this process is always valid; the out pointers are live locals.
        let cpu = (unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) }
            != 0)
            .then(|| seconds(kernel) + seconds(user));
        // SAFETY: an all-zero PROCESS_MEMORY_COUNTERS_EX is valid; `cb` gives its size.
        let mut memory: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        memory.cb = size;
        // SAFETY: as above; the struct is live and as large as `size` says.
        let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut memory).cast(), size) } != 0;
        if ok {
            (cpu, Some(memory.PrivateUsage as f64), Some(memory.PeakPagefileUsage as f64))
        } else {
            (cpu, None, None)
        }
    }
}

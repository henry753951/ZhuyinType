//! Port of the Qt `DesktopServicesTest` (usage statistics and process metrics; the update check is not ported).

mod common;

use std::sync::Arc;

use chrono::{DateTime, Days, Local, NaiveDate, TimeZone as _};
use common::TempDir;
use parking_lot::Mutex;
use serde_json::Value;
use zt_core::stats::{PickList, SAVE_INTERVAL_MS, UsageEvent, UsageStats, process_metrics};

struct Clocks {
    now: Arc<Mutex<DateTime<Local>>>,
    uptime: Arc<Mutex<i64>>,
}

fn store(path: Option<std::path::PathBuf>, start: DateTime<Local>) -> (UsageStats, Clocks) {
    let now = Arc::new(Mutex::new(start));
    let uptime = Arc::new(Mutex::new(0));
    let (n, u) = (now.clone(), uptime.clone());
    (UsageStats::with_clocks(path, move || *n.lock(), move || *u.lock()), Clocks { now, uptime })
}

fn commit(characters: i64) -> UsageEvent {
    UsageEvent::Commit {
        characters,
        composition_ended: true,
        clean: None,
        corrected_shown: false,
        corrected_committed: false,
    }
}

fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(year, month, day, hour, minute, 0).unwrap()
}

#[test]
fn counts_like_macos() {
    let dir = TempDir::new();
    let path = dir.join("stats.json");
    let start = local(2026, 10, 25, 9, 30); // A Sunday.
    {
        let (stats, clocks) = store(Some(path.clone()), start);
        // Off: nothing is counted.
        stats.record(&commit(5));
        assert_eq!(stats.usage().counters.characters, 0);
        stats.set_enabled(true);
        for i in 0..4 {
            stats.record(&UsageEvent::Key { latency_ms: 2.5 + f64::from(i), composing: true });
            *clocks.uptime.lock() += 200;
        }
        stats.record(&UsageEvent::Key { latency_ms: 1.0, composing: false }); // Not composing: no key, no latency.
        stats.record(&commit(2));
        *clocks.uptime.lock() += 5000; // A thinking pause (over the 2 s threshold, under a minute).
        stats.record(&UsageEvent::Key { latency_ms: 40.0, composing: true });
        stats.record(&UsageEvent::Pick(PickList::Corrections));
        stats.record(&UsageEvent::Backspace);
        stats.record(&UsageEvent::CorrectionShown);
        stats.record(&commit(3));
        stats.record(&UsageEvent::PhraseAdded);
        let usage = stats.usage();
        let c = usage.counters;
        assert_eq!(c.characters, 5);
        assert_eq!(c.compositions, 2);
        assert_eq!(c.clean_compositions, 1);
        assert_eq!(c.keys, 5);
        assert_eq!(c.picks_corrections, 1);
        assert_eq!(c.backspaces, 1);
        assert_eq!(c.corrections(), 2);
        assert_eq!(c.corrected_shown, 1);
        assert_eq!(c.phrases_added, 1);
        assert_eq!(c.typing_milliseconds, 800);
        assert_eq!(c.thinking_milliseconds, 5000);
        assert_eq!(usage.first_time_right, Some(0.5));
        assert_eq!(usage.latency.total, 5);
        assert_eq!(usage.latency.maximum, Some(40.0));
        assert!(usage.latency.p50.is_some_and(|p| p > 2.0 && p <= 5.0));
        assert_eq!(usage.rhythm.longest_run, 3, "the pause ended the first burst");
        assert_eq!(usage.rhythm.longest_clean_run, 3);
        assert_eq!(usage.rhythm.hours[9], 5, "Sunday 09:00");
        assert_eq!(usage.streak, 1);
        assert_eq!(usage.best_day.map(|d| d.characters), Some(5));
        // Written at most every 10 minutes: nothing yet, then on request.
        assert_eq!(stats.writes(), 0);
        assert!(!path.exists());
        stats.save_if_needed().unwrap();
        assert_eq!(stats.writes(), 1);
        stats.save_if_needed().unwrap();
        assert_eq!(stats.writes(), 1);
        *clocks.uptime.lock() += SAVE_INTERVAL_MS;
        stats.record(&UsageEvent::Escape);
        assert_eq!(stats.writes(), 2);
        drop(clocks.now);
    }
    // The macOS stats.json schema, read back.
    let object: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(object["schema"], 2);
    assert!(object["totals"].get("cleanCompositions").is_some());
    assert_eq!(object["latency"]["counts"].as_array().unwrap().len(), 22);
    assert_eq!(object["rhythm"]["hours"].as_array().unwrap().len(), 168);
    assert!(object["days"].get("2026-10-25").is_some());
    let (again, _clocks) = store(Some(path), start);
    let usage = again.usage();
    assert_eq!(usage.counters.characters, 5);
    assert_eq!(usage.counters.escapes, 1);
    assert_eq!(usage.since, NaiveDate::from_ymd_opt(2026, 10, 25).unwrap());
}

#[test]
fn periods_and_retention() {
    let dir = TempDir::new();
    let (stats, clocks) = store(Some(dir.join("stats.json")), local(2026, 1, 1, 12, 0));
    let advance = |days| {
        let mut now = clocks.now.lock();
        *now = now.checked_add_days(Days::new(days)).unwrap();
    };
    stats.set_enabled(true);
    stats.record(&commit(10));
    advance(29);
    stats.record(&commit(3));
    assert_eq!(stats.usage().counters.characters, 13);
    assert_eq!(stats.usage().days.len(), 30);
    advance(1);
    assert_eq!(stats.usage().counters.characters, 3);
    assert_eq!(stats.usage().streak, 1, "yesterday counts while today has none");
    stats.set_period(zt_core::stats::Period::All);
    assert_eq!(stats.usage().counters.characters, 13);
    for _ in 0..70 {
        advance(1);
        stats.record(&commit(1));
    }
    assert_eq!(stats.usage().counters.characters, 83);
    assert_eq!(stats.usage().streak, 60);
    stats.set_period(zt_core::stats::Period::Month);
    assert_eq!(stats.usage().counters.characters, 30);
    stats.save_if_needed().unwrap();
    let object: Value = serde_json::from_slice(&std::fs::read(dir.join("stats.json")).unwrap()).unwrap();
    assert_eq!(object["days"].as_object().unwrap().len(), zt_core::stats::KEPT_DAYS);
}

#[test]
fn caller_measured_rhythm() {
    let (stats, clocks) = store(None, Local::now());
    stats.set_enabled(true);
    stats.record(&UsageEvent::Pause { ms: 3000.0 });
    stats.record(&UsageEvent::Interval { ms: 150.0 });
    *clocks.uptime.lock() += 9000;
    stats.record(&UsageEvent::Key { latency_ms: 3.0, composing: true }); // Gaps are no longer measured here.
    let usage = stats.usage();
    assert_eq!(usage.counters.thinking_milliseconds, 3000);
    assert_eq!(usage.counters.typing_milliseconds, 150);
    assert!(usage.rhythm.pause_median.is_some_and(|m| m > 2.0));
    stats.record(&UsageEvent::Pause { ms: 61_000.0 }); // Away, not thinking.
    assert_eq!(stats.usage().counters.thinking_milliseconds, 3000);
}

#[test]
fn corrupted_file_is_kept_until_cleared() {
    let dir = TempDir::new();
    let path = dir.join("stats.json");
    std::fs::write(&path, "{broken").unwrap();
    let stats = UsageStats::open(Some(path.clone()));
    stats.set_enabled(true);
    assert!(stats.error().is_some());
    assert!(!stats.usage().valid);
    stats.record(&commit(1));
    stats.save_if_needed().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
    stats.clear().unwrap();
    assert!(stats.usage().valid);
    assert!(!path.exists());
    stats.record(&commit(1));
    assert_eq!(stats.usage().counters.characters, 1);
}

#[test]
fn invalid_events_are_ignored() {
    let stats = UsageStats::open(None);
    stats.set_enabled(true);
    stats.record(&commit(-1));
    stats.record(&commit(1_000_001));
    stats.record(&UsageEvent::Key { latency_ms: -4.0, composing: true });
    let usage = stats.usage();
    assert_eq!(usage.counters.characters, 0);
    assert_eq!(usage.latency.total, 0);
    assert!(usage.latency.p95.is_none());
}

#[test]
fn process_metrics_are_sampled() {
    let metrics = process_metrics();
    assert!(metrics.cpu_seconds.is_some_and(|s| s >= 0.0));
    assert!(metrics.running_seconds >= 0.0);
    let memory = metrics.memory_bytes.unwrap();
    assert!(memory > 0.0);
    assert!(metrics.peak_memory_bytes.unwrap() >= memory * 0.5);
}

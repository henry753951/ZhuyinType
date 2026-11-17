//! Where the 分析 page's numbers come from: the service's statistics when it runs; otherwise nothing (the page's
//! empty state), or, for screenshots (`ZT_DEMO=analytics-sample`), sample data shaped like the macOS renderer's.
//! Process metrics come from the core once the service runs, else from `/proc` on Linux.

use chrono::{Datelike, Duration, Local, NaiveDate};
use zt_core::stats::{Counters, DayCount, LatencySummary, Period, ProcessMetrics, RhythmSummary, Usage};

/// The latency histogram's bucket bounds (ms), as the core keeps them.
const BOUNDS: [f64; 22] =
    [1., 2., 3., 4., 5., 6., 8., 10., 12., 15., 20., 25., 30., 40., 50., 75., 100., 150., 200., 300., 500., 1000.];

/// A small deterministic generator, so sample screenshots compare run to run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() * (high - low + 1) as f64) as i64
    }
}

fn add(a: &mut Counters, b: &Counters) {
    a.characters += b.characters;
    a.compositions += b.compositions;
    a.clean_compositions += b.clean_compositions;
    a.keys += b.keys;
    a.picks_normal += b.picks_normal;
    a.picks_corrections += b.picks_corrections;
    a.picks_single += b.picks_single;
    a.picks_punctuation += b.picks_punctuation;
    a.backspaces += b.backspaces;
    a.escapes += b.escapes;
    a.delete_after_commit += b.delete_after_commit;
    a.literal_zhuyin += b.literal_zhuyin;
    a.undo_literal += b.undo_literal;
    a.corrected_shown += b.corrected_shown;
    a.corrected_committed += b.corrected_committed;
    a.corrections_reverted += b.corrections_reverted;
    a.phrases_added += b.phrases_added;
    a.typing_milliseconds += b.typing_milliseconds;
    a.thinking_milliseconds += b.thinking_milliseconds;
}

/// 45 days of typing with a few days off (macOS `UIPreviewRenderer.sampleStats`).
pub fn sample(period: Period) -> Usage {
    let mut random = Lcg(20_261_116);
    let today = Local::now().date_naive();
    let mut days = Vec::new();
    let mut month = Counters::default();
    let mut total = Counters::default();
    let mut best: Option<DayCount> = None;
    for back in (0..45).rev() {
        let date = today - Duration::days(back);
        let off = [3, 4, 11, 18, 25].contains(&(back % 30));
        let mut day = Counters::default();
        if !off {
            day.characters = random.range(300, 2400);
            day.compositions = day.characters / 9;
            day.keys = day.characters * 3;
            day.picks_normal = day.characters / 60;
            day.picks_corrections = day.characters / 400;
            day.picks_single = day.characters / 500;
            day.picks_punctuation = day.characters / 700;
            day.backspaces = day.characters / 45;
            day.escapes = day.characters / 300;
            day.delete_after_commit = day.characters / 600;
            day.literal_zhuyin = day.characters / 1500;
            day.undo_literal = day.characters / 4000;
            day.corrected_shown = day.compositions / 25;
            day.corrected_committed = day.compositions / 30;
            day.corrections_reverted = day.compositions / 120;
            day.phrases_added = i64::from(back % 9 == 0);
            day.clean_compositions = day.compositions * 82 / 100;
            day.typing_milliseconds = day.characters * 60_000 / random.range(38, 52);
            day.thinking_milliseconds = day.typing_milliseconds * random.range(35, 70) / 100;
        }
        add(&mut total, &day);
        if back < 30 {
            add(&mut month, &day);
            days.push(DayCount { date, characters: day.characters });
        }
        if day.characters > 0 && best.is_none_or(|b| day.characters > b.characters) {
            best = Some(DayCount { date, characters: day.characters });
        }
    }
    let counters = if period == Period::All { total } else { month };
    let mut counts = vec![0i64; BOUNDS.len() + 1];
    for _ in 0..8000 {
        let ms = (-(random.next().max(0.0001)).ln() * 3.2).max(0.4);
        let bucket = BOUNDS.iter().position(|bound| ms <= *bound).unwrap_or(BOUNDS.len());
        counts[bucket] += 1;
    }
    counts[BOUNDS.iter().position(|bound| *bound >= 87.).unwrap_or(0)] += 1;
    let mut hours = vec![0i64; 7 * 24];
    for day in 0..7 {
        for hour in 0..24 {
            let work = (9..=12).contains(&hour) || (14..=18).contains(&hour);
            let evening = (20..=23).contains(&hour);
            let weekend = day == 0 || day == 6;
            let base = if work {
                if weekend { 40 } else { 420 }
            } else if evening {
                260
            } else if hour < 7 {
                0
            } else {
                60
            };
            hours[day * 24 + hour] = if base == 0 { 0 } else { base + random.range(0, base / 2) };
        }
    }
    let streak =
        days.iter().rev().skip_while(|d| d.date == today && d.characters == 0).take_while(|d| d.characters > 0).count();
    let typing_minutes = counters.typing_milliseconds as f64 / 60_000.;
    Usage {
        valid: true,
        since: today - Duration::days(45),
        days,
        counters,
        active_speed: (typing_minutes > 0.).then(|| counters.characters as f64 / typing_minutes),
        first_time_right: (counters.compositions > 0)
            .then(|| counters.clean_compositions as f64 / counters.compositions as f64),
        streak: streak as u32,
        best_day: best,
        latency: LatencySummary {
            total: 8001,
            p50: Some(2.2),
            p95: Some(9.6),
            maximum: Some(87.),
            counts,
            bounds: BOUNDS.to_vec(),
        },
        rhythm: RhythmSummary {
            longest_run: 186,
            longest_clean_run: 74,
            pause_threshold: 2.,
            pause_median: Some(5.3),
            hours,
        },
    }
}

/// The empty statistics: what the page shows before anything was counted, or without the service.
pub fn empty() -> Usage {
    let today = Local::now().date_naive();
    Usage {
        valid: true,
        since: today,
        days: (0..30).rev().map(|back| DayCount { date: today - Duration::days(back), characters: 0 }).collect(),
        counters: Counters::default(),
        active_speed: None,
        first_time_right: None,
        streak: 0,
        best_day: None,
        latency: LatencySummary {
            bounds: BOUNDS.to_vec(),
            counts: vec![0; BOUNDS.len() + 1],
            ..LatencySummary::default()
        },
        rhythm: RhythmSummary { pause_threshold: 2., hours: vec![0; 7 * 24], ..RhythmSummary::default() },
    }
}

/// This process's CPU and memory, from `/proc` (Linux); None where it cannot be read.
pub fn local_metrics() -> Option<ProcessMetrics> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the command name, which may hold spaces: state is field 3, utime 14, stime 15, starttime 22.
    let rest = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let ticks = 100.;
    let cpu = fields.get(11)?.parse::<f64>().ok()? / ticks + fields.get(12)?.parse::<f64>().ok()? / ticks;
    let start = fields.get(19)?.parse::<f64>().ok()? / ticks;
    let uptime: f64 = std::fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()?;
    let running = (uptime - start).max(0.);
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<f64>().ok())
            .map(|kb| kb * 1024.)
    };
    Some(ProcessMetrics {
        launched: Local::now() - Duration::milliseconds((running * 1000.) as i64),
        running_seconds: running,
        cpu_seconds: Some(cpu),
        memory_bytes: kb("VmRSS:"),
        peak_memory_bytes: kb("VmHWM:"),
    })
}

/// 「10/18」.
pub fn month_day(date: NaiveDate) -> String {
    format!("{}/{}", date.month(), date.day())
}

/// 「10月16日」.
pub fn month_day_long(date: NaiveDate) -> String {
    format!("{}月{}日", date.month(), date.day())
}

/// 「2026年10月2日」.
pub fn long_date(date: NaiveDate) -> String {
    format!("{}年{}月{}日", date.year(), date.month(), date.day())
}

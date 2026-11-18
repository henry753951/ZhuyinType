//! Where the 字詞 pages get personal data: the running service, or (for screenshots, `ZT_DEMO=data`) an in-memory
//! sample with the same rules, or nothing (no engine: the lists show their empty states and ＋ is off).
//!
//! `ZT_DEMO` is a comma-separated list of screenshot states read by the 字詞, 進階 and 關於 pages: `data` (sample
//! words, apps and records), `many` (5 000 learned words), `search:TEXT`, `select`, `add`, `edit`, `clear-all`,
//! `invalid`, `reset`, `clear-log`, `add-app`, `doc:FILE`, `open:ENTRY`.

use std::cell::RefCell;
use std::path::Path;

use anyhow::{Result, anyhow, ensure};
use gpui_kit::{App, Global};
use zt_core::engine::Phrase;
use zt_core::service::Service;

use crate::app;

/// The value of screenshot state `name` in `ZT_DEMO` (`Some("")` for a bare flag).
pub fn demo(name: &str) -> Option<String> {
    let states = std::env::var("ZT_DEMO").ok()?;
    states.split(',').find_map(|state| match state.split_once(':') {
        Some((key, value)) if key == name => Some(value.to_owned()),
        None if state == name => Some(String::new()),
        _ => None,
    })
}

/// The sample personal data of screenshots.
pub struct Sample {
    pub phrases: Vec<Phrase>,
    pub learned: Vec<Phrase>,
    pub excluded: Vec<Phrase>,
    /// Words set to 不要再推薦.
    pub blocked: Vec<Phrase>,
    pub apps: Vec<String>,
    pub corrections: usize,
    pub revision: u64,
}

struct SampleCell(RefCell<Sample>);

impl Global for SampleCell {}

fn phrase(word: &str, reading: &str, count: f64) -> Phrase {
    Phrase { word: word.to_owned(), reading: reading.to_owned(), count }
}

/// Installs the sample when `ZT_DEMO` asks for it (once).
pub fn install_sample(cx: &mut App) {
    if cx.has_global::<SampleCell>() || (demo("data").is_none() && demo("many").is_none()) {
        return;
    }
    let mut learned: Vec<Phrase> = [
        ("今天", "ㄐㄧㄣ ㄊㄧㄢ", 41.),
        ("天氣", "ㄊㄧㄢ ㄑㄧˋ", 37.),
        ("謝謝", "ㄒㄧㄝˋ ㄒㄧㄝ˙", 18.),
        ("朋友", "ㄆㄥˊ ㄧㄡˇ", 9.),
        ("公園", "ㄍㄨㄥ ㄩㄢˊ", 8.),
        ("工作", "ㄍㄨㄥ ㄗㄨㄛˋ", 5.),
        ("手機", "ㄕㄡˇ ㄐㄧ", 4.5),
        ("加油", "ㄐㄧㄚ ㄧㄡˊ", 4.),
        ("台北", "ㄊㄞˊ ㄅㄟˇ", 3.6),
        ("吃飯", "ㄔ ㄈㄢˋ", 3.2),
        ("我們", "ㄨㄛˇ ㄇㄣ˙", 3.),
        ("明天", "ㄇㄧㄥˊ ㄊㄧㄢ", 2.6),
        ("昨天", "ㄗㄨㄛˊ ㄊㄧㄢ", 2.4),
        ("電腦", "ㄉㄧㄢˋ ㄋㄠˇ", 2.),
        ("咖啡", "ㄎㄚ ㄈㄟ", 1.8),
        ("捷運", "ㄐㄧㄝˊ ㄩㄣˋ", 1.5),
        ("颱風", "ㄊㄞˊ ㄈㄥ", 1.2),
        ("便當", "ㄅㄧㄢˋ ㄉㄤ", 1.),
        ("週末", "ㄓㄡ ㄇㄛˋ", 0.8),
        ("圖書館", "ㄊㄨˊ ㄕㄨ ㄍㄨㄢˇ", 0.6),
    ]
    .into_iter()
    .map(|(word, reading, count)| phrase(word, reading, count))
    .collect();
    if demo("many").is_some() {
        let syllables = ["ㄅㄚ", "ㄆㄛˊ", "ㄇㄧˇ", "ㄈㄨˋ", "ㄉㄜ˙", "ㄊㄞˊ", "ㄋㄢˊ", "ㄌㄧㄡˊ", "ㄍㄨㄥ", "ㄎㄚ"];
        let chars = ["八", "婆", "米", "父", "的", "台", "南", "流", "工", "咖"];
        for n in 0..5000usize {
            let (a, b) = (n % 10, (n / 10) % 10);
            let c = (n / 100) % 10;
            learned.push(phrase(
                &format!("{}{}{}", chars[a], chars[b], chars[c]),
                &format!("{} {} {}", syllables[a], syllables[b], syllables[c]),
                (n % 97) as f64 / 10.,
            ));
        }
    }
    let sample = Sample {
        phrases: vec![
            phrase("台南", "ㄊㄞˊ ㄋㄢˊ", 0.),
            phrase("注音", "ㄓㄨˋ ㄧㄣ", 0.),
            phrase("王小明", "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ", 0.),
        ],
        learned,
        excluded: vec![phrase("的話", "ㄉㄜ˙ ㄏㄨㄚˋ", 0.)],
        blocked: vec![phrase("八", "ㄅㄚ", 0.)],
        apps: vec!["wezterm".to_owned(), "Code.exe".to_owned()],
        corrections: 3,
        revision: 1,
    };
    cx.set_global(SampleCell(RefCell::new(sample)));
}

enum Source<'a> {
    Service(&'a Service),
    Sample(&'a RefCell<Sample>),
    None,
}

fn source(cx: &App) -> Source<'_> {
    if let Some(service) = app::service(cx) {
        Source::Service(service)
    } else if let Some(sample) = cx.try_global::<SampleCell>() {
        Source::Sample(&sample.0)
    } else {
        Source::None
    }
}

/// Whether there is personal data to show and change (the engine runs, or a screenshot sample).
pub fn available(cx: &App) -> bool {
    !matches!(source(cx), Source::None)
}

/// Changes whenever personal data may have changed.
pub fn revision(cx: &App) -> u64 {
    match source(cx) {
        Source::Service(service) => service.personal_revision(),
        Source::Sample(sample) => sample.borrow().revision,
        Source::None => 0,
    }
}

pub fn user_phrases(cx: &App) -> Vec<Phrase> {
    match source(cx) {
        Source::Service(service) => service.user_phrases(),
        Source::Sample(sample) => sample.borrow().phrases.clone(),
        Source::None => Vec::new(),
    }
}

pub fn learned_entries(cx: &App) -> Vec<Phrase> {
    match source(cx) {
        Source::Service(service) => service.learned_entries(),
        Source::Sample(sample) => sample.borrow().learned.clone(),
        Source::None => Vec::new(),
    }
}

pub fn excluded_entries(cx: &App) -> Vec<Phrase> {
    match source(cx) {
        Source::Service(service) => service.excluded_entries(),
        Source::Sample(sample) => sample.borrow().excluded.clone(),
        Source::None => Vec::new(),
    }
}

/// Words set to 不要再推薦 (from the candidate window).
pub fn blocked_entries(cx: &App) -> Vec<Phrase> {
    match source(cx) {
        Source::Service(service) => service.blocked_entries(),
        Source::Sample(sample) => sample.borrow().blocked.clone(),
        Source::None => Vec::new(),
    }
}

/// Suggests a blocked word again.
pub fn unblock(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.unblock(word, reading);
    }
    with_sample(cx, |sample| {
        sample.blocked.retain(|entry| !same(entry, word, reading));
        Ok(())
    })
}

/// The readings of `word` the lexicon knows (the add sheet's suggestions).
pub fn readings(word: &str, cx: &App) -> Vec<String> {
    if word.is_empty() {
        return Vec::new();
    }
    match source(cx) {
        Source::Service(service) => service.readings(word),
        Source::Sample(_) => SAMPLE_READINGS
            .iter()
            .filter(|(known, _)| *known == word)
            .map(|(_, reading)| (*reading).to_owned())
            .collect(),
        Source::None => Vec::new(),
    }
}

const SAMPLE_READINGS: &[(&str, &str)] =
    &[("台南", "ㄊㄞˊ ㄋㄢˊ"), ("注音", "ㄓㄨˋ ㄧㄣ"), ("行人", "ㄒㄧㄥˊ ㄖㄣˊ"), ("行人", "ㄏㄤˊ ㄖㄣˊ")];

/// The sample's version of the engine's check: 1–6 characters, one syllable each.
fn check_sample(word: &str, reading: &str) -> Result<()> {
    let chars = word.chars().count();
    ensure!((1..=6).contains(&chars), "詞需為 1 到 6 個字");
    ensure!(reading.split_whitespace().count() == chars, "讀音需為每個字一個注音");
    Ok(())
}

fn with_sample<R>(cx: &mut App, change: impl FnOnce(&mut Sample) -> Result<R>) -> Result<R> {
    let cell = cx.try_global::<SampleCell>().ok_or_else(|| anyhow!("詞庫沒有載入"))?;
    let mut sample = cell.0.borrow_mut();
    let result = change(&mut sample)?;
    sample.revision += 1;
    drop(sample);
    cx.refresh_windows();
    Ok(result)
}

fn same(entry: &Phrase, word: &str, reading: &str) -> bool {
    entry.word == word && entry.reading == reading
}

pub fn add_phrase(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.add_phrase(word, reading);
    }
    with_sample(cx, |sample| {
        check_sample(word, reading)?;
        if !sample.phrases.iter().any(|entry| same(entry, word, reading)) {
            sample.phrases.push(phrase(word, reading, 0.));
        }
        Ok(())
    })
}

pub fn edit_phrase(old_word: &str, old_reading: &str, word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.edit_phrase(old_word, old_reading, word, reading);
    }
    with_sample(cx, |sample| {
        check_sample(word, reading)?;
        sample.phrases.retain(|entry| !same(entry, old_word, old_reading));
        if !sample.phrases.iter().any(|entry| same(entry, word, reading)) {
            sample.phrases.push(phrase(word, reading, 0.));
        }
        Ok(())
    })
}

pub fn remove_phrase(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.remove_phrase(word, reading);
    }
    with_sample(cx, |sample| {
        sample.phrases.retain(|entry| !same(entry, word, reading));
        Ok(())
    })
}

pub fn forget_learned(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.forget_learned(word, reading);
    }
    with_sample(cx, |sample| {
        sample.learned.retain(|entry| !same(entry, word, reading));
        Ok(())
    })
}

pub fn exclude_learned(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.exclude_learned(word, reading);
    }
    with_sample(cx, |sample| {
        sample.learned.retain(|entry| !same(entry, word, reading));
        if !sample.excluded.iter().any(|entry| same(entry, word, reading)) {
            sample.excluded.push(phrase(word, reading, 0.));
        }
        Ok(())
    })
}

pub fn include_learned(word: &str, reading: &str, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.include_learned(word, reading);
    }
    with_sample(cx, |sample| {
        sample.excluded.retain(|entry| !same(entry, word, reading));
        Ok(())
    })
}

pub fn forget_all_learned(cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.forget_all_learned();
    }
    with_sample(cx, |sample| {
        sample.learned.clear();
        Ok(())
    })
}

pub fn import_personal(path: &Path, cx: &mut App) -> Result<usize> {
    if let Some(service) = app::service(cx) {
        return service.import_personal(path);
    }
    Err(anyhow!("詞庫沒有載入"))
}

pub fn export_personal(path: &Path, cx: &mut App) -> Result<()> {
    if let Some(service) = app::service(cx) {
        return service.export_personal(path);
    }
    Err(anyhow!("詞庫沒有載入"))
}

/// The sample's 在候選字視窗顯示組字的 App (screenshots).
pub fn sample_apps(cx: &App) -> Option<Vec<String>> {
    cx.try_global::<SampleCell>().map(|sample| sample.0.borrow().apps.clone())
}

/// Replaces the sample's app list.
pub fn set_sample_apps(apps: Vec<String>, cx: &mut App) {
    let _ = with_sample(cx, |sample| {
        sample.apps = apps;
        Ok(())
    });
}

/// The sample's correction records (screenshots).
pub fn sample_corrections(cx: &App) -> Option<usize> {
    cx.try_global::<SampleCell>().map(|sample| sample.0.borrow().corrections)
}

/// The key that runs shortcut `command`, written for running text (「Shift+←」), or None when it has none or its
/// feature is off.
pub fn key_text(command: &str, cx: &App) -> Option<String> {
    key_label(command, cx).map(|label| crate::ui::keycap::prose(&label))
}

/// The key that runs shortcut `command` as the shortcuts store it (「⇧←」).
fn key_label(command: &str, cx: &App) -> Option<String> {
    let label = crate::ui::shortcuts::store::store(cx).active_label(command);
    (!label.is_empty()).then_some(label)
}

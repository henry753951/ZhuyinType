//! The reading channel in matching: accepted readings and corrections, one variant syllable per word, lexicon rows
//! winning over the channel, third-tone sandhi, the neutral tone, the settings, and the binary's checks.  The costs
//! themselves are checked against the Python reference (`zhuyintype_ml.training.lexicon.test_engine_channel`).

use std::sync::{Arc, Mutex};

use zhuyin_core::DataError;
use zhuyin_core::api::{CharacterModelSyllable, ComposerOptions, KeyInput, TypoTolerance};
use zhuyin_core::composer::{Composer, VariantScores};
use zhuyin_core::conversion::{CompositionToken, Request};
use zhuyin_core::engine::{Engine, EngineConfig, Match};
use zhuyin_core::reading_channel::{ChannelClass, ChannelSettings, ChannelTable};

const LEXICON: &str = "ㄕㄨˊ\t熟\t-8\nㄕㄡˊ\t受\t-9\nㄕㄨˊ ㄒㄧ\t熟悉\t-9\nㄒㄧ\t悉\t-10\nㄋㄧㄠˇ ㄏㄨㄟˋ\t鳥喙\t-14\n\
ㄏㄨㄟˋ\t喙\t-13\nㄓㄡˋ\t宙\t-9\nㄋㄧㄠˇ\t鳥\t-8\nㄒㄧㄢ ㄜˊ\t鮮蚵\t-14\nㄒㄧㄢ ㄎㄜ\t鮮蚵\t-16\nㄒㄧㄢ\t鮮\t-9\n\
ㄜˊ\t蚵\t-12\nㄎㄜ\t科\t-8\nㄋㄧˇ ㄏㄠˇ\t你好\t-6\nㄋㄧˇ\t你\t-7\nㄋㄧˊ\t泥\t-9\nㄏㄠˇ\t好\t-7\nㄏㄠˋ\t號\t-8\n\
ㄕㄨˊ ㄑㄧ\t熟期\t-12\nㄑㄧ\t期\t-7\nㄑㄧˊ\t其\t-7\nㄕㄣˊ ㄇㄜ˙\t什麼\t-6\nㄕㄣˊ\t神\t-7\nㄇㄛˊ\t模\t-9\n\
ㄧˇ\t以\t-7\nㄧˇ\t一\t-20\nㄧ ㄉㄧㄥˋ\t一定\t-6\nㄧˊ ㄉㄧㄥˋ\t一定\t-6.5\n";

fn table() -> ChannelTable {
    let rows: Vec<(char, &str, ChannelClass, f64)> = [
        ('什', "ㄕㄣˊ", ChannelClass::Lexicon, 1.0),
        ('喙', "ㄏㄨㄟˋ", ChannelClass::Lexicon, 0.9),
        ('喙', "ㄓㄡˋ", ChannelClass::Series, 0.1),
        ('悉', "ㄒㄧ", ChannelClass::Lexicon, 1.0),
        ('期', "ㄑㄧ", ChannelClass::Lexicon, 0.7),
        ('期', "ㄑㄧˊ", ChannelClass::Taiwan, 0.2),
        ('期', "ㄐㄧ", ChannelClass::Word, 0.1),
        ('熟', "ㄕㄨˊ", ChannelClass::Lexicon, 0.9),
        ('熟', "ㄕㄡˊ", ChannelClass::Taiwan, 0.1),
        ('蚵', "ㄜˊ", ChannelClass::Lexicon, 0.6),
        ('蚵', "ㄎㄜ", ChannelClass::Word, 0.3),
        ('蚵', "ㄏㄜˊ", ChannelClass::Unihan, 0.1),
        ('麼', "ㄇㄛˊ", ChannelClass::Lexicon, 0.7),
        ('麼', "ㄇㄚ", ChannelClass::Attested, 0.2),
        ('麼', "ㄇㄜ", ChannelClass::Cns, 0.1),
        ('鮮', "ㄒㄧㄢ", ChannelClass::Lexicon, 1.0),
    ]
    .into_iter()
    .map(|(c, r, k, p): (char, &str, ChannelClass, f64)| (c, r, k, p.ln()))
    .collect();
    let taiwan = [('期', "ㄑㄧ", "ㄑㄧˊ", 0.95f64.ln(), true), ('熟', "ㄕㄨˊ", "ㄕㄡˊ", 0.4f64.ln(), false)];
    ChannelTable::new(&rows, &taiwan, "什悉期熟麼鮮", 0.005, 0.01)
}

fn engine(settings: ChannelSettings) -> Engine {
    Engine::from_text_with_channel(LEXICON, &table(), EngineConfig { channel: settings, ..EngineConfig::default() })
}

fn tokens(readings: &[&str]) -> Vec<CompositionToken> {
    readings.iter().map(|r| CompositionToken::syllable(r, r, false)).collect()
}

fn find<'a>(matches: &'a [Match], word: &str) -> Option<&'a Match> {
    matches.iter().find(|m| &*m.entry.word == word)
}

fn matches(engine: &Engine, readings: &[&str]) -> Vec<Match> {
    engine.matches(&tokens(readings), 0, TypoTolerance::Off)
}

#[test]
fn accepted_readings_are_not_corrections() {
    let engine = engine(ChannelSettings::every_class());
    let single = matches(&engine, &["ㄕㄡˊ"]);
    let shu = find(&single, "熟").expect("熟 typed ㄕㄡˊ");
    let channel = shu.channel.expect("through the channel");
    assert_eq!((channel.class, channel.correction, channel.position), (ChannelClass::Taiwan, false, 0));
    assert!(shu.annotation.is_none());
    assert!((channel.cost - (0.9f64 / 0.1).ln()).abs() < 1e-12);
    assert!((shu.entry.penalty - channel.cost).abs() < 1e-12);
    assert_eq!(shu.entry.reading.as_deref(), Some("ㄕㄨˊ"));
    // In a word, taiwan.tsv's reading: −log 0.4, and the word wins.
    let word = matches(&engine, &["ㄕㄡˊ", "ㄒㄧ"]);
    let shuxi = find(&word, "熟悉").expect("熟悉 typed ㄕㄡˊ ㄒㄧ");
    assert_eq!(shuxi.channel.unwrap().class, ChannelClass::TaiwanWord);
    assert!((shuxi.channel.unwrap().cost + 0.4f64.ln()).abs() < 1e-12);
    let tokens = tokens(&["ㄕㄡˊ", "ㄒㄧ"]);
    let words = engine.convert(&Request::new(&tokens, &[], TypoTolerance::Off), &mut None);
    assert_eq!(words.iter().map(|w| &*w.text).collect::<String>(), "熟悉");
    assert!(!words[0].corrected && !words[0].reading_corrected);
    // The standard Taiwan reading costs nothing.
    let qi = matches(&engine, &["ㄕㄨˊ", "ㄑㄧˊ"]);
    assert!(find(&qi, "熟期").is_some_and(|m| m.channel.unwrap().cost == 0.0 && m.annotation.is_none()));
}

#[test]
fn corrections_are_annotated() {
    let engine = engine(ChannelSettings::every_class());
    let found = matches(&engine, &["ㄋㄧㄠˇ", "ㄓㄡˋ"]);
    let beak = find(&found, "鳥喙").expect("鳥喙 typed with its phonetic's reading");
    assert_eq!(beak.annotation.as_deref(), Some("ㄋㄧㄠˇ ㄏㄨㄟˋ"));
    assert!(beak.channel.unwrap().correction && beak.reading_corrected() && !beak.key_corrected);
    let tokens = tokens(&["ㄋㄧㄠˇ", "ㄓㄡˋ"]);
    let sentences = engine.sentences(&Request::new(&tokens, &[], TypoTolerance::Off), 4);
    let corrected = sentences.iter().find(|s| s.text() == "鳥喙").expect("in the n-best");
    assert!(corrected.corrected() && corrected.reading_corrected_only());
    // A familiar character has no misreadings in longer words; a single character keeps its other readings.
    assert!(find(&matches(&engine, &["ㄐㄧ", "ㄓㄨㄥ"]), "期中").is_none());
}

#[test]
fn rows_win_and_one_syllable_per_word() {
    let engine = engine(ChannelSettings::every_class());
    // 鮮蚵 has its own row for ㄒㄧㄢ ㄎㄜ: no channel variant beside it.
    let found = matches(&engine, &["ㄒㄧㄢ", "ㄎㄜ"]);
    assert!(find(&found, "鮮蚵").is_some_and(|m| m.channel.is_none()));
    assert!(find(&matches(&engine, &["ㄒㄧㄢ", "ㄏㄜˊ"]), "鮮蚵").is_some_and(|m| m.channel.is_some()));
    // Both syllables of 熟期 typed as variants: not through the channel.
    assert!(find(&matches(&engine, &["ㄕㄡˊ", "ㄑㄧˊ"]), "熟期").is_none());
    assert!(find(&matches(&engine, &["ㄕㄡˊ", "ㄑㄧ"]), "熟期").is_some());
}

#[test]
fn sandhi_and_the_neutral_tone() {
    let engine = engine(ChannelSettings::every_class());
    let found = matches(&engine, &["ㄋㄧˊ", "ㄏㄠˇ"]);
    let nihao = find(&found, "你好").expect("你好 typed with sandhi");
    let channel = nihao.channel.unwrap();
    assert_eq!((channel.class, channel.position), (ChannelClass::Sandhi, 0));
    assert!((channel.cost + 0.005f64.ln()).abs() < 1e-12 && nihao.annotation.is_none());
    // The single character too, read before the next syllable; never before another tone.
    assert!(find(&found, "你").is_some_and(|m| m.channel.is_some()));
    assert!(find(&matches(&engine, &["ㄋㄧˊ", "ㄏㄠˋ"]), "你").is_none());
    // 一 and 不 keep the lexicon's own sandhi rows: third-tone sandhi never reaches them.
    let yi = matches(&engine, &["ㄧˊ", "ㄏㄠˇ"]);
    assert!(find(&yi, "以").is_some() && find(&yi, "一").is_none());
    // A neutral tone typed with another reading of its character.
    let shenme = matches(&engine, &["ㄕㄣˊ", "ㄇㄛˊ"]);
    assert_eq!(find(&shenme, "什麼").unwrap().channel.unwrap().class, ChannelClass::Neutral);
}

#[test]
fn settings_and_off() {
    let off = Engine::from_text(LEXICON, None, None, EngineConfig::default());
    assert!(off.reading_channel().is_none());
    assert!(find(&matches(&off, &["ㄕㄡˊ", "ㄒㄧ"]), "熟悉").is_none());
    assert!(matches(&off, &["ㄕㄡˊ"]).iter().all(|m| m.channel.is_none()));
    // Turned off at run time: exactly as without a channel; on again, the variants come back.
    let switched = engine(ChannelSettings::every_class());
    let before = matches(&switched, &["ㄕㄡˊ", "ㄒㄧ"]);
    switched.set_reading_channel_enabled(false);
    assert!(switched.reading_channel().is_none() && switched.has_reading_channel());
    assert_eq!(matches(&switched, &["ㄕㄡˊ", "ㄒㄧ"]), matches(&off, &["ㄕㄡˊ", "ㄒㄧ"]));
    switched.set_reading_channel_enabled(true);
    assert_eq!(matches(&switched, &["ㄕㄡˊ", "ㄒㄧ"]), before);
    let mut settings = ChannelSettings::every_class();
    settings.set_uses(ChannelClass::Sandhi, false);
    settings.set_correction(ChannelClass::TaiwanWord, true);
    let engine = engine(settings.clone());
    assert!(find(&matches(&engine, &["ㄋㄧˊ", "ㄏㄠˇ"]), "你好").is_none());
    let shuxi = matches(&engine, &["ㄕㄡˊ", "ㄒㄧ"]);
    assert_eq!(find(&shuxi, "熟悉").unwrap().annotation.as_deref(), Some("ㄕㄨˊ ㄒㄧ"));
    settings.offsets[ChannelClass::TaiwanWord as usize] = 9.0;
    let engine = self::engine(settings);
    assert!(find(&matches(&engine, &["ㄕㄡˊ", "ㄒㄧ"]), "熟悉").is_none(), "above max_cost");
}

#[test]
fn options_are_capped() {
    let one = ChannelSettings { options_per_syllable: 1, ..ChannelSettings::every_class() };
    let engine = engine(one);
    assert!(engine.reading_channel().unwrap().options("ㄎㄜ").len() <= 1);
    assert_eq!(self::engine(ChannelSettings::every_class()).reading_channel().unwrap().options("ㄕㄡˊ").len(), 1);
}

#[test]
fn invalid_binaries_are_refused() {
    let directory = std::env::temp_dir().join(format!("zhuyin-channel-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("channel.bin");
    for data in [&b""[..], &b"ZTCB\x01\0\0\0"[..], &[0u8; 96][..]] {
        std::fs::write(&path, data).unwrap();
        assert!(matches!(ChannelTable::open(&path), Err(DataError::InvalidChannel)));
    }
    let _ = std::fs::remove_dir_all(&directory);
}

type Calls = Arc<Mutex<Vec<(Vec<CharacterModelSyllable>, Vec<Vec<String>>)>>>;

/// Types `keys` with a character model that answers −1 everywhere and records each call.
fn model_calls(mode: VariantScores, window: f64, keys: &str) -> Vec<(Vec<CharacterModelSyllable>, Vec<Vec<String>>)> {
    let engine = Arc::new(engine(ChannelSettings::every_class()));
    let options = ComposerOptions { typo_tolerance: TypoTolerance::Off, ..ComposerOptions::default() };
    let mut composer = Composer::new(engine, options);
    let mut tuning = composer.tuning();
    tuning.variant_scores = mode;
    tuning.variant_window = window;
    composer.set_tuning(tuning);
    let calls = Calls::default();
    let record = calls.clone();
    composer.set_character_scorer(Some(Box::new(move |syllables, lists| {
        record.lock().unwrap().push((syllables.to_vec(), lists.to_vec()));
        Some(lists.iter().map(|l| vec![-1.0; l.len()]).collect())
    })));
    for key in keys.chars() {
        composer.handle(KeyInput::Char(key));
    }
    calls.lock().unwrap().clone()
}

#[test]
fn the_character_model_sees_variant_characters() {
    // ㄕㄡˊ: 熟 comes only through the channel; it is in the list (ranked without the variant's cost).
    let standard = model_calls(VariantScores::Standard, f64::INFINITY, "g.6");
    let (syllables, lists) = &standard[standard.len() - 2];
    assert_eq!((syllables[0].base.as_deref(), syllables[0].tone), (Some("ㄕㄡ"), 2));
    assert!(lists[0].contains(&"熟".to_owned()));
    // (a) one more call with the lexicon's syllable, asking about 熟 alone.
    let (syllables, lists) = standard.last().unwrap();
    assert_eq!((syllables[0].base.as_deref(), syllables[0].tone, syllables[0].cost_bucket), (Some("ㄕㄨ"), 2, 0));
    assert_eq!(lists[0], ["熟"]);
    // The other modes ask once; an accepted reading needs no bucket.
    for mode in [VariantScores::Best, VariantScores::Zero, VariantScores::Typed] {
        let calls = model_calls(mode, f64::INFINITY, "g.6");
        assert!(calls.iter().all(|(s, _)| s[0].base.as_deref() == Some("ㄕㄡ") && s[0].cost_bucket == 0), "{mode:?}");
    }
    // A correction, as typed: one more call with its cost bucket (−log P ≤ 6: bucket 1), as v3's training noise.
    let typed = model_calls(VariantScores::Typed, f64::INFINITY, "sul35.4");
    let (syllables, lists) = typed.last().unwrap();
    assert_eq!((syllables[1].base.as_deref(), syllables[1].cost_bucket), (Some("ㄓㄡ"), 1));
    assert_eq!(lists[1], ["喙"]);
    // Outside the window 喙 (−13, far from 宙 at −9) is ranked with its cost; when it is still listed, a correction
    // as typed keeps its cost bucket, as in v3's training noise.
    let windowed = model_calls(VariantScores::Typed, 3.0, "sul35.4");
    let listed = windowed.iter().any(|(_, lists)| lists.get(1).is_some_and(|l| l.contains(&"喙".to_owned())));
    let bucketed = windowed.iter().any(|(s, lists)| {
        s.get(1).is_some_and(|x| x.base.as_deref() == Some("ㄓㄡ") && x.cost_bucket == 1) && lists[1] == ["喙"]
    });
    assert!(listed, "喙 stays in the list at its cost");
    assert!(bucketed);
    // The other modes keep the window: no bucket outside it.
    for mode in [VariantScores::Standard, VariantScores::Best, VariantScores::Zero] {
        let calls = model_calls(mode, 3.0, "sul35.4");
        assert!(calls.iter().all(|(s, _)| s.iter().all(|x| x.cost_bucket == 0)), "{mode:?}");
    }
}

#[test]
fn accepted_readings_reach_the_model_with_bucket_zero_as_typed() {
    // ㄕㄡˊ for 熟 is accepted (taiwan): as typed, one call, bucket 0, at any window.
    for window in [f64::INFINITY, 3.0, 0.0] {
        let calls = model_calls(VariantScores::Typed, window, "g.6");
        assert_eq!(calls.iter().filter(|(s, _)| s.iter().any(|x| x.cost_bucket != 0)).count(), 0, "{window}");
    }
}

//! The `ZhuyinCoreChecks` checks of the engine, personal data and conversion that need no composer (the composer's
//! own checks come with it in phase 3).  Keys are turned into syllable tokens the way the composer does for valid
//! syllables typed with a tone key or Space.

mod common;

use std::fmt::Write as _;
use std::path::Path;

use common::{LmTrigram, TempDir, bigram, bigram_backoff, lexicon_fixture, lm_fixture};
use zhuyin_core::api::TypoTolerance;
use zhuyin_core::conversion::{CompositionToken, Request};
use zhuyin_core::engine::{CommittedWord, Engine, EngineConfig, EngineError};
use zhuyin_core::keyboard;
use zhuyin_core::{LanguageModel as _, LanguageModelHistory};

const TINY: &str = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄅㄚ\t吧\t-2.4\nㄆㄚ\t趴\t-2\nㄇㄚ\t媽\t-2\nㄅㄚ ㄆㄚ\t八趴\t-1\n\
ㄅㄚ ㄆㄚ\t巴趴\t-1.2\nㄅㄚ ㄆㄚ ㄇㄚ\t八趴媽\t-0.5\nㄦ\t兒\t-2\n";

fn tokens(keys: &str) -> Vec<CompositionToken> {
    let mut result = vec![];
    let mut typed = String::new();
    for key in keys.chars() {
        typed.push(key);
        let tone = keyboard::tone(key);
        if tone.is_some() || key == ' ' {
            let mut slots = [None; 3];
            for k in typed.chars() {
                if let Some(symbol) = keyboard::symbol(k) {
                    slots[keyboard::slot(symbol).unwrap()] = Some(symbol);
                }
            }
            let reading = slots.iter().flatten().collect::<String>() + tone.unwrap_or("");
            result.push(CompositionToken::syllable(&reading, &typed, false));
            typed.clear();
        }
    }
    result
}

fn convert_with(engine: &Engine, keys: &str, typo: TypoTolerance) -> String {
    let tokens = tokens(keys);
    engine.convert(&Request::new(&tokens, &[], typo), &mut None).iter().map(|w| w.text.to_string()).collect()
}

fn convert(engine: &Engine, keys: &str) -> String {
    convert_with(engine, keys, TypoTolerance::Off)
}

fn text(lexicon: &str, model: Option<&Path>) -> Engine {
    Engine::from_text(lexicon, None, model, EngineConfig::default())
}

#[test]
fn viterbi_and_tolerance() {
    // "Viterbi picks three-syllable word", "phrase-only syllables complete", "typed fourth tone matches neutral phrase
    // ending", "neutral lexicon accepts any typed tone", "typed neutral accepts another tone".
    let engine = text(TINY, None);
    assert_eq!(convert(&engine, "18 q8 a8 "), "八趴媽");
    let phrase_only = text("ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\t-2\n", None);
    assert_eq!(convert(&phrase_only, "vu,4vu,7"), "謝謝");
    assert_eq!(convert(&phrase_only, "vu,4vu,4"), "謝謝");
    let neutral = text("ㄅㄚ˙\t吧\t-2\n", None);
    for keys in ["18 ", "186", "183", "184", "187"] {
        assert!(neutral.is_valid_syllable(tokens(keys)[0].reading().unwrap()));
        assert_eq!(convert(&neutral, keys), "吧", "{keys}");
    }
    let fourth = text("ㄅㄚˋ\t爸\t-2\n", None);
    assert_eq!(convert(&fourth, "187"), "爸");
    assert!(!fourth.is_valid_syllable("ㄅㄚˇ"), "different non-neutral tones never match");
    // Penalties: 0.4 for a missing neutral tone, 1.2 for a typed one, bracketed by competitors 0.01 on either side.
    for (typed, source, keys, penalty) in [
        ("ㄅㄚ", "ㄅㄚ˙", "18 ", 0.4),
        ("ㄅㄚ˙", "ㄅㄚ", "187", 1.2),
        ("ㄅㄚ ㄆㄚ", "ㄅㄚ˙ ㄆㄚ˙", "18 q8 ", 0.8),
        ("ㄅㄚ˙ ㄆㄚ˙", "ㄅㄚ ㄆㄚ", "187q87", 2.4),
    ] {
        let (word, rival) = if typed.contains(' ') { ("甲乙", "丙丁") } else { ("甲", "丙") };
        for delta in [-0.01, 0.01] {
            let engine = text(&format!("{source}\t{word}\t-2\n{typed}\t{rival}\t{}\n", -2.0 - penalty + delta), None);
            assert_eq!(convert(&engine, keys), if delta < 0.0 { word } else { rival }, "{penalty} penalty");
        }
    }
    // "a word's likelier tolerant reading beats its rare exact one", "equally likely readings: the exact one".
    assert_eq!(convert(&text("ㄅㄚ\t甲\t-5\nㄅㄚ˙\t甲\t0\nㄅㄚ\t乙\t-4\n", None), "18 "), "甲");
    assert_eq!(convert(&text("ㄅㄚ\t甲\t-1\nㄅㄚ˙\t甲\t-1\nㄅㄚ\t乙\t-1.2\n", None), "18 "), "甲");
}

#[test]
fn language_model_conversions() {
    let temp = TempDir::new("engine-lm");
    let lm = temp.dir("lm");
    std::fs::write(lm.join("unigram.tsv"), "八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n").unwrap();
    std::fs::write(lm.join("bigram.tsv"), "<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n趴\t</s>\t-0.1\n")
        .unwrap();
    std::fs::write(lm.join("backoff.tsv"), "八\t-0.4\n").unwrap();
    let lexicon = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄆㄚ\t趴\t-2\n";
    // "Viterbi keeps last-word state", "neutral penalty is subtracted after bigram scoring".
    assert_eq!(convert(&text(lexicon, Some(&lm)), "18 q8 "), "巴趴");
    assert_eq!(convert(&text("ㄅㄚ\t八\t-2\nㄅㄚ˙\t巴\t-9\n", Some(&lm)), "187"), "巴");
    // "adjacent lm discovered", "explicit nil disables LM".
    let lexicon_path = temp.path().join("lexicon.tsv");
    std::fs::write(&lexicon_path, lexicon).unwrap();
    assert_eq!(convert(&Engine::open(&lexicon_path, None, EngineConfig::default()).unwrap(), "18 q8 "), "巴趴");
    let disabled = Engine::open_with_model(&lexicon_path, None, None, EngineConfig::default()).unwrap();
    assert_eq!(convert(&disabled, "18 q8 "), "八趴");
    // "end boundary participates in best path", "incomplete optional LM safely falls back to unigram".
    std::fs::write(lm.join("bigram.tsv"), "<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t</s>\t-10\n巴\t</s>\t-0.1\n").unwrap();
    assert_eq!(convert(&text(lexicon, Some(&lm)), "18 "), "巴");
    std::fs::remove_file(lm.join("backoff.tsv")).unwrap();
    assert_eq!(convert(&text(lexicon, Some(&lm)), "18 "), "八");

    // The binary fixture: "trigram changes best conversion", "bigram alone prefers the other conversion", "unknown
    // calibration participates in conversion", "missing calibration keeps lexicon probability", "OOV word becomes
    // unk context for following word", "new user phrase resolves its ID before decoding".
    let binary = temp.dir("binary");
    let unigrams =
        [("</s>", -1.0f32), ("<unk>", -8.0), ("八", -2.0), ("巴", -2.3), ("趴", -2.0), ("媽", -3.0), ("麻", -3.0)];
    let bigrams = [
        bigram("<s>", "八", -0.1),
        bigram("<s>", "巴", -0.5),
        bigram_backoff("八", "趴", -0.2, -0.7),
        bigram_backoff("巴", "趴", -0.2, -0.8),
        bigram("趴", "媽", -0.1),
        bigram("趴", "麻", -1.0),
        bigram("媽", "</s>", -0.1),
        bigram("麻", "</s>", -0.1),
        bigram("<unk>", "趴", -0.05),
    ];
    let trigrams = [
        LmTrigram { first: "八", second: "趴", third: "媽", logp: -4.0 },
        LmTrigram { first: "八", second: "趴", third: "麻", logp: -0.05 },
        LmTrigram { first: "巴", second: "趴", third: "媽", logp: -0.01 },
    ];
    let backoffs = [("八", -0.4f32), ("趴", -0.2)];
    std::fs::write(binary.join("lm.bin"), lm_fixture(3, &unigrams, &backoffs, &bigrams, &trigrams)).unwrap();
    std::fs::write(binary.join("lm.json"), r#"{"unknown_word_shift":-2.3}"#).unwrap();
    let binary_lexicon = format!("{lexicon}ㄇㄚ\t媽\t-3\nㄇㄚ\t麻\t-3\n");
    assert_eq!(convert(&text(&binary_lexicon, Some(&binary)), "18 q8 a8 "), "八趴麻");
    let competition = "ㄅㄚ\t八\t-2\nㄅㄚ\t新\t-0.05\n";
    assert_eq!(convert(&text(competition, Some(&binary)), "18 "), "八");
    let oov = text("ㄅㄚ\t新\t-0.2\nㄆㄚ\t趴\t-2\n", Some(&binary));
    assert_eq!(convert(&oov, "18 q8 "), "新趴");
    oov.add_user_phrase("新", "ㄅㄚ").unwrap();
    assert_eq!(convert(&oov, "18 q8 "), "新趴");
    std::fs::remove_file(binary.join("lm.json")).unwrap();
    assert_eq!(convert(&text(competition, Some(&binary)), "18 "), "新");
    std::fs::write(binary.join("lm.bin"), lm_fixture(2, &unigrams, &backoffs, &bigrams, &[])).unwrap();
    assert_eq!(convert(&text(&binary_lexicon, Some(&binary)), "18 q8 a8 "), "八趴媽");
    // "corrupt binary falls back without crash".
    std::fs::write(binary.join("lm.bin"), b"ZTLM").unwrap();
    assert_eq!(convert(&text(lexicon, Some(&binary)), "18 q8 "), "八趴");
}

#[test]
fn mapped_lexicon_engine() {
    // checkMappedLexicon: "mapped reading prior and neutral-tone lookup", "mapped reverse index ordered by original
    // score", "mapped best-reading fallback", "personal phrase overlays mapped entry", "removing overlay preserves
    // mapped entries", "mapped lexicon accepts new personal syllables", "absent binary falls back to TSV".
    let temp = TempDir::new("mapped-engine");
    let tsv = temp.path().join("lexicon.tsv");
    std::fs::write(temp.path().join("lexicon.bin"), lexicon_fixture()).unwrap();
    let engine = Engine::open_with_model(&tsv, None, None, EngineConfig::default()).unwrap();
    assert_eq!(convert(&engine, "18 "), "吧");
    assert_eq!(engine.readings("吧"), ["ㄅㄚ˙", "ㄅㄚ"]);
    assert_eq!(engine.readings("八吧"), ["ㄅㄚ ㄅㄚ˙"]);
    engine.add_user_phrase("八", "ㄅㄚ").unwrap();
    assert_eq!(convert(&engine, "18 "), "八");
    engine.remove_user_phrase("八", "ㄅㄚ");
    assert_eq!(convert(&engine, "18 "), "吧");
    engine.add_user_phrase("謝謝", "ㄒㄧㄝˋ ㄒㄧㄝ˙").unwrap();
    assert_eq!(convert(&engine, "vu,4vu,4"), "謝謝");
    engine.remove_user_phrase("謝謝", "ㄒㄧㄝˋ ㄒㄧㄝ˙");
    assert!(!engine.is_valid_syllable("ㄒㄧㄝ"), "removed personal syllables leave mapped index intact");
    assert!(!engine.export_personal_data().is_empty());
    std::fs::write(temp.path().join("lexicon.bin"), &lexicon_fixture()[..200]).unwrap();
    assert!(Engine::open_with_model(&tsv, None, None, EngineConfig::default()).is_err(), "reject truncated lexicon");
    std::fs::remove_file(temp.path().join("lexicon.bin")).unwrap();
    std::fs::write(&tsv, "ㄅㄚ\t八\t-3\n").unwrap();
    assert_eq!(convert(&Engine::open_with_model(&tsv, None, None, EngineConfig::default()).unwrap(), "18 "), "八");
}

#[test]
fn user_phrases_and_persistence() {
    // "reject invalid phrase", "bad disk rows ignored", "loaded phrase supplies internal syllables", "debounced
    // persistence round-trip", "forget and remove persist", "learning capped at 20000", "cap drops lowest boosts",
    // "90-day half-life reduces boost below 1.0", "fresh count 1 gives 2 ln(2) boost".
    let engine = text(TINY, None);
    for (word, reading) in [
        ("", "ㄅㄚ"),
        ("八", ""),
        ("八", "foo"),
        ("八趴", "ㄅㄚ"),
        ("八\n", "ㄅㄚ"),
        ("八", "ㄅㄚˊˋ"),
        ("八", "ㄅㄆㄚ"),
        ("八", "ㄚㄅ"),
    ] {
        assert_eq!(engine.add_user_phrase(word, reading), Err(EngineError::InvalidPhrase), "{word}/{reading}");
    }
    let temp = TempDir::new("persistence");
    std::fs::write(temp.path().join("learned.tsv"), "bad\nㄅㄚ\t八\tNaN\t0\nㄅㄚ\t巴\t1\tNaN\n").unwrap();
    std::fs::write(temp.path().join("user_phrases.tsv"), "bad\nnot-reading\t八\n").unwrap();
    let persisted = Engine::from_text(TINY, Some(temp.path()), None, EngineConfig::default());
    assert!(persisted.learned_entries().is_empty() && persisted.user_phrases().is_empty());
    let neutral = temp.dir("neutral");
    std::fs::write(neutral.join("user_phrases.tsv"), "ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\n").unwrap();
    let loaded = Engine::from_text("", Some(&neutral), None, EngineConfig::default());
    assert_eq!(convert(&loaded, "vu,4vu,7"), "謝謝");
    assert_eq!(convert(&loaded, "vu,4vu,4"), "謝謝");

    persisted.add_user_phrase("媽八", "ㄇㄚ ㄅㄚ").unwrap();
    let picked =
        CommittedWord { range: 0..1, text: "巴".into(), reading: "ㄅㄚ".into(), word_id: None, picked: true };
    persisted.learn(&[picked], 1, &[]);
    persisted.flush_personal_data().unwrap();
    let reloaded = Engine::from_text(TINY, Some(temp.path()), None, EngineConfig::default());
    assert!(reloaded.learned_entries().iter().any(|(k, _)| k.word == "巴") && reloaded.user_phrases().len() == 1);
    assert_eq!(convert(&reloaded, "18 "), "巴", "persisted boost used");
    persisted.forget_all_learned();
    persisted.remove_user_phrase("媽八", "ㄇㄚ ㄅㄚ");
    persisted.flush_personal_data().unwrap();
    let forgotten = Engine::from_text(TINY, Some(temp.path()), None, EngineConfig::default());
    assert!(forgotten.learned_entries().is_empty() && forgotten.user_phrases().is_empty());

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    let decay = temp.dir("decay");
    let lexicon = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-3\n";
    std::fs::write(decay.join("learned.tsv"), format!("ㄅㄚ\t巴\t1\t{}\n", now - 90.0 * 86400.0)).unwrap();
    assert_eq!(convert(&Engine::from_text(lexicon, Some(&decay), None, EngineConfig::default()), "18 "), "八");
    std::fs::write(decay.join("learned.tsv"), format!("ㄅㄚ\t巴\t1\t{now}\n")).unwrap();
    assert_eq!(convert(&Engine::from_text(lexicon, Some(&decay), None, EngineConfig::default()), "18 "), "巴");

    let capacity = temp.dir("capacity");
    let rows = (0..20_002u32).fold(String::new(), |mut rows, i| {
        writeln!(rows, "ㄅㄚ\t{}\t{}\t{now}", char::from_u32(0x4e00 + i).unwrap(), i + 1).unwrap();
        rows
    });
    std::fs::write(capacity.join("learned.tsv"), rows).unwrap();
    let capped = Engine::from_text(TINY, Some(&capacity), None, EngineConfig::default());
    let entries = capped.learned_entries();
    assert_eq!(entries.len(), 20_000);
    assert!(!entries.iter().any(|(k, _)| k.word == "一" || k.word == "丁"));
}

#[test]
fn typing_habits_export_and_import() {
    // "personal slips get cheaper with each correction", "slips travel with an export", "a foreign file is
    // refused", "exclusions are listed", "exclusions travel with an export".
    let engine = text("ㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\nㄉㄜˊ\t得\t-2\n", None);
    let cost = |engine: &Engine| {
        engine.typo_alternatives("rk4").into_iter().find(|a| a.0 == "ㄍㄜˋ").map_or(f64::INFINITY, |a| a.1)
    };
    assert!(!engine.typo_alternatives("ek4").iter().any(|a| a.0 == "ㄍㄜˋ"));
    engine.record_typos(&[("rk4".into(), "ㄍㄜˋ".into())]);
    let before = cost(&engine);
    engine.record_typos(&[("rk4".into(), "ㄍㄜˋ".into()), ("rk4".into(), "ㄍㄜˋ".into())]);
    assert!(cost(&engine) < before);
    assert!(engine.export_personal_data().contains("[typos]\nr\te\t3.0"));
    assert_eq!(engine.import_personal_data(b"not an export"), Err(EngineError::NotAnExport));
    let picked =
        CommittedWord { range: 0..1, text: "特".into(), reading: "ㄍㄜˋ".into(), word_id: None, picked: true };
    engine.learn(&[picked], 1, &[]);
    assert!(
        engine.learned_entries().iter().any(|(k, _)| k.reading == "ㄍㄜˋ" && k.word == "特"),
        "correction learns typed reading"
    );
    assert_eq!(convert(&engine, "ek4"), "特", "repeat typed error recalls learned word directly");
    let receiving = text("ㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\n", None);
    let rows = receiving.import_personal_data(engine.export_personal_data().as_bytes()).unwrap();
    assert!(rows > 0);
    assert_eq!(convert(&receiving, "ek4"), "特", "imported memory converts the same");
    receiving.exclude_from_learning("特", "ㄍㄜˋ");
    assert_ne!(convert(&receiving, "ek4"), "特", "excluding forgets the word");
    let picked =
        CommittedWord { range: 0..1, text: "特".into(), reading: "ㄍㄜˋ".into(), word_id: None, picked: true };
    receiving.learn(&[picked], 1, &[]);
    assert_ne!(convert(&receiving, "ek4"), "特", "an excluded word is not learned from a pick");
    assert_eq!(receiving.excluded_entries().iter().map(|k| k.word.as_str()).collect::<Vec<_>>(), ["特"]);
    let reimported = text("ㄍㄜˋ\t各\t-2\n", None);
    reimported.import_personal_data(receiving.export_personal_data().as_bytes()).unwrap();
    assert_eq!(reimported.excluded_entries().len(), 1);
}

#[test]
fn personal_model_settles_its_own_context() {
    // The engine half of "Personal language model: one correction settles its own context and stays out of
    // others": picking 操 and 媽 (and weakening what they replaced, as the composer does) is remembered for the same
    // input and the same words inside a longer input, not for 你ㄇㄚ elsewhere.
    let temp = TempDir::new("personal");
    let lm = temp.dir("lm");
    std::fs::write(lm.join("unigram.tsv"), "肏\t-9\n操\t-10\n你\t-5\n我\t-5\n嗎\t-7\n媽\t-8\n</s>\t-3\n").unwrap();
    std::fs::write(lm.join("bigram.tsv"), "你\t嗎\t-0.5\n你\t媽\t-7\n").unwrap();
    std::fs::write(lm.join("backoff.tsv"), "你\t-1\n").unwrap();
    let lexicon = "ㄘㄠˋ\t肏\t-9\nㄘㄠˋ\t操\t-10\nㄋㄧˇ\t你\t-5\nㄨㄛˇ\t我\t-5\nㄇㄚ\t嗎\t-7\nㄇㄚ\t媽\t-8\n";
    let engine = text(lexicon, Some(&lm));
    engine.set_clock(|| 1_800_000_000.0);
    assert_eq!(convert(&engine, "hl4su3a8 "), "肏你嗎");
    let before = engine.convert(&Request::new(&tokens("hl4su3a8 "), &[], TypoTolerance::Off), &mut None);
    let model = engine.language_model();
    let start = LanguageModelHistory::default();
    engine.weaken(&before[2].text, &before[2].reading, before[2].word_id, before[1].history);
    engine.weaken(&before[0].text, &before[0].reading, before[0].word_id, Some(start));
    let committed = [
        CommittedWord {
            range: 0..1,
            text: "操".into(),
            reading: "ㄘㄠˋ".into(),
            word_id: model.word_id("操"),
            picked: true,
        },
        CommittedWord {
            range: 1..2,
            text: "你".into(),
            reading: "ㄋㄧˇ".into(),
            word_id: model.word_id("你"),
            picked: false,
        },
        CommittedWord {
            range: 2..3,
            text: "媽".into(),
            reading: "ㄇㄚ".into(),
            word_id: model.word_id("媽"),
            picked: true,
        },
    ];
    engine.learn(&committed, 3, &[]);
    assert_eq!(convert(&engine, "hl4su3a8 "), "操你媽");
    assert!(convert(&engine, "ji3hl4su3a8 ").ends_with("操你媽"));
    assert_eq!(convert(&engine, "su3a8 "), "你嗎");
    assert_eq!(convert(&engine, "ji3su3a8 "), "我你嗎");
    let fresh = text(lexicon, Some(&lm));
    assert_eq!(convert(&fresh, "hl4su3a8 "), "肏你嗎", "empty memory leaves conversions unchanged");
}

#[test]
fn matches_rejects_out_of_bounds_start() {
    let engine = text(TINY, None);
    let tokens = tokens("18 ");
    assert!(engine.matches(&tokens, tokens.len(), TypoTolerance::Off).is_empty());
    assert!(engine.matches(&tokens, usize::MAX, TypoTolerance::Off).is_empty());
}

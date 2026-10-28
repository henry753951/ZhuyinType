//! The real-lexicon section of `ZhuyinCoreChecks` (context across commits, deletion following, candidate lists,
//! key-run decoding, typing habits, export/import, real conversions, latency) and the personal-model fixture through
//! the composer.  The real-data checks skip without `ZHUYIN_DATA`.  Latency targets are asserted in release builds
//! unless `ZHUYIN_CHECKS_TIMING=warn`, as in Swift; debug builds only report them.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use common::TempDir;
use zhuyin_core::api::{ComposerOptions, KeyDecoderMethod, KeyInput, PunctuationKeys, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::key_channel;
use zhuyin_core::tsv::scan_tsv;

/// The tests of this file run one at a time: latency is measured without the other tests competing for the CPU.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn legacy() -> ComposerOptions {
    ComposerOptions {
        typo_tolerance: TypoTolerance::Off,
        punctuation_keys: PunctuationKeys::Mac,
        ..ComposerOptions::default()
    }
}

fn type_keys(keys: &str, c: &mut Composer) {
    for k in keys.chars() {
        c.handle(if k == ' ' { KeyInput::Space } else { KeyInput::Char(k) });
    }
}

fn composer_with(e: &Arc<Engine>, keys: &str, options: ComposerOptions) -> Composer {
    let mut c = Composer::new(e.clone(), options);
    type_keys(keys, &mut c);
    c
}

fn text(c: &Composer) -> String {
    c.presentation().text.clone()
}

fn items(c: &Composer) -> Vec<String> {
    c.presentation().candidates.as_ref().map(|l| l.items().iter().map(|i| i.text.clone()).collect()).unwrap_or_default()
}

/// Keys for a reading on the standard layout (Space after a first tone).
fn keys_for_reading(reading: &str) -> String {
    reading.split(' ').filter(|p| !p.is_empty()).filter_map(key_channel::keys_for_reading).collect()
}

/// Swift's `ordered[Int(Double(count - 1) * p)]`.
fn percentile_index(count: usize, p: f64) -> usize {
    // Non-negative and below `count` by construction.
    #[allow(clippy::cast_sign_loss)]
    let index = ((count - 1) as f64 * p) as usize;
    index
}

fn timing_enforced() -> bool {
    !cfg!(debug_assertions) && std::env::var("ZHUYIN_CHECKS_TIMING").as_deref() != Ok("warn")
}

fn timed<T>(body: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let result = body();
    (result, start.elapsed().as_secs_f64() * 1000.0)
}

fn latency(name: &str, ms: f64, target: f64) {
    eprintln!("TIMING {name}: {ms:.3} ms (target <{target} ms)");
    if timing_enforced() {
        assert!(ms < target, "{name} under {target} ms");
    }
}

fn open(data: &Path) -> Arc<Engine> {
    Arc::new(Engine::open(&data.join("lexicon.tsv"), None, EngineConfig::default()).unwrap())
}

#[test]
#[allow(clippy::too_many_lines)]
fn real_lexicon_behaviour() {
    let _serial = serial();
    let Some(data) = common::data_directory() else { return };
    let (real, init_ms) = timed(|| open(&data));
    let (_, lexicon_ms) =
        timed(|| Engine::open_with_model(&data.join("lexicon.tsv"), None, None, EngineConfig::default()).unwrap());
    eprintln!("TIMING real lexicon + discovered LM init: {init_ms:.3} ms; lexicon alone {lexicon_ms:.3} ms");
    latency("real lexicon init", lexicon_ms, 400.0);
    let unlearned = ComposerOptions { learning_enabled: false, ..legacy() };
    let fresh = composer_with(&real, "g4", unlearned.clone());
    let mut continuing = composer_with(&real, "ji3ap7", unlearned.clone());
    assert_eq!(continuing.commit_all(), "我們", "context commit");
    type_keys("g4", &mut continuing);
    assert_eq!(text(&continuing), "是", "ㄕˋ after committed 我們 reads 是");
    assert_ne!(text(&fresh), "是", "the continuation check needs a reading the sentence start converts differently");
    continuing.reset();
    assert!(!continuing.handle(KeyInput::Left).handled, "left passes through with nothing composed");
    type_keys("g4", &mut continuing);
    assert_eq!(text(&continuing), text(&fresh), "a passed-through key restarts the sentence");
    let mut restarted = composer_with(&real, "ji3ap7", unlearned.clone());
    restarted.commit_all();
    restarted.break_context();
    type_keys("g4", &mut restarted);
    assert_eq!(text(&restarted), text(&fresh), "breakContext restarts the sentence");
    let mut deleting = composer_with(&real, "ji3ap7g4", unlearned.clone());
    assert_eq!(deleting.commit_all(), "我們是", "deletion context commit");
    assert!(!deleting.handle(KeyInput::Backspace).handled, "backspace with nothing composed passes through");
    type_keys("g4", &mut deleting);
    assert_eq!(text(&deleting), "是", "after deleting 是, ㄕˋ continues from 我們");
    deleting.reset();
    for _ in 0..2 {
        deleting.handle(KeyInput::Backspace);
    }
    type_keys("g4", &mut deleting);
    assert_eq!(text(&deleting), text(&fresh), "deleting the whole commit leaves no context");

    let correcting = open(&data);
    let mut corrector = composer_with(&correcting, "ek4", legacy());
    let first = corrector.commit_all();
    corrector.handle(KeyInput::Backspace);
    type_keys("ek4", &mut corrector);
    corrector.handle(KeyInput::Down);
    let other = items(&corrector).iter().position(|t| *t != first && t.chars().count() == 1).unwrap();
    let picked = items(&corrector)[other].clone();
    corrector.select_candidate(other);
    corrector.commit_all();
    assert_eq!(text(&composer_with(&correcting, "ek4", legacy())), picked, "a deleted and retyped word is remembered");

    let mut listing = composer_with(&real, "ji3ap7", unlearned.clone());
    listing.handle(KeyInput::Home);
    listing.handle(KeyInput::Down);
    let full = listing.presentation().candidates.clone().unwrap().items().to_vec();
    let first_correction = full.iter().position(|i| i.annotation.is_some()).unwrap_or(full.len());
    assert!(
        full[first_correction..].iter().all(|i| i.annotation.is_some())
            || full.iter().take(1).any(|i| i.annotation.is_some()),
        "corrections follow every word for the keys as typed"
    );
    assert!(
        full.iter().any(|i| i.text == "我們") && full.iter().any(|i| i.text == "我"),
        "the full list has phrases and characters"
    );
    listing.handle(KeyInput::ShiftDown);
    let single = listing.presentation().candidates.clone().unwrap().items().to_vec();
    assert!(
        single.iter().all(|i| i.text.chars().count() == 1 && i.annotation.is_none()) && single[0].text == "我",
        "Shift-↓ in the window lists only the character, for the keys as typed"
    );
    listing.handle(KeyInput::ShiftDown);
    assert_eq!(
        listing.presentation().candidates.clone().unwrap().items().to_vec(),
        full,
        "Shift-↓ again returns to the full list"
    );
    listing.reset();
    type_keys("su3cl3", &mut listing);
    listing.handle(KeyInput::ShiftDown);
    assert_eq!(items(&listing), ["好", "郝"], "Shift-↓ opens the single-character list");

    let decoded = |method: KeyDecoderMethod, keys: &str, gaps: &[(usize, f64)]| {
        let options =
            ComposerOptions { key_decoder: method, typo_tolerance: TypoTolerance::Standard, ..unlearned.clone() };
        let mut c = Composer::new(real.clone(), options);
        let mut time = 100.0;
        for (index, key) in keys.chars().enumerate() {
            time += gaps.iter().find(|g| g.0 == index).map_or(0.12, |g| g.1);
            c.handle_at(if key == ' ' { KeyInput::Space } else { KeyInput::Char(key) }, time);
        }
        text(&c)
    };
    assert!(
        decoded(KeyDecoderMethod::Slots, "rupwu0 ", &[]) != "今天"
            && decoded(KeyDecoderMethod::Boundaries, "rupwu0 ", &[]) == "今天",
        "first tone without Space before an initial"
    );
    for method in KeyDecoderMethod::ALL {
        assert!(
            decoded(method, "ji3ap7g4", &[]) == "我們是" && decoded(method, "su3cl3", &[]) == "你好",
            "{method:?} keeps exact typing"
        );
    }
    assert!(
        decoded(KeyDecoderMethod::Boundaries, "jia3p7g4", &[]) != "我們是"
            && decoded(KeyDecoderMethod::Reorder, "jia3p7g4", &[]) == "我們是",
        "reorder repairs a swap across syllables"
    );
    assert_eq!(
        decoded(KeyDecoderMethod::Timed, "suc3l3", &[(3, 0.006)]),
        "你好",
        "timed repairs keys pressed together out of order"
    );
    let mut run_edit =
        Composer::new(real.clone(), ComposerOptions { key_decoder: KeyDecoderMethod::Reorder, ..unlearned.clone() });
    for key in "rupw".chars() {
        run_edit.handle_at(KeyInput::Char(key), 1.0);
    }
    run_edit.handle(KeyInput::Backspace);
    for key in "wu0 ".chars() {
        run_edit.handle_at(if key == ' ' { KeyInput::Space } else { KeyInput::Char(key) }, 2.0);
    }
    assert_eq!(text(&run_edit), "今天", "Backspace inside the run, then typing on");
    let mut as_typed = Composer::new(
        real.clone(),
        ComposerOptions {
            key_decoder: KeyDecoderMethod::Timed,
            typo_tolerance: TypoTolerance::Off,
            ..unlearned.clone()
        },
    );
    type_keys("jia3p7g4", &mut as_typed);
    assert_ne!(text(&as_typed), "我們是", "with correction off, keys are taken as typed");

    let slips = open(&data);
    let before = slips.typo_alternatives("ek4").into_iter().find(|a| a.0 == "ㄍㄜˋ");
    slips.record_typos(&[("rk4".into(), "ㄍㄜˋ".into())]);
    let cost_before =
        slips.typo_alternatives("rk4").into_iter().find(|a| a.0 == "ㄍㄜˋ").map_or(f64::INFINITY, |a| a.1);
    slips.record_typos(&[("rk4".into(), "ㄍㄜˋ".into()), ("rk4".into(), "ㄍㄜˋ".into())]);
    let cost_after = slips.typo_alternatives("rk4").into_iter().find(|a| a.0 == "ㄍㄜˋ").map_or(f64::INFINITY, |a| a.1);
    assert!(before.is_none() && cost_after < cost_before, "personal slips get cheaper with each correction");
    assert!(slips.export_personal_data().contains("[typos]\nr\te\t3"), "slips travel with an export");
    let receiving = open(&data);
    let rows = receiving.import_personal_data(correcting.export_personal_data().as_bytes()).unwrap();
    assert!(
        rows > 0 && text(&composer_with(&receiving, "ek4", legacy())) == picked,
        "imported memory converts the same"
    );
    assert!(receiving.import_personal_data(b"not an export").is_err(), "a foreign file is refused");
    receiving.exclude_from_learning(&picked, "ㄍㄜˋ");
    assert_ne!(text(&composer_with(&receiving, "ek4", legacy())), picked, "excluding forgets the word");
    let mut excluded_pick = composer_with(&receiving, "ek4", legacy());
    excluded_pick.handle(KeyInput::Down);
    if let Some(index) = items(&excluded_pick).iter().position(|t| *t == picked) {
        excluded_pick.select_candidate(index);
    }
    excluded_pick.commit_all();
    assert_ne!(
        text(&composer_with(&receiving, "ek4", legacy())),
        picked,
        "an excluded word is not learned from a pick"
    );
    assert_eq!(
        receiving.excluded_entries().iter().map(|k| k.word.clone()).collect::<Vec<_>>(),
        std::slice::from_ref(&picked),
        "exclusions are listed"
    );
    let reimported = open(&data);
    reimported.import_personal_data(receiving.export_personal_data().as_bytes()).unwrap();
    assert_eq!(
        reimported.excluded_entries().iter().map(|k| k.word.clone()).collect::<Vec<_>>(),
        [picked],
        "exclusions travel with an export"
    );

    let mut conversions: Vec<(String, String)> = [
        ("謝謝", "vu,4vu,7"),
        ("謝謝", "vu,4vu,4"),
        ("我們", "ji3ap7"),
        ("我們", "ji3ap "),
        ("東西", "2j/ vu "),
        ("東西", "2j/ vu7"),
        ("今天天氣很好", "rup wu0 wu0 fu4cp3cl3"),
        ("你好", "su3cl3"),
    ]
    .iter()
    .map(|(w, k)| ((*w).to_owned(), (*k).to_owned()))
    .collect();
    for word in ["什麼", "喜歡", "朋友"] {
        let reading = real.readings(word).first().cloned();
        assert!(reading.is_some(), "real lexicon has a reading for {word}");
        conversions.push((word.to_owned(), reading.map(|r| keys_for_reading(&r)).unwrap_or_default()));
    }
    for (word, keys) in conversions {
        assert_eq!(
            text(&composer_with(&real, &keys, ComposerOptions::default())),
            word,
            "real conversion {word} [{keys}]"
        );
    }
    for keys in ["su3cl38 ", "rup wu0 wu0 fu4cp3cl3"] {
        let mut c = composer_with(&real, keys, ComposerOptions::default());
        c.handle(KeyInput::Down);
        assert!(items(&c).len() <= 60, "real candidate list capped at 60");
    }
    let mut real_end = composer_with(&real, "cp3cl3", legacy());
    real_end.handle(KeyInput::Down);
    assert_eq!(items(&real_end)[..3], ["很好", "好", "郝"], "real end-cursor phrase candidates");
    let mut real_phrase_end = composer_with(&real, "vu,4vu,4", legacy());
    real_phrase_end.handle(KeyInput::Down);
    assert_eq!(items(&real_phrase_end)[0], "謝謝", "real tolerant multi-syllable candidate at end cursor");
}

#[test]
fn real_latency() {
    let _serial = serial();
    let Some(data) = common::data_directory() else { return };
    let real = open(&data);
    let mut long =
        composer_with(&real, &format!("{}su3cl3su3cl3", "rup wu0 wu0 fu4cp3cl3".repeat(6)), ComposerOptions::default());
    let (mut open_samples, mut highlight_samples) = (vec![], vec![]);
    for _ in 0..30 {
        open_samples.push(timed(|| long.handle(KeyInput::Down)).1);
        for _ in 0..12 {
            highlight_samples.push(timed(|| long.handle(KeyInput::Down)).1);
        }
        long.handle(KeyInput::Escape);
    }
    let max = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
    latency("40-syllable candidate opening", max(&open_samples), 5.0);
    latency("40-syllable candidate highlight", max(&highlight_samples), 5.0);
    for position in [0, 20] {
        long.handle(KeyInput::Home);
        for _ in 0..position {
            long.handle(KeyInput::Right);
        }
        latency(&format!("40-syllable candidate opening at {position}"), timed(|| long.handle(KeyInput::Down)).1, 5.0);
        long.handle(KeyInput::Escape);
    }
    let nbest_long =
        composer_with(&real, &format!("{}su3cl3", "rup wu0 wu0 fu4cp3cl3".repeat(3)), ComposerOptions::default());
    let samples: Vec<f64> = (0..30).map(|_| timed(|| nbest_long.n_best(8)).1).collect();
    assert_eq!(nbest_long.n_best(8).len(), 8, "20-syllable 8-best");
    latency("20-syllable 8-best", max(&samples), 10.0);
    // Previously unseen syllables at the 40-token limit exercise cold channel/span caches too.
    let tsv = std::fs::read(data.join("lexicon.tsv")).unwrap();
    let mut readings = BTreeSet::new();
    scan_tsv(&tsv, false, |fields| {
        if fields.len() == 3 && !fields[0].contains(' ') {
            readings.insert(fields[0].to_string());
        }
    });
    let mut cold = composer_with(&real, &"wu0 ".repeat(40), ComposerOptions::default());
    let mut cold_samples = vec![];
    for reading in readings.iter().step_by(37).take(80) {
        for key in keys_for_reading(reading).chars() {
            cold_samples.push(timed(|| cold.handle(if key == ' ' { KeyInput::Space } else { KeyInput::Char(key) })).1);
        }
        cold.handle(KeyInput::Backspace);
    }
    cold_samples.sort_by(f64::total_cmp);
    latency("cold channel on 40 syllables (p99)", cold_samples[percentile_index(cold_samples.len(), 0.99)], 3.0);
    measure_latency(&real, "discovered LM");
    // A 150 MB structural trigram model with the real vocabulary (synthetic probabilities), and lm2 when given.
    let mut words = BTreeSet::new();
    scan_tsv(&tsv, false, |fields| {
        if fields.len() == 3 {
            words.insert(fields[1].to_string());
        }
    });
    let temp = TempDir::new("large-lm");
    std::fs::write(temp.path().join("lm.bin"), common::large_lm_fixture(&words)).unwrap();
    let (large, ms) = timed(|| {
        Arc::new(
            Engine::open_with_model(&data.join("lexicon.tsv"), None, Some(temp.path()), EngineConfig::default())
                .unwrap(),
        )
    });
    eprintln!("TIMING 150 MB synthetic LM complete Engine init: {ms:.3} ms");
    measure_latency(&large, "150 MB synthetic trigram");
    if let Some(trigram) = std::env::var_os("ZHUYIN_TRIGRAM_LM") {
        let lm2 = Arc::new(
            Engine::open_with_model(
                &data.join("lexicon.tsv"),
                None,
                Some(Path::new(&trigram)),
                EngineConfig::default(),
            )
            .unwrap(),
        );
        measure_latency(&lm2, "lm2");
    }
}

/// Per-key latency on 40 syllables: moving the cursor and typing one more syllable, standard and aggressive.  The
/// target is on the slowest key, as in Swift; a run over it is measured once more, so one scheduling hiccup on a busy
/// machine does not fail the check.
fn measure_latency(engine: &Arc<Engine>, name: &str) {
    for (typo, suffix) in [(TypoTolerance::Standard, ""), (TypoTolerance::Aggressive, " aggressive")] {
        for (label, keys, next) in
            [("first-tone", "wu0 ".repeat(40), "wu0 "), ("neutral-tolerant", "vu,4vu,4".repeat(20), "vu,4")]
        {
            let run = || {
                let mut long = composer_with(
                    engine,
                    &keys,
                    ComposerOptions { typo_tolerance: typo, ..ComposerOptions::default() },
                );
                let mut samples = vec![];
                for _ in 0..100 {
                    samples.push(timed(|| long.handle(KeyInput::Left)).1);
                    long.handle(KeyInput::Right);
                    for key in next.chars() {
                        samples.push(
                            timed(|| long.handle(if key == ' ' { KeyInput::Space } else { KeyInput::Char(key) })).1,
                        );
                    }
                    long.handle(KeyInput::Backspace);
                }
                samples.sort_by(f64::total_cmp);
                samples
            };
            let mut samples = run();
            if samples[samples.len() - 1] >= 3.0 {
                samples = run();
            }
            eprintln!(
                "TIMING {name}{suffix} {label} handle on 40 syllables: median {:.3} ms, p95 {:.3} ms, max {:.3} ms",
                samples[samples.len() / 2],
                samples[percentile_index(samples.len(), 0.95)],
                samples[samples.len() - 1]
            );
            latency(&format!("40-syllable {name}{suffix} {label} handle"), samples[samples.len() - 1], 3.0);
        }
    }
}

#[test]
fn personal_model_through_the_composer() {
    let _serial = serial();
    let temp = TempDir::new("personal-composer");
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "肏\t-9\n操\t-10\n你\t-5\n我\t-5\n嗎\t-7\n媽\t-8\n</s>\t-3\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "你\t嗎\t-0.5\n你\t媽\t-7\n").unwrap();
    std::fs::write(dir.join("backoff.tsv"), "你\t-1\n").unwrap();
    let lexicon = "ㄘㄠˋ\t肏\t-9\nㄘㄠˋ\t操\t-10\nㄋㄧˇ\t你\t-5\nㄨㄛˇ\t我\t-5\nㄇㄚ\t嗎\t-7\nㄇㄚ\t媽\t-8\n";
    let open = || Arc::new(Engine::from_text(lexicon, None, Some(&dir), EngineConfig::default()));
    let personal = open();
    let clock =
        Arc::new(Mutex::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()));
    let reading = clock.clone();
    personal.set_clock(move || *reading.lock().unwrap());
    let plain = ComposerOptions { typo_tolerance: TypoTolerance::Off, ..ComposerOptions::default() };
    let convert = |e: &Arc<Engine>, keys: &str| text(&composer_with(e, keys, plain.clone()));
    let pick = |c: &mut Composer, word: &str| {
        let index = items(c).iter().position(|t| t == word).unwrap();
        c.select_candidate(index);
    };
    assert_eq!(convert(&personal, "hl4su3a8 "), "肏你嗎", "base model prefers 嗎 after 你");
    let mut correcting = composer_with(&personal, "hl4su3a8 ", plain.clone());
    correcting.handle(KeyInput::Down);
    pick(&mut correcting, "媽");
    correcting.handle(KeyInput::Home);
    correcting.handle(KeyInput::Down);
    pick(&mut correcting, "操");
    assert_eq!(correcting.commit_all(), "操你媽", "corrections committed");
    assert_eq!(convert(&personal, "hl4su3a8 "), "操你媽", "one correction settles the same input");
    assert!(convert(&personal, "ji3hl4su3a8 ").ends_with("操你媽"), "and the same words inside a longer input");
    assert_eq!(convert(&personal, "su3a8 "), "你嗎", "but not 你ㄇㄚ elsewhere");
    assert_eq!(convert(&personal, "ji3su3a8 "), "我你嗎", "nor after another word");
    *clock.lock().unwrap() += 7.0 * 86400.0;
    assert_eq!(convert(&personal, "hl4su3a8 "), "操你媽", "still remembered a week later");
    let mut back = composer_with(&personal, "hl4su3a8 ", plain.clone());
    back.handle(KeyInput::Down);
    pick(&mut back, "嗎");
    back.commit_all();
    assert!(convert(&personal, "hl4su3a8 ").ends_with("你嗎"), "the newest pick wins");
    let fading = open();
    let fading_clock = Arc::new(Mutex::new(*clock.lock().unwrap()));
    let reading = fading_clock.clone();
    fading.set_clock(move || *reading.lock().unwrap());
    let mut once = composer_with(&fading, "su3a8 ", plain.clone());
    once.handle(KeyInput::Down);
    pick(&mut once, "媽");
    once.commit_all();
    assert_eq!(convert(&fading, "su3a8 "), "你媽", "a correction of a whole input is recalled");
    *fading_clock.lock().unwrap() += 120.0 * 86400.0;
    assert_eq!(convert(&fading, "su3a8 "), "你嗎", "an unused single correction fades in months");
    assert_eq!(convert(&open(), "hl4su3a8 "), "肏你嗎", "empty memory leaves conversions unchanged");
}

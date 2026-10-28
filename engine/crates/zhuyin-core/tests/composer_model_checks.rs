//! The composer checks of `ZhuyinCoreChecks` that need language models, plus anchors, groups, selections, N-best
//! and reranking, the incremental/full decode equivalence and the wide-beam fixtures.

// The Swift checks compare clause lists such as `[0..<3]`; the Rust ones keep the same literals.
#![allow(clippy::single_range_in_vec_init)]

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use common::{LmTrigram, TempDir, bigram, bigram_backoff, lm_fixture};
use zhuyin_core::LanguageModel;
use zhuyin_core::api::{CandidateAnchor, ComposerOptions, KeyInput, KeyResult, PunctuationKeys, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::language_model::BigramLanguageModel;

const TINY: &str = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄅㄚ\t吧\t-2.4\nㄅㄚ\t芭\t-2.5\nㄅㄚ\t疤\t-2.6\nㄅㄚ\t笆\t-2.7\n\
ㄅㄚ\t捌\t-2.8\nㄅㄚ\t粑\t-2.9\nㄅㄚ\t叭\t-3.0\nㄅㄚ\t豝\t-3.1\nㄅㄚ\t仈\t-3.2\nㄅㄚˊ\t拔\t-2\nㄅㄚˇ\t把\t-2\nㄅㄚˋ\t爸\t-2\n\
ㄅㄚ˙\t罷\t-2\nㄆㄚ\t趴\t-2\nㄇㄚ\t媽\t-2\nㄇㄚˊ\t麻\t-2\nㄧ\t一\t-2\nㄨ\t烏\t-2\nㄩ\t迂\t-2\nㄅㄧㄢ\t邊\t-2\n\
ㄅㄚ ㄆㄚ\t八趴\t-1\nㄅㄚ ㄆㄚ\t巴趴\t-1.2\nㄅㄚ ㄆㄚ ㄇㄚ\t八趴媽\t-0.5\nㄅㄚ ㄅㄚ\t八八\t-5\nㄋㄧˇ\t你\t-2\nㄏㄠˇ\t好\t-2\n\
ㄋㄧˇ ㄏㄠˇ\t你好\t-1\nㄒㄧㄝˋ\t謝\t-2\nㄐㄧㄣ\t今\t-2\nㄊㄧㄢ\t天\t-2\nㄑㄧˋ\t氣\t-2\nㄏㄣˇ\t很\t-2\nㄐㄧㄣ ㄊㄧㄢ\t今天\t-1\n\
ㄊㄧㄢ ㄑㄧˋ\t天氣\t-1\nㄏㄣˇ ㄏㄠˇ\t很好\t-1";

fn engine(lexicon: &str, user: Option<&Path>, model: Option<&Path>) -> Arc<Engine> {
    Arc::new(Engine::from_text(lexicon, user, model, EngineConfig::default()))
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

fn composer(e: &Arc<Engine>, keys: &str) -> Composer {
    composer_with(e, keys, legacy())
}

fn text(c: &Composer) -> String {
    c.presentation().text.clone()
}

fn items(c: &Composer) -> Vec<String> {
    c.presentation().candidates.as_ref().map(|l| l.items().iter().map(|i| i.text.clone()).collect()).unwrap_or_default()
}

fn index_of(c: &Composer, word: &str) -> usize {
    items(c).iter().position(|t| t == word).unwrap_or_else(|| panic!("{word} not in {:?}", items(c)))
}

fn focused_range(c: &Composer) -> Option<std::ops::Range<usize>> {
    c.presentation().focused_clause.map(|i| c.presentation().clauses[i].clone())
}

fn valid_presentation(c: &Composer, label: &str) {
    let p = c.presentation();
    let length = p.text.encode_utf16().count();
    assert!(p.cursor <= length, "{label}: caret in bounds");
    let mut end = 0;
    for range in &p.clauses {
        assert!(range.start == end && !range.is_empty(), "{label}: contiguous nonempty clauses");
        end = range.end;
    }
    assert_eq!(end, length, "{label}: clauses cover text");
}

fn with_anchor(c: &mut Composer, anchor: CandidateAnchor) {
    let mut options = c.options().clone();
    options.candidate_anchor = anchor;
    c.set_options(options);
}

/// The binary fixture of `ZhuyinCoreChecks` (trigram, with an unknown-word shift).
fn binary_directory(temp: &TempDir, order: u32) -> std::path::PathBuf {
    let dir = temp.dir(&format!("binary{order}"));
    let unigrams = [
        ("</s>", -1.0f32),
        ("<unk>", -8.0),
        ("八", -2.0),
        ("巴", -2.3),
        ("趴", -2.0),
        ("媽", -3.0),
        ("麻", -3.0),
        ("𠮷", -4.0),
    ];
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
    let trigrams: &[LmTrigram] = if order == 3 { &trigrams } else { &[] };
    std::fs::write(dir.join("lm.bin"), lm_fixture(order, &unigrams, &[("八", -0.4), ("趴", -0.2)], &bigrams, trigrams))
        .unwrap();
    std::fs::write(dir.join("lm.json"), r#"{"unknown_word_shift":-2.3}"#).unwrap();
    dir
}

const LM_LEXICON: &str = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄆㄚ\t趴\t-2\n";

fn binary_lexicon() -> String {
    format!("{LM_LEXICON}ㄇㄚ\t媽\t-3\nㄇㄚ\t麻\t-3\n")
}

#[test]
fn trigram_history_through_the_composer() {
    let temp = TempDir::new("composer-trigram");
    let binary = binary_directory(&temp, 3);
    let trigram = engine(&binary_lexicon(), None, Some(&binary));
    assert_eq!(text(&composer(&trigram, "18 q8 a8 ")), "八趴麻", "trigram changes best conversion");
    let fixed_options = ComposerOptions { learning_enabled: false, ..ComposerOptions::default() };
    let mut fixed_history = composer_with(&trigram, "18 ", fixed_options.clone());
    fixed_history.handle(KeyInput::Down);
    let index = index_of(&fixed_history, "巴");
    fixed_history.select_candidate(index);
    type_keys("q8 a8 ", &mut fixed_history);
    assert_eq!(text(&fixed_history), "巴趴媽", "user-picked span retains its word in trigram history");
    for literal in [KeyInput::ShiftedChar(','), KeyInput::ShiftedChar('a')] {
        let mut reset_history = composer(&trigram, "18 q8 ");
        reset_history.handle(literal);
        type_keys("a8 ", &mut reset_history);
        assert!(text(&reset_history).ends_with('媽'), "literal resets both history words to sentence start");
    }
    let personal = temp.dir("binary-personal");
    std::fs::write(personal.join("user_phrases.tsv"), "ㄅㄚ\t巴\n").unwrap();
    let loaded = engine("ㄆㄚ\t趴\t-2\nㄇㄚ\t媽\t-3\nㄇㄚ\t麻\t-3\n", Some(&personal), Some(&binary));
    assert_eq!(text(&composer(&loaded, "18 q8 a8 ")), "巴趴媽", "persisted phrase has its model ID");

    let mut contextual = composer_with(&trigram, "18 q8 a8 ", fixed_options.clone());
    with_anchor(&mut contextual, CandidateAnchor::BeforeCursor);
    contextual.handle(KeyInput::Left);
    contextual.handle(KeyInput::Down);
    contextual.select_candidate(0);
    assert_eq!(text(&contextual), "八趴麻", "fixed multi-word phrase scores its internal words and last-word history");
    assert!(
        contextual.n_best(8).iter().all(|s| s.text().starts_with("八趴")),
        "N-best keeps fixed multi-word phrase verbatim"
    );
    let history_engine = engine(&format!("{}ㄇㄚ\t碼\t1\n", binary_lexicon()), None, Some(&binary));
    let mut history = composer_with(&history_engine, "18 q8 a8 ", fixed_options);
    with_anchor(&mut history, CandidateAnchor::BeforeCursor);
    history.handle(KeyInput::Left);
    history.handle(KeyInput::Down);
    history.select_candidate(0);
    with_anchor(&mut history, CandidateAnchor::AfterCursor);
    history.handle(KeyInput::Down);
    assert_eq!(
        items(&history)[..3],
        ["麻", "碼", "媽"],
        "candidate scores after a fixed phrase retain both words of trigram history"
    );
}

/// The old, unpruned last-word Viterbi recurrence (`legacyBigramConversion` in the Swift checks).
fn legacy_bigram_conversion(
    readings: &[&str],
    lexicon: &str,
    model: &BigramLanguageModel,
) -> (String, Vec<std::ops::Range<usize>>) {
    #[derive(Clone)]
    struct Node {
        score: f64,
        last: String,
        previous: String,
        words: Vec<String>,
    }
    let mut entries: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for line in lexicon.split('\n').filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').filter(|f| !f.is_empty()).collect();
        if fields.len() == 3
            && let Ok(score) = fields[2].parse::<f64>()
        {
            entries.entry(fields[0].to_owned()).or_default().push((fields[1].to_owned(), score));
        }
    }
    let mut states: Vec<HashMap<String, Node>> = vec![HashMap::new(); readings.len() + 1];
    states[0].insert(String::new(), Node { score: 0.0, last: "<s>".into(), previous: String::new(), words: vec![] });
    for start in 0..readings.len() {
        for end in start + 1..=(start + 6).min(readings.len()) {
            let reading = readings[start..end].join(" ");
            let previous_states: Vec<Node> = states[start].values().cloned().collect();
            for (word, unigram) in entries.get(&reading).cloned().unwrap_or_default() {
                for previous in &previous_states {
                    let total = previous.score + model.score_after(&word, &previous.last, unigram);
                    let old = states[end].get(&word);
                    if total > old.map_or(f64::NEG_INFINITY, |o| o.score)
                        || (old.is_some_and(|o| total == o.score && previous.last < o.previous))
                    {
                        let mut words = previous.words.clone();
                        words.push(word.clone());
                        states[end].insert(
                            word.clone(),
                            Node { score: total, last: word.clone(), previous: previous.last.clone(), words },
                        );
                    }
                }
            }
        }
    }
    let best = states[readings.len()].values().max_by(|a, b| {
        let x = a.score + model.end_score_after(&a.last, 1.0);
        let y = b.score + model.end_score_after(&b.last, 1.0);
        if x == y { b.last.cmp(&a.last) } else { x.partial_cmp(&y).unwrap() }
    });
    let words = best.map(|b| b.words.clone()).unwrap_or_default();
    let mut position = 0;
    let clauses = words
        .iter()
        .map(|w| {
            let length = w.encode_utf16().count();
            position += length;
            position - length..position
        })
        .collect();
    (words.concat(), clauses)
}

#[test]
fn bigram_equivalence_with_the_original_recurrence() {
    let temp = TempDir::new("equivalence");
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n趴\t</s>\t-0.1\n")
        .unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    let lexicon = format!("{TINY}\nㄇㄚ\t麻\t-2\n");
    let model = BigramLanguageModel::open(&dir).unwrap();
    let e = engine(&lexicon, None, Some(&dir));
    let scripts = [("ㄅㄚ", "18 "), ("ㄆㄚ", "q8 "), ("ㄇㄚ", "a8 ")];
    for length in 1..=6u32 {
        for code in 0..3usize.pow(length) {
            let (mut value, mut readings, mut keys) = (code, vec![], String::new());
            for _ in 0..length {
                let syllable = scripts[value % 3];
                value /= 3;
                readings.push(syllable.0);
                keys += syllable.1;
            }
            let old = legacy_bigram_conversion(&readings, &lexicon, &model);
            let new = composer(&e, &keys);
            assert!(
                new.presentation().text == old.0 && new.presentation().clauses == old.1,
                "old/new bigram equivalence {code}/{length}"
            );
        }
    }
}

#[test]
fn incremental_and_full_decodes_agree() {
    let temp = TempDir::new("incremental");
    let six = engine(
        &format!(
            "{TINY}\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t八八八八八八\t-0.1\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t巴巴巴巴巴巴巴\t0\n"
        ),
        None,
        None,
    );
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "八\t-2\n各\t-2\n特\t-2\n</s>\t0\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "<s>\t八\t0\n<s>\t各\t0\n<s>\t特\t0\n八\t各\t-1000\n八\t特\t0\n").unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    let context = engine("ㄅㄚ\t八\t-2\nㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\n", None, Some(&dir));
    let binary = binary_directory(&temp, 3);
    let trigram = engine(&binary_lexicon(), None, Some(&binary));
    let mut script: Vec<KeyInput> = "18 q8 a8 18 q8 a8 18 q8 a8 "
        .chars()
        .map(|c| if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) })
        .collect();
    script.extend([
        KeyInput::Left,
        KeyInput::Left,
        KeyInput::Backspace,
        KeyInput::Char('1'),
        KeyInput::Char('8'),
        KeyInput::Space,
        KeyInput::Home,
        KeyInput::ForwardDelete,
        KeyInput::End,
        KeyInput::Backspace,
        KeyInput::ShiftedChar(','),
        KeyInput::Char('1'),
        KeyInput::Char('8'),
        KeyInput::Space,
        KeyInput::Char('e'),
        KeyInput::Char('k'),
        KeyInput::Char('4'),
        KeyInput::Char('`'),
        KeyInput::Backspace,
    ]);
    for model in [six, context, trigram] {
        let options = ComposerOptions { learning_enabled: false, ..ComposerOptions::default() };
        let mut c = composer_with(&model, "", options);
        for key in &script {
            c.handle(*key);
            let incremental = c.presentation().clone();
            model.forget_learned("__full_decode_check__", "ㄦ");
            c.handle(KeyInput::Up);
            assert_eq!(*c.presentation(), incremental, "incremental/full decode equivalence after {key:?}");
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn anchors_groups_and_selections() {
    let group_lexicon = format!("{TINY}\nㄚ\t啊\t-2\nㄏㄠˇ ㄚ\t好啊\t-5\n");
    let group = engine(&group_lexicon, None, None);
    assert_eq!(
        ComposerOptions::default().candidate_anchor,
        CandidateAnchor::AfterCursor,
        "after-cursor anchor is default"
    );
    for anchor in [CandidateAnchor::AfterCursor, CandidateAnchor::BeforeCursor] {
        for position in 0..=3usize {
            let options = ComposerOptions { candidate_anchor: anchor, learning_enabled: false, ..legacy() };
            let mut c = composer_with(&group, "su3cl38 ", options);
            c.handle(KeyInput::Home);
            for _ in 0..position {
                c.handle(KeyInput::Right);
            }
            c.handle(KeyInput::Down);
            let forward = position == 0 || (anchor == CandidateAnchor::AfterCursor && position < 3);
            let expected = if forward { position..3 } else { 0..position };
            assert_eq!(
                focused_range(&c),
                Some(expected.clone()),
                "anchor {anchor:?} at {position} focuses {expected:?}"
            );
            let shown: String = "你好啊".chars().skip(expected.start).take(expected.len()).collect();
            assert_eq!(items(&c)[0], shown, "anchor current text at {position}");
            let mut previous_length = 7;
            let count = items(&c).len();
            for index in 0..count {
                let range = focused_range(&c).unwrap();
                assert!(range.len() <= previous_length, "candidate lengths descend");
                assert!(
                    if forward { range.start == position } else { range.end == position },
                    "candidate follows anchor direction"
                );
                assert_eq!(
                    c.presentation().zhuyin_hint.as_ref().unwrap().split(' ').count(),
                    range.len(),
                    "hint tracks highlighted span length"
                );
                valid_presentation(&c, "candidate length");
                previous_length = range.len();
                if index + 1 < count {
                    c.handle(KeyInput::Down);
                }
            }
        }
    }
    let mut grouped = composer(&group, "su3cl38 ");
    assert_eq!(grouped.presentation().clauses, [0..2, 2..3], "three-syllable fixture decodes as two words");
    grouped.handle(KeyInput::Down);
    assert_eq!(items(&grouped), ["你好啊", "好啊", "啊"], "generated phrase then two-syllable word then single");
    assert_eq!(focused_range(&grouped), Some(0..3), "multi-word phrase is one focused clause");
    grouped.select_candidate(0);
    assert_eq!(grouped.presentation().clauses, [0..3], "picked phrase is one fixed span");
    type_keys("18 ", &mut grouped);
    assert_eq!(text(&grouped), "你好啊八", "fixed phrase survives insertion after it");
    grouped.handle(KeyInput::Home);
    type_keys("q8 ", &mut grouped);
    assert_eq!(text(&grouped), "趴你好啊八", "fixed phrase survives insertion before it");
    grouped.handle(KeyInput::Backspace);
    assert_eq!(text(&grouped), "你好啊八", "fixed phrase survives deletion elsewhere");
    grouped.commit_all();
    assert!(
        group.learned_entries().iter().any(|(k, _)| k.reading == "ㄋㄧˇ ㄏㄠˇ ㄚ" && k.word == "你好啊"),
        "whole phrase learned under typed reading"
    );
    let mut recalled = composer(&group, "su3cl38 ");
    assert!(
        text(&recalled) == "你好啊" && recalled.presentation().clauses == [0..3],
        "learned phrase recalled as a direct entry"
    );
    recalled.handle(KeyInput::Down);
    assert_eq!(items(&recalled)[0], "你好啊", "learned phrase appears first");

    let interior = engine(&format!("{TINY}\nㄅㄚ ㄆㄚ\t巴趴\t-0.1\n"), None, None);
    let mut constrained = composer(&interior, "18 q8 a8 ");
    with_anchor(&mut constrained, CandidateAnchor::BeforeCursor);
    constrained.handle(KeyInput::Left);
    constrained.handle(KeyInput::Down);
    assert_eq!(items(&constrained)[..2], ["八趴", "巴趴"], "current span precedes best constrained conversion");
    let group = engine(&group_lexicon, None, None);
    let mut broken = composer(&group, "su3cl3");
    broken.handle(KeyInput::ShiftedChar(','));
    type_keys("8 ", &mut broken);
    broken.handle(KeyInput::Down);
    assert!(
        focused_range(&broken) == Some(3..4) && broken.presentation().zhuyin_hint.as_deref() == Some("ㄚ"),
        "candidates never cross a literal"
    );

    let temp = TempDir::new("quick-phrase");
    let selection_engine = engine(&group_lexicon, Some(temp.path()), None);
    let mut selected = composer(&selection_engine, "su3cl38 ");
    selected.handle(KeyInput::ShiftLeft);
    selected.handle(KeyInput::ShiftLeft);
    assert!(
        focused_range(&selected) == Some(1..3)
            && selected.presentation().notice.as_deref() == Some("Enter 加入自訂詞：好啊"),
        "backward selection notice"
    );
    assert!(
        selected.presentation().candidates.is_none() && selected.presentation().zhuyin_hint.is_none(),
        "selection closes candidate window"
    );
    selected.handle(KeyInput::ShiftRight);
    selected.handle(KeyInput::ShiftRight);
    assert!(
        selected.presentation().focused_clause.is_none() && selected.presentation().notice.is_none(),
        "returning to selection anchor clears visible selection"
    );
    for _ in 0..3 {
        selected.handle(KeyInput::ShiftLeft);
    }
    let saved = selected.handle(KeyInput::Enter);
    assert!(saved == KeyResult::CONSUMED && text(&selected) == "你好啊", "Enter saves without commit");
    assert!(
        selected.presentation().notice.as_deref() == Some("已加入自訂詞：你好啊") && focused_range(&selected).is_none(),
        "saved notice clears selection"
    );
    selected.handle(KeyInput::End);
    assert_eq!(selected.presentation().clauses, [0..3], "quick phrase fixes whole span");
    assert!(
        selection_engine.user_phrases().iter().any(|k| k.word == "你好啊" && k.reading == "ㄋㄧˇ ㄏㄠˇ ㄚ"),
        "quick phrase permanent entry"
    );
    selected.handle(KeyInput::End);
    type_keys("18 ", &mut selected);
    assert!(
        text(&selected) == "你好啊八" && selected.presentation().notice.is_none(),
        "saved notice lasts until next key; fixed phrase survives edit"
    );
    selection_engine.flush_personal_data().unwrap();
    let loaded = engine(&group_lexicon, Some(temp.path()), None);
    assert!(loaded.user_phrases().iter().any(|k| k.word == "你好啊"), "quick phrase persisted");
    let mut c = composer(&loaded, "su3cl38 ");
    assert_eq!(c.presentation().clauses, [0..3], "permanent phrase recalled directly after reload");
    c.handle(KeyInput::Down);
    assert_eq!(items(&c)[0], "你好啊", "saved phrase offered first next time");
    let mut crossed = composer(&group, "su3cl38 ");
    crossed.handle(KeyInput::Left);
    crossed.handle(KeyInput::ShiftLeft);
    assert_eq!(focused_range(&crossed), Some(1..2), "selection left from middle");
    crossed.handle(KeyInput::ShiftRight);
    crossed.handle(KeyInput::ShiftRight);
    assert!(
        focused_range(&crossed) == Some(2..3)
            && crossed.presentation().notice.as_deref() == Some("Enter 加入自訂詞：啊"),
        "selection crosses anchor to right"
    );
    crossed.handle(KeyInput::Escape);
    assert!(
        text(&crossed) == "你好啊"
            && crossed.presentation().focused_clause.is_none()
            && crossed.presentation().notice.is_none(),
        "Escape clears selection only"
    );
    crossed.handle(KeyInput::Home);
    crossed.handle(KeyInput::ShiftRight);
    crossed.handle(KeyInput::ShiftRight);
    assert_eq!(focused_range(&crossed), Some(0..2), "forward selection");
    for key in [KeyInput::Left, KeyInput::Right, KeyInput::Down, KeyInput::Char('1'), KeyInput::ShiftEnter] {
        let mut c = composer(&group, "su3cl38 ");
        c.handle(KeyInput::ShiftLeft);
        let result = c.handle(key);
        assert!(c.presentation().notice.is_none(), "other key clears selection notice {key:?}");
        if key == KeyInput::ShiftEnter {
            assert!(
                result.commit.as_deref() == Some("你好啊") && !result.handled,
                "Shift+Enter retains commit/pass-through semantics"
            );
        }
        if key == KeyInput::Left || key == KeyInput::Right {
            assert!(c.presentation().focused_clause.is_none(), "plain arrow clears selection");
        }
    }
    for with_literal in [false, true] {
        let mut c = composer(&group, &if with_literal { "su3cl3".to_owned() } else { "18 ".repeat(7) });
        if with_literal {
            c.handle(KeyInput::ShiftedChar(','));
        }
        let original = text(&c);
        for _ in 0..(if with_literal { 3 } else { 7 }) {
            c.handle(KeyInput::ShiftLeft);
        }
        assert!(
            c.handle(KeyInput::Enter) == KeyResult::CONSUMED && text(&c) == original,
            "invalid selection does not commit"
        );
        assert!(
            c.presentation().notice.as_deref() == Some("自訂詞需為 1 到 6 個字") && focused_range(&c).is_some(),
            "invalid selection remains with error notice"
        );
        c.handle(KeyInput::Escape);
        assert!(c.presentation().notice.is_none() && text(&c) == original, "invalid selection escape");
    }
    let mut hint_group = composer(&group, "su3cl38 ");
    hint_group.handle(KeyInput::Home);
    hint_group.handle(KeyInput::Right);
    hint_group.handle(KeyInput::Down);
    assert_eq!(
        hint_group.presentation().zhuyin_hint.as_deref(),
        Some("ㄏㄠˇ ㄚ"),
        "multi-syllable hint uses spaces and unmarked first tone"
    );
    assert!(
        hint_group.handle(KeyInput::Char('`')) == KeyResult::CONSUMED && text(&hint_group) == "你ㄏㄠˇ ㄚ",
        "window backtick replaces exact after-cursor span"
    );
    assert!(
        hint_group.presentation().candidates.is_none() && hint_group.presentation().zhuyin_hint.is_none(),
        "backtick closes window without commit"
    );
    valid_presentation(&hint_group, "multi-syllable literal");
}

#[test]
fn n_best_and_reranking() {
    let nbest_engine =
        engine("ㄅㄚ\t八\t-1\nㄅㄚ\t巴\t-2\nㄆㄚ\t趴\t-1\nㄆㄚ\t啪\t-3\nㄅㄚ ㄆㄚ\t八趴\t-1.5\n", None, None);
    let mut nbest = composer(&nbest_engine, "18 q8 ");
    let mut pending = composer(&nbest_engine, "");
    let calls = Arc::new(Mutex::new(0));
    let counter = calls.clone();
    pending.set_reranker(Some(Box::new(move |_| {
        *counter.lock().unwrap() += 1;
        Some(0)
    })));
    type_keys("18", &mut pending);
    assert_eq!(*calls.lock().unwrap(), 0, "reranker never receives an empty completed sentence list");
    pending.handle(KeyInput::Space);
    assert_eq!(*calls.lock().unwrap(), 1, "reranker runs when a syllable completes");
    let sentences = nbest.n_best(8);
    assert_eq!(
        sentences.iter().map(zhuyin_core::api::ScoredSentence::text).collect::<Vec<_>>(),
        ["八趴", "巴趴", "八啪", "巴啪"],
        "distinct N-best includes alternate histories and removes duplicate segmentations"
    );
    assert_eq!(
        sentences.iter().map(|s| s.score).collect::<Vec<_>>(),
        [-1.5, -3.0, -4.0, -5.0],
        "N-best scores ordered"
    );
    assert!(
        sentences[0].words.iter().map(|w| w.range.clone()).eq([0..2])
            && sentences[1].words.iter().map(|w| w.range.clone()).eq([0..1, 1..2]),
        "N-best carries segmentation and token spans"
    );
    assert!(nbest.n_best(2).len() == 2 && nbest.n_best(0).is_empty(), "N-best K bounds");
    assert_eq!(text(&nbest), sentences[0].text(), "default display stays top decoder result");
    nbest.set_reranker(Some(Box::new(|s| Some(s.len() - 1))));
    assert_eq!(text(&nbest), "巴啪", "reranker chooses displayed sentence");
    nbest.handle(KeyInput::Down);
    assert_eq!(items(&nbest)[0], "巴啪", "candidate list starts with current span under reranking");
    assert_eq!(
        nbest.n_best(8).iter().map(|s| s.score).collect::<Vec<_>>(),
        [-1.5, -3.0, -4.0, -5.0],
        "reranker leaves statistical scores ordered"
    );
    let remember_engine = engine("ㄅㄚ\t八\t-1\nㄅㄚ\t巴\t-2\nㄆㄚ\t趴\t-1\nㄆㄚ\t啪\t-3\n", None, None);
    let mut remember_pick = composer(&remember_engine, "q8 18 ");
    remember_pick.handle(KeyInput::Down);
    let index = index_of(&remember_pick, "巴");
    remember_pick.select_candidate(index);
    remember_pick.commit_all();
    let mut remembering = composer(&remember_engine, "q8 18 ");
    remembering.set_reranker(Some(Box::new(|s| Some(s.iter().position(|x| !x.text().ends_with('巴')).unwrap_or(0)))));
    assert!(
        text(&remembering) == "趴巴" && remembering.n_best(8).iter().any(|s| !s.text().ends_with('巴')),
        "reranker keeps a word picked in the same place before"
    );
    nbest.set_reranker(Some(Box::new(|_| Some(usize::MAX))));
    assert_eq!(text(&nbest), "八趴", "invalid reranker choice falls back");
    nbest.set_reranker(None);
    let mut fixed = composer(&nbest_engine, "18 ");
    fixed.handle(KeyInput::Down);
    let index = index_of(&fixed, "巴");
    fixed.select_candidate(index);
    type_keys("q8 ", &mut fixed);
    assert!(
        fixed.n_best(8).len() == 2 && fixed.n_best(8).iter().all(|s| s.text().starts_with('巴')),
        "N-best respects fixed spans"
    );
    fixed.handle(KeyInput::ShiftedChar(','));
    type_keys("18 ", &mut fixed);
    assert!(
        fixed
            .n_best(8)
            .iter()
            .all(|s| s.words.iter().any(|w| w.text == "，" && w.reading.is_empty() && w.range == (2..3))),
        "N-best preserves literals and spans"
    );
}

/// One line per item.
fn rows(items: impl IntoIterator<Item = String>) -> String {
    items.into_iter().fold(String::new(), |mut text, item| {
        text.push_str(&item);
        text.push('\n');
        text
    })
}

#[test]
fn wide_beams() {
    let temp = TempDir::new("wide-beam");
    let dir = temp.dir("wide");
    let words: Vec<String> = (0..80u32).map(|i| char::from_u32(0x4e00 + i).unwrap().to_string()).collect();
    let unigram: String = rows(words.iter().map(|w| format!("{w}\t-2"))) + "</s>\t0\n";
    let bigrams: String = rows(words.iter().map(|w| format!("<s>\t{w}\t0"))) + "一\t</s>\t-100\n";
    std::fs::write(dir.join("unigram.tsv"), unigram).unwrap();
    std::fs::write(dir.join("bigram.tsv"), bigrams).unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    let lexicon = rows(words.iter().map(|w| format!("ㄅㄚ\t{w}\t-2")));
    let wide_engine = engine(&lexicon, None, Some(&dir));
    let mut wide = composer(&wide_engine, "18 ");
    assert_eq!(text(&wide), "丁", "wide beam ranks EOS before pruning and resolves score ties");
    assert!(
        wide.n_best(20).len() == 20 && wide.n_best(20)[0].text() == "丁",
        "N-best K widens beyond the default beam"
    );
    wide.handle(KeyInput::Down);
    assert_eq!(items(&wide).len(), words.len(), "candidate list keeps every word in a wide lexicon");
    wide.handle(KeyInput::Escape);
    type_keys("18 ", &mut wide);
    assert_eq!(text(&wide), "一丁", "cached final-position scores never leak into an interior position");
    let six_words: Vec<String> = words.iter().map(|w| format!("八八八八八{w}")).collect();
    let unigram: String = rows(six_words.iter().map(|w| format!("{w}\t-2"))) + "</s>\t0\n";
    let bigrams: String =
        rows(six_words.iter().map(|w| format!("<s>\t{w}\t0"))) + &format!("{}\t</s>\t-100\n", six_words[0]);
    std::fs::write(dir.join("unigram.tsv"), unigram).unwrap();
    std::fs::write(dir.join("bigram.tsv"), bigrams).unwrap();
    let six_reading = ["ㄅㄚ"; 6].join(" ");
    let lexicon = format!("ㄅㄚ\t八\t-100\n{}", rows(six_words.iter().map(|w| format!("{six_reading}\t{w}\t-2"))));
    let wide_six_engine = engine(&lexicon, None, Some(&dir));
    let mut wide_six = composer(&wide_six_engine, &"18 ".repeat(6));
    assert_eq!(text(&wide_six), six_words[1], "six-syllable edge prunes with EOS");
    type_keys("18 ", &mut wide_six);
    assert_eq!(
        text(&wide_six),
        format!("{}八", six_words[0]),
        "six-syllable edge at old EOS is rebuilt before prefix reuse"
    );
    let tiny = engine(TINY, None, None);
    let mut invalid = composer(&tiny, "18 1u");
    invalid.handle(KeyInput::Down);
    assert!(
        invalid.presentation().candidates.is_none() && text(&invalid) == "八ㄅㄧ",
        "invalid unfinished buffer cannot enter a candidate replacement span"
    );
    let _ = tiny.language_model().order();
}

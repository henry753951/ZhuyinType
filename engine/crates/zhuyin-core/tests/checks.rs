//! The `ZhuyinCoreChecks` checks of the data layer, on fixtures (no real data needed).  Each test names the Swift
//! check messages it ports.  Keyboard and channel-geometry checks are unit tests next to their modules.

mod common;

use std::collections::HashMap;
use std::fmt::Write as _;

use common::{LmTrigram, TempDir, bigram, bigram_backoff, lexicon_fixture, lm_fixture};
use zhuyin_core::key_channel::{self, ReadingAlternative};
use zhuyin_core::key_decoder::{Costs, Key, KeyDecoder};
use zhuyin_core::language_model::{BigramLanguageModel, MixtureLanguageModel, NO_CONTEXT, VocabularyModel};
use zhuyin_core::syllables::SyllableSet;
use zhuyin_core::{LanguageModel, LanguageModelHistory, MappedLexicon, NGramLanguageModel};
use zhuyin_core::{api::KeyDecoderMethod, lexicon::TextLexicon};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.00001
}

fn alternatives(set: &SyllableSet, keys: &str) -> Vec<ReadingAlternative> {
    key_channel::alternatives(keys, false, |_, _| 0.0, |code| set.valid_reading(code))
}

/// The syllables of a TSV lexicon, as the engine builds them.
fn syllables_of(lexicon: &str) -> SyllableSet {
    let parsed = TextLexicon::parse(lexicon.as_bytes(), |_| None);
    SyllableSet::from_readings(parsed.entries.keys().map(String::as_str))
}

#[test]
fn keyboard_channel_alternatives() {
    // "3/4 tone alternative", "eight cheapest sorted alternatives", "adjacent transposition reparsed", "extra typed
    // key deleted", "missing key inserted", "four keys allow two edits", "three keys forbid two edits".
    let set = syllables_of("ㄊㄜˋ\t特\t-2\nㄊㄜˇ\t忑\t-2\nㄅㄧˇ\t筆\t-2\nㄊㄧㄢˇ\t舔\t-2\n");
    let tone = alternatives(&set, "wk3");
    assert!(tone.iter().any(|a| a.reading == "ㄊㄜˋ"));
    assert!(tone.len() <= 8 && tone.windows(2).all(|pair| pair[0].cost <= pair[1].cost));
    assert!(alternatives(&set, "13u").iter().any(|a| a.reading == "ㄅㄧˇ"));
    assert!(alternatives(&set, "1u03").iter().any(|a| a.reading == "ㄅㄧˇ"));
    assert!(alternatives(&set, "u3").iter().any(|a| a.reading == "ㄅㄧˇ"));
    assert!(alternatives(&set, "su04").iter().any(|a| a.reading == "ㄊㄧㄢˇ"));
    assert!(!alternatives(&set, "sk4").iter().any(|a| a.reading == "ㄊㄜˇ"));
    // The 3/4 swap costs exactly the calibrated constant.
    assert_eq!(tone.iter().find(|a| a.reading == "ㄊㄜˋ").unwrap().cost, key_channel::TONE34);
    // "invalid exact syllable accepted via neighbour alternative": ek4 is not a syllable here, wk4 is.
    let only = syllables_of("ㄊㄜˋ\t特\t-2\n");
    assert!(!only.is_valid("ㄍㄜˋ"));
    assert_eq!(alternatives(&only, "ek4").first().map(|a| (a.reading.as_str(), a.cost)), Some(("ㄊㄜˋ", 11.0)));
}

#[test]
fn personal_slips_get_cheaper() {
    // "personal slips get cheaper with each correction": the engine's discount is min(4, 1.5 ln(1 + n)).
    let set = syllables_of("ㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\n");
    let cost = |n: f64| {
        let discount = move |typed: u8, intended: u8| {
            if (typed, intended) == (b'r', b'e') && n > 0.0 { f64::min(4.0, 1.5 * (1.0 + n).ln()) } else { 0.0 }
        };
        key_channel::alternatives("rk4", false, discount, |code| set.valid_reading(code))
            .into_iter()
            .find(|a| a.reading == "ㄍㄜˋ")
            .map_or(f64::INFINITY, |a| a.cost)
    };
    assert!(cost(1.0) < cost(0.0) && cost(3.0) < cost(1.0));
    assert_eq!(cost(100.0), key_channel::SUBSTITUTION - 4.0);
}

#[test]
fn hotspot_costs() {
    let set = syllables_of("ㄊㄜˋ\t特\t-2\n");
    let hot = key_channel::alternatives("ek4", true, |_, _| 0.0, |code| set.valid_reading(code));
    assert_eq!(hot[0].reading, "ㄊㄜˋ");
    assert_eq!(hot[0].cost, key_channel::hotspot_cost('w', 'e'));
}

#[test]
fn tsv_bigram_model() {
    // "bigram backoff = log alpha + unigram", "seen bigram".
    let lm = BigramLanguageModel::from_tsv(
        "八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n".as_bytes(),
        "<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n趴\t</s>\t-0.1\n".as_bytes(),
        "八\t-0.4\n".as_bytes(),
    );
    assert!(close(lm.score_after("巴", "八", -9.0), -2.7));
    assert_eq!(lm.score_after("趴", "巴", -9.0), -0.1);
    assert_eq!(lm.score_after("不存在", "八", -9.0), -0.4 + -9.0);
    assert_eq!(lm.end_score_after("趴", 1.0), -0.1);
    // "incomplete optional LM safely falls back": a directory without backoff.tsv does not load.
    let temp = TempDir::new("tsv");
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "八\t-2\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "<s>\t八\t-0.1\n").unwrap();
    assert!(BigramLanguageModel::open(&dir).is_err());
    assert!(VocabularyModel::load(&dir).is_none());
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    assert!(matches!(VocabularyModel::load(&dir), Some(VocabularyModel::Bigram(_))));
}

fn fixture_unigrams() -> Vec<(&'static str, f32)> {
    vec![
        ("</s>", -1.0),
        ("<unk>", -8.0),
        ("八", -2.0),
        ("巴", -2.3),
        ("趴", -2.0),
        ("媽", -3.0),
        ("麻", -3.0),
        ("𠮷", -4.0),
    ]
}

fn fixture_bigrams() -> Vec<common::LmBigram> {
    vec![
        bigram("<s>", "八", -0.1),
        bigram("<s>", "巴", -0.5),
        bigram_backoff("八", "趴", -0.2, -0.7),
        bigram_backoff("巴", "趴", -0.2, -0.8),
        bigram("趴", "媽", -0.1),
        bigram("趴", "麻", -1.0),
        bigram("媽", "</s>", -0.1),
        bigram("麻", "</s>", -0.1),
        bigram("<unk>", "趴", -0.05),
    ]
}

fn fixture_trigrams() -> Vec<LmTrigram> {
    vec![
        LmTrigram { first: "八", second: "趴", third: "媽", logp: -4.0 },
        LmTrigram { first: "八", second: "趴", third: "麻", logp: -0.05 },
        LmTrigram { first: "巴", second: "趴", third: "媽", logp: -0.01 },
    ]
}

const FIXTURE_BACKOFFS: [(&str, f32); 2] = [("八", -0.4), ("趴", -0.2)];

fn trigram_fixture() -> Vec<u8> {
    lm_fixture(3, &fixture_unigrams(), &FIXTURE_BACKOFFS, &fixture_bigrams(), &fixture_trigrams())
}

fn bigram_fixture() -> Vec<u8> {
    lm_fixture(2, &fixture_unigrams(), &FIXTURE_BACKOFFS, &fixture_bigrams(), &[])
}

#[test]
fn binary_backoff_branches() {
    // "binary backoff …", "binary sentence start without older history", "binary one-word backoff", "binary end
    // uses both backoffs", "binary reserved word IDs", "binary UTF-8 vocabulary search and missing words", "explicit
    // unk uses model unigram", "missing metadata defaults unknown shift to zero", "unknown lexicon probability plus
    // metadata shift", "invalid external IDs cannot access outside the mapping", "unknown history scores as the
    // explicit unk context".
    let temp = TempDir::new("binary");
    let dir = temp.dir("binary");
    std::fs::write(dir.join("lm.bin"), trigram_fixture()).unwrap();
    let binary = NGramLanguageModel::open(&dir).unwrap();
    let score = |word: &str, older: Option<&str>, previous: &str| {
        binary.score(
            binary.word_id(word),
            LanguageModelHistory::new(older.and_then(|o| binary.word_id(o)), binary.word_id(previous).unwrap()),
            -9.0,
        )
    };
    for (word, older, previous, expected) in [
        ("媽", "八", "趴", -4.0),
        ("麻", "巴", "趴", -1.8),
        ("巴", "八", "趴", -3.2),
        ("媽", "媽", "趴", -0.1),
        ("巴", "媽", "八", -2.7),
    ] {
        assert!(close(score(word, Some(older), previous), expected), "binary backoff {older} {previous} → {word}");
    }
    assert!(close(score("八", None, "<s>"), -0.1));
    assert!(close(score("巴", None, "八"), -2.7));
    assert!(close(score("</s>", Some("八"), "趴"), -1.9));
    assert_eq!((binary.word_id("</s>"), binary.word_id("<s>"), binary.word_id("<unk>")), (Some(0), Some(1), Some(2)));
    assert!(binary.word_id("𠮷").is_some() && binary.word_id("不存在").is_none() && binary.word_id("").is_none());
    assert_eq!(binary.score(Some(2), LanguageModelHistory::default(), -9.0), -8.0);
    assert_eq!(binary.score(None, LanguageModelHistory::default(), -9.0), -9.0);
    assert_eq!(binary.unknown_word_shift(), 0.0);
    // No context: the unigram alone.
    assert_eq!(binary.score(binary.word_id("八"), LanguageModelHistory::new(None, NO_CONTEXT), -9.0), -2.0);

    std::fs::write(dir.join("lm.json"), r#"{"unknown_word_shift":-2.3}"#).unwrap();
    let shifted = NGramLanguageModel::open(&dir).unwrap();
    assert!(close(shifted.score(None, LanguageModelHistory::default(), -9.0), -11.3));
    assert!(shifted.score(Some(u32::MAX), LanguageModelHistory::new(None, u32::MAX), -9.0).is_finite());
    assert!(close(shifted.score(shifted.word_id("趴"), LanguageModelHistory::new(None, 2), -9.0), -0.05));
    // A field of the wrong type discards all metadata, as Swift's JSONDecoder does.
    std::fs::write(dir.join("lm.json"), r#"{"unknown_word_shift":-2.3,"tsv_compatibility":"yes"}"#).unwrap();
    assert_eq!(NGramLanguageModel::open(&dir).unwrap().unknown_word_shift(), 0.0);
    std::fs::write(dir.join("lm.json"), "not json").unwrap();
    assert_eq!(NGramLanguageModel::open(&dir).unwrap().unknown_word_shift(), 0.0);

    // "order two ignores older history".
    std::fs::write(dir.join("lm.bin"), bigram_fixture()).unwrap();
    let order_two = NGramLanguageModel::open(&dir).unwrap();
    assert_eq!(order_two.order(), 2);
    let history = LanguageModelHistory::new(order_two.word_id("八"), order_two.word_id("趴").unwrap());
    assert!(close(order_two.score(order_two.word_id("媽"), history, -9.0), -0.1));
    // "empty bigram/trigram sections are valid".
    std::fs::write(dir.join("lm.bin"), lm_fixture(3, &[], &[], &[], &[])).unwrap();
    assert!(NGramLanguageModel::open(&dir).is_ok());
}

fn replacing32(data: &[u8], offset: usize, value: u32) -> Vec<u8> {
    let mut result = data.to_vec();
    result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    result
}

#[test]
fn corrupt_binary_models_are_rejected() {
    // "reject corrupt/truncated binary fixture \(index)", "reject every truncated binary prefix \(size)", "corrupt
    // binary falls back to complete TSV model", "binary model takes precedence over TSV".
    let fixture = trigram_fixture();
    let order_two = bigram_fixture();
    let at = |offset: usize| u64::from_le_bytes(fixture[offset..offset + 8].try_into().unwrap()) as usize;
    let (vocab, bi, tri) = (at(24), at(40), at(48));
    let corrupt = [
        vec![],
        fixture[..63].to_vec(),
        fixture[..fixture.len() - 1].to_vec(),
        replacing32(&fixture, 0, 0),
        replacing32(&fixture, 4, 2),
        replacing32(&fixture, 8, 4),
        replacing32(&fixture, 12, u32::MAX),
        replacing32(&fixture, 16, u32::MAX),
        replacing32(&fixture, 20, u32::MAX),
        replacing32(&fixture, 24, 65),
        replacing32(&fixture, 32, 64),
        replacing32(&fixture, 40, u32::MAX),
        replacing32(&fixture, 48, 0),
        replacing32(&fixture, 56, 64),
        replacing32(&fixture, vocab, 1),
        replacing32(&fixture, vocab + 8, u32::MAX),
        replacing32(&fixture, bi, 1),
        replacing32(&fixture, bi + 8, u32::MAX),
        replacing32(&fixture, tri, 1),
        replacing32(&fixture, tri + 8, u32::MAX),
        replacing32(&order_two, 20, 1),
        replacing32(&order_two, 48, 64),
    ];
    let temp = TempDir::new("corrupt");
    let dir = temp.dir("binary");
    for (index, bytes) in corrupt.iter().enumerate() {
        std::fs::write(dir.join("lm.bin"), bytes).unwrap();
        assert!(NGramLanguageModel::open(&dir).is_err(), "reject corrupt/truncated binary fixture {index}");
        assert!(VocabularyModel::load(&dir).is_none(), "no TSV to fall back to {index}");
    }
    for size in 0..fixture.len() {
        std::fs::write(dir.join("lm.bin"), &fixture[..size]).unwrap();
        assert!(NGramLanguageModel::open(&dir).is_err(), "reject every truncated binary prefix {size}");
    }
    std::fs::write(dir.join("unigram.tsv"), "八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n").unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    assert!(matches!(VocabularyModel::load(&dir), Some(VocabularyModel::Bigram(_))));
    std::fs::write(dir.join("lm.bin"), &fixture).unwrap();
    assert!(matches!(VocabularyModel::load(&dir), Some(VocabularyModel::NGram(_))));
}

#[test]
fn tsv_binary_mixture() {
    // checkTSVBinaryMixture: "mapped LM supports ASCII before reserved spellings", "mixed TSV/binary models preserve
    // backoff for \(previous) → \(word)".
    let temp = TempDir::new("mixture");
    let mut directories = HashMap::new();
    for (name, extra) in [("wiki", "八"), ("chat", "巴")] {
        let dir = temp.dir(name);
        let unigrams = [("</s>", -1.0f32), ("<unk>", -8.0), ("!", -2.0), (extra, -3.0)];
        let leaked: &'static str = Box::leak(extra.to_owned().into_boxed_str());
        let bigrams = [bigram("<s>", leaked, -0.1), bigram(leaked, "!", -0.2)];
        std::fs::write(dir.join("lm.bin"), lm_fixture(2, &unigrams, &[("<s>", -0.7), (extra, -0.4)], &bigrams, &[]))
            .unwrap();
        std::fs::write(dir.join("lm.json"), r#"{"tsv_compatibility":true,"missing_unigrams":[1]}"#).unwrap();
        // Swift writes Float's description: -1.0, -8.0, -0.1 ...
        let unigram_text = unigrams.iter().fold(String::new(), |mut text, (w, s)| {
            writeln!(text, "{w}\t{s:?}").unwrap();
            text
        });
        std::fs::write(dir.join("unigram.tsv"), unigram_text).unwrap();
        std::fs::write(dir.join("bigram.tsv"), format!("<s>\t{extra}\t-0.1\n{extra}\t!\t-0.2\n")).unwrap();
        std::fs::write(dir.join("backoff.tsv"), format!("<s>\t-0.7\n{extra}\t-0.4\n")).unwrap();
        directories.insert(name, dir);
    }
    let tsv = |name: &str| VocabularyModel::Bigram(BigramLanguageModel::open(&directories[name]).unwrap());
    let bin = |name: &str| VocabularyModel::NGram(NGramLanguageModel::open(&directories[name]).unwrap());
    assert_eq!(bin("wiki").word_id("!"), Some(3));
    let baseline = MixtureLanguageModel::new(tsv("chat"), tsv("wiki"), 0.5);
    let variants = [
        MixtureLanguageModel::new(bin("chat"), bin("wiki"), 0.5),
        MixtureLanguageModel::new(tsv("chat"), bin("wiki"), 0.5),
        MixtureLanguageModel::new(bin("chat"), tsv("wiki"), 0.5),
    ];
    for word in ["八", "巴", "!", "</s>", "<s>", "<unk>", "不存在"] {
        for previous in ["八", "巴", "<s>", "<unk>", "不存在"] {
            let expected = baseline.score_after(word, previous, -9.0);
            for model in &variants {
                assert!(
                    (model.score_after(word, previous, -9.0) - expected).abs() < 1e-6,
                    "mixed TSV/binary models preserve backoff for {previous} → {word}"
                );
            }
        }
    }
    // The chat share is clamped, and the end points use one model alone.
    let mixture = &variants[0];
    mixture.set_chat_weight(2.0);
    assert_eq!(mixture.chat_weight(), 1.0);
    let (chat, encyclopedia) = mixture.scores(mixture.word_id("巴"), 1, -9.0);
    assert_eq!(mixture.score(mixture.word_id("巴"), LanguageModelHistory::default(), -9.0), chat);
    mixture.set_chat_weight(f64::NAN);
    assert_eq!(mixture.chat_weight(), 0.0);
    assert_eq!(mixture.score(mixture.word_id("巴"), LanguageModelHistory::default(), -9.0), encyclopedia);
}

#[test]
fn mapped_lexicon_fixture() {
    // checkMappedLexicon: "mapped reading prior and neutral-tone lookup", "mapped reverse index ordered by original
    // score", "reject truncated lexicon \(length)", "reject lexicon string outside mapping".
    let temp = TempDir::new("lexicon");
    let path = temp.path().join("lexicon.bin");
    let bytes = lexicon_fixture();
    std::fs::write(&path, &bytes).unwrap();
    let lexicon = MappedLexicon::open(&path).unwrap();
    assert_eq!(lexicon.word_count(), 2);
    let entries = lexicon.entries("ㄅㄚ");
    assert_eq!(
        entries.iter().map(|e| (&*e.word, e.logp, e.reading_prior)).collect::<Vec<_>>(),
        [("八", -3.0, 0.0), ("吧", -2.0, -8.0)]
    );
    assert_eq!(&*lexicon.entries("ㄅㄚ˙")[0].word, "吧");
    assert!(lexicon.entries("ㄆㄚ").is_empty());
    assert_eq!(lexicon.readings_for_base("ㄅㄚ"), ["ㄅㄚ", "ㄅㄚ˙"]);
    assert!(lexicon.readings_for_base("ㄆㄚ").is_empty());
    assert!(lexicon.contains_prefix("ㄅㄚ") && !lexicon.contains_prefix("ㄅ"));
    assert_eq!(lexicon.syllables(), ["ㄅㄚ", "ㄅㄚ˙"]);
    let mut readings = lexicon.readings_of("吧");
    assert_eq!(readings, [("ㄅㄚ".to_owned(), -10.0), ("ㄅㄚ˙".to_owned(), -2.0)]);
    readings.sort_by(|a, b| b.1.total_cmp(&a.1));
    assert_eq!(readings[0].0, "ㄅㄚ˙", "mapped reverse index ordered by original score");
    assert_eq!(lexicon.best_reading("吧"), Some(("ㄅㄚ˙".to_owned(), -2.0)));
    assert_eq!(lexicon.best_reading("八"), Some(("ㄅㄚ".to_owned(), -3.0)));
    assert_eq!(lexicon.best_reading("八吧"), None);
    assert_eq!((lexicon.word_index("八"), lexicon.word_index("吧"), lexicon.word_index("X")), (Some(0), Some(1), None));
    for length in 0..bytes.len() {
        std::fs::write(&path, &bytes[..length]).unwrap();
        assert!(MappedLexicon::open(&path).is_err(), "reject truncated lexicon {length}");
    }
    let mut corrupt = bytes.clone();
    corrupt[128..132].copy_from_slice(&[255; 4]);
    std::fs::write(&path, &corrupt).unwrap();
    assert!(MappedLexicon::open(&path).is_err(), "reject lexicon string outside mapping");
    assert!(MappedLexicon::open(temp.path().join("absent.bin")).is_err());
}

/// A decoder over a fixture lexicon, closing syllables as the composer does (valid, else the cheapest correction).
fn decode(set: &SyllableSet, method: KeyDecoderMethod, keys: &str, gaps: &[(usize, f64)]) -> Vec<String> {
    let closing = |reading: &str, keys: &str| {
        if set.is_valid(reading) {
            return Some(0.0);
        }
        alternatives(set, keys).first().map(|a| a.cost)
    };
    let decoder = KeyDecoder::new(method, Costs::default(), closing);
    let mut time = 100.0;
    let keys: Vec<Key> = keys
        .chars()
        .enumerate()
        .map(|(index, c)| {
            time += gaps.iter().find(|g| g.0 == index).map_or(0.12, |g| g.1);
            Key::new(c, Some(time))
        })
        .collect();
    let best = decoder.decode_best(&keys);
    best.syllables.into_iter().map(|s| s.reading).collect()
}

#[test]
fn key_run_decoding() {
    // The decoder half of "first tone without Space before an initial", "\(method) keeps exact typing", "reorder
    // repairs a swap across syllables", "timed repairs keys pressed together out of order".
    let set = syllables_of(
        "ㄐㄧㄣ\t今\t-2\nㄊㄧㄢ\t天\t-2\nㄨㄛˇ\t我\t-2\nㄇㄣ˙\t們\t-2\nㄕˋ\t是\t-2\nㄋㄧˇ\t你\t-2\nㄏㄠˇ\t好\t-2\n\
         ㄨㄚ\t挖\t-2\nㄇㄚ\t媽\t-2\n",
    );
    assert_ne!(decode(&set, KeyDecoderMethod::Slots, "rupwu0 ", &[]), ["ㄐㄧㄣ", "ㄊㄧㄢ"]);
    assert_eq!(decode(&set, KeyDecoderMethod::Boundaries, "rupwu0 ", &[]), ["ㄐㄧㄣ", "ㄊㄧㄢ"]);
    for method in KeyDecoderMethod::ALL {
        assert_eq!(decode(&set, method, "ji3ap7g4", &[]), ["ㄨㄛˇ", "ㄇㄣ˙", "ㄕˋ"], "{method:?} keeps exact typing");
        assert_eq!(decode(&set, method, "su3cl3", &[]), ["ㄋㄧˇ", "ㄏㄠˇ"]);
    }
    assert_ne!(decode(&set, KeyDecoderMethod::Boundaries, "jia3p7g4", &[]), ["ㄨㄛˇ", "ㄇㄣ˙", "ㄕˋ"]);
    assert_eq!(decode(&set, KeyDecoderMethod::Reorder, "jia3p7g4", &[]), ["ㄨㄛˇ", "ㄇㄣ˙", "ㄕˋ"]);
    assert_eq!(decode(&set, KeyDecoderMethod::Timed, "suc3l3", &[(3, 0.006)]), ["ㄋㄧˇ", "ㄏㄠˇ"]);

    // Costs: an implicit boundary costs 4; a run with no keys is one empty split.
    let decoder =
        KeyDecoder::new(KeyDecoderMethod::Timed, Costs::default(), |r: &str, _: &str| set.is_valid(r).then_some(0.0));
    let keys: Vec<Key> = "rupwu0 ".chars().map(|c| Key::new(c, None)).collect();
    let splits = decoder.decode(&keys, 6);
    assert_eq!(splits[0].cost, 4.0);
    assert!(splits.windows(2).all(|pair| pair[0].cost <= pair[1].cost) && splits.len() <= 6);
    let empty = decoder.decode(&[], 6);
    assert_eq!(empty.len(), 1);
    assert_eq!((empty[0].cost, empty[0].syllables.len(), empty[0].buffer), (0.0, 0, [None; 3]));
    // An unfinished syllable stays in the buffer with its keys.
    let open = decoder.decode_best(&"ji3a".chars().map(|c| Key::new(c, None)).collect::<Vec<_>>());
    assert_eq!(open.buffer, [Some('ㄇ'), None, None]);
    assert_eq!(open.buffer_keys, "a");
    assert_eq!(open.syllables[0].keys, "ji3");
    // Nothing closes: infinite cost.
    let none = KeyDecoder::new(KeyDecoderMethod::Slots, Costs::default(), |_: &str, _: &str| None);
    assert_eq!(none.decode_best(&[Key::new(' ', None)]).cost, f64::INFINITY);
}

#[test]
fn equal_cost_splits_rank_in_the_order_first_reached() {
    // ZhuyinCoreChecks: "equal-cost splits rank in the order first reached", "a fresh decoder decodes alike".
    let valid = ["ㄅㄚ", "ㄚ", "ㄅ"];
    let closing = |reading: &str, _: &str| valid.contains(&reading).then_some(0.0);
    let keys: Vec<Key> = "18 18 ".chars().map(|c| Key::new(c, None)).collect();
    let splits = |decoder: &KeyDecoder<_>| -> Vec<String> {
        decoder
            .decode(&keys, 6)
            .iter()
            .map(|r| {
                let syllables: Vec<String> = r.syllables.iter().map(|s| format!("{}/{}", s.reading, s.keys)).collect();
                format!("{} {}", syllables.join(","), r.cost as i64)
            })
            .collect()
    };
    let first = splits(&KeyDecoder::new(KeyDecoderMethod::Reorder, Costs::default(), closing));
    assert_eq!(
        first,
        [
            "ㄅㄚ/18 ,ㄅㄚ/18  0",
            "ㄅㄚ/18 ,ㄅ/1  10",
            "ㄅㄚ/18 ,ㄅ/1  13",
            "ㄅㄚ/18 ,ㄚ/8  13",
            "ㄅ/1 ,ㄅㄚ/18  13",
            "ㄚ/8 ,ㄅㄚ/18  13",
        ]
    );
    assert_eq!(first, splits(&KeyDecoder::new(KeyDecoderMethod::Reorder, Costs::default(), closing)));
}

//! A composition that reaches `max_syllables`: the key that does not fit first commits the head of what is shown, and
//! every key typed ends up in the commits or the composition, once.

use std::sync::{Arc, Mutex};

use zhuyin_core::api::{ComposerOptions, KeyDecoderMethod, KeyInput, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::keyboard;

/// One character per reading, so the text of any segmentation is known; the two-syllable words give word
/// boundaries a head may not cut through.
const LEXICON: &str = "ㄋㄧˇ\t你\t-2
ㄏㄠˇ\t好\t-2
ㄐㄧㄣ\t今\t-2
ㄊㄧㄢ\t天\t-2
ㄑㄧˋ\t氣\t-2
ㄏㄣˇ\t很\t-2
ㄒㄧㄝˋ\t謝\t-2
ㄇㄚ\t媽\t-2
ㄆㄚ\t趴\t-2
ㄨㄛˇ\t我\t-2
ㄇㄣ˙\t們\t-2
ㄕˋ\t是\t-2
ㄒㄩㄝˊ\t學\t-2
ㄕㄥ\t生\t-2
ㄋㄧˇ ㄏㄠˇ\t你好\t-1
ㄐㄧㄣ ㄊㄧㄢ\t今天\t-1
ㄊㄧㄢ ㄑㄧˋ\t天氣\t-1
ㄏㄣˇ ㄏㄠˇ\t很好\t-1
ㄨㄛˇ ㄇㄣ˙\t我們\t-1
ㄒㄩㄝˊ ㄕㄥ\t學生\t-1
ㄕˋ ㄒㄩㄝˊ ㄕㄥ\t是學生\t-1.5";

/// A sentence that cycles through the lexicon: (reading, character).  Every syllable starts with a letter key, so a
/// key typed with the candidate window open is never a selection key.
const CYCLE: [(&str, &str); 16] = [
    ("ㄨㄛˇ", "我"),
    ("ㄇㄣ˙", "們"),
    ("ㄐㄧㄣ", "今"),
    ("ㄊㄧㄢ", "天"),
    ("ㄏㄣˇ", "很"),
    ("ㄏㄠˇ", "好"),
    ("ㄋㄧˇ", "你"),
    ("ㄏㄠˇ", "好"),
    ("ㄕˋ", "是"),
    ("ㄒㄩㄝˊ", "學"),
    ("ㄕㄥ", "生"),
    ("ㄊㄧㄢ", "天"),
    ("ㄑㄧˋ", "氣"),
    ("ㄒㄧㄝˋ", "謝"),
    ("ㄇㄚ", "媽"),
    ("ㄆㄚ", "趴"),
];

fn engine() -> Arc<Engine> {
    Arc::new(Engine::from_text(LEXICON, None, None, EngineConfig::default()))
}

/// The keys of a reading: its symbols, then the tone key or Space.
fn keys(reading: &str) -> Vec<KeyInput> {
    let mut keys: Vec<KeyInput> = reading
        .chars()
        .filter(|&c| !keyboard::is_tone_mark(c))
        .map(|c| KeyInput::Char(keyboard::key_for_symbol(c).expect("a Zhuyin symbol")))
        .collect();
    keys.push(match reading.chars().last() {
        Some('ˊ') => KeyInput::Char('6'),
        Some('ˇ') => KeyInput::Char('3'),
        Some('ˋ') => KeyInput::Char('4'),
        Some('˙') => KeyInput::Char('7'),
        _ => KeyInput::Space,
    });
    keys
}

fn han(text: &str) -> usize {
    text.chars().filter(|&c| ('\u{4E00}'..='\u{9FFF}').contains(&c)).count()
}

/// Types into a composer and keeps what it commits.
struct Typist {
    composer: Composer,
    commits: Vec<String>,
    /// Syllables the composition may have past the limit (typing at its start, with nothing before the cursor).
    over: usize,
}

impl Typist {
    fn new(options: ComposerOptions) -> Self {
        Self { composer: Composer::new(engine(), options), commits: vec![], over: 0 }
    }

    fn press(&mut self, key: KeyInput) {
        let shown = self.composer.presentation().text.clone();
        let result = self.composer.handle(key);
        assert!(result.handled, "{key:?} was not handled");
        if let Some(commit) = result.commit {
            // Exactly what was shown, the user's picks included.
            assert!(shown.starts_with(&commit), "commit {commit:?} is not the head of {shown:?}");
            let rest = self.composer.presentation().text.clone();
            assert!(!rest.is_empty(), "an automatic commit leaves the rest composing");
            // What stays on screen does not move: the converted part after the head is still there, as it was (the
            // unfinished syllable's symbols aside).
            let converted = |text: &str| -> String {
                text.chars()
                    .filter(|&c| !('\u{3105}'..='\u{312F}').contains(&c) && !keyboard::is_tone_mark(c))
                    .collect()
            };
            let kept = converted(&shown[commit.len()..]);
            assert!(converted(&rest).starts_with(&kept), "the rest changed: {kept:?} became {rest:?}");
            self.commits.push(commit);
        }
        let limit = self.composer.options().max_syllables;
        assert!(han(&self.composer.presentation().text) <= limit + self.over, "the composition stays within the limit");
    }

    fn syllable(&mut self, reading: &str) {
        for key in keys(reading) {
            self.press(key);
        }
    }

    /// Everything committed, and the composition sent with Enter.
    fn finish(mut self) -> (String, Vec<String>) {
        let rest = self.composer.handle(KeyInput::Enter).commit.unwrap_or_default();
        let mut text: String = self.commits.concat();
        text.push_str(&rest);
        (text, self.commits)
    }
}

fn options(decoder: KeyDecoderMethod, typo: TypoTolerance) -> ComposerOptions {
    ComposerOptions { key_decoder: decoder, typo_tolerance: typo, ..ComposerOptions::default() }
}

const DECODERS: [(KeyDecoderMethod, TypoTolerance); 4] = [
    (KeyDecoderMethod::Timed, TypoTolerance::Standard),
    (KeyDecoderMethod::Boundaries, TypoTolerance::Off),
    (KeyDecoderMethod::Slots, TypoTolerance::Standard),
    (KeyDecoderMethod::Slots, TypoTolerance::Off),
];

#[test]
fn no_key_is_lost_without_punctuation() {
    for (decoder, typo) in DECODERS {
        let mut typist = Typist::new(options(decoder, typo));
        let mut expected = String::new();
        for index in 0..230 {
            let (reading, character) = CYCLE[index % CYCLE.len()];
            typist.syllable(reading);
            expected.push_str(character);
        }
        let (text, commits) = typist.finish();
        assert_eq!(text, expected, "{decoder:?}/{typo:?}: every syllable once, in order");
        assert!(commits.len() >= 8, "{decoder:?}: 230 syllables commit about every 20 ({})", commits.len());
        for commit in &commits {
            // About the first third of the limit, ending between two words.
            assert!((18..=20).contains(&han(commit)), "{decoder:?}: head {commit:?}");
            assert!(!commit.ends_with('今') && !commit.ends_with('你'), "{decoder:?}: {commit:?} cuts a word");
        }
    }
}

#[test]
fn heads_end_at_punctuation() {
    for (decoder, typo) in DECODERS {
        let mut typist = Typist::new(options(decoder, typo));
        let mut expected = String::new();
        for index in 0..240 {
            let (reading, character) = CYCLE[index % CYCLE.len()];
            typist.syllable(reading);
            expected.push_str(character);
            if index % 13 == 12 {
                typist.press(KeyInput::ControlChar(if index % 26 == 12 { ',' } else { '.' }));
                expected.push_str(if index % 26 == 12 { "，" } else { "。" });
            }
            if index % 29 == 5 {
                typist.press(KeyInput::ShiftedChar('a'));
                expected.push('A');
            }
        }
        let (text, commits) = typist.finish();
        assert_eq!(text, expected, "{decoder:?}/{typo:?}: every syllable and mark once, in order");
        for commit in &commits {
            assert!(commit.ends_with('，') || commit.ends_with('。'), "{decoder:?}: {commit:?} ends at punctuation");
            assert!(han(commit) >= 20, "{decoder:?}: at most two thirds of the limit stay ({commit:?})");
        }
    }
}

#[test]
fn candidate_window_open_and_picks() {
    for (decoder, typo) in DECODERS {
        let mut typist = Typist::new(options(decoder, typo));
        let mut expected = String::new();
        for index in 0..210 {
            if index > 0 {
                // The window is open when the next syllable's first key comes; now and then a pick fixes a word.
                typist.press(KeyInput::Down);
                assert!(typist.composer.presentation().candidates.is_some(), "the window opened");
                if index % 7 == 0 {
                    typist.press(KeyInput::Enter);
                    typist.press(KeyInput::Down);
                }
            }
            let (reading, character) = CYCLE[index % CYCLE.len()];
            typist.syllable(reading);
            expected.push_str(character);
        }
        let (text, commits) = typist.finish();
        assert_eq!(text, expected, "{decoder:?}/{typo:?}: with the window open, every syllable once");
        assert!(commits.len() >= 7, "{decoder:?}: {} commits", commits.len());
    }
}

#[test]
fn unfinished_syllable_at_the_limit() {
    // 60 syllables, then an unfinished one (its symbols on screen), then its tone and more.
    for (decoder, typo) in DECODERS {
        let mut typist = Typist::new(options(decoder, typo));
        let mut expected = String::new();
        for index in 0..60 {
            let (reading, character) = CYCLE[index % CYCLE.len()];
            typist.syllable(reading);
            expected.push_str(character);
        }
        assert!(typist.commits.is_empty(), "{decoder:?}: 60 syllables fit");
        typist.press(KeyInput::Char('j'));
        typist.press(KeyInput::Char('i'));
        assert_eq!(typist.commits.len(), 1, "{decoder:?}: the 61st syllable's first key sends the head");
        assert!(typist.composer.presentation().text.ends_with("ㄨㄛ"), "{decoder:?}: the symbols stay on screen");
        typist.press(KeyInput::Char('3'));
        expected.push('我');
        typist.syllable("ㄇㄣ˙");
        expected.push('們');
        let (text, _) = typist.finish();
        assert_eq!(text, expected, "{decoder:?}");
    }
}

#[test]
fn small_limit_reaches_into_the_run() {
    // A limit smaller than a run of keys: the head is taken from the run's syllables too.
    for (decoder, typo) in DECODERS {
        let mut typist = Typist::new(ComposerOptions { max_syllables: 4, ..options(decoder, typo) });
        let mut expected = String::new();
        for index in 0..50 {
            let (reading, character) = CYCLE[index % CYCLE.len()];
            typist.syllable(reading);
            expected.push_str(character);
        }
        let (text, commits) = typist.finish();
        assert_eq!(text, expected, "{decoder:?}/{typo:?}");
        assert!(commits.len() >= 12, "{decoder:?}: {} commits", commits.len());
    }
}

#[test]
fn cursor_inside_the_composition() {
    let mut typist = Typist::new(options(KeyDecoderMethod::Timed, TypoTolerance::Standard));
    let mut typed: Vec<&str> = vec![];
    for index in 0..60 {
        let (reading, character) = CYCLE[index % CYCLE.len()];
        typist.syllable(reading);
        typed.push(character);
    }
    // At the start nothing can go before the cursor: the composition grows past the limit by the one syllable.
    typist.press(KeyInput::Home);
    typist.over = 1;
    typist.syllable("ㄕˋ");
    assert!(typist.commits.is_empty(), "nothing before the cursor to commit");
    typed.insert(0, "是");
    // Six characters in, the head can only end before the cursor.
    for _ in 0..5 {
        typist.press(KeyInput::Right);
    }
    typist.syllable("ㄇㄚ");
    typed.insert(6, "媽");
    assert_eq!(typist.commits.len(), 1, "a syllable typed inside a full composition sends the head");
    assert!(han(&typist.commits[0]) <= 6, "the head ends before the cursor: {:?}", typist.commits[0]);
    let (text, _) = typist.finish();
    assert_eq!(text, typed.concat());
}

#[test]
fn head_learns_and_continues_the_context() {
    let engine = engine();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut composer = Composer::new(engine.clone(), options(KeyDecoderMethod::Timed, TypoTolerance::Standard));
    let listened = log.clone();
    composer.set_commit_listener(Some(Box::new(move |text| listened.lock().unwrap().push(format!("commit {text}")))));
    let reranked = log.clone();
    composer.set_reranker(Some(Box::new(move |_| {
        reranked.lock().unwrap().push("rerank".to_owned());
        None
    })));
    let mut commits = vec![];
    for index in 0..61 {
        for key in keys(CYCLE[index % CYCLE.len()].0) {
            if let Some(commit) = composer.handle(key).commit {
                commits.push(commit);
            }
        }
    }
    assert_eq!(commits.len(), 1);
    let entries = log.lock().unwrap().clone();
    let at = entries.iter().position(|e| e.starts_with("commit")).expect("the listener heard the head");
    assert_eq!(entries[at], format!("commit {}", commits[0]), "the listener hears exactly the commit");
    assert!(entries[at + 1..].iter().any(|e| e == "rerank"), "the rest is converted again after the listener");
    assert!(!engine.personal().is_empty(), "the head is learned like a commit");
}

#[test]
fn secure_input_learns_nothing() {
    let engine = engine();
    let mut composer = Composer::new(engine.clone(), options(KeyDecoderMethod::Timed, TypoTolerance::Standard));
    composer.learning_suspended = true;
    let mut text = String::new();
    let mut expected = String::new();
    for index in 0..130 {
        let (reading, character) = CYCLE[index % CYCLE.len()];
        expected.push_str(character);
        for key in keys(reading) {
            text.push_str(&composer.handle(key).commit.unwrap_or_default());
        }
    }
    text.push_str(&composer.commit_all());
    assert_eq!(text, expected);
    assert!(engine.personal().is_empty(), "nothing is learned under secure input");
}

#[test]
fn early_commit_at_sentence_ends() {
    let early = ComposerOptions { early_commit_syllables: 20, ..ComposerOptions::default() };
    let mut typist = Typist::new(early);
    let mut expected = String::new();
    for index in 0..100 {
        let (reading, character) = CYCLE[index % CYCLE.len()];
        typist.syllable(reading);
        expected.push_str(character);
        if index % 12 == 11 {
            typist.press(KeyInput::ControlChar(if index % 24 == 11 { ',' } else { '.' }));
            expected.push_str(if index % 24 == 11 { "，" } else { "。" });
        }
    }
    let (text, commits) = typist.finish();
    assert_eq!(text, expected);
    assert!(!commits.is_empty(), "sentences are sent early");
    for commit in &commits {
        assert!(commit.ends_with('。'), "only at a sentence end: {commit:?}");
    }
    // Off by default: nothing is sent below the limit.
    let mut typist = Typist::new(ComposerOptions::default());
    for index in 0..60 {
        typist.syllable(CYCLE[index % CYCLE.len()].0);
        if index % 12 == 11 {
            typist.press(KeyInput::ControlChar('.'));
        }
    }
    assert!(typist.commits.is_empty(), "no early commit by default");
}

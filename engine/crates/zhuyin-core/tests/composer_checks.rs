//! The composer checks of `ZhuyinCoreChecks` (everything before the real-lexicon section), ported check by check;
//! each assertion message is the Swift check's message.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::TempDir;
use zhuyin_core::api::{
    CandidateAnchor, ComposerOptions, KeyInput, KeyResult, PunctuationKeys, SpaceAction, TypoTolerance,
};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};

pub(crate) const TINY: &str = "# Tiny, deliberately ambiguous lexicon
ㄅㄚ\t八\t-2
ㄅㄚ\t巴\t-2.3
ㄅㄚ\t吧\t-2.4
ㄅㄚ\t芭\t-2.5
ㄅㄚ\t疤\t-2.6
ㄅㄚ\t笆\t-2.7
ㄅㄚ\t捌\t-2.8
ㄅㄚ\t粑\t-2.9
ㄅㄚ\t叭\t-3.0
ㄅㄚ\t豝\t-3.1
ㄅㄚ\t仈\t-3.2
ㄅㄚˊ\t拔\t-2
ㄅㄚˇ\t把\t-2
ㄅㄚˋ\t爸\t-2
ㄅㄚ˙\t罷\t-2
ㄆㄚ\t趴\t-2
ㄇㄚ\t媽\t-2
ㄇㄚˊ\t麻\t-2
ㄧ\t一\t-2
ㄨ\t烏\t-2
ㄩ\t迂\t-2
ㄅㄧㄢ\t邊\t-2
ㄅㄚ ㄆㄚ\t八趴\t-1
ㄅㄚ ㄆㄚ\t巴趴\t-1.2
ㄅㄚ ㄆㄚ ㄇㄚ\t八趴媽\t-0.5
ㄅㄚ ㄅㄚ\t八八\t-5
ㄋㄧˇ\t你\t-2
ㄏㄠˇ\t好\t-2
ㄋㄧˇ ㄏㄠˇ\t你好\t-1
ㄒㄧㄝˋ\t謝\t-2
ㄐㄧㄣ\t今\t-2
ㄊㄧㄢ\t天\t-2
ㄑㄧˋ\t氣\t-2
ㄏㄣˇ\t很\t-2
ㄐㄧㄣ ㄊㄧㄢ\t今天\t-1
ㄊㄧㄢ ㄑㄧˋ\t天氣\t-1
ㄏㄣˇ ㄏㄠˇ\t很好\t-1";

fn engine(lexicon: &str) -> Arc<Engine> {
    Arc::new(Engine::from_text(lexicon, None, None, EngineConfig::default()))
}

fn engine_with(lexicon: &str, user: Option<&Path>, model: Option<&Path>) -> Arc<Engine> {
    Arc::new(Engine::from_text(lexicon, user, model, EngineConfig::default()))
}

fn legacy() -> ComposerOptions {
    ComposerOptions {
        typo_tolerance: TypoTolerance::Off,
        punctuation_keys: PunctuationKeys::Mac,
        ..ComposerOptions::default()
    }
}

fn key_of(c: char) -> KeyInput {
    if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) }
}

fn type_keys(keys: &str, composer: &mut Composer) {
    for c in keys.chars() {
        composer.handle(key_of(c));
    }
}

fn composer_with(engine: &Arc<Engine>, keys: &str, options: ComposerOptions) -> Composer {
    let mut c = Composer::new(engine.clone(), options);
    type_keys(keys, &mut c);
    c
}

fn composer(engine: &Arc<Engine>, keys: &str) -> Composer {
    composer_with(engine, keys, legacy())
}

fn text(c: &Composer) -> &str {
    &c.presentation().text
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
    if let Some(index) = p.focused_clause {
        assert!(index < p.clauses.len(), "{label}: valid focused clause");
    }
}

#[test]
fn keys_symbols_and_tones() {
    let tiny = engine(TINY);
    let mut empty = composer(&tiny, "");
    for key in [
        KeyInput::Space,
        KeyInput::ShiftSpace,
        KeyInput::Enter,
        KeyInput::ShiftEnter,
        KeyInput::Backspace,
        KeyInput::OptionBackspace,
        KeyInput::ForwardDelete,
        KeyInput::Left,
        KeyInput::Right,
        KeyInput::ShiftLeft,
        KeyInput::ShiftRight,
        KeyInput::Home,
        KeyInput::End,
        KeyInput::Up,
        KeyInput::Down,
        KeyInput::PageUp,
        KeyInput::PageDown,
        KeyInput::Tab,
        KeyInput::ShiftTab,
        KeyInput::Escape,
        KeyInput::Char('?'),
    ] {
        assert_eq!(empty.handle(key), KeyResult::PASS_THROUGH, "empty pass-through {key:?}");
    }
    for key in "6347".chars() {
        assert!(
            empty.handle(KeyInput::Char(key)) == KeyResult::CONSUMED && !empty.is_composing(),
            "empty tone ignored"
        );
    }
    for (key, symbol) in zhuyin_core::keyboard::symbol_pairs() {
        assert_eq!(text(&composer(&tiny, &key.to_string())), symbol.to_string(), "keyboard {key} = {symbol}");
    }
    assert_eq!(text(&composer(&tiny, "1q8iujm")), "ㄆㄩㄛ", "replace all three slots");
    let mut ordered = composer(&tiny, "0u1");
    assert_eq!(text(&ordered), "ㄅㄧㄢ", "slot order independent of key order");
    ordered.handle(KeyInput::Space);
    assert_eq!(text(&ordered), "邊", "first tone completes");
    for (tone, word) in [("6", "拔"), ("3", "把"), ("4", "爸"), ("7", "罷")] {
        assert_eq!(text(&composer(&tiny, &format!("18{tone}"))), word, "tone {tone}");
    }
    let mut rejected = composer(&tiny, "q86");
    assert_eq!(text(&rejected), "ㄆㄚ", "invalid tone retains buffer without tone");
    rejected.handle(KeyInput::Space);
    assert_eq!(text(&rejected), "趴", "rejected tone can be corrected");
    let mut invalid = composer(&tiny, "1u3");
    assert_eq!(text(&invalid), "ㄅㄧ", "unknown syllable rejected");
    invalid.handle(KeyInput::Space);
    assert_eq!(text(&invalid), "ㄅㄧ", "invalid first tone rejected");

    let phrase_only = engine("ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\t-2\n");
    assert_eq!(text(&composer(&phrase_only, "vu,4vu,7")), "謝謝", "phrase-only syllables complete");
    assert_eq!(text(&composer(&phrase_only, "vu,4vu,4")), "謝謝", "typed fourth tone matches neutral phrase ending");
    let non_neutral = engine("ㄒㄧㄝˋ ㄒㄧㄝˋ\t謝謝\t-2\n");
    assert_eq!(
        text(&composer(&non_neutral, "vu,7vu,7")),
        "謝謝",
        "neutral syllables valid through non-neutral phrase only"
    );
    let neutral_only = engine("ㄅㄚ˙\t吧\t-2\n");
    for keys in ["18 ", "186", "183", "184", "187"] {
        assert_eq!(text(&composer(&neutral_only, keys)), "吧", "neutral lexicon accepts any typed tone: {keys}");
    }
    let fourth_only = engine("ㄅㄚˋ\t爸\t-2\n");
    assert_eq!(text(&composer(&fourth_only, "187")), "爸", "typed neutral accepts another tone");
    assert_eq!(text(&composer(&fourth_only, "183")), "ㄅㄚ", "different non-neutral tones never match");
}

#[test]
fn tolerance_penalties_and_readings() {
    for (typed, source, keys, penalty) in [
        ("ㄅㄚ", "ㄅㄚ˙", "18 ", 0.4),
        ("ㄅㄚˊ", "ㄅㄚ˙", "186", 0.4),
        ("ㄅㄚˇ", "ㄅㄚ˙", "183", 0.4),
        ("ㄅㄚˋ", "ㄅㄚ˙", "184", 0.4),
        ("ㄅㄚ˙", "ㄅㄚ", "187", 1.2),
        ("ㄅㄚ˙", "ㄅㄚˊ", "187", 1.2),
        ("ㄅㄚ˙", "ㄅㄚˇ", "187", 1.2),
        ("ㄅㄚ˙", "ㄅㄚˋ", "187", 1.2),
        ("ㄅㄚ ㄆㄚ", "ㄅㄚ˙ ㄆㄚ˙", "18 q8 ", 0.8),
        ("ㄅㄚ˙ ㄆㄚ˙", "ㄅㄚ ㄆㄚ", "187q87", 2.4),
        ("ㄅㄚ ㄆㄚ˙", "ㄅㄚ˙ ㄆㄚˋ", "18 q87", 1.6),
    ] {
        let (word, rival) = if typed.contains(' ') { ("甲乙", "丙丁") } else { ("甲", "丙") };
        for delta in [-0.01, 0.01] {
            let e = engine(&format!("{source}\t{word}\t-2\n{typed}\t{rival}\t{}\n", -2.0 - penalty + delta));
            let mut c = composer(&e, keys);
            let expected = if delta < 0.0 { word } else { rival };
            assert_eq!(text(&c), expected, "{penalty} penalty: {typed} vs {source}, delta {delta}");
            c.handle(KeyInput::Down);
            assert_eq!(
                items(&c)[..2],
                [expected, if delta < 0.0 { rival } else { word }],
                "candidate ranking includes adjusted tolerant score"
            );
        }
    }
    let exact_preferred = engine("ㄅㄚ\t甲\t-5\nㄅㄚ˙\t甲\t0\nㄅㄚ\t乙\t-4\n");
    let mut exact_choice = composer(&exact_preferred, "18 ");
    assert_eq!(text(&exact_choice), "甲", "a word's likelier tolerant reading beats its rare exact one");
    exact_choice.handle(KeyInput::Down);
    assert_eq!(items(&exact_choice), ["甲", "乙"], "candidates keep one entry per word");
    let same_reading = engine("ㄅㄚ\t甲\t-1\nㄅㄚ˙\t甲\t-1\nㄅㄚ\t乙\t-1.2\n");
    assert_eq!(
        text(&composer(&same_reading, "18 ")),
        "甲",
        "equally likely readings: the exact one, without tolerance cost"
    );
    let phrase_only = engine("ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\t-2\n");
    let mut tolerant_pick = composer(&phrase_only, "vu,4vu,4");
    tolerant_pick.handle(KeyInput::Down);
    assert_eq!(items(&tolerant_pick)[0], "謝謝", "tolerant phrase offered at end cursor");
    tolerant_pick.select_candidate(0);
    tolerant_pick.handle(KeyInput::Left);
    assert_eq!(text(&tolerant_pick), "謝謝", "tolerant pick fixed through navigation");
    assert_eq!(tolerant_pick.commit_all(), "謝謝", "tolerant pick commits");
    let learned = phrase_only.learned_entries();
    assert!(
        learned.iter().any(|(k, _)| k.reading == "ㄒㄧㄝˋ ㄒㄧㄝˋ" && k.word == "謝謝"),
        "a committed tolerant pick is recalled under its typed reading"
    );
    assert!(!learned.iter().any(|(k, _)| k.reading.contains('˙')), "learning does not leak into lexicon reading");
    let end_words = engine("ㄏㄣˇ\t很\t-2\nㄏㄠˇ\t好\t-2\nㄏㄠˇ\t郝\t-3\nㄏㄣˇ ㄏㄠˇ\t很好\t-1\n");
    let mut end_candidates = composer(&end_words, "cp3cl3");
    end_candidates.handle(KeyInput::Down);
    assert_eq!(items(&end_candidates), ["很好", "好", "郝"], "end cursor offers phrase before final syllable");
}

#[test]
fn editing_and_navigation() {
    let tiny = engine(TINY);
    let mut deletion = composer(&tiny, "1u0");
    for expected in ["ㄅㄧ", "ㄅ", ""] {
        deletion.handle(KeyInput::Backspace);
        assert_eq!(text(&deletion), expected, "backspace one buffer symbol");
    }
    type_keys("18 q8 a8 ", &mut deletion);
    assert_eq!(text(&deletion), "八趴媽", "Viterbi picks three-syllable word");
    deletion.handle(KeyInput::Backspace);
    assert_eq!(text(&deletion), "八趴", "backspace one token, never whole word");
    deletion.handle(KeyInput::Backspace);
    assert_eq!(text(&deletion), "八", "backspace retains earlier token");
    let mut word_delete = composer(&tiny, "18 q8 a8 ");
    word_delete.handle(KeyInput::Left);
    word_delete.handle(KeyInput::OptionBackspace);
    assert!(!word_delete.is_composing(), "option-backspace removes clause containing token before cursor");
    let mut buffer_delete = composer(&tiny, "18 q8");
    buffer_delete.handle(KeyInput::OptionBackspace);
    assert_eq!(text(&buffer_delete), "八", "option-backspace only clears buffer");

    let mut middle = composer(&tiny, "18 q8 ");
    middle.handle(KeyInput::Left);
    assert_eq!(middle.presentation().cursor, 1, "left one token inside word");
    assert_eq!(
        middle.presentation().target_clause.map(|i| middle.presentation().clauses[i].clone()),
        Some(1..2),
        "with the cursor inside the composition, the character ↓ would change is its own clause"
    );
    type_keys("a8", &mut middle);
    assert!(text(&middle) == "八ㄇㄚ趴" && middle.presentation().cursor == 3, "buffer at token cursor");
    valid_presentation(&middle, "buffer inside a word");
    middle.handle(KeyInput::Left);
    middle.handle(KeyInput::Right);
    assert_eq!(middle.presentation().cursor, 3, "left/right ignored with buffer");
    middle.handle(KeyInput::ForwardDelete);
    assert_eq!(text(&middle), "八ㄇㄚ趴", "forward-delete ignored with buffer");
    middle.handle(KeyInput::Space);
    assert!(text(&middle) == "八媽趴" && middle.presentation().cursor == 2, "insert completed token in middle");
    middle.handle(KeyInput::ForwardDelete);
    assert_eq!(text(&middle), "八媽", "forward-delete removes next token");
    middle.handle(KeyInput::Home);
    assert_eq!(middle.presentation().cursor, 0, "home");
    assert!(
        middle.handle(KeyInput::Backspace) == KeyResult::CONSUMED && text(&middle) == "八媽",
        "backspace at start consumed"
    );
    middle.handle(KeyInput::Left);
    assert_eq!(middle.presentation().cursor, 0, "left clamps");
    middle.handle(KeyInput::End);
    middle.handle(KeyInput::Right);
    assert_eq!(middle.presentation().cursor, 2, "end/right clamp");
    assert_eq!(middle.handle(KeyInput::ForwardDelete), KeyResult::CONSUMED, "forward-delete at end consumed");
    let mut move_buffer = composer(&tiny, "18 q8");
    move_buffer.handle(KeyInput::Home);
    assert!(
        text(&move_buffer) == "ㄆㄚ八" && move_buffer.presentation().cursor == 2,
        "home relocates unfinished buffer"
    );
    move_buffer.handle(KeyInput::End);
    assert_eq!(text(&move_buffer), "八ㄆㄚ", "end relocates unfinished buffer");
    for key in [KeyInput::Up, KeyInput::PageUp, KeyInput::PageDown, KeyInput::Tab, KeyInput::ShiftTab] {
        assert_eq!(middle.handle(key), KeyResult::CONSUMED, "composition consumes {key:?}");
    }
}

#[test]
// One test per section of the Swift checks.
#[allow(clippy::too_many_lines)]
fn literals_and_punctuation_keys() {
    let tiny = engine(TINY);
    let plain = [('[', "「"), (']', "」"), ('\'', "、"), ('\\', "＼"), ('=', "＝")];
    let shifted = [
        (',', "，"),
        ('.', "。"),
        ('/', "？"),
        (';', "："),
        ('\'', "；"),
        ('1', "！"),
        ('2', "＠"),
        ('3', "＃"),
        ('4', "＄"),
        ('5', "％"),
        ('6', "…"),
        ('7', "＆"),
        ('8', "＊"),
        ('9', "（"),
        ('0', "）"),
        ('-', "——"),
        ('=', "＋"),
        ('[', "『"),
        (']', "』"),
        ('\\', "｜"),
        ('`', "～"),
    ];
    for (is_shifted, table) in [(false, &plain[..]), (true, &shifted[..])] {
        for &(key, expected) in table {
            let mut c = composer(&tiny, "");
            let key_input = if is_shifted { KeyInput::ShiftedChar(key) } else { KeyInput::Char(key) };
            assert!(
                c.handle(key_input) == KeyResult::CONSUMED && text(&c) == expected,
                "literal mapping {key}, shift={is_shifted}"
            );
            valid_presentation(&c, "literal");
        }
    }
    let windows = ComposerOptions { punctuation_keys: PunctuationKeys::Windows, ..legacy() };
    assert_eq!(
        ComposerOptions::default().punctuation_keys,
        PunctuationKeys::Windows,
        "Windows punctuation keys by default"
    );
    for (key, expected) in [
        (',', "<"),
        ('.', ">"),
        ('/', "?"),
        (';', ":"),
        ('\'', "\""),
        ('1', "!"),
        ('6', "^"),
        ('-', "_"),
        ('=', "+"),
        ('[', "{"),
        (']', "}"),
        ('\\', "|"),
        ('`', "~"),
    ] {
        let mut c = composer_with(&tiny, "", windows.clone());
        assert!(
            c.handle(KeyInput::ShiftedChar(key)) == KeyResult::CONSUMED && text(&c) == expected,
            "Windows Shift+{key} types {expected}"
        );
    }
    for (key, expected) in [
        (',', "，"),
        ('.', "。"),
        ('\'', "、"),
        (';', "；"),
        (':', "："),
        ('?', "？"),
        ('!', "！"),
        ('{', "『"),
        ('}', "』"),
        ('(', "（"),
        (')', "）"),
        ('<', "《"),
        ('>', "》"),
        ('-', "－"),
        ('_', "——"),
        ('=', "＝"),
        ('~', "～"),
    ] {
        let mut c = composer_with(&tiny, "", windows.clone());
        assert!(
            c.handle(KeyInput::ControlChar(key)) == KeyResult::CONSUMED && text(&c) == expected,
            "Control+{key} types {expected}"
        );
    }
    let mut emoticon = composer_with(&tiny, "", windows.clone());
    for key in [KeyInput::ShiftedChar('t'), KeyInput::ShiftedChar('-'), KeyInput::ShiftedChar('t')] {
        emoticon.handle(key);
    }
    assert_eq!(text(&emoticon), "T_T", "T_T with Windows keys");
    let mut control_bracket = composer_with(&tiny, "18 ", windows);
    let bracket_result = control_bracket.handle(KeyInput::ControlChar('['));
    assert!(
        !bracket_result.handled && bracket_result.commit.as_deref() == Some("八") && !control_bracket.is_composing(),
        "Control+[ sends the composition and goes to the app"
    );
    assert_eq!(
        composer(&tiny, "").handle(KeyInput::ControlChar('[')),
        KeyResult::PASS_THROUGH,
        "Control+[ with nothing composed is the app's"
    );
    for key in 'a'..='z' {
        let mut c = composer(&tiny, "");
        c.handle(KeyInput::ShiftedChar(key));
        assert_eq!(text(&c), key.to_ascii_uppercase().to_string(), "uppercase literal {key}");
    }
    let mut punctuation = composer(&tiny, "18");
    punctuation.handle(KeyInput::ShiftedChar(','));
    assert_eq!(text(&punctuation), "八，", "punctuation completes buffer as first tone");
    type_keys("1u", &mut punctuation);
    punctuation.handle(KeyInput::ShiftedChar('a'));
    assert_eq!(text(&punctuation), "八，A", "uppercase drops invalid buffer");
    type_keys("q8 ", &mut punctuation);
    assert_eq!(text(&punctuation), "八，A趴", "words never cross literals");
    punctuation.handle(KeyInput::ShiftSpace);
    assert_eq!(text(&punctuation), "八，A趴 ", "shift-space literal");
    let mut literal_delete = composer(&tiny, "");
    literal_delete.handle(KeyInput::ShiftedChar('-'));
    literal_delete.handle(KeyInput::Backspace);
    assert!(!literal_delete.is_composing(), "multi-character literal is one token");
}

#[test]
// One block per Swift check group, kept together.
#[allow(clippy::too_many_lines)]
fn commit_modes_escape_and_limits() {
    let tiny = engine(TINY);
    for shifted in [false, true] {
        let mut c = composer(&tiny, "18 q8");
        let result = c.handle(if shifted { KeyInput::ShiftEnter } else { KeyInput::Enter });
        assert!(
            result == KeyResult::new(!shifted, Some("八ㄆㄚ".into())) && !c.is_composing(),
            "enter/shift-enter sends the unfinished syllable as typed"
        );
        type_keys("18 1u", &mut c);
        assert_eq!(c.commit_all(), "八ㄅㄧ", "commit sends the unfinished symbols, never a guess");
    }
    let mut commit_space = legacy();
    commit_space.set_space_opens_candidates(false);
    let mut spaced = composer_with(&tiny, "18 ", commit_space);
    assert!(
        spaced.handle(KeyInput::Space).commit.as_deref() == Some("八") && !spaced.is_composing(),
        "space option commits"
    );
    assert_eq!(spaced.handle(KeyInput::Space), KeyResult::PASS_THROUGH, "space with nothing composed goes to the app");
    assert_eq!(
        ComposerOptions::default().space_action,
        SpaceAction::OpenCandidates,
        "the engine default for Space opens the candidate window"
    );
    for action in SpaceAction::ALL {
        let options = ComposerOptions { space_action: action, ..legacy() };
        let mut typing = composer_with(&tiny, "18 q8", options.clone());
        assert!(
            typing.handle(KeyInput::Space) == KeyResult::CONSUMED
                && text(&typing) == "八趴"
                && typing.presentation().candidates.is_none(),
            "{action:?}: space completes a first-tone syllable"
        );
        let result = typing.handle(KeyInput::Space);
        match action {
            SpaceAction::OpenCandidates => {
                assert!(
                    result == KeyResult::CONSUMED && typing.presentation().candidates.is_some(),
                    "openCandidates: space opens the list"
                );
                typing.handle(KeyInput::Space);
                assert!(
                    typing.is_composing() && typing.presentation().candidates.is_some(),
                    "openCandidates: space in the list pages, keeps composing"
                );
            }
            SpaceAction::Commit => {
                assert!(
                    result == KeyResult::new(true, Some("八趴".into())) && !typing.is_composing(),
                    "commit: space commits, app gets nothing"
                );
            }
            SpaceAction::CommitAndSpace => {
                assert!(
                    result == KeyResult::new(false, Some("八趴".into())) && !typing.is_composing(),
                    "commitAndSpace: space commits and goes on to the app"
                );
            }
            SpaceAction::InsertSpace => {
                assert!(
                    result == KeyResult::CONSUMED
                        && typing.is_composing()
                        && text(&typing) == "八趴 "
                        && typing.presentation().candidates.is_none(),
                    "insertSpace: space goes into the composition, nothing is sent"
                );
                type_keys("18 ", &mut typing);
                assert_eq!(
                    typing.handle(KeyInput::Enter),
                    KeyResult::new(true, Some("八趴 八".into())),
                    "insertSpace: Enter sends the text with its space"
                );
            }
        }
        let mut listed = composer_with(&tiny, "18 ", options);
        listed.handle(KeyInput::Down);
        assert!(
            listed.handle(KeyInput::Space) == KeyResult::CONSUMED && listed.is_composing(),
            "{action:?}: space in an open list stays in the list"
        );
    }
    let mut escape = composer(&tiny, "18 q8");
    escape.handle(KeyInput::Escape);
    assert_eq!(text(&escape), "八", "escape clears only buffer first");
    escape.handle(KeyInput::Escape);
    assert!(!escape.is_composing(), "escape clears composition");
    let no_escape = ComposerOptions { escape_clears_composition: false, ..ComposerOptions::default() };
    let mut kept = composer_with(&tiny, "18 ", no_escape);
    kept.handle(KeyInput::Escape);
    assert_eq!(text(&kept), "八", "escape option retains composition");
    let limited_options = ComposerOptions { max_syllables: 2, ..legacy() };
    let mut limited = composer_with(&tiny, "18 q8 ", limited_options.clone());
    assert!(
        limited.handle(KeyInput::Char('6')) == KeyResult::CONSUMED && text(&limited) == "八趴",
        "tone at limit empty ignored"
    );
    limited.handle(KeyInput::ShiftedChar('a'));
    assert_eq!(text(&limited), "八趴A", "literal allowed at limit");
    assert_eq!(
        limited.handle(KeyInput::Char('a')),
        KeyResult::new(true, Some("八趴".into())),
        "maxSyllables: a key past the limit commits the head instead of being refused"
    );
    assert_eq!(text(&limited), "Aㄇ", "the key starts the rest");
    type_keys("8 ", &mut limited);
    assert_eq!(text(&limited), "A媽");
    let mut deleting = composer_with(&tiny, "18 q8 ", limited_options);
    deleting.handle(KeyInput::Backspace);
    type_keys("a8 ", &mut deleting);
    assert_eq!(text(&deleting), "八媽", "deleting a syllable restores capacity");
}

#[test]
#[allow(clippy::too_many_lines)]
fn candidate_window() {
    let tiny = engine(TINY);
    let two_keys = ComposerOptions { selection_keys: vec!['a', 's'], ..legacy() };
    let mut candidates = composer_with(&tiny, "18 q8 ", two_keys.clone());
    candidates.handle(KeyInput::Space);
    assert_eq!(
        items(&candidates).into_iter().filter(|t| t != "ㄆㄚ").take(2).collect::<Vec<_>>(),
        ["八趴", "巴趴"],
        "current first, longer spans before singles"
    );
    assert_eq!(items(&candidates).last().map(String::as_str), Some("趴"), "short span after long spans");
    assert_eq!(candidates.presentation().focused_clause, Some(0), "focused converted word");
    valid_presentation(&candidates, "candidate window");
    let index = index_of(&candidates, "巴趴");
    candidates.select_candidate(index);
    assert!(
        text(&candidates) == "巴趴"
            && candidates.presentation().candidates.is_none()
            && candidates.presentation().cursor == 2,
        "selection key picks and fixes, cursor unchanged"
    );
    candidates.handle(KeyInput::Space);
    assert_eq!(items(&candidates)[0], "巴趴", "fixed current first despite lower score");
    candidates.handle(KeyInput::Escape);
    assert_eq!(text(&candidates), "巴趴", "escape closes without changing");
    candidates.handle(KeyInput::Space);
    candidates.handle(KeyInput::Backspace);
    assert!(
        text(&candidates) == "巴趴" && candidates.presentation().candidates.is_none(),
        "backspace only closes window"
    );
    let mut paging = composer_with(&tiny, "18 ", two_keys.clone());
    paging.handle(KeyInput::Down);
    let list = |c: &Composer| c.presentation().candidates.clone().unwrap();
    assert_eq!(list(&paging).page_size(), 2, "page size from selection keys");
    paging.handle(KeyInput::Up);
    assert_eq!(list(&paging).selection(), 0, "up clamps at start");
    paging.handle(KeyInput::Down);
    paging.handle(KeyInput::Down);
    assert!(list(&paging).selection() == 2 && list(&paging).page() == 1, "down crosses page");
    paging.handle(KeyInput::PageUp);
    assert_eq!(list(&paging).page(), 0, "page up previous page");
    for key in [KeyInput::PageDown, KeyInput::Tab] {
        let previous = list(&paging).page();
        paging.handle(key);
        assert_eq!(list(&paging).page(), previous + 1, "next page {key:?}");
    }
    for key in [KeyInput::PageUp, KeyInput::ShiftTab] {
        let previous = list(&paging).page();
        paging.handle(key);
        assert_eq!(list(&paging).page() + 1, previous, "previous page {key:?}");
    }
    for _ in 0..30 {
        paging.handle(KeyInput::Down);
    }
    assert_eq!(list(&paging).selection(), list(&paging).items().len() - 1, "down clamps at final item");
    paging.handle(KeyInput::Space);
    assert_eq!(list(&paging).page(), 0, "space wraps to first page");
    paging.handle(KeyInput::Right);
    let highlighted = list(&paging).items()[list(&paging).selection()].text.clone();
    paging.handle(KeyInput::Enter);
    assert!(text(&paging) == highlighted && paging.presentation().candidates.is_none(), "enter picks highlighted");
    tiny.forget_all_learned();
    let mut dismiss = composer_with(&tiny, "18 ", two_keys);
    dismiss.handle(KeyInput::Space);
    dismiss.handle(KeyInput::Char('q'));
    assert!(text(&dismiss) == "八ㄆ" && dismiss.presentation().candidates.is_none(), "symbol dismisses then processes");
    dismiss.handle(KeyInput::Escape);
    dismiss.handle(KeyInput::Space);
    dismiss.handle(KeyInput::Char('6'));
    assert!(
        text(&dismiss) == "八" && dismiss.presentation().candidates.is_none(),
        "non-selection tone dismisses then processes"
    );
    dismiss.handle(KeyInput::Space);
    dismiss.handle(KeyInput::ShiftedChar(','));
    assert!(
        text(&dismiss) == "八，" && dismiss.presentation().candidates.is_none(),
        "punctuation dismisses then processes"
    );
    let mut down_buffer = composer(&tiny, "18");
    down_buffer.handle(KeyInput::Down);
    assert!(
        text(&down_buffer) == "八" && down_buffer.presentation().candidates.is_some(),
        "down completes valid buffer"
    );
    let mut at_start = composer(&tiny, "18 q8 a8 ");
    at_start.handle(KeyInput::Home);
    at_start.handle(KeyInput::Down);
    assert_eq!(items(&at_start)[0], "八趴媽", "cursor zero candidates start at zero");
    assert!(items(&at_start).iter().any(|t| t == "八趴"), "cursor zero offers shorter spans");
    let mut punct = composer(&tiny, "");
    punct.handle(KeyInput::ShiftedChar(','));
    punct.handle(KeyInput::Down);
    assert_eq!(items(&punct)[..3], ["，", ",", "、"], "punctuation alternatives");
    punct.select_candidate(1);
    assert!(text(&punct) == "," && punct.presentation().cursor == 1, "literal candidate pick");
    assert_eq!(punct.select_candidate(usize::MAX), KeyResult::CONSUMED, "invalid click safe");
    let mut bracket = composer(&tiny, "[");
    bracket.handle(KeyInput::Down);
    assert_eq!(items(&bracket), ["「", "『", "（", "〈", "《", "【"], "opening bracket alternatives");
    let three_keys =
        ComposerOptions { selection_keys: vec!['1', '2', '3'], learning_enabled: false, ..ComposerOptions::default() };
    let mut moving = composer_with(&tiny, "18 q8 ", three_keys.clone());
    moving.handle(KeyInput::Home);
    moving.handle(KeyInput::ShiftDown);
    assert!(
        list(&moving).single_character() && focused_range(&moving) == Some(0..1),
        "single list at the first character"
    );
    moving.handle(KeyInput::Right);
    assert!(
        focused_range(&moving) == Some(1..2) && list(&moving).single_character(),
        "→ moves the list to the next character"
    );
    moving.handle(KeyInput::Right);
    assert_eq!(focused_range(&moving), Some(1..2), "→ stops at the last character");
    moving.handle(KeyInput::Left);
    assert_eq!(focused_range(&moving), Some(0..1), "← moves the list to the previous character");
    let horizontal = ComposerOptions { vertical_list: false, ..three_keys.clone() };
    let mut sideways = composer_with(&tiny, "18 ", horizontal);
    sideways.handle(KeyInput::Down);
    sideways.handle(KeyInput::Right);
    assert_eq!(list(&sideways).selection(), 1, "horizontal → moves the highlight");
    sideways.handle(KeyInput::Down);
    assert_eq!(list(&sideways).page(), 1, "horizontal ↓ turns the page");
    sideways.handle(KeyInput::Up);
    assert_eq!(list(&sideways).page(), 0, "horizontal ↑ turns back");
    let mut page_pick = composer_with(&tiny, "18 ", three_keys.clone());
    page_pick.handle(KeyInput::Down);
    page_pick.handle(KeyInput::Tab);
    let page_word = items(&page_pick)[4].clone();
    page_pick.handle(KeyInput::Char('2'));
    assert_eq!(text(&page_pick), page_word, "selection key is relative to current page");
    let mut numeric_tone = composer_with(&tiny, "18 ", three_keys);
    numeric_tone.handle(KeyInput::Down);
    let third = items(&numeric_tone)[2].clone();
    numeric_tone.handle(KeyInput::Char('3'));
    assert!(
        text(&numeric_tone) == third && numeric_tone.presentation().candidates.is_none(),
        "selection key takes precedence over tone"
    );
    let dedupe = engine(&format!("{TINY}\nㄅㄚ\t八\t-2\n"));
    let mut deduped = composer(&dedupe, "18 ");
    deduped.handle(KeyInput::Down);
    assert_eq!(items(&deduped).iter().filter(|t| *t == "八").count(), 1, "candidate text deduplicated");
    let interior_engine = engine(&format!("{TINY}\nㄅㄚ ㄆㄚ\t巴趴\t-0.1\n"));
    let mut interior = composer(&interior_engine, "18 q8 a8 ");
    interior.handle(KeyInput::Left);
    interior.handle(KeyInput::Down);
    assert_eq!(items(&interior)[0], "媽", "after-cursor candidates start inside a longer word");
    let mut options = interior.options().clone();
    options.candidate_anchor = CandidateAnchor::BeforeCursor;
    interior.set_options(options);
    interior.handle(KeyInput::Down);
    assert_eq!(items(&interior)[0], "八趴", "before-cursor shown text first inside a longer word");
    let inside_engine = engine(&format!("{TINY}\nㄆㄚ\t啪\t-3\nㄅㄚ ㄆㄚ\t疤趴\t-0.5\n"));
    let mut inside = composer(&inside_engine, "18 q8 ");
    inside.handle(KeyInput::Left);
    inside.handle(KeyInput::Down);
    let index = index_of(&inside, "啪");
    inside.select_candidate(index);
    assert_eq!(text(&inside), "疤啪", "a pick inside a word keeps the rest of that word");
    let six_engine = engine(&format!(
        "{TINY}\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t八八八八八八\t-0.1\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t巴巴巴巴巴巴巴\t0\n"
    ));
    let mut six = composer(&six_engine, &"18 ".repeat(7));
    assert!(six.presentation().clauses.len() == 2 && !text(&six).contains('巴'), "word spans capped at six syllables");
    six.handle(KeyInput::Down);
    let index = index_of(&six, "八");
    six.select_candidate(index);
    six.handle(KeyInput::Down);
    assert_eq!(items(&six)[0], "八八八八八八", "longest span precedes current single word");
    assert_eq!(items(&six).last().unwrap().chars().count(), 1, "single-syllable spans come after all longer spans");
}

fn fixed_pair(e: &Arc<Engine>) -> Composer {
    let mut c = composer(e, "18 q8 ");
    c.handle(KeyInput::Down);
    let index = index_of(&c, "巴趴");
    c.select_candidate(index);
    c
}

#[test]
fn fixed_spans_and_learning() {
    let tiny = engine(TINY);
    let mut fixed = fixed_pair(&tiny);
    type_keys("a8 ", &mut fixed);
    assert_eq!(text(&fixed), "巴趴媽", "hard span beats higher-scoring longer word");
    fixed.handle(KeyInput::Backspace);
    assert_eq!(text(&fixed), "巴趴", "delete after fixed span preserves it");
    fixed.handle(KeyInput::Home);
    type_keys("a8 ", &mut fixed);
    assert_eq!(text(&fixed), "媽巴趴", "insert before fixed span shifts it");
    fixed.handle(KeyInput::Backspace);
    assert_eq!(text(&fixed), "巴趴", "delete before fixed span shifts it");
    fixed.handle(KeyInput::Right);
    type_keys("a8 ", &mut fixed);
    assert_eq!(text(&fixed), "八媽趴", "insert inside fixed span drops constraint");
    let mut overlap = fixed_pair(&tiny);
    overlap.handle(KeyInput::Backspace);
    assert_eq!(text(&overlap), "八", "deleting fixed token removes constraint");

    let learned_engine = engine(TINY);
    let mut learned_composer = composer(&learned_engine, "18 ");
    learned_composer.handle(KeyInput::Down);
    let index = index_of(&learned_composer, "巴");
    learned_composer.select_candidate(index);
    assert!(learned_engine.learned_entries().is_empty(), "a pick alone does not learn");
    assert_eq!(learned_composer.commit_all(), "巴", "picked word committed");
    assert!(
        learned_engine.learned_entries().iter().any(|(k, _)| k.reading == "ㄅㄚ" && k.word == "巴"),
        "commit learns the correction"
    );
    assert_eq!(text(&composer(&learned_engine, "18 ")), "巴", "learning boosts next conversion");
    let before_reset = learned_engine.learned_entries()[0].1;
    let mut resetter = composer(&learned_engine, "18 ");
    resetter.reset();
    assert!(learned_engine.learned_entries()[0].1 == before_reset && !resetter.is_composing(), "reset never learns");
    learned_engine.forget_learned("巴", "ㄅㄚ");
    assert!(
        learned_engine.learned_entries().is_empty() && text(&composer(&learned_engine, "18 ")) == "八",
        "forget removes boost"
    );
    let no_learning = ComposerOptions { learning_enabled: false, ..ComposerOptions::default() };
    let mut disabled = composer_with(&learned_engine, "18 ", no_learning);
    disabled.handle(KeyInput::Down);
    disabled.select_candidate(1);
    disabled.commit_all();
    assert!(learned_engine.learned_entries().is_empty(), "learning option respected for picks and commits");
    let _ = fixed_pair(&learned_engine);
    learned_engine.forget_all_learned();
    assert!(learned_engine.learned_entries().is_empty(), "forget all");
}

#[test]
fn user_phrases_and_persistence_through_the_composer() {
    let personal = engine(TINY);
    assert_eq!(personal.readings("八"), ["ㄅㄚ"], "known reading");
    assert_eq!(personal.readings("媽八"), ["ㄇㄚ ㄅㄚ"], "unknown phrase combines most likely character readings");
    assert!(personal.readings("龘").is_empty() && personal.readings("").is_empty(), "unknown or empty word");
    personal.add_user_phrase("新趴", "ㄅㄚ ㄆㄚ").unwrap();
    assert!(personal.user_phrases().len() == 1 && personal.readings("新趴") == ["ㄅㄚ ㄆㄚ"], "add personal phrase");
    let phrase_engine = engine("ㄅㄚ\t八\t-9\nㄅㄚ\t巴\t-10\n");
    phrase_engine.add_user_phrase("巴", "ㄅㄚ").unwrap();
    assert_eq!(text(&composer(&phrase_engine, "18 ")), "巴", "personal phrase floor and +1");
    personal.remove_user_phrase("新趴", "ㄅㄚ ㄆㄚ");
    assert!(personal.user_phrases().is_empty(), "remove personal phrase");
    let phrase_updates = engine("ㄅㄚ\t八\t-9\n");
    assert_eq!(text(&composer(&phrase_updates, "18 ")), "八", "warm lookup before phrase insertion");
    phrase_updates.add_user_phrase("吧", "ㄅㄚ˙").unwrap();
    assert_eq!(text(&composer(&phrase_updates, "18 ")), "吧", "new neutral phrase invalidates tolerant lookup cache");
    phrase_updates.remove_user_phrase("吧", "ㄅㄚ˙");
    assert_eq!(
        text(&composer(&phrase_updates, "18 ")),
        "八",
        "removed neutral phrase invalidates tolerant lookup cache"
    );
    let new_syllables = engine("");
    new_syllables.add_user_phrase("謝謝", "ㄒㄧㄝˋ ㄒㄧㄝ˙").unwrap();
    assert_eq!(
        text(&composer(&new_syllables, "vu,4vu,4")),
        "謝謝",
        "new personal phrase supplies all syllables and tolerant index"
    );
    new_syllables.remove_user_phrase("謝謝", "ㄒㄧㄝˋ ㄒㄧㄝ˙");
    assert_eq!(text(&composer(&new_syllables, "vu,4")), "ㄒㄧㄝ", "removed personal-only syllable no longer accepted");

    let temp = TempDir::new("composer-persistence");
    let persisted = engine_with(TINY, Some(temp.path()), None);
    persisted.add_user_phrase("媽八", "ㄇㄚ ㄅㄚ").unwrap();
    let mut saving = composer(&persisted, "18 ");
    saving.handle(KeyInput::Down);
    let index = index_of(&saving, "巴");
    saving.select_candidate(index);
    saving.commit_all();
    persisted.flush_personal_data();
    let round_trip = engine_with(TINY, Some(temp.path()), None);
    assert!(
        round_trip.learned_entries().iter().any(|(k, _)| k.word == "巴") && round_trip.user_phrases().len() == 1,
        "debounced persistence round-trip"
    );
    assert_eq!(text(&composer(&round_trip, "18 ")), "巴", "persisted boost used");
    let removed_engine = engine(TINY);
    removed_engine.add_user_phrase("餌", "ㄦ").unwrap();
    let mut removed = composer(&removed_engine, "- ");
    removed.handle(KeyInput::Down);
    removed.select_candidate(0);
    removed_engine.remove_user_phrase("餌", "ㄦ");
    removed.handle(KeyInput::Left);
    assert_eq!(text(&removed), "餌", "fixed word survives phrase removal");
    assert_eq!(removed.commit_all(), "餌", "removed fixed phrase remains committable");
}

#[test]
fn caches_unicode_and_stress() {
    let cache_engine = engine("ㄅㄚ\t八\t-9\nㄅㄚ\t巴\t-10\n");
    let mut live = composer(&cache_engine, "18 ");
    cache_engine.add_user_phrase("巴", "ㄅㄚ").unwrap();
    live.handle(KeyInput::Left);
    assert_eq!(text(&live), "巴", "phrase insertion invalidates live conversion cache");
    cache_engine.remove_user_phrase("巴", "ㄅㄚ");
    live.handle(KeyInput::Right);
    assert_eq!(text(&live), "八", "phrase removal invalidates live conversion cache");
    let mut learner = composer(&cache_engine, "18 ");
    learner.handle(KeyInput::Down);
    let index = index_of(&learner, "巴");
    learner.select_candidate(index);
    learner.commit_all();
    live.handle(KeyInput::Left);
    assert_eq!(text(&live), "巴", "learning invalidates another composer's conversion cache");
    cache_engine.forget_all_learned();
    live.handle(KeyInput::Right);
    assert_eq!(text(&live), "八", "forgetting invalidates live conversion cache");

    let unicode_engine = engine("ㄅㄚ\t𠮷\t-1\nㄆㄚ\t趴\t-2\nㄅㄚ ㄆㄚ\t𠮷趴\t-1\n");
    let mut unicode = composer(&unicode_engine, "18 q8 ");
    unicode.handle(KeyInput::Left);
    assert_eq!(unicode.presentation().cursor, 2, "supplementary Han caret uses UTF-16");
    type_keys("a8", &mut unicode);
    assert_eq!(unicode.presentation().cursor, 4, "buffer after supplementary character");
    valid_presentation(&unicode, "supplementary Han + buffer");
    let tiny = engine(TINY);
    let zero_keys = ComposerOptions { selection_keys: vec![], ..ComposerOptions::default() };
    let mut zero = composer_with(&tiny, "18 ", zero_keys);
    zero.handle(KeyInput::Down);
    assert_eq!(zero.presentation().candidates.as_ref().unwrap().page_size(), 1, "empty selection keys safe");
    zero.handle(KeyInput::Space);
    zero.handle(KeyInput::Enter);
    assert!(zero.presentation().candidates.is_none(), "empty selection keys navigation still works");
    let mut stress = composer(&tiny, "");
    let mixed = [
        KeyInput::Char('1'),
        KeyInput::Char('8'),
        KeyInput::Space,
        KeyInput::Char('q'),
        KeyInput::Char('8'),
        KeyInput::Space,
        KeyInput::Down,
        KeyInput::Right,
        KeyInput::Enter,
        KeyInput::Left,
        KeyInput::Char('a'),
        KeyInput::Char('8'),
        KeyInput::Space,
        KeyInput::Home,
        KeyInput::ForwardDelete,
        KeyInput::ShiftedChar('-'),
        KeyInput::End,
        KeyInput::Backspace,
        KeyInput::OptionBackspace,
        KeyInput::Escape,
        KeyInput::ShiftSpace,
    ];
    for _ in 0..4 {
        for key in mixed {
            stress.handle(key);
            valid_presentation(&stress, "mixed editing");
        }
    }
}

#[test]
fn keyboard_channel_through_the_composer() {
    assert_eq!(
        ComposerOptions::default().typo_tolerance,
        TypoTolerance::Standard,
        "standard typo tolerance is the public default"
    );
    let invalid_typo = engine("ㄊㄜˋ\t特\t-2\n");
    let mut accepted = composer_with(&invalid_typo, "ek4", ComposerOptions::default());
    assert_eq!(text(&accepted), "特", "invalid exact syllable accepted via neighbour alternative");
    accepted.handle(KeyInput::Down);
    let annotation = accepted
        .presentation()
        .candidates
        .as_ref()
        .unwrap()
        .items()
        .iter()
        .find(|i| i.text == "特")
        .and_then(|i| i.annotation.clone());
    assert_eq!(annotation.as_deref(), Some("ㄊㄜˋ"), "corrected candidate annotates actual reading");
    let index = index_of(&accepted, "特");
    accepted.select_candidate(index);
    assert_eq!(accepted.commit_all(), "特", "picked correction fixed and committed");
    assert!(
        invalid_typo.learned_entries().iter().any(|(k, _)| k.reading == "ㄍㄜˋ" && k.word == "特"),
        "correction learns typed reading"
    );
    let mut recalled = composer_with(&invalid_typo, "ek4", ComposerOptions::default());
    assert_eq!(text(&recalled), "特", "repeat typed error recalls learned word directly");
    recalled.handle(KeyInput::Down);
    let annotation = recalled
        .presentation()
        .candidates
        .as_ref()
        .unwrap()
        .items()
        .iter()
        .find(|i| i.text == "特")
        .and_then(|i| i.annotation.clone());
    assert_eq!(annotation, None, "learned typed reading no longer needs channel correction");
    invalid_typo.forget_all_learned();
    assert_eq!(text(&composer(&invalid_typo, "ek4")), "ㄍㄜ", "off disables invalid acceptance");
    let tie = engine("ㄊㄜˇ\t甲\t-2\nㄊㄜˋ\t乙\t-2\n");
    assert_eq!(
        text(&composer_with(&tie, "wk3", ComposerOptions::default())),
        "甲",
        "exact beats corrected when LM scores tie"
    );
    let tone_cost = tie.typo_alternatives("wk3").into_iter().find(|a| a.0 == "ㄊㄜˋ").unwrap().1;
    let strength = engine(&format!("ㄊㄜˇ\t甲\t{}\nㄊㄜˋ\t乙\t0\n", -tone_cost * 0.85));
    let aggressive = ComposerOptions { typo_tolerance: TypoTolerance::Aggressive, ..ComposerOptions::default() };
    assert_eq!(
        text(&composer_with(&strength, "wk3", ComposerOptions::default())),
        "甲",
        "standard channel cost resists weaker evidence"
    );
    assert_eq!(
        text(&composer_with(&strength, "wk3", aggressive.clone())),
        "乙",
        "aggressive applies the 0.7 cost multiplier"
    );
    let provenance = engine("ㄊㄢˇ\t坦\t-2\n");
    assert_eq!(
        text(&composer_with(&provenance, "we04", ComposerOptions::default())),
        "坦",
        "token retains overwritten raw keys and its tone for two-edit recovery"
    );
    let temp = TempDir::new("typo-context");
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "八\t-2\n各\t-2\n特\t-2\n</s>\t0\n").unwrap();
    std::fs::write(dir.join("bigram.tsv"), "<s>\t八\t0\n<s>\t各\t0\n<s>\t特\t0\n八\t各\t-1000\n八\t特\t0\n").unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    let context = engine_with("ㄅㄚ\t八\t-2\nㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\n", None, Some(&dir));
    assert_eq!(
        text(&composer_with(&context, "ek4", ComposerOptions::default())),
        "各",
        "equal context retains exact reading"
    );
    assert_eq!(
        text(&composer_with(&context, "18 ek4", ComposerOptions::default())),
        "八特",
        "far likelier context overcomes neighbour cost"
    );
    assert_eq!(text(&composer(&context, "18 ek4")), "八各", "off disables corrected conversion");
    assert_eq!(text(&composer_with(&context, "18 ek4", aggressive)), "八特", "aggressive uses correction path");
    let mut live = composer_with(&context, "18 ek4", ComposerOptions::default());
    let mut options = live.options().clone();
    options.typo_tolerance = TypoTolerance::Off;
    live.set_options(options);
    live.handle(KeyInput::Left);
    assert_eq!(text(&live), "八各", "changing typo option invalidates live conversion");
}

#[test]
fn recall_budget_backtick_and_hints() {
    let temp = TempDir::new("typo-recall");
    let lexicon = "ㄊㄜˇ\t甲\t0\nㄊㄜˋ\t乙\t-100\n";
    let recall = engine_with(lexicon, Some(temp.path()), None);
    let mut pick = composer_with(&recall, "wk3", ComposerOptions::default());
    pick.handle(KeyInput::Down);
    assert!(!items(&pick).iter().any(|t| t == "乙"), "the list starts with words for the keys as typed");
    pick.toggle_corrections();
    assert_eq!(pick.presentation().candidates.as_ref().unwrap().tolerant(), Some(true), "toggling lists corrections");
    let index = index_of(&pick, "乙");
    pick.select_candidate(index);
    pick.commit_all();
    assert_eq!(
        text(&composer_with(&recall, "wk3", ComposerOptions::default())),
        "乙",
        "explicit typo recall outranks a much more frequent exact word"
    );
    recall.flush_personal_data();
    let reloaded = engine_with(lexicon, Some(temp.path()), None);
    assert_eq!(
        text(&composer_with(&reloaded, "wk3", ComposerOptions::default())),
        "乙",
        "corrected typed-reading recall persists"
    );
    recall.forget_all_learned();
    assert_eq!(
        text(&composer_with(&recall, "wk3", ComposerOptions::default())),
        "甲",
        "forget invalidates corrected-reading recall"
    );

    for (length, allowed) in [(3usize, false), (4, true), (6, true)] {
        let reading = vec!["ㄊㄜˋ"; length].join(" ");
        let word = "特".repeat(length);
        let e = engine(&format!("ㄍㄜˋ\t各\t-100\nㄊㄜˋ\t特\t-100\n{reading}\t{word}\t1000\n"));
        let mut c = composer_with(&e, &format!("ek4ek4{}", "wk4".repeat(length - 2)), ComposerOptions::default());
        c.handle(KeyInput::Down);
        let found = c
            .presentation()
            .candidates
            .as_ref()
            .is_some_and(|l| l.items().iter().any(|i| i.text == word && i.annotation.is_some()));
        assert_eq!(found, allowed, "{length}-token span correction budget");
    }

    let tiny = engine(TINY);
    let mut plain_empty = composer(&tiny, "");
    assert_eq!(plain_empty.handle(KeyInput::Char('`')), KeyResult::PASS_THROUGH, "empty backtick passes through");
    let mut plain_buffer = composer(&tiny, "su");
    assert!(
        plain_buffer.handle(KeyInput::Char('`')) == KeyResult::CONSUMED && text(&plain_buffer) == "ㄋㄧ",
        "backtick literalises unfinished buffer"
    );
    plain_buffer.handle(KeyInput::Backspace);
    assert!(!plain_buffer.is_composing(), "plain buffer is a single literal token");
    for (keys, plain) in [("18 ", "ㄅㄚ"), ("186", "ㄅㄚˊ"), ("183", "ㄅㄚˇ"), ("184", "ㄅㄚˋ"), ("187", "˙ㄅㄚ")]
    {
        let mut c = composer(&tiny, keys);
        c.handle(KeyInput::Char('`'));
        assert_eq!(text(&c), plain, "backtick standard plain formatting {keys}");
        valid_presentation(&c, "plain previous syllable");
    }
    let mut plain_middle = composer(&tiny, "18 q8 ");
    plain_middle.handle(KeyInput::Left);
    plain_middle.handle(KeyInput::Char('`'));
    assert!(
        text(&plain_middle) == "ㄅㄚ趴" && plain_middle.presentation().cursor == 2,
        "backtick replaces token before interior cursor"
    );
    for size in [0, 1, 2, 9, 30] {
        let options = ComposerOptions {
            selection_keys: "123456789abcdefghijklmnopqrstu".chars().take(size).collect(),
            ..legacy()
        };
        let mut c = composer_with(&tiny, "18 ", options);
        c.handle(KeyInput::Down);
        assert!(!items(&c).iter().any(|t| t == "ㄅㄚ"), "plain Zhuyin absent from list of size {size}");
        assert_eq!(c.presentation().zhuyin_hint.as_deref(), Some("ㄅㄚ"), "plain hint independent of page size {size}");
        c.handle(KeyInput::Char('`'));
        assert!(text(&c) == "ㄅㄚ" && c.presentation().zhuyin_hint.is_none(), "footer action fixes literal");
    }
    for (keys, hint) in [("18 ", "ㄅㄚ"), ("186", "ㄅㄚˊ"), ("183", "ㄅㄚˇ"), ("184", "ㄅㄚˋ"), ("187", "˙ㄅㄚ")]
    {
        let mut c = composer(&tiny, keys);
        c.handle(KeyInput::Down);
        assert_eq!(c.presentation().zhuyin_hint.as_deref(), Some(hint), "footer tone notation {hint}");
    }
    let phrase_only = engine("ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\t-2\n");
    let mut neutral_hint = composer(&phrase_only, "vu,4vu,7");
    neutral_hint.handle(KeyInput::Down);
    assert_eq!(
        neutral_hint.presentation().zhuyin_hint.as_deref(),
        Some("ㄒㄧㄝˋ ˙ㄒㄧㄝ"),
        "multi-syllable neutral hint"
    );
}

#[test]
fn backtick_undo() {
    let tiny = engine(TINY);
    // A ` pressed by mistake: ` again or Esc right after puts the syllables back; any other key keeps the literal.
    let mut undo_again = composer(&tiny, "18 q8 ");
    undo_again.handle(KeyInput::Char('`'));
    assert_eq!(text(&undo_again), "八ㄆㄚ", "backtick before undo");
    undo_again.handle(KeyInput::Char('`'));
    assert!(
        text(&undo_again) == "八趴" && undo_again.presentation().cursor == 2,
        "second backtick restores the syllable"
    );
    undo_again.handle(KeyInput::Char('`'));
    assert_eq!(text(&undo_again), "八ㄆㄚ", "a third backtick converts again");
    let mut undo_escape = composer(&tiny, "18 q8 ");
    undo_escape.handle(KeyInput::Char('`'));
    undo_escape.handle(KeyInput::Escape);
    assert!(
        text(&undo_escape) == "八趴" && undo_escape.is_composing(),
        "Esc right after a backtick restores instead of clearing"
    );
    let mut undo_buffer = composer(&tiny, "18 q8");
    undo_buffer.handle(KeyInput::Char('`'));
    undo_buffer.handle(KeyInput::Escape);
    type_keys(" ", &mut undo_buffer);
    assert_eq!(text(&undo_buffer), "八趴", "restored unfinished syllable completes as typed");
    let mut undo_list = composer(&tiny, "18 q8 ");
    undo_list.handle(KeyInput::Down);
    undo_list.handle(KeyInput::Char('`'));
    let list_literal = text(&undo_list).to_owned();
    undo_list.handle(KeyInput::Char('`'));
    assert!(list_literal != "八趴" && text(&undo_list) == "八趴", "backtick from the candidate list is undone too");
    let mut undo_gone = composer(&tiny, "18 q8 ");
    undo_gone.handle(KeyInput::Char('`'));
    type_keys("a8 ", &mut undo_gone);
    undo_gone.handle(KeyInput::Escape);
    assert!(!undo_gone.is_composing(), "after another key, Esc clears as usual");
}

#[test]
fn page_at_list_edge_off() {
    let tiny = engine(TINY);
    let list = |c: &Composer| c.presentation().candidates.clone().unwrap();
    let options = ComposerOptions { selection_keys: vec!['1', '2'], page_at_list_edge: false, ..legacy() };
    let mut stay = composer_with(&tiny, "18 ", options.clone());
    stay.handle(KeyInput::Down);
    stay.handle(KeyInput::Down);
    assert_eq!(list(&stay).selection(), 1, "↓ moves within the page");
    stay.handle(KeyInput::Down);
    assert!(list(&stay).selection() == 1 && list(&stay).page() == 0, "↓ at the page's last candidate stays");
    stay.handle(KeyInput::Tab);
    assert_eq!(list(&stay).selection(), 2, "the page keys still turn pages");
    stay.handle(KeyInput::Up);
    assert_eq!(list(&stay).selection(), 2, "↑ at the page's first candidate stays");
    let horizontal = ComposerOptions { vertical_list: false, ..options };
    let mut sideways = composer_with(&tiny, "18 ", horizontal);
    sideways.handle(KeyInput::Down);
    sideways.handle(KeyInput::Right);
    sideways.handle(KeyInput::Right);
    assert_eq!(list(&sideways).selection(), 1, "horizontal → at the page's edge stays");
    sideways.handle(KeyInput::Down);
    assert_eq!(list(&sideways).page(), 1, "horizontal ↓ still turns the page");
    sideways.handle(KeyInput::Left);
    assert_eq!(list(&sideways).selection(), 2, "horizontal ← at the page's first candidate stays");
    let mut across = composer_with(&tiny, "18 ", ComposerOptions { selection_keys: vec!['1', '2'], ..legacy() });
    across.handle(KeyInput::Down);
    across.handle(KeyInput::Down);
    across.handle(KeyInput::Down);
    assert!(list(&across).selection() == 2 && list(&across).page() == 1, "on (the default): ↓ crosses the page");
}

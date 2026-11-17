//! Automatic corrections made visible and undone: the corrected spans in the presentation, converting them as typed
//! (`Composer::revert_correction`, `Request::literal`) and ⌫ right after a correction
//! (`ComposerOptions::undo_correction_with_backspace`).

use std::sync::Arc;

use zhuyin_core::api::{ComposerOptions, KeyInput, KeyResult, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::conversion::{CompositionToken, Request};
use zhuyin_core::engine::{Engine, EngineConfig};

/// ㄊㄜˇ typed (`wk3`) reads 乙 through the keyboard channel's tone correction: the exact 甲 is far rarer.
const LEXICON: &str = "ㄊㄜˇ\t甲\t-9\nㄊㄜˋ\t乙\t0\nㄅㄚ\t八\t-2\n";

fn engine() -> Arc<Engine> {
    Arc::new(Engine::from_text(LEXICON, None, None, EngineConfig::default()))
}

fn options(undo: bool) -> ComposerOptions {
    ComposerOptions { undo_correction_with_backspace: undo, ..ComposerOptions::default() }
}

fn typed(engine: &Arc<Engine>, keys: &str, undo: bool) -> Composer {
    let mut composer = Composer::new(engine.clone(), options(undo));
    for c in keys.chars() {
        composer.handle(if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) });
    }
    composer
}

fn text(composer: &Composer) -> &str {
    &composer.presentation().text
}

#[test]
fn corrected_spans_are_listed() {
    let e = engine();
    let composer = typed(&e, "18 wk3", false);
    let p = composer.presentation();
    assert_eq!(p.text, "八乙", "the correction is what the conversion shows");
    assert_eq!(p.corrections, vec![1usize..2], "only the corrected character is marked");
    assert!(!p.backspace_reverts, "⌫ deletes with the option off");
    assert_eq!(
        typed(&e, "18 ", false).presentation().corrections,
        Vec::<std::ops::Range<usize>>::new(),
        "exact words are not marked"
    );
}

#[test]
fn the_option_leaves_conversion_unchanged() {
    let e = engine();
    for keys in ["wk3", "18 wk3", "wk318 ", "18 wk318 wk3"] {
        assert_eq!(text(&typed(&e, keys, true)), text(&typed(&e, keys, false)), "{keys}");
    }
}

#[test]
fn literal_request_converts_the_keys_as_typed() {
    let e = engine();
    let tokens = [CompositionToken::syllable("ㄅㄚ", "18 ", false), CompositionToken::syllable("ㄊㄜˇ", "wk3", false)];
    let request = Request::new(&tokens, &[], TypoTolerance::Standard);
    let text =
        |request: &Request<'_>| e.convert(request, &mut None).iter().map(|w| w.text.to_string()).collect::<String>();
    assert_eq!(text(&request), "八乙");
    let mut literal = Request::new(&tokens, &[], TypoTolerance::Standard);
    literal.literal = Some(1..2);
    assert_eq!(text(&literal), "八甲", "no correction over the literal tokens");
    literal.literal = Some(0..1);
    assert_eq!(text(&literal), "八乙", "corrections outside the literal tokens stay");
}

#[test]
fn revert_correction_converts_as_typed_without_learning() {
    let e = engine();
    let mut composer = typed(&e, "18 wk3", false);
    assert!(composer.revert_correction(), "the correction the last key made");
    let p = composer.presentation();
    assert_eq!(p.text, "八甲");
    assert!(p.corrections.is_empty(), "nothing left to mark");
    assert!(p.reverted, "counted by the client");
    assert!(!composer.revert_correction(), "nothing left to revert");
    composer.handle(KeyInput::Left);
    assert_eq!(text(&composer), "八甲", "the reverted word stays through navigation");
    assert_eq!(composer.commit_all(), "八甲");
    assert!(!e.learned_entries().iter().any(|(k, _)| k.word == "乙"), "a revert is not a pick of the correction");
}

#[test]
fn revert_at_the_cursor() {
    let e = engine();
    let mut composer = typed(&e, "wk318 ", false);
    assert_eq!(text(&composer), "乙八");
    composer.handle(KeyInput::Home);
    assert!(composer.revert_correction(), "the character ↓ would change");
    assert_eq!(text(&composer), "甲八");
}

#[test]
fn first_backspace_after_a_correction_reverts_it() {
    let e = engine();
    let mut composer = typed(&e, "18 wk3", true);
    assert!(composer.presentation().backspace_reverts, "the key made a correction appear");
    assert_eq!(composer.handle(KeyInput::Backspace), KeyResult::CONSUMED);
    assert_eq!(text(&composer), "八甲", "the first ⌫ puts back the keys as typed");
    assert!(composer.presentation().reverted);
    assert!(!composer.presentation().backspace_reverts);
    composer.handle(KeyInput::Backspace);
    assert_eq!(text(&composer), "八", "the second ⌫ deletes");
    assert!(!composer.presentation().reverted);
}

#[test]
fn backspace_deletes_when_off_or_later() {
    let e = engine();
    let mut off = typed(&e, "18 wk3", false);
    off.handle(KeyInput::Backspace);
    assert_eq!(text(&off), "八", "the option off: ⌫ deletes");
    let mut later = typed(&e, "wk318 ", true);
    assert!(!later.presentation().backspace_reverts, "the last key made no correction");
    later.handle(KeyInput::Backspace);
    assert_eq!(text(&later), "乙", "⌫ deletes the last character");
    let mut typing = typed(&e, "wk31", true);
    assert!(!typing.presentation().backspace_reverts, "a syllable is being typed");
    typing.handle(KeyInput::Backspace);
    assert_eq!(text(&typing), "乙", "⌫ deletes the unfinished syllable's key");
}

#[test]
fn candidate_window_shows_the_typed_reading_and_backspace_reverts() {
    let e = engine();
    let mut composer = typed(&e, "wk3", true);
    composer.handle(KeyInput::Down);
    assert_eq!(composer.presentation().typed_reading.as_deref(), Some("ㄊㄜˇ"), "你打的是 ㄊㄜˇ");
    composer.handle(KeyInput::Backspace);
    let p = composer.presentation();
    assert!(p.candidates.is_none(), "the window closes");
    assert_eq!(p.text, "甲");
    assert!(p.reverted);
    let mut exact = typed(&e, "18 ", true);
    exact.handle(KeyInput::Down);
    assert_eq!(exact.presentation().typed_reading, None, "no correction in the span");
    exact.handle(KeyInput::Backspace);
    assert!(exact.presentation().candidates.is_none() && text(&exact) == "八", "⌫ only closes the window");
}

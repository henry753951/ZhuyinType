//! Word actions from the candidate window (不要再推薦, 忘記這個選字, 加入自訂詞) and their undo.

mod common;

use std::sync::Arc;

use common::TempDir;
use zhuyin_core::api::{ComposerOptions, KeyInput, PunctuationKeys, TypoTolerance};
use zhuyin_core::composer::{CandidateAction, Composer};
use zhuyin_core::engine::{CommittedWord, Engine, EngineConfig};

const LEXICON: &str = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄅㄚ\t吧\t-2.4\nㄇㄚ\t媽\t-2\nㄅㄚ ㄇㄚ\t八媽\t-1\n";

fn engine() -> Arc<Engine> {
    Arc::new(Engine::from_text(LEXICON, None, None, EngineConfig::default()))
}

fn composer(engine: &Arc<Engine>, keys: &str) -> Composer {
    let options = ComposerOptions {
        typo_tolerance: TypoTolerance::Off,
        punctuation_keys: PunctuationKeys::Mac,
        ..Default::default()
    };
    let mut composer = Composer::new(engine.clone(), options);
    for c in keys.chars() {
        composer.handle(if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) });
    }
    composer
}

fn text(c: &Composer) -> String {
    c.presentation().text.clone()
}

fn items(c: &Composer) -> Vec<String> {
    c.presentation().candidates.as_ref().map(|l| l.items().iter().map(|i| i.text.clone()).collect()).unwrap_or_default()
}

fn learn_pick(engine: &Engine, word: &str) {
    let picked =
        CommittedWord { range: 0..1, text: word.into(), reading: "ㄅㄚ".into(), word_id: None, picked: true };
    engine.learn(&[picked], 1, &[]);
}

#[test]
fn block_moves_the_word_out_of_conversion_and_to_the_end_of_the_list() {
    let engine = engine();
    let mut c = composer(&engine, "18 ");
    assert_eq!(text(&c), "八");
    c.handle(KeyInput::Down);
    assert_eq!(items(&c)[0], "八");
    assert!(c.candidate_action_available(CandidateAction::Block, None));
    assert_eq!(c.perform_candidate_action(CandidateAction::Block, None).as_deref(), Some("不再推薦「八」"));
    assert_eq!(text(&c), "巴", "the next word is converted");
    assert_eq!(items(&c).last().map(String::as_str), Some("八"), "still listed, last: {:?}", items(&c));
    assert!(!c.candidate_action_available(CandidateAction::Block, Some(items(&c).len() - 1)), "already blocked");
    assert_eq!(c.presentation().notice.as_deref(), Some("不再推薦「八」"));
    assert_eq!(text(&composer(&engine, "18 ")), "巴", "a new composition too");
    assert!(engine.is_blocked("八", "ㄅㄚ"));

    assert!(c.undo_candidate_action());
    assert_eq!(text(&c), "八", "undo converts as before");
    assert_eq!(items(&c)[0], "八");
    assert!(!engine.is_blocked("八", "ㄅㄚ") && engine.blocked_entries().is_empty());
    assert_eq!(c.presentation().notice.as_deref(), Some("已復原"));
    assert!(!c.undo_candidate_action(), "one level");
}

#[test]
fn block_forgets_a_learned_pick_and_undo_restores_it() {
    let engine = engine();
    learn_pick(&engine, "巴");
    assert_eq!(text(&composer(&engine, "18 ")), "巴");
    let before = engine.export_personal_data();
    let mut c = composer(&engine, "18 ");
    c.handle(KeyInput::Down);
    c.perform_candidate_action(CandidateAction::Block, None).unwrap();
    assert!(engine.learned_entries().iter().all(|(k, _)| k.word != "巴"), "forgotten");
    assert_eq!(text(&c), "八");
    c.undo_candidate_action();
    assert_eq!(engine.export_personal_data(), before, "personal data exactly as before");
    assert_eq!(text(&c), "巴");
}

#[test]
fn forget_removes_a_learned_choice_and_undo_puts_it_back() {
    let engine = engine();
    let mut fresh = composer(&engine, "18 ");
    fresh.handle(KeyInput::Down);
    assert!(!fresh.candidate_action_available(CandidateAction::Forget, None), "nothing learned");
    assert_eq!(fresh.perform_candidate_action(CandidateAction::Forget, None), None);

    learn_pick(&engine, "巴");
    let before = engine.export_personal_data();
    let mut c = composer(&engine, "18 ");
    assert_eq!(text(&c), "巴");
    c.handle(KeyInput::Down);
    assert_eq!(items(&c)[0], "巴");
    assert_eq!(c.perform_candidate_action(CandidateAction::Forget, Some(0)).as_deref(), Some("已忘記「巴」"));
    assert_eq!(text(&c), "八");
    assert!(!engine.has_learned_choice("巴", "ㄅㄚ"));
    assert!(!engine.is_blocked("巴", "ㄅㄚ"), "forgetting does not block");
    c.undo_candidate_action();
    assert_eq!(engine.export_personal_data(), before);
    assert_eq!(text(&c), "巴");
}

#[test]
fn add_phrase_from_the_list_only() {
    let engine = engine();
    // With no list open there is nothing highlighted: adding characters of the composition is the selection's
    // (⇧← ⇧→, then Return), so the action does not apply and nothing changes.
    let mut c = composer(&engine, "18 a8 ");
    assert_eq!(text(&c), "八媽");
    assert!(!c.candidate_action_available(CandidateAction::AddPhrase, None), "not without a list");
    assert_eq!(c.perform_candidate_action(CandidateAction::AddPhrase, None), None);
    assert!(engine.user_phrases().is_empty());
    assert!(!c.can_undo_candidate_action());

    c.handle(KeyInput::Down);
    let highlighted = items(&c)[0].clone();
    assert_eq!(
        c.perform_candidate_action(CandidateAction::AddPhrase, None),
        Some(format!("已加入自訂詞：{highlighted}"))
    );
    assert!(c.undo_candidate_action());
    assert!(engine.user_phrases().is_empty());

    let mut listed = composer(&engine, "18 ");
    listed.handle(KeyInput::Down);
    let index = items(&listed).iter().position(|w| w == "吧").unwrap();
    assert_eq!(
        listed.perform_candidate_action(CandidateAction::AddPhrase, Some(index)).as_deref(),
        Some("已加入自訂詞：吧")
    );
    assert!(engine.is_user_phrase("吧", "ㄅㄚ"));
    assert!(listed.presentation().candidates.is_some(), "the list stays open");
    assert!(!listed.candidate_action_available(CandidateAction::Block, Some(99)), "out of range");
}

#[test]
fn adding_a_blocked_word_suggests_it_again_and_blocking_a_custom_word_removes_it() {
    let engine = engine();
    engine.add_user_phrase("吧", "ㄅㄚ").unwrap();
    let undo = engine.block_suggestion("吧", "ㄅㄚ");
    assert!(engine.user_phrases().is_empty() && engine.is_blocked("吧", "ㄅㄚ"));
    engine.undo_word_action(undo);
    assert!(engine.is_user_phrase("吧", "ㄅㄚ") && !engine.is_blocked("吧", "ㄅㄚ"));

    engine.remove_user_phrase("吧", "ㄅㄚ");
    let _ = engine.block_suggestion("吧", "ㄅㄚ");
    let undo = engine.add_phrase_undoable("吧", "ㄅㄚ").unwrap();
    assert!(!engine.is_blocked("吧", "ㄅㄚ"));
    engine.undo_word_action(undo);
    assert!(engine.is_blocked("吧", "ㄅㄚ") && engine.user_phrases().is_empty());
    assert!(engine.add_phrase_undoable("吧吧吧吧吧吧吧", "ㄅㄚ").is_err());
}

#[test]
fn literal_candidates_have_no_word_actions() {
    let engine = engine();
    let mut c = composer(&engine, "");
    c.handle(KeyInput::ShiftedChar(','));
    c.handle(KeyInput::Down);
    assert!(c.presentation().candidates.is_some(), "punctuation alternatives");
    for action in CandidateAction::ALL {
        assert!(!c.candidate_action_available(action, None), "{action:?}");
        assert_eq!(c.perform_candidate_action(action, None), None);
    }
    let empty = composer(&engine, "");
    for action in CandidateAction::ALL {
        assert!(!empty.candidate_action_available(action, None));
        assert_eq!(CandidateAction::from_raw(action.raw_value()), Some(action));
    }
    assert_eq!(CandidateAction::from_raw(9), None);
}

#[test]
fn blocked_words_persist_and_travel_in_the_export() {
    let temp = TempDir::new("blocked");
    let first = Engine::from_text(LEXICON, Some(temp.path()), None, EngineConfig::default());
    first.flush_personal_data().unwrap();
    assert!(!temp.path().join("blocked.tsv").exists(), "no file while nothing is blocked");
    let plain_export = first.export_personal_data();
    assert!(!plain_export.contains("[blocked]"), "the export is unchanged while nothing is blocked");
    let _ = first.block_suggestion("八", "ㄅㄚ");
    first.flush_personal_data().unwrap();
    assert_eq!(std::fs::read_to_string(temp.path().join("blocked.tsv")).unwrap(), "ㄅㄚ\t八\n");
    let second = Engine::from_text(LEXICON, Some(temp.path()), None, EngineConfig::default());
    assert!(second.is_blocked("八", "ㄅㄚ"));
    let export = second.export_personal_data();
    assert!(export.ends_with("[blocked]\nㄅㄚ\t八\n"), "{export}");

    let receiving = Engine::from_text(LEXICON, None, None, EngineConfig::default());
    receiving.import_personal_data(export.as_bytes()).unwrap();
    assert!(receiving.is_blocked("八", "ㄅㄚ"));
    receiving.import_personal_data("# ZhuyinType personal data 1\n[blocked]\nnot-a-reading\t八\n".as_bytes()).unwrap();
    assert_eq!(receiving.blocked_entries().len(), 1, "invalid rows ignored");

    second.unblock("八", "ㄅㄚ");
    second.flush_personal_data().unwrap();
    assert!(!temp.path().join("blocked.tsv").exists(), "removed when the last word is unblocked");
    std::fs::write(temp.path().join("blocked.tsv"), "bad\nㄅㄚ\n\tㄅㄚ\nfoo\t八\n").unwrap();
    let damaged = Engine::from_text(LEXICON, Some(temp.path()), None, EngineConfig::default());
    assert!(damaged.blocked_entries().is_empty(), "damaged rows ignored");
}

//! 聯想詞: the follower lists of the language models, the engine's next-word suggestions and the composer's list
//! after a commit (`composer/association.rs`).

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{LmTrigram, TempDir, bigram, bigram_backoff, lm_fixture};
use zhuyin_core::api::{ComposerOptions, KeyInput, KeyResult, PunctuationKeys, SpaceAction, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::language_model::{BigramLanguageModel, VocabularyLanguageModel};
use zhuyin_core::{LanguageModel, LanguageModelHistory, NGramLanguageModel};

/// 謝謝 is followed by 你 (likely), 大家 (likely), 妳 (rare single character) and 。 (punctuation).
const LEXICON: &str = "ㄒㄧㄝˋ ㄒㄧㄝˋ\t謝謝\t-3\nㄋㄧˇ\t你\t-2\nㄋㄧˇ\t妳\t-6\nㄉㄚˋ ㄐㄧㄚ\t大家\t-3\n\
ㄏㄠˇ\t好\t-2\nㄌㄠˇ ㄅㄢˇ\t老闆\t-4\n";

fn model_directory(temp: &TempDir) -> std::path::PathBuf {
    let dir = temp.dir("lm");
    std::fs::write(dir.join("unigram.tsv"), "謝謝\t-3\n你\t-2\n妳\t-6\n大家\t-3\n好\t-2\n老闆\t-4\n。\t-1\n</s>\t-1\n")
        .unwrap();
    std::fs::write(
        dir.join("bigram.tsv"),
        "<s>\t謝謝\t-1\n謝謝\t你\t-0.5\n謝謝\t大家\t-1\n謝謝\t妳\t-5\n謝謝\t。\t-0.3\n謝謝\t</s>\t-0.2\n你\t好\t-0.2\n\
大家\t好\t-0.3\n",
    )
    .unwrap();
    std::fs::write(dir.join("backoff.tsv"), "").unwrap();
    dir
}

fn engine(model: &Path) -> Arc<Engine> {
    let engine = Engine::from_text(LEXICON, None, Some(model), EngineConfig::default());
    engine.set_clock(|| 1_000_000.0);
    Arc::new(engine)
}

fn options() -> ComposerOptions {
    ComposerOptions {
        typo_tolerance: TypoTolerance::Off,
        punctuation_keys: PunctuationKeys::Mac,
        space_action: SpaceAction::Commit,
        ..ComposerOptions::default()
    }
}

fn type_keys(composer: &mut Composer, keys: &str) -> String {
    let mut committed = String::new();
    for c in keys.chars() {
        let key = if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) };
        committed += composer.handle(key).commit.as_deref().unwrap_or("");
    }
    committed
}

fn items(composer: &Composer) -> Vec<String> {
    composer
        .presentation()
        .candidates
        .as_ref()
        .map(|l| l.items().iter().map(|i| i.text.clone()).collect())
        .unwrap_or_default()
}

fn associated(composer: &Composer) -> bool {
    composer.presentation().candidates.as_ref().is_some_and(zhuyin_core::api::CandidateList::associated)
}

/// 謝謝 typed and committed with Enter.
fn after_thanks(engine: &Arc<Engine>, on: bool) -> (Composer, String) {
    let mut composer = Composer::new(engine.clone(), options());
    composer.set_associated_phrases(on);
    let mut committed = type_keys(&mut composer, "vu,4vu,4");
    committed += composer.handle(KeyInput::Enter).commit.as_deref().unwrap_or("");
    (composer, committed)
}

#[test]
fn tsv_followers_best_first() {
    let temp = TempDir::new("followers-tsv");
    let model = BigramLanguageModel::open(model_directory(&temp)).unwrap();
    let thanks = model.word_id("謝謝").unwrap();
    let followers: Vec<String> = model
        .followers(LanguageModelHistory::new(None, thanks), 3)
        .into_iter()
        .map(|(id, _)| model.word(id).into_owned())
        .collect();
    assert_eq!(followers, ["</s>", "。", "你"]);
    assert!(model.followers(LanguageModelHistory::new(None, model.word_id("老闆").unwrap()), 5).is_empty());
    assert!(model.followers(LanguageModelHistory::new(None, thanks), 0).is_empty());
}

#[test]
fn binary_followers_merge_trigrams() {
    let unigrams = [("</s>", -1.0), ("<unk>", -8.0), ("八", -2.0), ("趴", -2.0), ("媽", -3.0), ("麻", -3.0)];
    let bigrams = [bigram("<s>", "八", -0.1), bigram_backoff("八", "趴", -0.2, -0.7), bigram("趴", "媽", -0.1)];
    let trigrams = [LmTrigram { first: "八", second: "趴", third: "麻", logp: -0.05 }];
    let temp = TempDir::new("followers-bin");
    let dir = temp.dir("lm");
    std::fs::write(dir.join("lm.bin"), lm_fixture(3, &unigrams, &[("八", -0.4)], &bigrams, &trigrams)).unwrap();
    let model = NGramLanguageModel::open(&dir).unwrap();
    let id = |w: &str| model.word_id(w).unwrap();
    let names = |history| -> Vec<String> {
        model.followers(history, 5).into_iter().map(|(id, _)| model.word(id).into_owned()).collect()
    };
    assert_eq!(names(LanguageModelHistory::new(None, id("趴"))), ["媽"], "bigram group only");
    assert_eq!(names(LanguageModelHistory::new(Some(id("八")), id("趴"))), ["麻", "媽"], "trigram first");
    assert!(names(LanguageModelHistory::new(None, 9999)).is_empty(), "outside the vocabulary");
}

#[test]
fn engine_filters_punctuation_and_rare_characters() {
    let temp = TempDir::new("predict-engine");
    let engine = engine(&model_directory(&temp));
    let words: Vec<String> = engine.predict_next("謝謝", 9, true).into_iter().map(|p| p.text).collect();
    assert_eq!(words, ["你", "大家"], "no 。, no </s>, no rare 妳");
    assert!(engine.predict_next("", 9, true).is_empty(), "no context");
    assert!(engine.predict_next("abc", 9, true).is_empty(), "no Han context");
    assert!(engine.predict_next("謝謝", 0, true).is_empty());
    let first = &engine.predict_next("謝謝", 1, true)[0];
    assert_eq!((first.text.as_str(), first.reading.as_str(), first.personal), ("你", "ㄋㄧˇ", false));
}

#[test]
fn off_leaves_commits_unchanged() {
    let temp = TempDir::new("predict-off");
    let engine = engine(&model_directory(&temp));
    let (off, committed_off) = after_thanks(&engine, false);
    let (on, committed_on) = after_thanks(&engine, true);
    assert_eq!(committed_off, "謝謝");
    assert_eq!(committed_on, committed_off);
    assert!(off.presentation().is_empty() && !off.is_composing());
    assert_eq!(items(&on), ["你", "大家"]);
    assert!(associated(&on) && on.is_composing() && on.presentation().text.is_empty());
}

fn passive(composer: &Composer) -> bool {
    composer.presentation().candidates.as_ref().is_some_and(zhuyin_core::api::CandidateList::passive)
}

/// Enters the list with ↓ and picks word `index` with its selection key.
fn pick(composer: &mut Composer, index: usize) -> Option<String> {
    assert!(composer.handle(KeyInput::Down).handled);
    composer.handle(KeyInput::Char(char::from(b'1' + index as u8))).commit
}

#[test]
fn down_enters_and_keys_pick() {
    let temp = TempDir::new("predict-pick");
    let engine = engine(&model_directory(&temp));
    let (mut composer, _) = after_thanks(&engine, true);
    assert!(passive(&composer), "the list opens without a highlight");
    // ↓ enters (the first word highlighted), ↓ again moves, ↑ on the first word goes back to passive.
    assert_eq!(composer.handle(KeyInput::Down), KeyResult::CONSUMED);
    let list = composer.presentation().candidates.clone().unwrap();
    assert!(!list.passive() && list.selection() == 0);
    composer.handle(KeyInput::Down);
    assert_eq!(composer.presentation().candidates.as_ref().unwrap().selection(), 1);
    composer.handle(KeyInput::Down);
    assert_eq!(composer.presentation().candidates.as_ref().unwrap().selection(), 1, "stays on the last word");
    composer.handle(KeyInput::Up);
    composer.handle(KeyInput::Up);
    assert!(passive(&composer));
    // Return sends the highlighted word and lists, passively, what follows it.
    composer.handle(KeyInput::Down);
    composer.handle(KeyInput::Down);
    assert_eq!(composer.presentation().candidates.as_ref().map(|l| (l.selection(), l.passive())), Some((1, false)));
    let result = composer.handle(KeyInput::Enter);
    assert_eq!((result.handled, result.commit.as_deref()), (true, Some("大家")));
    assert_eq!(items(&composer), ["好"]);
    assert!(passive(&composer));
    // In the list, a selection key and ⇧ with it pick too.  (Picking 大家 above made it the first suggestion.)
    let (mut composer, _) = after_thanks(&engine, true);
    assert_eq!(items(&composer), ["大家", "你"]);
    assert_eq!(pick(&mut composer, 0).as_deref(), Some("大家"));
    composer.handle(KeyInput::Down);
    assert_eq!(composer.handle(KeyInput::ShiftedChar('1')).commit.as_deref(), Some("好"));
    assert!(composer.presentation().is_empty(), "nothing follows 好 in the model");
    // Esc in the list closes it and is used up.
    let (mut composer, _) = after_thanks(&engine, true);
    composer.handle(KeyInput::Down);
    assert_eq!(composer.handle(KeyInput::Escape), KeyResult::CONSUMED);
    assert!(composer.presentation().is_empty());
}

#[test]
fn passive_list_leaves_every_other_key_alone() {
    let temp = TempDir::new("predict-keys");
    let engine = engine(&model_directory(&temp));
    // ⇧1 after 謝謝 types ！ (⇧ with a symbol key types Chinese punctuation here), not a suggestion.
    let (mut composer, _) = after_thanks(&engine, true);
    let result = composer.handle(KeyInput::ShiftedChar('1'));
    assert!(result.handled && result.commit.is_none());
    assert_eq!(composer.presentation().text, "！");
    assert!(composer.presentation().candidates.is_none(), "the list closed");
    assert_eq!(composer.handle(KeyInput::Enter).commit.as_deref(), Some("！"));
    assert!(composer.presentation().is_empty(), "nothing is suggested after punctuation");
    // With ⇧ typing the key's own symbol, ⇧1 does exactly what it does with the feature off.
    let windows = ComposerOptions { punctuation_keys: PunctuationKeys::Windows, ..options() };
    let mut results = Vec::new();
    for on in [false, true] {
        let mut composer = Composer::new(engine.clone(), windows.clone());
        composer.set_associated_phrases(on);
        type_keys(&mut composer, "vu,4vu,4");
        composer.handle(KeyInput::Enter);
        assert_eq!(composer.presentation().candidates.is_some(), on);
        results.push((composer.handle(KeyInput::ShiftedChar('1')), composer.presentation().clone()));
    }
    assert_eq!(results[0], results[1]);
    // A Zhuyin key (1 is ㄅ) starts a composition.
    let (mut composer, _) = after_thanks(&engine, true);
    let result = composer.handle(KeyInput::Char('1'));
    assert!(result.handled && result.commit.is_none());
    assert_eq!(composer.presentation().text, "ㄅ");
    assert!(composer.presentation().candidates.is_none());
    // Space, Return, Tab, Esc, ← and ⌫ go to the app, as with no list.
    for key in [KeyInput::Space, KeyInput::Enter, KeyInput::Tab, KeyInput::Escape, KeyInput::Left, KeyInput::Backspace]
    {
        let (mut composer, _) = after_thanks(&engine, true);
        assert!(!composer.handle(key).handled, "{key:?}");
        assert!(composer.presentation().is_empty(), "{key:?}");
    }
}

#[test]
fn click_and_commit_all() {
    let temp = TempDir::new("predict-click");
    let engine = engine(&model_directory(&temp));
    let (mut composer, _) = after_thanks(&engine, true);
    assert_eq!(composer.select_candidate(0).commit.as_deref(), Some("你"), "a click picks without entering");
    assert_eq!(items(&composer), ["好"]);
    assert_eq!(composer.commit_all(), "", "the app's commit with only suggestions shown sends nothing");
    assert!(composer.presentation().is_empty());
    let (mut composer, _) = after_thanks(&engine, true);
    composer.set_associated_phrases(false);
    assert!(composer.presentation().is_empty(), "turning the setting off closes the list");
    // Another setting redraws: the list is gone, and ↓ no longer enters it.
    let (mut composer, _) = after_thanks(&engine, true);
    composer.set_options(ComposerOptions { vertical_list: false, ..options() });
    assert!(composer.presentation().is_empty());
    assert!(!composer.handle(KeyInput::Down).handled);
}

#[test]
fn private_fields_and_learning() {
    let temp = TempDir::new("predict-private");
    let engine = engine(&model_directory(&temp));
    let mut composer = Composer::new(engine.clone(), options());
    composer.set_associated_phrases(true);
    composer.set_learning_suspended(true);
    type_keys(&mut composer, "vu,4vu,4");
    composer.handle(KeyInput::Enter);
    assert!(composer.presentation().is_empty(), "no suggestions while learning is suspended");
    // Learning on: picking 大家 makes it the user's first suggestion after 謝謝.
    let (mut composer, _) = after_thanks(&engine, true);
    pick(&mut composer, 1);
    for _ in 0..2 {
        let (mut again, _) = after_thanks(&engine, true);
        let index = items(&again).iter().position(|w| w == "大家").unwrap();
        pick(&mut again, index);
    }
    let personal = engine.predict_next("謝謝", 9, true);
    assert_eq!((personal[0].text.as_str(), personal[0].personal), ("大家", true));
    // 「記住選過的字詞」 off: only the shipped model suggests, in its own order.
    let base: Vec<String> = engine.predict_next("謝謝", 9, false).into_iter().map(|p| p.text).collect();
    assert_eq!(base, ["你", "大家"]);
    let mut composer = Composer::new(engine.clone(), ComposerOptions { learning_enabled: false, ..options() });
    composer.set_associated_phrases(true);
    type_keys(&mut composer, "vu,4vu,4");
    composer.handle(KeyInput::Enter);
    assert_eq!(items(&composer), ["你", "大家"]);
}

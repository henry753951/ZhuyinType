//! Port of the Qt `EngineSessionTest`, on zhuyin-core directly with the same small lexicons.

mod common;

use common::{LEXICON, TempDir, data, type_session, write_lexicon};
use serde_json::{Map, Value, json};
use zt_core::engine::{EngineData, EngineSession, Phrase};
use zt_core::keys::{Key, code, modifier};

fn values(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn contains(list: &[Phrase], word: &str) -> bool {
    list.iter().any(|p| p.word == word)
}

fn key(session: &mut EngineSession, code: i32, modifiers: u32) -> bool {
    session.key(&Key::new(code, modifiers)).unwrap().handled
}

/// Picks candidate 1 of the ㄋㄧˇ list (妳) and sends it.
fn learn_second(session: &mut EngineSession) {
    type_session(session, "su3");
    assert!(key(session, code::DOWN, 0));
    assert!(session.choose(1).unwrap().handled);
    assert!(key(session, code::RETURN, 0));
}

#[test]
fn unopened_and_failed_reopen() {
    let mut session = EngineSession::new();
    assert!(!session.ready());
    assert!(session.key(&Key::new('S' as i32, 0)).is_err());
    assert!(session.add_phrase("你好", "ㄋㄧˇ ㄏㄠˇ").is_err());
    let dir = TempDir::new();
    assert!(EngineData::from_lexicon(&dir.join("lexicon.tsv")).is_err(), "no lexicon in the directory");
    session.open(&data(dir.path(), LEXICON), Some(&dir.join("personal"))).unwrap();
    type_session(&mut session, "su3");
    let before = session.presentation().composition.clone();
    let missing = EngineData::from_lexicon(&dir.join("missing.tsv")).unwrap();
    assert!(session.open(&missing, Some(&dir.join("other"))).is_err());
    assert!(session.ready());
    assert_eq!(session.presentation().composition, before);
    session.reset().unwrap();
    assert!(session.presentation().composition.is_empty());
}

#[test]
fn real_composition_candidates_and_repeated_commits() {
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    session
        .apply_settings(&values(json!({"learningEnabled": false, "spaceAction": "candidates", "candidatesPerPage": 3})))
        .unwrap();
    session.open(&data(dir.path(), LEXICON), Some(&dir.join("personal"))).unwrap();
    let mut commits = Vec::new();
    let mut timings = 0;
    for _ in 0..2 {
        for c in "su3cl3".chars() {
            let outcome = session.key(&common::key_for(c)).unwrap();
            timings += usize::from(outcome.elapsed_ms.is_some_and(|ms| ms >= 0.0));
        }
        let p = session.presentation();
        assert_eq!(p.composition, "你好");
        assert_eq!(p.cursor, 2);
        assert!(session.composing());
        assert!(!p.clauses.is_empty());
        let outcome = session.key(&Key::new(code::RETURN, 0)).unwrap();
        assert!(outcome.handled);
        timings += usize::from(outcome.elapsed_ms.is_some());
        assert_eq!(session.committed(), "你好");
        assert!(!session.composing());
        commits.push(outcome.commit);
    }
    assert_eq!(commits, ["你好", "你好"]);
    assert!(timings >= 14);
    type_session(&mut session, "su3");
    assert!(key(&mut session, code::SPACE, 0));
    let p = session.presentation().clone();
    assert!(p.count >= 4);
    assert_eq!(p.page_size, 3);
    assert_eq!(p.rows.len(), 3);
    assert_eq!(p.page, 0);
    assert!(p.page_count >= 2);
    assert!(key(&mut session, code::PAGE_DOWN, 0));
    let p = session.presentation().clone();
    assert_eq!(p.page, 1);
    let picked = p.selection.unwrap();
    let word = p.rows.iter().find(|row| row.index == picked).unwrap().text.clone();
    assert!(session.choose(p.count).is_err());
    let outcome = session.choose(picked).unwrap();
    assert!(outcome.handled && outcome.elapsed_ms.is_some());
    key(&mut session, code::RETURN, 0);
    assert_eq!(session.committed(), word);
}

#[test]
fn settings_validation_modifiers_and_pass_through() {
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    session.open(&data(dir.path(), LEXICON), Some(&dir.join("personal"))).unwrap();
    assert!(session.apply_settings(&values(json!({"candidatesPerPage": 3.5}))).is_err());
    assert!(session.apply_settings(&values(json!({"candidatesPerPage": null}))).is_err());
    assert!(session.apply_settings(&values(json!({"learningEnabled": "false"}))).is_err());
    assert!(session.apply_settings(&values(json!({"spaceAction": "bogus"}))).is_err());
    assert!(session.apply_settings(&values(json!({"feature.deleteWord": 1}))).is_err());
    assert!(!key(&mut session, 'A' as i32, modifier::META));
    assert!(!key(&mut session, 'A' as i32, modifier::CONTROL | modifier::ALT));
    assert!(session.presentation().composition.is_empty());
    session.apply_settings(&values(json!({"punctuationKeys": "mac"}))).unwrap();
    assert!(key(&mut session, '<' as i32, modifier::SHIFT));
    assert!(format!("{}{}", session.presentation().composition, session.committed()).contains('，'));
    session.reset().unwrap();
    session.apply_settings(&values(json!({"feature.controlPunctuation": false}))).unwrap();
    assert!(!key(&mut session, ',' as i32, modifier::CONTROL));
    session.apply_settings(&values(json!({"feature.controlPunctuation": true}))).unwrap();
    assert!(key(&mut session, ',' as i32, modifier::CONTROL));
    session.reset().unwrap();
    session.apply_settings(&values(json!({"feature.deleteWord": false}))).unwrap();
    type_session(&mut session, "su3");
    let before = session.presentation().composition.clone();
    assert!(!key(&mut session, code::BACKSPACE, modifier::ALT));
    assert_eq!(session.presentation().composition, before);
    session.apply_settings(&values(json!({"spaceAction": "commit"}))).unwrap();
    let outcome = session.key(&Key::new(code::SPACE, 0)).unwrap();
    assert!(!outcome.handled);
    assert!(outcome.elapsed_ms.is_some(), "a commit that passes the key through is still timed");
    assert_eq!(session.committed(), "你");
    assert!(session.presentation().composition.is_empty());
}

#[test]
fn fuzzy_sounds_change_conversion() {
    let dir = TempDir::new();
    let path =
        write_lexicon(dir.path(), "ㄕㄨ\t書\t-3\nㄙㄨ\t蘇\t-4\nㄈㄨˊ\t服\t-3\nㄈㄨˊ\t福\t-4\nㄕㄨ ㄈㄨˊ\t舒服\t-2\n");
    let data = EngineData::from_lexicon(&path).unwrap();
    let converts = |session: &mut EngineSession| {
        session.reset().unwrap();
        type_session(session, "nj zj6"); // ㄙㄨ ㄈㄨˊ
        let mut found = session.presentation().composition.contains("舒服");
        if !found && key(session, code::DOWN, 0) {
            found = session.presentation().rows.iter().any(|row| row.text == "舒服");
        }
        session.reset().unwrap();
        found
    };
    // Settings applied before the engine opens.
    let mut early = EngineSession::new();
    early.apply_settings(&values(json!({"learningEnabled": false, "fuzzy.sSh": true}))).unwrap();
    early.open(&data, Some(&dir.join("early"))).unwrap();
    assert!(converts(&mut early));
    // Toggled on a live engine, off and on again.
    let mut live = EngineSession::new();
    live.apply_settings(&values(json!({"learningEnabled": false}))).unwrap();
    live.open(&data, Some(&dir.join("live"))).unwrap();
    assert!(!converts(&mut live));
    live.apply_settings(&values(json!({"fuzzy.sSh": true}))).unwrap();
    assert!(converts(&mut live));
    live.apply_settings(&values(json!({"fuzzy.sSh": false}))).unwrap();
    assert!(!converts(&mut live));
    live.apply_settings(&values(json!({"fuzzy.zZh": true, "fuzzy.enEng": true}))).unwrap();
    assert!(!converts(&mut live));
}

#[test]
fn commit_all_and_secure_learning() {
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    session.open(&data(dir.path(), LEXICON), Some(&dir.join("personal"))).unwrap();
    type_session(&mut session, "su3cl3");
    assert!(key(&mut session, code::DOWN, 0));
    assert!(session.presentation().has_list());
    let before = session.presentation().composition.clone();
    let outcome = session.commit_all().unwrap();
    assert!(outcome.elapsed_ms.is_some());
    assert_eq!(outcome.commit, before);
    assert_eq!(session.committed(), before);
    assert!(!session.presentation().has_list());
    assert!(!session.composing());
    assert!(session.commit_all().unwrap().commit.is_empty());
    session.set_learning_suspended(true).unwrap();
    session.set_preceding_text("前文").unwrap();
    type_session(&mut session, "su3");
    assert!(key(&mut session, code::DOWN, 0));
    assert!(session.choose(1).unwrap().handled);
    session.commit_all().unwrap();
    assert!(session.learned_entries().is_empty());
    session.set_learning_suspended(false).unwrap();
    assert!(session.set_preceding_text("前\0文").is_err());
    session.set_preceding_text("").unwrap();
    type_session(&mut session, "su3");
    assert!(key(&mut session, code::DOWN, 0));
    assert!(session.choose(1).unwrap().handled);
    session.commit_all().unwrap();
    assert!(contains(&session.learned_entries(), "妳"));
}

#[test]
fn personal_round_trip_and_validation() {
    let dir = TempDir::new();
    let data = data(dir.path(), LEXICON);
    let user = dir.join("personal");
    let backup = dir.join("backup.txt");
    {
        let mut session = EngineSession::new();
        session.open(&data, Some(&user)).unwrap();
        assert!(session.add_phrase("", "").is_err());
        assert!(session.add_phrase("壞\0詞", "ㄏㄠˇ").is_err());
        assert!(session.add_phrase("好", "invalid").is_err());
        session.add_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ").unwrap();
        assert!(contains(&session.user_phrases(), "郝好"));
        assert_eq!(session.user_phrases()[0].reading, "ㄏㄠˇ ㄏㄠˇ");
        learn_second(&mut session);
        assert!(contains(&session.learned_entries(), "妳"));
        session.exclude_learned("妳", "ㄋㄧˇ").unwrap();
        assert!(!contains(&session.learned_entries(), "妳"));
        assert!(contains(&session.excluded_entries(), "妳"));
        session.export_personal(&backup).unwrap();
        std::fs::write(dir.join("blocker"), "").unwrap();
        assert!(session.export_personal(&dir.join("blocker").join("backup.txt")).is_err());
        session.remove_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ").unwrap();
        session.include_learned("妳", "ㄋㄧˇ").unwrap();
        assert!(session.user_phrases().is_empty());
        assert!(session.excluded_entries().is_empty());
        assert!(session.import_personal(&backup).unwrap() >= 2);
        assert!(contains(&session.user_phrases(), "郝好"));
        assert!(contains(&session.excluded_entries(), "妳"));
        session.include_learned("妳", "ㄋㄧˇ").unwrap();
        learn_second(&mut session);
        session.forget_learned("妳", "ㄋㄧˇ").unwrap();
        assert!(!contains(&session.learned_entries(), "妳"));
        let bad = dir.join("bad.txt");
        std::fs::write(&bad, "not json").unwrap();
        assert!(session.import_personal(&bad).is_err());
        assert!(contains(&session.user_phrases(), "郝好"));
    }
    let mut reopened = EngineSession::new();
    reopened.open(&data, Some(&user)).unwrap();
    assert!(contains(&reopened.user_phrases(), "郝好"));
    assert!(reopened.learned_entries().is_empty());
}

#[test]
fn readings_edit_forget_all_and_load_times() {
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    assert!(session.readings("你好").is_empty());
    assert!(session.forget_all_learned().is_err());
    assert!(session.load_ms().is_none());
    session.open(&data(dir.path(), LEXICON), Some(&dir.join("personal"))).unwrap();
    assert!(session.load_ms().is_some_and(|ms| ms >= 0.0));
    assert!(session.readings("你好").contains(&"ㄋㄧˇ ㄏㄠˇ".to_owned()));
    assert!(session.readings("").is_empty());
    session.add_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ").unwrap();
    session.edit_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ", "好你", "ㄏㄠˇ ㄋㄧˇ").unwrap();
    assert!(contains(&session.user_phrases(), "好你"));
    assert!(!contains(&session.user_phrases(), "郝好"));
    // A rejected new phrase keeps the old one.
    assert!(session.edit_phrase("好你", "ㄏㄠˇ ㄋㄧˇ", "好", "invalid").is_err());
    assert!(contains(&session.user_phrases(), "好你"));
    learn_second(&mut session);
    assert!(contains(&session.learned_entries(), "妳"));
    session.exclude_learned("你", "ㄋㄧˇ").unwrap();
    session.forget_all_learned().unwrap();
    assert!(session.learned_entries().is_empty());
    assert!(contains(&session.user_phrases(), "好你"));
    assert!(contains(&session.excluded_entries(), "你"));
}

#[test]
fn failed_flush_is_reported_and_retryable() {
    let dir = TempDir::new();
    let data = data(dir.path(), LEXICON);
    let user = dir.join("personal");
    let mut session = EngineSession::new();
    session.open(&data, Some(&user)).unwrap();
    std::fs::remove_dir_all(&user).unwrap();
    std::fs::write(&user, "").unwrap();
    assert!(session.add_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ").is_err());
    assert!(contains(&session.user_phrases(), "郝好"));
    std::fs::remove_file(&user).unwrap();
    std::fs::create_dir_all(&user).unwrap();
    session.flush().unwrap();
    let mut reopened = EngineSession::new();
    reopened.open(&data, Some(&user)).unwrap();
    assert!(contains(&reopened.user_phrases(), "郝好"));
}

#[test]
fn punctuation_width_starts_full_and_toggles() {
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    session.open(&data(dir.path(), LEXICON), None).unwrap();
    assert_eq!(session.punctuation_width(), zhuyin_core::api::PunctuationWidth::Full);
    type_session(&mut session, "su3");
    let before = session.presentation().composition.clone();
    assert_ne!(session.toggle_punctuation_width().unwrap(), zhuyin_core::api::PunctuationWidth::Full);
    assert_eq!(session.presentation().composition, before, "the composition stays");
}

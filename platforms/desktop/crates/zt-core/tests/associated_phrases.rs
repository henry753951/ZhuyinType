//! 聯想詞 (`associatedPhrases`): next-word suggestions after a commit, through the input controller and the broker
//! host's view of what the candidate window shows.

mod common;

use common::{TempDir, key_for};
use serde_json::Value;
use zt_core::controller::{ControllerEvent, InputController};
use zt_core::engine::{EngineData, EngineSession};
use zt_core::keys::{Key, code, modifier};
use zt_core::settings::Settings;
use zt_core::shortcuts::Shortcuts;

const LEXICON: &str = "ㄒㄧㄝˋ ㄒㄧㄝˋ\t謝謝\t-3\nㄋㄧˇ\t你\t-2\nㄉㄚˋ ㄐㄧㄚ\t大家\t-3\nㄏㄠˇ\t好\t-2\n";

/// A data directory with the lexicon and a bigram model in which 你 and 大家 follow 謝謝.
fn controller(dir: &TempDir, on: bool) -> InputController {
    std::fs::write(dir.join("lexicon.tsv"), LEXICON).unwrap();
    let lm = dir.join("lm");
    std::fs::create_dir_all(&lm).unwrap();
    std::fs::write(lm.join("unigram.tsv"), "謝謝\t-3\n你\t-2\n大家\t-3\n好\t-2\n</s>\t-1\n").unwrap();
    std::fs::write(lm.join("bigram.tsv"), "<s>\t謝謝\t-1\n謝謝\t你\t-0.5\n謝謝\t大家\t-1\n大家\t好\t-0.3\n").unwrap();
    std::fs::write(lm.join("backoff.tsv"), "").unwrap();
    let settings = Settings::open(Some(dir.join("settings.json")));
    settings.set("associatedPhrases", Value::Bool(on)).unwrap();
    let shortcuts = Shortcuts::new(settings.clone());
    let mut session = EngineSession::new();
    session.apply(&settings).unwrap();
    session.open(&EngineData::from_lexicon(&dir.join("lexicon.tsv")).unwrap(), Some(&dir.join("personal"))).unwrap();
    InputController::new(session, settings, shortcuts)
}

fn commits(input: &mut InputController) -> Vec<String> {
    input
        .take_events()
        .into_iter()
        .filter_map(|event| match event {
            ControllerEvent::Commit(text) => Some(text),
            _ => None,
        })
        .collect()
}

/// Types 謝謝 and sends it with Enter.
fn thanks(input: &mut InputController) {
    for c in "vu,4vu,4".chars() {
        input.press(&key_for(c));
    }
    assert!(input.press(&Key::new(code::RETURN, 0)));
    assert_eq!(commits(input), ["謝謝"]);
}

fn rows(input: &InputController) -> Vec<String> {
    input.presentation().rows.iter().map(|row| row.text.clone()).collect()
}

#[test]
fn off_by_default_and_when_off() {
    let dir = TempDir::new();
    assert_eq!(zt_core::settings::descriptor("associatedPhrases").map(|d| d.default.clone()), Some(Value::Bool(false)));
    let mut input = controller(&dir, false);
    thanks(&mut input);
    assert!(!input.presentation().has_list());
}

#[test]
fn list_after_commit_and_shift_pick() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    thanks(&mut input);
    let p = input.presentation();
    assert!(p.associated && p.composition.is_empty());
    assert_eq!(rows(&input), ["你", "大家"]);
    assert!(p.panel_model(None).associated);
    // ⇧2 sends 大家 and lists what follows it.
    assert!(input.press(&Key::new('@' as i32, modifier::SHIFT)));
    assert_eq!(commits(&mut input), ["大家"]);
    assert_eq!(rows(&input), ["好"]);
    // A click picks too.
    assert!(input.choose(0));
    assert_eq!(commits(&mut input), ["好"]);
    assert!(!input.presentation().has_list());
}

#[test]
fn other_keys_close_the_list() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    thanks(&mut input);
    // Space goes to the app (it is not a pick) and closes the list.
    assert!(!input.press(&Key::new(code::SPACE, 0)));
    assert!(!input.presentation().has_list() && commits(&mut input).is_empty());
    // A Zhuyin key starts a composition.
    thanks(&mut input);
    assert!(input.press(&key_for('s')));
    assert_eq!(input.presentation().composition, "ㄋ");
    assert!(!input.presentation().has_list());
    // Esc only closes it.
    let mut input = controller(&TempDir::new(), true);
    thanks(&mut input);
    assert!(input.press(&Key::new(code::ESCAPE, 0)));
    assert!(!input.presentation().has_list());
}

#[test]
fn not_in_private_fields() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    input.set_private(true);
    thanks(&mut input);
    assert!(!input.presentation().has_list());
}

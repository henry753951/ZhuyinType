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
fn passive_list_down_enters_and_keys_pick() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    thanks(&mut input);
    let p = input.presentation();
    assert!(p.associated && p.composition.is_empty());
    assert_eq!((p.selection, p.selected), (None, None), "passive: nothing highlighted");
    assert_eq!(rows(&input), ["你", "大家"]);
    assert!(p.panel_model(None).associated && p.passive_suggestions());
    // No key labels while passive: 1 and 2 type ㄅ and ㄉ, so a label would name a key that does not pick the row.
    assert!(p.rows.iter().all(|row| row.key.is_none()), "{:?}", p.rows);
    // ↓ enters the list: the keys that now pick label the rows; 2 sends 大家 and lists, passively, what follows it.
    assert!(input.press(&Key::new(code::DOWN, 0)));
    assert_eq!(input.presentation().selected, Some(0));
    assert!(!input.presentation().passive_suggestions());
    assert_eq!(input.presentation().rows.iter().map(|row| row.key).collect::<Vec<_>>(), [Some('1'), Some('2')]);
    assert!(input.press(&key_for('2')));
    assert_eq!(commits(&mut input), ["大家"]);
    assert_eq!(rows(&input), ["好"]);
    assert_eq!(input.presentation().selected, None);
    // A click picks without entering.
    assert!(input.choose(0));
    assert_eq!(commits(&mut input), ["好"]);
    assert!(!input.presentation().has_list());
}

#[test]
fn shift_digit_types_as_with_no_list() {
    // ⇧1 after 謝謝 does exactly what it does with the feature off (it never sends a suggestion).
    let mut outcomes = Vec::new();
    for on in [false, true] {
        let dir = TempDir::new();
        let mut input = controller(&dir, on);
        thanks(&mut input);
        assert_eq!(input.presentation().has_list(), on);
        let handled = input.press(&Key::new('!' as i32, modifier::SHIFT));
        outcomes.push((handled, commits(&mut input), input.presentation().composition.clone()));
        assert!(!input.presentation().has_list());
    }
    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn not_in_private_fields() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    input.set_private(true);
    thanks(&mut input);
    assert!(!input.presentation().has_list());
}

#[test]
fn digit_on_a_passive_list_types_zhuyin() {
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    thanks(&mut input);
    assert!(input.press(&key_for('1')));
    assert!(commits(&mut input).is_empty());
    let p = input.presentation();
    assert!(!p.has_list() && p.composition == "ㄅ", "{p:?}");
}

/// What the broker tells a text service (TSF) that claims only the keys it knows the host takes: a passive list is not
/// a composition (so Return, arrows and shortcuts stay the application's) but says `associated`, so ↓ is claimed;
/// once entered, the list takes every key.
#[test]
fn the_broker_claims_only_down_for_a_passive_list() {
    use serde_json::json;
    use zt_core::broker::host::Host;
    let dir = TempDir::new();
    let mut input = controller(&dir, true);
    let mut host = Host::default();
    let mut request = |input: &mut InputController, fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        host.dispatch(input, 1, object.as_object().unwrap()).0
    };
    request(&mut input, json!({"op": "focus", "panel": true}));
    let key = |code: i32| json!({"op": "key", "key": code, "modifiers": 0, "release": false});
    for c in "vu,4vu,4".chars() {
        request(&mut input, key(key_for(c).code));
    }
    let reply = request(&mut input, key(code::RETURN));
    assert_eq!(reply["commit"], json!("謝謝"));
    assert_eq!((reply["candidates"].clone(), reply["composing"].clone()), (json!(true), json!(false)));
    assert_eq!(reply["associated"], json!(true));
    // ↓ enters: a list like any other, every key the service's.
    let reply = request(&mut input, key(code::DOWN));
    assert_eq!((reply["handled"].clone(), reply["composing"].clone()), (json!(true), json!(true)));
    assert!(reply.get("associated").is_none());
    // ↑ on the first word goes back to passive; ← then closes the list and is the application's.
    let reply = request(&mut input, key(code::UP));
    assert_eq!(reply["associated"], json!(true));
    let reply = request(&mut input, key(code::LEFT));
    assert_eq!((reply["handled"].clone(), reply["candidates"].clone()), (json!(false), json!(false)));
    assert!(reply.get("associated").is_none() && reply["composing"] == json!(false));
}

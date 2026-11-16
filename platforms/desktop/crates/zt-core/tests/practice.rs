//! Practice sessions: a sandbox of their own (settings, keys, personal data), typing into a text of their own.

mod common;

use common::{CONTROLLER_LEXICON, TempDir, write_lexicon};
use serde_json::json;
use zt_core::keys::{Key, ShiftSide, code, modifier};
use zt_core::practice::{PracticeEvent, PracticeGoal, event};
use zt_core::service::{PopupAction, Service, ServiceConfig};

fn service(dir: &TempDir) -> Service {
    write_lexicon(dir.path(), CONTROLLER_LEXICON);
    let config = ServiceConfig {
        data_dir: Some(dir.path().to_path_buf()),
        user_dir: Some(dir.join("user")),
        settings_file: Some(dir.join("settings.json")),
        ..ServiceConfig::default()
    };
    Service::start(config).unwrap().0
}

fn type_keys(field: &mut zt_core::practice::PracticeSession, keys: &str) {
    for c in keys.chars() {
        field.key_down(&common::key_for(c));
    }
}

#[test]
fn types_into_its_own_text_and_reports_the_outcome() {
    let dir = TempDir::new();
    let service = service(&dir);
    let mut field = service.practice().unwrap();
    type_keys(&mut field, "su3cl3");
    assert_eq!(field.presentation().composition, "你好");
    assert_eq!(field.shown(), "你好");
    assert!(field.key_down(&Key::new(code::RETURN, 0)));
    assert_eq!(field.text(), "你好");
    assert!(PracticeGoal::Text("你好".into()).is_met(field.outcome()));
    // A pick from the list in place of different text.
    type_keys(&mut field, "su3");
    field.key_down(&Key::new(code::DOWN, 0));
    assert!(field.outcome().events.contains(&event::OPEN.to_owned()));
    field.popup_action(PopupAction::Pick(1));
    assert_eq!(field.outcome().picks, ["妳"]);
    assert!(PracticeGoal::Correct("妳".into()).is_met(field.outcome()));
    field.end_session();
    assert_eq!(field.text(), "你好妳");
    // Keys the input method leaves to the app edit the text, as a text field does.
    assert!(field.key_down(&Key::new(code::BACKSPACE, 0)));
    assert_eq!(field.text(), "你好");
    field.key_down(&Key::new(code::LEFT, 0));
    assert_eq!(field.insertion(), 1);
    assert!(!field.key_down(&Key::new(code::TAB, 0)), "Tab with nothing composed goes on to the window");
    // 英: letters are typed as they are.
    field.key_down(&Key::shift(ShiftSide::Left));
    field.key_up(&Key { shift_side: ShiftSide::Left, ..Key::new(code::SHIFT, 0) });
    assert!(field.english());
    assert!(field.take_events().contains(&PracticeEvent::ModeToast { english: true }));
    field.key_down(&Key { text: "a".into(), ..Key::new('A' as i32, 0) });
    assert_eq!(field.text(), "你a好");
    assert!(PracticeGoal::SwitchMode(1).is_met(field.outcome()));
    field.reset("前文");
    assert_eq!(field.text(), "前文");
    assert!(field.outcome().events.is_empty());
}

#[test]
fn nothing_reaches_the_user() {
    let dir = TempDir::new();
    let service = service(&dir);
    let mut field = service.practice().unwrap();
    field.settings().set("candidateLayout", json!("horizontal")).unwrap();
    assert_eq!(service.settings().string("candidateLayout"), "vertical");
    field.shortcuts().record("typeZhuyin", 'U' as i32, modifier::ALT, None).unwrap();
    assert!(!service.shortcuts().is_customised("typeZhuyin"));
    field.controller_mut().session_mut().add_phrase("郝好", "ㄏㄠˇ ㄏㄠˇ").unwrap();
    assert!(service.user_phrases().is_empty());
    type_keys(&mut field, "su3");
    field.key_down(&Key::new(code::RETURN, 0));
    assert!(service.learned_entries().is_empty());
    assert_eq!(service.stats().usage().counters.characters, 0);
    // Practice fields open at the same time share their practice data.
    let other = service.practice().unwrap();
    assert!(other.presentation().is_empty());
    let mut other = other;
    assert!(other.controller_mut().session().user_phrases().iter().any(|p| p.word == "郝好"));
    // A preset chosen again: the user's settings and keys come back.
    field.restart();
    assert_eq!(field.settings().string("candidateLayout"), "vertical");
    assert!(!field.shortcuts().is_customised("typeZhuyin"));
}

#[test]
fn added_user_phrases_are_reported() {
    let dir = TempDir::new();
    let service = service(&dir);
    let mut field = service.practice().unwrap();
    type_keys(&mut field, "su3cl3");
    field.key_down(&Key::new(code::LEFT, modifier::SHIFT));
    field.key_down(&Key::new(code::LEFT, modifier::SHIFT));
    field.key_down(&Key::new(code::RETURN, 0));
    let added = field.added_phrases();
    assert!(added.iter().any(|(word, _)| word == "你好"), "{added:?} {:?}", field.outcome());
    assert!(PracticeGoal::AddUserPhrase("你好".into()).is_met(field.outcome()));
}

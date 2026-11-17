//! 快速設定: 暫停學習 (nothing learned, no correction log, never stored) and 繁／簡 output (the composition, the
//! candidates and the commits converted as they leave; the engine and learning stay traditional).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::Fixture;
use serde_json::{Value, json};
use zt_core::controller::ControllerEvent;
use zt_core::corrections::CorrectionLog;
use zt_core::keys::{code, modifier};

/// Every file under `dir` with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return found };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            for (name, bytes) in files(&path) {
                found.insert(format!("{}/{name}", entry.file_name().to_string_lossy()), bytes);
            }
        } else {
            found.insert(entry.file_name().to_string_lossy().into_owned(), std::fs::read(&path).unwrap_or_default());
        }
    }
    found
}

/// Types 妳 (the second candidate of ㄋㄧˇ) and sends it: a pick the engine learns and the correction log records.
fn pick_second(f: &mut Fixture) {
    f.type_keys("su3");
    assert!(f.press(code::DOWN, 0));
    assert!(f.press('2' as i32, 0));
    assert!(f.press(code::RETURN, 0));
}

#[test]
fn paused_learning_keeps_personal_data_and_correction_log() {
    let mut f = Fixture::new();
    let log = CorrectionLog::open(Some(&f.dir.join("user")));
    f.input.set_correction_log(Some(log.clone()), "test");
    f.settings.set("recordsCorrections", Value::Bool(true)).unwrap();
    assert!(!f.input.learning_paused(), "a new controller learns");
    f.input.session_mut().flush().unwrap();
    let before = files(&f.dir.join("personal"));

    f.input.set_learning_paused(true);
    assert!(f.input.learning_paused() && f.input.unlearned() && !f.input.private());
    pick_second(&mut f);
    assert_eq!(f.commits(), ["妳"], "typing works while paused");
    f.input.session_mut().flush().unwrap();
    assert!(f.input.session().learned_entries().is_empty(), "nothing learned");
    assert_eq!(files(&f.dir.join("personal")), before, "the personal data on disk is unchanged");
    assert_eq!(log.count(), 0, "no correction log");
    assert_eq!(f.input.recent_text(), "", "nothing sent is kept as context");

    f.input.set_learning_paused(false);
    pick_second(&mut f);
    f.input.session_mut().flush().unwrap();
    assert!(!f.input.session().learned_entries().is_empty(), "learning again");
    assert_eq!(log.count(), 1);
    assert_ne!(files(&f.dir.join("personal")), before);
}

#[test]
fn paused_learning_is_never_stored() {
    let mut f = Fixture::new();
    f.input.set_learning_paused(true);
    // A new controller on the same settings (the next start) learns again.
    let again = Fixture::new();
    assert!(!again.input.learning_paused());
    assert!(f.settings.stored().keys().all(|key| !key.to_lowercase().contains("pause")));
    // A private field ending does not end the pause.
    f.input.set_private(true);
    f.input.set_private(false);
    assert!(f.input.learning_paused() && f.input.unlearned());
}

const SCRIPT_LEXICON: &str = "ㄏㄨㄚˋ\t話\t-3\nㄏㄨㄚˋ\t畫\t-4\n";

fn script_fixture() -> Fixture {
    let mut f = Fixture::new();
    let data = common::data(f.dir.path(), SCRIPT_LEXICON);
    f.input.session_mut().open(&data, None).unwrap();
    f.input.refresh_settings();
    f
}

#[test]
fn simplified_output_converts_composition_candidates_and_commits() {
    let mut f = script_fixture();
    f.type_keys("cj84");
    assert_eq!(f.composition(), "話");
    assert_eq!(f.input.output_presentation().composition, "話", "繁體 by default");
    f.settings.set("outputScript", json!("simplified")).unwrap();
    assert!(f.input.simplified());
    assert_eq!(f.composition(), "話", "the engine stays traditional");
    assert_eq!(f.input.output_presentation().composition, "话");
    assert!(f.press(code::DOWN, 0));
    let shown = f.input.output_presentation().into_owned();
    let rows: Vec<&str> = shown.rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(rows, ["话", "画"]);
    assert_eq!(shown.cursor, f.input.presentation().cursor);
    assert_eq!(shown.clauses, f.input.presentation().clauses);
    assert!(f.press('2' as i32, 0));
    assert!(f.press(code::RETURN, 0));
    assert_eq!(f.commits(), ["画"]);
    // Back to 繁體.
    f.settings.set("outputScript", json!("traditional")).unwrap();
    f.type_keys("cj84");
    let traditional = f.composition();
    assert!(traditional == "畫" || traditional == "話");
    assert!(f.press(code::RETURN, 0));
    assert_eq!(f.commits(), [traditional]);
}

#[test]
fn quick_settings_shortcut_has_no_key_and_opens_the_panel() {
    let mut f = Fixture::new();
    assert!(f.shortcuts.bindings("quickSettings").is_empty(), "no default key");
    let command = zt_core::shortcuts::Shortcuts::command("quickSettings").expect("a command");
    assert_eq!((command.label, command.title, command.required), ("快速設定", "開啟快速設定", false));
    f.shortcuts.record("quickSettings", 'Q' as i32, modifier::ALT, None).unwrap();
    f.input.take_events();
    assert!(f.press('Q' as i32, modifier::ALT));
    assert!(f.input.take_events().contains(&ControllerEvent::QuickPanel));
    // In English too: the panel is how to switch back.
    f.input.set_english(true);
    f.input.take_events();
    assert!(f.press('Q' as i32, modifier::ALT));
    assert!(f.input.take_events().contains(&ControllerEvent::QuickPanel));
}

#[test]
fn full_width_follows_the_quick_switch() {
    let mut f = Fixture::new();
    assert!(f.input.full_width());
    f.input.take_events();
    f.input.set_full_width(false);
    assert!(!f.input.full_width());
    assert_eq!(f.input.take_events(), [ControllerEvent::WidthSwitched { full: false }]);
    f.input.set_full_width(false);
    assert!(f.input.take_events().is_empty(), "unchanged: no badge");
}

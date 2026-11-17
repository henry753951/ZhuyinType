//! Port of the Qt `StateTest`: the preview candidate list and the settings store.

mod common;

use common::TempDir;
use serde_json::Value;
use zhuyin_core::api::Candidate;
use zt_core::presentation::{PreviewIntent, PreviewList, Rect, popup_position};
use zt_core::settings::Settings;

fn entries(count: usize) -> Vec<Candidate> {
    (0..count).map(|i| Candidate { text: i.to_string(), annotation: None }).collect()
}

#[test]
fn empty_candidates_and_boundary_intents() {
    let mut list = PreviewList::default();
    assert_eq!(list.selected(), None);
    assert!(list.presentation("", &[]).rows.is_empty());
    assert_eq!(list.turn_page(1), None);
    assert_eq!(list.move_selection(-1), None);
    list.present(vec![Candidate { text: "字".into(), annotation: None }], None, 0);
    let revision = list.revision();
    assert_eq!(list.turn_page(0), None);
    assert_eq!(list.move_selection(0), None);
    assert_eq!(list.turn_page(i64::MIN), Some(PreviewIntent::Boundary(-1)));
    assert_eq!(list.move_selection(i64::MAX), Some(PreviewIntent::Boundary(1)));
    assert_eq!(list.revision(), revision);
    assert_eq!(list.page(), 0);
    assert_eq!(list.selected(), Some(0));
}

#[test]
fn resize_preserves_absolute_selection() {
    let mut list = PreviewList::default();
    list.present(entries(23), None, 22);
    for size in 3..=9 {
        list.set_page_size(size);
        assert_eq!(list.page() * size + list.selected().unwrap(), 22);
        assert_eq!(list.pick(list.selected().unwrap()), Some(PreviewIntent::Picked(22)));
    }
    list.present(Vec::new(), None, 0);
    list.set_page_size(3);
    assert_eq!(list.page(), 0);
    assert_eq!(list.selected(), None);
}

#[test]
fn revisions_separate_lists_from_navigation() {
    let mut list = PreviewList::default();
    let mut words = entries(12);
    list.present(words.clone(), Some("ㄇㄧㄥˊ".into()), 0);
    let revision = list.revision();
    list.turn_page(1);
    list.move_selection(1);
    let _ = list.pick(0);
    assert_eq!(list.revision(), revision);
    list.present(words.clone(), Some("ㄇㄧㄥˊ".into()), 10);
    assert_eq!(list.revision(), revision);
    words[0].text = "名".into();
    list.present(words, Some("ㄇㄧㄥˊ".into()), 0);
    assert!(list.revision() > revision);
    let updated = list.revision();
    list.set_page_size(4);
    assert!(list.revision() > updated);
}

#[test]
fn page_boundaries_and_selection() {
    let mut list = PreviewList::default();
    list.present(entries(10), Some("reading".into()), 8);
    list.turn_page(1);
    assert_eq!(list.selected(), Some(0));
    assert_eq!(list.pick(1), None);
    assert_eq!(list.pick(0), Some(PreviewIntent::Picked(9)));
    list.set_page_size(3);
    assert_eq!(list.page(), 3);
    list.turn_page(i64::MAX);
    assert_eq!(list.page(), 3);
    list.turn_page(-100);
    assert_eq!(list.page(), 0);
    let p = list.presentation("小明", &['1', '2', '3']);
    assert_eq!(p.rows.len(), 3);
    assert_eq!(p.rows[2].key, Some('3'));
    assert_eq!(p.page_count, 4);
    list.present(Vec::new(), None, 0);
    assert_eq!(list.selected(), None);
    assert_eq!(list.pick(0), None);
    list.set_page_size(0);
    assert_eq!(list.page_size(), 3);
}

#[test]
fn navigation_does_not_commit_and_filters_are_asked_for() {
    let mut list = PreviewList::default();
    list.present(entries(10), None, 8);
    assert_eq!(list.move_selection(1), None);
    assert_eq!((list.page(), list.selected()), (1, Some(0)));
    assert_eq!(list.move_selection(i64::MAX), Some(PreviewIntent::Boundary(1)));
    assert_eq!(list.move_selection(-1), None);
    assert_eq!((list.page(), list.selected()), (0, Some(8)));
    assert_eq!(list.toggle_single(), PreviewIntent::Filter { single: true, corrections: false });
    assert_eq!(list.toggle_corrections(), PreviewIntent::Filter { single: true, corrections: true });
}

#[test]
fn settings_validate_and_persist() {
    let dir = TempDir::new();
    let path = dir.join("settings.json");
    {
        let settings = Settings::open(Some(path.clone()));
        assert!(settings.set("candidateFontSize", Value::from(37)).is_err());
        assert!(settings.set("candidateFontSize", Value::from(18.5)).is_err());
        assert!(settings.set("candidateFontSize", Value::Null).is_err());
        assert!(settings.set("candidateFontSize", Value::from("18")).is_err());
        assert!(settings.set("panelAppearance", Value::from("glass")).is_err());
        assert!(settings.set("showsZhuyinHint", Value::from("false")).is_err());
        settings.set("candidateFontSize", Value::from(24)).unwrap();
        settings.set("showsZhuyinHint", Value::Bool(false)).unwrap();
        settings.set("panelAppearance", Value::from("dark")).unwrap();
    }
    let restored = Settings::open(Some(path));
    assert_eq!(restored.number("candidateFontSize"), 24.0);
    assert!(!restored.bool("showsZhuyinHint"));
    assert_eq!(restored.string("panelAppearance"), "dark");
}

#[test]
fn corrupt_values_use_defaults() {
    let dir = TempDir::new();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{"candidatesPerPage": -99, "panelAppearance": "invalid"}"#).unwrap();
    let settings = Settings::open(Some(path));
    assert_eq!(settings.number("candidatesPerPage"), 9.0);
    assert_eq!(settings.string("panelAppearance"), "system");
}

#[test]
fn restore_only_known_preferences() {
    let dir = TempDir::new();
    let settings = Settings::open(Some(dir.join("settings.json")));
    settings.set("unrelated", Value::from("keep")).unwrap();
    settings.set("candidateFontSize", Value::from(27)).unwrap();
    let before = settings.revision();
    settings.reset_all().unwrap();
    assert!(settings.revision() > before);
    assert_eq!(settings.number("candidateFontSize"), 18.0);
    assert_eq!(settings.string("unrelated"), "keep");
}

#[test]
fn an_unreadable_settings_file_is_set_aside_before_it_is_replaced() {
    // A file cut short by a crash, broken in an editor, or of another shape: the defaults apply, and the first change
    // must not silently replace the user's settings (and keys) with that one value.
    for original in [r#"{"candidateFontSize": 24, "panelAppearance": "da"#, "[1, 2, 3]", ""] {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        std::fs::write(&path, original).unwrap();
        let settings = Settings::open(Some(path.clone()));
        assert_eq!(settings.number("candidateFontSize"), 18.0);
        settings.set("showsZhuyinHint", Value::Bool(false)).unwrap();
        let aside = dir.join("settings.unreadable.json");
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), original, "{original:?}");
        // The next change leaves the copy alone.
        settings.set("candidateFontSize", Value::from(20)).unwrap();
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), original);
        let restored = Settings::open(Some(path));
        assert!(!restored.bool("showsZhuyinHint"));
        assert_eq!(restored.number("candidateFontSize"), 20.0);
    }
}

#[test]
fn a_missing_settings_file_sets_nothing_aside() {
    let dir = TempDir::new();
    let settings = Settings::open(Some(dir.join("settings.json")));
    settings.set("showsZhuyinHint", Value::Bool(false)).unwrap();
    assert!(!dir.join("settings.unreadable.json").exists());
}

#[test]
fn a_reset_that_cannot_be_saved_changes_nothing() {
    let dir = TempDir::new();
    let folder = dir.join("config");
    let settings = Settings::open(Some(folder.join("settings.json")));
    settings.set("candidateFontSize", Value::from(24)).unwrap();
    // The folder is gone and a file has its name: nothing can be written.
    std::fs::remove_dir_all(&folder).unwrap();
    std::fs::write(&folder, "").unwrap();
    assert!(settings.reset("candidateFontSize").is_err());
    assert_eq!(settings.number("candidateFontSize"), 24.0);
    assert!(settings.reset_all().is_err());
    assert_eq!(settings.number("candidateFontSize"), 24.0);
}

#[test]
fn files_written_at_once_by_several_threads_stay_whole() {
    // Statistics are saved from the broker's thread and the UI's; an export may share a folder with other files.
    let dir = TempDir::new();
    let path = dir.join("stats.json");
    std::fs::write(dir.join("stats.tmp"), "someone else's file").unwrap();
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let path = path.clone();
            std::thread::spawn(move || {
                let text = format!("{{\"writer\": {n}, \"padding\": \"{}\"}}", "x".repeat(64 * 1024));
                for _ in 0..20 {
                    zt_core::settings::write_atomically(&path, text.as_bytes()).unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(serde_json::from_str::<Value>(&text).is_ok());
    assert_eq!(std::fs::read_to_string(dir.join("stats.tmp")).unwrap(), "someone else's file");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 2, "{left:?}");
}

#[test]
fn assign_is_all_or_nothing_and_sandboxes_stay_apart() {
    let settings = Settings::open(None);
    let bad = [("candidateFontSize".to_owned(), Value::from(20)), ("panelAppearance".to_owned(), Value::from("glass"))];
    assert!(settings.assign(&bad.into_iter().collect()).is_err());
    assert_eq!(settings.number("candidateFontSize"), 18.0);
    let sandbox = settings.sandbox();
    sandbox.set("candidateFontSize", Value::from(22)).unwrap();
    assert_eq!(settings.number("candidateFontSize"), 18.0);
    assert_eq!(sandbox.number("candidateFontSize"), 22.0);
}

#[test]
fn popup_goes_below_or_above_the_caret() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let caret = Rect::new(300.0, 300.0, 1.0, 18.0);
    assert_eq!(popup_position(caret, 200.0, 100.0, false, 30.0, screen), (270.0, 322.0));
    assert_eq!(popup_position(caret, 200.0, 100.0, true, 30.0, screen), (270.0, 196.0));
    // No room below: above; never off the screen.
    let low = Rect::new(1900.0, 1060.0, 1.0, 18.0);
    assert_eq!(popup_position(low, 200.0, 100.0, false, 0.0, screen), (1720.0, 956.0));
}

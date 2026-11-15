//! Port of the Qt `ShortcutTest` (Linux and Windows words; the Qt V1 storage migration has no counterpart here).

mod common;

use common::TempDir;
use serde_json::{Value, json};
use zt_core::keys::{code, modifier};
use zt_core::settings::Settings;
use zt_core::shortcuts::{STORAGE_KEY, Shortcuts};

/// Alt with a letter is free on every platform.
const FREE: u32 = modifier::ALT;

fn fresh() -> (Settings, Shortcuts) {
    let settings = Settings::open(None);
    let shortcuts = Shortcuts::new(settings.clone());
    (settings, shortcuts)
}

fn resolve(shortcuts: &Shortcuts, key: i32, modifiers: u32) -> Option<&'static str> {
    shortcuts.resolve(key, modifiers).map(|command| command.id)
}

#[test]
fn defaults_and_normalization() {
    let (_, shortcuts) = fresh();
    assert_eq!(Shortcuts::commands().len(), 13, "the macOS commands, with 標點全半形 and 插入空白");
    assert!(shortcuts.error().is_none());
    assert!(!shortcuts.any_customised());
    assert_eq!(resolve(&shortcuts, code::TAB, 0), Some("nextPage"));
    assert_eq!(resolve(&shortcuts, code::BACKTAB, 0), Some("previousPage"));
    assert_eq!(resolve(&shortcuts, code::ENTER, modifier::SHIFT | modifier::KEYPAD), Some("commitWithNewline"));
    assert_eq!(resolve(&shortcuts, code::SPACE, modifier::SHIFT), Some("togglePunctuationWidth"));
    assert_eq!(shortcuts.bindings("nextPage").len(), 3);
    for command in Shortcuts::commands() {
        assert_eq!(command.row(), format!("keys-{}", command.id));
        for binding in &command.defaults {
            assert_eq!(resolve(&shortcuts, binding.key, binding.modifiers), Some(command.id));
        }
    }
}

#[test]
fn labels() {
    let (settings, shortcuts) = fresh();
    assert_eq!(shortcuts.active_label("singleCharacterList"), "⇧↓");
    assert_eq!(shortcuts.active_label("typeZhuyin"), "`");
    let next = shortcuts.bindings("nextPage");
    assert_eq!(next[1].label, "PgDn");
    assert_eq!(next[2].label, "Alt+↓");
    assert_eq!(next[2].spoken, "Alt+↓");
    assert_eq!(shortcuts.bindings("previousPage")[0].spoken, "Shift+Tab");
    shortcuts.record("typeZhuyin", 'U' as i32, FREE | modifier::SHIFT, Some(0)).unwrap();
    assert_eq!(shortcuts.active_label("typeZhuyin"), "Alt+⇧U");
    settings.set("feature.typeZhuyin", Value::Bool(false)).unwrap();
    assert_eq!(shortcuts.active_label("typeZhuyin"), "");
    assert_eq!(Shortcuts::modifier_label(modifier::CONTROL | modifier::ALT), "Ctrl+Alt+");
}

#[test]
fn persists_only_changed_commands() {
    let dir = TempDir::new();
    let path = dir.join("settings.json");
    {
        let settings = Settings::open(Some(path.clone()));
        settings.set("unrelated", Value::from("keep")).unwrap();
        let shortcuts = Shortcuts::new(settings.clone());
        shortcuts.record("typeZhuyin", 'U' as i32, FREE, None).unwrap();
        assert!(shortcuts.is_customised("typeZhuyin"));
        assert!(!shortcuts.is_customised("nextPage"));
        assert_eq!(shortcuts.bindings("typeZhuyin").len(), 2);
        shortcuts.remove("typeZhuyin", 0).unwrap();
        assert_eq!(resolve(&shortcuts, code::QUOTE_LEFT, 0), None);
        let stored = settings.get(STORAGE_KEY);
        assert_eq!(stored.as_object().unwrap().keys().collect::<Vec<_>>(), ["typeZhuyin"]);
    }
    let settings = Settings::open(Some(path));
    let restored = Shortcuts::new(settings.clone());
    assert_eq!(resolve(&restored, 'U' as i32, FREE), Some("typeZhuyin"));
    assert_eq!(restored.bindings("typeZhuyin").len(), 1);
    restored.reset(None).unwrap();
    assert!(!restored.is_customised("typeZhuyin"));
    assert_eq!(settings.string("unrelated"), "keep");
}

#[test]
fn reserved_keys_and_invalid_inputs() {
    let (settings, shortcuts) = fresh();
    let original = shortcuts.all();
    let revision = settings.revision();
    let invalid: &[(i32, u32)] = &[
        ('A' as i32, 0),
        (code::SPACE, 0),
        (code::ESCAPE, 0),
        (code::LEFT, 0),
        (code::HOME, 0),
        (code::PAGE_DOWN, 0),
        (code::BACKSPACE, 0),
        ('A' as i32, modifier::SHIFT),
        ('1' as i32, modifier::ALT),
        ('A' as i32, modifier::META),
        (',' as i32, modifier::CONTROL),
        (code::CONTROL, 0),
        (code::UNKNOWN, 0),
        ('A' as i32, modifier::GROUP_SWITCH),
        (-1, 0),
        ('A' as i32, u32::MAX),
        ('!' as i32, modifier::SHIFT),
        ('C' as i32, modifier::CONTROL),
        ('T' as i32, modifier::CONTROL | modifier::SHIFT),
    ];
    for &(key, modifiers) in invalid {
        assert!(shortcuts.record("typeZhuyin", key, modifiers, Some(0)).is_err(), "{key:#x} {modifiers:#x}");
        assert!(shortcuts.error().is_some());
        assert_eq!(shortcuts.all(), original);
    }
    let reason = |key, modifiers| shortcuts.record("typeZhuyin", key, modifiers, Some(0)).unwrap_err().to_string();
    assert!(reason(code::PAGE_DOWN, 0).contains("用來翻頁"));
    assert!(reason(code::HOME, 0).contains("移到組字的開頭"));
    assert!(reason(',' as i32, modifier::CONTROL).contains("中文標點"));
    assert!(shortcuts.record("unknown", 'U' as i32, FREE, None).is_err());
    assert!(shortcuts.record("typeZhuyin", 'U' as i32, FREE, Some(999)).is_err());
    assert_eq!(settings.revision(), revision, "nothing was stored");
    // With 中文標點 off, Ctrl with a symbol key is free.
    settings.set("feature.controlPunctuation", Value::Bool(false)).unwrap();
    shortcuts.record("typeZhuyin", ',' as i32, modifier::CONTROL, Some(0)).unwrap();
}

#[test]
fn conflicts_and_partial_reset() {
    let (_, shortcuts) = fresh();
    let error = |result: anyhow::Result<()>| result.unwrap_err().to_string();
    assert_eq!(error(shortcuts.record("typeZhuyin", code::TAB, 0, None)), "已用於「下一頁」");
    assert_eq!(error(shortcuts.record("nextPage", code::PAGE_DOWN, 0, Some(0))), "已是這個功能的另一個按鍵");
    shortcuts.remove("nextPage", 0).unwrap();
    shortcuts.record("typeZhuyin", code::TAB, 0, None).unwrap();
    // 還原預設 brings back the defaults no other command uses: Tab stays with 打出注音本身.
    shortcuts.reset(Some("nextPage")).unwrap();
    assert_eq!(shortcuts.bindings("nextPage").len(), 2);
    assert_eq!(resolve(&shortcuts, code::TAB, 0), Some("typeZhuyin"));
    // Every default taken: the reset says which command has it.
    shortcuts.record("commitWithNewline", 'J' as i32, FREE, Some(0)).unwrap();
    shortcuts.record("typeZhuyin", code::RETURN, modifier::SHIFT, None).unwrap();
    let previous = shortcuts.all();
    assert_eq!(error(shortcuts.reset(Some("commitWithNewline"))), "已用於「打出注音本身」");
    assert_eq!(shortcuts.all(), previous);
    shortcuts.reset(None).unwrap();
    assert_eq!(resolve(&shortcuts, code::TAB, 0), Some("nextPage"));
    assert!(shortcuts.error().is_none());
}

#[test]
fn defaults_taken_by_another_command_conflict() {
    let (settings, shortcuts) = fresh();
    settings.set(STORAGE_KEY, json!({"typeZhuyin": [{"key": code::RETURN, "modifiers": modifier::SHIFT}]})).unwrap();
    assert!(shortcuts.error().is_none());
    // 送出並換行 keeps its only default although 打出注音本身 has it: one conflict, shown on both.
    assert_eq!(shortcuts.conflicts().len(), 1);
    assert_eq!(shortcuts.conflicts_of("commitWithNewline").get(&0), Some(&"typeZhuyin"));
    assert_eq!(shortcuts.conflicts_of("typeZhuyin").get(&0), Some(&"commitWithNewline"));
    // A command whose feature is off has no conflicts.
    settings.set("feature.typeZhuyin", Value::Bool(false)).unwrap();
    assert!(shortcuts.conflicts().is_empty());
    assert_eq!(resolve(&shortcuts, code::RETURN, modifier::SHIFT), Some("commitWithNewline"));
}

#[test]
fn punctuation_commands_yield_to_user_keys() {
    let (settings, shortcuts) = fresh();
    settings.set(STORAGE_KEY, json!({"insertSpace": [{"key": code::SPACE, "modifiers": modifier::SHIFT}]})).unwrap();
    assert!(shortcuts.bindings("togglePunctuationWidth").is_empty());
    assert!(shortcuts.conflicts().is_empty());
    assert_eq!(resolve(&shortcuts, code::SPACE, modifier::SHIFT), Some("insertSpace"));
}

#[test]
fn limits_and_required_commands() {
    let (_, shortcuts) = fresh();
    for key in 'A'..'E' {
        shortcuts.record("typeZhuyin", key as i32, FREE, None).unwrap();
    }
    assert_eq!(shortcuts.bindings("typeZhuyin").len(), 5);
    assert!(shortcuts.record("typeZhuyin", 'F' as i32, FREE, None).is_err());
    assert_eq!(shortcuts.remove("commitWithNewline", 0).unwrap_err().to_string(), "「送出並換行」至少要有一個按鍵");
    shortcuts.record("commitWithNewline", 'G' as i32, FREE, None).unwrap();
    shortcuts.remove("commitWithNewline", 0).unwrap();
    assert!(shortcuts.remove("commitWithNewline", 0).is_err());
    assert!(shortcuts.remove("unknown", 0).is_err());
    assert!(shortcuts.remove("typeZhuyin", 5).is_err());
    // A command that is not required may be left with no key.
    shortcuts.remove("singleCharacterList", 0).unwrap();
    assert_eq!(shortcuts.active_label("singleCharacterList"), "");
}

#[test]
fn corrupt_storage_is_preserved() {
    let (settings, shortcuts) = fresh();
    for stored in [json!("not-json"), json!({"unknown": []}), json!({"typeZhuyin": [{"key": 65.5, "modifiers": 0}]})] {
        settings.set(STORAGE_KEY, stored.clone()).unwrap();
        assert!(shortcuts.error().is_some(), "{stored}");
        assert_eq!(resolve(&shortcuts, code::TAB, 0), Some("nextPage"));
        assert_eq!(resolve(&shortcuts, code::QUOTE_LEFT, 0), Some("typeZhuyin"));
        assert_eq!(settings.get(STORAGE_KEY), stored);
    }
    settings.set(STORAGE_KEY, json!({})).unwrap();
    assert!(shortcuts.error().is_none());
}

#[test]
fn shifted_punctuation_canonicalizes() {
    let (_, shortcuts) = fresh();
    shortcuts.record("typeZhuyin", '!' as i32, modifier::CONTROL, Some(0)).unwrap();
    assert_eq!(resolve(&shortcuts, '1' as i32, modifier::CONTROL | modifier::SHIFT), Some("typeZhuyin"));
    assert!(shortcuts.record("selectLeft", '1' as i32, modifier::CONTROL | modifier::SHIFT, None).is_err());
}

#[test]
fn failed_write_does_not_publish() {
    let dir = TempDir::new();
    std::fs::write(dir.join("file"), "").unwrap();
    let settings = Settings::open(Some(dir.join("file").join("settings.json")));
    let shortcuts = Shortcuts::new(settings.clone());
    let before = shortcuts.all();
    let revision = settings.revision();
    assert!(shortcuts.record("typeZhuyin", 'U' as i32, FREE, None).is_err());
    assert_eq!(shortcuts.all(), before);
    assert_eq!(settings.revision(), revision);
    assert!(shortcuts.error().is_some());
}

//! 符號與 Emoji in the input controller and the broker: the shortcut opens the picker, every key goes to it, the
//! symbol is sent like a picked candidate and remembered, and the native adapters see it as a candidate list.

mod common;

use std::sync::Arc;

use common::Fixture;
use serde_json::Value;
use zt_core::keys::{code, modifier};
use zt_core::symbols::{Category, Item, Source};

const PICKER: (i32, u32) = (',' as i32, modifier::CONTROL | modifier::ALT);

/// A small catalog: 常用標點 (，。、) and 箭頭 (← →); 「ㄒㄧㄠ」 finds 😀.
struct Catalog;

impl Source for Catalog {
    fn categories(&self) -> Arc<Vec<Category>> {
        let category = |id: &str, title: &str, texts: &[&str]| Category {
            id: id.into(),
            title: title.into(),
            items: texts.iter().map(|t| Item { text: (*t).into(), name: String::new() }).collect(),
        };
        Arc::new(vec![
            category("punctuation", "常用標點", &["，", "。", "、"]),
            category("arrows", "箭頭", &["←", "→"]),
        ])
    }
    fn search(&self, query: &str, _limit: usize) -> Vec<Item> {
        if query == "vul" || query == "你" {
            vec![Item { text: "😀".into(), name: "笑臉".into() }]
        } else {
            Vec::new()
        }
    }
    fn display_query(&self, query: &str) -> String {
        query.to_owned()
    }
    fn name_of(&self, _text: &str) -> Option<String> {
        None
    }
    fn recent_from_text(&self, text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }
    fn recent_pushed(&self, recent: &[String], symbol: &str) -> (Vec<String>, String) {
        let mut list = vec![symbol.to_owned()];
        list.extend(recent.iter().filter(|s| *s != symbol).cloned());
        let text = list.join("\n");
        (list, text)
    }
}

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.input.set_symbol_source(Arc::new(Catalog));
    let dir = f.dir.path().join("user");
    f.input.set_recent_symbols_dir(Some(&dir));
    f
}

#[test]
fn shortcut_opens_keys_route_and_digits_insert() {
    let mut f = fixture();
    assert!(f.press(PICKER.0, PICKER.1));
    assert!(f.input.picker_open());
    let list = f.input.presentation().clone();
    assert_eq!(list.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["，", "。", "、"]);
    // Zhuyin keys go to the search, not the composer.
    assert!(f.press('V' as i32, 0));
    assert!(!f.input.session().composing());
    assert!(f.press(code::ESCAPE, 0), "Esc clears the search");
    assert!(f.input.picker_open());
    assert!(f.press(code::TAB, 0));
    assert!(f.press('2' as i32, 0));
    assert!(!f.input.picker_open());
    assert_eq!(f.commits(), ["→"]);
    assert_eq!(f.input.recent_symbols(), ["→"]);
    // Recent comes first next time.
    f.press(PICKER.0, PICKER.1);
    assert_eq!(f.input.presentation().rows[0].text, "→");
    assert!(f.press(PICKER.0, PICKER.1), "the shortcut again closes");
    assert!(!f.input.picker_open());
    assert!(f.commits().is_empty());
}

#[test]
fn composition_prefills_and_is_dropped() {
    let mut f = fixture();
    f.type_keys("su3");
    assert_eq!(f.composition(), "你");
    f.press(PICKER.0, PICKER.1);
    assert!(!f.input.session().composing(), "the composition is cancelled, not sent");
    assert_eq!(f.input.presentation().rows[0].text, "😀");
    assert!(f.press(code::RETURN, 0));
    assert_eq!(f.commits(), ["😀"]);
}

#[test]
fn esc_closes_and_the_switch_turns_it_off() {
    let mut f = fixture();
    f.press(PICKER.0, PICKER.1);
    assert!(f.press(code::ESCAPE, 0));
    assert!(!f.input.picker_open());
    assert!(f.commits().is_empty());
    f.settings.set("feature.symbolPicker", Value::Bool(false)).unwrap();
    assert!(!f.press(PICKER.0, PICKER.1), "off: the key is the app's");
    assert!(!f.input.picker_open());
}

#[test]
fn clicks_pick_and_recent_file_is_cleared() {
    let mut f = fixture();
    f.press(PICKER.0, PICKER.1);
    assert!(f.input.choose(1), "a click on the native list");
    assert_eq!(f.commits(), ["。"]);
    let file = f.dir.path().join("user").join(zt_core::symbols::RECENT_FILE);
    assert!(file.exists());
    f.input.clear_recent_symbols();
    assert!(!file.exists());
    assert!(f.input.recent_symbols().is_empty());
}

#[test]
fn english_mode_and_focus_loss() {
    let mut f = fixture();
    f.input.set_english(true);
    assert!(f.press(PICKER.0, PICKER.1), "opens in English too");
    assert!(f.press('A' as i32, 0), "keys go to the picker");
    f.input.deactivate();
    assert!(!f.input.picker_open());
    assert!(f.commits().is_empty());
}

#[test]
fn the_broker_claims_the_shortcut_and_keeps_keys_while_open() {
    use serde_json::json;
    use zt_core::broker::host::Host;
    let mut f = fixture();
    let mut host = Host::default();
    let mut request = |host: &mut Host, f: &mut Fixture, fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        host.dispatch(&mut f.input, 1, object.as_object().unwrap()).0
    };
    let reply = request(&mut host, &mut f, json!({"op": "focus", "panel": true}));
    assert_eq!(reply["claims"], json!([[',' as i32, modifier::CONTROL | modifier::ALT]]));
    let key = |code: i32, modifiers: u32| json!({"op": "key", "key": code, "modifiers": modifiers, "release": false});
    let reply = request(&mut host, &mut f, key(PICKER.0, PICKER.1));
    assert_eq!(reply["handled"], json!(true));
    assert_eq!(reply["composing"], json!(true), "the adapter sends every key while the picker is open");
    assert_eq!(reply["candidates"], json!(true));
    assert_eq!(reply["preedit"], json!(""));
    let reply = request(&mut host, &mut f, key('1' as i32, 0));
    assert_eq!(reply["commit"], json!("，"));
    assert_eq!(reply["composing"], json!(false));
    f.settings.set("feature.symbolPicker", Value::Bool(false)).unwrap();
    let reply = request(&mut host, &mut f, key('J' as i32, 0));
    assert_eq!(reply["claims"], json!([]));
}

//! 符號與 Emoji in the input controller and the broker: the shortcut opens the picker, every key goes to it, the
//! symbol is sent like a picked candidate and remembered, and the native adapters see it as a candidate list.

mod common;

use std::sync::Arc;

use common::Fixture;
use serde_json::Value;
use zt_core::keys::{code, modifier};
use zt_core::symbols::{Category, Item, Source};

const PICKER: (i32, u32) = ('/' as i32, modifier::CONTROL | modifier::SHIFT);

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
    assert!(f.press(code::BACKSPACE, 0), "⌫ edits the search");
    assert!(f.input.picker_open());
    assert!(f.press(code::TAB, 0));
    assert!(f.press('2' as i32, 0), "while the search has the keys a digit types there (ㄉ)");
    assert!(f.input.picker_open());
    assert!(f.commits().is_empty());
    assert!(f.press(code::BACKSPACE, 0));
    assert!(f.press(code::DOWN, 0), "↓ moves into the grid");
    assert!(f.press('2' as i32, 0), "now 2 picks");
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
fn the_broker_keeps_keys_while_open() {
    use serde_json::json;
    use zt_core::broker::host::Host;
    let mut f = fixture();
    let mut host = Host::default();
    let request = |host: &mut Host, f: &mut Fixture, fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        host.dispatch(&mut f.input, 1, object.as_object().unwrap()).0
    };
    let reply = request(&mut host, &mut f, json!({"op": "focus", "panel": true}));
    assert_eq!(reply["ok"], json!(true));
    let key = |code: i32, modifiers: u32| json!({"op": "key", "key": code, "modifiers": modifiers, "release": false});
    let reply = request(&mut host, &mut f, key(PICKER.0, PICKER.1));
    assert_eq!(reply["handled"], json!(true));
    assert_eq!(reply["composing"], json!(true), "the adapter sends every key while the picker is open");
    assert_eq!(reply["candidates"], json!(true));
    assert_eq!(reply["preedit"], json!(""));
    let reply = request(&mut host, &mut f, key('1' as i32, 0));
    assert_eq!((&reply["commit"], &reply["composing"]), (&json!(""), &json!(true)), "1 types ㄅ in the search");
    request(&mut host, &mut f, key(code::BACKSPACE, 0));
    request(&mut host, &mut f, key(code::DOWN, 0));
    let reply = request(&mut host, &mut f, key('1' as i32, 0));
    assert_eq!(reply["commit"], json!("，"));
    assert_eq!(reply["composing"], json!(false));
}

#[test]
fn adapters_that_draw_the_window_get_the_row_across() {
    use serde_json::json;
    use zt_core::service::{Service, ServiceConfig};
    let dir = common::TempDir::new();
    common::write_lexicon(dir.path(), common::CONTROLLER_LEXICON);
    let config =
        ServiceConfig { data_dir: Some(dir.path().to_path_buf()), in_memory: true, ..ServiceConfig::default() };
    let (service, _events) = Service::start(config).unwrap();
    service.with_controller(|input| input.set_symbol_source(Arc::new(Catalog)));
    let request = |fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        service.request(1, object.as_object().unwrap())
    };
    request(json!({"op": "focus", "panel": true}));
    let reply = request(json!({"op": "key", "key": PICKER.0, "modifiers": PICKER.1, "release": false}));
    assert_eq!(reply["panel"]["style"]["layout"], json!("horizontal"));
    assert_eq!(reply["panel"]["model"]["rows"][0]["text"], json!("，"));
}

#[test]
fn adapters_that_draw_the_window_get_the_grid_and_click_its_cells_and_tabs() {
    use serde_json::json;
    use zt_core::service::{Service, ServiceConfig};
    let dir = common::TempDir::new();
    common::write_lexicon(dir.path(), common::CONTROLLER_LEXICON);
    let config =
        ServiceConfig { data_dir: Some(dir.path().to_path_buf()), in_memory: true, ..ServiceConfig::default() };
    let (service, _events) = Service::start(config).unwrap();
    service.with_controller(|input| input.set_symbol_source(Arc::new(Catalog)));
    let request = |fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        service.request(1, object.as_object().unwrap())
    };
    let reply = request(json!({"op": "focus", "panel": true}));
    assert!(reply["panel"].is_null());
    let reply = request(json!({"op": "key", "key": PICKER.0, "modifiers": PICKER.1, "release": false}));
    let picker = &reply["panel"]["picker"];
    assert_eq!(picker["tabs"][1]["title"], json!("箭頭"));
    assert_eq!(picker["tab"], json!(0));
    assert_eq!(picker["items"][2]["text"], json!("、"));
    // A click on the 箭頭 tab, then on its second cell.
    let reply = request(json!({"op": "action", "action": "symbolTab", "value": 1}));
    assert_eq!(reply["panel"]["picker"]["tab"], json!(1));
    assert_eq!(reply["panel"]["picker"]["items"][0]["text"], json!("←"));
    let reply = request(json!({"op": "action", "action": "symbol", "value": 1}));
    assert_eq!(reply["commit"], json!("→"));
    assert!(reply["panel"].is_null(), "the picker closed");
    let reply = request(json!({"op": "action", "action": "symbol", "value": -1}));
    assert_eq!(reply["ok"], json!(false));
}

/// A broker request for the field `{A}` of connection 1.
fn host_request(
    host: &mut zt_core::broker::host::Host,
    f: &mut Fixture,
    fields: Value,
) -> serde_json::Map<String, Value> {
    let mut object = serde_json::json!({"v": 1, "context": "{A}", "secure": false});
    for (key, value) in fields.as_object().unwrap() {
        object[key] = value.clone();
    }
    host.dispatch(&mut f.input, 1, object.as_object().unwrap()).0
}

fn key_fields(code: i32, modifiers: u32, release: bool) -> Value {
    serde_json::json!({"op": "key", "key": code, "modifiers": modifiers, "release": release})
}

/// The adapters that pass on only the keys they predict the service takes (TSF) decide on `composing`: every reply
/// while the picker is open says so, for every key of the search and the grid, presses and releases, in either mode
/// and in a private field, so none of them goes to the application.
#[test]
fn every_reply_while_open_keeps_the_keys_with_the_picker() {
    use serde_json::json;
    use zt_core::broker::host::Host;
    let keys = [
        // Zhuyin on digit keys (ㄞˋ) types in the search.
        ('9' as i32, 0),
        ('4' as i32, 0),
        (code::BACKSPACE, 0),
        (code::BACKSPACE, 0),
        ('V' as i32, 0),
        ('U' as i32, 0),
        ('L' as i32, 0),
        (code::SPACE, 0),
        (code::BACKSPACE, 0),
        (code::LEFT, 0),
        (code::DOWN, 0),
        // In the grid: a digit with no item does nothing, ↑ on the first row goes back to the search.
        ('9' as i32, 0),
        (code::UP, 0),
        (code::TAB, 0),
        (code::TAB, modifier::SHIFT),
        ('H' as i32, 0),
        ('E' as i32, modifier::SHIFT),
    ];
    for (english, private) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut f = fixture();
        let mut host = Host::default();
        let focus = host_request(&mut host, &mut f, json!({"op": "focus", "panel": true, "private": private}));
        assert_eq!(focus["ok"], json!(true));
        if english {
            f.input.set_english(true);
        }
        let open = host_request(&mut host, &mut f, key_fields(PICKER.0, PICKER.1, false));
        assert_eq!((&open["handled"], &open["composing"]), (&json!(true), &json!(true)), "english={english}");
        for (code, modifiers) in keys {
            for release in [false, true] {
                let reply = host_request(&mut host, &mut f, key_fields(code, modifiers, release));
                assert_eq!(reply["ok"], json!(true));
                assert_eq!(reply["composing"], json!(true), "{code:#x} release={release} english={english}");
                assert_eq!(reply["candidates"], json!(true));
                assert_eq!(reply["preedit"], json!(""));
                assert_eq!(reply["commit"], json!(""));
                assert!(reply.get("associated").is_none());
                if !release {
                    assert_eq!(reply["handled"], json!(true), "{code:#x} english={english}");
                }
            }
            assert!(f.input.picker_open(), "{code:#x}");
        }
        assert_eq!(f.input.english(), english, "a Shift in the search does not switch 中/英");
    }
}

/// English letters and Zhuyin keys reach the search the same way: typed into the picker, shown as the query.
#[test]
fn the_search_reads_keys_in_either_mode() {
    for english in [false, true] {
        let mut f = fixture();
        f.input.set_english(english);
        assert!(f.press(PICKER.0, PICKER.1));
        for c in ['V', 'U', 'L'] {
            assert!(f.press(c as i32, 0), "{c} english={english}");
        }
        let view = f.input.picker_view().unwrap();
        assert_eq!(view.query, "vul");
        assert_eq!(view.items[0].text, "😀");
        assert!(f.press(code::RETURN, 0));
        assert_eq!(f.commits(), ["😀"]);
        assert!(!f.input.session().composing() && !f.input.picker_open());
    }
}

/// A reset (the application ended the composition, or the field was reset) closes the picker; its reply hides the
/// window and gives the keys back, and an adapter that draws the window must apply it (TSF did not: the grid stayed
/// on screen with no picker behind it).
#[test]
fn a_reset_closes_the_picker_and_its_window() {
    use serde_json::json;
    use zt_core::service::{Service, ServiceConfig};
    let dir = common::TempDir::new();
    common::write_lexicon(dir.path(), common::CONTROLLER_LEXICON);
    let config =
        ServiceConfig { data_dir: Some(dir.path().to_path_buf()), in_memory: true, ..ServiceConfig::default() };
    let (service, _events) = Service::start(config).unwrap();
    service.with_controller(|input| input.set_symbol_source(Arc::new(Catalog)));
    let request = |fields: Value| {
        let mut object = json!({"v": 1, "context": "{A}", "secure": false});
        for (key, value) in fields.as_object().unwrap() {
            object[key] = value.clone();
        }
        service.request(1, object.as_object().unwrap())
    };
    request(json!({"op": "focus", "panel": true}));
    let open = request(key_fields(PICKER.0, PICKER.1, false));
    assert!(open["panel"]["picker"].is_object());
    let typed = request(key_fields('V' as i32, 0, false));
    assert_eq!(typed["panel"]["picker"]["query"], json!("v"), "the search follows each key");
    let reset = request(json!({"op": "reset"}));
    assert!(reset["panel"].is_null(), "the window closes with the picker");
    assert_eq!(reset["composing"], json!(false));
    let after = request(key_fields('V' as i32, 0, false));
    assert_ne!(after["preedit"], json!(""), "the next key types again");
    assert!(after.get("panel").is_none_or(|panel| panel.get("picker").is_none_or(Value::is_null)));
}

/// The 0.7.1 failure: in Chinese mode with a short 最近使用, ㄞˋ (9 4) and ㄅ (1) did nothing or picked a symbol.
/// Now they type into the search, and ↓ 2 inserts the second match.
#[test]
fn zhuyin_on_digit_keys_types_into_the_search() {
    struct Hearts;
    impl Source for Hearts {
        fn categories(&self) -> Arc<Vec<Category>> {
            Catalog.categories()
        }
        fn search(&self, query: &str, _limit: usize) -> Vec<Item> {
            let item = |text: &str| Item { text: text.into(), name: String::new() };
            match query {
                "94" | "9" => vec![item("❤️"), item("🩶")],
                "1" => vec![item("👍")],
                _ => Vec::new(),
            }
        }
        fn display_query(&self, query: &str) -> String {
            Catalog.display_query(query)
        }
        fn name_of(&self, text: &str) -> Option<String> {
            Catalog.name_of(text)
        }
        fn recent_from_text(&self, text: &str) -> Vec<String> {
            Catalog.recent_from_text(text)
        }
        fn recent_pushed(&self, recent: &[String], symbol: &str) -> (Vec<String>, String) {
            Catalog.recent_pushed(recent, symbol)
        }
    }
    let mut f = fixture();
    f.input.set_symbol_source(Arc::new(Hearts));
    // Two recent symbols, as in the report's screenshot.
    for _ in 0..2 {
        f.press(PICKER.0, PICKER.1);
        f.press(code::DOWN, 0);
        f.press('1' as i32, 0);
    }
    f.commits();
    assert!(f.press(PICKER.0, PICKER.1));
    assert!(f.press('9' as i32, 0) && f.press('4' as i32, 0));
    assert!(f.input.picker_open() && f.commits().is_empty(), "the digits typed, nothing was picked");
    let view = f.input.picker_view().unwrap();
    assert_eq!(view.query, "94");
    assert_eq!(view.items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["❤️", "🩶"]);
    assert!(!view.numbered);
    assert!(f.press(code::DOWN, 0) && f.press('2' as i32, 0));
    assert_eq!(f.commits(), ["🩶"]);
    assert!(f.press(PICKER.0, PICKER.1) && f.press('1' as i32, 0), "ㄅ");
    assert_eq!(f.input.picker_view().unwrap().items[0].text, "👍");
    assert!(f.press(code::RETURN, 0), "Return inserts the first match from the search");
    assert_eq!(f.commits(), ["👍"]);
}

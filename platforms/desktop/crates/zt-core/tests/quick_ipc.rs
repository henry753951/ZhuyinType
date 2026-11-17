//! `op: "quick"` (快速設定) over the broker's real Unix socket, as the Fcitx5 addon and the Windows text service
//! send it, and the `quickPanel` reply of the 開啟快速設定 shortcut.
#![cfg(unix)]

mod common;

use common::{LEXICON, TempDir, write_lexicon};
use serde_json::{Value, json};
use zt_core::broker::Client;
use zt_core::keys::{code, modifier};
use zt_core::service::{Service, ServiceConfig, UiEvent};

struct Fixture {
    _dir: TempDir,
    service: Service,
    events: async_channel::Receiver<UiEvent>,
    client: Client,
}

impl Fixture {
    fn new() -> Self {
        let dir = TempDir::new();
        write_lexicon(dir.path(), &format!("{LEXICON}ㄏㄨㄚˋ\t話\t-3\nㄏㄨㄚˋ\t畫\t-4\n"));
        let config = ServiceConfig {
            data_dir: Some(dir.path().to_path_buf()),
            in_memory: true,
            endpoint: Some(dir.join("run").join("ime").to_string_lossy().into_owned()),
            broker: true,
            app_version: "test".into(),
            ..ServiceConfig::default()
        };
        let (service, events) = Service::start(config).unwrap();
        let client = Client::connect(&service.endpoint().unwrap()).unwrap();
        Self { _dir: dir, service, events, client }
    }

    fn request(&mut self, op: &str, fields: Value) -> Value {
        self.client.request(op, fields).unwrap()
    }

    fn key(&mut self, key: i32, modifiers: u32) -> Value {
        self.request("key", json!({"secure": false, "key": key, "modifiers": modifiers, "release": false}))
    }

    fn keys(&mut self, keys: &str) -> Value {
        let mut reply = Value::Null;
        for c in keys.chars() {
            reply = self.key(c.to_ascii_uppercase() as i32, 0);
        }
        reply
    }

    fn ui_events(&self) -> Vec<UiEvent> {
        std::iter::from_fn(|| self.events.try_recv().ok()).collect()
    }
}

#[test]
fn quick_reads_and_sets_every_switch() {
    let mut f = Fixture::new();
    let focus = f.request("focus", json!({"secure": false}));
    assert_eq!((&focus["simplified"], &focus["learningPaused"]), (&json!(false), &json!(false)));
    let read = f.request("quick", json!({}));
    assert_eq!(read["ok"], true);
    assert_eq!(
        read["quick"],
        json!({"english": false, "fullWidth": true, "simplified": false, "learningPaused": false, "symbols": false})
    );
    let set =
        f.request("quick", json!({"english": true, "fullWidth": false, "simplified": true, "learningPaused": true}));
    assert_eq!(
        set["quick"],
        json!({"english": true, "fullWidth": false, "simplified": true, "learningPaused": true, "symbols": false})
    );
    assert_eq!(
        (&set["english"], &set["simplified"], &set["learningPaused"]),
        (&json!(true), &json!(true), &json!(true))
    );
    assert_eq!(f.service.settings().string("outputScript"), "simplified", "繁／簡 is the setting");
    // Only the switches present change.
    let one = f.request("quick", json!({"english": false}));
    assert_eq!(
        one["quick"],
        json!({"english": false, "fullWidth": false, "simplified": true, "learningPaused": true, "symbols": false})
    );
    // Every reply carries the badge state.
    let poll = f.request("poll", json!({}));
    assert_eq!(
        (&poll["simplified"], &poll["learningPaused"], &poll["fullWidth"]),
        (&json!(true), &json!(true), &json!(false))
    );
    assert!(f.service.with_controller(|c| c.unlearned()));
    // Bad values are refused, and nothing changes.
    let bad = f.request("quick", json!({"learningPaused": "no"}));
    assert_eq!(bad["ok"], false);
    let back = f.request("quick", json!({"simplified": false, "learningPaused": false}));
    assert_eq!(back["quick"]["simplified"], false);
    assert_eq!(back["quick"]["learningPaused"], false);
    assert_eq!(f.service.settings().string("outputScript"), "traditional");
}

#[test]
fn quick_english_waits_for_the_composition() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false}));
    f.keys("su3");
    let reply = f.request("quick", json!({"english": true}));
    assert_eq!(reply["quick"]["english"], false, "not while composing, as the mode op");
    // A secure field: 中／英 stays.
    f.request("secure", json!({"secure": true}));
    let reply = f.request("quick", json!({"english": true}));
    assert_eq!(reply["quick"]["english"], false);
}

#[test]
fn simplified_output_reaches_the_reply_and_the_window() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false}));
    f.request("quick", json!({"simplified": true}));
    let typed = f.keys("cj84");
    assert_eq!(typed["preedit"], "话");
    f.key(code::DOWN, 0);
    let popup = f.service.popup().expect("the list");
    let rows: Vec<&str> = popup.presentation.rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(rows, ["话", "画"]);
    // Return picks, and with the list closed sends.
    let picked = f.key(code::RETURN, 0);
    let sent = if picked["commit"] == "" { f.key(code::RETURN, 0) } else { picked };
    assert_eq!(sent["commit"], "话");
    // Learning stays traditional.
    assert!(f.service.learned_entries().iter().all(|phrase| !phrase.word.contains('话')));
    // An adapter that draws the window gets it converted too.
    f.request("focus", json!({"secure": false, "panel": true}));
    f.keys("cj84");
    let reply = f.key(code::DOWN, 0);
    let panel = serde_json::to_string(&reply["panel"]).unwrap();
    assert!(panel.contains("话") && panel.contains("画") && !panel.contains('話'), "{panel}");
    // The 中 badge reads 简.
    f.key(code::ESCAPE, 0);
    f.key(code::ESCAPE, 0);
    f.request("focus", json!({"secure": false}));
    f.ui_events();
    f.request("quick", json!({"english": true}));
    f.request("quick", json!({"english": false}));
    let toasts: Vec<&'static str> = f
        .ui_events()
        .into_iter()
        .filter_map(|e| match e {
            UiEvent::Toast { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(toasts, ["英", "简"]);
}

#[test]
fn the_shortcut_asks_for_the_panel() {
    let mut f = Fixture::new();
    f.service.shortcuts().record("quickSettings", 'Q' as i32, modifier::ALT, None).unwrap();
    f.request("focus", json!({"secure": false, "rect": [100, 200, 2, 20]}));
    f.ui_events();
    let reply = f.key('Q' as i32, modifier::ALT);
    assert_eq!((&reply["handled"], &reply["quickPanel"]), (&json!(true), &json!(true)));
    let panels: Vec<UiEvent> = f.ui_events().into_iter().filter(|e| matches!(e, UiEvent::QuickPanel { .. })).collect();
    assert_eq!(panels.len(), 1, "the app shows its popup");
    let UiEvent::QuickPanel { anchor } = &panels[0] else { unreachable!() };
    assert_eq!((anchor.x, anchor.y), (100.0, 200.0));
    // An adapter that draws its own windows shows its own menu: no UI event.
    f.request("focus", json!({"secure": false, "panel": true}));
    f.ui_events();
    let reply = f.key('Q' as i32, modifier::ALT);
    assert_eq!(reply["quickPanel"], true);
    assert!(!f.ui_events().iter().any(|e| matches!(e, UiEvent::QuickPanel { .. })));
    // Other replies do not carry it.
    assert!(f.request("poll", json!({})).get("quickPanel").is_none());
}

#[test]
fn a_new_service_learns() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false}));
    f.request("quick", json!({"learningPaused": true}));
    let again = Fixture::new();
    assert!(!again.service.with_controller(|c| c.learning_paused()), "every start learns");
}

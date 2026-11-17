//! Port of the Qt `ImeHostTest`: the service with its broker on a real Unix socket, driven by a client the way the
//! Fcitx5 addon drives it.
#![cfg(unix)]

mod common;

use common::{LEXICON, TempDir, write_lexicon};
use serde_json::{Value, json};
use zt_core::broker::{self, Client, ReserveError};
use zt_core::keys::{code, modifier};
use zt_core::service::{PopupAction, Service, ServiceConfig, UiEvent};

struct Fixture {
    dir: TempDir,
    service: Service,
    events: async_channel::Receiver<UiEvent>,
    client: Client,
}

fn config(dir: &TempDir) -> ServiceConfig {
    ServiceConfig {
        data_dir: Some(dir.path().to_path_buf()),
        in_memory: true,
        endpoint: Some(dir.join("run").join("ime").to_string_lossy().into_owned()),
        broker: true,
        app_version: "test".into(),
        ..ServiceConfig::default()
    }
}

impl Fixture {
    fn new() -> Self {
        Self::with_lexicon(LEXICON)
    }

    fn with_lexicon(lexicon: &str) -> Self {
        let dir = TempDir::new();
        write_lexicon(dir.path(), lexicon);
        let (service, events) = Service::start(config(&dir)).unwrap();
        assert!(service.engine_error().is_none());
        let client = Client::connect(&service.endpoint().unwrap()).unwrap();
        Self { dir, service, events, client }
    }

    fn request(&mut self, op: &str, fields: Value) -> Value {
        self.client.request(op, fields).unwrap()
    }

    fn key(&mut self, key: i32) -> Value {
        self.request("key", json!({"secure": false, "key": key, "modifiers": 0, "release": false}))
    }

    fn keys(&mut self, keys: &str) {
        for c in keys.chars() {
            self.key(c.to_ascii_uppercase() as i32);
        }
    }

    fn composition(&self) -> String {
        self.service.presentation().composition
    }

    fn visible(&self) -> bool {
        self.service.popup().is_some()
    }

    fn ui_events(&self) -> Vec<UiEvent> {
        std::iter::from_fn(|| self.events.try_recv().ok()).collect()
    }
}

fn ok(reply: &Value) -> bool {
    reply["ok"] == true
}

#[test]
fn delivery_secure_and_exactly_once() {
    let mut f = Fixture::new();
    assert!(ok(&f.request("focus", json!({"secure": false}))));
    f.keys("su");
    let reply = f.key('3' as i32);
    assert_eq!(reply["preedit"], "你");
    assert_eq!(reply["cursor"], 1);
    assert_eq!(reply["composing"], true);
    assert_eq!(f.key(code::DOWN)["candidates"], true);
    assert!(f.visible());
    assert!(f.ui_events().iter().any(|e| matches!(e, UiEvent::Popup(Some(_)))));
    f.service.popup_action(PopupAction::Pick(1));
    let picked = f.request("poll", json!({}));
    // A pick edits the composer; Return then delivers it once to the adapter.
    assert!(ok(&picked));
    let commit = f.key(code::RETURN);
    assert_eq!(format!("{}{}", picked["commit"].as_str().unwrap(), commit["commit"].as_str().unwrap()), "妳");
    assert_eq!(f.request("poll", json!({}))["commit"], "");
    f.keys("su3");
    let secure = f.request("secure", json!({"secure": true}));
    assert_eq!(secure["preedit"], "");
    assert_eq!(secure["commit"], "");
    assert!(!f.visible());
    let secure_key = f.request("key", json!({"secure": true, "key": 'S' as i32, "modifiers": 0, "release": false}));
    assert_eq!(secure_key["handled"], false);
    assert_eq!(secure_key["preedit"], "");
    assert_eq!(f.request("blur", json!({}))["commit"], "");
}

#[test]
fn focus_isolation_and_validation() {
    let mut f = Fixture::new();
    assert!(!ok(&f.key('S' as i32)));
    f.request("focus", json!({"secure": false, "rect": [20, 30, 1, 18]}));
    f.keys("su3");
    let invalid = f.request("key", json!({"secure": false, "key": 3.5, "modifiers": 0, "release": false}));
    assert!(!ok(&invalid));
    assert_eq!(f.composition(), "你");
    for field in ["secure", "repeat", "capsLock", "inlinePreedit"] {
        let mut malformed = json!({"secure": false});
        malformed[field] = json!("false");
        assert!(!ok(&f.request("focus", malformed)), "{field}");
        assert_eq!(f.composition(), "你");
    }
    assert!(!ok(&f.request("poll", json!({"rect": [20, 30, 1, 18], "scale": "1"}))));
    assert!(!ok(&f.request("poll", json!({"rect": [20, 30, -1, 18]}))));
    assert!(!ok(&f.request("bogus", json!({}))));
    let stale = f.request("reset", json!({"context": "other"}));
    assert!(!ok(&stale));
    assert_eq!(stale["context"], "other", "the reply names the request's context");
    assert_eq!(f.composition(), "你");
    assert_eq!(f.request("blur", json!({}))["commit"], "你");
    assert!(!ok(&f.request("poll", json!({}))));
    f.request("focus", json!({"secure": false, "context": "other"}));
    assert!(f.composition().is_empty());
    f.keys("su3");
    drop(f.client);
    // The broker notices the closed connection and drops its field.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !f.service.presentation().composition.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(f.service.presentation().composition.is_empty());
    assert!(f.service.popup().is_none());
}

#[test]
fn initial_caps_lock_and_live_settings() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false, "capsLock": true}));
    assert_eq!(f.key('S' as i32)["handled"], false);
    assert!(f.composition().is_empty());
    f.key(code::CAPS_LOCK);
    f.keys("su3");
    f.service.settings().set("candidatesPerPage", json!(3)).unwrap();
    f.key(code::DOWN);
    let popup = f.service.popup().unwrap();
    assert_eq!(popup.presentation.page_size, 3);
    assert_eq!(popup.presentation.rows.len(), 3);
    f.service.popup_action(PopupAction::TurnPage(1));
    let popup = f.service.popup().unwrap();
    assert_eq!(popup.presentation.page, 1);
    assert!(!popup.presentation.rows.is_empty());
    f.service.popup_action(PopupAction::TurnPage(1));
    assert!(f.ui_events().contains(&UiEvent::Boundary(1)), "the last page stops");
    // Secure transition must also discard a mouse action pending its next poll.
    f.service.popup_action(PopupAction::Pick(0));
    f.request("secure", json!({"secure": true}));
    assert_eq!(f.request("poll", json!({}))["commit"], "");
}

#[test]
fn composition_fallback_does_not_require_candidate_list() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false, "inlinePreedit": false}));
    f.keys("su3");
    let popup = f.service.popup().unwrap();
    assert!(popup.presentation.rows.is_empty());
    assert!(!popup.inline_preedit);
    assert_eq!(popup.presentation.composition, "你");
    f.request("secure", json!({"secure": true}));
    assert!(!f.visible());
}

#[test]
fn preceding_text_mode_toast_and_caret() {
    let mut f = Fixture::new();
    let reply = f.request("focus", json!({"secure": false, "app": "org.example.editor", "before": "我說"}));
    assert!(ok(&reply));
    f.keys("su3");
    assert_eq!(f.service.with_controller(|c| c.recent_text().to_owned()), "我說");
    assert!(ok(&f.request("context", json!({"before": "前文"}))));
    assert!(!ok(&f.request("context", json!({"before": 3}))));
    assert!(!ok(&f.request("focus", json!({"secure": false, "app": "a".repeat(300)}))));
    f.request("focus", json!({"secure": false, "app": "org.example.editor"}));
    f.ui_events();
    // A Shift tap in the focused field asks for the 中／英 toast, by the caret.
    f.request("poll", json!({"rect": [600, 400, 2, 36], "scale": 2}));
    let shift = |release| json!({"secure": false, "key": code::SHIFT, "modifiers": if release { 0 } else { modifier::SHIFT }, "release": release});
    f.request("key", shift(false));
    let reply = f.request("key", shift(true));
    assert_eq!(reply["english"], true);
    let toasts: Vec<UiEvent> = f.ui_events().into_iter().filter(|e| matches!(e, UiEvent::Toast { .. })).collect();
    assert_eq!(toasts.len(), 1);
    let UiEvent::Toast { text, anchor } = &toasts[0] else { unreachable!() };
    assert_eq!(*text, "英");
    assert_eq!(
        (anchor.x, anchor.y, anchor.width, anchor.height),
        (300.0, 200.0, 1.0, 18.0),
        "scaled to logical pixels"
    );
    // 「中英提示」 off: no toast.
    f.service.settings().set("showModeToast", json!(false)).unwrap();
    f.request("key", shift(false));
    f.request("key", shift(true));
    assert!(!f.ui_events().iter().any(|e| matches!(e, UiEvent::Toast { .. })));
}

#[test]
fn listed_apps_show_composition_in_the_window() {
    let mut f = Fixture::new();
    f.service.set_popup_composition_apps(&["Notepad.exe".to_owned()]).unwrap();
    f.request("focus", json!({"secure": false, "app": "notepad.exe"}));
    f.keys("su");
    let reply = f.key('3' as i32);
    assert_eq!(reply["preedit"], "");
    assert_eq!(reply["cursor"], 0, "no cursor beyond an empty preedit");
    assert_eq!(reply["composing"], true);
    let popup = f.service.popup().unwrap();
    assert!(!popup.inline_preedit);
    assert_eq!(popup.presentation.composition, "你");
    f.request("blur", json!({}));
    f.request("focus", json!({"secure": false, "app": "editor"}));
    f.keys("su");
    assert_eq!(f.key('3' as i32)["preedit"], "你");
    assert!(!f.visible(), "an app that shows the composition itself needs no window");
}

#[test]
fn an_adapter_that_draws_the_window_gets_it_in_its_replies() {
    let mut f = Fixture::new();
    assert!(ok(&f.request("focus", json!({"secure": false, "panel": true}))));
    assert!(!ok(&f.request("focus", json!({"secure": false, "panel": "yes"}))));
    assert!(ok(&f.request("focus", json!({"secure": false, "panel": true}))));
    f.ui_events();
    f.keys("su3cl3");
    let reply = f.key(code::DOWN);
    assert_eq!(reply["candidates"], true);
    let panel = &reply["panel"];
    assert_eq!(panel["model"]["rows"][0]["text"], "你好");
    assert_eq!(panel["model"]["selected"], 0);
    assert_eq!(panel["style"]["layout"], "vertical");
    assert_eq!(panel["appearance"], "system");
    assert!(!f.visible(), "the app's own window stays hidden");
    assert!(!f.ui_events().iter().any(|e| matches!(e, UiEvent::Popup(Some(_)))));
    // Unchanged: left out; changed: sent again.
    assert!(f.request("poll", json!({})).get("panel").is_none());
    let moved = f.key(code::DOWN);
    assert_eq!(moved["panel"]["model"]["selected"], 1);
    // A click on the second row picks it.
    assert!(!ok(&f.request("action", json!({"action": "pick", "value": -1}))));
    assert!(!ok(&f.request("action", json!({"action": "nonsense"}))));
    let picked = f.request("action", json!({"action": "pick", "value": 0}));
    assert_eq!(picked["preedit"], "你好", "the clause becomes the picked candidate");
    assert_eq!(picked["panel"], Value::Null, "the window closes");
    // The 中/英 badge comes in the reply, not as an event.
    let shift = |release| json!({"secure": false, "key": code::SHIFT, "modifiers": if release { 0 } else { modifier::SHIFT }, "release": release});
    f.request("key", shift(false));
    let reply = f.request("key", shift(true));
    assert_eq!(reply["toast"], "英");
    assert!(!f.ui_events().iter().any(|e| matches!(e, UiEvent::Toast { .. })));
    // A switch outside a request, and after the blur, never makes the app draw a second badge.
    f.service.with_controller(|controller| controller.set_english(false));
    f.request("blur", json!({}));
    f.service.with_controller(|controller| controller.set_english(true));
    assert!(!f.ui_events().iter().any(|e| matches!(e, UiEvent::Toast { .. } | UiEvent::Boundary(_))));
}

#[test]
fn replies_mark_the_words_and_the_one_being_changed() {
    let mut f = Fixture::new();
    let focused = f.request("focus", json!({"secure": false}));
    assert!(focused.get("clauses").is_none(), "no composition, no marks");
    f.keys("su3cl");
    let typed = f.key('3' as i32);
    assert_eq!(typed["preedit"], "你好");
    assert_eq!(typed["clauses"], json!([[0, 2]]), "UTF-16 ranges of the words");
    assert_eq!((&typed["focus"], &typed["target"]), (&Value::Null, &Value::Null));
    let listed = f.key(code::DOWN);
    assert_eq!(listed["candidates"], true);
    assert_eq!(listed["focus"], 0, "the word the list replaces");
    let secure = f.request("key", json!({"secure": true}));
    assert!(secure.get("clauses").is_none(), "nothing about the text leaves a secure field");
}

#[test]
fn replies_mark_the_corrected_words() {
    let mut f = Fixture::with_lexicon(common::CORRECTION_LEXICON);
    f.request("focus", json!({"secure": false}));
    f.keys("18 wk");
    let typed = f.key('3' as i32);
    assert_eq!(typed["preedit"], "八乙");
    assert_eq!(typed["corrections"], json!([[1, 2]]), "UTF-16 ranges of the corrected words");
    f.service.settings().set("showCorrections", json!(false)).unwrap();
    let typed = f.key(code::LEFT);
    assert_eq!(typed["preedit"], "八乙");
    assert!(typed.get("corrections").is_none(), "「標示自動修正」 off");
}

#[test]
fn a_private_field_types_chinese_without_reading_or_learning() {
    let mut f = Fixture::new();
    assert!(ok(&f.request("focus", json!({"secure": false, "private": true, "app": "browser", "before": "前文"}))));
    assert!(f.service.with_controller(|controller| controller.private()));
    f.keys("su");
    let typed = f.key('3' as i32);
    assert_eq!(typed["preedit"], "你", "typing works in a private field");
    assert!(!ok(&f.request("focus", json!({"secure": false, "private": "yes"}))));
    assert!(ok(&f.request("focus", json!({"secure": false}))));
    assert!(!f.service.with_controller(|controller| controller.private()), "a new focus is not private");
}

#[test]
fn mode_switch_from_the_indicator() {
    let mut f = Fixture::new();
    assert!(ok(&f.request("focus", json!({"secure": false}))));
    assert!(!ok(&f.request("mode", json!({"english": "yes"}))));
    let reply = f.request("mode", json!({"english": true}));
    assert!(ok(&reply));
    assert_eq!(reply["english"], true);
    assert_eq!(f.key('S' as i32)["handled"], false);
    assert_eq!(f.request("mode", json!({"english": false}))["english"], false);
    f.key('S' as i32);
    assert_eq!(f.request("mode", json!({"english": true}))["english"], false, "not while composing");
    f.request("reset", json!({}));
    f.request("secure", json!({"secure": true}));
    assert_eq!(f.request("mode", json!({"english": true}))["english"], false, "not in a secure field");
}

/// The addon connects the moment the socket answers (sign-in, an update restarting the service) and types at once:
/// the endpoint is served only once the engine is open, so the very first keys compose and convert, and none goes
/// on to the application as a raw letter or as its Zhuyin.
#[test]
fn the_first_keys_after_start_are_converted() {
    let dir = TempDir::new();
    write_lexicon(dir.path(), LEXICON);
    let endpoint = config(&dir).endpoint.unwrap();
    let typist = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut client = loop {
            match Client::connect(&endpoint) {
                Ok(client) => break client,
                Err(_) if std::time::Instant::now() < deadline => std::thread::yield_now(),
                Err(error) => panic!("the endpoint was never served: {error}"),
            }
        };
        assert!(ok(&client.request("focus", json!({"secure": false})).unwrap()));
        let mut replies = Vec::new();
        for key in "SU3CL3".chars().map(|c| c as i32).chain([code::RETURN]) {
            let reply =
                client.request("key", json!({"secure": false, "key": key, "modifiers": 0, "release": false})).unwrap();
            replies.push(reply);
        }
        replies
    });
    let (service, _events) = Service::start(config(&dir)).unwrap();
    let replies = typist.join().unwrap();
    assert!(replies.iter().all(|reply| ok(reply) && reply["handled"] == true), "{replies:?}");
    let commits: String = replies.iter().map(|reply| reply["commit"].as_str().unwrap()).collect();
    assert_eq!(commits, "你好");
    drop(service);
}

#[test]
fn show_settings_and_single_instance() {
    let f = Fixture::new();
    let error = Service::start(config(&f.dir)).err().expect("a second instance");
    assert!(error.downcast_ref::<zt_core::service::AlreadyRunning>().is_some());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut shown = false;
    while !shown && std::time::Instant::now() < deadline {
        shown = f.ui_events().contains(&UiEvent::ShowSettings(None));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(shown, "the running instance was asked for its settings window");
}

#[test]
fn a_copy_started_after_a_crash_takes_over_once_the_old_one_is_gone() {
    // The UI thread panicked: the old copy is still ending when the new one starts.  The new one waits for the
    // endpoint instead of asking the dying copy for its settings window and quitting, which left no input method.
    let Fixture { dir, service, events, client } = Fixture::new();
    let mut waiting = config(&dir);
    waiting.wait_for_endpoint = Some(std::time::Duration::from_secs(5));
    let old = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop((client, events, service));
    });
    let (service, _events) = Service::start(waiting).expect("takes over the endpoint");
    old.join().unwrap();
    let mut client = Client::connect(&service.endpoint().unwrap()).unwrap();
    assert!(ok(&client.request("focus", json!({"secure": false})).unwrap()));

    // Nothing goes away: after the wait it is an ordinary second copy, and it does not open the settings window.
    let mut short = config(&dir);
    short.wait_for_endpoint = Some(std::time::Duration::from_millis(200));
    let error = Service::start(short).err().expect("a second instance");
    assert!(error.downcast_ref::<zt_core::service::AlreadyRunning>().is_some());
}

#[test]
fn a_quiet_second_instance_opens_no_window() {
    let f = Fixture::new();
    let error =
        Service::start(ServiceConfig { quiet_if_running: true, ..config(&f.dir) }).err().expect("a second instance");
    assert!(error.downcast_ref::<zt_core::service::AlreadyRunning>().is_some());
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(!f.ui_events().contains(&UiEvent::ShowSettings(None)), "zhuyintype --background never opens settings");
}

#[test]
fn refuses_occupied_file() {
    let dir = TempDir::new();
    let endpoint = dir.join("occupied");
    std::fs::write(&endpoint, "keep").unwrap();
    assert!(matches!(broker::reserve(endpoint.to_str().unwrap()), Err(ReserveError::Failed(_))));
    assert_eq!(std::fs::read_to_string(&endpoint).unwrap(), "keep");
}

#[test]
fn refuses_a_directory_others_can_enter() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new();
    let open = dir.join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(broker::reserve(open.join("ime").to_str().unwrap()), Err(ReserveError::Failed(_))));
}

#[test]
fn duplicate_sequence_and_oversized_frames_disconnect() {
    let mut f = Fixture::new();
    f.request("focus", json!({"secure": false}));
    f.client.send_raw(r#"{"v":1,"id":1,"context":"field","op":"poll"}"#).unwrap();
    assert!(f.client.read_reply().is_err(), "a repeated id closes the connection");
    let mut client = Client::connect(&f.service.endpoint().unwrap()).unwrap();
    client.send_raw(&"x".repeat(broker::MAX_FRAME + 10)).unwrap_or(());
    assert!(client.read_reply().is_err(), "a frame over 256 KiB closes the connection");
    let mut client = Client::connect(&f.service.endpoint().unwrap()).unwrap();
    client.send_raw("not json").unwrap();
    assert!(client.read_reply().is_err());
}

#[test]
fn word_actions_from_the_window_the_adapter_draws() {
    let mut f = Fixture::new();
    assert!(ok(&f.request("focus", json!({"secure": false, "panel": true}))));
    f.keys("su3cl3");
    let reply = f.key(code::DOWN);
    let row = &reply["panel"]["model"]["rows"][0];
    assert_eq!(row["text"], "你好");
    assert_eq!(row["actions"], 0b101, "不要再推薦 and 加入自訂詞; nothing learned to forget");
    assert!(!ok(&f.request("action", json!({"action": "block"}))), "block needs a row");
    let blocked = f.request("action", json!({"action": "block", "value": 0}));
    assert!(ok(&blocked));
    assert_eq!(blocked["panel"]["model"]["notice"], "不再推薦「你好」");
    assert_eq!(blocked["panel"]["model"]["undo"], true);
    assert!(f.service.blocked_entries().iter().any(|p| p.word == "你好"));
    let undone = f.request("action", json!({"action": "undo"}));
    assert!(ok(&undone));
    assert_eq!(undone["panel"]["model"]["notice"], "已復原");
    assert!(undone["panel"]["model"].get("undo").is_none(), "left out when false");
    assert!(f.service.blocked_entries().is_empty());
    // The app's own window: the same through popup_action.
    f.service.popup_action(PopupAction::Word(zhuyin_core::composer::CandidateAction::Block, 0));
    assert!(!f.service.blocked_entries().is_empty());
    f.service.popup_action(PopupAction::UndoWordAction);
    assert!(f.service.blocked_entries().is_empty());
}

#[test]
fn selected_text_opens_the_add_form_filled_in() {
    use zt_core::phrase_draft::{NO_SELECTION, PhraseDraft};
    let mut f = Fixture::new();
    let (j, alt) = ('J' as i32, modifier::ALT);
    f.service.shortcuts().record("addSelectedText", j, alt, None).unwrap();
    assert!(ok(&f.request("focus", json!({"secure": false}))));
    let press = json!({"secure": false, "key": j, "modifiers": alt, "release": false});
    let reply = f.request("key", press.clone());
    assert_eq!((reply["handled"].clone(), reply["selection"].clone()), (json!(true), json!(true)));
    f.ui_events();
    assert!(ok(&f.request("selection", json!({"text": "你好"}))));
    let filled = PhraseDraft { word: "你好".into(), reading: "ㄋㄧˇ ㄏㄠˇ".into(), problem: None };
    assert!(f.ui_events().contains(&UiEvent::AddPhrase(filled)));
    assert!(ok(&f.request("selection", json!({"text": ""}))));
    assert!(
        f.ui_events().iter().any(|e| matches!(e, UiEvent::AddPhrase(d) if d.problem.as_deref() == Some(NO_SELECTION)))
    );
    assert!(!ok(&f.request("selection", json!({"text": "字".repeat(300)}))), "too long");
    assert!(!ok(&f.request("selection", json!({"text": 3}))));
    // While composing the key does nothing.
    f.keys("su3");
    let composing = f.request("key", press.clone());
    assert_eq!(composing["handled"], true);
    assert!(composing.get("selection").is_none());
    assert_eq!(f.composition(), "你");
    // A private field and a secure one: never asked, nothing taken.
    assert!(ok(&f.request("focus", json!({"secure": false, "private": true}))));
    let private =
        f.request("key", json!({"secure": false, "private": true, "key": j, "modifiers": alt, "release": false}));
    assert!(private.get("selection").is_none());
    f.ui_events();
    assert!(ok(&f.request("selection", json!({"text": "你好", "private": true}))));
    assert!(ok(&f.request("focus", json!({"secure": true}))));
    let secure = f.request("key", json!({"secure": true, "key": j, "modifiers": alt, "release": false}));
    assert!(secure.get("selection").is_none());
    f.request("selection", json!({"text": "你好"}));
    assert!(
        !f.ui_events().iter().any(|e| matches!(e, UiEvent::AddPhrase(_))),
        "nothing from a private or secure field"
    );
}

/// `claims` (IPC.md §攔截的按鍵): the keys of 開啟快速設定, 符號與 Emoji and 選取的字加入自訂詞 the adapter claims with
/// nothing composed, each where its command can run; sent at focus and when they change.
#[test]
fn claims_list_the_keys_reachable_with_nothing_composed() {
    let mut f = Fixture::new();
    let picker = json!([',' as i32, modifier::CONTROL | modifier::ALT]);
    let quick = json!(['Q' as i32, modifier::ALT]);
    let add = json!(['J' as i32, modifier::ALT]);
    let focus = f.request("focus", json!({"secure": false}));
    assert_eq!(focus["claims"], json!([picker]), "only 符號與 Emoji has a key by default, sent at focus");
    assert!(f.request("poll", json!({})).get("claims").is_none(), "unchanged: left out");
    // A shortcut edit.
    f.service.shortcuts().record("quickSettings", 'Q' as i32, modifier::ALT, None).unwrap();
    f.service.shortcuts().record("addSelectedText", 'J' as i32, modifier::ALT, None).unwrap();
    let changed = f.request("poll", json!({}));
    assert_eq!(changed["claims"], json!([quick, picker, add]));
    assert!(f.request("poll", json!({})).get("claims").is_none());
    // English mode: 選取的字加入自訂詞 is Chinese only.
    let english = f.request("mode", json!({"english": true}));
    assert_eq!(english["claims"], json!([quick, picker]));
    let chinese = f.request("mode", json!({"english": false}));
    assert_eq!(chinese["claims"], json!([quick, picker, add]));
    // A private field: never 選取的字加入自訂詞; a secure one: nothing (the service takes no key there).
    let private = f.request("focus", json!({"secure": false, "private": true}));
    assert_eq!(private["claims"], json!([quick, picker]));
    let secure = f.request("secure", json!({"secure": true}));
    assert_eq!(secure["claims"], json!([]));
    let open = f.request("secure", json!({"secure": false, "private": false}));
    assert_eq!(open["claims"], json!([quick, picker, add]));
    // 符號與 Emoji turned off.
    f.service.settings().set("feature.symbolPicker", Value::Bool(false)).unwrap();
    assert_eq!(f.request("poll", json!({}))["claims"], json!([quick, add]));
    // Every focus carries the list, changed or not.
    assert_eq!(f.request("focus", json!({"secure": false}))["claims"], json!([quick, add]));
    assert!(f.request("blur", json!({})).get("claims").is_none(), "not at blur");
}

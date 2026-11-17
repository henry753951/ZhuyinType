//! The word actions from the keyboard: 不要再推薦, 忘記這個選字 and 加入自訂詞 on their keys while composing, Ctrl+Z
//! undoing them for a few seconds, and the tip the first lists show.

mod common;

use std::sync::Arc;

use common::Fixture;
use parking_lot::Mutex;
use serde_json::Value;
use zt_core::controller::{ControllerEvent, WORD_ACTION_TIP_KEY};
use zt_core::keys::{code, modifier};

fn manual_clock(f: &mut Fixture) -> Arc<Mutex<f64>> {
    let time = Arc::new(Mutex::new(100.0));
    let clock = time.clone();
    f.input.set_clock(move || *clock.lock());
    time
}

const CTRL_Z: (i32, u32) = ('Z' as i32, modifier::CONTROL);

#[test]
fn the_block_key_blocks_the_highlighted_candidate_and_ctrl_z_undoes_it() {
    let mut f = Fixture::new();
    let time = manual_clock(&mut f);
    f.type_keys("su3cl3");
    assert!(f.press(code::DOWN, 0));
    let first = f.input.presentation().rows[0].text.clone();
    assert!(f.press(code::DELETE, modifier::CONTROL), "Ctrl+Delete is 不要再推薦");
    f.input.sync_word_overlay();
    let p = f.input.presentation().clone();
    assert_eq!(p.notice.as_deref(), Some(format!("不再推薦「{first}」").as_str()));
    assert!(p.undo, "the notice offers 復原");
    let blocked = |f: &Fixture| f.input.session().engine().unwrap().blocked_entries().iter().any(|k| k.word == first);
    assert!(blocked(&f));
    assert!(f.commits().is_empty(), "the composition stays");
    *time.lock() += 2.0;
    assert!(f.press(CTRL_Z.0, CTRL_Z.1), "Ctrl+Z undoes within 5 s");
    f.input.sync_word_overlay();
    assert_eq!(f.input.presentation().notice.as_deref(), Some("已復原"));
    assert!(!f.input.presentation().undo);
    assert!(!blocked(&f));
    // A second action, then too late: Ctrl+Z is the app's again (the composition is sent first).
    assert!(f.press(code::DELETE, modifier::CONTROL));
    *time.lock() += 6.0;
    f.input.sync_word_overlay();
    assert!(!f.input.presentation().undo, "after 5 s 復原 is gone");
    assert!(!f.press(CTRL_Z.0, CTRL_Z.1));
}

#[test]
fn word_action_keys_pass_through_when_not_composing() {
    let mut f = Fixture::new();
    for (key, modifiers) in [
        (code::DELETE, modifier::CONTROL),
        (code::DELETE, modifier::CONTROL | modifier::SHIFT),
        (code::RETURN, modifier::CONTROL),
        CTRL_Z,
    ] {
        assert!(!f.press(key, modifiers), "{key:#x} goes to the app");
    }
    assert!(f.commits().is_empty());
}

#[test]
fn adding_without_a_list_takes_the_whole_composition() {
    let mut f = Fixture::new();
    f.type_keys("gj zj6");
    let composition = f.composition();
    assert!(f.press(code::RETURN, modifier::CONTROL), "Ctrl+Return is 加入自訂詞");
    assert_eq!(f.input.presentation().notice.as_deref(), Some(format!("已加入自訂詞：{composition}").as_str()));
    assert!(f.input.session().user_phrases().iter().any(|p| p.word == composition));
    assert!(f.commits().is_empty());
    assert!(f.press(CTRL_Z.0, CTRL_Z.1));
    assert!(!f.input.session().user_phrases().iter().any(|p| p.word == composition));
}

#[test]
fn the_first_lists_show_the_tip() {
    let mut f = Fixture::new();
    f.type_keys("su3cl3");
    f.press(code::DOWN, 0);
    f.input.sync_word_overlay();
    assert_eq!(f.input.presentation().tip.as_deref(), Some("Ctrl+Del 不要再推薦・右鍵看更多"));
    assert!(f.input.take_events().contains(&ControllerEvent::WordActionTip));
    f.input.sync_word_overlay();
    assert!(!f.input.take_events().contains(&ControllerEvent::WordActionTip), "counted once per list");
    f.press(code::ESCAPE, 0);
    f.input.sync_word_overlay();
    assert!(f.input.presentation().tip.is_none());
    f.settings.set(WORD_ACTION_TIP_KEY, Value::from(5)).unwrap();
    f.press(code::DOWN, 0);
    f.input.sync_word_overlay();
    assert!(f.input.presentation().has_list());
    assert!(f.input.presentation().tip.is_none(), "five lists showed it");
}

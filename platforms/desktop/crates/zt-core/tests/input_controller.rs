//! Port of the Qt `InputControllerTest`.

mod common;

use std::sync::Arc;

use common::Fixture;
use parking_lot::Mutex;
use serde_json::Value;
use zt_core::controller::{ControllerEvent, UsageSink};
use zt_core::corrections::CorrectionLog;
use zt_core::engine::EngineSession;
use zt_core::keys::{Key, ShiftSide, code, modifier};
use zt_core::settings::Settings;
use zt_core::stats::{PickList, UsageEvent};

/// A clock the test moves.
fn manual_clock(f: &mut Fixture) -> Arc<Mutex<f64>> {
    let time = Arc::new(Mutex::new(100.0));
    let clock = time.clone();
    f.input.set_clock(move || *clock.lock());
    time
}

fn shift(side: ShiftSide, modifiers: u32) -> Key {
    Key { shift_side: side, ..Key::new(code::SHIFT, modifiers) }
}

fn tap_shift(f: &mut Fixture) {
    f.input.press(&shift(ShiftSide::Unknown, modifier::SHIFT));
    f.input.release(&shift(ShiftSide::Unknown, 0));
}

fn mode_changes(events: &[ControllerEvent]) -> usize {
    events.iter().filter(|e| matches!(e, ControllerEvent::ModeChanged)).count()
}

#[test]
fn pass_through_commits() {
    // (command to change, feature to turn off, key, modifiers)
    let cases: &[(&str, &str, i32, u32)] = &[
        ("", "feature.deleteWord", code::BACKSPACE, modifier::CONTROL),
        ("", "feature.controlPunctuation", ',' as i32, modifier::CONTROL),
        ("nextPage", "", code::TAB, 0),
        ("previousPage", "", code::BACKTAB, 0),
        ("commitWithNewline", "", code::ENTER, modifier::SHIFT | modifier::KEYPAD),
        ("", "", 'C' as i32, modifier::META),
        ("", "", code::F1, 0),
    ];
    for &(command, feature, key, modifiers) in cases {
        let mut f = Fixture::new();
        if !feature.is_empty() {
            f.settings.set(feature, Value::Bool(false)).unwrap();
        }
        if command == "nextPage" {
            f.shortcuts.remove(command, 0).unwrap();
        } else if !command.is_empty() {
            f.shortcuts.record(command, 'J' as i32, modifier::ALT, Some(0)).unwrap();
        }
        f.type_keys("su3cl3");
        assert_eq!(f.composition(), "你好", "{command}{feature}");
        f.commits();
        assert!(!f.press(key, modifiers), "{key:#x} passes through");
        assert!(!f.input.session().composing(), "a host-owned key must not leave the composition stranded");
        assert_eq!(f.commits(), ["你好"], "{key:#x}");
    }
}

#[test]
fn shift_tap_rules() {
    let mut f = Fixture::new();
    let time = manual_clock(&mut f);
    f.input.press(&shift(ShiftSide::Unknown, modifier::SHIFT | modifier::CONTROL));
    f.input.release(&shift(ShiftSide::Unknown, modifier::CONTROL));
    assert!(!f.input.english());
    f.input.press(&shift(ShiftSide::Unknown, modifier::SHIFT));
    *time.lock() += 0.38;
    f.input.release(&shift(ShiftSide::Unknown, 0));
    assert!(!f.input.english(), "a long press is not a tap");
    f.input.press(&Key { repeat: true, ..shift(ShiftSide::Unknown, modifier::SHIFT) });
    f.input.release(&shift(ShiftSide::Unknown, 0));
    assert!(!f.input.english());
    f.type_keys("su3cl3");
    f.input.take_events();
    tap_shift(&mut f);
    assert!(f.input.english());
    let events = f.input.take_events();
    assert_eq!(mode_changes(&events), 1);
    assert!(events.contains(&ControllerEvent::ModeSwitched { english: true }));
    assert_eq!(events.iter().filter(|e| matches!(e, ControllerEvent::Commit(_))).count(), 1);
    assert!(!f.input.session().composing());
    f.input.release(&shift(ShiftSide::Unknown, 0));
    assert_eq!(mode_changes(&f.input.take_events()), 0);
}

#[test]
fn remapped_command_waits_for_modifier_chord() {
    let mut f = Fixture::new();
    f.shortcuts.record("deleteWord", 'D' as i32, modifier::ALT, Some(0)).unwrap();
    f.type_keys("su3cl3");
    f.commits();
    assert!(!f.press(code::ALT, modifier::ALT));
    assert_eq!(f.composition(), "你好");
    assert!(f.press('D' as i32, modifier::ALT));
    assert!(f.composition().is_empty());
    assert!(f.commits().is_empty());
}

#[test]
fn remapped_newline_inserts_after_commit() {
    let mut f = Fixture::new();
    f.shortcuts.record("commitWithNewline", 'J' as i32, modifier::ALT, Some(0)).unwrap();
    f.type_keys("su3cl3");
    assert!(f.press('J' as i32, modifier::ALT));
    assert_eq!(f.commits(), ["你好", "\n"]);
    assert!(!f.input.session().composing());
    f.shortcuts.reset(Some("commitWithNewline")).unwrap();
    f.type_keys("su3cl3");
    assert!(!f.press(code::RETURN, modifier::SHIFT));
    assert_eq!(f.commits(), ["你好"]);
}

#[test]
fn shift_side_and_overlapping_keys() {
    let mut f = Fixture::new();
    f.settings.set("shiftToggle", Value::from("left")).unwrap();
    tap_shift(&mut f);
    assert!(!f.input.english());
    f.input.press(&shift(ShiftSide::Right, modifier::SHIFT));
    f.input.release(&shift(ShiftSide::Right, 0));
    assert!(!f.input.english());
    f.input.press(&shift(ShiftSide::Left, modifier::SHIFT));
    f.input.press(&shift(ShiftSide::Right, modifier::SHIFT));
    f.input.release(&shift(ShiftSide::Left, modifier::SHIFT));
    f.input.release(&shift(ShiftSide::Right, 0));
    assert!(!f.input.english());
    f.input.press(&shift(ShiftSide::Left, modifier::SHIFT));
    f.input.press(&shift(ShiftSide::Left, modifier::SHIFT));
    f.input.release(&shift(ShiftSide::Left, 0));
    assert!(f.input.english());
}

#[test]
fn caps_lock_observed_state_and_disabled_feature() {
    let mut f = Fixture::new();
    f.settings.set("feature.capsLockEnglish", Value::Bool(false)).unwrap();
    f.type_keys("su3cl3");
    f.press(code::CAPS_LOCK, 0);
    assert!(!f.input.english());
    assert_eq!(f.composition(), "你好");
    f.commits();
    f.settings.set("feature.capsLockEnglish", Value::Bool(true)).unwrap();
    assert!(f.input.english());
    // The service refreshes the controller when a setting changes; here the test does.
    f.input.refresh_settings();
    assert_eq!(f.commits(), ["你好"]);
    f.input.press(&Key { repeat: true, ..Key::new(code::CAPS_LOCK, 0) });
    assert!(f.input.english());
    assert!(!f.press('S' as i32, 0));
    f.press(code::CAPS_LOCK, 0);
    assert!(!f.input.english());
    f.type_keys("su3");
    assert!(f.input.session().composing());
}

#[test]
fn english_rejects_candidate_clicks() {
    let mut f = Fixture::new();
    tap_shift(&mut f);
    assert!(f.input.english());
    common::type_session(f.input.session_mut(), "su3");
    f.input.session_mut().key(&Key::new(code::DOWN, 0)).unwrap();
    assert!(f.input.presentation().has_list());
    let before = f.composition();
    f.input.take_events();
    assert!(!f.input.choose(1));
    assert_eq!(f.composition(), before);
    assert!(!f.input.take_events().iter().any(|e| matches!(e, ControllerEvent::CandidateChosen { .. })));
}

#[test]
fn focus_loss_cancels_pending_tap_and_commits_once() {
    let mut f = Fixture::new();
    f.type_keys("su3");
    f.press(code::DOWN, 0);
    assert!(f.input.presentation().has_list());
    f.commits();
    f.input.press(&shift(ShiftSide::Unknown, modifier::SHIFT));
    f.input.deactivate();
    f.input.release(&shift(ShiftSide::Unknown, 0));
    assert!(!f.input.english());
    assert!(!f.input.session().composing());
    assert_eq!(f.commits().len(), 1);
    f.input.deactivate();
    assert!(f.commits().is_empty());
}

#[test]
fn secure_transitions_discard_and_never_commit_to_host() {
    let mut f = Fixture::new();
    f.type_keys("su3cl3");
    f.commits();
    f.input.set_secure(true);
    assert!(!f.input.session().composing());
    assert!(!f.press('S' as i32, 0));
    assert!(!f.input.choose(0));
    tap_shift(&mut f);
    assert!(!f.input.english());
    // An external caller must not leak a stale secure composition during focus loss.
    common::type_session(f.input.session_mut(), "su3");
    assert!(f.input.session().composing());
    f.input.deactivate();
    assert!(f.commits().is_empty());
    common::type_session(f.input.session_mut(), "su3");
    f.input.set_secure(false);
    assert!(!f.input.session().composing());
    assert!(f.commits().is_empty());
    f.type_keys("su3");
    f.press(code::DOWN, 0);
    assert!(f.input.choose(1));
    assert!(f.press(code::RETURN, 0));
    assert!(!f.input.session().learned_entries().is_empty());
}

#[test]
fn fuzzy_setting_reaches_conversion() {
    let mut f = Fixture::new();
    // ㄙㄨ ㄈㄨˊ: as typed, no 舒服 (read ㄕㄨ ㄈㄨˊ) unless ㄙ and ㄕ are fuzzy (typing corrections off: they too
    // would offer it in the ⇧↑ list).
    f.settings.set("typoTolerance", Value::from("off")).unwrap();
    let typed = |f: &mut Fixture| {
        f.input.deactivate();
        f.type_keys("nj zj6");
        let mut found = f.composition() == "舒服";
        if f.press(code::UP, modifier::SHIFT) {
            found |= f.input.presentation().rows.iter().any(|row| row.text == "舒服");
        }
        f.input.deactivate();
        found
    };
    assert!(!typed(&mut f));
    f.settings.set("fuzzy.sSh", Value::Bool(true)).unwrap();
    assert!(typed(&mut f));
    f.settings.set("fuzzy.sSh", Value::Bool(false)).unwrap();
    assert!(!typed(&mut f));
    // A fresh process reads the saved toggle back from the file and opens with it.
    f.settings.set("fuzzy.sSh", Value::Bool(true)).unwrap();
    let restored = Settings::open(Some(f.dir.join("settings.json")));
    assert!(restored.bool("fuzzy.sSh"));
    let mut session = EngineSession::new();
    session.apply(&restored).unwrap();
    let data = zt_core::engine::EngineData::from_lexicon(&f.dir.join("lexicon.tsv")).unwrap();
    session.open(&data, Some(&f.dir.join("restored"))).unwrap();
    common::type_session(&mut session, "nj zj6");
    assert_eq!(session.presentation().composition, "舒服");
}

#[test]
fn usage_events_follow_mac_stats() {
    let mut f = Fixture::new();
    f.settings.set("recordsUsageStats", Value::Bool(true)).unwrap();
    let events: Arc<Mutex<Vec<UsageEvent>>> = Arc::default();
    let sink_events = events.clone();
    let sink: UsageSink = Arc::new(move |event| sink_events.lock().push(*event));
    f.input.set_usage_sink(Some(sink));
    let take = || std::mem::take(&mut *events.lock());
    f.type_keys("su3");
    let seen = take();
    assert!(seen.iter().any(|e| matches!(e, UsageEvent::Key { .. })));
    assert!(seen.iter().any(|e| matches!(e, UsageEvent::Interval { .. })));
    assert!(f.press(code::DOWN, 0));
    assert!(f.press('2' as i32, 0));
    assert!(take().contains(&UsageEvent::Pick(PickList::Normal)));
    assert!(f.press(code::RETURN, 0));
    let commit = take().into_iter().find(|e| matches!(e, UsageEvent::Commit { .. })).unwrap();
    assert_eq!(
        commit,
        UsageEvent::Commit {
            characters: 1,
            composition_ended: true,
            clean: Some(false),
            corrected_shown: false,
            corrected_committed: false
        }
    );
    f.type_keys("su3cl3");
    assert!(f.press(code::RETURN, 0));
    assert!(take().iter().any(|e| matches!(e, UsageEvent::Commit { clean: Some(true), .. })));
    assert!(!f.press(code::BACKSPACE, 0));
    assert!(take().contains(&UsageEvent::DeleteAfterCommit));
    // Nothing is counted with the setting off.
    f.settings.set("recordsUsageStats", Value::Bool(false)).unwrap();
    f.type_keys("su3");
    assert!(take().is_empty());
}

#[test]
fn corrections_are_marked_and_backspace_puts_them_back() {
    let mut f = Fixture::with_lexicon(common::CORRECTION_LEXICON);
    f.settings.set("recordsUsageStats", Value::Bool(true)).unwrap();
    let events: Arc<Mutex<Vec<UsageEvent>>> = Arc::default();
    let sink_events = events.clone();
    let sink: UsageSink = Arc::new(move |event| sink_events.lock().push(*event));
    f.input.set_usage_sink(Some(sink));
    let take = || std::mem::take(&mut *events.lock());
    f.type_keys("wk3");
    let p = f.input.presentation();
    assert_eq!(p.composition, "乙", "the tone corrected");
    assert!(p.corrected.len() == 1 && p.corrected[0] == (0..1), "「標示自動修正」 marks it");
    assert!(p.backspace_reverts, "「⌫ 還原自動修正」 is on by default");
    take();
    // ⌫ right after the correction converts the keys as typed; counted as a revert, not a deletion.
    assert!(f.press(code::BACKSPACE, 0));
    assert_eq!(f.composition(), "甲");
    assert!(f.input.presentation().corrected.is_empty());
    let seen = take();
    assert!(seen.contains(&UsageEvent::CorrectionReverted));
    assert!(!seen.contains(&UsageEvent::Backspace));
    // The window on a corrected span names the keys as typed.
    f.input.deactivate();
    f.type_keys("wk3");
    assert!(f.press(code::DOWN, 0));
    let p = f.input.presentation();
    assert_eq!(p.typed_reading.as_deref(), Some("ㄊㄜˇ"));
    assert_eq!(p.typed_footer().as_deref(), Some("你打的是 ㄊㄜˇ · ⌫ 還原"));
    assert_eq!(p.panel_model(None).typed.as_deref(), Some("你打的是 ㄊㄜˇ · ⌫ 還原"));
    f.settings.set("undoCorrectionWithBackspace", Value::Bool(false)).unwrap();
    f.input.deactivate();
    f.type_keys("wk3");
    assert!(!f.input.presentation().backspace_reverts);
    assert!(f.press(code::DOWN, 0));
    assert_eq!(f.input.presentation().typed_footer().as_deref(), Some("你打的是 ㄊㄜˇ"));
    // 「標示自動修正」 off: no marks, no footer.
    f.settings.set("showCorrections", Value::Bool(false)).unwrap();
    f.input.deactivate();
    f.type_keys("wk3");
    assert_eq!(f.composition(), "乙");
    assert!(f.input.presentation().corrected.is_empty());
    assert!(f.press(code::DOWN, 0));
    assert_eq!(f.input.presentation().typed_footer(), None);
}

#[test]
fn correction_log_writes_mac_records() {
    let mut f = Fixture::new();
    let log = CorrectionLog::open(Some(&f.dir.join("user")));
    f.input.set_correction_log(Some(log.clone()), "test");
    f.settings.set("recordsCorrections", Value::Bool(true)).unwrap();
    f.type_keys("su3");
    assert!(f.press(code::DOWN, 0));
    assert!(f.press('2' as i32, 0));
    assert!(f.press(code::RETURN, 0));
    assert_eq!(log.count(), 1);
    let text = std::fs::read_to_string(log.location().unwrap()).unwrap();
    assert!(text.ends_with('\n'));
    let record: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(record["schema"], 1);
    assert_eq!(record["app_version"], "test");
    assert_eq!(record["corrections"][0], "pick");
    let keys = record["keys"].as_str().unwrap();
    assert!(keys.starts_with("s<dt:") && keys.contains("<down>"), "{keys}");
    assert!(record["typing"].as_str().unwrap().starts_with('s'));
    assert!(record["settings"].get("typo_tolerance").is_some());
    assert_eq!(record["settings"]["punctuation_width"], "full");
    assert!(record.get("context").is_none());
    let events = record["events"].as_array().unwrap();
    assert_eq!(events.last().unwrap()["type"], "commit");
    let pick = events.iter().find(|e| e["type"] == "pick" && e["rank"] == 2).expect("a pick of rank 2");
    assert_eq!(pick["candidate_text"], "妳");
    assert_eq!(pick["candidate_reading"], "ㄋㄧˇ");
    // The keys are sorted, as the Mac writes them.
    let line = text.lines().next().unwrap();
    assert!(line.starts_with(r#"{"app_version":"test","committed":"#), "{line}");
    // A composition with no correction is not recorded; the log is cleared on request.
    f.type_keys("su3cl3");
    assert!(f.press(code::RETURN, 0));
    assert_eq!(log.count(), 1);
    log.clear().unwrap();
    assert_eq!(log.count(), 0);
}

#[test]
fn preceding_text_and_same_app_carry_over() {
    let mut f = Fixture::new();
    let clock = Arc::new(Mutex::new(1_000_000_i64));
    let now = clock.clone();
    f.input.set_now(move || *now.lock());
    f.input.set_client("editor", Some("我說".to_owned()));
    f.type_keys("su3cl3");
    assert_eq!(f.input.recent_text(), "我說");
    assert!(f.press(code::RETURN, 0));
    assert_eq!(f.input.recent_text(), "我說你好");
    // Another app, then back without the app's text: what was sent there within the context window is the context.
    f.input.set_client("other", None);
    f.type_keys("su3");
    assert_eq!(f.input.recent_text(), "");
    f.input.deactivate();
    f.input.set_client("editor", None);
    *clock.lock() += 50_000;
    f.type_keys("su3");
    assert!(f.input.recent_text().starts_with("我說你好"));
    f.input.deactivate();
    f.input.set_client("x", None);
    f.input.set_client("editor", None);
    *clock.lock() += 70_000;
    f.type_keys("su3");
    assert_eq!(f.input.recent_text(), "");
    // Secure input forgets it.
    f.input.deactivate();
    f.input.set_secure(true);
    assert_eq!(f.input.recent_text(), "");
}

#[test]
fn punctuation_width_and_insert_space_commands() {
    let mut f = Fixture::new();
    f.type_keys("su3");
    let before = f.composition();
    assert!(f.press(code::SPACE, modifier::SHIFT), "Shift+Space switches the punctuation width");
    assert_eq!(f.composition(), before);
    assert_ne!(f.input.session().punctuation_width(), zhuyin_core::api::PunctuationWidth::Full);
    assert!(
        f.press(code::SPACE, modifier::CONTROL | modifier::SHIFT),
        "Ctrl+Shift+Space puts a space into the composition"
    );
    assert!(f.composition().contains(' '), "{}", f.composition());
}

#[test]
fn list_edge_is_reported() {
    let mut f = Fixture::new();
    f.type_keys("su3");
    f.press(code::DOWN, 0);
    f.input.take_events();
    // Two candidates on one page: Page Up at the first page stops.
    f.press(code::PAGE_UP, 0);
    assert!(f.input.take_events().contains(&ControllerEvent::Boundary(-1)));
}

#[test]
fn dispatch_latency() {
    let mut f = Fixture::new();
    let mut samples: Vec<u128> = (0..300)
        .map(|_| {
            let started = std::time::Instant::now();
            f.press(code::F1, 0);
            started.elapsed().as_nanos()
        })
        .collect();
    samples.sort_unstable();
    println!("Idle host-key dispatch ns p50={} p95={} p99={}", samples[150], samples[285], samples[297]);
}

#[test]
fn page_up_and_page_down_page_with_the_paging_keys_off() {
    // 翻頁 off hands Tab, ⇧Tab and Ctrl+↓ ↑ to the app; Page Up and Page Down still page (macOS does the same).  The
    // test list has one page: the engine paging shows as the list's edge.
    let mut f = Fixture::new();
    f.settings.set("feature.pageKeys", Value::Bool(false)).unwrap();
    f.type_keys("su3");
    f.press(code::DOWN, 0);
    f.input.take_events();
    assert!(f.press(code::PAGE_DOWN, 0), "Page Down pages");
    assert!(f.input.take_events().contains(&ControllerEvent::Boundary(1)));
    assert!(f.press(code::PAGE_UP, 0));
    assert!(f.input.take_events().contains(&ControllerEvent::Boundary(-1)));
    assert!(f.commits().is_empty());
    assert!(!f.press(code::DOWN, modifier::CONTROL), "Ctrl+↓ goes to the app");
    assert!(!f.commits().is_empty(), "after the composition is sent");
}

#[test]
fn ctrl_arrows_list_the_neighbouring_character() {
    // A vertical list does it with ← → too: Ctrl+← is the same key there.
    let mut a = Fixture::new();
    let mut b = Fixture::new();
    for f in [&mut a, &mut b] {
        f.type_keys("su3cl3");
        f.press(code::DOWN, 0);
    }
    assert!(a.press(code::LEFT, modifier::CONTROL));
    assert!(b.press(code::LEFT, 0));
    assert_eq!(a.input.presentation(), b.input.presentation());
    // A horizontal list moves its highlight with ← →: Ctrl+← lists the previous character, in the same kind of list.
    let mut f = Fixture::new();
    f.settings.set("candidateLayout", Value::from("horizontal")).unwrap();
    f.type_keys("su3cl3");
    f.press(code::DOWN, modifier::SHIFT);
    let before = f.input.presentation().clone();
    assert!(before.single && before.count > 0);
    assert!(f.press(code::LEFT, modifier::CONTROL));
    let after = f.input.presentation().clone();
    assert!(after.single, "still 只列一個字");
    assert_ne!(after.focused_clause.or(after.target_clause), None);
    assert_ne!((after.cursor, &after.rows), (before.cursor, &before.rows), "moved to the previous character");
    assert!(f.press(code::RIGHT, modifier::CONTROL));
    assert_eq!(f.input.presentation().rows, before.rows, "and back");
    // With no list it is ← →: the cursor moves; with nothing composed the app gets it.
    f.press(code::ESCAPE, 0);
    let cursor = f.input.presentation().cursor;
    assert!(f.press(code::LEFT, modifier::CONTROL));
    assert!(f.input.presentation().cursor < cursor);
    let mut idle = Fixture::new();
    assert!(!idle.press(code::LEFT, modifier::CONTROL));
    assert!(!idle.press(code::BACKSPACE, modifier::CONTROL), "Ctrl+⌫ deletes the app's word");
}

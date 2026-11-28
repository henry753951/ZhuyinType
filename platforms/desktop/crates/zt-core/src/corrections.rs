//! The correction log (「記錄改字」, `docs/product/DESIGN.md` §2.7), ported from macOS `IMK/CorrectionRecorder.swift`
//! (through the Qt `CorrectionLog`): every composition in which the user corrected the engine becomes one JSON line
//! of `corrections.jsonl`, in the macOS schema (schema 1, snake_case keys in sorted order), so `zt eval corrections`
//! reads both.
//!
//! [`CorrectionRecorder`] only observes the input controller: it is told each key (with its key-down time), click
//! and commit with the presentation before and after, and never changes what happens.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use zhuyin_core::api::{KeyInput, PunctuationWidth};

use crate::keys;
use crate::presentation::Presentation;
use crate::settings::{self, Settings};

/// The log file's name in the user's data directory.
pub const FILE_NAME: &str = "corrections.jsonl";

/// What the input controller reports to the recorder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObservedInput {
    /// A key, with the monotonic time (seconds) it went down.
    Key { input: KeyInput, time: f64 },
    /// A click on candidate `index` of the whole list.
    Click { index: usize },
    /// The candidate window's corrections switch.
    ToggleCorrections,
    /// Everything composed was sent (focus loss, a key for the app).
    Commit,
}

/// A candidate as the engine knows it, read before a pick (macOS `CandidateDetails`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateDetails {
    /// The engine's word, before display forms.
    pub text: String,
    /// The reading learned from (as typed); empty for a literal.
    pub reading: String,
    /// The composition tokens it covers.
    pub start: usize,
    pub end: usize,
}

/// The recording name of a punctuation width.
#[must_use]
pub fn width_name(width: PunctuationWidth) -> &'static str {
    match width {
        PunctuationWidth::Legacy => "legacy",
        PunctuationWidth::Full => "full",
        PunctuationWidth::Half => "half",
    }
}

/// The engine settings that change what the keys produce, recorded with each composition (macOS
/// `CorrectionSettings`).
#[must_use]
pub fn correction_settings(settings: &Settings, punctuation_width: Option<PunctuationWidth>) -> Map<String, Value> {
    let fuzzy: Vec<Value> =
        [("fuzzy.zZh", "zh"), ("fuzzy.cCh", "ch"), ("fuzzy.sSh", "sh"), ("fuzzy.enEng", "en"), ("fuzzy.anAng", "an")]
            .into_iter()
            .filter(|(key, _)| settings.bool(key))
            .map(|(_, name)| Value::from(name))
            .collect();
    let space = match settings.string("spaceAction").as_str() {
        "candidates" => "openCandidates",
        "insert" => "insertSpace",
        _ => "commitAndSpace",
    };
    let mut object = Map::new();
    object.insert("typo_tolerance".into(), settings.string("typoTolerance").into());
    object.insert("key_order".into(), settings.string("keyOrder").into());
    object.insert("fuzzy_sounds".into(), Value::Array(fuzzy));
    object.insert("punctuation_keys".into(), settings.string("punctuationKeys").into());
    if let Some(width) = punctuation_width {
        object.insert("punctuation_width".into(), width_name(width).into());
    }
    object.insert("space_action".into(), space.into());
    object.insert("character_forms".into(), settings.string("characterForms").into());
    object.insert("candidate_anchor".into(), settings.string("candidateAnchor").into());
    object.insert("selection_keys".into(), settings.string("selectionKeys").into());
    object.insert("vertical_list".into(), (settings.string("candidateLayout") != "horizontal").into());
    object.insert("learning".into(), settings.bool("learningEnabled").into());
    object.insert("preceding_text".into(), settings.bool("usesPrecedingText").into());
    object.insert("reading_channel".into(), settings.bool("readingChannel").into());
    object
}

// MARK: - Text helpers (Unicode scalars, as Swift's unicodeScalars and Qt's toUcs4)

fn scalars(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn from_scalars(values: &[char]) -> String {
    values.iter().collect()
}

fn is_zhuyin(c: char) -> bool {
    matches!(c as u32, 0x3105..=0x3129 | 0x02CA | 0x02C7 | 0x02CB | 0x02D9)
}

fn milliseconds(seconds: f64) -> i64 {
    (seconds * 1000.0).round().max(0.0) as i64
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn at_end(p: &Presentation) -> bool {
    p.is_empty() || p.cursor == utf16_len(&p.composition)
}

/// The scalar offset of UTF-16 offset `utf16` in `text`.
fn scalar_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    let mut count = 0;
    for c in text.chars() {
        if units >= utf16 {
            break;
        }
        units += c.len_utf16();
        count += 1;
    }
    count
}

fn is_content(key: KeyInput) -> bool {
    matches!(
        key,
        KeyInput::Char(_)
            | KeyInput::ShiftedChar(_)
            | KeyInput::Printed(_)
            | KeyInput::ControlChar(_)
            | KeyInput::Space
            | KeyInput::ShiftSpace
            | KeyInput::Backspace
            | KeyInput::OptionBackspace
            | KeyInput::ForwardDelete
            | KeyInput::Escape
            | KeyInput::Enter
            | KeyInput::ShiftEnter
    )
}

/// Where two texts differ: the start (in Unicode scalars), the part of `a` replaced and the part of `b` in its place.
#[must_use]
pub fn difference(a: &str, b: &str) -> (usize, String, String) {
    let (x, y) = (scalars(a), scalars(b));
    let shorter = x.len().min(y.len());
    let prefix = (0..shorter).take_while(|&i| x[i] == y[i]).count();
    let suffix = (0..shorter - prefix).take_while(|&i| x[x.len() - 1 - i] == y[y.len() - 1 - i]).count();
    (prefix, from_scalars(&x[prefix..x.len() - suffix]), from_scalars(&y[prefix..y.len() - suffix]))
}

/// The list's kind as recorded.
fn kind(p: &Presentation) -> &'static str {
    match (p.single, p.tolerant == Some(true)) {
        (true, true) => "single_corrections",
        (true, false) => "single",
        (false, true) => "corrections",
        (false, false) => "normal",
    }
}

/// The open list's state (not its items), for "the list changed".
/// Count, page, page size, selection, corrections mode and single-character flag.
type ListState = (usize, usize, usize, Option<usize>, Option<bool>, bool);

fn list_state(p: &Presentation) -> Option<ListState> {
    p.has_list().then_some((p.count, p.page, p.page_size, p.selection, p.tolerant, p.single))
}

fn event(kind: &str, t: i64, shown: &str) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("type".into(), kind.into());
    object.insert("t".into(), t.into());
    object.insert("shown".into(), shown.into());
    object
}

fn set_optional(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    match value {
        Some(value) => {
            object.insert(key.into(), value.into());
        }
        None => {
            object.remove(key);
        }
    }
}

fn str_field<'a>(object: &'a Map<String, Value>, key: &str) -> &'a str {
    object.get(key).and_then(Value::as_str).unwrap_or_default()
}

// MARK: - Recorder

/// The converted part of a composition and its unfinished Zhuyin: the "shape" deletions go back to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Shape {
    converted: usize,
    pending: String,
}

impl Shape {
    fn of(text: &str) -> Self {
        let all = scalars(text);
        let pending = all.iter().rev().take_while(|c| is_zhuyin(**c)).count();
        Self { converted: all.len() - pending, pending: from_scalars(&all[all.len() - pending..]) }
    }
}

#[derive(Clone, Debug)]
struct TypedKey {
    label: String,
    time: f64,
}

#[derive(Clone, Debug)]
struct Retyping {
    event: usize,
    target: usize,
    position: usize,
    keys: Vec<TypedKey>,
}

#[derive(Clone, Debug)]
struct Session {
    start: f64,
    context: Option<String>,
    punctuation_width: Option<String>,
    keys: String,
    last_key: Option<f64>,
    typing: Vec<TypedKey>,
    original: Vec<TypedKey>,
    history: Vec<(usize, Shape)>,
    tracking: bool,
    replayable: bool,
    original_replayable: bool,
    events: Vec<Map<String, Value>>,
    corrections: Vec<String>,
    shown: Option<String>,
    retyping: Option<Retyping>,
    selecting: bool,
    literal: Option<usize>,
    after_commit: Option<(String, String)>,
    sent: String,
}

impl Session {
    fn new(start: f64) -> Self {
        Self {
            start,
            context: None,
            punctuation_width: None,
            keys: String::new(),
            last_key: None,
            typing: Vec::new(),
            original: Vec::new(),
            history: vec![(0, Shape::default())],
            tracking: true,
            replayable: true,
            original_replayable: true,
            events: Vec::new(),
            corrections: Vec::new(),
            shown: None,
            retyping: None,
            selecting: false,
            literal: None,
            after_commit: None,
            sent: String::new(),
        }
    }

    fn last_type(&self) -> Option<&str> {
        self.events.last().and_then(|e| e.get("type")).and_then(Value::as_str)
    }

    fn key_time(&self) -> f64 {
        self.last_key.unwrap_or(0.0)
    }
}

#[derive(Clone, Debug)]
struct LastCommit {
    time: f64,
    text: String,
    committed: String,
    passed: usize,
    deleted: usize,
    typing: String,
    replayable: bool,
    context: Option<String>,
    punctuation_width: Option<String>,
}

/// Where finished records go.
pub type RecordSink = Box<dyn FnMut(Map<String, Value>) + Send>;

/// Turns what the controller reports into correction records.
pub struct CorrectionRecorder {
    sink: RecordSink,
    /// Seconds after a commit in which deleting it and typing again is a correction.
    pub window: f64,
    /// 一併記錄游標前的文字.
    pub records_context: bool,
    /// The engine settings recorded with each composition ([`correction_settings`]).
    pub settings: Map<String, Value>,
    pub app_version: String,
    pub selection_keys: Vec<char>,
    /// Today's date as recorded (`YYYY-MM-DD`); tests replace it.
    pub today: Box<dyn Fn() -> String + Send>,
    session: Option<Session>,
    suspended: Option<(Session, f64)>,
    last_commit: Option<LastCommit>,
}

impl CorrectionRecorder {
    /// A recorder that hands finished records to `sink`.
    #[must_use]
    pub fn new(sink: RecordSink) -> Self {
        Self {
            sink,
            window: 30.0,
            records_context: false,
            settings: Map::new(),
            app_version: "dev".to_owned(),
            selection_keys: "123456789".chars().collect(),
            today: Box::new(|| chrono::Local::now().format("%Y-%m-%d").to_string()),
            session: None,
            suspended: None,
            last_commit: None,
        }
    }

    /// The insertion point moved: deleting what was sent is no longer a correction of it.
    pub fn break_context(&mut self) {
        self.last_commit = None;
        self.suspended = None;
    }

    /// The candidate an input is about to pick (read before the composer handles it), for the pick event.
    #[must_use]
    pub fn candidate_index(&self, input: &ObservedInput, before: &Presentation) -> Option<usize> {
        if !before.has_list() {
            return None;
        }
        match *input {
            ObservedInput::Click { index } => Some(index),
            ObservedInput::Key { input: KeyInput::Enter, .. } => before.selection,
            ObservedInput::Key { input: KeyInput::Char(c), .. } => {
                let number = self.selection_keys.iter().position(|k| *k == c)?;
                Some(before.page * before.page_size + number)
            }
            _ => None,
        }
    }

    /// The punctuation width changed (標點全半形): a state change even when the composition stays the same.
    pub fn punctuation_width_changed(
        &mut self,
        width: PunctuationWidth,
        after: &Presentation,
        secure: bool,
        time: f64,
    ) {
        self.settings.insert("punctuation_width".into(), width_name(width).into());
        if secure {
            self.session = None;
            self.suspended = None;
            self.last_commit = None;
            return;
        }
        let session = match (&mut self.session, &mut self.suspended) {
            (Some(s), _) | (None, Some((s, _))) => s,
            (None, None) => return,
        };
        if let Some(last) = session.last_key {
            let _ = write!(session.keys, "<dt:{}>", milliseconds(time - last));
        }
        session.keys.push_str("<punct-width>");
        session.last_key = Some(time);
        let key = TypedKey { label: "<punct-width>".into(), time };
        if session.retyping.is_some() {
            session.original.push(key.clone());
        }
        append_typing(session, key, after);
    }

    /// One key, click or commit, with the presentation before and after it.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        input: &ObservedInput,
        before: &Presentation,
        after: &Presentation,
        commit: &str,
        handled: bool,
        secure: bool,
        candidate: Option<CandidateDetails>,
        time: f64,
        context: &dyn Fn() -> String,
    ) {
        let has_commit = !commit.is_empty();
        if secure {
            self.session = None;
            self.suspended = None;
            self.last_commit = None;
            return;
        }
        if before.is_empty() && after.is_empty() && !has_commit {
            // A key for the app with nothing composed.
            let ObservedInput::Key { input: key, .. } = input else { return };
            if handled {
                return;
            }
            if *key == KeyInput::Backspace {
                self.follow_deletion(time);
            } else {
                self.last_commit = None;
            }
            self.suspended = None;
            return;
        }
        if self.session.is_none() {
            // A composition that started before recording was turned on is not recorded.
            if !before.is_empty() {
                return;
            }
            match self.suspended.take() {
                Some((kept, at)) if time - at < self.window => self.session = Some(kept),
                _ => {
                    let mut fresh = Session::new(time);
                    fresh.punctuation_width =
                        self.settings.get("punctuation_width").and_then(Value::as_str).map(str::to_owned);
                    if self.records_context {
                        fresh.context = Some(context());
                    }
                    if let Some(last) = &self.last_commit
                        && last.deleted > last.passed
                    {
                        let committed = scalars(&last.committed);
                        let take = committed.len().min(last.deleted - last.passed);
                        let deleted = from_scalars(&committed[committed.len() - take..]);
                        let mut e = event("delete_after_commit", 0, &before.composition);
                        e.insert("deleted".into(), deleted.clone().into());
                        e.insert("previous_committed".into(), last.committed.clone().into());
                        e.insert("previous_typing".into(), last.typing.clone().into());
                        e.insert("previous_replayable".into(), last.replayable.into());
                        set_optional(&mut e, "previous_context", last.context.clone());
                        set_optional(&mut e, "previous_punctuation_width", last.punctuation_width.clone());
                        fresh.events.push(e);
                        fresh.after_commit = Some((deleted, last.committed.clone()));
                    }
                    self.session = Some(fresh);
                }
            }
            self.suspended = None;
            self.last_commit = None;
        }
        let mut s = self.session.take().expect("a session");
        let label = self.key_label(input, before);
        if let Some(last) = s.last_key {
            let _ = write!(s.keys, "<dt:{}>", milliseconds(time - last));
        }
        s.keys.push_str(label.as_deref().unwrap_or("<?>"));
        s.last_key = Some(time);
        let t = milliseconds(time - s.start);
        // A typing key that made the composition send its head first: the key is typing, and the record goes on.
        let carried = has_commit && !after.is_empty();
        if carried {
            let mut whole = after.clone();
            whole.composition = format!("{commit}{}", after.composition);
            end_retyping(&mut s, &whole);
        }
        match *input {
            ObservedInput::Commit => {}
            ObservedInput::Click { index } => pick(&mut s, index, "click", before, after, candidate, t),
            ObservedInput::ToggleCorrections => {
                if after.has_list() && list_state(after) != list_state(before) {
                    let mut e = event("switch_list", t, &after.composition);
                    e.insert("list".into(), kind(after).into());
                    s.events.push(e);
                }
            }
            ObservedInput::Key { input: key, .. } => {
                if before.has_list() && self.is_list_key(key) {
                    self.list_key(&mut s, key, before, after, candidate, t);
                } else {
                    if before.has_list() {
                        s.events.push(event("close_list", t, &after.composition));
                    }
                    composing_key(&mut s, key, label.as_deref(), before, after, !carried && has_commit, t);
                }
            }
        }
        if carried {
            carry(&mut s, commit, after, t);
            self.session = Some(s);
        } else if has_commit || (after.is_empty() && !s.sent.is_empty()) {
            // Sent (or, emptied after part of it was sent: the record ends with what was sent).
            self.finish(s, if has_commit { commit } else { "" }, input, handled, time, t);
        } else if after.is_empty() {
            // Emptied without a commit (Escape or Backspace): typing it again continues this record.
            s.literal = None;
            s.selecting = false;
            self.suspended = Some((s, time));
        } else {
            self.session = Some(s);
        }
    }

    fn list_key(
        &self,
        s: &mut Session,
        key: KeyInput,
        before: &Presentation,
        after: &Presentation,
        candidate: Option<CandidateDetails>,
        t: i64,
    ) {
        end_retyping(s, after);
        s.literal = None;
        if key == KeyInput::Char('`') {
            // The list's span as literal Zhuyin: depends on the list, so the typing no longer replays.
            if before != after {
                let mut e = event("literal_zhuyin", t, &after.composition);
                e.insert("shown_before".into(), before.composition.clone().into());
                s.events.push(e);
                correction(s, "literal_zhuyin", &before.composition);
                s.literal = Some(s.events.len() - 1);
                s.replayable = false;
                s.original_replayable = false;
                s.tracking = false;
            }
            return;
        }
        if !after.has_list() {
            let number = match key {
                KeyInput::Char(c) => self.selection_keys.iter().position(|k| *k == c),
                _ => None,
            };
            if let Some(number) = number {
                pick(s, before.page * before.page_size + number, "key", before, after, candidate, t);
            } else if key == KeyInput::Enter {
                pick(s, before.selection.unwrap_or(0), "key", before, after, candidate, t);
            } else {
                s.events.push(event("close_list", t, &after.composition));
            }
        } else if kind(after) != kind(before) {
            let mut e = event("switch_list", t, &after.composition);
            e.insert("list".into(), kind(after).into());
            s.events.push(e);
        }
    }

    fn finish(&mut self, mut s: Session, last: &str, input: &ObservedInput, handled: bool, time: f64, t: i64) {
        end_retyping(&mut s, &Presentation::default());
        if !last.is_empty() || s.sent.is_empty() {
            let mut e = event("commit", t, "");
            e.insert("text".into(), last.into());
            s.events.push(e);
        }
        let committed = format!("{}{last}", s.sent);
        if let Some((deleted, previous)) = s.after_commit.clone() {
            let all = scalars(&committed);
            let length = deleted.chars().count().min(all.len());
            if from_scalars(&all[..length]) != deleted {
                correction(&mut s, "delete_after_commit", &previous);
            }
        }
        let typing = serialize(&s.typing);
        if !s.corrections.is_empty() {
            let mut record = Map::new();
            record.insert("schema".into(), 1.into());
            record.insert("id".into(), uuid::Uuid::new_v4().to_string().to_uppercase().into());
            record.insert("date".into(), (self.today)().into());
            record.insert("app_version".into(), self.app_version.clone().into());
            let mut settings = self.settings.clone();
            set_optional(&mut settings, "punctuation_width", s.punctuation_width.clone());
            record.insert("settings".into(), Value::Object(settings));
            record.insert("keys".into(), s.keys.clone().into());
            record.insert("typing".into(), typing.clone().into());
            record.insert("replayable".into(), s.replayable.into());
            record.insert("original_typing".into(), serialize(&s.original).into());
            record.insert("original_replayable".into(), (s.replayable && s.original_replayable).into());
            record.insert("shown".into(), s.shown.clone().unwrap_or_default().into());
            record.insert("committed".into(), committed.clone().into());
            record.insert("corrections".into(), json!(s.corrections));
            record.insert("events".into(), Value::Array(s.events.drain(..).map(Value::Object).collect()));
            if let Some(context) = &s.context {
                record.insert("context".into(), context.clone().into());
            }
            (self.sink)(record);
        }
        // What the app received after the commit: Backspace deletes it first.
        let passed = match (handled, input) {
            (false, ObservedInput::Key { input: KeyInput::Space, .. }) => " ",
            (false, ObservedInput::Key { input: KeyInput::Enter | KeyInput::ShiftEnter, .. }) => "\n",
            (false, ObservedInput::Key { .. }) => {
                self.last_commit = None;
                return;
            }
            _ => "",
        };
        self.last_commit = Some(LastCommit {
            time,
            text: format!("{committed}{passed}"),
            committed,
            passed: passed.chars().count(),
            deleted: 0,
            typing,
            replayable: s.replayable,
            context: s.context,
            punctuation_width: s.punctuation_width,
        });
    }

    fn follow_deletion(&mut self, time: f64) {
        let window = self.window;
        match &mut self.last_commit {
            Some(last) if time - last.time < window && last.deleted < last.text.chars().count() => last.deleted += 1,
            _ => self.last_commit = None,
        }
    }

    fn key_label(&self, input: &ObservedInput, before: &Presentation) -> Option<String> {
        match *input {
            ObservedInput::Key { input, .. } => keys::label(input),
            ObservedInput::ToggleCorrections => Some("<sup>".into()),
            ObservedInput::Commit => Some("<enter>".into()),
            ObservedInput::Click { index } => {
                // A click is written as the selection key at that place on the page, which picks the same item.
                if !before.has_list() {
                    return None;
                }
                let number = index.checked_sub(before.page * before.page_size)?;
                self.selection_keys.get(number).map(char::to_string)
            }
        }
    }

    fn is_list_key(&self, key: KeyInput) -> bool {
        match key {
            KeyInput::Char(c) => c == '`' || self.selection_keys.contains(&c),
            KeyInput::Enter
            | KeyInput::ShiftDown
            | KeyInput::ShiftUp
            | KeyInput::Escape
            | KeyInput::Backspace
            | KeyInput::Left
            | KeyInput::Right
            | KeyInput::Up
            | KeyInput::Down
            | KeyInput::PageUp
            | KeyInput::PageDown
            | KeyInput::Tab
            | KeyInput::ShiftTab
            | KeyInput::Space => true,
            _ => false,
        }
    }
}

fn pick(
    s: &mut Session,
    index: usize,
    via: &str,
    before: &Presentation,
    after: &Presentation,
    candidate: Option<CandidateDetails>,
    t: i64,
) {
    if !before.has_list() || after.has_list() {
        return;
    }
    end_retyping(s, after);
    s.literal = None;
    let mut e = event("pick", t, &after.composition);
    let (start, removed, inserted) = difference(&before.composition, &after.composition);
    e.insert("before".into(), before.composition.clone().into());
    e.insert("after".into(), after.composition.clone().into());
    e.insert("start".into(), start.into());
    e.insert("removed".into(), removed.into());
    e.insert("inserted".into(), inserted.into());
    e.insert("rank".into(), (index + 1).into());
    if let Some(candidate) = candidate {
        e.insert("candidate_text".into(), candidate.text.into());
        e.insert("candidate_reading".into(), candidate.reading.into());
        e.insert("candidate_start".into(), candidate.start.into());
        e.insert("candidate_end".into(), candidate.end.into());
    }
    e.insert("list".into(), kind(before).into());
    e.insert("via".into(), via.into());
    s.events.push(e);
    if before.composition != after.composition {
        correction(s, "pick", &before.composition);
    }
}

fn composing_key(
    s: &mut Session,
    key: KeyInput,
    label: Option<&str>,
    before: &Presentation,
    after: &Presentation,
    committed: bool,
    t: i64,
) {
    let literal = s.literal.take();
    if s.selecting {
        let last_select = s.last_type() == Some("select");
        if matches!(key, KeyInput::ShiftLeft | KeyInput::ShiftRight) && last_select {
            let last = s.events.last_mut().expect("a select event");
            set_optional(last, "selected", after.focused_text());
            last.insert("shown".into(), after.composition.clone().into());
            return;
        }
        if key == KeyInput::Escape {
            s.selecting = false;
            s.events.push(event("cancel_select", t, &after.composition));
            return;
        }
        if key == KeyInput::Enter && !committed {
            s.selecting = false;
            let mut e = event("add_phrase", t, &after.composition);
            if let Some(selected) = s.events.last().and_then(|e| e.get("selected")).cloned() {
                e.insert("phrase".into(), selected);
            }
            let added = after.notice.as_deref().is_some_and(|n| n.starts_with("已加入"));
            e.insert("added".into(), added.into());
            s.events.push(e);
            if added {
                correction(s, "add_phrase", &before.composition);
            }
            return;
        }
        s.selecting = false;
    }
    if committed {
        // The key that sent the composition is not typing; a replay ends with <enter>.
        end_retyping(s, after);
        return;
    }
    match key {
        KeyInput::ShiftLeft | KeyInput::ShiftRight => {
            end_retyping(s, after);
            s.selecting = true;
            let mut e = event("select", t, &after.composition);
            e.insert("shown_before".into(), before.composition.clone().into());
            set_optional(&mut e, "selected", after.focused_text());
            s.events.push(e);
            correction(s, "select", &before.composition);
            return;
        }
        KeyInput::Left | KeyInput::Right | KeyInput::Home | KeyInput::End if before.cursor != after.cursor => {
            end_retyping(s, after);
            let position = scalar_offset(&after.composition, after.cursor);
            if s.last_type() == Some("cursor") {
                let last = s.events.last_mut().expect("a cursor event");
                last.insert("position".into(), position.into());
                last.insert("t".into(), t.into());
            } else {
                let mut e = event("cursor", t, &after.composition);
                e.insert("position".into(), position.into());
                s.events.push(e);
            }
            return;
        }
        _ => {}
    }
    if !before.has_list() && after.has_list() {
        // Opening the list may first finish the unfinished syllable: keep the key, closed again, as typing.
        end_retyping(s, after);
        if before.composition != after.composition
            && let Some(label) = label
        {
            let key = TypedKey { label: format!("{label}<esc>"), time: s.key_time() };
            append_typing(s, key, after);
        }
        let mut e = event("open_list", t, &after.composition);
        e.insert("list".into(), kind(after).into());
        set_optional(&mut e, "target", after.focused_text());
        s.events.push(e);
        return;
    }
    let mid = !at_end(before) || !at_end(after);
    let content = is_content(key);
    if !content && before.composition == after.composition {
        return;
    }
    if mid {
        s.tracking = false;
        s.replayable = false;
        s.original_replayable = false;
        if content {
            correction(s, "cursor_edit", &before.composition);
        }
    }
    let typed = TypedKey { label: label.unwrap_or("<?>").to_owned(), time: s.key_time() };
    // Esc or ` right after ` restores the Zhuyin as it was.
    if let Some(literal) = literal
        && matches!(key, KeyInput::Char('`') | KeyInput::Escape)
        && before != after
        && !after.is_empty()
        && after.composition == str_field(&s.events[literal], "shown_before")
    {
        let mut e = event("undo_literal", t, &after.composition);
        e.insert("shown_before".into(), before.composition.clone().into());
        s.events.push(e);
        correction(s, "undo_literal", &before.composition);
        append_typing(s, typed, after);
        return;
    }
    match key {
        KeyInput::Char('`') => {
            if before == after {
                return;
            }
            end_retyping(s, after);
            let mut e = event("literal_zhuyin", t, &after.composition);
            e.insert("shown_before".into(), before.composition.clone().into());
            if mid {
                e.insert("mid".into(), true.into());
            }
            s.events.push(e);
            correction(s, "literal_zhuyin", &before.composition);
            append_typing(s, TypedKey { label: "`".into(), time: s.key_time() }, after);
            s.literal = Some(s.events.len() - 1);
            return;
        }
        KeyInput::Backspace | KeyInput::OptionBackspace | KeyInput::ForwardDelete | KeyInput::Escape => {
            deletion(s, key, label, before, after, mid, t);
            return;
        }
        _ => {}
    }
    typed_key(s, typed, after, mid, t);
    if label.is_none() {
        s.replayable = false;
        s.original_replayable = false;
    }
}

fn typed_key(s: &mut Session, key: TypedKey, after: &Presentation, mid: bool, t: i64) {
    let continues = s.last_type() == Some("type")
        && s.events.last().and_then(|e| e.get("mid")).and_then(Value::as_bool).unwrap_or(false) == mid;
    if continues {
        let last = s.events.last_mut().expect("a type event");
        let keys = format!("{}{}", str_field(last, "keys"), key.label);
        last.insert("keys".into(), keys.into());
        last.insert("shown".into(), after.composition.clone().into());
        last.insert("t".into(), t.into());
    } else {
        let mut e = event("type", t, &after.composition);
        e.insert("keys".into(), key.label.clone().into());
        if mid {
            e.insert("mid".into(), true.into());
        }
        s.events.push(e);
    }
    append_typing(s, key, after);
    if s.retyping.as_ref().is_some_and(|r| Shape::of(&after.composition).converted >= r.target) {
        end_retyping(s, after);
    }
}

fn append_typing(s: &mut Session, key: TypedKey, after: &Presentation) {
    s.typing.push(key.clone());
    match &mut s.retyping {
        Some(retyping) => retyping.keys.push(key),
        None => s.original.push(key),
    }
    if s.tracking {
        s.history.push((s.typing.len(), Shape::of(&after.composition)));
    }
}

fn deletion(
    s: &mut Session,
    key: KeyInput,
    label: Option<&str>,
    before: &Presentation,
    after: &Presentation,
    mid: bool,
    t: i64,
) {
    if before == after {
        return;
    }
    let kind = if key == KeyInput::Escape { "escape" } else { "backspace" };
    let cleared =
        (key == KeyInput::Escape).then(|| if after.is_empty() { "composition" } else { "symbols" }.to_owned());
    let removed = difference(&before.composition, &after.composition).1;
    let shape = Shape::of(&after.composition);
    // The typing keys behind what was removed: back to the last time the screen had the shape it has now.
    let index = s.history.iter().rposition(|(_, h)| *h == shape);
    let usable = index.filter(|&i| s.tracking && !mid && s.history[i].0 < s.typing.len());
    let Some(index) = usable else {
        end_retyping(s, after);
        let typed = TypedKey { label: label.unwrap_or("<?>").to_owned(), time: s.key_time() };
        s.typing.push(typed.clone());
        s.original.push(typed);
        if s.tracking {
            s.history.push((s.typing.len(), shape));
        }
        let mut e = event(kind, t, &after.composition);
        e.insert("shown_before".into(), before.composition.clone().into());
        e.insert("removed".into(), removed.into());
        set_optional(&mut e, "cleared", cleared);
        if mid {
            e.insert("mid".into(), true.into());
        }
        s.events.push(e);
        if mid {
            correction(s, "cursor_edit", &before.composition);
        }
        return;
    };
    let mark = s.history[index].0;
    let deleted: Vec<TypedKey> = s.typing.split_off(mark);
    if deleted.iter().any(|k| k.label == "<punct-width>") {
        // Edits across a width toggle need state-aware deletion snapshots before they can be replayed.
        s.replayable = false;
        s.original_replayable = false;
    }
    s.history.truncate(index + 1);
    let deleted_keys: String = deleted.iter().map(|k| k.label.as_str()).collect();
    let last = s.events.len().wrapping_sub(1);
    let last_is_kind = s.events.last().is_some_and(|e| str_field(e, "type") == kind);
    if let Some(retyping) = &mut s.retyping {
        if retyping.keys.is_empty() && retyping.event == last && last_is_kind {
            // Backspace held or pressed again: one deletion.
            retyping.position = shape.converted;
            let e = &mut s.events[last];
            let keys = format!("{deleted_keys}{}", str_field(e, "deleted_keys"));
            e.insert("deleted_keys".into(), keys.into());
            let removed = difference(str_field(e, "shown_before"), &after.composition).1;
            e.insert("removed".into(), removed.into());
            e.insert("shown".into(), after.composition.clone().into());
            e.insert("t".into(), t.into());
            set_optional(e, "cleared", cleared);
            return;
        }
        if deleted.len() <= retyping.keys.len() {
            // A slip while retyping: the retyped keys lose it and the correction goes on.
            let keep = retyping.keys.len() - deleted.len();
            retyping.keys.truncate(keep);
            let mut e = event(kind, t, &after.composition);
            e.insert("shown_before".into(), before.composition.clone().into());
            e.insert("removed".into(), removed.into());
            e.insert("deleted_keys".into(), deleted_keys.into());
            set_optional(&mut e, "cleared", cleared);
            s.events.push(e);
            return;
        }
        // Deleting past the retyped keys: the keys as first typed can no longer be told apart.
        s.original_replayable = false;
    }
    end_retyping(s, after);
    let mut e = event(kind, t, &after.composition);
    e.insert("shown_before".into(), before.composition.clone().into());
    e.insert("removed".into(), removed.into());
    e.insert("deleted_keys".into(), deleted_keys.into());
    set_optional(&mut e, "cleared", cleared);
    s.events.push(e);
    let was = Shape::of(&before.composition);
    s.retyping = Some(Retyping {
        event: s.events.len() - 1,
        target: was.converted + usize::from(!was.pending.is_empty()),
        position: shape.converted,
        keys: Vec::new(),
    });
}

fn end_retyping(s: &mut Session, after: &Presentation) {
    let Some(retyping) = s.retyping.take() else { return };
    let e = s.events[retyping.event].clone();
    if retyping.keys.is_empty() {
        let deleted = str_field(&e, "deleted_keys");
        let labels: String = s.original.iter().map(|k| k.label.as_str()).collect();
        if labels.ends_with(deleted) {
            let mut remaining = deleted.len();
            while remaining > 0 {
                let Some(key) = s.original.pop() else { break };
                remaining = remaining.saturating_sub(key.label.len());
            }
        } else {
            s.original_replayable = false;
        }
        return;
    }
    let retyped: String = retyping.keys.iter().map(|k| k.label.as_str()).collect();
    s.events[retyping.event].insert("retyped_keys".into(), retyped.into());
    let all = scalars(&after.composition);
    let end = all.len() - Shape::of(&after.composition).pending.chars().count();
    if retyping.position <= end {
        let replaced = from_scalars(&all[retyping.position..end]);
        s.events[retyping.event].insert("replaced_by".into(), replaced.into());
    }
    let shown = if e.contains_key("shown_before") { str_field(&e, "shown_before") } else { str_field(&e, "shown") };
    correction(s, str_field(&e, "type"), shown);
}

fn correction(s: &mut Session, kind: &str, shown: &str) {
    if s.shown.is_none() {
        s.shown = Some(format!("{}{shown}", s.sent));
    }
    if !s.corrections.iter().any(|c| c == kind) {
        s.corrections.push(kind.to_owned());
    }
}

fn carry(s: &mut Session, committed: &str, after: &Presentation, t: i64) {
    let mut e = event("commit", t, &after.composition);
    e.insert("text".into(), committed.into());
    s.events.push(e);
    s.sent.push_str(committed);
    // A deletion can no longer go back past the head: from here, deleting keys are recorded as typed.
    if s.tracking {
        s.history = vec![(s.typing.len(), Shape::of(&after.composition))];
    }
}

fn serialize(keys: &[TypedKey]) -> String {
    let mut result = String::new();
    let mut previous: Option<f64> = None;
    for key in keys {
        if let Some(previous) = previous {
            let _ = write!(result, "<dt:{}>", milliseconds(key.time - previous));
        }
        result.push_str(&key.label);
        previous = Some(key.time);
    }
    result
}

// MARK: - The file

/// A record line: compact JSON with every object's keys sorted (the macOS `JSONEncoder` with `.sortedKeys`;
/// slashes are not escaped).
#[must_use]
pub fn encode(record: &Value) -> String {
    fn write(value: &Value, out: &mut String) {
        match value {
            Value::Object(object) => {
                let mut keys: Vec<&String> = object.keys().collect();
                keys.sort();
                out.push('{');
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push_str(&Value::String(key.clone()).to_string());
                    out.push(':');
                    write(&object[key], out);
                }
                out.push('}');
            }
            Value::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write(item, out);
                }
                out.push(']');
            }
            scalar => out.push_str(&scalar.to_string()),
        }
    }
    let mut out = String::new();
    write(record, &mut out);
    out
}

/// `corrections.jsonl`: appended, counted, exported and cleared for the 進階 page.  Cheap to clone; clones share it.
#[derive(Clone)]
pub struct CorrectionLog {
    inner: Arc<Mutex<Log>>,
}

struct Log {
    path: Option<PathBuf>,
    memory: Vec<Value>,
    count: usize,
    error: Option<String>,
}

impl CorrectionLog {
    /// The log in `directory` (`None`: records are kept in memory, for practice sessions and tests).
    #[must_use]
    pub fn open(directory: Option<&Path>) -> Self {
        let path = directory.map(|d| d.join(FILE_NAME));
        let log = Self { inner: Arc::new(Mutex::new(Log { path, memory: Vec::new(), count: 0, error: None })) };
        log.recount();
        log
    }

    fn recount(&self) {
        let mut log = self.inner.lock();
        let Some(path) = log.path.clone() else { return };
        let mut lines = 0;
        if let Ok(mut file) = std::fs::File::open(path) {
            let mut buffer = vec![0; 1 << 16];
            while let Ok(read) = file.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                lines += buffer[..read].iter().filter(|b| **b == b'\n').count();
            }
        }
        log.count = lines;
    }

    /// How many records the log has.
    #[must_use]
    pub fn count(&self) -> usize {
        self.inner.lock().count
    }

    /// The file, when the log is on disk.
    #[must_use]
    pub fn location(&self) -> Option<PathBuf> {
        self.inner.lock().path.clone()
    }

    /// Appends one record.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn append(&self, record: &Value) -> Result<()> {
        let mut log = self.inner.lock();
        let Some(path) = log.path.clone() else {
            log.memory.push(record.clone());
            log.count += 1;
            return Ok(());
        };
        let line = encode(record) + "\n";
        let written = (|| -> std::io::Result<bool> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut options = OpenOptions::new();
            options.create(true).append(true).read(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(&path)?;
            // A record cut short (the process ended while writing it) is ended here, so this one is a line of its
            // own and stays readable.
            let mut last = [0u8; 1];
            let length = file.metadata()?.len();
            let torn = length > 0 && {
                use std::io::Seek as _;
                file.seek(std::io::SeekFrom::Start(length - 1))?;
                file.read_exact(&mut last)?;
                last[0] != b'\n'
            };
            let line = if torn { format!("\n{line}") } else { line };
            file.write_all(line.as_bytes())?;
            Ok(torn)
        })();
        let Ok(torn) = written else {
            log.error = Some("無法寫入改字記錄。".to_owned());
            return Err(anyhow!("無法寫入改字記錄。"));
        };
        // The cut-short record is a line now too.
        log.count += 1 + usize::from(torn);
        log.error = None;
        Ok(())
    }

    /// Copies the log to `destination`.
    ///
    /// # Errors
    /// When it cannot be read or written.
    pub fn export(&self, destination: &Path) -> Result<()> {
        let mut log = self.inner.lock();
        let bytes = match &log.path {
            Some(path) if path.exists() => match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    log.error = Some("無法讀取改字記錄。".to_owned());
                    return Err(anyhow!("無法讀取改字記錄。"));
                }
            },
            Some(_) => Vec::new(),
            None => log.memory.iter().map(|r| encode(r) + "\n").collect::<String>().into_bytes(),
        };
        if settings::write_atomically(destination, &bytes).is_err() {
            log.error = Some("無法匯出改字記錄。".to_owned());
            return Err(anyhow!("無法匯出改字記錄。"));
        }
        Ok(())
    }

    /// Deletes every record.
    ///
    /// # Errors
    /// When the file cannot be deleted.
    pub fn clear(&self) -> Result<()> {
        {
            let mut log = self.inner.lock();
            log.memory.clear();
            if let Some(path) = log.path.clone()
                && path.exists()
                && std::fs::remove_file(&path).is_err()
            {
                log.error = Some("無法清除改字記錄。".to_owned());
                return Err(anyhow!("無法清除改字記錄。"));
            }
            log.count = 0;
        }
        self.recount();
        Ok(())
    }

    /// The records kept in memory (a log opened without a directory).
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        self.inner.lock().memory.clone()
    }

    /// Why the last operation failed, if it did.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.inner.lock().error.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences_and_sorted_encoding() {
        assert_eq!(difference("你好嗎", "妳好嗎"), (0, "你".into(), "妳".into()));
        assert_eq!(difference("abc", "abc"), (3, String::new(), String::new()));
        assert_eq!(Shape::of("你好ㄇㄚ"), Shape { converted: 2, pending: "ㄇㄚ".into() });
        let record = json!({"z": 1, "a": {"y": "a/b", "b": [true, null]}});
        assert_eq!(encode(&record), r#"{"a":{"b":[true,null],"y":"a/b"},"z":1}"#);
    }

    #[test]
    fn a_record_cut_short_by_a_crash_does_not_swallow_the_next() {
        let dir = std::env::temp_dir().join(format!("zt-corrections-torn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // The process ended (power loss) halfway through writing a record.
        std::fs::write(dir.join(FILE_NAME), "{\"schema\":1}\n{\"schema\":1,\"comm").unwrap();
        let log = CorrectionLog::open(Some(&dir));
        log.append(&json!({"schema": 1, "committed": "你好"})).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        let last = text.lines().last().unwrap();
        assert_eq!(serde_json::from_str::<Value>(last).unwrap()["committed"], "你好");
        assert!(text.ends_with('\n'));
        assert_eq!(log.count(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

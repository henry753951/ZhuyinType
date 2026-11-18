//! The broker protocol as the text service speaks it (platforms/desktop/IPC.md): one line of UTF-8 JSON per request
//! and per reply, at most 256 KiB.  Pure, so it is tested on any system.

use serde_json::{Map, Value};

use crate::quick_menu::QuickState;

/// The largest frame either side sends, newline included.
pub(crate) const MAX_FRAME: usize = 256 * 1024;

/// The candidate window in a reply (`IPC.md` §候選字視窗).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum PanelUpdate {
    /// Left out: as in the last reply.
    #[default]
    Unchanged,
    /// `null`: no window.
    Hidden,
    Show(Box<zt_panel::style::Panel>),
}

impl PanelUpdate {
    /// Whether 符號與 Emoji's grid is on screen once this update is applied over a window that showed it or not
    /// (`before`).
    pub(crate) fn shows_picker(&self, before: bool) -> bool {
        match self {
            Self::Unchanged => before,
            Self::Hidden => false,
            Self::Show(panel) => panel.picker.is_some(),
        }
    }
}

/// What the text service needs from a reply.  Text is UTF-16, the unit TSF ranges count in.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Reply {
    pub(crate) handled: bool,
    pub(crate) candidates: bool,
    pub(crate) english: bool,
    /// A composition or a candidate list is open; the text service then claims every key.
    pub(crate) composing: bool,
    /// A 聯想詞 list is shown but not entered (`composing` is false): the text service claims ↓ too, and tells the
    /// host about every other key it leaves to the application, which closes the list.  Older hosts leave it out.
    pub(crate) associated: bool,
    pub(crate) commit: Vec<u16>,
    pub(crate) preedit: Vec<u16>,
    /// The caret inside `preedit`, in UTF-16 units, never inside a surrogate pair.
    pub(crate) cursor: usize,
    /// The words of `preedit` (UTF-16 ranges, in order, not overlapping), each underlined.
    pub(crate) clauses: Vec<(usize, usize)>,
    /// The word the candidate list replaces, set apart from the others.
    pub(crate) focus: Option<usize>,
    /// The word ↓ would change while no list is open.
    pub(crate) target: Option<usize>,
    /// The parts of `preedit` (UTF-16 ranges, in order, not overlapping) converted through an automatic typing
    /// correction or a fuzzy sound, marked with a dotted underline; empty when the user turned the marks off.
    pub(crate) corrections: Vec<(usize, usize)>,
    /// The candidate window, which the text service draws.
    pub(crate) panel: PanelUpdate,
    /// The 中/英 badge to show by the caret.
    pub(crate) toast: Option<String>,
    /// A page turn stopped at the list's end (1 past the last, -1 before the first): the page nudges.
    pub(crate) boundary: Option<i32>,
    /// 簡體 output (the 中 badge shows 简); older hosts leave it out.
    pub(crate) simplified: bool,
    /// 暫停學習 is on (the button's tooltip says so); older hosts leave it out.
    pub(crate) learning_paused: bool,
    /// The four switches, in a reply to the `quick` op.
    pub(crate) quick: Option<QuickState>,
    /// The `quickSettings` shortcut was pressed: the text service shows its quick menu at the caret.
    pub(crate) quick_panel: bool,
    /// 選取的字加入自訂詞: send the field's selected text next (op `selection`).
    pub(crate) selection: bool,
    /// The keys to claim even when nothing is composed (IPC.md §攔截的按鍵): `(key, modifiers)`, already filtered by
    /// the host for this field and mode; `None`: left out (unchanged since the last), keep the last ones.
    pub(crate) claims: Option<Vec<(i32, i32)>>,
}

impl Reply {
    /// A reply that only commits `text` (a character the host declined and the text service types itself).
    pub(crate) fn commit_only(text: Vec<u16>, english: bool) -> Self {
        Self { commit: text, english, ..Self::default() }
    }
}

/// The request line for `op`, with the version, id and context added to `fields`; `None` if it would not fit in a
/// frame.
pub(crate) fn request_line(id: u64, context: &str, op: &str, mut fields: Map<String, Value>) -> Option<Vec<u8>> {
    fields.insert("v".into(), Value::from(1));
    fields.insert("id".into(), Value::from(id));
    fields.insert("context".into(), Value::from(context));
    fields.insert("op".into(), Value::from(op));
    let mut line = serde_json::to_vec(&Value::Object(fields)).ok()?;
    if line.len() + 1 > MAX_FRAME {
        return None;
    }
    line.push(b'\n');
    Some(line)
}

/// Where a reply being read stands.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    /// No newline yet; read more.
    Incomplete,
    /// Exactly one line, ending at the last byte.
    Complete,
    /// Too long, or more than one line: the connection is out of step and must be closed.
    Invalid,
}

/// Classifies the bytes read so far for one reply.
pub(crate) fn frame(bytes: &[u8]) -> Frame {
    match bytes.iter().position(|&byte| byte == b'\n') {
        Some(end) if end + 1 == bytes.len() && bytes.len() <= MAX_FRAME => Frame::Complete,
        Some(_) => Frame::Invalid,
        None if bytes.len() >= MAX_FRAME => Frame::Invalid,
        None => Frame::Incomplete,
    }
}

/// Parses and checks one reply line (newline included): the version, the id and the context must be the request's,
/// every field must have its type, and the cursor must fall on a character boundary of the composition.
pub(crate) fn parse_reply(line: &[u8], id: u64, context: &str) -> Option<Reply> {
    if frame(line) != Frame::Complete {
        return None;
    }
    let value: Value = serde_json::from_slice(&line[..line.len() - 1]).ok()?;
    let object = value.as_object()?;
    let number = |key: &str| object.get(key).and_then(Value::as_f64);
    let boolean = |key: &str| object.get(key).and_then(Value::as_bool);
    let text = |key: &str| object.get(key).and_then(Value::as_str);
    // Ids stay below 2^53, so they are exact as JSON numbers.
    if number("v")? != 1.0 || number("id")? != id as f64 || text("context")? != context || !boolean("ok")? {
        return None;
    }
    if object.contains_key("error") {
        text("error")?;
    }
    let preedit: Vec<u16> = text("preedit")?.encode_utf16().collect();
    let cursor = number("cursor")?;
    if !cursor.is_finite() || cursor.fract() != 0.0 || cursor < 0.0 || cursor > preedit.len() as f64 {
        return None;
    }
    let cursor = cursor as usize;
    if cursor > 0 && cursor < preedit.len() && (0xdc00..=0xdfff).contains(&preedit[cursor]) {
        return None;
    }
    let candidates = boolean("candidates")?;
    // Older hosts send no `composing`; their composition is the preedit.
    let composing = match object.get("composing") {
        Some(value) => value.as_bool()?,
        None => !preedit.is_empty() || candidates,
    };
    let panel = match object.get("panel") {
        None => PanelUpdate::Unchanged,
        Some(Value::Null) => PanelUpdate::Hidden,
        Some(value) => PanelUpdate::Show(Box::new(serde_json::from_value(value.clone()).ok()?)),
    };
    let toast = match object.get("toast") {
        None => None,
        Some(value) => Some(value.as_str().filter(|text| text.chars().count() <= 4)?.to_owned()),
    };
    // Left out by older hosts and with no composition; a bad value fails the reply like any other field.
    let clauses = match object.get("clauses") {
        Some(value) => ranges(value, preedit.len())?,
        None => Vec::new(),
    };
    let clause = |key: &str| -> Option<Option<usize>> {
        match object.get(key) {
            None | Some(Value::Null) => Some(None),
            Some(value) => Some(Some(usize::try_from(value.as_u64()?).ok().filter(|&i| i < clauses.len())?)),
        }
    };
    let focus = clause("focus")?;
    let target = clause("target")?;
    // Only a mark: left out by older hosts, with no composition and with the marks off, and a bad value drops the
    // marks rather than the key.
    let corrections = object.get("corrections").and_then(|value| ranges(value, preedit.len())).unwrap_or_default();
    let boundary = match object.get("boundary") {
        None => None,
        Some(value) => Some(value.as_i64().filter(|d| d.abs() == 1)? as i32),
    };
    // Left out by older hosts; a value of the wrong type fails the reply.
    let optional = |key: &str| -> Option<bool> {
        match object.get(key) {
            None => Some(false),
            Some(value) => value.as_bool(),
        }
    };
    let quick = match object.get("quick") {
        None => None,
        Some(value) => {
            let quick = value.as_object()?;
            let flag = |key: &str| quick.get(key).and_then(Value::as_bool);
            Some(QuickState {
                english: flag("english")?,
                full_width: flag("fullWidth")?,
                simplified: flag("simplified")?,
                learning_paused: flag("learningPaused")?,
                symbols: match quick.get("symbols") {
                    None => false,
                    Some(value) => value.as_bool()?,
                },
            })
        }
    };
    let selection = match object.get("selection") {
        None => false,
        Some(value) => value.as_bool()?,
    };
    let claims = match object.get("claims") {
        None => None,
        Some(value) => {
            let mut keys = Vec::new();
            for pair in value.as_array().filter(|list| list.len() <= 64)? {
                let pair = pair.as_array().filter(|pair| pair.len() == 2)?;
                keys.push((i32::try_from(pair[0].as_i64()?).ok()?, i32::try_from(pair[1].as_i64()?).ok()?));
            }
            Some(keys)
        }
    };
    Some(Reply {
        handled: boolean("handled")?,
        candidates,
        english: boolean("english")?,
        composing,
        associated: optional("associated")?,
        commit: text("commit")?.encode_utf16().collect(),
        preedit,
        cursor,
        clauses,
        focus,
        target,
        corrections,
        panel,
        toast,
        boundary,
        simplified: optional("simplified")?,
        learning_paused: optional("learningPaused")?,
        quick,
        quick_panel: optional("quickPanel")?,
        selection,
        claims,
    })
}

/// `[[start, end], ...]`: ranges of a text `length` units long, in order, not empty and not overlapping.
fn ranges(value: &Value, length: usize) -> Option<Vec<(usize, usize)>> {
    let mut list = Vec::new();
    let mut end = 0;
    for range in value.as_array()? {
        let pair = range.as_array().filter(|pair| pair.len() == 2)?;
        let start = usize::try_from(pair[0].as_u64()?).ok()?;
        let stop = usize::try_from(pair[1].as_u64()?).ok()?;
        if start < end || stop <= start || stop > length {
            return None;
        }
        list.push((start, stop));
        end = stop;
    }
    Some(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn good() -> Value {
        json!({"v": 1, "id": 7, "context": "{A}", "ok": true, "handled": true, "candidates": false,
               "english": false, "commit": "", "preedit": "ㄓ", "cursor": 1})
    }

    #[test]
    fn request_carries_identity() {
        let mut fields = Map::new();
        fields.insert("secure".into(), Value::from(false));
        let bytes = request_line(3, "{C}", "focus", fields).unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value, json!({"v": 1, "id": 3, "context": "{C}", "op": "focus", "secure": false}));
    }

    #[test]
    fn request_too_large_is_refused() {
        let mut fields = Map::new();
        fields.insert("before".into(), Value::from("x".repeat(MAX_FRAME)));
        assert!(request_line(1, "c", "context", fields).is_none());
    }

    #[test]
    fn claims_are_parsed() {
        let mut value = good();
        assert_eq!(parse_reply(&line(value.clone()), 7, "{A}").unwrap().claims, None);
        value["claims"] = json!([[44, 0x0c00_0000]]);
        assert_eq!(parse_reply(&line(value.clone()), 7, "{A}").unwrap().claims, Some(vec![(44, 0x0c00_0000)]));
        value["claims"] = json!([]);
        assert_eq!(parse_reply(&line(value.clone()), 7, "{A}").unwrap().claims, Some(vec![]));
        value["claims"] = json!([[44]]);
        assert!(parse_reply(&line(value.clone()), 7, "{A}").is_none());
        value["claims"] = json!([["Q", 0]]);
        assert!(parse_reply(&line(value), 7, "{A}").is_none());
    }

    #[test]
    fn passive_suggestions_are_parsed() {
        let mut value = good();
        assert!(!parse_reply(&line(value.clone()), 7, "{A}").unwrap().associated);
        value["associated"] = json!(true);
        assert!(parse_reply(&line(value.clone()), 7, "{A}").unwrap().associated);
        value["associated"] = json!(1);
        assert!(parse_reply(&line(value), 7, "{A}").is_none());
    }

    #[test]
    fn reply_is_parsed() {
        let reply = parse_reply(&line(good()), 7, "{A}").unwrap();
        assert!(reply.handled && reply.composing && !reply.english);
        assert_eq!(reply.preedit, "ㄓ".encode_utf16().collect::<Vec<_>>());
        assert_eq!(reply.cursor, 1);
    }

    #[test]
    fn reply_identity_is_checked() {
        assert!(parse_reply(&line(good()), 8, "{A}").is_none());
        assert!(parse_reply(&line(good()), 7, "{B}").is_none());
        let mut wrong = good();
        wrong["v"] = json!(2);
        assert!(parse_reply(&line(wrong), 7, "{A}").is_none());
        let mut failed = good();
        failed["ok"] = json!(false);
        assert!(parse_reply(&line(failed), 7, "{A}").is_none());
    }

    #[test]
    fn reply_types_are_checked() {
        for (key, value) in
            [("handled", json!(1)), ("commit", json!(null)), ("cursor", json!("1")), ("error", json!(3))]
        {
            let mut wrong = good();
            wrong[key] = value;
            assert!(parse_reply(&line(wrong), 7, "{A}").is_none(), "{key}");
        }
        let mut missing = good();
        missing.as_object_mut().unwrap().remove("english");
        assert!(parse_reply(&line(missing), 7, "{A}").is_none());
    }

    #[test]
    fn panel_toast_and_boundary() {
        let reply = parse_reply(&line(good()), 7, "{A}").unwrap();
        assert_eq!((reply.panel, reply.toast, reply.boundary), (PanelUpdate::Unchanged, None, None));
        let mut hidden = good();
        hidden["panel"] = json!(null);
        assert_eq!(parse_reply(&line(hidden), 7, "{A}").unwrap().panel, PanelUpdate::Hidden);
        let panel = zt_panel::style::Panel {
            model: zt_panel::model::PanelModel::default(),
            style: zt_panel::style::PanelStyle::default(),
            appearance: zt_panel::style::Appearance::Dark,
            translucent: true,
            animates: true,
            above: false,
            accent: zt_panel::a11y::AccentChoice::Zhuyintype,
            picker: None,
        };
        let mut shown = good();
        shown["panel"] = serde_json::to_value(&panel).unwrap();
        shown["toast"] = json!("英");
        shown["boundary"] = json!(-1);
        let reply = parse_reply(&line(shown), 7, "{A}").unwrap();
        assert_eq!(reply.panel, PanelUpdate::Show(Box::new(panel.clone())));
        assert_eq!((reply.toast.as_deref(), reply.boundary), (Some("英"), Some(-1)));
        // 符號與 Emoji's grid rides along in the panel.
        let mut with_grid = panel.clone();
        with_grid.picker = Some(zt_panel::picker::PickerView {
            tabs: vec![zt_panel::picker::Tab { id: "smileys".into(), title: "笑臉與情緒".into() }],
            tab: Some(0),
            query: String::new(),
            prefilled: false,
            first_row: 0,
            row_count: 1,
            items: vec![zt_panel::picker::Item { text: "😀".into(), name: "笑臉".into() }],
            highlight: Some(0),
            numbered: false,
            hint: "↓ 選取・Esc 關閉".into(),
        });
        let mut grid = good();
        grid["panel"] = serde_json::to_value(&with_grid).unwrap();
        let grid = parse_reply(&line(grid), 7, "{A}").unwrap().panel;
        assert_eq!(grid, PanelUpdate::Show(Box::new(with_grid)));
        // Whether the grid is on screen after each kind of update (the text service then takes every key).
        for before in [false, true] {
            assert!(grid.shows_picker(before));
            assert!(!PanelUpdate::Show(Box::new(panel.clone())).shows_picker(before), "a candidate list");
            assert!(!PanelUpdate::Hidden.shows_picker(before));
            assert_eq!(PanelUpdate::Unchanged.shows_picker(before), before, "left out: as it was");
        }
        for (key, value) in [("panel", json!(3)), ("toast", json!(1)), ("boundary", json!(2))] {
            let mut bad = good();
            bad[key] = value;
            assert!(parse_reply(&line(bad), 7, "{A}").is_none(), "{key}");
        }
    }

    #[test]
    fn clauses_focus_and_target() {
        let reply = parse_reply(&line(good()), 7, "{A}").unwrap();
        assert_eq!((reply.clauses, reply.focus, reply.target), (vec![], None, None));
        let mut marked = good();
        marked["preedit"] = json!("你好嗎");
        marked["cursor"] = json!(3);
        marked["clauses"] = json!([[0, 2], [2, 3]]);
        marked["focus"] = json!(1);
        marked["target"] = json!(null);
        let reply = parse_reply(&line(marked.clone()), 7, "{A}").unwrap();
        assert_eq!((reply.clauses, reply.focus, reply.target), (vec![(0, 2), (2, 3)], Some(1), None));
        for (key, value) in [
            ("clauses", json!([[0, 4]])),
            ("clauses", json!([[1, 1]])),
            ("clauses", json!([[0, 2], [1, 3]])),
            ("clauses", json!([[0]])),
            ("clauses", json!("0-2")),
            ("focus", json!(2)),
            ("target", json!(-1)),
        ] {
            let mut bad = marked.clone();
            bad[key] = value;
            assert!(parse_reply(&line(bad), 7, "{A}").is_none(), "{key}");
        }
    }

    #[test]
    fn corrections() {
        let reply = parse_reply(&line(good()), 7, "{A}").unwrap();
        assert_eq!(reply.corrections, vec![]);
        let mut marked = good();
        marked["preedit"] = json!("你好嗎");
        marked["cursor"] = json!(3);
        marked["clauses"] = json!([[0, 2], [2, 3]]);
        marked["corrections"] = json!([[1, 2], [2, 3]]);
        let reply = parse_reply(&line(marked.clone()), 7, "{A}").unwrap();
        assert_eq!((reply.clauses, reply.corrections), (vec![(0, 2), (2, 3)], vec![(1, 2), (2, 3)]));
        // A bad list drops the marks, not the reply.
        for value in [
            json!([[0, 4]]),
            json!([[1, 1]]),
            json!([[0, 2], [1, 3]]),
            json!([[0]]),
            json!([[-1, 1]]),
            json!("0-2"),
            json!(null),
        ] {
            let mut bad = marked.clone();
            bad["corrections"] = value.clone();
            let reply = parse_reply(&line(bad), 7, "{A}").unwrap_or_else(|| panic!("{value}"));
            assert_eq!((reply.clauses.len(), reply.corrections), (2, vec![]), "{value}");
        }
    }

    #[test]
    fn cursor_stays_on_a_character() {
        let mut beyond = good();
        beyond["cursor"] = json!(2);
        assert!(parse_reply(&line(beyond), 7, "{A}").is_none());
        let mut fraction = good();
        fraction["cursor"] = json!(0.5);
        assert!(parse_reply(&line(fraction), 7, "{A}").is_none());
        let mut surrogate = good();
        surrogate["preedit"] = json!("𠀀");
        surrogate["cursor"] = json!(1);
        assert!(parse_reply(&line(surrogate.clone()), 7, "{A}").is_none());
        surrogate["cursor"] = json!(2);
        assert_eq!(parse_reply(&line(surrogate), 7, "{A}").unwrap().cursor, 2);
    }

    #[test]
    fn composing_defaults_to_the_preedit() {
        let mut old = good();
        old.as_object_mut().unwrap().remove("composing");
        assert!(parse_reply(&line(old.clone()), 7, "{A}").unwrap().composing);
        old["preedit"] = json!("");
        old["cursor"] = json!(0);
        assert!(!parse_reply(&line(old), 7, "{A}").unwrap().composing);
    }

    #[test]
    fn quick_settings() {
        let reply = parse_reply(&line(good()), 7, "{A}").unwrap();
        assert_eq!(
            (reply.simplified, reply.learning_paused, reply.quick, reply.quick_panel),
            (false, false, None, false)
        );
        let mut quick = good();
        quick["simplified"] = json!(true);
        quick["learningPaused"] = json!(true);
        quick["quickPanel"] = json!(true);
        quick["quick"] =
            json!({"english": false, "fullWidth": true, "simplified": true, "learningPaused": true, "symbols": false});
        let reply = parse_reply(&line(quick.clone()), 7, "{A}").unwrap();
        assert!(reply.simplified && reply.learning_paused && reply.quick_panel);
        assert_eq!(
            reply.quick,
            Some(QuickState {
                english: false,
                full_width: true,
                simplified: true,
                learning_paused: true,
                symbols: false
            })
        );
        let mut no_symbols = quick.clone();
        no_symbols["quick"].as_object_mut().unwrap().remove("symbols");
        assert!(!parse_reply(&line(no_symbols), 7, "{A}").unwrap().quick.unwrap().symbols);
        for (key, value) in [
            ("simplified", json!(1)),
            ("learningPaused", json!("yes")),
            ("quickPanel", json!(null)),
            ("quick", json!(true)),
            ("quick", json!({"english": false})),
            ("quick", json!({"english": false, "fullWidth": 1, "simplified": true, "learningPaused": true})),
        ] {
            let mut bad = quick.clone();
            bad[key] = value;
            assert!(parse_reply(&line(bad), 7, "{A}").is_none(), "{key}");
        }
    }

    #[test]
    fn frames() {
        assert_eq!(frame(b"{}"), Frame::Incomplete);
        assert_eq!(frame(b"{}\n"), Frame::Complete);
        assert_eq!(frame(b"{}\n{}"), Frame::Invalid);
        assert_eq!(frame(&vec![b' '; MAX_FRAME]), Frame::Invalid);
        assert!(parse_reply(b"{\"v\":1}\n{}\n", 1, "c").is_none());
        assert!(parse_reply(b"\xff\n", 1, "c").is_none());
    }
}

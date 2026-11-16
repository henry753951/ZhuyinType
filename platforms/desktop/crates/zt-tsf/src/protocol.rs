//! The broker protocol as the text service speaks it (platforms/desktop/IPC.md): one line of UTF-8 JSON per request
//! and per reply, at most 256 KiB.  Pure, so it is tested on any system.

use serde_json::{Map, Value};

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

/// What the text service needs from a reply.  Text is UTF-16, the unit TSF ranges count in.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Reply {
    pub(crate) handled: bool,
    pub(crate) candidates: bool,
    pub(crate) english: bool,
    /// A composition or a candidate list is open; the text service then claims every key.
    pub(crate) composing: bool,
    pub(crate) commit: Vec<u16>,
    pub(crate) preedit: Vec<u16>,
    /// The caret inside `preedit`, in UTF-16 units, never inside a surrogate pair.
    pub(crate) cursor: usize,
    /// The candidate window, which the text service draws.
    pub(crate) panel: PanelUpdate,
    /// The 中/英 badge to show by the caret.
    pub(crate) toast: Option<String>,
    /// A page turn stopped at the list's end (1 past the last, -1 before the first): the page nudges.
    pub(crate) boundary: Option<i32>,
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
    let boundary = match object.get("boundary") {
        None => None,
        Some(value) => Some(value.as_i64().filter(|d| d.abs() == 1)? as i32),
    };
    Some(Reply {
        handled: boolean("handled")?,
        candidates,
        english: boolean("english")?,
        composing,
        commit: text("commit")?.encode_utf16().collect(),
        preedit,
        cursor,
        panel,
        toast,
        boundary,
    })
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
            animates: true,
            above: false,
        };
        let mut shown = good();
        shown["panel"] = serde_json::to_value(&panel).unwrap();
        shown["toast"] = json!("英");
        shown["boundary"] = json!(-1);
        let reply = parse_reply(&line(shown), 7, "{A}").unwrap();
        assert_eq!(reply.panel, PanelUpdate::Show(Box::new(panel)));
        assert_eq!((reply.toast.as_deref(), reply.boundary), (Some("英"), Some(-1)));
        for (key, value) in [("panel", json!(3)), ("toast", json!(1)), ("boundary", json!(2))] {
            let mut bad = good();
            bad[key] = value;
            assert!(parse_reply(&line(bad), 7, "{A}").is_none(), "{key}");
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
    fn frames() {
        assert_eq!(frame(b"{}"), Frame::Incomplete);
        assert_eq!(frame(b"{}\n"), Frame::Complete);
        assert_eq!(frame(b"{}\n{}"), Frame::Invalid);
        assert_eq!(frame(&vec![b' '; MAX_FRAME]), Frame::Invalid);
        assert!(parse_reply(b"{\"v\":1}\n{}\n", 1, "c").is_none());
        assert!(parse_reply(b"\xff\n", 1, "c").is_none());
    }
}

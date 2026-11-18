//! What the candidate window shows: the current page of the list, the reading row, a notice, the composition for an
//! application that cannot show it.  A projection of the core's `Presentation` that travels over the broker (the
//! Windows text service draws it), so it is plain data.

use serde::{Deserialize, Serialize};

/// One candidate of the current page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The selection key that picks it; none while no key picks it (a passive 聯想詞 list, before ↓ enters it).
    pub key: Option<char>,
    pub text: String,
    /// Small grey text after it (a correction's reading).
    pub annotation: Option<String>,
    /// The word actions this candidate offers (its right-click menu): bit `n` is the action whose raw value is `n`
    /// ([`WORD_ACTIONS`]).  Left out of the JSON when none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub actions: u8,
}

/// The word actions in menu order: their IPC names (`action` op) and titles; bit `n` of [`Row::actions`] is entry `n`.
pub const WORD_ACTIONS: [(&str, &str); 3] =
    [("block", "不要再推薦"), ("forget", "忘記這個選字"), ("addPhrase", "加入自訂詞")];

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's skip_serializing_if passes a reference")]
fn is_zero(value: &u8) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's skip_serializing_if passes a reference")]
fn is_false(value: &bool) -> bool {
    !*value
}

/// One state of the window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelModel {
    /// The current page's rows; empty without a list.
    pub rows: Vec<Row>,
    /// How many candidates the whole list has.
    pub count: usize,
    pub page: usize,
    pub page_count: usize,
    pub page_size: usize,
    /// The highlighted row of the page.
    pub selected: Option<usize>,
    /// The highlighted span's Zhuyin (the reading row).
    pub reading: Option<String>,
    /// A short message (「Enter 加入自訂詞：王小明」, 「已加入自訂詞：王小明」).
    pub notice: Option<String>,
    /// The list has only the target's one character (單字 on).
    pub single: bool,
    /// The list's corrections mode (修正 on: `Some(true)`); `None`: the list cannot have corrections.
    pub tolerant: Option<bool>,
    /// The clause the list changes (a different clause is a different list, not a page turn).
    pub focused_clause: Option<usize>,
    /// The composition, shown above the list when the application cannot show it.
    pub preview: Option<String>,
    /// The notice reports a word action that 「復原」 (a link at the notice's end, or Ctrl+Z) can undo.  Left out of
    /// the JSON when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub undo: bool,
    /// A line under the list that teaches the word actions (「Ctrl+Del 不要再推薦・右鍵看更多」), the first few times a
    /// list opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
    /// The right-click menu open on this row of the page, drawn inside the window (the GPUI app only; the Windows
    /// text service shows a system menu instead).  Not sent.
    #[serde(skip)]
    pub menu: Option<usize>,
    /// The list is next-word suggestions after a commit (聯想詞), not conversions: the window says so.
    #[serde(default)]
    pub associated: bool,
    /// The footer for a span shown through an automatic correction: 「你打的是 ㄊㄜˇ」 (+「 · ⌫ 還原」).
    #[serde(default)]
    pub typed: Option<String>,
    /// UTF-16 ranges of `preview` shown through an automatic correction (a dotted underline marks them).
    #[serde(default)]
    pub preview_corrected: Vec<std::ops::Range<usize>>,
}

impl PanelModel {
    /// A list is open.
    #[must_use]
    pub fn has_list(&self) -> bool {
        self.count > 0
    }
}

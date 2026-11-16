//! What the candidate window shows: the current page of the list, the reading row, a notice, the composition for an
//! application that cannot show it.  A projection of the core's `Presentation` that travels over the broker (the
//! Windows text service draws it), so it is plain data.

use serde::{Deserialize, Serialize};

/// One candidate of the current page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The selection key that picks it.
    pub key: Option<char>,
    pub text: String,
    /// Small grey text after it (a correction's reading).
    pub annotation: Option<String>,
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
}

impl PanelModel {
    /// A list is open.
    #[must_use]
    pub fn has_list(&self) -> bool {
        self.count > 0
    }
}

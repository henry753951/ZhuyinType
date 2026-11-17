//! The snapshot every view draws: the composition, its reading and clauses, and the current page of the candidate
//! list.  The candidate window, the 視窗與提示 preview, the 介紹 typing demo and the wizard all draw a
//! [`Presentation`]; observers (usage statistics, the correction log) compare the one before a key with the one after.
//!
//! Offsets (`cursor`, `clauses`, `corrected`) are UTF-16 code units, as the engine and the broker protocol report them.

use std::ops::Range;

use zhuyin_core::api::{Candidate, CandidateList};

/// A rectangle in logical pixels (screen coordinates for the caret), and where the candidate window goes.
pub use zt_panel::position::{Rect, popup_position};

/// One row of the current page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateRow {
    /// Index into the whole list (what [`crate::service::PopupAction::Pick`] does not take: that takes the row).
    pub index: usize,
    /// The text the candidate inserts.
    pub text: String,
    /// Small grey text after it (the reading of a rare character, a correction's reading), if any.
    pub annotation: Option<String>,
    /// The selection key that picks it (`1`…`9`, `a`…`l`).
    pub key: Option<char>,
}

/// What the composer shows at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presentation {
    /// The composition (marked text): converted characters with the unfinished syllable's Zhuyin at the cursor.
    pub composition: String,
    /// Standard Zhuyin of the highlighted span (the window footer's ` action), when there is one.
    pub reading: Option<String>,
    /// Caret position in `composition`, in UTF-16 units.
    pub cursor: usize,
    /// UTF-16 ranges of the words in `composition`, for clause underlines.
    pub clauses: Vec<Range<usize>>,
    /// The clause the highlighted candidate replaces (or selected for adding a user phrase).
    pub focused_clause: Option<usize>,
    /// The clause ↓ would change, with the cursor inside the composition and no list open.
    pub target_clause: Option<usize>,
    /// The current page's rows; empty when no list is open.
    pub rows: Vec<CandidateRow>,
    /// How many candidates the whole list has (0: no list).
    pub count: usize,
    /// The highlighted candidate's index in the whole list.
    pub selection: Option<usize>,
    /// The highlighted row on the current page.
    pub selected: Option<usize>,
    /// The current page (0-based), the number of pages (0 without a list) and candidates per page.
    pub page: usize,
    pub page_count: usize,
    pub page_size: usize,
    /// The list has only the one character at the target (⇧↓).
    pub single: bool,
    /// The list shows typing corrections (⇧↑).
    pub corrections: bool,
    /// The list's corrections mode: `Some(true)` corrections, `Some(false)` as typed, `None` the list cannot have
    /// corrections (punctuation, corrections off) or no list.
    pub tolerant: Option<bool>,
    /// A short message in the candidate window (for example 「↩ 加入自訂詞」).
    pub notice: Option<String>,
    /// The engine's reading channel (「接受常見的其他讀法」) is loaded and on.
    pub reading_channel_active: bool,
    /// The word actions each row of the page offers (`zt_panel::model::Row::actions`), in row order.
    pub row_actions: Vec<u8>,
    /// The notice reports a word action that 「復原」 or Ctrl+Z can still undo.
    pub undo: bool,
    /// The word actions' tip under the list, the first few times a list opens.
    pub tip: Option<String>,
    /// The list is next-word suggestions after a commit (聯想詞): no composition; ↓ enters it.  While it is not
    /// entered (passive) nothing is highlighted: `selection` and `selected` are `None`.
    pub associated: bool,
    /// UTF-16 ranges of `composition` shown through an automatic typing correction or fuzzy sound (「標示自動修正」
    /// marks them); empty when that setting is off.
    pub corrected: Vec<Range<usize>>,
    /// The Zhuyin as typed for the corrections in the window's span, while the window is open on one (the footer
    /// 「你打的是 ㄊㄜˇ」); `None` when 「標示自動修正」 is off.
    pub typed_reading: Option<String>,
    /// ⌫ in the window puts the correction back (「⌫ 還原自動修正」 on): the footer says so.
    pub undo_hint: bool,
    /// The next ⌫ puts back the keys as typed instead of deleting.
    pub backspace_reverts: bool,
    /// The last key put a correction back to the keys as typed (counted in the usage statistics).
    pub reverted: bool,
}

impl Presentation {
    /// The engine's presentation as the views draw it.  `selection_keys` label the rows; `page_size` is used when no
    /// list is open.
    #[must_use]
    pub fn from_engine(
        p: &zhuyin_core::api::Presentation,
        selection_keys: &[char],
        page_size: usize,
        reading_channel_active: bool,
    ) -> Self {
        let mut result = Self {
            composition: p.text.clone(),
            reading: p.zhuyin_hint.clone(),
            cursor: p.cursor,
            clauses: p.clauses.clone(),
            focused_clause: p.focused_clause,
            target_clause: p.target_clause,
            page_size: page_size.max(1),
            notice: p.notice.clone(),
            reading_channel_active,
            corrected: p.corrections.clone(),
            typed_reading: p.typed_reading.clone(),
            backspace_reverts: p.backspace_reverts,
            reverted: p.reverted,
            ..Self::default()
        };
        if let Some(list) = &p.candidates {
            result.fill_list(list, selection_keys);
        }
        result
    }

    /// As the settings show corrections: `show` 「標示自動修正」 (off: no marks, no typed-reading footer), `undo`
    /// 「⌫ 還原自動修正」 (the footer names ⌫).
    #[must_use]
    pub fn with_correction_marks(mut self, show: bool, undo: bool) -> Self {
        if !show {
            self.corrected.clear();
            self.typed_reading = None;
        }
        self.undo_hint = undo && self.typed_reading.is_some();
        self
    }

    /// The candidate window's footer for a corrected span: 「你打的是 ㄊㄜˇ」, with 「 · ⌫ 還原」 when ⌫ puts it back.
    #[must_use]
    pub fn typed_footer(&self) -> Option<String> {
        let reading = self.typed_reading.as_deref().filter(|reading| !reading.is_empty())?;
        Some(if self.undo_hint {
            format!("你打的是 {reading} · ⌫ 還原")
        } else {
            format!("你打的是 {reading}")
        })
    }

    fn fill_list(&mut self, list: &CandidateList, selection_keys: &[char]) {
        self.page_size = list.page_size();
        self.count = list.items().len();
        self.single = list.single_character();
        self.tolerant = list.tolerant();
        self.corrections = list.tolerant() == Some(true);
        self.associated = list.associated();
        if self.count == 0 {
            return;
        }
        self.selection = Some(list.selection());
        self.page = list.page();
        self.page_count = list.page_count();
        let range = list.page_range();
        self.selected = Some(list.selection() - range.start);
        if list.passive() {
            self.selection = None;
            self.selected = None;
        }
        self.rows = list.items()[range.clone()]
            .iter()
            .zip(range)
            .enumerate()
            .map(|(row, (Candidate { text, annotation }, index))| CandidateRow {
                index,
                text: text.clone(),
                annotation: annotation.clone(),
                key: selection_keys.get(row).copied(),
            })
            .collect();
    }

    /// The presentation with its texts in 簡體 (繁／簡): the composition and every candidate.  The conversion keeps
    /// the length in UTF-16 units, so the cursor and clauses stay valid; readings, annotations and notices are not
    /// characters to send and stay as they are.
    #[must_use]
    pub fn simplified(&self) -> Self {
        use zhuyin_core::simplified::to_simplified;
        let mut p = self.clone();
        p.composition = to_simplified(&p.composition);
        for row in &mut p.rows {
            row.text = to_simplified(&row.text);
        }
        p
    }

    /// Nothing composed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.composition.is_empty()
    }

    /// A candidate list is open.
    #[must_use]
    pub fn has_list(&self) -> bool {
        self.count > 0
    }

    /// The text of clause `index`, if it is a valid range.
    #[must_use]
    pub fn clause_text(&self, index: usize) -> Option<String> {
        let range = self.clauses.get(index)?;
        let units: Vec<u16> = self.composition.encode_utf16().collect();
        (range.start <= range.end && range.end <= units.len()).then(|| String::from_utf16_lossy(&units[range.clone()]))
    }

    /// What the candidate window shows of this (`zt_panel`), with the composition above the list when the
    /// application cannot show it (`preview`).
    #[must_use]
    pub fn panel_model(&self, preview: Option<&str>) -> zt_panel::model::PanelModel {
        zt_panel::model::PanelModel {
            rows: self
                .rows
                .iter()
                .enumerate()
                .map(|(n, row)| zt_panel::model::Row {
                    key: row.key,
                    text: row.text.clone(),
                    annotation: row.annotation.clone(),
                    actions: self.row_actions.get(n).copied().unwrap_or(0),
                })
                .collect(),
            count: self.count,
            page: self.page,
            page_count: self.page_count,
            page_size: self.page_size,
            selected: self.selected,
            reading: self.reading.clone(),
            notice: self.notice.clone(),
            single: self.single,
            tolerant: self.tolerant,
            focused_clause: self.focused_clause,
            preview: preview.filter(|text| !text.is_empty()).map(str::to_owned),
            undo: self.undo,
            tip: self.tip.clone(),
            menu: None,
            associated: self.associated,
            typed: self.typed_footer(),
            preview_corrected: if preview.is_some_and(|text| text == self.composition) {
                self.corrected.clone()
            } else {
                Vec::new()
            },
        }
    }

    /// The focused clause's text.
    #[must_use]
    pub fn focused_text(&self) -> Option<String> {
        self.focused_clause.and_then(|index| self.clause_text(index))
    }
}

/// A list of fixed candidates that pages like the real one, for previews (port of the Qt `Candidates` model): the
/// 視窗與提示 page shows it with example words.  Picks and filter switches are reported, not carried out.
#[derive(Clone, Debug)]
pub struct PreviewList {
    entries: Vec<Candidate>,
    reading: Option<String>,
    page: usize,
    selected: Option<usize>,
    page_size: usize,
    single: bool,
    corrections: bool,
    revision: u64,
}

/// What a preview list asks its owner to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewIntent {
    /// The candidate at this index in the whole list was picked.
    Picked(usize),
    /// Show the list for these filters (single character, corrections).
    Filter { single: bool, corrections: bool },
    /// A page turn or highlight move stopped at the list's end (1 past the last, -1 before the first).
    Boundary(i32),
}

impl Default for PreviewList {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            reading: None,
            page: 0,
            selected: None,
            page_size: 9,
            single: false,
            corrections: false,
            revision: 0,
        }
    }
}

impl PreviewList {
    /// Shows `entries` with `selected` highlighted; a different list starts a new revision (sizing and animation).
    pub fn present(&mut self, entries: Vec<Candidate>, reading: Option<String>, selected: usize) {
        if entries != self.entries || reading != self.reading {
            self.revision += 1;
        }
        self.entries = entries;
        self.reading = reading;
        if self.entries.is_empty() {
            self.page = 0;
            self.selected = None;
        } else {
            let selected = selected.min(self.entries.len() - 1);
            self.page = selected / self.page_size;
            self.selected = Some(selected % self.page_size);
        }
    }

    /// Candidates per page (3…9; other values are ignored), keeping the highlighted candidate.
    pub fn set_page_size(&mut self, size: usize) {
        if size == 0 || size == self.page_size {
            return;
        }
        let absolute = self.absolute();
        self.page_size = size;
        self.revision += 1;
        if let Some(absolute) = absolute {
            self.page = absolute / size;
            self.selected = Some(absolute % size);
        }
    }

    fn absolute(&self) -> Option<usize> {
        self.selected.map(|row| self.page * self.page_size + row)
    }

    fn rows_on(&self, page: usize) -> usize {
        self.entries.len().saturating_sub(page * self.page_size).min(self.page_size)
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.entries.len().div_ceil(self.page_size)
    }

    #[must_use]
    pub fn page(&self) -> usize {
        self.page
    }

    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    #[must_use]
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Turns `delta` pages (clamped to the list); at an end, reports the boundary.
    pub fn turn_page(&mut self, delta: i64) -> Option<PreviewIntent> {
        if delta == 0 || self.entries.is_empty() {
            return None;
        }
        let last = self.page_count() - 1;
        let target = (self.page as i64).saturating_add(delta).clamp(0, last as i64) as usize;
        if target == self.page {
            return Some(PreviewIntent::Boundary(if delta > 0 { 1 } else { -1 }));
        }
        self.page = target;
        self.selected = Some(0);
        None
    }

    /// Moves the highlight one step (`delta` > 0 forward), across pages; at an end, reports the boundary.
    pub fn move_selection(&mut self, delta: i64) -> Option<PreviewIntent> {
        let absolute = self.absolute()?;
        if delta == 0 {
            return None;
        }
        let forward = delta > 0;
        if (forward && absolute + 1 >= self.entries.len()) || (!forward && absolute == 0) {
            return Some(PreviewIntent::Boundary(if forward { 1 } else { -1 }));
        }
        let next = if forward { absolute + 1 } else { absolute - 1 };
        self.page = next / self.page_size;
        self.selected = Some(next % self.page_size);
        None
    }

    /// Picks row `row` of the current page.
    #[must_use]
    pub fn pick(&self, row: usize) -> Option<PreviewIntent> {
        (row < self.rows_on(self.page)).then(|| PreviewIntent::Picked(self.page * self.page_size + row))
    }

    /// Asks for the single-character list (or back).
    pub fn toggle_single(&mut self) -> PreviewIntent {
        self.single = !self.single;
        PreviewIntent::Filter { single: self.single, corrections: self.corrections }
    }

    /// Asks for the corrections list (or back).
    pub fn toggle_corrections(&mut self) -> PreviewIntent {
        self.corrections = !self.corrections;
        PreviewIntent::Filter { single: self.single, corrections: self.corrections }
    }

    /// The list as the views draw it, with `composition` as the text being composed.
    #[must_use]
    pub fn presentation(&self, composition: &str, selection_keys: &[char]) -> Presentation {
        let start = self.page * self.page_size;
        let rows = self.entries[start.min(self.entries.len())..start + self.rows_on(self.page)]
            .iter()
            .enumerate()
            .map(|(row, candidate)| CandidateRow {
                index: start + row,
                text: candidate.text.clone(),
                annotation: candidate.annotation.clone(),
                key: selection_keys.get(row).copied(),
            })
            .collect();
        Presentation {
            composition: composition.to_owned(),
            reading: self.reading.clone(),
            cursor: composition.encode_utf16().count(),
            rows,
            count: self.entries.len(),
            selection: self.absolute(),
            selected: self.selected,
            page: self.page,
            page_count: self.page_count(),
            page_size: self.page_size,
            single: self.single,
            corrections: self.corrections,
            tolerant: Some(self.corrections),
            ..Presentation::default()
        }
    }
}

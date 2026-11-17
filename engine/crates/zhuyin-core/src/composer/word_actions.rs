//! Word actions on a candidate (or, for adding a custom word, the whole composition), with one level of undo.
//! The engine changes the personal data ([`crate::engine::WordUndo`]); this part picks the word, refreshes the
//! conversion and the open list, and sets the notice the candidate window shows.

use super::Composer;
use crate::conversion::FixedWord;
use crate::engine::valid_phrase;

/// What the user asked to do with a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CandidateAction {
    /// 「不要再推薦」: never suggest the word for its reading.
    Block,
    /// 「忘記這個選字」: forget that the user chose it.
    Forget,
    /// 「加入自訂詞」: add it with its reading as a custom word.
    AddPhrase,
}

impl CandidateAction {
    /// Every action, in menu order.
    pub const ALL: [CandidateAction; 3] = [CandidateAction::Block, CandidateAction::Forget, CandidateAction::AddPhrase];

    /// The menu item's title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            CandidateAction::Block => "不要再推薦",
            CandidateAction::Forget => "忘記這個選字",
            CandidateAction::AddPhrase => "加入自訂詞",
        }
    }

    /// The value clients pass across the C interface and IPC.
    #[must_use]
    pub fn raw_value(self) -> u32 {
        match self {
            CandidateAction::Block => 0,
            CandidateAction::Forget => 1,
            CandidateAction::AddPhrase => 2,
        }
    }

    /// The action for [`raw_value`](Self::raw_value).
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.raw_value() == raw)
    }
}

/// The word an action applies to.
struct Target {
    text: String,
    reading: String,
    /// From the candidate list (true) or the whole composition (false).
    listed: bool,
}

impl Composer {
    /// Candidate `index` of the open list, else the highlighted one; with no list open, the whole composition (for
    /// [`CandidateAction::AddPhrase`] only).
    fn action_target(&self, action: CandidateAction, index: Option<usize>) -> Option<Target> {
        if self.candidate_window.is_open() {
            let choice: &FixedWord = match index {
                Some(index) => self.candidate_window.get(index)?,
                None => self.candidate_window.selected()?,
            };
            if choice.reading.is_empty() || choice.annotation.is_some() {
                return None;
            }
            return Some(Target { text: choice.text.to_string(), reading: choice.reading.to_string(), listed: true });
        }
        if action != CandidateAction::AddPhrase || index.is_some() || self.has_buffer() || self.tokens.is_empty() {
            return None;
        }
        let range = 0..self.tokens.len();
        let readings: Vec<String> = self.tokens.iter().filter_map(|t| t.reading().map(str::to_owned)).collect();
        if readings.len() != range.len() {
            return None;
        }
        Some(Target { text: self.shown(&range), reading: readings.join(" "), listed: false })
    }

    /// Whether `action` can be done on candidate `index` (or the highlighted one; see
    /// [`perform_candidate_action`](Self::perform_candidate_action)).
    #[must_use]
    pub fn candidate_action_available(&self, action: CandidateAction, index: Option<usize>) -> bool {
        let Some(target) = self.action_target(action, index) else { return false };
        let engine = &self.engine;
        match action {
            CandidateAction::Block => !engine.is_blocked(&target.text, &target.reading),
            CandidateAction::Forget => engine.has_learned_choice(&target.text, &target.reading),
            CandidateAction::AddPhrase => {
                valid_phrase(&target.text, &target.reading) && !engine.is_user_phrase(&target.text, &target.reading)
            }
        }
    }

    /// Does `action` on candidate `index` of the open list (`None`: the highlighted one; with no list open, adding
    /// takes the whole composition).  Returns the notice shown, or `None` when the action does not apply.  The open
    /// list is rebuilt at the same place; [`undo_candidate_action`](Self::undo_candidate_action) reverses it.
    pub fn perform_candidate_action(&mut self, action: CandidateAction, index: Option<usize>) -> Option<String> {
        if !self.candidate_action_available(action, index) {
            return None;
        }
        let target = self.action_target(action, index)?;
        let (text, reading) = (target.text.as_str(), target.reading.as_str());
        let undo = match action {
            CandidateAction::Block => {
                // A pick of the word in this composition gives way too.
                self.fixed.retain(|f| !(*f.text == *text && *f.reading == *reading));
                self.engine.block_suggestion(text, reading)
            }
            CandidateAction::Forget => self.engine.forget_choice(text, reading)?,
            CandidateAction::AddPhrase => self.engine.add_phrase_undoable(text, reading).ok()?,
        };
        let notice = match action {
            CandidateAction::Block => format!("不再推薦「{text}」"),
            CandidateAction::Forget => format!("已忘記「{text}」"),
            CandidateAction::AddPhrase => format!("已加入自訂詞：{text}"),
        };
        self.word_undo = Some(undo);
        self.after_word_action(target.listed, notice.clone());
        Some(notice)
    }

    /// Whether the last word action can be undone.
    #[must_use]
    pub fn can_undo_candidate_action(&self) -> bool {
        self.word_undo.is_some()
    }

    /// Undoes the last word action; false when there is none.
    pub fn undo_candidate_action(&mut self) -> bool {
        let Some(undo) = self.word_undo.take() else { return false };
        self.engine.undo_word_action(undo);
        let listed = self.candidate_window.is_open();
        self.after_word_action(listed, "已復原".to_owned());
        true
    }

    /// The personal data changed: convert again and rebuild the open list where it was.
    fn after_word_action(&mut self, listed: bool, notice: String) {
        self.conversion_dirty = true;
        self.refresh();
        if listed && self.candidate_window.is_open() {
            self.open_candidates(self.candidate_window.single_character(), self.candidate_window.tolerant());
        }
        self.notice = Some(notice);
        self.refresh();
    }
}

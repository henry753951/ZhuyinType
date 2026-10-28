//! The contract between the engine and an input method client (port of `API.swift`).
//!
//! The client turns hardware keys into [`KeyInput`], hands them to a composer, inserts [`KeyResult::commit`] into the
//! application, then draws the composer's [`Presentation`] (marked text and the candidate window).  Nothing here
//! touches a platform API, so every behaviour can be checked from the command line.

use std::ops::Range;

/// A key as the composer sees it.  Clients map hardware key codes on the US layout to these, so a character is the
/// key's unshifted ASCII character whatever keyboard layout the user has selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyInput {
    /// A printable ASCII key with no modifier: `a`, `1`, `,`, `-`, `/`, `` ` ``, `[`, ...
    Char(char),
    /// A printable ASCII key with Shift held, named by its unshifted character: Shift+1 is `ShiftedChar('1')`.
    ShiftedChar(char),
    /// A digit or symbol key with Option held: the ASCII character printed on it, typed as is (Option+. is `.`,
    /// Option+Shift+. is `>`).
    Printed(char),
    /// A symbol key with Control held, named by the character printed on it (Control+Shift+/ is `?`): full-width
    /// Chinese punctuation, as Windows' Microsoft Bopomofo types it.
    ControlChar(char),
    /// Space.
    Space,
    /// Shift+Space.
    ShiftSpace,
    /// Return.
    Enter,
    /// Shift+Return.
    ShiftEnter,
    /// Backspace.
    Backspace,
    /// Option+Backspace.
    OptionBackspace,
    /// Forward delete.
    ForwardDelete,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Shift+Tab.
    ShiftTab,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Shift+Left.
    ShiftLeft,
    /// Shift+Right.
    ShiftRight,
    /// Candidates for the one character at the target; in the window, switches the single-character list.
    ShiftDown,
    /// Typing corrections and fuzzy matches first; in the window, switches the corrections list.
    ShiftUp,
}

/// What the composer did with a key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyResult {
    /// False: the key was not for the input method and goes on to the application (after `commit`, if any).
    pub handled: bool,
    /// Text to insert into the client, before the new presentation is shown.
    pub commit: Option<String>,
}

impl KeyResult {
    /// The key goes on to the application.
    pub const PASS_THROUGH: KeyResult = KeyResult { handled: false, commit: None };
    /// The key was used and nothing is committed.
    pub const CONSUMED: KeyResult = KeyResult { handled: true, commit: None };

    /// A result with the given fields.
    #[must_use]
    pub fn new(handled: bool, commit: Option<String>) -> Self {
        Self { handled, commit }
    }
}

/// One entry of the candidate window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The text the candidate inserts.
    pub text: String,
    /// Small grey text after the candidate, e.g. the reading of a rare character.  `None` for most candidates.
    pub annotation: Option<String>,
}

/// The open candidate window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateList {
    /// Every candidate, over all pages.
    pub items: Vec<Candidate>,
    /// Highlighted index into `items`.
    pub selection: usize,
    /// Candidates per page; at least 1.
    pub page_size: usize,
    /// Whether the list shows typing corrections (true) or only words for the keys as typed (false); `None` when
    /// the list cannot have corrections (punctuation, correction off).
    pub tolerant: Option<bool>,
    /// Only the one character at the target (Shift-↓), not every span from it.
    pub single_character: bool,
}

impl CandidateList {
    /// A list; `page_size` is raised to 1 when smaller.
    #[must_use]
    pub fn new(
        items: Vec<Candidate>,
        selection: usize,
        page_size: usize,
        tolerant: Option<bool>,
        single_character: bool,
    ) -> Self {
        Self { items, selection, page_size: page_size.max(1), tolerant, single_character }
    }

    /// The page of the highlighted candidate.
    #[must_use]
    pub fn page(&self) -> usize {
        self.selection / self.page_size
    }

    /// How many pages the list has; at least 1.
    #[must_use]
    pub fn page_count(&self) -> usize {
        1.max(self.items.len().div_ceil(self.page_size))
    }

    /// Range of `items` on the current page.
    #[must_use]
    pub fn page_range(&self) -> Range<usize> {
        let start = self.page() * self.page_size;
        start..self.items.len().min(start + self.page_size)
    }
}

/// What the client draws: the marked text and the candidate window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presentation {
    /// The marked text: converted characters, with the unfinished syllable's Zhuyin symbols at the cursor.
    pub text: String,
    /// Caret position in `text`, in UTF-16 units.
    pub cursor: usize,
    /// UTF-16 ranges of the words in `text`, for clause underlines.  Covers `text` without gaps.
    pub clauses: Vec<Range<usize>>,
    /// Index into `clauses` replaced by the highlighted candidate, or selected for adding a user phrase.
    pub focused_clause: Option<usize>,
    /// Index into `clauses` of the character ↓ would change, while the cursor is inside the composition and no list
    /// is open: shows which side of the cursor the list is for.
    pub target_clause: Option<usize>,
    /// The open candidate window, or `None`.
    pub candidates: Option<CandidateList>,
    /// A short message shown in the candidate window.
    pub notice: Option<String>,
    /// Standard plain Zhuyin for the highlighted span; the backtick action in the window footer.
    pub zhuyin_hint: Option<String>,
}

impl Presentation {
    /// Nothing composed and no window.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.candidates.is_none()
    }
}

/// What Shift with a symbol key types in Chinese mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PunctuationKeys {
    /// The symbol printed on the key, half-width (Shift+, is `<`, Shift+- is `_`); Control with the key types Chinese
    /// punctuation (Control+, is `，`).  As Windows' Microsoft Bopomofo.
    #[default]
    Windows,
    /// Chinese punctuation (Shift+, is `，`, Shift+- is `——`), as the Zhuyin input of macOS.
    Mac,
}

impl PunctuationKeys {
    /// Every option, in declaration order.
    pub const ALL: [PunctuationKeys; 2] = [PunctuationKeys::Windows, PunctuationKeys::Mac];

    /// The name used by settings and the command line (`windows`, `mac`).
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            PunctuationKeys::Windows => "windows",
            PunctuationKeys::Mac => "mac",
        }
    }

    /// The option named `raw` (see [`raw_value`](Self::raw_value)).
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.raw_value() == raw)
    }
}

/// Space in a composition with no unfinished syllable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpaceAction {
    /// Opens the candidate window; in the window, Space turns the page.
    #[default]
    OpenCandidates,
    /// Commits the composition; the app does not receive the Space.
    Commit,
    /// Commits the composition, then the app receives the Space, so a space follows the text.
    CommitAndSpace,
    /// Puts a space into the composition and keeps composing (what Shift+Space always does); nothing is sent yet.
    InsertSpace,
}

impl SpaceAction {
    /// Every action, in declaration order.
    pub const ALL: [SpaceAction; 4] =
        [SpaceAction::OpenCandidates, SpaceAction::Commit, SpaceAction::CommitAndSpace, SpaceAction::InsertSpace];

    /// The Swift case name (`openCandidates`, `commit`, `commitAndSpace`, `insertSpace`).
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            SpaceAction::OpenCandidates => "openCandidates",
            SpaceAction::Commit => "commit",
            SpaceAction::CommitAndSpace => "commitAndSpace",
            SpaceAction::InsertSpace => "insertSpace",
        }
    }

    /// The action named `raw` (see [`raw_value`](Self::raw_value)).
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.raw_value() == raw)
    }
}

/// Strength of the physical-keyboard error channel.  Neutral-tone matching is independent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TypoTolerance {
    /// No keyboard correction.
    Off,
    /// The calibrated channel costs.
    #[default]
    Standard,
    /// 70% of the standard channel costs.
    Aggressive,
}

impl TypoTolerance {
    /// Every option, in declaration order.
    pub const ALL: [TypoTolerance; 3] = [TypoTolerance::Off, TypoTolerance::Standard, TypoTolerance::Aggressive];

    /// The factor applied to keyboard-channel costs.
    #[must_use]
    pub fn cost_multiplier(self) -> f64 {
        if self == TypoTolerance::Aggressive { 0.7 } else { 1.0 }
    }

    /// The name used by settings and the command line (`off`, `standard`, `aggressive`).
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            TypoTolerance::Off => "off",
            TypoTolerance::Standard => "standard",
            TypoTolerance::Aggressive => "aggressive",
        }
    }

    /// The option named `raw` (see [`raw_value`](Self::raw_value)).
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.raw_value() == raw)
    }
}

/// Sound pairs many speakers merge.  With a pair on, each sound also matches the other one at a small cost, so an
/// exact match ranks first unless the other word is clearly likelier; a syllable can use one initial and one final
/// pair.  A bit set with the same raw values as the Swift `OptionSet`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FuzzySounds(pub i64);

/// One fuzzy pair: its flag, the two sounds, and whether they are initials (true) or finals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuzzyPair {
    /// The flag that turns the pair on.
    pub set: FuzzySounds,
    /// One sound.
    pub a: char,
    /// The other sound.
    pub b: char,
    /// The sounds are initials (true) or finals (false).
    pub initial: bool,
}

impl FuzzySounds {
    /// No pairs.
    pub const NONE: FuzzySounds = FuzzySounds(0);
    /// ㄗ and ㄓ.
    pub const Z_ZH: FuzzySounds = FuzzySounds(1 << 0);
    /// ㄘ and ㄔ.
    pub const C_CH: FuzzySounds = FuzzySounds(1 << 1);
    /// ㄙ and ㄕ.
    pub const S_SH: FuzzySounds = FuzzySounds(1 << 2);
    /// ㄣ and ㄥ.
    pub const EN_ENG: FuzzySounds = FuzzySounds(1 << 3);
    /// ㄢ and ㄤ.
    pub const AN_ANG: FuzzySounds = FuzzySounds(1 << 4);

    /// Each pair, in the order fuzzy matching applies them.
    pub const PAIRS: [FuzzyPair; 5] = [
        FuzzyPair { set: Self::Z_ZH, a: 'ㄗ', b: 'ㄓ', initial: true },
        FuzzyPair { set: Self::C_CH, a: 'ㄘ', b: 'ㄔ', initial: true },
        FuzzyPair { set: Self::S_SH, a: 'ㄙ', b: 'ㄕ', initial: true },
        FuzzyPair { set: Self::EN_ENG, a: 'ㄣ', b: 'ㄥ', initial: false },
        FuzzyPair { set: Self::AN_ANG, a: 'ㄢ', b: 'ㄤ', initial: false },
    ];

    /// The raw bits.
    #[must_use]
    pub fn raw_value(self) -> i64 {
        self.0
    }

    /// No pair is on.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Every pair of `other` is on in `self`.
    #[must_use]
    pub fn contains(self, other: FuzzySounds) -> bool {
        self.0 & other.0 == other.0
    }

    /// Turns on the pairs of `other`.
    pub fn insert(&mut self, other: FuzzySounds) {
        self.0 |= other.0;
    }
}

impl std::ops::BitOr for FuzzySounds {
    type Output = FuzzySounds;
    fn bitor(self, rhs: FuzzySounds) -> FuzzySounds {
        FuzzySounds(self.0 | rhs.0)
    }
}

/// Which side of the cursor the candidate window lists spans for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CandidateAnchor {
    /// Spans start at the cursor, except at the composition end where they end at the cursor.
    #[default]
    AfterCursor,
    /// Spans end at the cursor, except at the composition start where they start at zero.
    BeforeCursor,
}

/// How a run of typed keys is split into syllables and whether key order mistakes are repaired.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyDecoderMethod {
    /// Every key goes into the current syllable's slots; only a tone key or Space ends it (the original behaviour).
    Slots,
    /// Also ends a first-tone syllable when the next key starts another one (Space left out).
    Boundaries,
    /// Boundaries, plus two neighbouring keys taken in the other order, and an extra key skipped, at fixed costs.
    Reorder,
    /// Like `Reorder`, but keys pressed almost together cost little to reorder or skip: fast typing presses keys out
    /// of order or two at once, while keys typed apart are meant as typed.
    #[default]
    Timed,
}

impl KeyDecoderMethod {
    /// Every method, in declaration order (Swift's `allCases`).
    pub const ALL: [KeyDecoderMethod; 4] =
        [KeyDecoderMethod::Slots, KeyDecoderMethod::Boundaries, KeyDecoderMethod::Reorder, KeyDecoderMethod::Timed];

    /// The name used by the command line (`slots`, `boundaries`, `reorder`, `timed`).
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            KeyDecoderMethod::Slots => "slots",
            KeyDecoderMethod::Boundaries => "boundaries",
            KeyDecoderMethod::Reorder => "reorder",
            KeyDecoderMethod::Timed => "timed",
        }
    }

    /// The method named `raw` (see [`raw_value`](Self::raw_value)).
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.raw_value() == raw)
    }
}

/// One syllable as a character model reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterModelSyllable {
    /// Toneless reading; `None` for a literal.
    pub base: Option<String>,
    /// 0 none, 1 first, 2 ˊ, 3 ˇ, 4 ˋ, 5 ˙.
    pub tone: i32,
    /// How far the reading is from the keys typed (0 exact … 3 large correction).
    pub cost_bucket: i32,
    /// The syllable ended without a tone key or Space (a first tone found by the key decoder).
    pub implicit: bool,
}

/// One word of a scored sentence.  Ranges are composition token offsets, not UTF-16 offsets.  Literals have an
/// empty reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentenceWord {
    /// Composition tokens the word covers.
    pub range: Range<usize>,
    /// The word.
    pub text: String,
    /// Its reading, syllables separated by spaces; empty for a literal.
    pub reading: String,
    /// Matched through a typing correction or a fuzzy sound, not the keys as typed.
    pub corrected: bool,
}

/// One of the engine's N best sentences.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoredSentence {
    /// The words, in order.
    pub words: Vec<SentenceWord>,
    /// Natural-log decoder score, including sentence boundaries.
    pub score: f64,
}

impl ScoredSentence {
    /// The sentence's text.
    #[must_use]
    pub fn text(&self) -> String {
        self.words.iter().map(|word| word.text.as_str()).collect()
    }

    /// Some word differs from the keys as typed.
    #[must_use]
    pub fn corrected(&self) -> bool {
        self.words.iter().any(|word| word.corrected)
    }
}

/// Which of two interchangeable character forms to write (台 or 臺).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CharacterForms {
    /// The everyday forms: 台.
    #[default]
    Common,
    /// The forms of the Ministry of Education's standard: 臺.
    Standard,
}

impl CharacterForms {
    /// Every option, in declaration order.
    pub const ALL: [CharacterForms; 2] = [CharacterForms::Common, CharacterForms::Standard];

    /// The name used by settings and the command line (`common`, `standard`).
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            CharacterForms::Common => "common",
            CharacterForms::Standard => "standard",
        }
    }

    /// The option named `raw` (see [`raw_value`](Self::raw_value)).
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.raw_value() == raw)
    }

    /// The common form of a standard character, if it has one.
    fn common_form(character: char) -> Option<char> {
        (character == '臺').then_some('台')
    }

    /// `text` written in these forms.  Conversion and learning use the lexicon's own forms; this only changes the
    /// text shown and committed.
    ///
    /// Swift compares extended grapheme clusters, so 臺 followed by a variation selector or a combining mark is a
    /// different `Character` and stays as it is; this checks the scalar after it for the same reason.
    #[must_use]
    pub fn apply(self, text: &str) -> String {
        if self != CharacterForms::Common || !text.chars().any(|c| Self::common_form(c).is_some()) {
            return text.to_owned();
        }
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(character) = chars.next() {
            match Self::common_form(character) {
                Some(common) if !chars.peek().is_some_and(|&next| extends_grapheme(next)) => result.push(common),
                _ => result.push(character),
            }
        }
        result
    }
}

/// Scalars that join the previous one into the same grapheme cluster, for the characters this crate replaces:
/// variation selectors, combining marks, ZWJ and emoji modifiers.
pub(crate) fn extends_grapheme(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
        | 0x200C | 0x200D | 0xFE00..=0xFE0F | 0xE0100..=0xE01EF | 0x1F3FB..=0x1F3FF | 0x3099 | 0x309A)
}

/// Options of a composer.
// Each flag is an independent user setting, as in Swift's `ComposerOptions`; no two form a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq)]
pub struct ComposerOptions {
    /// Keys that pick candidates 1..n on the current page.  Also sets the page size.
    pub selection_keys: Vec<char>,
    /// What Space does when no syllable is being typed (with one, Space is the first tone).  The engine default
    /// opens the candidate window; the app's own default is set in its preferences.
    pub space_action: SpaceAction,
    /// Escape with no candidate window open clears the whole composition (true) or only the unfinished syllable.
    pub escape_clears_composition: bool,
    /// Learn from commits and candidate picks.
    pub learning_enabled: bool,
    /// Longest composition, in syllables.  A key that would make it longer first commits the head of the composition
    /// as shown (`KeyResult::commit`) and composes the rest: up to the last clause or sentence punctuation when at most
    /// two thirds of the limit follow it, otherwise about the first third, ending between two words.
    pub max_syllables: usize,
    /// 0: off.  Otherwise a typing key that makes the composition longer than this many syllables first commits
    /// everything up to the last sentence-ending punctuation (。！？…), if there is one before the cursor.
    pub early_commit_syllables: usize,
    /// Keyboard typo correction; aggressive uses 70% of standard channel costs.
    pub typo_tolerance: TypoTolerance,
    /// Which side of the cursor the candidate window is for.
    pub candidate_anchor: CandidateAnchor,
    /// How keys typed at the end of the composition become syllables.
    pub key_decoder: KeyDecoderMethod,
    /// The candidate window lists top to bottom: ↑↓ move the highlight and ←→ move to the neighbouring character.
    /// Left to right, ←→ move the highlight and ↑↓ turn pages.
    pub vertical_list: bool,
    /// Moving the highlight past a page's last candidate goes to the next page's first (and past the first, back to
    /// the previous page's last), stopping at the ends of the list without wrapping.  Off, the highlight stays at the
    /// edge of its page; the page keys still turn pages.
    pub page_at_list_edge: bool,
    /// Which of two interchangeable character forms to write (台 or 臺).
    pub character_forms: CharacterForms,
    /// What Shift and Control with a symbol key type.
    pub punctuation_keys: PunctuationKeys,
}

impl ComposerOptions {
    /// Space opens the candidate window (true) or commits the composition (false).
    #[must_use]
    pub fn space_opens_candidates(&self) -> bool {
        self.space_action == SpaceAction::OpenCandidates
    }

    /// Sets Space to open the candidate window (true) or to commit (false).
    pub fn set_space_opens_candidates(&mut self, value: bool) {
        self.space_action = if value { SpaceAction::OpenCandidates } else { SpaceAction::Commit };
    }
}

impl Default for ComposerOptions {
    fn default() -> Self {
        Self {
            selection_keys: "123456789".chars().collect(),
            space_action: SpaceAction::OpenCandidates,
            escape_clears_composition: true,
            learning_enabled: true,
            max_syllables: 60,
            early_commit_syllables: 0,
            typo_tolerance: TypoTolerance::Standard,
            candidate_anchor: CandidateAnchor::AfterCursor,
            key_decoder: KeyDecoderMethod::Timed,
            vertical_list: true,
            page_at_list_edge: true,
            character_forms: CharacterForms::Common,
            punctuation_keys: PunctuationKeys::Windows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_swift() {
        let options = ComposerOptions::default();
        assert_eq!(options.punctuation_keys, PunctuationKeys::Windows, "Windows punctuation keys by default");
        assert_eq!(options.typo_tolerance, TypoTolerance::Standard, "standard typo tolerance is the public default");
        assert_eq!(options.key_decoder, KeyDecoderMethod::Timed);
        assert_eq!(options.selection_keys.len(), 9);
        assert_eq!(options.max_syllables, 60);
        assert_eq!(options.early_commit_syllables, 0, "no early commit by default");
    }

    #[test]
    fn candidate_list_pages() {
        let items = (0..20).map(|i| Candidate { text: i.to_string(), annotation: None }).collect();
        let list = CandidateList::new(items, 10, 9, None, false);
        assert_eq!(list.page(), 1);
        assert_eq!(list.page_count(), 3);
        assert_eq!(list.page_range(), 9..18);
        assert_eq!(CandidateList::new(vec![], 0, 0, None, false).page_size, 1);
        assert_eq!(CandidateList::new(vec![], 0, 9, None, false).page_count(), 1);
    }

    #[test]
    fn character_forms() {
        assert_eq!(CharacterForms::Common.apply("臺灣臺北"), "台灣台北");
        assert_eq!(CharacterForms::Standard.apply("臺灣"), "臺灣");
        assert_eq!(CharacterForms::Common.apply("臺\u{FE00}"), "臺\u{FE00}");
        assert_eq!(TypoTolerance::Aggressive.cost_multiplier(), 0.7);
        assert_eq!(TypoTolerance::from_raw("off"), Some(TypoTolerance::Off));
        assert_eq!(KeyDecoderMethod::from_raw("timed"), Some(KeyDecoderMethod::Timed));
    }
}

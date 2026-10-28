//! The composer handle: keys in, commits and the presentation out, and the hooks through which the host's models
//! choose sentences and score candidates.

use std::ffi::{c_char, c_void};

use zhuyin_core::api::{
    CandidateAnchor, CharacterForms, CharacterModelSyllable, ComposerOptions, KeyDecoderMethod, KeyInput, KeyResult,
    PunctuationKeys, ScoredSentence, SpaceAction, TypoTolerance,
};
use zhuyin_core::composer::{Composer, VariantScores};

use crate::ZtEngine;
use crate::strings::{ZtStrings, borrowed, owned, owned_optional};

/// One composition state machine.  Used by one thread at a time.
pub struct ZtComposer {
    pub(crate) composer: Composer,
}

impl std::fmt::Debug for ZtComposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZtComposer").finish_non_exhaustive()
    }
}

/// `ZtKey::kind`: a printable ASCII key with no modifier (`character`).
pub const ZT_KEY_CHAR: u32 = 0;
/// `ZtKey::kind`: a printable key with Shift held, named by its unshifted `character`.
pub const ZT_KEY_SHIFTED_CHAR: u32 = 1;
/// `ZtKey::kind`: a digit or symbol key with Option held: the `character` printed on it.
pub const ZT_KEY_PRINTED: u32 = 2;
/// `ZtKey::kind`: a symbol key with Control held, named by the `character` printed on it.
pub const ZT_KEY_CONTROL_CHAR: u32 = 3;
/// `ZtKey::kind`: Space.
pub const ZT_KEY_SPACE: u32 = 4;
/// `ZtKey::kind`: Shift+Space.
pub const ZT_KEY_SHIFT_SPACE: u32 = 5;
/// `ZtKey::kind`: Return.
pub const ZT_KEY_ENTER: u32 = 6;
/// `ZtKey::kind`: Shift+Return.
pub const ZT_KEY_SHIFT_ENTER: u32 = 7;
/// `ZtKey::kind`: Backspace.
pub const ZT_KEY_BACKSPACE: u32 = 8;
/// `ZtKey::kind`: Option+Backspace.
pub const ZT_KEY_OPTION_BACKSPACE: u32 = 9;
/// `ZtKey::kind`: forward delete.
pub const ZT_KEY_FORWARD_DELETE: u32 = 10;
/// `ZtKey::kind`: Escape.
pub const ZT_KEY_ESCAPE: u32 = 11;
/// `ZtKey::kind`: Tab.
pub const ZT_KEY_TAB: u32 = 12;
/// `ZtKey::kind`: Shift+Tab.
pub const ZT_KEY_SHIFT_TAB: u32 = 13;
/// `ZtKey::kind`: ←.
pub const ZT_KEY_LEFT: u32 = 14;
/// `ZtKey::kind`: →.
pub const ZT_KEY_RIGHT: u32 = 15;
/// `ZtKey::kind`: ↑.
pub const ZT_KEY_UP: u32 = 16;
/// `ZtKey::kind`: ↓.
pub const ZT_KEY_DOWN: u32 = 17;
/// `ZtKey::kind`: Home.
pub const ZT_KEY_HOME: u32 = 18;
/// `ZtKey::kind`: End.
pub const ZT_KEY_END: u32 = 19;
/// `ZtKey::kind`: Page Up.
pub const ZT_KEY_PAGE_UP: u32 = 20;
/// `ZtKey::kind`: Page Down.
pub const ZT_KEY_PAGE_DOWN: u32 = 21;
/// `ZtKey::kind`: Shift+←.
pub const ZT_KEY_SHIFT_LEFT: u32 = 22;
/// `ZtKey::kind`: Shift+→.
pub const ZT_KEY_SHIFT_RIGHT: u32 = 23;
/// `ZtKey::kind`: Shift+↓ (candidates for the one character at the target).
pub const ZT_KEY_SHIFT_DOWN: u32 = 24;
/// `ZtKey::kind`: Shift+↑ (typing corrections first).
pub const ZT_KEY_SHIFT_UP: u32 = 25;

/// A key (Swift's `KeyInput`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtKey {
    /// One of the `ZT_KEY_*` constants.
    pub kind: u32,
    /// The Unicode scalar for the character kinds; ignored otherwise.
    pub character: u32,
}

fn key_input(key: ZtKey) -> Option<KeyInput> {
    let character = || char::from_u32(key.character);
    Some(match key.kind {
        ZT_KEY_CHAR => KeyInput::Char(character()?),
        ZT_KEY_SHIFTED_CHAR => KeyInput::ShiftedChar(character()?),
        ZT_KEY_PRINTED => KeyInput::Printed(character()?),
        ZT_KEY_CONTROL_CHAR => KeyInput::ControlChar(character()?),
        ZT_KEY_SPACE => KeyInput::Space,
        ZT_KEY_SHIFT_SPACE => KeyInput::ShiftSpace,
        ZT_KEY_ENTER => KeyInput::Enter,
        ZT_KEY_SHIFT_ENTER => KeyInput::ShiftEnter,
        ZT_KEY_BACKSPACE => KeyInput::Backspace,
        ZT_KEY_OPTION_BACKSPACE => KeyInput::OptionBackspace,
        ZT_KEY_FORWARD_DELETE => KeyInput::ForwardDelete,
        ZT_KEY_ESCAPE => KeyInput::Escape,
        ZT_KEY_TAB => KeyInput::Tab,
        ZT_KEY_SHIFT_TAB => KeyInput::ShiftTab,
        ZT_KEY_LEFT => KeyInput::Left,
        ZT_KEY_RIGHT => KeyInput::Right,
        ZT_KEY_UP => KeyInput::Up,
        ZT_KEY_DOWN => KeyInput::Down,
        ZT_KEY_HOME => KeyInput::Home,
        ZT_KEY_END => KeyInput::End,
        ZT_KEY_PAGE_UP => KeyInput::PageUp,
        ZT_KEY_PAGE_DOWN => KeyInput::PageDown,
        ZT_KEY_SHIFT_LEFT => KeyInput::ShiftLeft,
        ZT_KEY_SHIFT_RIGHT => KeyInput::ShiftRight,
        ZT_KEY_SHIFT_DOWN => KeyInput::ShiftDown,
        ZT_KEY_SHIFT_UP => KeyInput::ShiftUp,
        _ => return None,
    })
}

/// What a key did (Swift's `KeyResult`).
#[repr(C)]
#[derive(Debug)]
pub struct ZtKeyResult {
    /// False: the key goes on to the application (after `commit`, if any).
    pub handled: bool,
    /// Text to insert before showing the new presentation, or NULL; free with `zt_string_free`.
    pub commit: *mut c_char,
}

fn key_result(result: &KeyResult) -> ZtKeyResult {
    ZtKeyResult { handled: result.handled, commit: owned_optional(result.commit.as_deref()) }
}

/// `ZtComposerOptions::space_action`: Space opens the candidate window.
pub const ZT_SPACE_OPEN_CANDIDATES: u32 = 0;
/// `ZtComposerOptions::space_action`: Space commits.
pub const ZT_SPACE_COMMIT: u32 = 1;
/// `ZtComposerOptions::space_action`: Space commits and the app receives the Space.
pub const ZT_SPACE_COMMIT_AND_SPACE: u32 = 2;
/// `ZtComposerOptions::space_action`: Space puts a space into the composition (as Shift+Space); nothing is sent.
pub const ZT_SPACE_INSERT_SPACE: u32 = 3;
/// `ZtComposerOptions::typo_tolerance`: off.
pub const ZT_TYPO_OFF: u32 = 0;
/// `ZtComposerOptions::typo_tolerance`: standard.
pub const ZT_TYPO_STANDARD: u32 = 1;
/// `ZtComposerOptions::typo_tolerance`: aggressive.
pub const ZT_TYPO_AGGRESSIVE: u32 = 2;
/// `ZtComposerOptions::candidate_anchor`: spans start at the cursor.
pub const ZT_ANCHOR_AFTER_CURSOR: u32 = 0;
/// `ZtComposerOptions::candidate_anchor`: spans end at the cursor.
pub const ZT_ANCHOR_BEFORE_CURSOR: u32 = 1;
/// `ZtComposerOptions::key_decoder`: slots.
pub const ZT_DECODER_SLOTS: u32 = 0;
/// `ZtComposerOptions::key_decoder`: boundaries.
pub const ZT_DECODER_BOUNDARIES: u32 = 1;
/// `ZtComposerOptions::key_decoder`: reorder.
pub const ZT_DECODER_REORDER: u32 = 2;
/// `ZtComposerOptions::key_decoder`: timed (the default).
pub const ZT_DECODER_TIMED: u32 = 3;
/// `ZtComposerOptions::character_forms`: the everyday forms (台).
pub const ZT_FORMS_COMMON: u32 = 0;
/// `ZtComposerOptions::character_forms`: the standard forms (臺).
pub const ZT_FORMS_STANDARD: u32 = 1;
/// `ZtComposerOptions::punctuation_keys`: as Windows' Microsoft Bopomofo.
pub const ZT_PUNCTUATION_WINDOWS: u32 = 0;
/// `ZtComposerOptions::punctuation_keys`: as macOS Zhuyin.
pub const ZT_PUNCTUATION_MAC: u32 = 1;

/// The composer's options (Swift's `ComposerOptions`); start from [`zt_composer_options_default`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtComposerOptions {
    /// Keys that pick candidates 1..n on the current page (UTF-8, one character per key); also sets the page size.
    pub selection_keys: *const c_char,
    /// One of `ZT_SPACE_*`.
    pub space_action: u32,
    /// Escape with no window open clears the whole composition.
    pub escape_clears_composition: bool,
    /// Learn from commits and picks.
    pub learning_enabled: bool,
    /// Longest composition, in syllables.
    pub max_syllables: usize,
    /// One of `ZT_TYPO_*`.
    pub typo_tolerance: u32,
    /// One of `ZT_ANCHOR_*`.
    pub candidate_anchor: u32,
    /// One of `ZT_DECODER_*`.
    pub key_decoder: u32,
    /// The candidate window lists top to bottom.
    pub vertical_list: bool,
    /// Moving the highlight past a page's edge turns the page (stopping at the list's ends); false: it stays.
    pub page_at_list_edge: bool,
    /// One of `ZT_FORMS_*`.
    pub character_forms: u32,
    /// One of `ZT_PUNCTUATION_*`.
    pub punctuation_keys: u32,
}

static DEFAULT_SELECTION_KEYS: &[u8] = b"123456789\0";

/// The default options (selection keys point to a static string).
#[unsafe(no_mangle)]
pub extern "C" fn zt_composer_options_default() -> ZtComposerOptions {
    let options = ComposerOptions::default();
    ZtComposerOptions {
        selection_keys: DEFAULT_SELECTION_KEYS.as_ptr().cast(),
        space_action: options.space_action as u32,
        escape_clears_composition: options.escape_clears_composition,
        learning_enabled: options.learning_enabled,
        max_syllables: options.max_syllables,
        typo_tolerance: options.typo_tolerance as u32,
        candidate_anchor: options.candidate_anchor as u32,
        key_decoder: options.key_decoder as u32,
        vertical_list: options.vertical_list,
        page_at_list_edge: options.page_at_list_edge,
        character_forms: options.character_forms as u32,
        punctuation_keys: options.punctuation_keys as u32,
    }
}

/// # Safety
/// `options` is NULL or valid with a C string (or NULL) for the selection keys.
unsafe fn composer_options(options: *const ZtComposerOptions) -> Option<ComposerOptions> {
    // SAFETY: NULL or valid per the contract.
    let Some(o) = (unsafe { options.as_ref() }) else { return Some(ComposerOptions::default()) };
    let pick = |value: u32, count: usize| (value as usize) < count;
    if !pick(o.space_action, SpaceAction::ALL.len())
        || !pick(o.typo_tolerance, 3)
        || !pick(o.candidate_anchor, 2)
        || !pick(o.key_decoder, 4)
        || !pick(o.character_forms, 2)
        || !pick(o.punctuation_keys, 2)
    {
        return None;
    }
    // SAFETY: NULL or a C string.
    let keys =
        unsafe { borrowed(o.selection_keys) }.map_or_else(|| "123456789".chars().collect(), |k| k.chars().collect());
    Some(ComposerOptions {
        selection_keys: keys,
        space_action: SpaceAction::ALL[o.space_action as usize],
        escape_clears_composition: o.escape_clears_composition,
        learning_enabled: o.learning_enabled,
        max_syllables: o.max_syllables,
        typo_tolerance: TypoTolerance::ALL[o.typo_tolerance as usize],
        candidate_anchor: [CandidateAnchor::AfterCursor, CandidateAnchor::BeforeCursor][o.candidate_anchor as usize],
        key_decoder: KeyDecoderMethod::ALL[o.key_decoder as usize],
        vertical_list: o.vertical_list,
        page_at_list_edge: o.page_at_list_edge,
        character_forms: CharacterForms::ALL[o.character_forms as usize],
        punctuation_keys: PunctuationKeys::ALL[o.punctuation_keys as usize],
    })
}

/// A composer on `engine` with `options` (NULL: the defaults); NULL when an option is out of range.
///
/// # Safety
/// `engine` is a live handle; `options` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_new(
    engine: *const ZtEngine,
    options: *const ZtComposerOptions,
) -> *mut ZtComposer {
    // SAFETY: per the contract above.
    let (Some(engine), Some(options)) = (unsafe { (engine.as_ref(), composer_options(options)) }) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ZtComposer { composer: Composer::new(engine.engine.clone(), options) }))
}

/// Frees a composer (releasing its hooks' contexts).  NULL is ignored.
///
/// # Safety
/// `composer` is NULL or a handle from this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_free(composer: *mut ZtComposer) {
    if !composer.is_null() {
        // SAFETY: allocated by `zt_composer_new`.
        drop(unsafe { Box::from_raw(composer) });
    }
}

/// # Safety
/// `composer` is NULL or a live handle not used elsewhere during the call.
unsafe fn composer_mut<'a>(composer: *mut ZtComposer) -> Option<&'a mut Composer> {
    // SAFETY: per the contract above.
    unsafe { composer.as_mut() }.map(|c| &mut c.composer)
}

/// Replaces the options; false (nothing changed) when one is out of range.
///
/// # Safety
/// `composer` is a live handle; `options` is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_options(composer: *mut ZtComposer, options: *const ZtComposerOptions) -> bool {
    // SAFETY: per the contract above.
    let (Some(composer), Some(options)) = (unsafe { (composer_mut(composer), composer_options(options)) }) else {
        return false;
    };
    composer.set_options(options);
    true
}

/// Handles a key at the current time.  An unknown key passes through.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_handle(composer: *mut ZtComposer, key: ZtKey) -> ZtKeyResult {
    // SAFETY: live handle.
    match (unsafe { composer_mut(composer) }, key_input(key)) {
        (Some(composer), Some(key)) => key_result(&composer.handle(key)),
        _ => key_result(&KeyResult::PASS_THROUGH),
    }
}

/// Handles a key pressed at `time` (seconds, the clock of the key events).
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_handle_at(composer: *mut ZtComposer, key: ZtKey, time: f64) -> ZtKeyResult {
    // SAFETY: live handle.
    match (unsafe { composer_mut(composer) }, key_input(key)) {
        (Some(composer), Some(key)) => key_result(&composer.handle_at(key, time)),
        _ => key_result(&KeyResult::PASS_THROUGH),
    }
}

/// Picks candidate `index` of the open list.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_select_candidate(composer: *mut ZtComposer, index: usize) -> ZtKeyResult {
    // SAFETY: live handle.
    match unsafe { composer_mut(composer) } {
        Some(composer) => key_result(&composer.select_candidate(index)),
        None => key_result(&KeyResult::PASS_THROUGH),
    }
}

/// Commits the whole composition; returns the text (free with `zt_string_free`).
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_commit_all(composer: *mut ZtComposer) -> *mut c_char {
    // SAFETY: live handle.
    unsafe { composer_mut(composer) }.map_or(std::ptr::null_mut(), |c| owned(&c.commit_all()))
}

/// The insertion point may have moved: the next composition starts a sentence.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_break_context(composer: *mut ZtComposer) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.break_context();
    }
}

/// Drops the composition without committing.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_reset(composer: *mut ZtComposer) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.reset();
    }
}

/// The candidate window switches between words for the keys as typed and typing corrections.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_toggle_corrections(composer: *mut ZtComposer) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.toggle_corrections();
    }
}

/// The text before the insertion point, read from the app when a composition starts.
///
/// # Safety
/// `composer` is a live handle; `text` a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_preceding_text(composer: *mut ZtComposer, text: *const c_char) {
    // SAFETY: per the contract above.
    if let (Some(composer), Some(text)) = unsafe { (composer_mut(composer), borrowed(text)) } {
        composer.set_preceding_text(text);
    }
}

/// Whether something is being composed.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_is_composing(composer: *mut ZtComposer) -> bool {
    // SAFETY: live handle.
    unsafe { composer_mut(composer) }.is_some_and(|c| c.is_composing())
}

/// Pauses learning (secure input).
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_learning_suspended(composer: *mut ZtComposer, suspended: bool) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.learning_suspended = suspended;
    }
}

/// Whether learning is paused.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_learning_suspended(composer: *mut ZtComposer) -> bool {
    // SAFETY: live handle.
    unsafe { composer_mut(composer) }.is_some_and(|c| c.learning_suspended)
}

/// How many sentences the reranker hook chooses among.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_reranking_count(composer: *mut ZtComposer, count: usize) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.set_reranking_count(count);
    }
}

/// How much the character scorer's scores count in the lattice.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_character_weight(composer: *mut ZtComposer, weight: f64) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.character_weight = weight;
    }
}

/// How the character scorer scores characters only a reading-channel variant brings: 0 by their lexicon reading
/// (one more call), 1 the position's best, 2 nothing, 3 as typed; only those within `window` nats of the best other
/// character at their position.  Other values leave the setting.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_variant_scores(composer: *mut ZtComposer, mode: u8, window: f64) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        if let Some(&mode) = VariantScores::ALL.get(usize::from(mode)) {
            composer.variant_scores = mode;
        }
        composer.variant_window = window;
    }
}

/// Whether the character scorer also orders the candidate list.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_character_scores_in_list(composer: *mut ZtComposer, in_list: bool) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.character_scores_in_list = in_list;
    }
}

/// Whether a pick freezes the other words of the composition.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_freezes_on_pick(composer: *mut ZtComposer, freezes: bool) {
    // SAFETY: live handle.
    if let Some(composer) = unsafe { composer_mut(composer) } {
        composer.freezes_on_pick = freezes;
    }
}

/// A UTF-16 range.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtRange {
    /// First unit.
    pub start: usize,
    /// One past the last unit.
    pub end: usize,
}

/// One candidate.
#[repr(C)]
#[derive(Debug)]
pub struct ZtCandidate {
    /// The candidate.
    pub text: *mut c_char,
    /// Small grey text after it, or NULL.
    pub annotation: *mut c_char,
}

/// The open candidate window.
#[repr(C)]
#[derive(Debug)]
pub struct ZtCandidateList {
    /// `count` candidates.
    pub items: *mut ZtCandidate,
    /// How many.
    pub count: usize,
    /// The highlighted index.
    pub selection: usize,
    /// Candidates per page.
    pub page_size: usize,
    /// 1: the list shows typing corrections; 0: only words for the keys as typed; −1: it cannot have corrections.
    pub tolerant: i8,
    /// Only the one character at the target.
    pub single_character: bool,
}

/// What to draw (Swift's `Presentation`).  Offsets are UTF-16 units.
#[repr(C)]
#[derive(Debug)]
pub struct ZtPresentation {
    /// The marked text.
    pub text: *mut c_char,
    /// The caret in `text`.
    pub cursor: usize,
    /// The words of `text`, for clause underlines.
    pub clauses: *mut ZtRange,
    /// How many clauses.
    pub clause_count: usize,
    /// Index into `clauses` the highlighted candidate replaces, or −1.
    pub focused_clause: isize,
    /// Index into `clauses` of the character ↓ would change, or −1.
    pub target_clause: isize,
    /// The open candidate window, or NULL.
    pub candidates: *mut ZtCandidateList,
    /// A notice, or NULL.
    pub notice: *mut c_char,
    /// Standard plain Zhuyin for the highlighted span, or NULL.
    pub zhuyin_hint: *mut c_char,
}

fn index(value: Option<usize>) -> isize {
    value.and_then(|v| isize::try_from(v).ok()).unwrap_or(-1)
}

/// The current presentation (free with [`zt_presentation_free`]).
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_presentation(composer: *mut ZtComposer) -> *mut ZtPresentation {
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return std::ptr::null_mut() };
    let p = composer.presentation();
    let clauses: Box<[ZtRange]> = p.clauses.iter().map(|r| ZtRange { start: r.start, end: r.end }).collect();
    let clause_count = clauses.len();
    let candidates = p.candidates.as_ref().map_or(std::ptr::null_mut(), |list| {
        let items: Box<[ZtCandidate]> = list
            .items
            .iter()
            .map(|c| ZtCandidate { text: owned(&c.text), annotation: owned_optional(c.annotation.as_deref()) })
            .collect();
        let count = items.len();
        Box::into_raw(Box::new(ZtCandidateList {
            items: Box::into_raw(items).cast(),
            count,
            selection: list.selection,
            page_size: list.page_size,
            tolerant: list.tolerant.map_or(-1, i8::from),
            single_character: list.single_character,
        }))
    });
    Box::into_raw(Box::new(ZtPresentation {
        text: owned(&p.text),
        cursor: p.cursor,
        clauses: Box::into_raw(clauses).cast(),
        clause_count,
        focused_clause: index(p.focused_clause),
        target_clause: index(p.target_clause),
        candidates,
        notice: owned_optional(p.notice.as_deref()),
        zhuyin_hint: owned_optional(p.zhuyin_hint.as_deref()),
    }))
}

/// Frees a presentation.  NULL is ignored.
///
/// # Safety
/// `presentation` is NULL or returned by [`zt_composer_presentation`] and not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_presentation_free(presentation: *mut ZtPresentation) {
    if presentation.is_null() {
        return;
    }
    // SAFETY: every pointer was allocated by `zt_composer_presentation`.
    unsafe {
        let p = Box::from_raw(presentation);
        crate::zt_string_free(p.text);
        crate::zt_string_free(p.notice);
        crate::zt_string_free(p.zhuyin_hint);
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(p.clauses, p.clause_count)));
        if !p.candidates.is_null() {
            let list = Box::from_raw(p.candidates);
            let items = Box::from_raw(std::ptr::slice_from_raw_parts_mut(list.items, list.count));
            for item in &*items {
                crate::zt_string_free(item.text);
                crate::zt_string_free(item.annotation);
            }
        }
    }
}

// MARK: - Hooks

/// The engine's best sentences, passed to a reranker hook; valid during the callback.  Read with the
/// `zt_sentences_*` functions or hand it to `zt_reranker_choose`.
#[derive(Debug)]
pub struct ZtSentences {
    pub(crate) sentences: *const ScoredSentence,
    pub(crate) count: usize,
}

impl ZtSentences {
    /// # Safety
    /// Used while the slice it was made from lives (during the callback).
    pub(crate) unsafe fn slice(&self) -> &[ScoredSentence] {
        // SAFETY: made from a live slice; the callback contract keeps it alive.
        unsafe { std::slice::from_raw_parts(self.sentences, self.count) }
    }
}

/// # Safety
/// `sentences` is NULL or a live view.
unsafe fn sentence<'a>(sentences: *const ZtSentences, index: usize) -> Option<&'a ScoredSentence> {
    // SAFETY: per the contract above.
    unsafe { sentences.as_ref() }.and_then(|s| unsafe { s.slice() }.get(index))
}

/// How many sentences.
///
/// # Safety
/// `sentences` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_sentences_count(sentences: *const ZtSentences) -> usize {
    // SAFETY: live view.
    unsafe { sentences.as_ref() }.map_or(0, |s| s.count)
}

/// Sentence `index`'s text (free with `zt_string_free`); NULL when out of range.
///
/// # Safety
/// `sentences` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_sentences_text(sentences: *const ZtSentences, index: usize) -> *mut c_char {
    // SAFETY: live view.
    unsafe { sentence(sentences, index) }.map_or(std::ptr::null_mut(), |s| owned(&s.text()))
}

/// Sentence `index`'s decoder score (natural log); NaN when out of range.
///
/// # Safety
/// `sentences` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_sentences_score(sentences: *const ZtSentences, index: usize) -> f64 {
    // SAFETY: live view.
    unsafe { sentence(sentences, index) }.map_or(f64::NAN, |s| s.score)
}

/// Whether sentence `index` differs from the keys as typed.
///
/// # Safety
/// `sentences` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_sentences_corrected(sentences: *const ZtSentences, index: usize) -> bool {
    // SAFETY: live view.
    unsafe { sentence(sentences, index) }.is_some_and(ScoredSentence::corrected)
}

/// The host's context pointer and its release function, released once when dropped.
struct Context {
    context: *mut c_void,
    release: Option<unsafe extern "C" fn(*mut c_void)>,
}

// SAFETY: the host promises (header: Threading) that a hook's context may be used from whichever thread drives the
// composer, one call at a time.
unsafe impl Send for Context {}

impl Context {
    fn get(&self) -> *mut c_void {
        self.context
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            // SAFETY: the host's release function for its own context, called once.
            unsafe { release(self.context) };
        }
    }
}

/// The reranker hook (Swift's `Composer.reranker`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtRerankerHook {
    /// Passed to `choose` and `release`.
    pub context: *mut c_void,
    /// Chooses the displayed sentence: returns an index into `sentences`, or a negative number to keep the first.
    /// NULL removes the hook.
    pub choose: Option<unsafe extern "C" fn(context: *mut c_void, sentences: *const ZtSentences) -> isize>,
    /// Called once when the hook is replaced or the composer freed; may be NULL.
    pub release: Option<unsafe extern "C" fn(context: *mut c_void)>,
}

/// Sets (or, with a NULL `choose`, removes) the reranker hook.
///
/// # Safety
/// `composer` is a live handle; the hook follows the threading and ownership rules of the header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_reranker(composer: *mut ZtComposer, hook: ZtRerankerHook) {
    let context = Context { context: hook.context, release: hook.release };
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return };
    let Some(choose) = hook.choose else {
        composer.set_reranker(None);
        return;
    };
    composer.set_reranker(Some(Box::new(move |sentences: &[ScoredSentence]| {
        let view = ZtSentences { sentences: sentences.as_ptr(), count: sentences.len() };
        // SAFETY: the host's callback with its own context and a view valid for the call.
        let index = unsafe { choose(context.get(), &raw const view) };
        usize::try_from(index).ok()
    })));
}

/// The candidate scorer hook (Swift's `Composer.candidateScorer`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtCandidateScorerHook {
    /// Passed to `score` and `release`.
    pub context: *mut c_void,
    /// Scores candidates for list order: writes one score per candidate to `scores` and returns true, or returns
    /// false to leave the order as it is.  `before` is the composition text before the candidates' span.  NULL
    /// removes the hook.
    pub score: Option<
        unsafe extern "C" fn(
            context: *mut c_void,
            before: *const c_char,
            candidates: *const ZtStrings,
            scores: *mut f64,
        ) -> bool,
    >,
    /// Called once when the hook is replaced or the composer freed; may be NULL.
    pub release: Option<unsafe extern "C" fn(context: *mut c_void)>,
}

/// Sets (or, with a NULL `score`, removes) the candidate scorer hook.
///
/// # Safety
/// `composer` is a live handle; the hook follows the threading and ownership rules of the header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_candidate_scorer(composer: *mut ZtComposer, hook: ZtCandidateScorerHook) {
    let context = Context { context: hook.context, release: hook.release };
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return };
    let Some(score) = hook.score else {
        composer.candidate_scorer = None;
        return;
    };
    composer.candidate_scorer = Some(Box::new(move |before: &str, candidates: &[String]| {
        let before = std::ffi::CString::new(before).ok()?;
        let view = ZtStrings::new(candidates);
        let mut scores = vec![0.0; candidates.len()];
        // SAFETY: the host's callback with its own context, a view valid for the call and room for every score.
        let ok = unsafe { score(context.get(), before.as_ptr(), &raw const view, scores.as_mut_ptr()) };
        ok.then_some(scores)
    }));
}

/// One syllable as the character model reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtSyllable {
    /// Toneless reading (UTF-8), or NULL for a literal.
    pub base: *const c_char,
    /// 0 none (literal), 1 first, 2 ˊ, 3 ˇ, 4 ˋ, 5 ˙.
    pub tone: i32,
    /// 0 exact … 3 large correction.
    pub cost_bucket: i32,
    /// The syllable ended without a tone key or Space.
    pub implicit: bool,
}

/// Each syllable's candidate characters, passed to a character scorer hook; valid during the callback.  Read with
/// the `zt_candidate_rows_*` functions or hand it to
/// `zt_syllable_scorer_scores_rows`.
#[derive(Debug)]
pub struct ZtCandidateRows {
    pub(crate) rows: *const Vec<String>,
    pub(crate) count: usize,
}

impl ZtCandidateRows {
    /// # Safety
    /// Used while the slice it was made from lives (during the callback).
    pub(crate) unsafe fn slice(&self) -> &[Vec<String>] {
        // SAFETY: made from a live slice; the callback contract keeps it alive.
        unsafe { std::slice::from_raw_parts(self.rows, self.count) }
    }
}

/// How many rows (syllables).
///
/// # Safety
/// `rows` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_candidate_rows_count(rows: *const ZtCandidateRows) -> usize {
    // SAFETY: live view.
    unsafe { rows.as_ref() }.map_or(0, |r| r.count)
}

/// How many candidates row `row` has.
///
/// # Safety
/// `rows` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_candidate_rows_length(rows: *const ZtCandidateRows, row: usize) -> usize {
    // SAFETY: live view.
    unsafe { rows.as_ref() }.and_then(|r| unsafe { r.slice() }.get(row)).map_or(0, Vec::len)
}

/// Candidate `index` of row `row` (free with `zt_string_free`); NULL when out of range.
///
/// # Safety
/// `rows` is the view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_candidate_rows_get(rows: *const ZtCandidateRows, row: usize, index: usize) -> *mut c_char {
    // SAFETY: live view.
    unsafe { rows.as_ref() }
        .and_then(|r| unsafe { r.slice() }.get(row))
        .and_then(|r| r.get(index))
        .map_or(std::ptr::null_mut(), |c| owned(c))
}

/// The character scorer hook (Swift's `Composer.characterScorer`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtCharacterScorerHook {
    /// Passed to `score` and `release`.
    pub context: *mut c_void,
    /// Scores each syllable's candidates: row `i` (of `count`) has room for its candidate count of scores, rows
    /// packed one after another in `scores`; writes how many it scored (at most the row's length) to `lengths[i]` and
    /// returns true, or returns false when it cannot score them.  NULL removes the hook.
    pub score: Option<
        unsafe extern "C" fn(
            context: *mut c_void,
            syllables: *const ZtSyllable,
            count: usize,
            candidates: *const ZtCandidateRows,
            scores: *mut f64,
            lengths: *mut usize,
        ) -> bool,
    >,
    /// Called once when the hook is replaced or the composer freed; may be NULL.
    pub release: Option<unsafe extern "C" fn(context: *mut c_void)>,
}

/// C views of syllables; the strings live as long as the returned owner.
pub(crate) fn c_syllables(syllables: &[CharacterModelSyllable]) -> (Vec<Option<std::ffi::CString>>, Vec<ZtSyllable>) {
    let bases: Vec<Option<std::ffi::CString>> =
        syllables.iter().map(|s| s.base.as_deref().and_then(|b| std::ffi::CString::new(b).ok())).collect();
    let views = syllables
        .iter()
        .zip(&bases)
        .map(|(s, base)| ZtSyllable {
            base: base.as_ref().map_or(std::ptr::null(), |b| b.as_ptr()),
            tone: s.tone,
            cost_bucket: s.cost_bucket,
            implicit: s.implicit,
        })
        .collect();
    (bases, views)
}

/// Rows of packed scores, `lengths[i]` of row `i` (each at most its candidate count).
pub(crate) fn unpack(scores: &[f64], lengths: &[usize], candidates: &[Vec<String>]) -> Option<Vec<Vec<f64>>> {
    let mut start = 0;
    let mut rows = Vec::with_capacity(candidates.len());
    for (list, &length) in candidates.iter().zip(lengths) {
        if length > list.len() {
            return None;
        }
        rows.push(scores[start..start + length].to_vec());
        start += list.len();
    }
    Some(rows)
}

/// Sets (or, with a NULL `score`, removes) the character scorer hook.
///
/// # Safety
/// `composer` is a live handle; the hook follows the threading and ownership rules of the header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_set_character_scorer(composer: *mut ZtComposer, hook: ZtCharacterScorerHook) {
    let context = Context { context: hook.context, release: hook.release };
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return };
    let Some(score) = hook.score else {
        composer.set_character_scorer(None);
        return;
    };
    composer.set_character_scorer(Some(Box::new(
        move |syllables: &[CharacterModelSyllable], candidates: &[Vec<String>]| {
            let (_bases, views) = c_syllables(syllables);
            let rows = ZtCandidateRows { rows: candidates.as_ptr(), count: candidates.len() };
            let mut scores = vec![0.0; candidates.iter().map(Vec::len).sum()];
            let mut lengths = vec![0; candidates.len()];
            // SAFETY: the host's callback with its own context, views valid for the call and room for every score.
            let ok = unsafe {
                score(
                    context.get(),
                    views.as_ptr(),
                    views.len(),
                    &raw const rows,
                    scores.as_mut_ptr(),
                    lengths.as_mut_ptr(),
                )
            };
            if ok { unpack(&scores, &lengths, candidates) } else { None }
        },
    )));
}

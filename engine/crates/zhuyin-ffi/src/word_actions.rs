//! Word actions from the candidate window: 不要再推薦 (0), 忘記這個選字 (1), 加入自訂詞 (2), and their undo.

use std::ffi::c_char;

use zhuyin_core::composer::{CandidateAction, Composer};

use crate::composer::composer_mut;
use crate::engine::{engine_ref, phrase_list};
use crate::strings::{borrowed, owned};
use crate::{ZtComposer, ZtEngine, ZtPhraseList};

/// `index` from C: below zero is the highlighted candidate (or, with no list open, the whole composition).
fn candidate_index(index: isize) -> Option<usize> {
    usize::try_from(index).ok()
}

/// Whether `action` (0 block, 1 forget, 2 add a custom word) applies to candidate `index` of the open list; a
/// negative `index` is the highlighted candidate, or with no list open the whole composition (adding only).
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_candidate_action_available(
    composer: *mut ZtComposer,
    action: u32,
    index: isize,
) -> bool {
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return false };
    CandidateAction::from_raw(action).is_some_and(|a| composer.candidate_action_available(a, candidate_index(index)))
}

/// Does `action` on candidate `index` (as for `zt_composer_candidate_action_available`) and returns the notice the
/// candidate window shows (free with `zt_string_free`), or NULL when the action does not apply.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_perform_candidate_action(
    composer: *mut ZtComposer,
    action: u32,
    index: isize,
) -> *mut c_char {
    // SAFETY: live handle.
    let Some(composer) = (unsafe { composer_mut(composer) }) else { return std::ptr::null_mut() };
    let Some(action) = CandidateAction::from_raw(action) else { return std::ptr::null_mut() };
    composer.perform_candidate_action(action, candidate_index(index)).map_or(std::ptr::null_mut(), |n| owned(&n))
}

/// Whether the last word action can be undone.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_can_undo_candidate_action(composer: *mut ZtComposer) -> bool {
    // SAFETY: live handle.
    unsafe { composer_mut(composer) }.is_some_and(|c| c.can_undo_candidate_action())
}

/// Undoes the last word action; false when there is none.
///
/// # Safety
/// `composer` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_composer_undo_candidate_action(composer: *mut ZtComposer) -> bool {
    // SAFETY: live handle.
    unsafe { composer_mut(composer) }.is_some_and(Composer::undo_candidate_action)
}

/// The words never suggested (不要再推薦), sorted; free with `zt_phrase_list_free`.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_blocked_entries(engine: *const ZtEngine) -> *mut ZtPhraseList {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return std::ptr::null_mut() };
    phrase_list(engine.blocked_entries().into_iter().map(|p| (p.reading, p.word, 0.0)))
}

/// Suggests a blocked word again.
///
/// # Safety
/// `engine` is a live handle; the strings are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_unblock(engine: *const ZtEngine, word: *const c_char, reading: *const c_char) {
    // SAFETY: per the contract above.
    let arguments = unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) };
    if let (Some(engine), Some(word), Some(reading)) = arguments {
        engine.unblock(word, reading);
    }
}

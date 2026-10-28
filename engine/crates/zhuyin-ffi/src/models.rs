//! The neural models: their decisions in Rust ([`zhuyin_models`]), their tensor inference supplied by the host as C
//! callbacks (Core ML on macOS).

use std::ffi::{c_char, c_void};
use std::path::Path;
use std::sync::Arc;

use zhuyin_core::api::CharacterModelSyllable;
use zhuyin_models::{
    ModelError, NeuralReranker, PackedTree, RerankerModel, SyllableInput, SyllableModel, SyllableScorer,
};

use crate::composer::{ZtCandidateRows, ZtSentences, ZtSyllable};
use crate::strings::{ZtStrings, borrowed, set_error, string_array};

/// The host's context pointer and its release function, released once when dropped.
struct HostContext {
    context: *mut c_void,
    release: Option<unsafe extern "C" fn(*mut c_void)>,
}

// SAFETY: the host promises (header: Threading) that a model's context may be used from any thread, one call at a
// time; the scorers serialize their model calls.
unsafe impl Send for HostContext {}

impl Drop for HostContext {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            // SAFETY: the host's release function for its own context, called once.
            unsafe { release(self.context) };
        }
    }
}

/// The reranker's prefix tree, as the tree model reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtPackedTree {
    /// `node_count` token IDs (shape `[1, node_count]`).
    pub ids: *const i32,
    /// `node_count` depths (shape `[1, node_count]`).
    pub positions: *const i32,
    /// How many nodes.
    pub node_count: usize,
    /// `node_count × node_count` ancestor mask, row-major.
    pub ancestors: *const i32,
    /// `target_count` parent node indices.
    pub parents: *const i32,
    /// `target_count` target tokens.
    pub targets: *const i32,
    /// How many targets (and scores to write).
    pub target_count: usize,
}

/// Batched reranker inference: `rows × length` IDs and targets in; one log probability per target out (written to
/// `logprob`, row-major).  Returns false when the model cannot run.
type ZtRerankerLogprobFn = unsafe extern "C" fn(
    context: *mut c_void,
    ids: *const i32,
    targets: *const i32,
    rows: usize,
    length: usize,
    logprob: *mut f64,
) -> bool;

/// The host's reranker inference.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtRerankerModel {
    /// Passed to the functions.
    pub context: *mut c_void,
    /// Required.
    pub logprob: Option<
        unsafe extern "C" fn(
            context: *mut c_void,
            ids: *const i32,
            targets: *const i32,
            rows: usize,
            length: usize,
            logprob: *mut f64,
        ) -> bool,
    >,
    /// Tree reranker inference: one log probability per target of `tree` out, false when the model cannot run;
    /// NULL without a tree model.
    pub tree_logprob:
        Option<unsafe extern "C" fn(context: *mut c_void, tree: *const ZtPackedTree, logprob: *mut f64) -> bool>,
    /// Called once when the reranker is freed (or not created); may be NULL.
    pub release: Option<unsafe extern "C" fn(context: *mut c_void)>,
}

struct HostReranker {
    context: HostContext,
    logprob: ZtRerankerLogprobFn,
    tree: Option<unsafe extern "C" fn(context: *mut c_void, tree: *const ZtPackedTree, logprob: *mut f64) -> bool>,
}

impl RerankerModel for HostReranker {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError> {
        let mut out = vec![f64::NAN; ids.len()];
        // SAFETY: the host's function with its context; inputs and output sized as documented.
        let ok = unsafe {
            (self.logprob)(self.context.context, ids.as_ptr(), targets.as_ptr(), rows, length, out.as_mut_ptr())
        };
        if ok { Ok(out) } else { Err(ModelError("reranker model failed".to_owned())) }
    }

    fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    fn tree_logprob(&mut self, tree: &PackedTree) -> Result<Vec<f64>, ModelError> {
        let function = self.tree.ok_or_else(|| ModelError("no tree model".to_owned()))?;
        let view = ZtPackedTree {
            ids: tree.ids.as_ptr(),
            positions: tree.positions.as_ptr(),
            node_count: tree.ids.len(),
            ancestors: tree.ancestors.as_ptr(),
            parents: tree.parents.as_ptr(),
            targets: tree.targets.as_ptr(),
            target_count: tree.targets.len(),
        };
        let mut out = vec![f64::NAN; tree.targets.len()];
        // SAFETY: the host's function with its context; the view and output are sized as documented.
        let ok = unsafe { function(self.context.context, &raw const view, out.as_mut_ptr()) };
        if ok { Ok(out) } else { Err(ModelError("reranker tree model failed".to_owned())) }
    }
}

/// A sentence reranker: the decisions of `NeuralReranker` with the host's inference.  Thread-safe.
#[derive(Debug)]
pub struct ZtReranker {
    pub(crate) reranker: Arc<NeuralReranker>,
}

fn host_reranker(model: ZtRerankerModel) -> Result<Box<dyn RerankerModel>, String> {
    let context = HostContext { context: model.context, release: model.release };
    let logprob = model.logprob.ok_or("the reranker model needs a logprob function")?;
    Ok(Box::new(HostReranker { context, logprob, tree: model.tree_logprob }))
}

/// # Safety
/// `directory` is a C string; `error` is NULL or writable.
unsafe fn make_reranker(
    directory: *const c_char,
    model: ZtRerankerModel,
    error: *mut *mut c_char,
    build: impl FnOnce(&Path, Box<dyn RerankerModel>) -> Result<NeuralReranker, ModelError>,
) -> *mut ZtReranker {
    let result = host_reranker(model).and_then(|model| {
        // SAFETY: NULL or a C string.
        let directory = unsafe { borrowed(directory) }.ok_or("directory is not a UTF-8 string")?;
        build(Path::new(directory), model).map_err(|e| e.to_string())
    });
    match result {
        Ok(reranker) => Box::into_raw(Box::new(ZtReranker { reranker: Arc::new(reranker) })),
        Err(message) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &message) };
            std::ptr::null_mut()
        }
    }
}

/// A reranker reading `vocab.json` in `directory`, with `weight`.  On failure the model is released.
///
/// # Safety
/// `directory` is a C string; `model` follows the header's rules; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_new(
    directory: *const c_char,
    weight: f64,
    model: ZtRerankerModel,
    error: *mut *mut c_char,
) -> *mut ZtReranker {
    // SAFETY: per the contract above.
    unsafe { make_reranker(directory, model, error, |d, m| NeuralReranker::new(d, weight, m)) }
}

/// The reranker as the app loads it: weight and sentence count from `directory/config.json`.
///
/// # Safety
/// `directory` is a C string; `model` follows the header's rules; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_configured(
    directory: *const c_char,
    model: ZtRerankerModel,
    error: *mut *mut c_char,
) -> *mut ZtReranker {
    // SAFETY: per the contract above.
    unsafe { make_reranker(directory, model, error, NeuralReranker::configured) }
}

/// Frees a reranker (releasing its model's context).  NULL is ignored.
///
/// # Safety
/// `reranker` is NULL or a handle from this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_free(reranker: *mut ZtReranker) {
    if !reranker.is_null() {
        // SAFETY: allocated by `make_reranker`.
        drop(unsafe { Box::from_raw(reranker) });
    }
}

/// How many of the engine's best sentences the reranker chooses among (`config.json`'s count).
///
/// # Safety
/// `reranker` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_candidate_count(reranker: *const ZtReranker) -> usize {
    // SAFETY: live handle.
    unsafe { reranker.as_ref() }.map_or(0, |r| r.reranker.candidate_count)
}

/// The weight of the model's log probability.
///
/// # Safety
/// `reranker` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_weight(reranker: *const ZtReranker) -> f64 {
    // SAFETY: live handle.
    unsafe { reranker.as_ref() }.map_or(f64::NAN, |r| r.reranker.weight)
}

/// Sets the exact margin, the gate τ and the switch margin δ.  Only before the reranker is shared (returns false
/// otherwise).
///
/// # Safety
/// `reranker` is a live handle not used by another thread during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_set_margins(
    reranker: *mut ZtReranker,
    exact: f64,
    gate: f64,
    switch: f64,
) -> bool {
    // SAFETY: live handle.
    let Some(handle) = (unsafe { reranker.as_mut() }) else { return false };
    let Some(reranker) = Arc::get_mut(&mut handle.reranker) else { return false };
    reranker.exact_margin = exact;
    reranker.gate_margin = gate;
    reranker.switch_margin = switch;
    true
}

/// The index of the sentence to display.
///
/// # Safety
/// `reranker` is a live handle; `sentences` the view passed to a running reranker hook; `context` a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_choose(
    reranker: *const ZtReranker,
    sentences: *const ZtSentences,
    context: *const c_char,
) -> usize {
    // SAFETY: per the contract above.
    let (Some(reranker), Some(sentences)) = (unsafe { (reranker.as_ref(), sentences.as_ref()) }) else {
        return 0;
    };
    // SAFETY: NULL or a C string; the view is live during the hook.
    let (context, sentences) = unsafe { (borrowed(context).unwrap_or_default(), sentences.slice()) };
    reranker.reranker.choose(sentences, context)
}

fn write(values: &[f64], out: *mut f64) -> bool {
    if !values.is_empty() && out.is_null() {
        return false;
    }
    if !values.is_empty() {
        // SAFETY: callers pass room for `values.len()` scores (documented per function).
        unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), out, values.len()) };
    }
    true
}

/// log P(candidate | context) for `count` candidates, written to `scores`; false when the model cannot score them.
///
/// # Safety
/// `reranker` is a live handle; `context` a C string; `candidates` `count` C strings; `scores` room for `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_scores(
    reranker: *const ZtReranker,
    context: *const c_char,
    candidates: *const *const c_char,
    count: usize,
    scores: *mut f64,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(reranker), Some(candidates)) = (unsafe { (reranker.as_ref(), string_array(candidates, count)) }) else {
        return false;
    };
    // SAFETY: NULL or a C string.
    let context = unsafe { borrowed(context) }.unwrap_or_default();
    reranker.reranker.scores(context, &candidates).is_some_and(|values| write(&values, scores))
}

/// `weight` × log P(candidate | context) for candidate-list order, for `count` candidates; false when the model
/// cannot score them.
///
/// # Safety
/// `reranker` is a live handle; `context` a C string; `candidates` `count` C strings; `scores` room for `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_candidate_scores(
    reranker: *const ZtReranker,
    context: *const c_char,
    candidates: *const *const c_char,
    count: usize,
    scores: *mut f64,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(reranker), Some(candidates)) = (unsafe { (reranker.as_ref(), string_array(candidates, count)) }) else {
        return false;
    };
    // SAFETY: NULL or a C string.
    let context = unsafe { borrowed(context) }.unwrap_or_default();
    reranker.reranker.candidate_scores(context, &candidates).is_some_and(|values| write(&values, scores))
}

/// [`zt_reranker_candidate_scores`] for the view passed to a candidate scorer hook.
///
/// # Safety
/// `reranker` is a live handle; `context` a C string; `candidates` the view passed to the running hook; `scores` room
/// for one score per candidate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_reranker_candidate_scores_view(
    reranker: *const ZtReranker,
    context: *const c_char,
    candidates: *const ZtStrings,
    scores: *mut f64,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(reranker), Some(candidates)) = (unsafe { (reranker.as_ref(), candidates.as_ref()) }) else {
        return false;
    };
    // SAFETY: NULL or a C string; the view is live during the hook.
    let (context, candidates) = unsafe { (borrowed(context).unwrap_or_default(), candidates.slice()) };
    reranker.reranker.candidate_scores(context, candidates).is_some_and(|values| write(&values, scores))
}

/// The character model's inputs: `count` syllables, each with `width` candidate IDs (row-major, padded with 0).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtSyllableInput {
    /// The text before the composition as character IDs, `<bos>` first.
    pub context_ids: *const i32,
    /// How many context IDs.
    pub context_count: usize,
    /// `count` toneless reading IDs.
    pub syllable_ids: *const i32,
    /// `count` tones.
    pub tones: *const i32,
    /// `count` correction buckets.
    pub costs: *const i32,
    /// `count` implicit-boundary flags (0 or 1).
    pub boundaries: *const i32,
    /// How many syllables.
    pub count: usize,
    /// `count × width` candidate character IDs.
    pub candidates: *const i32,
    /// Candidates per syllable.
    pub width: usize,
}

/// Character model inference: writes `count × width` log probabilities (row-major) to `logprob`; false when the
/// model cannot run.
type ZtSyllablePredictFn =
    unsafe extern "C" fn(context: *mut c_void, input: *const ZtSyllableInput, logprob: *mut f64) -> bool;

/// The host's character model inference.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtSyllableModel {
    /// Passed to the functions.
    pub context: *mut c_void,
    /// Required.
    pub predict:
        Option<unsafe extern "C" fn(context: *mut c_void, input: *const ZtSyllableInput, logprob: *mut f64) -> bool>,
    /// Called once when the scorer is freed (or not created); may be NULL.
    pub release: Option<unsafe extern "C" fn(context: *mut c_void)>,
}

struct HostSyllableModel {
    context: HostContext,
    predict: ZtSyllablePredictFn,
}

impl SyllableModel for HostSyllableModel {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        let count = input.syllable_ids.len();
        let width = input.candidates.iter().map(Vec::len).max().unwrap_or(0);
        let flat: Vec<i32> = input
            .candidates
            .iter()
            .flat_map(|r| r.iter().copied().chain(std::iter::repeat_n(0, width - r.len())))
            .collect();
        let view = ZtSyllableInput {
            context_ids: input.context_ids.as_ptr(),
            context_count: input.context_ids.len(),
            syllable_ids: input.syllable_ids.as_ptr(),
            tones: input.tones.as_ptr(),
            costs: input.costs.as_ptr(),
            boundaries: input.boundaries.as_ptr(),
            count,
            candidates: flat.as_ptr(),
            width,
        };
        let mut out = vec![f64::NAN; count * width];
        // SAFETY: the host's function with its context; the view and output are sized as documented.
        let ok = unsafe { (self.predict)(self.context.context, &raw const view, out.as_mut_ptr()) };
        if !ok {
            return Err(ModelError("character model failed".to_owned()));
        }
        Ok(input
            .candidates
            .iter()
            .enumerate()
            .map(|(row, list)| out[row * width..row * width + list.len()].to_vec())
            .collect())
    }
}

/// The character model: the decisions of `SyllableScorer` with the host's inference.  Thread-safe.
#[derive(Debug)]
pub struct ZtSyllableScorer {
    pub(crate) scorer: Arc<SyllableScorer>,
}

/// A character model reading `chars.json` and `syllables.json` in `directory`.  On failure the model is released.
///
/// # Safety
/// `directory` is a C string; `model` follows the header's rules; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_syllable_scorer_new(
    directory: *const c_char,
    model: ZtSyllableModel,
    error: *mut *mut c_char,
) -> *mut ZtSyllableScorer {
    let context = HostContext { context: model.context, release: model.release };
    let result = (|| -> Result<SyllableScorer, String> {
        let predict = model.predict.ok_or("the character model needs a predict function")?;
        // SAFETY: NULL or a C string.
        let directory = unsafe { borrowed(directory) }.ok_or("directory is not a UTF-8 string")?;
        SyllableScorer::new(Path::new(directory), Box::new(HostSyllableModel { context, predict }))
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(scorer) => Box::into_raw(Box::new(ZtSyllableScorer { scorer: Arc::new(scorer) })),
        Err(message) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &message) };
            std::ptr::null_mut()
        }
    }
}

/// Frees a character model (releasing its model's context).  NULL is ignored.
///
/// # Safety
/// `scorer` is NULL or a handle from this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_syllable_scorer_free(scorer: *mut ZtSyllableScorer) {
    if !scorer.is_null() {
        // SAFETY: allocated by `zt_syllable_scorer_new`.
        drop(unsafe { Box::from_raw(scorer) });
    }
}

/// # Safety
/// `syllables` points to `count` valid entries (bases NULL or C strings).
unsafe fn rust_syllables(syllables: *const ZtSyllable, count: usize) -> Option<Vec<CharacterModelSyllable>> {
    if count == 0 {
        return Some(vec![]);
    }
    if syllables.is_null() {
        return None;
    }
    // SAFETY: `count` entries per the contract.
    let views = unsafe { std::slice::from_raw_parts(syllables, count) };
    views
        .iter()
        .map(|s| {
            let base = if s.base.is_null() {
                None
            } else {
                // SAFETY: a C string per the contract.
                Some(unsafe { borrowed(s.base) }?.to_owned())
            };
            Some(CharacterModelSyllable { base, tone: s.tone, cost_bucket: s.cost_bucket, implicit: s.implicit })
        })
        .collect()
}

fn write_rows(rows: &[Vec<f64>], candidates: &[Vec<String>], scores: *mut f64, lengths: *mut usize) -> bool {
    if (!rows.is_empty() && lengths.is_null()) || (rows.iter().any(|row| !row.is_empty()) && scores.is_null()) {
        return false;
    }
    let mut start = 0;
    for (index, (row, list)) in rows.iter().zip(candidates).enumerate() {
        // SAFETY: callers pass room for every candidate's score and one length per row.
        unsafe {
            if !row.is_empty() {
                std::ptr::copy_nonoverlapping(row.as_ptr(), scores.add(start), row.len());
            }
            *lengths.add(index) = row.len();
        }
        start += list.len();
    }
    true
}

/// [`zt_syllable_scorer_scores`] for the view passed to a character scorer hook (the hook's `scores` and `lengths`
/// can be passed straight through).
///
/// # Safety
/// `scorer` is a live handle; `context` a C string; `syllables` `count` entries; `candidates` the view passed to the
/// running hook; `scores` room for every candidate; `lengths` room for `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_syllable_scorer_scores_rows(
    model: *const ZtSyllableScorer,
    context: *const c_char,
    syllables: *const ZtSyllable,
    count: usize,
    candidates: *const ZtCandidateRows,
    scores: *mut f64,
    lengths: *mut usize,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(handle), Some(syllables), Some(rows)) =
        (unsafe { (model.as_ref(), rust_syllables(syllables, count), candidates.as_ref()) })
    else {
        return false;
    };
    // SAFETY: NULL or a C string; the view is live during the hook.
    let (context, candidates) = (unsafe { borrowed(context) }.unwrap_or_default(), unsafe { rows.slice() });
    let Some(result) = handle.scorer.scores(context, &syllables, candidates) else { return false };
    write_rows(&result, candidates, scores, lengths)
}

/// For each of `count` syllables, log P over its candidate characters: row `i`'s candidates are the C strings
/// `candidates[row_starts[i] .. row_starts[i + 1]]` (`row_starts` has `count + 1` entries).  Scores are written row
/// after row with room for every candidate, how many per row (at most 64) to `lengths`.  False when the model cannot
/// score them.
///
/// # Safety
/// `scorer` is a live handle; `context` a C string; `syllables` `count` entries; `candidates` and `row_starts` as
/// described; `scores` room for every candidate; `lengths` room for `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_syllable_scorer_scores(
    model: *const ZtSyllableScorer,
    context: *const c_char,
    syllables: *const ZtSyllable,
    count: usize,
    candidates: *const *const c_char,
    row_starts: *const usize,
    scores: *mut f64,
    lengths: *mut usize,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(handle), Some(syllables)) = (unsafe { (model.as_ref(), rust_syllables(syllables, count)) }) else {
        return false;
    };
    if row_starts.is_null() {
        return false;
    }
    // SAFETY: `count + 1` entries per the contract.
    let starts = unsafe { std::slice::from_raw_parts(row_starts, count + 1) };
    if starts.windows(2).any(|w| w[0] > w[1]) {
        return false;
    }
    // SAFETY: `starts[count]` strings per the contract.
    let Some(all) = (unsafe { string_array(candidates, starts[count]) }) else { return false };
    let rows: Vec<Vec<String>> = starts.windows(2).map(|w| all[w[0]..w[1]].to_vec()).collect();
    // SAFETY: NULL or a C string.
    let context = unsafe { borrowed(context) }.unwrap_or_default();
    let Some(result) = handle.scorer.scores(context, &syllables, &rows) else { return false };
    write_rows(&result, &rows, scores, lengths)
}

/// The result of [`zt_syllable_scorer_check_vectors`].
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZtVectorCheck {
    /// Vectors within tolerance.
    pub passed: usize,
    /// Vectors checked.
    pub total: usize,
    /// The largest absolute difference.
    pub worst: f64,
}

/// Runs the scorer's model on `directory/test_vectors.json` (the v3 golden outputs; `zhuyin-cli --check-v3`): every
/// expected value above −9000 must be within 0.002 + 0.002 × |value|.
///
/// # Safety
/// `scorer` is a live handle; `directory` a C string; `result` writable; `error` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_syllable_scorer_check_vectors(
    scorer: *const ZtSyllableScorer,
    directory: *const c_char,
    result: *mut ZtVectorCheck,
    error: *mut *mut c_char,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(handle), Some(directory)) = (unsafe { (scorer.as_ref(), borrowed(directory)) }) else {
        // SAFETY: `error` is NULL or writable.
        unsafe { set_error(error, "invalid argument") };
        return false;
    };
    match zhuyin_models::check_vectors(&handle.scorer, Path::new(directory)) {
        Ok(check) => {
            if !result.is_null() {
                // SAFETY: checked non-null; writable per the contract.
                unsafe { *result = ZtVectorCheck { passed: check.passed, total: check.total, worst: check.worst } };
            }
            true
        }
        Err(e) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &e.to_string()) };
            false
        }
    }
}

#[cfg(test)]
mod output_tests {
    use super::{write, write_rows};

    #[test]
    fn missing_output_is_rejected() {
        assert!(!write(&[1.0], std::ptr::null_mut()));
        assert!(write(&[], std::ptr::null_mut()));
        let mut length = 0;
        let mut score = 0.0;
        let rows = vec![vec![1.0]];
        let candidates = vec![vec![String::from("a")]];
        assert!(!write_rows(&rows, &candidates, std::ptr::null_mut(), &raw mut length));
        assert!(!write_rows(&rows, &candidates, &raw mut score, std::ptr::null_mut()));
        assert!(write_rows(&rows, &candidates, &raw mut score, &raw mut length));
        assert_eq!((score, length), (1.0, 1));
    }
}

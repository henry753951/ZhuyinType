//! Interfaces of the neural models the composer consults (`DESIGN.md` §5): the sentence reranker (v2b) and the
//! syllable-to-character model (v3).
//!
//! As in the Swift engine's `Prediction` closures, these traits cover tensor inference only; tokenization, the
//! prefix tree, caching and every decision (the gate, `exactMargin`) are platform-independent Rust (phase 4).  macOS
//! implements the traits with Core ML through C callbacks, Linux and Windows with ONNX Runtime, and the evaluation
//! CLI with its model server.

use std::fmt;

/// A model could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

/// The reranker's candidates packed as one prefix tree (Swift's `RerankerTree`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackedTree {
    /// Token IDs of the tree's nodes.
    pub ids: Vec<i32>,
    /// Each node's depth.
    pub positions: Vec<i32>,
    /// Row-major `ids.len()²` mask: node j is an ancestor of node i (or i itself).
    pub ancestors: Vec<i32>,
    /// For each scored target, the node it follows.
    pub parents: Vec<i32>,
    /// The scored targets.
    pub targets: Vec<i32>,
}

/// v2b: per position, log P(target | the tokens before it).
pub trait RerankerModel: Send + Sync {
    /// `rows` sequences of `length` token IDs and their targets; the log probability of every target, row-major.
    ///
    /// # Errors
    /// When the model cannot run.
    fn logprob(&self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError>;

    /// The prefix-tree model: the log probability of every target of `tree`; `None` without a tree model.
    fn tree_logprob(&self, _tree: &PackedTree) -> Option<Result<Vec<f64>, ModelError>> {
        None
    }
}

/// The inputs of one v3 prediction (Swift's `SyllableScorer.Prediction`).
#[derive(Clone, Copy, Debug)]
pub struct SyllableInput<'a> {
    /// The text before the composition, as character IDs (starting with the begin token).
    pub context_ids: &'a [i32],
    /// Each syllable's toneless reading ID.
    pub syllable_ids: &'a [i32],
    /// Each syllable's tone (0 literal, 1–5).
    pub tones: &'a [i32],
    /// Each syllable's correction bucket (0 exact … 3).
    pub costs: &'a [i32],
    /// Each syllable ended without a tone key or Space (1) or not (0).
    pub boundaries: &'a [i32],
    /// Per syllable, the candidate character IDs.
    pub candidates: &'a [Vec<i32>],
}

/// v3: for each syllable, log P over its candidate characters.  Implementations may cache the context encoder's
/// output per context.
pub trait SyllableModel: Send + Sync {
    /// One row of log probabilities per syllable, one value per candidate.
    ///
    /// # Errors
    /// When the model cannot run.
    fn predict(&self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError>;
}

/// The weight of the character model in the app's lattice (Swift's `SyllableScorer.appWeight`).
pub const APP_SYLLABLE_WEIGHT: f64 = 0.5;

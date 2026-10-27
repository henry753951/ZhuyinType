//! The neural models the composer consults (`DESIGN.md` §5): the sentence reranker (v2b, [`NeuralReranker`]) and
//! the syllable-to-character model (v3, [`SyllableScorer`]).
//!
//! As in the Swift engine's `Prediction` closures, only tensor inference sits behind a trait ([`RerankerModel`],
//! [`SyllableModel`]); tokenization, the prefix tree, caching and every decision (the gate τ, `exactMargin`, the
//! switch margin) are platform-independent Rust.  macOS implements the traits with Core ML through C callbacks
//! (`zhuyin-ffi`), Linux and Windows with ONNX Runtime (the `onnx` module, feature `onnx`), and the evaluation CLI also with
//! its model server.

mod reranker;
mod syllable;
mod tree;
mod vocabulary;

#[cfg(feature = "onnx")]
pub mod onnx;

use std::fmt;

pub use reranker::NeuralReranker;
pub use syllable::{SyllableScorer, VectorCheck, check_vectors};
pub use tree::RerankerTree;

/// A model could not run, or its files could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

impl From<std::io::Error> for ModelError {
    fn from(error: std::io::Error) -> Self {
        ModelError(error.to_string())
    }
}

/// The reranker's candidates packed as one prefix tree (the tensors of Swift's `RerankerTree`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackedTree {
    /// Token IDs of the nodes whose hidden state some target needs.
    pub ids: Vec<i32>,
    /// Each of those nodes' depth.
    pub positions: Vec<i32>,
    /// Row-major `ids.len()²` mask: node j is an ancestor of node i (or i itself).
    pub ancestors: Vec<i32>,
    /// For each scored target, the index (into `ids`) of the node it follows.
    pub parents: Vec<i32>,
    /// The scored targets: every node's token.
    pub targets: Vec<i32>,
}

/// v2b: per position, log P(target | the tokens before it).  Calls are serialized by [`NeuralReranker`].
pub trait RerankerModel: Send {
    /// `rows` sequences of `length` token IDs and their targets; the log probability of every target, row-major.
    ///
    /// # Errors
    /// When the model cannot run.
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError>;

    /// Whether [`tree_logprob`](Self::tree_logprob) is available (Swift's optional `treePrediction`).
    fn has_tree(&self) -> bool {
        false
    }

    /// The prefix-tree model: the log probability of every target of `tree`.
    ///
    /// # Errors
    /// When the model cannot run or there is no tree model.
    fn tree_logprob(&mut self, _tree: &PackedTree) -> Result<Vec<f64>, ModelError> {
        Err(ModelError("no tree model".to_owned()))
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
    /// Per syllable, the candidate character IDs, every row padded with 0 to the same width.
    pub candidates: &'a [Vec<i32>],
}

/// v3: for each syllable, log P over its candidate characters.  Implementations may cache the context encoder's
/// output per context.  Calls are serialized by [`SyllableScorer`].
pub trait SyllableModel: Send {
    /// One row of log probabilities per syllable, one value per candidate column.
    ///
    /// # Errors
    /// When the model cannot run.
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError>;
}

/// The weight of the character model in the app's lattice (Swift's `SyllableScorer.appWeight`).
pub const APP_SYLLABLE_WEIGHT: f64 = 0.5;

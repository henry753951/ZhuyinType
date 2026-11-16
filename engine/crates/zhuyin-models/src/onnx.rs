//! The ONNX Runtime backend (Linux, Windows; feature `onnx`), set up as Swift's `ONNXBridge`: one CPU session per
//! model with one intra-op and one inter-op thread (the evaluator supplies process parallelism), every graph
//! optimization, telemetry off; `int32` inputs and `float32` outputs widened to `f64`.  The Linux and Windows apps'
//! models are ORT-format files, memory-mapped (see [`session`]).  Tokenization, penalties and
//! every decision stay in [`NeuralReranker`](crate::NeuralReranker) and [`SyllableScorer`](crate::SyllableScorer).

use std::path::Path;
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::{DynValue, Tensor};

use crate::{ModelError, PackedTree, RerankerModel, SyllableInput, SyllableModel};

fn error(error: impl std::fmt::Display) -> ModelError {
    ModelError(error.to_string())
}

/// The process's ONNX Runtime environment (Swift creates one per session; the name and telemetry match).
fn environment() -> Result<&'static Environment, ModelError> {
    static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| ort::init().with_name("ZhuyinType").with_telemetry(false).build().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| ModelError(e.clone()))
}

/// `directory/name.ort` (memory-mapped) or `directory/name.onnx` as a session.
///
/// - An ORT-format file (the Linux and Windows apps' models) is mapped and the session runs from it: its weights are
///   used in place (`session.use_ort_model_bytes_for_initializers`) and not repacked, so they stay file-backed pages
///   the system can drop, as Core ML's are on macOS.
/// - An ONNX file is read into memory.  A weight stored as int8 with a `DequantizeLinear` is folded into an fp32
///   constant once at load (`session.disable_quant_qdq`).
fn session(directory: &Path, name: &str) -> Result<Session, ModelError> {
    let builder = Session::builder(environment()?).map_err(error)?;
    let builder = builder.with_intra_threads(1).map_err(error)?;
    let builder = builder.with_inter_threads(1).map_err(error)?;
    let builder = builder.with_optimization_level(GraphOptimizationLevel::All).map_err(error)?;
    let mapped = directory.join(format!("{name}.ort"));
    if mapped.is_file() {
        let file = std::fs::File::open(&mapped)?;
        // SAFETY: a read-only model file the app installed; the mapping lives as long as the process, and nothing
        // writes the file while it runs.
        #[allow(unsafe_code)]
        let map = unsafe { memmap2::Mmap::map(&file) }?;
        let bytes: &'static [u8] = Box::leak(Box::new(map));
        let builder = builder.with_config_entry("session.use_ort_model_bytes_directly", "1").map_err(error)?;
        let builder = builder.with_config_entry("session.use_ort_model_bytes_for_initializers", "1").map_err(error)?;
        let mut builder = builder.with_config_entry("session.disable_prepacking", "1").map_err(error)?;
        return builder.commit_from_memory(bytes).map_err(error);
    }
    let mut builder = builder.with_config_entry("session.disable_quant_qdq", "1").map_err(error)?;
    builder.commit_from_file(directory.join(format!("{name}.onnx"))).map_err(error)
}

fn tensor(values: &[i32], shape: &[usize]) -> Result<Tensor<i32>, ModelError> {
    if shape.contains(&0) || shape.iter().product::<usize>() != values.len() {
        return Err(ModelError("Invalid tensor shape".to_owned()));
    }
    let shape: Vec<i64> = shape.iter().map(|&d| i64::try_from(d).unwrap_or(i64::MAX)).collect();
    Tensor::from_array((shape, values.to_vec())).map_err(error)
}

fn doubles(value: &DynValue) -> Result<Vec<f64>, ModelError> {
    let (_, data) =
        value.try_extract_tensor::<f32>().map_err(|_| ModelError("Expected fp32 ONNX logprob output".to_owned()))?;
    Ok(data.iter().map(|&v| f64::from(v)).collect())
}

/// `Reranker.onnx` and `RerankerTree.onnx`: either may be missing, not both.
pub struct OnnxReranker {
    batch: Option<Session>,
    tree: Option<Session>,
}

impl std::fmt::Debug for OnnxReranker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxReranker").field("tree", &self.tree.is_some()).finish_non_exhaustive()
    }
}

impl OnnxReranker {
    /// The reranker models in `directory`.  With the tree model there, the batched one is optional: every call
    /// the engine makes fits the tree, and the apps ship the tree only (as the macOS app does with Core ML).
    /// Without it, a tree model that fails to load is left out, as in Swift.
    ///
    /// # Errors
    /// When no model can be loaded: `Reranker.onnx` when there is no tree model, `RerankerTree.onnx` otherwise.
    pub fn new(directory: &Path) -> Result<Self, ModelError> {
        let present = |name: &str| ["onnx", "ort"].iter().any(|ext| directory.join(format!("{name}.{ext}")).exists());
        if !present("Reranker") {
            return Ok(OnnxReranker { batch: None, tree: Some(session(directory, "RerankerTree")?) });
        }
        let batch = Some(session(directory, "Reranker")?);
        let tree = if present("RerankerTree") { session(directory, "RerankerTree").ok() } else { None };
        Ok(OnnxReranker { batch, tree })
    }
}

impl RerankerModel for OnnxReranker {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError> {
        let inputs =
            ort::inputs!["ids" => tensor(ids, &[rows, length])?, "targets" => tensor(targets, &[rows, length])?];
        let batch = self.batch.as_mut().ok_or_else(|| ModelError("no batched reranker model".to_owned()))?;
        let outputs = batch.run(inputs).map_err(error)?;
        doubles(outputs.get("logprob").ok_or_else(|| ModelError("no logprob output".to_owned()))?)
    }

    fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    fn tree_logprob(&mut self, tree: &PackedTree) -> Result<Vec<f64>, ModelError> {
        let session = self.tree.as_mut().ok_or_else(|| ModelError("no tree model".to_owned()))?;
        let n = tree.ids.len();
        let t = tree.targets.len();
        let inputs = ort::inputs![
            "ids" => tensor(&tree.ids, &[1, n])?,
            "positions" => tensor(&tree.positions, &[1, n])?,
            "ancestors" => tensor(&tree.ancestors, &[n, n])?,
            "parents" => tensor(&tree.parents, &[t])?,
            "targets" => tensor(&tree.targets, &[t])?,
        ];
        let outputs = session.run(inputs).map_err(error)?;
        doubles(outputs.get("logprob").ok_or_else(|| ModelError("no logprob output".to_owned()))?)
    }
}

/// `ContextEncoder.onnx` and `SyllableEncoder.onnx`; the context encoder's memory is kept for the last context.
pub struct OnnxSyllableModel {
    context: Session,
    syllable: Session,
    cached_ids: Vec<i32>,
    memory: Option<(DynValue, DynValue)>,
}

impl std::fmt::Debug for OnnxSyllableModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSyllableModel").field("cached_ids", &self.cached_ids.len()).finish_non_exhaustive()
    }
}

impl OnnxSyllableModel {
    /// The v3 models in `directory`.
    ///
    /// # Errors
    /// When either model cannot be loaded.
    pub fn new(directory: &Path) -> Result<Self, ModelError> {
        Ok(OnnxSyllableModel {
            context: session(directory, "ContextEncoder")?,
            syllable: session(directory, "SyllableEncoder")?,
            cached_ids: vec![],
            memory: None,
        })
    }
}

impl SyllableModel for OnnxSyllableModel {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        let count = input.syllable_ids.len();
        let width = input.candidates.iter().map(Vec::len).max().unwrap_or(0);
        let shapes_fit = [input.tones.len(), input.costs.len(), input.boundaries.len(), input.candidates.len()]
            .iter()
            .all(|&n| n == count);
        if count == 0 || count > 64 || width == 0 || width > 64 || input.context_ids.len() > 256 || !shapes_fit {
            return Err(ModelError("Invalid syllable shape".to_owned()));
        }
        if self.memory.is_none() || self.cached_ids != input.context_ids {
            let ids = input.context_ids;
            let mut outputs =
                self.context.run(ort::inputs!["context_ids" => tensor(ids, &[1, ids.len()])?]).map_err(error)?;
            let missing = || ModelError("context encoder output".to_owned());
            let k = outputs.remove("memory_k").ok_or_else(missing)?;
            let v = outputs.remove("memory_v").ok_or_else(missing)?;
            drop(outputs);
            self.memory = Some((k, v));
            ids.clone_into(&mut self.cached_ids);
        }
        let (k, v) = self.memory.as_ref().ok_or_else(|| ModelError("context encoder output".to_owned()))?;
        let flat: Vec<i32> = input
            .candidates
            .iter()
            .flat_map(|row| row.iter().copied().chain(std::iter::repeat_n(0, width - row.len())))
            .collect();
        let inputs = ort::inputs![
            "syllable_ids" => tensor(input.syllable_ids, &[1, count])?,
            "tone_ids" => tensor(input.tones, &[1, count])?,
            "cost_bucket" => tensor(input.costs, &[1, count])?,
            "boundary" => tensor(input.boundaries, &[1, count])?,
            "candidate_ids" => tensor(&flat, &[1, count, width])?,
            "memory_k" => k,
            "memory_v" => v,
        ];
        let outputs = self.syllable.run(inputs).map_err(error)?;
        let output = doubles(outputs.get("logprob").ok_or_else(|| ModelError("no logprob output".to_owned()))?)?;
        if output.len() != count * width {
            return Err(ModelError("Invalid syllable output".to_owned()));
        }
        Ok(input
            .candidates
            .iter()
            .enumerate()
            .map(|(row, list)| output[row * width..row * width + list.len()].to_vec())
            .collect())
    }
}

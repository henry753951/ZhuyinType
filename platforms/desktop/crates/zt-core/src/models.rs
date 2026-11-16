//! The neural models (macOS `EngineHost.loadReranker`, `loadCharacterModel` and the hooks of `ControllerCore`): the
//! sentence reranker (v2b) and the syllable-to-character model (v3), run with ONNX Runtime.
//!
//! - The models are ONNX Runtime's dynamic int8 of the macOS app's weights, in the ORT format, which the session
//!   memory-maps (`data/reranker/RerankerTree.ort`, `data/v3/ContextEncoder.ort`, `SyllableEncoder.ort`); the
//!   decisions are `zhuyin-models`, shared with macOS.
//! - ONNX Runtime is loaded at run time from beside the app (`onnxruntime.dll`; on Linux
//!   `/usr/lib/zhuyintype/libonnxruntime.so.1`), or `ORT_DYLIB_PATH`.  Without it, or without the model files, the
//!   engine runs without the models, as a development build without them does on macOS.
//! - The models read the text before the composition: what the controller gave as 前文 when the composition started,
//!   plus what a long composition committed by itself since (`Context`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use zhuyin_core::composer::Composer;
use zhuyin_models::onnx::{OnnxReranker, OnnxSyllableModel};
use zhuyin_models::{APP_SYLLABLE_WEIGHT, NeuralReranker, SyllableScorer};

/// The loaded models, shared by every engine session (the input method's and the practice fields').
#[derive(Default)]
pub struct NeuralModels {
    pub reranker: Option<Arc<NeuralReranker>>,
    pub character: Option<Arc<SyllableScorer>>,
    /// How long loading took, for 分析 › 效能.
    pub reranker_ms: Option<f64>,
    pub character_ms: Option<f64>,
    /// Why a model is missing, if it was expected.
    pub error: Option<String>,
    /// How often the composer has asked each model (character model, reranker's choice, candidate list).
    pub calls: Arc<[std::sync::atomic::AtomicUsize; 3]>,
}

impl std::fmt::Debug for NeuralModels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralModels")
            .field("reranker", &self.reranker.is_some())
            .field("character", &self.character.is_some())
            .field("error", &self.error)
            .finish()
    }
}

/// Where ONNX Runtime may be, in order.
fn runtime_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH").filter(|path| !path.is_empty()) {
        paths.push(PathBuf::from(path));
    }
    let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    if cfg!(windows) {
        paths.extend(exe_dir.map(|dir| dir.join("onnxruntime.dll")));
    } else {
        paths.extend(exe_dir.map(|dir| dir.join("../lib/zhuyintype/libonnxruntime.so.1")));
        paths.push(PathBuf::from("/usr/lib/zhuyintype/libonnxruntime.so.1"));
    }
    paths
}

/// Loads ONNX Runtime once for the process.
fn runtime() -> Result<(), String> {
    static LOADED: OnceLock<Result<(), String>> = OnceLock::new();
    LOADED
        .get_or_init(|| {
            let candidates = runtime_candidates();
            let Some(path) = candidates.iter().find(|path| path.is_file()) else {
                return Err(format!(
                    "找不到 ONNX Runtime（{}）",
                    candidates.first().map_or_else(String::new, |p| p.display().to_string())
                ));
            };
            ort::init_from(path).map(|_| ()).map_err(|error| format!("無法載入 ONNX Runtime：{error}"))
        })
        .clone()
}

/// The models in `reranker` and `character` (directories), whichever are given.
#[must_use]
pub fn load(reranker: Option<&Path>, character: Option<&Path>) -> NeuralModels {
    let mut models = NeuralModels::default();
    if reranker.is_none() && character.is_none() {
        return models;
    }
    if let Err(error) = runtime() {
        log::warn!("neural models: {error}");
        models.error = Some(error);
        return models;
    }
    if let Some(directory) = reranker {
        let started = Instant::now();
        match OnnxReranker::new(directory).and_then(|model| NeuralReranker::configured(directory, Box::new(model))) {
            Ok(model) => {
                models.reranker = Some(Arc::new(model));
                models.reranker_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
            }
            Err(error) => {
                log::warn!("reranker: {error}");
                models.error = Some(format!("整句重排模型：{error}"));
            }
        }
    }
    if let Some(directory) = character {
        let started = Instant::now();
        match OnnxSyllableModel::new(directory).and_then(|model| SyllableScorer::new(directory, Box::new(model))) {
            Ok(model) => {
                models.character = Some(Arc::new(model));
                models.character_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
            }
            Err(error) => {
                log::warn!("character model: {error}");
                models.error = Some(format!("逐字模型：{error}"));
            }
        }
    }
    models
}

/// The text before the composition as the models read it (macOS `ControllerCore.modelContext`).
#[derive(Clone, Default)]
pub struct Context(Arc<Mutex<String>>);

impl Context {
    pub fn set(&self, text: &str) {
        text.clone_into(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner));
    }

    fn append(&self, text: &str) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push_str(text);
    }

    fn get(&self) -> String {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

/// Whether the sentence the reranker last chose has a typing correction (for 分析's 自動修正).
#[derive(Clone, Default)]
pub struct ShownCorrected(Arc<AtomicBool>);

impl ShownCorrected {
    #[must_use]
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Gives `composer` the models, as the macOS controller does: the character model at the app's weight, the
/// reranker choosing among its configured number of sentences and ordering the candidate list.  `suspended` (secure
/// input, private fields) keeps commits out of the context.
pub fn attach(
    composer: &mut Composer,
    models: &NeuralModels,
    context: &Context,
    shown: &ShownCorrected,
    suspended: &Arc<AtomicBool>,
) {
    let (committed, suspended) = (context.clone(), suspended.clone());
    composer.set_commit_listener(Some(Box::new(move |text| {
        if !suspended.load(Ordering::Relaxed) {
            committed.append(text);
        }
    })));
    if let Some(model) = &models.character {
        let mut tuning = composer.tuning();
        tuning.character_weight = APP_SYLLABLE_WEIGHT;
        composer.set_tuning(tuning);
        let (model, context, calls) = (model.clone(), context.clone(), models.calls.clone());
        composer.set_character_scorer(Some(Box::new(move |syllables, candidates| {
            calls[0].fetch_add(1, Ordering::Relaxed);
            model.scores(&context.get(), syllables, candidates)
        })));
    }
    if let Some(reranker) = &models.reranker {
        composer.set_reranking_count(reranker.candidate_count);
        let (chooser, choose_context, shown) = (reranker.clone(), context.clone(), shown.clone());
        let calls = models.calls.clone();
        composer.set_reranker(Some(Box::new(move |sentences| {
            calls[1].fetch_add(1, Ordering::Relaxed);
            let index = chooser.choose(sentences, &choose_context.get());
            let chosen = sentences.get(index).or_else(|| sentences.first());
            shown.0.store(chosen.is_some_and(zhuyin_core::api::ScoredSentence::corrected), Ordering::Relaxed);
            Some(index)
        })));
        let (scorer, score_context, calls) = (reranker.clone(), context.clone(), models.calls.clone());
        composer.set_candidate_scorer(Some(Box::new(move |before, candidates| {
            calls[2].fetch_add(1, Ordering::Relaxed);
            scorer.candidate_scores(&(score_context.get() + before), candidates)
        })));
    }
}

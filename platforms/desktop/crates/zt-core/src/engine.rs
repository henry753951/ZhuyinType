//! The engine session (port of the Qt `EngineSession`): the engine data, one composer, the settings that reach it, and
//! the personal data (user phrases, learned words, exclusions, backups).  It uses `zhuyin-core` directly.
//!
//! The engine data ([`EngineData`]: where the lexicon and models are, and how to open them) is shared through `Arc`
//! between the input method's session and practice sessions.  Each practice session opens an engine of its own with
//! in-memory personal data; the lexicon and language models are memory-mapped, so the second engine shares their
//! pages with the first.
//!
//! The neural models (reranker, character model) run with ONNX Runtime (`crate::models`); [`EngineData`] loads them
//! once and every session's composer gets them through its hooks, as the macOS app sets them up with Core ML.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow, ensure};
use serde_json::{Map, Value};
use zhuyin_core::api::{
    CandidateAnchor, CharacterForms, ComposerOptions, FuzzySounds, KeyInput, PunctuationKeys, PunctuationWidth,
    SpaceAction, TypoTolerance,
};
use zhuyin_core::composer::{CandidateAction, Composer};
use zhuyin_core::engine::{Engine, EngineConfig, PhraseKey};

use crate::keys::Key;
use crate::presentation::Presentation;
use crate::settings::{self, Settings};

/// The data files an engine loads, found in one directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFiles {
    /// The directory.
    pub directory: PathBuf,
    /// `lexicon.tsv` (the engine reads the compiled `lexicon.bin` beside it when that exists).
    pub lexicon: PathBuf,
    /// The lexicon is compiled (`lexicon.bin`).
    pub compiled: bool,
    /// The encyclopedic language model (`lm/`: `lm.bin` or the TSV files), if present.
    pub language_model: Option<PathBuf>,
    /// The chat-domain language model (`lm_chat/`), if present.
    pub chat_model: Option<PathBuf>,
    /// The reading channel's table (`channel.bin`), if present.
    pub reading_channel: Option<PathBuf>,
    /// The sentence reranker (`reranker/`: `vocab.json` and `RerankerTree.ort`), if present.
    pub reranker: Option<PathBuf>,
    /// The character model (`v3/`: `chars.json`, `ContextEncoder.ort`, `SyllableEncoder.ort`), if present.
    pub character_model: Option<PathBuf>,
}

impl DataFiles {
    /// The data in `directory`: `None` without a lexicon (`lexicon.bin` or `lexicon.tsv`).
    #[must_use]
    pub fn locate(directory: &Path) -> Option<Self> {
        let compiled = directory.join("lexicon.bin").is_file();
        if !compiled && !directory.join("lexicon.tsv").is_file() {
            return None;
        }
        let model = |name: &str, files: &[&str]| {
            let path = directory.join(name);
            files.iter().any(|file| path.join(file).is_file()).then_some(path)
        };
        let channel = directory.join("channel.bin");
        Some(Self {
            directory: directory.to_path_buf(),
            lexicon: directory.join("lexicon.tsv"),
            compiled,
            language_model: model("lm", &["lm.bin", "bigram.tsv", "unigram.tsv"]),
            chat_model: model("lm_chat", &["lm.bin", "bigram.tsv"]),
            reading_channel: channel.is_file().then_some(channel),
            reranker: model("reranker", &["RerankerTree.ort", "RerankerTree.onnx", "Reranker.onnx"])
                .filter(|path| path.join("vocab.json").is_file()),
            character_model: model("v3", &["ContextEncoder.ort", "ContextEncoder.onnx"]).filter(|path| {
                path.join("chars.json").is_file()
                    && ["SyllableEncoder.ort", "SyllableEncoder.onnx"].iter().any(|file| path.join(file).is_file())
            }),
        })
    }

    /// Where the app looks for its data, in order: `ZT_DATA_DIR`, then `configured` (the `dataDirectory` setting or
    /// a command-line option), then `<exe dir>/data` (Windows) or `/usr/share/zhuyintype/data` (Linux), then the
    /// development checkout's `data/build`.
    #[must_use]
    pub fn search_path(configured: Option<&Path>) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(value) = std::env::var_os("ZT_DATA_DIR").filter(|value| !value.is_empty()) {
            paths.push(PathBuf::from(value));
        }
        paths.extend(configured.map(Path::to_path_buf));
        if cfg!(windows) {
            if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
                paths.push(dir.join("data"));
            }
        } else {
            paths.push(PathBuf::from("/usr/share/zhuyintype/data"));
        }
        // platforms/desktop/crates/zt-core → the repository root.
        paths.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../data/build"));
        paths
    }

    /// The first directory of [`DataFiles::search_path`] with a lexicon.
    #[must_use]
    pub fn find(configured: Option<&Path>) -> Option<Self> {
        Self::search_path(configured).iter().find_map(|path| Self::locate(path))
    }
}

/// What every engine of this process opens: the data files and the engine configuration.  Shared (`Arc`) by the
/// input method's session and the practice sessions.
#[derive(Debug)]
pub struct EngineData {
    files: DataFiles,
    config: EngineConfig,
    models: std::sync::OnceLock<crate::models::NeuralModels>,
}

impl EngineData {
    /// The engine data for `files`, with the chat model and reading channel when present.
    #[must_use]
    pub fn new(files: DataFiles) -> Arc<Self> {
        let config = EngineConfig {
            chat_model_directory: files.chat_model.clone(),
            reading_channel: files.reading_channel.clone(),
            ..EngineConfig::default()
        };
        Arc::new(Self { files, config, models: std::sync::OnceLock::new() })
    }

    /// The neural models, loaded on first use (by the first session that opens, or
    /// [`load_models_in_background`](Self::load_models_in_background)); a caller waits while they load.
    pub fn models(&self) -> &crate::models::NeuralModels {
        self.models
            .get_or_init(|| crate::models::load(self.files.reranker.as_deref(), self.files.character_model.as_deref()))
    }

    /// The neural models if they have been loaded, without loading them (檢查輸入法 reads this).
    #[must_use]
    pub fn loaded_models(&self) -> Option<&crate::models::NeuralModels> {
        self.models.get()
    }

    /// Starts loading the neural models on a thread of their own, so they load while the first session opens the
    /// lexicon and language models instead of after them; the session's [`models`](Self::models) waits for them.
    pub fn load_models_in_background(self: &Arc<Self>) {
        if self.files.reranker.is_none() && self.files.character_model.is_none() {
            return;
        }
        let data = Arc::clone(self);
        let spawned = std::thread::Builder::new().name("zt-models".into()).spawn(move || {
            data.models();
        });
        if let Err(error) = spawned {
            log::warn!("neural models: no loading thread ({error}); they load with the engine");
        }
    }

    /// The engine data for the lexicon at `path` (a `lexicon.tsv`, which need not exist when `lexicon.bin` does),
    /// with whatever models sit beside it.
    ///
    /// # Errors
    /// When there is no lexicon there.
    pub fn from_lexicon(path: &Path) -> Result<Arc<Self>> {
        let directory = path.parent().unwrap_or(Path::new("."));
        let mut files = DataFiles::locate(directory).ok_or_else(|| anyhow!("找不到詞庫：{}", path.display()))?;
        files.lexicon = path.to_path_buf();
        Ok(Self::new(files))
    }

    /// The data files.
    #[must_use]
    pub fn files(&self) -> &DataFiles {
        &self.files
    }

    /// Opens an engine on this data with personal data in `user` (`None`: in memory only, never written).
    ///
    /// # Errors
    /// When the lexicon or a model cannot be read.
    pub fn open_engine(&self, user: Option<&Path>) -> Result<Engine> {
        Engine::open(&self.files.lexicon, user, self.config.clone()).map_err(|error| anyhow!("無法開啟詞庫：{error}"))
    }
}

/// One personal-data entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    pub word: String,
    /// Syllables separated by spaces.
    pub reading: String,
    /// For learned words, how often (decaying); 0 otherwise.
    pub count: f64,
}

impl Phrase {
    fn from_key(key: PhraseKey, count: f64) -> Self {
        Self { word: key.word, reading: key.reading, count }
    }
}

/// What a key, a pick or a commit did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyOutcome {
    /// False: the key goes on to the application (after `commit`, if any).
    pub handled: bool,
    /// Text for the application, inserted before the key (if it passes through).
    pub commit: String,
    /// How long the engine took, when it did anything.
    pub elapsed_ms: Option<f64>,
}

/// The feature switch (`feature.*` setting) that turns a key off, if any.
fn feature_for(input: KeyInput) -> Option<&'static str> {
    Some(match input {
        KeyInput::OptionBackspace => "feature.deleteWord",
        KeyInput::ShiftUp => "feature.correctionsList",
        KeyInput::ShiftDown => "feature.singleCharacterList",
        KeyInput::ShiftLeft | KeyInput::ShiftRight => "feature.selectPhrase",
        // Page Up and Page Down page the list anyway (DESIGN.md「可以關閉的功能」, as macOS does).
        KeyInput::Tab | KeyInput::ShiftTab => "feature.pageKeys",
        KeyInput::ControlChar(_) => "feature.controlPunctuation",
        KeyInput::Printed(_) => "feature.optionSymbols",
        KeyInput::Char('`') => "feature.typeZhuyin",
        _ => return None,
    })
}

/// The settings the engine reads, merged with the defaults.
#[derive(Clone, Debug)]
struct Values(Map<String, Value>);

impl Values {
    fn defaults() -> Self {
        Self(settings::descriptors().iter().map(|d| (d.key.clone(), d.default.clone())).collect())
    }

    fn string(&self, key: &str) -> &str {
        self.0.get(key).and_then(Value::as_str).unwrap_or_default()
    }

    fn bool(&self, key: &str, default: bool) -> bool {
        self.0.get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    fn number(&self, key: &str) -> f64 {
        self.0.get(key).and_then(Value::as_f64).unwrap_or_default()
    }

    /// The composer's options.
    fn options(&self) -> ComposerOptions {
        let per_page = self.number("candidatesPerPage").max(1.0) as usize;
        ComposerOptions {
            selection_keys: self.string("selectionKeys").chars().take(per_page).collect(),
            space_action: match self.string("spaceAction") {
                "candidates" => SpaceAction::OpenCandidates,
                "insert" => SpaceAction::InsertSpace,
                _ => SpaceAction::CommitAndSpace,
            },
            escape_clears_composition: self.string("escape") == "composition",
            learning_enabled: self.bool("learningEnabled", true),
            typo_tolerance: TypoTolerance::from_raw(self.string("typoTolerance")).unwrap_or(TypoTolerance::Standard),
            candidate_anchor: if self.string("candidateAnchor") == "before" {
                CandidateAnchor::BeforeCursor
            } else {
                CandidateAnchor::AfterCursor
            },
            vertical_list: self.string("candidateLayout") != "horizontal",
            page_at_list_edge: self.bool("pageAtListEdge", true),
            character_forms: CharacterForms::from_raw(self.string("characterForms")).unwrap_or_default(),
            punctuation_keys: PunctuationKeys::from_raw(self.string("punctuationKeys")).unwrap_or_default(),
            undo_correction_with_backspace: self.bool("undoCorrectionWithBackspace", true),
            ..ComposerOptions::default()
        }
    }

    fn fuzzy(&self) -> FuzzySounds {
        let mut sounds = FuzzySounds::NONE;
        for (key, pair) in [
            ("fuzzy.zZh", FuzzySounds::Z_ZH),
            ("fuzzy.cCh", FuzzySounds::C_CH),
            ("fuzzy.sSh", FuzzySounds::S_SH),
            ("fuzzy.enEng", FuzzySounds::EN_ENG),
            ("fuzzy.anAng", FuzzySounds::AN_ANG),
        ] {
            if self.bool(key, false) {
                sounds.insert(pair);
            }
        }
        sounds
    }
}

/// `value` is valid for `key` (the settings registry's rules; `feature.*` switches are booleans).  Keys the engine
/// does not read are accepted as they are.
fn validate(key: &str, value: &Value) -> Result<()> {
    let valid = match settings::descriptor(key) {
        Some(descriptor) => descriptor.accepts(value),
        None => !key.starts_with("feature.") || value.is_boolean(),
    };
    ensure!(valid, "設定值無效：{key}");
    Ok(())
}

fn valid_text(text: &str) -> bool {
    !text.contains('\0')
}

fn validate_phrase(word: &str, reading: &str) -> Result<()> {
    ensure!(
        valid_text(word) && valid_text(reading) && !word.trim().is_empty() && !reading.trim().is_empty(),
        "請提供有效的字詞與注音。"
    );
    Ok(())
}

struct Open {
    data: Arc<EngineData>,
    engine: Arc<Engine>,
    composer: Composer,
}

/// One composer on an engine, with the settings applied and the presentation kept up to date.  Used by one thread
/// at a time (the input controller's owner locks it).
pub struct EngineSession {
    open: Option<Open>,
    /// What the models read as the text before the composition, whether the shown sentence was a correction, and
    /// whether commits stay out of that text (`crate::models`).
    model_context: crate::models::Context,
    shown_corrected: crate::models::ShownCorrected,
    suspended: Arc<std::sync::atomic::AtomicBool>,
    values: Values,
    presentation: Presentation,
    committed: String,
    learning_suspended: bool,
    personal_revision: u64,
    load_ms: Option<f64>,
}

impl Default for EngineSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        if let Some(open) = &self.open
            && open.engine.flush_personal_data().is_err()
        {
            log::warn!("EngineSession: personal data could not be flushed before closing");
        }
    }
}

const NOT_OPEN: &str = "引擎尚未開啟。";

impl EngineSession {
    /// A session with no engine yet (every call but `apply_settings` fails until `open`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: None,
            model_context: crate::models::Context::default(),
            shown_corrected: crate::models::ShownCorrected::default(),
            suspended: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            values: Values::defaults(),
            presentation: Presentation::default(),
            committed: String::new(),
            learning_suspended: false,
            personal_revision: 0,
            load_ms: None,
        }
    }

    /// Opens an engine on `data` with personal data in `user` (`None`: in memory).  On failure the previous engine,
    /// if any, stays open with its composition.
    ///
    /// # Errors
    /// When the user directory cannot be made, the previous engine's personal data cannot be written, or the data
    /// cannot be read.
    pub fn open(&mut self, data: &Arc<EngineData>, user: Option<&Path>) -> Result<()> {
        if let Some(user) = user {
            std::fs::create_dir_all(user).context("無法建立個人資料目錄。")?;
        }
        if self.open.is_some() {
            self.flush()?;
        }
        let started = Instant::now();
        let engine = Arc::new(data.open_engine(user)?);
        let load_ms = started.elapsed().as_secs_f64() * 1000.0;
        self.attach(data, engine);
        self.load_ms = Some(load_ms);
        Ok(())
    }

    /// Uses an engine opened elsewhere on `data` (practice sessions share one).
    pub fn open_shared(&mut self, data: &Arc<EngineData>, engine: Arc<Engine>) {
        self.attach(data, engine);
    }

    fn attach(&mut self, data: &Arc<EngineData>, engine: Arc<Engine>) {
        let mut composer = Composer::new(engine.clone(), self.values.options());
        // As the macOS app: punctuation is full width until the user switches it.
        composer.set_punctuation_width(PunctuationWidth::Full);
        Self::configure(&mut composer, &engine, &self.values);
        composer.set_learning_suspended(self.learning_suspended);
        crate::models::attach(
            &mut composer,
            data.models(),
            &self.model_context,
            &self.shown_corrected,
            &self.suspended,
        );
        self.open = Some(Open { data: data.clone(), engine, composer });
        self.committed.clear();
        self.personal_revision += 1;
        self.refresh();
    }

    fn configure(composer: &mut Composer, engine: &Engine, values: &Values) {
        composer.set_options(values.options());
        composer.set_associated_phrases(values.bool("associatedPhrases", false));
        engine.set_fuzzy_sounds(values.fuzzy());
        engine.set_reading_channel_enabled(values.bool("readingChannel", true));
        if !values.bool("usesPrecedingText", true) {
            composer.set_preceding_text("");
        }
    }

    fn refresh(&mut self) {
        let page_size = self.values.number("candidatesPerPage").max(1.0) as usize;
        self.presentation = match &self.open {
            Some(open) => {
                let mut p = Presentation::from_engine(
                    open.composer.presentation(),
                    &open.composer.options().selection_keys,
                    page_size,
                    open.engine.reading_channel().is_some(),
                )
                .with_correction_marks(
                    self.values.bool("showCorrections", true),
                    self.values.bool("undoCorrectionWithBackspace", true),
                );
                p.row_actions = p
                    .rows
                    .iter()
                    .map(|row| {
                        CandidateAction::ALL
                            .into_iter()
                            .filter(|action| open.composer.candidate_action_available(*action, Some(row.index)))
                            .fold(0, |bits, action| bits | 1 << action.raw_value())
                    })
                    .collect();
                p
            }
            None => Presentation::default(),
        };
    }

    // MARK: Word actions

    /// Whether `action` applies to candidate `index` of the open list (`None`: the highlighted one); never with no
    /// list open.
    #[must_use]
    pub fn word_action_available(&self, action: CandidateAction, index: Option<usize>) -> bool {
        self.open.as_ref().is_some_and(|open| open.composer.candidate_action_available(action, index))
    }

    /// Does `action` (see [`EngineSession::word_action_available`]); the notice it shows, or `None` when it does not
    /// apply.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn perform_word_action(&mut self, action: CandidateAction, index: Option<usize>) -> Result<Option<String>> {
        let notice = self.require()?.composer.perform_candidate_action(action, index);
        if notice.is_some() {
            self.personal_revision += 1;
        }
        self.committed.clear();
        self.refresh();
        Ok(notice)
    }

    /// The last word action can be undone.
    #[must_use]
    pub fn can_undo_word_action(&self) -> bool {
        self.open.as_ref().is_some_and(|open| open.composer.can_undo_candidate_action())
    }

    /// Undoes the last word action; false when there is none.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn undo_word_action(&mut self) -> Result<bool> {
        let undone = self.require()?.composer.undo_candidate_action();
        if undone {
            self.personal_revision += 1;
        }
        self.refresh();
        Ok(undone)
    }

    /// Shows or hides the notice's 「復原」 and the word actions' tip (the controller decides both).
    pub fn set_word_overlay(&mut self, undo: bool, tip: Option<String>) {
        self.presentation.undo = undo;
        self.presentation.tip = tip;
    }

    fn require(&mut self) -> Result<&mut Open> {
        self.open.as_mut().ok_or_else(|| anyhow!(NOT_OPEN))
    }

    /// An engine is open.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.open.is_some()
    }

    /// The engine data in use.
    #[must_use]
    pub fn data(&self) -> Option<&Arc<EngineData>> {
        self.open.as_ref().map(|open| &open.data)
    }

    /// The engine.
    #[must_use]
    pub fn engine(&self) -> Option<&Arc<Engine>> {
        self.open.as_ref().map(|open| &open.engine)
    }

    /// The composer, for the model hooks (the seam for a reranker or character model) and for tests.
    pub fn composer_mut(&mut self) -> Option<&mut Composer> {
        self.open.as_mut().map(|open| &mut open.composer)
    }

    /// Candidate `index` of the open list as the engine knows it (for the correction log), before a pick.
    #[must_use]
    pub fn candidate_details(&self, index: usize) -> Option<crate::corrections::CandidateDetails> {
        let candidate = self.open.as_ref()?.composer.candidate(index)?;
        Some(crate::corrections::CandidateDetails {
            text: candidate.text.to_string(),
            reading: candidate.reading.to_string(),
            start: candidate.range.start,
            end: candidate.range.end,
        })
    }

    /// What the composer shows now.
    #[must_use]
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    /// Something is composed.
    #[must_use]
    pub fn composing(&self) -> bool {
        !self.presentation.composition.is_empty()
    }

    /// The text the last call committed ("" if none).
    #[must_use]
    pub fn committed(&self) -> &str {
        &self.committed
    }

    /// The selection keys of the current options.
    #[must_use]
    pub fn selection_keys(&self) -> Vec<char> {
        self.open
            .as_ref()
            .map_or_else(|| self.values.options().selection_keys, |o| o.composer.options().selection_keys.clone())
    }

    /// How long the last `open` took to load the lexicon and language models, in ms.  The reranker and character
    /// model are not loaded on this platform.
    #[must_use]
    pub fn load_ms(&self) -> Option<f64> {
        self.load_ms
    }

    /// Increases whenever personal data may have changed (a commit, a pick, an edit, an import).
    #[must_use]
    pub fn personal_revision(&self) -> u64 {
        self.personal_revision
    }

    /// Handles a key.  A key the engine does not take (or whose feature is off) passes through unchanged.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn key(&mut self, key: &Key) -> Result<KeyOutcome> {
        self.require()?;
        let Some(input) = key.input() else {
            self.committed.clear();
            return Ok(KeyOutcome::default());
        };
        self.input(input)
    }

    /// Handles a composer input (keys mapped by the caller, or a command's input).
    ///
    /// # Errors
    /// When no engine is open.
    pub fn input(&mut self, input: KeyInput) -> Result<KeyOutcome> {
        if let Some(feature) = feature_for(input)
            && !self.values.bool(feature, true)
        {
            self.require()?;
            self.committed.clear();
            return Ok(KeyOutcome::default());
        }
        let open = self.require()?;
        let started = Instant::now();
        let result = open.composer.handle(input);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let commit = result.commit.unwrap_or_default();
        // Return may add a selected phrase without committing text.
        if !commit.is_empty() || (result.handled && input == KeyInput::Enter) {
            self.personal_revision += 1;
        }
        self.finish(result.handled, commit, elapsed)
    }

    fn finish(&mut self, handled: bool, commit: String, elapsed: f64) -> Result<KeyOutcome> {
        self.committed.clone_from(&commit);
        self.refresh();
        let elapsed_ms = (handled || !commit.is_empty()).then_some(elapsed);
        Ok(KeyOutcome { handled, commit, elapsed_ms })
    }

    /// Picks candidate `index` of the whole list.
    ///
    /// # Errors
    /// When no engine is open or the index is outside the list.
    pub fn choose(&mut self, index: usize) -> Result<KeyOutcome> {
        let count = self.presentation.count;
        let open = self.require()?;
        ensure!(index < count, "候選字索引超出範圍。");
        let started = Instant::now();
        let result = open.composer.select_candidate(index);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if result.handled {
            self.personal_revision += 1;
        }
        let mut outcome = self.finish(result.handled, result.commit.unwrap_or_default(), elapsed)?;
        if !outcome.handled {
            outcome.elapsed_ms = None;
        }
        Ok(outcome)
    }

    /// Switches the open list between words as typed and typing corrections.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn toggle_corrections(&mut self) -> Result<()> {
        self.require()?.composer.toggle_corrections();
        self.committed.clear();
        self.refresh();
        Ok(())
    }

    /// Puts the correction in the open window's span (else the one the last key made, else the one at the cursor's
    /// target) back to the keys as typed; false when there is none.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn revert_correction(&mut self) -> Result<bool> {
        let reverted = self.require()?.composer.revert_correction();
        self.committed.clear();
        self.refresh();
        Ok(reverted)
    }

    /// The width of punctuation typed from now on.
    #[must_use]
    pub fn punctuation_width(&self) -> PunctuationWidth {
        self.open.as_ref().map_or(PunctuationWidth::Full, |open| open.composer.punctuation_width())
    }

    /// Switches the width of punctuation typed from now on (標點全半形); the composition stays.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn toggle_punctuation_width(&mut self) -> Result<PunctuationWidth> {
        self.input(KeyInput::TogglePunctuationWidth)?;
        Ok(self.punctuation_width())
    }

    /// Clears the composition without committing or learning.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn reset(&mut self) -> Result<()> {
        self.require()?.composer.reset();
        self.committed.clear();
        self.refresh();
        Ok(())
    }

    /// Commits everything composed; returns the text ("" when nothing was composed).
    ///
    /// # Errors
    /// When no engine is open.
    pub fn commit_all(&mut self) -> Result<KeyOutcome> {
        let open = self.require()?;
        let started = Instant::now();
        let commit = open.composer.commit_all();
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if !commit.is_empty() {
            self.personal_revision += 1;
        }
        let mut outcome = self.finish(true, commit, elapsed)?;
        if outcome.commit.is_empty() {
            outcome.elapsed_ms = None;
        }
        Ok(outcome)
    }

    /// Applies settings (all of them, from the settings store).
    ///
    /// # Errors
    /// When the engine refuses them (never for valid stored settings).
    pub fn apply(&mut self, settings: &Settings) -> Result<()> {
        let values: Map<String, Value> =
            settings::descriptors().iter().map(|d| (d.key.clone(), settings.get(&d.key))).collect();
        self.apply_settings(&values)
    }

    /// Merges `values` (setting key → value) into the settings in effect; all or nothing.  Keys the engine does not
    /// read are kept but ignored.
    ///
    /// # Errors
    /// When a value is invalid for its setting.
    pub fn apply_settings(&mut self, values: &Map<String, Value>) -> Result<()> {
        let mut next = self.values.clone();
        for (key, value) in values {
            validate(key, value)?;
            next.0.insert(key.clone(), value.clone());
        }
        self.values = next;
        if let Some(open) = &mut self.open {
            Self::configure(&mut open.composer, &open.engine, &self.values);
        }
        self.refresh();
        Ok(())
    }

    /// The setting `key` in effect for the engine.
    #[must_use]
    pub fn setting(&self, key: &str) -> Value {
        self.values.0.get(key).cloned().unwrap_or(Value::Null)
    }

    /// The text before the insertion point; the next composition continues its last words.  Ignored (cleared) while
    /// learning is suspended or 「參考游標前的文字」 is off; ignored by the composer while composing.
    ///
    /// # Errors
    /// When no engine is open or the text contains NUL.
    pub fn set_preceding_text(&mut self, text: &str) -> Result<()> {
        ensure!(valid_text(text), "前文不是有效文字。");
        let uses = !self.learning_suspended && self.values.bool("usesPrecedingText", true);
        self.require()?.composer.set_preceding_text(if uses { text } else { "" });
        self.model_context.set(if uses { text } else { "" });
        self.refresh();
        Ok(())
    }

    /// The next composition starts a sentence (the insertion point moved, or the text before it is unknown).
    ///
    /// # Errors
    /// When no engine is open.
    pub fn break_context(&mut self) -> Result<()> {
        self.require()?.composer.break_context();
        self.model_context.set("");
        self.refresh();
        Ok(())
    }

    /// Suspends learning (secure input); the text before the insertion point is dropped.
    ///
    /// # Errors
    /// When no engine is open.
    pub fn set_learning_suspended(&mut self, suspended: bool) -> Result<()> {
        let open = self.require()?;
        open.composer.set_learning_suspended(suspended);
        if suspended {
            open.composer.set_preceding_text("");
        }
        if suspended {
            self.model_context.set("");
        }
        self.suspended.store(suspended, std::sync::atomic::Ordering::Relaxed);
        self.learning_suspended = suspended;
        self.refresh();
        Ok(())
    }

    /// The sentence the reranker chose last has a typing correction (分析's 自動修正; macOS `shownCorrected`).
    #[must_use]
    pub fn shown_corrected(&self) -> bool {
        self.shown_corrected.get()
    }

    /// Forgets which sentence the reranker chose (the composition ended).
    pub fn clear_shown_corrected(&self) {
        self.shown_corrected.clear();
    }

    /// How long the neural models took to load (reranker, character model), when they did.
    #[must_use]
    pub fn model_load_ms(&self) -> (Option<f64>, Option<f64>) {
        self.open.as_ref().map_or((None, None), |open| {
            let models = open.data.models();
            (models.reranker_ms, models.character_ms)
        })
    }

    /// Learning is suspended.
    #[must_use]
    pub fn learning_suspended(&self) -> bool {
        self.learning_suspended
    }

    /// Writes pending personal data now.
    ///
    /// # Errors
    /// When no engine is open or the data cannot be written.
    pub fn flush(&mut self) -> Result<()> {
        let open = self.require()?;
        open.engine.flush_personal_data().map_err(|_| anyhow!("個人資料尚未寫入磁碟；請確認目錄權限後重試。"))
    }

    fn personal_changed(&mut self) -> Result<()> {
        self.personal_revision += 1;
        self.flush()
    }

    // MARK: Personal data

    /// The user's own phrases.
    #[must_use]
    pub fn user_phrases(&self) -> Vec<Phrase> {
        self.engine().map_or_else(Vec::new, |engine| {
            engine.user_phrases().into_iter().map(|key| Phrase::from_key(key, 0.0)).collect()
        })
    }

    /// Learned words with their counts.
    #[must_use]
    pub fn learned_entries(&self) -> Vec<Phrase> {
        self.engine().map_or_else(Vec::new, |engine| {
            engine.learned_entries().into_iter().map(|(key, count)| Phrase::from_key(key, count)).collect()
        })
    }

    /// Words never learned.
    #[must_use]
    pub fn excluded_entries(&self) -> Vec<Phrase> {
        self.engine().map_or_else(Vec::new, |engine| {
            engine.excluded_entries().into_iter().map(|key| Phrase::from_key(key, 0.0)).collect()
        })
    }

    /// Words never suggested (不要再推薦).
    #[must_use]
    pub fn blocked_entries(&self) -> Vec<Phrase> {
        self.engine().map_or_else(Vec::new, |engine| {
            engine.blocked_entries().into_iter().map(|key| Phrase::from_key(key, 0.0)).collect()
        })
    }

    /// Suggests a blocked word again.
    ///
    /// # Errors
    /// When the arguments are invalid or the change cannot be saved.
    pub fn unblock(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        self.require()?.engine.unblock(word, reading);
        self.personal_changed()
    }

    /// Adds a user phrase.
    ///
    /// # Errors
    /// When the phrase is invalid (1–6 characters, one valid syllable each) or cannot be saved.
    pub fn add_phrase(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        let engine = self.require()?.engine.clone();
        engine.add_user_phrase(word, reading).map_err(|error| anyhow!("無法加入自訂詞：{error}"))?;
        self.personal_changed()
    }

    /// Removes a user phrase.
    ///
    /// # Errors
    /// When the arguments are invalid or the change cannot be saved.
    pub fn remove_phrase(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        self.require()?.engine.remove_user_phrase(word, reading);
        self.personal_changed()
    }

    /// Replaces a phrase: adds the new one, then removes the old one when it differs (macOS's edit).  A refused new
    /// phrase keeps the old one.
    ///
    /// # Errors
    /// When either phrase is invalid or the change cannot be saved.
    pub fn edit_phrase(&mut self, old_word: &str, old_reading: &str, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(old_word, old_reading)?;
        validate_phrase(word, reading)?;
        let engine = self.require()?.engine.clone();
        engine.add_user_phrase(word, reading).map_err(|error| anyhow!("無法加入自訂詞：{error}"))?;
        if old_word != word || old_reading != reading {
            engine.remove_user_phrase(old_word, old_reading);
        }
        self.personal_changed()
    }

    /// The readings of `word` the lexicon knows (for the add-phrase sheet); empty when no engine is open.
    #[must_use]
    pub fn readings(&self, word: &str) -> Vec<String> {
        let word = word.trim();
        if word.is_empty() || !valid_text(word) {
            return Vec::new();
        }
        self.engine().map_or_else(Vec::new, |engine| engine.readings(word))
    }

    /// Forgets everything learned; user phrases and excluded words stay.
    ///
    /// # Errors
    /// When no engine is open or the change cannot be saved.
    pub fn forget_all_learned(&mut self) -> Result<()> {
        self.require()?.engine.forget_all_learned();
        self.personal_changed()
    }

    /// Forgets one learned word.
    ///
    /// # Errors
    /// When the arguments are invalid or the change cannot be saved.
    pub fn forget_learned(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        self.require()?.engine.forget_learned(word, reading);
        self.personal_changed()
    }

    /// Never learns this word again (and forgets it).
    ///
    /// # Errors
    /// When the arguments are invalid or the change cannot be saved.
    pub fn exclude_learned(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        self.require()?.engine.exclude_from_learning(word, reading);
        self.personal_changed()
    }

    /// Learns this word again.
    ///
    /// # Errors
    /// When the arguments are invalid or the change cannot be saved.
    pub fn include_learned(&mut self, word: &str, reading: &str) -> Result<()> {
        self.require()?;
        validate_phrase(word, reading)?;
        self.require()?.engine.include_in_learning(word, reading);
        self.personal_changed()
    }

    /// Merges a backup made by [`EngineSession::export_personal`] (or the macOS app); returns how many entries were
    /// read.
    ///
    /// # Errors
    /// When the file cannot be read, is not a backup, or the result cannot be saved.
    pub fn import_personal(&mut self, path: &Path) -> Result<usize> {
        self.require()?;
        let data = std::fs::read(path).with_context(|| format!("無法讀取備份檔：{}", path.display()))?;
        let count =
            self.require()?.engine.import_personal_data(&data).map_err(|error| anyhow!("備份格式無效：{error}"))?;
        self.personal_changed()?;
        Ok(count)
    }

    /// Writes all personal data to `path` as a backup.
    ///
    /// # Errors
    /// When no engine is open or the file cannot be written.
    pub fn export_personal(&mut self, path: &Path) -> Result<()> {
        self.flush()?;
        let text = self.require()?.engine.export_personal_data();
        settings::write_atomically(path, text.as_bytes()).context("無法匯出個人資料。")
    }
}

//! The engine handle: loading, fuzzy sounds, and the personal data (user phrases, learned words, exclusions,
//! export and import).

use std::ffi::c_char;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zhuyin_core::api::FuzzySounds;
use zhuyin_core::engine::{Engine, EngineConfig};

use crate::ZtStringList;
use crate::strings::{borrowed, owned, set_error, string_list};

/// An engine: lexicon, language models and personal data.  Thread-safe.
pub struct ZtEngine {
    pub(crate) engine: Arc<Engine>,
}

impl std::fmt::Debug for ZtEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZtEngine").finish_non_exhaustive()
    }
}

/// Settings read when an engine is created (Swift's `Engine` static variables).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZtEngineConfig {
    /// Score a composition as a whole sentence.
    pub sentence_boundaries: bool,
    /// The chat-domain model's directory (`lm_chat/`), or NULL for the encyclopedic model alone.
    pub chat_model_directory: *const c_char,
    /// The share of the sentence-end probability.
    pub sentence_end_share: f64,
    /// The chat model's prior weight in the mixture.
    pub chat_prior: f64,
    /// Domain evidence scale.
    pub domain_evidence_scale: f64,
    /// Domain evidence centre.
    pub domain_evidence_centre: f64,
    /// Hotspot replacement costs in the typing channel.
    pub hotspot: bool,
    /// The reading channel's table (`channel.bin`), or NULL for no reading channel.
    pub reading_channel: *const c_char,
}

/// The default settings (no chat model).
#[unsafe(no_mangle)]
pub extern "C" fn zt_engine_config_default() -> ZtEngineConfig {
    let config = EngineConfig::default();
    ZtEngineConfig {
        sentence_boundaries: config.sentence_boundaries,
        chat_model_directory: std::ptr::null(),
        sentence_end_share: config.sentence_end_share,
        chat_prior: config.chat_prior,
        domain_evidence_scale: config.domain_evidence_scale,
        domain_evidence_centre: config.domain_evidence_centre,
        hotspot: config.hotspot,
        reading_channel: std::ptr::null(),
    }
}

/// # Safety
/// `config` is NULL or a valid `ZtEngineConfig` whose paths are NULL or C strings.
unsafe fn engine_config(config: *const ZtEngineConfig) -> EngineConfig {
    // SAFETY: the caller passes NULL or a valid config.
    let Some(config) = (unsafe { config.as_ref() }) else { return EngineConfig::default() };
    EngineConfig {
        sentence_boundaries: config.sentence_boundaries,
        // SAFETY: NULL or a C string per the contract.
        chat_model_directory: unsafe { borrowed(config.chat_model_directory) }.map(PathBuf::from),
        sentence_end_share: config.sentence_end_share,
        chat_prior: config.chat_prior,
        domain_evidence_scale: config.domain_evidence_scale,
        domain_evidence_centre: config.domain_evidence_centre,
        hotspot: config.hotspot,
        // SAFETY: NULL or a C string per the contract.
        reading_channel: unsafe { borrowed(config.reading_channel) }.map(PathBuf::from),
        ..EngineConfig::default()
    }
}

fn handle(engine: Engine) -> *mut ZtEngine {
    Box::into_raw(Box::new(ZtEngine { engine: Arc::new(engine) }))
}

/// Opens the lexicon at `lexicon_path` (`lexicon.tsv`, or the compiled `lexicon.bin` beside it) with the language
/// models beside it; personal data is read from and written to `user_data_directory` (NULL: none).  `config` may be
/// NULL for the defaults.
///
/// # Safety
/// The strings are NULL or valid C strings; `config` is NULL or valid; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_open(
    lexicon_path: *const c_char,
    user_data_directory: *const c_char,
    config: *const ZtEngineConfig,
    error: *mut *mut c_char,
) -> *mut ZtEngine {
    // SAFETY: per the contract above.
    let (lexicon, user, config) =
        unsafe { (borrowed(lexicon_path), borrowed(user_data_directory), engine_config(config)) };
    let Some(lexicon) = lexicon else {
        // SAFETY: `error` is NULL or writable.
        unsafe { set_error(error, "lexicon path is not a UTF-8 string") };
        return std::ptr::null_mut();
    };
    match Engine::open(Path::new(lexicon), user.map(Path::new), config) {
        Ok(engine) => handle(engine),
        Err(e) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &e.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// An engine for TSV lexicon text (reading, word, log probability per line), with the language models in
/// `language_model_directory` (NULL: none).
///
/// # Safety
/// The strings are NULL or valid C strings; `config` is NULL or valid; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_open_text(
    lexicon_text: *const c_char,
    user_data_directory: *const c_char,
    language_model_directory: *const c_char,
    config: *const ZtEngineConfig,
    error: *mut *mut c_char,
) -> *mut ZtEngine {
    // SAFETY: per the contract above.
    let (text, user, model, config) = unsafe {
        (
            borrowed(lexicon_text),
            borrowed(user_data_directory),
            borrowed(language_model_directory),
            engine_config(config),
        )
    };
    let Some(text) = text else {
        // SAFETY: `error` is NULL or writable.
        unsafe { set_error(error, "lexicon text is not a UTF-8 string") };
        return std::ptr::null_mut();
    };
    handle(Engine::from_text(text, user.map(Path::new), model.map(Path::new), config))
}

/// Frees an engine handle (composers made from it keep the engine alive).  NULL is ignored.
///
/// # Safety
/// `engine` is NULL or a handle from this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_free(engine: *mut ZtEngine) {
    if !engine.is_null() {
        // SAFETY: allocated by `handle`.
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// # Safety
/// `engine` is a live handle.
unsafe fn engine_ref<'a>(engine: *const ZtEngine) -> Option<&'a Engine> {
    // SAFETY: the caller passes a live handle or NULL.
    unsafe { engine.as_ref() }.map(|e| &*e.engine)
}

/// Sets the fuzzy sound pairs (bits: 1 ㄗㄓ, 2 ㄘㄔ, 4 ㄙㄕ, 8 ㄣㄥ, 16 ㄢㄤ).
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_set_fuzzy_sounds(engine: *const ZtEngine, fuzzy: u32) {
    // SAFETY: live handle.
    if let Some(engine) = unsafe { engine_ref(engine) } {
        let mut sounds = FuzzySounds::NONE;
        for (bit, pair) in [
            (1, FuzzySounds::Z_ZH),
            (2, FuzzySounds::C_CH),
            (4, FuzzySounds::S_SH),
            (8, FuzzySounds::EN_ENG),
            (16, FuzzySounds::AN_ANG),
        ] {
            if fuzzy & bit != 0 {
                sounds.insert(pair);
            }
        }
        engine.set_fuzzy_sounds(sounds);
    }
}

/// The fuzzy sound pairs (see [`zt_engine_set_fuzzy_sounds`]).
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_fuzzy_sounds(engine: *const ZtEngine) -> u32 {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return 0 };
    let sounds = engine.fuzzy_sounds();
    [
        (1, FuzzySounds::Z_ZH),
        (2, FuzzySounds::C_CH),
        (4, FuzzySounds::S_SH),
        (8, FuzzySounds::EN_ENG),
        (16, FuzzySounds::AN_ANG),
    ]
    .into_iter()
    .filter(|&(_, pair)| sounds.contains(pair))
    .map(|(bit, _)| bit)
    .sum()
}

/// Writes personal data now instead of after the short delay.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_flush_personal_data(engine: *const ZtEngine) {
    // SAFETY: live handle.
    if let Some(engine) = unsafe { engine_ref(engine) } {
        engine.flush_personal_data();
    }
}

/// Forgets everything learned (user phrases stay).
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_forget_all_learned(engine: *const ZtEngine) {
    // SAFETY: live handle.
    if let Some(engine) = unsafe { engine_ref(engine) } {
        engine.forget_all_learned();
    }
}

/// One personal-data entry.
#[repr(C)]
#[derive(Debug)]
pub struct ZtPhrase {
    /// Its reading, syllables separated by spaces.
    pub reading: *mut c_char,
    /// The word.
    pub word: *mut c_char,
    /// For learned words, the learned count; 0 otherwise.
    pub count: f64,
}

/// A list of personal-data entries.
#[repr(C)]
#[derive(Debug)]
pub struct ZtPhraseList {
    /// `count` entries.
    pub items: *mut ZtPhrase,
    /// How many.
    pub count: usize,
}

fn phrase_list(entries: impl Iterator<Item = (String, String, f64)>) -> *mut ZtPhraseList {
    let items: Box<[ZtPhrase]> = entries
        .map(|(reading, word, count)| ZtPhrase { reading: owned(&reading), word: owned(&word), count })
        .collect();
    let count = items.len();
    Box::into_raw(Box::new(ZtPhraseList { items: Box::into_raw(items).cast(), count }))
}

/// Frees a list returned by this library.  NULL is ignored.
///
/// # Safety
/// `list` is NULL or a list returned by this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_phrase_list_free(list: *mut ZtPhraseList) {
    if list.is_null() {
        return;
    }
    // SAFETY: allocated by `phrase_list`.
    unsafe {
        let list = Box::from_raw(list);
        let items = Box::from_raw(std::ptr::slice_from_raw_parts_mut(list.items, list.count));
        for item in &*items {
            crate::zt_string_free(item.reading);
            crate::zt_string_free(item.word);
        }
    }
}

/// The user's own phrases.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_user_phrases(engine: *const ZtEngine) -> *mut ZtPhraseList {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return std::ptr::null_mut() };
    phrase_list(engine.user_phrases().into_iter().map(|p| (p.reading, p.word, 0.0)))
}

/// Learned words with their counts.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_learned_entries(engine: *const ZtEngine) -> *mut ZtPhraseList {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return std::ptr::null_mut() };
    phrase_list(engine.learned_entries().into_iter().map(|(p, count)| (p.reading, p.word, count)))
}

/// Words excluded from learning.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_excluded_entries(engine: *const ZtEngine) -> *mut ZtPhraseList {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return std::ptr::null_mut() };
    phrase_list(engine.excluded_entries().into_iter().map(|p| (p.reading, p.word, 0.0)))
}

/// The readings of `word` the lexicon knows.
///
/// # Safety
/// `engine` is a live handle; `word` a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_readings(engine: *const ZtEngine, word: *const c_char) -> *mut ZtStringList {
    // SAFETY: per the contract above.
    let (Some(engine), Some(word)) = (unsafe { (engine_ref(engine), borrowed(word)) }) else {
        return std::ptr::null_mut();
    };
    string_list(&engine.readings(word))
}

/// Adds a user phrase.
///
/// # Safety
/// `engine` is a live handle; the strings are C strings; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_add_user_phrase(
    engine: *const ZtEngine,
    word: *const c_char,
    reading: *const c_char,
    error: *mut *mut c_char,
) -> bool {
    // SAFETY: per the contract above.
    let (Some(engine), Some(word), Some(reading)) =
        (unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) })
    else {
        // SAFETY: `error` is NULL or writable.
        unsafe { set_error(error, "invalid argument") };
        return false;
    };
    match engine.add_user_phrase(word, reading) {
        Ok(()) => true,
        Err(e) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &e.to_string()) };
            false
        }
    }
}

/// Removes a user phrase.
///
/// # Safety
/// `engine` is a live handle; the strings are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_remove_user_phrase(
    engine: *const ZtEngine,
    word: *const c_char,
    reading: *const c_char,
) {
    // SAFETY: per the contract above.
    let arguments = unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) };
    if let (Some(engine), Some(word), Some(reading)) = arguments {
        engine.remove_user_phrase(word, reading);
    }
}

/// Forgets one learned word.
///
/// # Safety
/// `engine` is a live handle; the strings are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_forget_learned(
    engine: *const ZtEngine,
    word: *const c_char,
    reading: *const c_char,
) {
    // SAFETY: per the contract above.
    let arguments = unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) };
    if let (Some(engine), Some(word), Some(reading)) = arguments {
        engine.forget_learned(word, reading);
    }
}

/// Never learns this word again (and forgets it).
///
/// # Safety
/// `engine` is a live handle; the strings are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_exclude_from_learning(
    engine: *const ZtEngine,
    word: *const c_char,
    reading: *const c_char,
) {
    // SAFETY: per the contract above.
    let arguments = unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) };
    if let (Some(engine), Some(word), Some(reading)) = arguments {
        engine.exclude_from_learning(word, reading);
    }
}

/// Learns this word again.
///
/// # Safety
/// `engine` is a live handle; the strings are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_include_in_learning(
    engine: *const ZtEngine,
    word: *const c_char,
    reading: *const c_char,
) {
    // SAFETY: per the contract above.
    let arguments = unsafe { (engine_ref(engine), borrowed(word), borrowed(reading)) };
    if let (Some(engine), Some(word), Some(reading)) = arguments {
        engine.include_in_learning(word, reading);
    }
}

/// All personal data as text, for a backup.
///
/// # Safety
/// `engine` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_export_personal_data(engine: *const ZtEngine) -> *mut c_char {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return std::ptr::null_mut() };
    owned(&engine.export_personal_data())
}

/// Merges exported personal data; returns how many entries were read, or −1 on error.
///
/// # Safety
/// `engine` is a live handle; `data` points to `length` bytes; `error` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_engine_import_personal_data(
    engine: *const ZtEngine,
    data: *const u8,
    length: usize,
    error: *mut *mut c_char,
) -> i64 {
    // SAFETY: live handle.
    let Some(engine) = (unsafe { engine_ref(engine) }) else { return -1 };
    let bytes = if length == 0 {
        &[][..]
    } else if data.is_null() {
        // SAFETY: `error` is NULL or writable.
        unsafe { set_error(error, "no data") };
        return -1;
    } else {
        // SAFETY: the caller passes `length` readable bytes.
        unsafe { std::slice::from_raw_parts(data, length) }
    };
    match engine.import_personal_data(bytes) {
        Ok(count) => i64::try_from(count).unwrap_or(i64::MAX),
        Err(e) => {
            // SAFETY: `error` is NULL or writable.
            unsafe { set_error(error, &e.to_string()) };
            -1
        }
    }
}

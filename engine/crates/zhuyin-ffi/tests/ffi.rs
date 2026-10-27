//! The C ABI from Rust: engine, composer, presentation, personal data, hooks and host models, against the Rust API
//! it wraps.
#![allow(missing_docs, unsafe_code, clippy::similar_names, clippy::cast_possible_wrap)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zhuyin::*;
use zhuyin_core::api::{ComposerOptions, KeyInput};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};

const LEXICON: &str = "ㄋㄧˇ\t你\t-3.0\nㄋㄧˇ\t妳\t-4.0\nㄏㄠˇ\t好\t-3.0\nㄏㄠˇ\t郝\t-6.0\nㄋㄧˇ ㄏㄠˇ\t你好\t-2.0\n\
                       ㄇㄚ˙\t嗎\t-3.0\nㄇㄚ˙\t麼\t-5.0\n";

fn text(pointer: *mut c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: a string returned by the library, freed right after.
    let string = unsafe { CStr::from_ptr(pointer) }.to_str().unwrap().to_owned();
    // SAFETY: returned by the library.
    unsafe { zt_string_free(pointer) };
    Some(string)
}

fn key(character: char) -> ZtKey {
    match character {
        ' ' => ZtKey { kind: ZT_KEY_SPACE, character: 0 },
        '\n' => ZtKey { kind: ZT_KEY_ENTER, character: 0 },
        '↓' => ZtKey { kind: ZT_KEY_DOWN, character: 0 },
        '←' => ZtKey { kind: ZT_KEY_LEFT, character: 0 },
        c => ZtKey { kind: ZT_KEY_CHAR, character: c as u32 },
    }
}

fn input(character: char) -> KeyInput {
    match character {
        ' ' => KeyInput::Space,
        '\n' => KeyInput::Enter,
        '↓' => KeyInput::Down,
        '←' => KeyInput::Left,
        c => KeyInput::Char(c),
    }
}

/// The presentation as one comparable line.
fn presentation_line(composer: *mut ZtComposer) -> String {
    // SAFETY: a live composer; the presentation is freed below.
    unsafe {
        let p = zt_composer_presentation(composer);
        let r = &*p;
        let clauses: Vec<String> = (0..r.clause_count)
            .map(|i| {
                let c = &*r.clauses.add(i);
                format!("{}-{}", c.start, c.end)
            })
            .collect();
        let list = if r.candidates.is_null() {
            "-".to_owned()
        } else {
            let l = &*r.candidates;
            let items: Vec<String> =
                (0..l.count).map(|i| CStr::from_ptr((*l.items.add(i)).text).to_str().unwrap().to_owned()).collect();
            format!("{}/{}/{}/{}", l.selection, l.page_size, l.tolerant, items.join(","))
        };
        let line = format!(
            "{}|{}|{}|{}|{}|{}",
            CStr::from_ptr(r.text).to_str().unwrap(),
            r.cursor,
            clauses.join(","),
            r.focused_clause,
            r.target_clause,
            list
        );
        zt_presentation_free(p);
        line
    }
}

fn rust_line(composer: &Composer) -> String {
    let p = composer.presentation();
    let clauses: Vec<String> = p.clauses.iter().map(|c| format!("{}-{}", c.start, c.end)).collect();
    let index = |v: Option<usize>| v.map_or(-1, |v| v as isize);
    let list = p.candidates.as_ref().map_or("-".to_owned(), |l| {
        let items: Vec<&str> = l.items.iter().map(|c| c.text.as_str()).collect();
        format!("{}/{}/{}/{}", l.selection, l.page_size, l.tolerant.map_or(-1, i8::from), items.join(","))
    });
    format!(
        "{}|{}|{}|{}|{}|{}",
        p.text,
        p.cursor,
        clauses.join(","),
        index(p.focused_clause),
        index(p.target_clause),
        list
    )
}

fn open_engine() -> *mut ZtEngine {
    let lexicon = CString::new(LEXICON).unwrap();
    let mut error = std::ptr::null_mut();
    // SAFETY: valid strings and an error slot.
    let engine = unsafe {
        zt_engine_open_text(lexicon.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), &raw mut error)
    };
    assert!(!engine.is_null(), "{:?}", text(error));
    engine
}

#[test]
fn composer_matches_the_rust_composer_key_by_key() {
    let engine = open_engine();
    let options = zt_composer_options_default();
    // SAFETY: a live engine and default options.
    let composer = unsafe { zt_composer_new(engine, &raw const options) };
    assert!(!composer.is_null());
    let rust_engine = Arc::new(Engine::from_text(LEXICON, None, None, EngineConfig::default()));
    let mut rust = Composer::new(rust_engine, ComposerOptions::default());
    let mut time = 1000.0;
    for script in ["su3cl3\n", "su3cl3←↓↓\n\n", "su3cl3 ↓\n", "su3a87\n", "su3cl3a87←←↓\n"] {
        for c in script.chars() {
            time += 0.12;
            // SAFETY: a live composer.
            let result = unsafe { zt_composer_handle_at(composer, key(c), time) };
            let expected = rust.handle_at(input(c), time);
            assert_eq!(result.handled, expected.handled, "{script:?} {c:?}");
            assert_eq!(text(result.commit), expected.commit, "{script:?} {c:?}");
            assert_eq!(presentation_line(composer), rust_line(&rust), "{script:?} {c:?}");
        }
    }
    // SAFETY: live handles.
    unsafe {
        zt_composer_reset(composer);
        assert!(!zt_composer_is_composing(composer));
        zt_engine_free(engine);
        // The composer keeps the engine alive.
        let _ = zt_composer_handle(composer, key('s'));
        assert!(zt_composer_is_composing(composer));
        assert_eq!(text(zt_composer_commit_all(composer)).as_deref(), Some("ㄋ"));
        zt_composer_free(composer);
    }
}

#[test]
fn options_are_checked() {
    let engine = open_engine();
    let mut options = zt_composer_options_default();
    options.typo_tolerance = 9;
    // SAFETY: a live engine.
    unsafe {
        assert!(zt_composer_new(engine, &raw const options).is_null());
        options.typo_tolerance = ZT_TYPO_OFF;
        let keys = CString::new("asdfghjkl").unwrap();
        options.selection_keys = keys.as_ptr();
        let composer = zt_composer_new(engine, &raw const options);
        assert!(!composer.is_null());
        for c in "su3 ".chars() {
            let _ = zt_composer_handle(composer, key(c));
        }
        // The window opened with the selection keys; `s` picks the second candidate.
        let result = zt_composer_handle(composer, key('s'));
        assert!(result.handled);
        assert_eq!(text(zt_composer_commit_all(composer)).as_deref(), Some("妳"));
        options.space_action = 7;
        assert!(!zt_composer_set_options(composer, &raw const options));
        zt_composer_free(composer);
        zt_engine_free(engine);
    }
}

#[test]
fn personal_data_round_trip() {
    let engine = open_engine();
    let word = CString::new("郝好").unwrap();
    let reading = CString::new("ㄏㄠˇ ㄏㄠˇ").unwrap();
    // SAFETY: live handle and valid strings.
    unsafe {
        let mut error = std::ptr::null_mut();
        assert!(zt_engine_add_user_phrase(engine, word.as_ptr(), reading.as_ptr(), &raw mut error));
        let list = zt_engine_user_phrases(engine);
        assert_eq!((*list).count, 1);
        assert_eq!(CStr::from_ptr((*(*list).items).word).to_str().unwrap(), "郝好");
        zt_phrase_list_free(list);
        let readings = zt_engine_readings(engine, CString::new("你").unwrap().as_ptr());
        assert_eq!((*readings).count, 1);
        zt_string_list_free(readings);
        let exported = text(zt_engine_export_personal_data(engine)).unwrap();
        zt_engine_remove_user_phrase(engine, word.as_ptr(), reading.as_ptr());
        let list = zt_engine_user_phrases(engine);
        assert_eq!((*list).count, 0);
        zt_phrase_list_free(list);
        let read = zt_engine_import_personal_data(engine, exported.as_ptr(), exported.len(), &raw mut error);
        assert!(read >= 1, "{read} {:?}", text(error));
        let list = zt_engine_user_phrases(engine);
        assert_eq!((*list).count, 1);
        zt_phrase_list_free(list);
        assert_eq!(zt_engine_import_personal_data(engine, b"\xff\xfe".as_ptr(), 2, &raw mut error), -1);
        assert!(text(error).is_some());
        // Learning from a commit, then forgetting it.
        let composer = zt_composer_new(engine, std::ptr::null());
        for c in "su3cl3\n".chars() {
            let _ = text(zt_composer_handle(composer, key(c)).commit);
        }
        zt_engine_forget_all_learned(engine);
        let learned = zt_engine_learned_entries(engine);
        assert_eq!((*learned).count, 0);
        zt_phrase_list_free(learned);
        zt_engine_set_fuzzy_sounds(engine, 1 | 16);
        assert_eq!(zt_engine_fuzzy_sounds(engine), 17);
        zt_composer_free(composer);
        zt_engine_free(engine);
    }
}

static RELEASED: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn release(_context: *mut c_void) {
    RELEASED.fetch_add(1, Ordering::SeqCst);
}

/// The hook test's own counter (tests run in parallel).
static RELEASED_HOOKS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn release_hook(_context: *mut c_void) {
    RELEASED_HOOKS.fetch_add(1, Ordering::SeqCst);
}

/// Picks the last sentence and records the texts it saw.
unsafe extern "C" fn choose_last(context: *mut c_void, sentences: *const ZtSentences) -> isize {
    // SAFETY: the test's own context, a Vec<String>.
    let seen = unsafe { &mut *context.cast::<Vec<String>>() };
    // SAFETY: the live view.
    let count = unsafe { zt_sentences_count(sentences) };
    for i in 0..count {
        // SAFETY: the live view.
        seen.push(text(unsafe { zt_sentences_text(sentences, i) }).unwrap());
    }
    count as isize - 1
}

#[test]
fn reranker_hook_chooses_the_sentence() {
    let engine = open_engine();
    // SAFETY: live handles; the context outlives the composer.
    unsafe {
        let composer = zt_composer_new(engine, std::ptr::null());
        zt_composer_set_reranking_count(composer, 4);
        let mut seen: Vec<String> = vec![];
        let before = RELEASED_HOOKS.load(Ordering::SeqCst);
        zt_composer_set_reranker(
            composer,
            ZtRerankerHook { context: (&raw mut seen).cast(), choose: Some(choose_last), release: Some(release_hook) },
        );
        for c in "su3".chars() {
            let _ = zt_composer_handle(composer, key(c));
        }
        assert!(!seen.is_empty());
        let shown = presentation_line(composer);
        assert!(shown.starts_with(seen.last().unwrap().as_str()), "{shown} {seen:?}");
        // Removing the hook releases its context once.
        zt_composer_set_reranker(
            composer,
            ZtRerankerHook { context: std::ptr::null_mut(), choose: None, release: None },
        );
        assert_eq!(RELEASED_HOOKS.load(Ordering::SeqCst), before + 1);
        zt_composer_free(composer);
        zt_engine_free(engine);
    }
}

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("zhuyin-ffi-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("vocab.json"), r#"["<pad>","<bos>","<unk>","你","好","妳"]"#).unwrap();
        std::fs::write(path.join("chars.json"), r#"["<pad>","<bos>","<unk>","<sep>","你","好","妳","郝"]"#).unwrap();
        std::fs::write(path.join("syllables.json"), r#"["<pad>","<literal>","ㄋㄧ","ㄏㄠ"]"#).unwrap();
        Directory(path)
    }

    fn c_path(&self) -> CString {
        CString::new(self.0.to_str().unwrap()).unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every target is likely except `妳` (ID 5).
unsafe extern "C" fn logprob(
    _context: *mut c_void,
    _ids: *const i32,
    targets: *const i32,
    rows: usize,
    length: usize,
    out: *mut f64,
) -> bool {
    for i in 0..rows * length {
        // SAFETY: sized by the library.
        unsafe { *out.add(i) = if *targets.add(i) == 5 { -9.0 } else { -1.0 } };
    }
    true
}

#[test]
fn host_reranker_scores_and_chooses() {
    let directory = Directory::new("reranker");
    std::fs::write(directory.0.join("config.json"), r#"{"weight": 2.0, "count": 6}"#).unwrap();
    let path = directory.c_path();
    // SAFETY: valid strings, models and handles.
    unsafe {
        let before = RELEASED.load(Ordering::SeqCst);
        let model = ZtRerankerModel {
            context: std::ptr::null_mut(),
            logprob: Some(logprob),
            tree_logprob: None,
            release: Some(release),
        };
        let reranker = zt_reranker_configured(path.as_ptr(), model, std::ptr::null_mut());
        assert!(!reranker.is_null());
        assert_eq!(zt_reranker_candidate_count(reranker), 6);
        assert_eq!(zt_reranker_weight(reranker), 2.0);
        let candidates = [CString::new("你好").unwrap(), CString::new("妳好").unwrap()];
        let pointers: Vec<*const c_char> = candidates.iter().map(|c| c.as_ptr()).collect();
        let mut scores = [0.0; 2];
        let context = CString::new("").unwrap();
        assert!(zt_reranker_scores(reranker, context.as_ptr(), pointers.as_ptr(), 2, scores.as_mut_ptr()));
        assert_eq!(scores, [-2.0, -10.0]);
        assert!(zt_reranker_candidate_scores(reranker, context.as_ptr(), pointers.as_ptr(), 2, scores.as_mut_ptr()));
        assert_eq!(scores, [-4.0, -20.0]);
        zt_reranker_free(reranker);
        assert_eq!(RELEASED.load(Ordering::SeqCst), before + 1);
        // A missing vocabulary fails and still releases the model.
        let missing = CString::new("/nonexistent/zhuyin").unwrap();
        let mut error = std::ptr::null_mut();
        let model = ZtRerankerModel {
            context: std::ptr::null_mut(),
            logprob: Some(logprob),
            tree_logprob: None,
            release: Some(release),
        };
        assert!(zt_reranker_new(missing.as_ptr(), 1.0, model, &raw mut error).is_null());
        assert!(text(error).is_some());
        assert_eq!(RELEASED.load(Ordering::SeqCst), before + 2);
    }
}

/// Scores each candidate column `c` as −c.
unsafe extern "C" fn predict(_context: *mut c_void, input: *const ZtSyllableInput, out: *mut f64) -> bool {
    // SAFETY: sized by the library.
    unsafe {
        let input = &*input;
        assert_eq!(*input.context_ids, 1);
        for row in 0..input.count {
            for column in 0..input.width {
                *out.add(row * input.width + column) = -(column as f64);
            }
        }
    }
    true
}

/// The composer's character hook, passing its view straight to the scorer in `context`.
unsafe extern "C" fn score_characters(
    context: *mut c_void,
    syllables: *const ZtSyllable,
    count: usize,
    candidates: *const ZtCandidateRows,
    scores: *mut f64,
    lengths: *mut usize,
) -> bool {
    let text = CString::new("").unwrap();
    // SAFETY: the context is the test's scorer; the views are live.
    unsafe {
        zt_syllable_scorer_scores_rows(context.cast(), text.as_ptr(), syllables, count, candidates, scores, lengths)
    }
}

#[test]
fn host_character_model_through_the_composer() {
    let directory = Directory::new("syllable");
    let path = directory.c_path();
    let engine = open_engine();
    // SAFETY: valid strings, models and handles.
    unsafe {
        let model = ZtSyllableModel { context: std::ptr::null_mut(), predict: Some(predict), release: None };
        let scorer = zt_syllable_scorer_new(path.as_ptr(), model, std::ptr::null_mut());
        assert!(!scorer.is_null());
        // Direct scoring: row starts 0, 2, 3.
        let strings = [CString::new("你").unwrap(), CString::new("妳").unwrap(), CString::new("好").unwrap()];
        let pointers: Vec<*const c_char> = strings.iter().map(|c| c.as_ptr()).collect();
        let bases = [CString::new("ㄋㄧ").unwrap(), CString::new("ㄏㄠ").unwrap()];
        let syllables = [
            ZtSyllable { base: bases[0].as_ptr(), tone: 3, cost_bucket: 0, implicit: false },
            ZtSyllable { base: bases[1].as_ptr(), tone: 3, cost_bucket: 0, implicit: false },
        ];
        let starts = [0usize, 2, 3];
        let mut scores = [f64::NAN; 3];
        let mut lengths = [0usize; 2];
        let context = CString::new("你").unwrap();
        assert!(zt_syllable_scorer_scores(
            scorer,
            context.as_ptr(),
            syllables.as_ptr(),
            2,
            pointers.as_ptr(),
            starts.as_ptr(),
            scores.as_mut_ptr(),
            lengths.as_mut_ptr()
        ));
        assert_eq!(lengths, [2, 1]);
        assert_eq!(scores, [0.0, -1.0, 0.0]);
        // Through the composer's hook: the conversion still completes with the scores in the lattice.
        let composer = zt_composer_new(engine, std::ptr::null());
        zt_composer_set_character_weight(composer, 0.5);
        zt_composer_set_character_scorer(
            composer,
            ZtCharacterScorerHook { context: scorer.cast(), score: Some(score_characters), release: None },
        );
        let mut committed = String::new();
        for c in "su3cl3\n".chars() {
            committed += &text(zt_composer_handle(composer, key(c)).commit).unwrap_or_default();
        }
        assert_eq!(committed, "你好");
        zt_composer_free(composer);
        zt_syllable_scorer_free(scorer);
        zt_engine_free(engine);
    }
}

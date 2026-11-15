//! The correction log (「記錄改字」, `docs/product/DESIGN.md` §2.7), ported from macOS `IMK/CorrectionRecorder.swift`
//! (through the Qt `CorrectionLog`): every composition in which the user corrected the engine becomes one JSON line
//! of `corrections.jsonl`, in the macOS schema (schema 1, snake_case keys in sorted order), so `zt eval corrections`
//! reads both.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::Value;
use zhuyin_core::api::KeyInput;

/// The log file's name in the user's data directory.
pub const FILE_NAME: &str = "corrections.jsonl";

/// What the input controller reports to the recorder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObservedInput {
    /// A key, with the monotonic time (seconds) it went down.
    Key { input: KeyInput, time: f64 },
    /// A click on candidate `index` of the whole list.
    Click { index: usize },
    /// The candidate window's corrections switch.
    ToggleCorrections,
    /// Everything composed was sent (focus loss, a key for the app).
    Commit,
}

/// `corrections.jsonl`: appended, counted, exported and cleared for the 進階 page.  Cheap to clone; clones share it.
#[derive(Clone)]
pub struct CorrectionLog {
    inner: Arc<Mutex<Log>>,
}

struct Log {
    path: Option<PathBuf>,
}

impl CorrectionLog {
    /// The log in `directory` (`None`: records are kept in memory, for practice sessions and tests).
    #[must_use]
    pub fn open(directory: Option<&Path>) -> Self {
        Self { inner: Arc::new(Mutex::new(Log { path: directory.map(|d| d.join(FILE_NAME)) })) }
    }

    /// How many records the log has.
    #[must_use]
    pub fn count(&self) -> usize {
        todo!()
    }

    /// The file, when the log is on disk.
    #[must_use]
    pub fn location(&self) -> Option<PathBuf> {
        self.inner.lock().path.clone()
    }

    /// Appends one record.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn append(&self, _record: &Value) -> Result<()> {
        todo!()
    }

    /// Copies the log to `destination`.
    ///
    /// # Errors
    /// When it cannot be read or written.
    pub fn export(&self, _destination: &Path) -> Result<()> {
        todo!()
    }

    /// Deletes every record.
    ///
    /// # Errors
    /// When the file cannot be deleted.
    pub fn clear(&self) -> Result<()> {
        todo!()
    }

    /// The records kept in memory (a log opened without a directory).
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        todo!()
    }

    /// Why the last write failed, if it did.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        todo!()
    }
}

/// A record line: compact JSON with every object's keys sorted (the macOS `JSONEncoder` with `.sortedKeys`).
#[must_use]
pub fn encode(_record: &Value) -> String {
    todo!()
}

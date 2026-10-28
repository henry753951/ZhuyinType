//! Memory-mapped data files: the ZTLX/1 lexicon ([`MappedLexicon`]) and the ZTLM/1 n-gram language model
//! ([`NGramLanguageModel`]).
//!
//! This is the only module with `unsafe` code, each block allowed one by one: creating the mapping
//! (`Mapping::open`), and turning lexicon byte ranges that were validated as UTF-8 when the file was opened into
//! `&str` without validating them again on every lookup (`MappedLexicon::spelling`).

mod lexicon;
mod ngram;

pub use lexicon::{MappedEntry, MappedLexicon};
pub use ngram::NGramLanguageModel;

use std::fmt;
use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

/// Why a data file could not be opened.
#[derive(Debug)]
pub enum DataError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file is not a valid ZTLX/1 lexicon.
    InvalidLexicon,
    /// The file is not a valid ZTLM/1 language model.
    InvalidLanguageModel,
    /// The file is not a valid ZTCB/1 reading channel.
    InvalidChannel,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io(error) => write!(f, "{error}"),
            DataError::InvalidLexicon => f.write_str("Invalid or truncated ZTLX/1 lexicon"),
            DataError::InvalidLanguageModel => f.write_str("Invalid or truncated ZTLM/1 language model"),
            DataError::InvalidChannel => f.write_str("Invalid or truncated ZTCB/1 reading channel"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<std::io::Error> for DataError {
    fn from(error: std::io::Error) -> Self {
        DataError::Io(error)
    }
}

/// A read-only file mapping.  Empty files have no mapping (mapping zero bytes fails on some systems).
pub(crate) struct Mapping {
    map: Option<Mmap>,
}

impl Mapping {
    /// Maps the whole file read-only.
    pub(crate) fn open(path: &Path) -> std::io::Result<Mapping> {
        let file = File::open(path)?;
        if file.metadata()?.len() == 0 {
            return Ok(Mapping { map: None });
        }
        // SAFETY: the mapping is read-only and private to this process.  The data files are installed atomically
        // (written to a temporary file, then renamed), so the mapped file is never modified in place while it is
        // mapped; a replacement is a new inode and leaves this mapping intact.  This is the same assumption the
        // Swift engine makes with `Data(contentsOf:options: .alwaysMapped)`.
        #[allow(unsafe_code)]
        let map = unsafe { Mmap::map(&file)? };
        Ok(Mapping { map: Some(map) })
    }

    /// The mapped bytes.
    #[inline]
    pub(crate) fn bytes(&self) -> &[u8] {
        self.map.as_deref().unwrap_or(&[])
    }
}

/// Little-endian `u32` at `offset`.
#[inline]
pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

/// Little-endian `u64` at `offset`.
#[inline]
pub(crate) fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
}

/// Little-endian IEEE-754 binary64 at `offset`.
#[inline]
pub(crate) fn f64_at(bytes: &[u8], offset: usize) -> f64 {
    f64::from_bits(u64_at(bytes, offset))
}

/// Little-endian IEEE-754 binary32 at `offset`, widened exactly to `f64`.
#[inline]
pub(crate) fn f32_at(bytes: &[u8], offset: usize) -> f64 {
    f64::from(f32::from_bits(u32_at(bytes, offset)))
}

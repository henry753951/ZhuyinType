//! Token tables read from the model directories' JSON files.

use std::collections::HashMap;
use std::path::Path;

use zhuyin_core::swift_format::grapheme_count;

use crate::ModelError;

/// A JSON array of strings.
pub(crate) fn read_strings(path: &Path) -> Result<Vec<String>, ModelError> {
    let data = std::fs::read(path)?;
    serde_json::from_slice(&data).map_err(|e| ModelError(format!("{}: {e}", path.display())))
}

/// One-character tokens (one grapheme each) and their IDs.  Keyed by `char` for the single-scalar ones (every Han
/// character), so the table holds no string per token, as Swift's `[Character: Int32]` holds them inline.
#[derive(Debug, Default)]
pub(crate) struct CharacterIndex {
    scalars: HashMap<char, i32>,
    clusters: HashMap<Box<str>, i32>,
}

impl CharacterIndex {
    /// The ID of `character` (one grapheme), if it has one.
    pub(crate) fn get(&self, character: &str) -> Option<i32> {
        let mut scalars = character.chars();
        match (scalars.next(), scalars.next()) {
            (Some(scalar), None) => self.scalars.get(&scalar).copied(),
            _ => self.clusters.get(character).copied(),
        }
    }
}

/// One-character tokens from ID `first` on; a later duplicate wins, as in Swift's dictionary assignment.
pub(crate) fn character_index(tokens: &[String], first: usize) -> CharacterIndex {
    let mut index = CharacterIndex::default();
    for (id, token) in tokens.iter().enumerate() {
        if id < first || grapheme_count(token) != 1 {
            continue;
        }
        let id = i32::try_from(id).unwrap_or(i32::MAX);
        let mut scalars = token.chars();
        match (scalars.next(), scalars.next()) {
            (Some(scalar), None) => {
                index.scalars.insert(scalar, id);
            }
            _ => {
                index.clusters.insert(token.as_str().into(), id);
            }
        }
    }
    index.scalars.shrink_to_fit();
    index
}

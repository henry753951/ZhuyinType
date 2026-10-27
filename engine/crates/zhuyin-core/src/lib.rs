//! The `ZhuyinType` input engine: the data layer, decoding and the composer.  Ported from the Swift engine (removed in
//! phase 5; `engine/DESIGN.md`), bit for bit; the table maps its files to the modules.
//!
//! | Swift (`ZhuyinCore`) | Rust |
//! |---|---|
//! | `API.swift` | [`api`] |
//! | `Keyboard.swift` (`ZhuyinKeyboard`) | [`keyboard`] |
//! | `KeyChannel.swift` | [`key_channel`] |
//! | `KeyDecoder.swift` | [`key_decoder`] |
//! | `LanguageModel.swift` | [`language_model`], [`tsv`] |
//! | `MappedLexicon.swift` | [`binary::MappedLexicon`] |
//! | `NGramLanguageModel.swift` | [`binary::NGramLanguageModel`] |
//! | `Engine.swift`: `WordEntry`, TSV lexicon loading | [`lexicon`] |
//! | `Engine.swift`: syllable validity and channel codes | [`syllables`] |
//! | `Engine.swift` | [`engine`] |
//! | `Conversion.swift` | [`conversion`] |
//! | `Composer.swift` | [`composer`] |
//! | `PersonalLanguageModel.swift` | [`personal`] |
//!
//! Numbers follow the Swift code operation for operation (`f64` where Swift uses `Double`, the same summation
//! order), so scores are bit-identical; the golden tests check this against values the Swift code produced.

pub mod api;
pub mod binary;
pub mod composer;
pub mod conversion;
pub mod engine;
pub mod key_channel;
pub mod key_decoder;
pub mod keyboard;
pub mod language_model;
pub mod lexicon;
pub mod personal;
pub mod swift_format;
pub mod syllables;
pub mod tsv;

pub use binary::{DataError, MappedLexicon, NGramLanguageModel};
pub use language_model::{LanguageModel, LanguageModelHistory, VocabularyLanguageModel};

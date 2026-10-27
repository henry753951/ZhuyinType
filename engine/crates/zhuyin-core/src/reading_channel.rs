//! The reading channel: a character typed with another reading than the lexicon's (熟 typed ㄕㄡˊ, 期中 typed
//! ㄑㄧˊ ㄓㄨㄥ, 蚵 typed ㄎㄜ, 什麼 typed ㄇㄛˊ), and third-tone sandhi (你好 typed ㄋㄧˊ ㄏㄠˇ).
//!
//! The table is `channel.bin` (ZTCB/1, compiled from `channel.tsv` and `taiwan.tsv` by `zt data compile`; formats in
//! `resources/formats/reading-channel.md`).  Costs are computed from it when the engine loads, with the settings'
//! offsets, so tuning a cost needs neither a new table nor a new lexicon.
//!
//! Each variant is in one of two states:
//!
//! - an accepted reading is typed as freely as the lexicon's: no annotation, no edit counted, not a correction for
//!   the reranker, `cost_bucket` 0 for the character model;
//! - a correction (a phonetic-series misreading, 有邊讀邊) is shown with the reading it replaces, counts as an edit,
//!   and must beat the best sentence matching the keys as typed by the reranker's `reading_margin`.
//!
//! At most one syllable of a word goes through the channel.  A word reaches a variant through an index built when
//! the engine loads, keyed by (lexicon reading, position, character), so typing ㄕㄡˊ looks up the words with 熟 read
//! ㄕㄨˊ at that position instead of every word read ㄕㄨˊ.  Words whose characters and syllables do not align (with
//! a letter or an erhua syllable) have no variants.  A lexicon row for the variant reading wins over the channel.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::binary::DataError;
use crate::engine::is_han;
use crate::lexicon::WordEntry;
use crate::syllables::without_tones;

/// Where a reading variant comes from.  The first eight are `channel.tsv`'s classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelClass {
    /// Another lexicon reading of the character (in a word that reads it otherwise).
    Lexicon,
    /// Wiktionary's Taiwan reading of the character.
    Taiwan,
    /// The character's reading inside dictionary words.
    Word,
    /// Another reading Wiktionary gives the character.
    Attested,
    /// A reading only CNS11643 lists.
    Cns,
    /// A reading only Unihan lists (mostly archaic).
    Unihan,
    /// The phonetic component's reading (有邊讀邊), for unfamiliar characters.
    Series,
    /// Another tone of an interjection.
    Interjection,
    /// `taiwan.tsv`: the Taiwan reading of a character inside words (期中 ㄑㄧˊ ㄓㄨㄥ).
    TaiwanWord,
    /// A neutral-tone syllable typed with another reading of its character (什麼 ㄕㄣˊ ㄇㄛˊ).
    Neutral,
    /// Third-tone sandhi: before a third tone, a second and a third tone typed as each other.
    Sandhi,
}

impl ChannelClass {
    /// Every class, in declaration order (the order of [`ChannelSettings::offsets`]).
    pub const ALL: [ChannelClass; 11] = [
        ChannelClass::Lexicon,
        ChannelClass::Taiwan,
        ChannelClass::Word,
        ChannelClass::Attested,
        ChannelClass::Cns,
        ChannelClass::Unihan,
        ChannelClass::Series,
        ChannelClass::Interjection,
        ChannelClass::TaiwanWord,
        ChannelClass::Neutral,
        ChannelClass::Sandhi,
    ];

    /// The name the table, the command line and the Python reference use.
    #[must_use]
    pub fn raw_value(self) -> &'static str {
        match self {
            ChannelClass::Lexicon => "lexicon",
            ChannelClass::Taiwan => "taiwan",
            ChannelClass::Word => "word",
            ChannelClass::Attested => "attested",
            ChannelClass::Cns => "cns",
            ChannelClass::Unihan => "unihan",
            ChannelClass::Series => "series",
            ChannelClass::Interjection => "interjection",
            ChannelClass::TaiwanWord => "taiwan-word",
            ChannelClass::Neutral => "neutral",
            ChannelClass::Sandhi => "sandhi",
        }
    }

    /// The class named `raw`.
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.raw_value() == raw)
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// The cost of each class and which classes are on: engine parameters (`resources/formats/reading-channel.md`).
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelSettings {
    /// The classes in use, one bit per [`ChannelClass`] (declaration order).
    pub classes: u16,
    /// The classes whose variants are corrections; the others are accepted readings.
    pub corrections: u16,
    /// Nats added to each class's cost, by [`ChannelClass`] order.
    pub offsets: [f64; 11],
    /// Variants that cost more than this (after the offset) are left out.
    pub max_cost: f64,
    /// The most lexicon readings one typed syllable reaches through the channel.
    pub options_per_syllable: usize,
}

impl Default for ChannelSettings {
    fn default() -> Self {
        Self {
            classes: ChannelClass::ALL.iter().fold(0, |bits, c| bits | c.bit()),
            corrections: ChannelClass::Series.bit(),
            offsets: [0.0; 11],
            max_cost: 8.0,
            options_per_syllable: 4,
        }
    }
}

impl ChannelSettings {
    /// Whether the class is on.
    #[must_use]
    pub fn uses(&self, class: ChannelClass) -> bool {
        self.classes & class.bit() != 0
    }

    /// Turns a class on or off.
    pub fn set_uses(&mut self, class: ChannelClass, on: bool) {
        if on { self.classes |= class.bit() } else { self.classes &= !class.bit() }
    }

    /// Whether the class's variants are corrections.
    #[must_use]
    pub fn is_correction(&self, class: ChannelClass) -> bool {
        self.corrections & class.bit() != 0
    }

    /// Makes the class's variants corrections or accepted readings.
    pub fn set_correction(&mut self, class: ChannelClass, correction: bool) {
        if correction { self.corrections |= class.bit() } else { self.corrections &= !class.bit() }
    }

    /// The class's offset.
    #[must_use]
    pub fn offset(&self, class: ChannelClass) -> f64 {
        self.offsets[class as usize]
    }
}

/// A variant never ties with the lexicon reading, even where a source gives it the larger share.
pub const TIE: f64 = 0.01;

#[derive(Clone, Debug)]
struct CharacterRows {
    start: usize,
    count: usize,
    familiar: bool,
}

#[derive(Clone, Copy, Debug)]
struct Row {
    syllable: u32,
    class: ChannelClass,
    probability: f64,
}

#[derive(Clone, Copy, Debug)]
struct TaiwanRow {
    taiwan: u32,
    strength: f64,
    standard: bool,
}

/// `channel.bin`: P(reading | character) with each reading's class, the Taiwan readings inside words, the familiar
/// characters and the rates the costs use.
#[derive(Clone, Debug)]
pub struct ChannelTable {
    syllables: Vec<Arc<str>>,
    syllable_ids: HashMap<Arc<str>, u32>,
    characters: HashMap<char, CharacterRows>,
    rows: Vec<Row>,
    taiwan: HashMap<(char, u32), TaiwanRow>,
    /// m: the share of character positions typed with another reading of the character.
    pub misread: f64,
    /// The share of neutral-tone syllables typed with a full tone.
    pub neutral_full_tone: f64,
}

/// One character's reading in [`ChannelTable::new`]: (character, reading, class, log P(reading | character)).
pub type TableRow<'a> = (char, &'a str, ChannelClass, f64);
/// One Taiwan reading in [`ChannelTable::new`]: (character, word reading, Taiwan reading, log strength, standard).
pub type TaiwanTableRow<'a> = (char, &'a str, &'a str, f64, bool);

/// One reading variant of a character in a word.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// The syllable typed.
    pub typed: Arc<str>,
    /// Its class.
    pub class: ChannelClass,
    /// Its cost in nats, offset included.
    pub cost: f64,
}

const MAGIC: u32 = 0x4243_545a; // "ZTCB"
const HEADER: usize = 96;
const CLASS_COUNT: u32 = 8;

impl ChannelTable {
    /// A table from rows (tests and tools; the engine reads [`open`](Self::open)).  Rows of a character keep their
    /// order, which is the order the costs sum them in.
    #[must_use]
    pub fn new(
        rows: &[TableRow<'_>],
        taiwan: &[TaiwanTableRow<'_>],
        familiar: &str,
        misread: f64,
        neutral_full_tone: f64,
    ) -> Self {
        let mut table = ChannelTable {
            syllables: vec![],
            syllable_ids: HashMap::new(),
            characters: HashMap::new(),
            rows: Vec::with_capacity(rows.len()),
            taiwan: HashMap::new(),
            misread,
            neutral_full_tone,
        };
        let familiar: HashSet<char> = familiar.chars().collect();
        // Each character's rows together, characters in order of first appearance, rows in their order.
        let mut slots: HashMap<char, usize> = HashMap::new();
        let mut placed: Vec<(usize, usize, Row)> = Vec::with_capacity(rows.len());
        for (order, &(character, reading, class, logp)) in rows.iter().enumerate() {
            let syllable = table.intern(reading);
            let next = slots.len();
            let slot = *slots.entry(character).or_insert(next);
            placed.push((slot, order, Row { syllable, class, probability: logp.exp() }));
        }
        placed.sort_unstable_by_key(|&(slot, order, _)| (slot, order));
        let mut characters: Vec<char> = vec!['\0'; slots.len()];
        for (&character, &slot) in &slots {
            characters[slot] = character;
        }
        for group in placed.chunk_by(|a, b| a.0 == b.0) {
            let character = characters[group[0].0];
            table.characters.insert(
                character,
                CharacterRows { start: table.rows.len(), count: group.len(), familiar: familiar.contains(&character) },
            );
            table.rows.extend(group.iter().map(|g| g.2));
        }
        for &(character, word_reading, taiwan_reading, log_strength, standard) in taiwan {
            let word_syllable = table.intern(word_reading);
            let taiwan_syllable = table.intern(taiwan_reading);
            table.taiwan.insert(
                (character, word_syllable),
                TaiwanRow { taiwan: taiwan_syllable, strength: log_strength.exp(), standard },
            );
        }
        table
    }

    fn intern(&mut self, syllable: &str) -> u32 {
        if let Some(&id) = self.syllable_ids.get(syllable) {
            return id;
        }
        let id = u32::try_from(self.syllables.len()).unwrap_or(u32::MAX);
        let shared: Arc<str> = syllable.into();
        self.syllables.push(shared.clone());
        self.syllable_ids.insert(shared, id);
        id
    }

    /// Reads a ZTCB/1 file (`resources/formats/reading-channel.md`), checking every count, offset, reference and
    /// score.
    ///
    /// # Errors
    /// [`DataError::Io`] when the file cannot be read, [`DataError::InvalidChannel`] when a check fails.
    pub fn open(path: &Path) -> Result<Self, DataError> {
        let data = std::fs::read(path)?;
        Self::parse(&data).ok_or(DataError::InvalidChannel)
    }

    fn parse(data: &[u8]) -> Option<Self> {
        let u32_at =
            |offset: usize| -> Option<u32> { Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?)) };
        let f64_at =
            |offset: usize| -> Option<f64> { Some(f64::from_le_bytes(data.get(offset..offset + 8)?.try_into().ok()?)) };
        let usize_at = |offset: usize| -> Option<usize> {
            usize::try_from(u64::from_le_bytes(data.get(offset..offset + 8)?.try_into().ok()?)).ok()
        };
        if data.len() < HEADER || u32_at(0)? != MAGIC || u32_at(4)? != 1 {
            return None;
        }
        let [syllable_count, character_count, row_count, taiwan_count] =
            [u32_at(8)?, u32_at(12)?, u32_at(16)?, u32_at(20)?].map(|n| n as usize);
        let (misread, neutral) = (f64_at(32)?, f64_at(40)?);
        let sections: Vec<usize> = (0..6).map(|i| usize_at(48 + i * 8)).collect::<Option<_>>()?;
        let lengths = [syllable_count * 8, character_count * 16, row_count * 16, taiwan_count * 24];
        if sections[5] != data.len()
            || sections[0] != HEADER
            || !sections.windows(2).all(|w| w[0] <= w[1])
            || !(0..4).all(|i| sections[i] + lengths[i] <= sections[i + 1])
            || !(misread > 0.0 && misread < 1.0 && neutral > 0.0 && neutral < 1.0)
        {
            return None;
        }
        let blob = &data[sections[4]..];
        let mut syllables: Vec<String> = Vec::with_capacity(syllable_count);
        for i in 0..syllable_count {
            let (start, length) = (u32_at(sections[0] + i * 8)? as usize, u32_at(sections[0] + i * 8 + 4)? as usize);
            let text = std::str::from_utf8(blob.get(start..start.checked_add(length)?)?).ok()?;
            if text.is_empty() || syllables.last().is_some_and(|last| last.as_bytes() >= text.as_bytes()) {
                return None;
            }
            syllables.push(text.to_owned());
        }
        let mut rows: Vec<TableRow<'_>> = Vec::with_capacity(row_count);
        let mut familiar = String::new();
        let mut covered = 0;
        let mut previous: Option<char> = None;
        for i in 0..character_count {
            let p = sections[1] + i * 16;
            let character = char::from_u32(u32_at(p)?)?;
            let (start, count, flag) = (u32_at(p + 4)? as usize, u32_at(p + 8)? as usize, u32_at(p + 12)?);
            if previous.is_some_and(|c| c >= character) || start != covered || count == 0 || flag > 1 {
                return None;
            }
            previous = Some(character);
            covered = start.checked_add(count)?;
            if covered > row_count {
                return None;
            }
            if flag == 1 {
                familiar.push(character);
            }
            for r in start..covered {
                let q = sections[2] + r * 16;
                let (logp, syllable, class) = (f64_at(q)?, u32_at(q + 8)? as usize, u32_at(q + 12)?);
                if !logp.is_finite() || logp > 1e-9 || syllable >= syllable_count || class >= CLASS_COUNT {
                    return None;
                }
                rows.push((character, &syllables[syllable], ChannelClass::ALL[class as usize], logp));
            }
        }
        if covered != row_count {
            return None;
        }
        let mut taiwan: Vec<TaiwanTableRow<'_>> = Vec::with_capacity(taiwan_count);
        for i in 0..taiwan_count {
            let p = sections[3] + i * 24;
            let character = char::from_u32(u32_at(p)?)?;
            let (word, standard_reading, standard) =
                (u32_at(p + 4)? as usize, u32_at(p + 8)? as usize, u32_at(p + 12)?);
            let strength = f64_at(p + 16)?;
            if word >= syllable_count
                || standard_reading >= syllable_count
                || standard > 1
                || !strength.is_finite()
                || strength > 1e-9
            {
                return None;
            }
            taiwan.push((character, &syllables[word], &syllables[standard_reading], strength, standard == 1));
        }
        Some(Self::new(&rows, &taiwan, &familiar, misread, neutral))
    }

    /// Number of (character, reading) rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn rows_of(&self, character: char) -> (&[Row], bool) {
        match self.characters.get(&character) {
            Some(c) => (&self.rows[c.start..c.start + c.count], c.familiar),
            None => (&[], false),
        }
    }

    /// The table's own copy of a syllable, or a new one.
    fn shared(&self, syllable: &str) -> Arc<str> {
        self.syllable_ids.get_key_value(syllable).map_or_else(|| Arc::from(syllable), |(shared, _)| shared.clone())
    }

    fn id(&self, syllable: &str) -> Option<u32> {
        self.syllable_ids.get(syllable).copied()
    }

    /// The readings a typist may enter for `character` where a word reads it `syllable` (`single`: the word is the
    /// character alone), with their classes and costs (`resources/formats/reading-channel.md`, 引擎該用的成本):
    ///
    /// - `taiwan.tsv` in a longer word: the Taiwan reading, at 0 when it is the standard, else −log strength;
    /// - a neutral tone in a longer word: another reading of the character with another syllable, at
    ///   −log neutral − log P(r | the character's non-neutral readings with another syllable);
    /// - a single character: each other reading at −log (P(r) / P(the lexicon's reading)), at least [`TIE`];
    /// - an unfamiliar character in a longer word: each reading with another syllable, at
    ///   −log m − log (P(r) / (1 − P(the word's reading))).
    ///
    /// The first class to give a reading keeps it.  Classes that are off are skipped; offsets are added and variants
    /// above `max_cost` dropped.  Sums run in the table's row order, as in the Python reference
    /// (`zhuyintype_ml.training.lexicon.engine_channel`).
    #[must_use]
    pub fn variants(&self, character: char, syllable: &str, single: bool, settings: &ChannelSettings) -> Vec<Variant> {
        let mut result = Vec::new();
        self.variants_into(character, syllable, single, settings, &mut result);
        result
    }

    /// [`variants`](Self::variants) appended to `result`, allocating nothing for a character without rows.
    pub(crate) fn variants_into(
        &self,
        character: char,
        syllable: &str,
        single: bool,
        settings: &ChannelSettings,
        result: &mut Vec<Variant>,
    ) {
        let (rows, familiar) = self.rows_of(character);
        let id = self.id(syllable);
        let taiwan = if single { None } else { id.and_then(|id| self.taiwan.get(&(character, id))) };
        if rows.is_empty() && taiwan.is_none() {
            return;
        }
        let mut found: Vec<(u32, ChannelClass, f64)> = vec![];
        let add = |found: &mut Vec<(u32, ChannelClass, f64)>, typed: u32, class: ChannelClass, cost: f64| {
            if settings.uses(class) && !found.iter().any(|f| f.0 == typed) {
                found.push((typed, class, cost));
            }
        };
        if let Some(row) = taiwan {
            add(&mut found, row.taiwan, ChannelClass::TaiwanWord, if row.standard { 0.0 } else { -row.strength.ln() });
        }
        let base = without_tones(syllable);
        if syllable.ends_with('˙') {
            if !single {
                let others: Vec<&Row> = rows
                    .iter()
                    .filter(|r| {
                        let reading = &self.syllables[r.syllable as usize];
                        !reading.ends_with('˙') && without_tones(reading) != base
                    })
                    .collect();
                let total: f64 = others.iter().map(|r| r.probability).sum();
                for row in others {
                    let cost = -self.neutral_full_tone.ln() - (row.probability / total).ln();
                    add(&mut found, row.syllable, ChannelClass::Neutral, cost);
                }
            }
        } else if single {
            if let Some(top) = rows.iter().find(|r| Some(r.syllable) == id).map(|r| r.probability) {
                for row in rows.iter().filter(|r| Some(r.syllable) != id) {
                    add(&mut found, row.syllable, row.class, TIE.max(-(row.probability / top).ln()));
                }
            }
        } else if !familiar {
            let rest = 1.0 - rows.iter().find(|r| Some(r.syllable) == id).map_or(0.0, |r| r.probability);
            if rest > 0.0 {
                for row in rows {
                    let reading = &self.syllables[row.syllable as usize];
                    if Some(row.syllable) != id && row.probability > 0.0 && without_tones(reading) != base {
                        let cost = -self.misread.ln() - (row.probability / rest).ln();
                        add(&mut found, row.syllable, row.class, cost);
                    }
                }
            }
        }
        result.extend(
            found
                .into_iter()
                .map(|(typed, class, cost)| (typed, class, cost + settings.offset(class)))
                .filter(|&(_, _, cost)| cost <= settings.max_cost)
                .map(|(typed, class, cost)| Variant { typed: self.syllables[typed as usize].clone(), class, cost }),
        );
    }

    /// The cost of third-tone sandhi, before the class offset: −log m.
    #[must_use]
    pub fn sandhi_cost(&self) -> f64 {
        -self.misread.ln()
    }
}

/// The variant one word took, once matched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelUse {
    /// The syllable's offset in the word.
    pub position: usize,
    /// The variant's class.
    pub class: ChannelClass,
    /// Its cost.
    pub cost: f64,
    /// A correction (annotated, an edit, the reranker's reading margin), not an accepted reading.
    pub correction: bool,
}

/// (character, single-character word, class, cost).
type OptionCharacter = (char, bool, ChannelClass, f64);

/// What typing one syllable reaches through the channel: a lexicon syllable and, per character, the variant's class
/// and cost (single-character words and longer words cost differently).
#[derive(Clone, Debug)]
pub struct ChannelOption {
    /// The lexicon syllable.
    pub reading: Arc<str>,
    /// Corrections and accepted readings are separate options.
    pub correction: bool,
    /// (character, single-character word, class, cost), sorted by character then `single`.
    characters: Vec<OptionCharacter>,
    /// The best score a word reaches through it: log P(word) + log P(reading | word) − cost.
    rank: f64,
}

impl ChannelOption {
    /// The class and cost of `character` in a word of one character (`single`) or more.
    #[must_use]
    pub fn character(&self, character: char, single: bool) -> Option<(ChannelClass, f64)> {
        self.characters
            .binary_search_by(|probe| (probe.0, probe.1).cmp(&(character, single)))
            .ok()
            .map(|i| (self.characters[i].2, self.characters[i].3))
    }
}

/// A lexicon word with a character that has variants at `position`; the word is a span of the index's text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct IndexedWord {
    word: (u32, u32),
    logp: f64,
    reading_prior: f64,
    /// The word's ID; `u32::MAX` for none.
    word_id: u32,
    pub character: char,
    position: u8,
    pub single: bool,
}

/// One variant of one lexicon row, as [`ChannelIndex::table`] lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantRow {
    /// The lexicon reading.
    pub reading: String,
    /// The word.
    pub word: String,
    /// The syllable's position in the word.
    pub position: usize,
    /// The variant.
    pub variant: Variant,
    /// A correction.
    pub correction: bool,
}

/// The variants of every lexicon word, indexed for matching.
#[derive(Debug, Default)]
pub struct ChannelIndex {
    pub(crate) settings: ChannelSettings,
    sandhi_cost: f64,
    options: HashMap<Arc<str>, Vec<ChannelOption>>,
    /// The indexed words' words and readings.
    text: String,
    /// Every indexed word, sorted by reading, then by position, character and word.
    words: Vec<IndexedWord>,
    /// Each indexed reading (a span of `text`) with its first word and count in `words`, sorted by reading.
    readings: Vec<((u32, u32), u32, u32)>,
    /// (word index, syllable): syllables at the word's variant position that it has its own lexicon rows for, sorted.
    excluded: Vec<(u32, Arc<str>)>,
    /// Hashes of the toned prefixes of the indexed readings, from their variant position on (a collision only lets a
    /// path go one lookup further).
    prefixes: HashSet<u64>,
}

fn prefix_hash(reading: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    reading.hash(&mut hasher);
    hasher.finish()
}

/// One lexicon row, borrowed.
pub(crate) struct RowRef<'a> {
    pub reading: &'a str,
    pub word: &'a str,
    pub logp: f64,
    pub reading_prior: f64,
    pub word_id: Option<u32>,
}

/// The lexicon as the index reads it: every row, each word's best reading and its readings.
pub(crate) trait LexiconRows {
    /// Calls `each` with every row.
    fn rows(&self, each: &mut dyn FnMut(&RowRef<'_>));
    /// Whether `reading` is the word's best reading.
    fn is_best(&self, word: &str, reading: &str) -> bool;
    /// Calls `each` with every reading of the word.
    fn readings(&self, word: &str, each: &mut dyn FnMut(&str));
}

/// Up to this many syllables per word are indexed (longer words are idioms and names; they keep their rows).
const LONGEST: usize = 8;

impl ChannelIndex {
    /// Calls `each` for every variant of every aligned lexicon row (single characters: only their best row), after
    /// the rows that win over it are removed.  Rows without variants allocate nothing.
    fn for_each_variant(
        table: &ChannelTable,
        settings: &ChannelSettings,
        lexicon: &dyn LexiconRows,
        mut each: impl FnMut(&RowRef<'_>, usize, char, bool, &[Variant], Box<[Arc<str>]>),
    ) {
        let mut variants: Vec<Variant> = Vec::new();
        lexicon.rows(&mut |row| {
            let mut syllables = [""; LONGEST];
            let mut characters = ['\0'; LONGEST];
            let mut count = 0;
            let mut chars = row.word.chars();
            for syllable in row.reading.split(' ').filter(|p| !p.is_empty()) {
                let Some(character) = chars.next() else { return };
                if count == LONGEST || !is_han(character.encode_utf8(&mut [0; 4])) {
                    return;
                }
                syllables[count] = syllable;
                characters[count] = character;
                count += 1;
            }
            if count == 0 || chars.next().is_some() {
                return;
            }
            let single = count == 1;
            if single && !lexicon.is_best(row.word, row.reading) {
                return;
            }
            let (syllables, characters) = (&syllables[..count], &characters[..count]);
            for position in 0..count {
                variants.clear();
                table.variants_into(characters[position], syllables[position], single, settings, &mut variants);
                if variants.is_empty() {
                    continue;
                }
                // A row of the word that differs only here: the lexicon has that reading itself.
                let mut excluded: Vec<Arc<str>> = Vec::new();
                lexicon.readings(row.word, &mut |other| {
                    let mut parts = other.split(' ').filter(|p| !p.is_empty());
                    let mut differs: Option<&str> = None;
                    for (i, &syllable) in syllables.iter().enumerate() {
                        match parts.next() {
                            Some(part) if part == syllable => {}
                            Some(part) if i == position => differs = Some(part),
                            _ => return,
                        }
                    }
                    if let Some(part) = differs.filter(|_| parts.next().is_none()) {
                        excluded.push(table.shared(part));
                    }
                });
                variants.retain(|v| !excluded.contains(&v.typed));
                if !variants.is_empty() {
                    each(row, position, characters[position], single, &variants, excluded.into_boxed_slice());
                }
            }
        });
    }

    /// Builds the index over the lexicon's rows.  Everything kept lives in a few large allocations (the text, the
    /// words, the reading table and the prefix hashes): an engine builds it once at load, and small allocations
    /// mixed with the scan's temporary ones would leave the heap fragmented.
    // One pass over the lexicon, then the options grouped and the words sorted.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn build(table: &ChannelTable, settings: &ChannelSettings, lexicon: &dyn LexiconRows) -> Self {
        struct OptionRow {
            typed: u32,
            lexicon: u32,
            correction: bool,
            character: char,
            single: bool,
            class: ChannelClass,
            cost: f64,
            score: f64,
        }
        let mut rows: Vec<OptionRow> = Vec::new();
        // (reading span, word); the word's index in `found` keys its exclusions.
        let mut found: Vec<((u32, u32), IndexedWord)> = Vec::new();
        let mut excluded_found: Vec<(u32, Arc<str>)> = Vec::new();
        let mut lexicon_syllables: Vec<Arc<str>> = Vec::new();
        let mut lexicon_ids: HashMap<Arc<str>, u32> = HashMap::new();
        let mut text = String::new();
        let mut prefixes: HashSet<u64> = HashSet::new();
        let mut prefix = String::new();
        let mut last_reading: Option<(u32, u32)> = None;
        let span = |text: &mut String, value: &str| -> (u32, u32) {
            let start = u32::try_from(text.len()).unwrap_or(u32::MAX);
            text.push_str(value);
            (start, u32::try_from(value.len()).unwrap_or(u32::MAX))
        };
        Self::for_each_variant(table, settings, lexicon, |row, position, character, single, variants, excluded| {
            let lexicon_syllable = row.reading.split(' ').filter(|p| !p.is_empty()).nth(position).unwrap_or_default();
            let lexicon_id = if let Some(&id) = lexicon_ids.get(lexicon_syllable) {
                id
            } else {
                let id = u32::try_from(lexicon_syllables.len()).unwrap_or(u32::MAX);
                let shared = table.shared(lexicon_syllable);
                lexicon_syllables.push(shared.clone());
                lexicon_ids.insert(shared, id);
                id
            };
            let score = row.logp + row.reading_prior;
            for variant in variants {
                rows.push(OptionRow {
                    typed: table.id(&variant.typed).unwrap_or(u32::MAX),
                    lexicon: lexicon_id,
                    correction: settings.is_correction(variant.class),
                    character,
                    single,
                    class: variant.class,
                    cost: variant.cost,
                    score: score - variant.cost,
                });
            }
            // Rows of one reading come together from both lexicons: keep its text once.
            let reading = match last_reading {
                Some(last) if text.get(last.0 as usize..(last.0 + last.1) as usize) == Some(row.reading) => last,
                _ => span(&mut text, row.reading),
            };
            last_reading = Some(reading);
            let index = u32::try_from(found.len()).unwrap_or(u32::MAX);
            excluded_found.extend(excluded.iter().map(|x| (index, x.clone())));
            let word = span(&mut text, row.word);
            found.push((
                reading,
                IndexedWord {
                    word,
                    logp: row.logp,
                    reading_prior: row.reading_prior,
                    word_id: row.word_id.unwrap_or(u32::MAX),
                    character,
                    position: u8::try_from(position).unwrap_or(u8::MAX),
                    single,
                },
            ));
            prefix.clear();
            for (i, syllable) in row.reading.split(' ').filter(|p| !p.is_empty()).enumerate() {
                if i > 0 {
                    prefix.push(' ');
                }
                prefix.push_str(syllable);
                if i >= position {
                    prefixes.insert(prefix_hash(&prefix));
                }
            }
        });
        drop(lexicon_ids);
        // Options: group by (typed, lexicon syllable, state), one entry per (character, single), the best score.
        rows.sort_unstable_by(|a, b| {
            (a.typed, &*lexicon_syllables[a.lexicon as usize], a.correction, a.character, a.single).cmp(&(
                b.typed,
                &*lexicon_syllables[b.lexicon as usize],
                b.correction,
                b.character,
                b.single,
            ))
        });
        let mut grouped: Vec<(u32, ChannelOption)> = Vec::new();
        for group in rows.chunk_by(|a, b| (a.typed, a.lexicon, a.correction) == (b.typed, b.lexicon, b.correction)) {
            let mut characters: Vec<OptionCharacter> = Vec::new();
            let mut rank = f64::NEG_INFINITY;
            for row in group {
                if characters.last().is_none_or(|c| (c.0, c.1) != (row.character, row.single)) {
                    characters.push((row.character, row.single, row.class, row.cost));
                }
                rank = rank.max(row.score);
            }
            let first = &group[0];
            let reading = lexicon_syllables[first.lexicon as usize].clone();
            grouped.push((first.typed, ChannelOption { reading, correction: first.correction, characters, rank }));
        }
        drop(rows);
        let mut options: HashMap<Arc<str>, Vec<ChannelOption>> = HashMap::new();
        for group in grouped.chunk_by_mut(|a, b| a.0 == b.0) {
            group.sort_by(|(_, a), (_, b)| {
                b.rank
                    .total_cmp(&a.rank)
                    .then_with(|| a.reading.as_bytes().cmp(b.reading.as_bytes()))
                    .then_with(|| a.correction.cmp(&b.correction))
            });
            let kept: Vec<ChannelOption> =
                group.iter().take(settings.options_per_syllable).map(|(_, option)| option.clone()).collect();
            options.insert(table.syllables[group[0].0 as usize].clone(), kept);
        }
        drop(grouped);
        let at = |span: (u32, u32)| &text[span.0 as usize..(span.0 + span.1) as usize];
        let mut order: Vec<u32> = (0..u32::try_from(found.len()).unwrap_or(u32::MAX)).collect();
        order.sort_unstable_by(|&a, &b| {
            let ((ra, a), (rb, b)) = (&found[a as usize], &found[b as usize]);
            (at(*ra).as_bytes(), a.position, a.character, at(a.word)).cmp(&(
                at(*rb).as_bytes(),
                b.position,
                b.character,
                at(b.word),
            ))
        });
        let mut new_index: Vec<u32> = vec![0; order.len()];
        let mut words: Vec<IndexedWord> = Vec::with_capacity(order.len());
        let mut readings: Vec<((u32, u32), u32, u32)> = Vec::new();
        for &old in &order {
            let (reading, word) = found[old as usize];
            new_index[old as usize] = u32::try_from(words.len()).unwrap_or(u32::MAX);
            match readings.last_mut() {
                Some(last) if at(last.0) == at(reading) => last.2 += 1,
                _ => readings.push((reading, u32::try_from(words.len()).unwrap_or(u32::MAX), 1)),
            }
            words.push(word);
        }
        drop(found);
        drop(order);
        let mut excluded: Vec<(u32, Arc<str>)> =
            excluded_found.into_iter().map(|(old, syllable)| (new_index[old as usize], syllable)).collect();
        excluded.sort_unstable_by(|a, b| (a.0, a.1.as_bytes()).cmp(&(b.0, b.1.as_bytes())));
        readings.shrink_to_fit();
        prefixes.shrink_to_fit();
        text.shrink_to_fit();
        let sandhi_cost = table.sandhi_cost() + settings.offset(ChannelClass::Sandhi);
        ChannelIndex { settings: settings.clone(), sandhi_cost, options, text, words, readings, excluded, prefixes }
    }

    fn text_of(&self, span: (u32, u32)) -> &str {
        &self.text[span.0 as usize..(span.0 + span.1) as usize]
    }

    /// The indexed words read `reading` with a variant at `position`, as lexicon entries, with each word's
    /// character, whether it is a single character, and the syllables it has its own rows for.
    pub(crate) fn entries(&self, reading: &str, position: usize) -> Vec<(WordEntry, char, bool, Vec<&str>)> {
        let Ok(found) = self.readings.binary_search_by(|r| self.text_of(r.0).as_bytes().cmp(reading.as_bytes())) else {
            return vec![];
        };
        let (_, start, count) = self.readings[found];
        let shared: Arc<str> = reading.into();
        (start..start + count)
            .filter(|&i| usize::from(self.words[i as usize].position) == position)
            .map(|i| {
                let w = &self.words[i as usize];
                let mut entry = WordEntry::new(self.text_of(w.word), w.logp)
                    .with_reading_prior(w.reading_prior)
                    .with_word_id((w.word_id != u32::MAX).then_some(w.word_id));
                entry.reading = Some(shared.clone());
                let first = self.excluded.partition_point(|e| e.0 < i);
                let excluded: Vec<&str> =
                    self.excluded[first..].iter().take_while(|e| e.0 == i).map(|e| &*e.1).collect();
                (entry, w.character, w.single, excluded)
            })
            .collect()
    }

    /// Every variant of every lexicon row (the table the Python reference checks), sorted.
    pub(crate) fn table(
        table: &ChannelTable,
        settings: &ChannelSettings,
        lexicon: &dyn LexiconRows,
    ) -> Vec<VariantRow> {
        let mut result = vec![];
        Self::for_each_variant(table, settings, lexicon, |row, position, _, _, variants, _| {
            for variant in variants {
                result.push(VariantRow {
                    reading: row.reading.to_owned(),
                    word: row.word.to_owned(),
                    position,
                    correction: settings.is_correction(variant.class),
                    variant: variant.clone(),
                });
            }
        });
        result.sort_by(|a, b| {
            (&a.reading, &a.word, a.position, &*a.variant.typed).cmp(&(
                &b.reading,
                &b.word,
                b.position,
                &*b.variant.typed,
            ))
        });
        result
    }

    /// The options typing `syllable` reaches.
    #[must_use]
    pub fn options(&self, syllable: &str) -> &[ChannelOption] {
        self.options.get(syllable).map_or(&[], Vec::as_slice)
    }

    /// Whether some indexed reading starts with `reading` (toned) from its variant position on.
    pub(crate) fn has_prefix(&self, reading: &str) -> bool {
        self.prefixes.contains(&prefix_hash(reading))
    }

    /// The sandhi syllable for `typed` before `next`: a second and a third tone swap before a third tone.
    #[must_use]
    pub fn sandhi(&self, typed: &str, next: Option<&str>) -> Option<String> {
        if !self.settings.uses(ChannelClass::Sandhi) || !next.is_some_and(|n| n.ends_with('ˇ')) {
            return None;
        }
        let base = typed.strip_suffix('ˊ').map(|b| (b, 'ˇ')).or_else(|| typed.strip_suffix('ˇ').map(|b| (b, 'ˊ')))?;
        (self.sandhi_cost <= self.settings.max_cost).then(|| format!("{}{}", base.0, base.1))
    }

    /// The sandhi cost, offset included.
    #[must_use]
    pub fn sandhi_cost(&self) -> f64 {
        self.sandhi_cost
    }

    /// Whether sandhi variants are corrections.
    #[must_use]
    pub fn sandhi_is_correction(&self) -> bool {
        self.settings.is_correction(ChannelClass::Sandhi)
    }
}

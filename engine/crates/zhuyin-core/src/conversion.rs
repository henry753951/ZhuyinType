//! Lattice decoding (port of `Conversion.swift`): Viterbi over the composition's word lattice with the language
//! model's history as state, N-best sentences, and the per-composer cache that lets typing at the end reuse the
//! decoded prefix.
//!
//! Path nodes live in an arena owned by the cache (indices instead of Swift's class references), so a composer and
//! its cache can move between threads.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use crate::api::{ScoredSentence, SentenceWord, TypoTolerance};
use crate::engine::{DECODING_BEAM_WIDTH, Engine, EngineModel};
use crate::language_model::{LanguageModel, LanguageModelHistory};
use crate::lexicon::WordEntry;
use crate::swift_format::graphemes;

/// One token of a composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompositionToken {
    /// A syllable: its reading, the keys behind it, and whether it ended without a tone key or Space.
    Syllable {
        /// The reading.
        reading: Arc<str>,
        /// The keys typed for it.
        keys: Arc<str>,
        /// Ended before the next initial without a tone key or Space (first tone).
        implicit: bool,
    },
    /// Punctuation or other literal text.
    Literal(Arc<str>),
}

impl CompositionToken {
    /// A syllable token.
    #[must_use]
    pub fn syllable(reading: &str, keys: &str, implicit: bool) -> Self {
        CompositionToken::Syllable { reading: reading.into(), keys: keys.into(), implicit }
    }

    /// The reading of a syllable.
    #[must_use]
    pub fn reading(&self) -> Option<&str> {
        match self {
            CompositionToken::Syllable { reading, .. } => Some(reading),
            CompositionToken::Literal(_) => None,
        }
    }
}

/// One word of a conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertedWord {
    /// Composition tokens it covers.
    pub range: Range<usize>,
    /// The word.
    pub text: Arc<str>,
    /// Its reading as typed; empty for a literal.
    pub reading: Arc<str>,
    /// Its language-model ID.
    pub word_id: Option<u32>,
    /// The language-model history after it.
    pub history: Option<LanguageModelHistory>,
    /// Matched through a typing correction or a fuzzy sound, not the keys as typed.
    pub corrected: bool,
}

/// A word the user picked or that stays as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct FixedWord {
    /// Composition tokens it covers.
    pub range: Range<usize>,
    /// The word.
    pub text: Arc<str>,
    /// Its reading.
    pub reading: Arc<str>,
    /// Its language-model ID.
    pub word_id: Option<u32>,
    /// The candidate's annotation.
    pub annotation: Option<Arc<str>>,
    /// Internal words of a fixed phrase, kept for the language-model history though shown as one clause.
    pub components: Vec<ConvertedWord>,
    /// Kept as shown because the user picked another word in the composition (not a pick of its own).
    pub frozen: bool,
}

/// Per syllable position, log P(character) from a character model, with the weight they enter the lattice at.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterScores {
    /// Scores by character, per position.
    pub positions: Vec<HashMap<String, f64>>,
    /// The weight.
    pub weight: f64,
    /// The score of a character the model was not asked about.
    pub floor: f64,
}

impl CharacterScores {
    /// Scores with the default floor (−12).
    #[must_use]
    pub fn new(positions: Vec<HashMap<String, f64>>, weight: f64) -> Self {
        Self { positions, weight, floor: -12.0 }
    }

    /// The weighted sum over a word's characters, which start at token `start`.
    #[must_use]
    pub fn score(&self, text: &str, start: usize) -> f64 {
        let mut total = 0.0;
        for (offset, character) in graphemes(text).into_iter().enumerate() {
            let Some(position) = self.positions.get(start + offset).filter(|p| !p.is_empty()) else { continue };
            total += position.get(character).copied().unwrap_or(self.floor);
        }
        self.weight * total
    }
}

#[derive(Clone)]
struct PathNode {
    score: f64,
    word: Option<Arc<ConvertedWord>>,
    previous: Option<u32>,
    history: LanguageModelHistory,
    beam_score: f64,
    /// Whether the word is a real word (a non-empty reading); its text then is the last word, else `<s>`.
    has_last_word: bool,
    text: String,
}

impl PathNode {
    fn last_word(&self) -> &str {
        match &self.word {
            Some(word) if self.has_last_word => &word.text,
            _ => "<s>",
        }
    }

    fn precedes(&self, other: &PathNode) -> bool {
        if self.beam_score != other.beam_score {
            return self.beam_score < other.beam_score;
        }
        if self.last_word() != other.last_word() {
            return self.last_word() > other.last_word();
        }
        self.history.older.unwrap_or(0) > other.history.older.unwrap_or(0)
    }
}

struct Component {
    entry: WordEntry,
    unigram: f64,
    adjustment: f64,
}

struct DecodingEdge {
    /// The word with the ID nodes give it (the last component's or the entry's).
    word: Arc<ConvertedWord>,
    entry: Option<WordEntry>,
    values: Option<(f64, f64)>,
    components: Vec<Component>,
    /// The character model's weighted score for this word here (0 without one).
    characters: f64,
    transitions: HashMap<LanguageModelHistory, (f64, LanguageModelHistory)>,
}

impl DecodingEdge {
    fn transition(&mut self, history: LanguageModelHistory, model: &EngineModel) -> (f64, LanguageModelHistory) {
        if let Some(&cached) = self.transitions.get(&history) {
            return cached;
        }
        let mut value = match (self.values, &self.entry) {
            (Some((unigram, adjustment)), entry) => {
                let recalled = entry.as_ref().is_some_and(|e| e.recalled);
                (if recalled { 0.0 } else { model.score(entry.as_ref().and_then(|e| e.word_id), history, unigram) })
                    + adjustment
            }
            (None, _) => model.end(history),
        };
        let mut next = match &self.entry {
            None => LanguageModelHistory::default(),
            Some(entry) => history.appending(entry.word_id, model.order()),
        };
        if !self.components.is_empty() {
            value = 0.0;
            next = history;
            for part in &self.components {
                value += (if part.entry.recalled { 0.0 } else { model.score(part.entry.word_id, next, part.unigram) })
                    + part.adjustment;
                next = next.appending(part.entry.word_id, model.order());
            }
        }
        value += self.characters;
        self.transitions.insert(history, (value, next));
        (value, next)
    }
}

struct DecodingPosition {
    edges: Vec<DecodingEdge>,
    best: HashMap<LanguageModelHistory, Vec<usize>>,
}

#[derive(Clone, Copy)]
struct Ranked {
    index: usize,
    rank: f64,
    history: LanguageModelHistory,
}

impl DecodingPosition {
    /// The edges worth expanding after `history`: per word length, the beam's best by score (with the end score at
    /// the composition end), one per resulting history.
    fn expansions(&mut self, history: LanguageModelHistory, model: &EngineModel, end: usize) -> Vec<usize> {
        if let Some(cached) = self.best.get(&history) {
            return cached.clone();
        }
        let mut groups: [Vec<Ranked>; 7] = Default::default();
        let edges = &mut self.edges;
        let better = |edges: &[DecodingEdge], a: &Ranked, b: &Ranked| -> bool {
            if a.rank != b.rank {
                return a.rank > b.rank;
            }
            let (a_word, b_word) = (&edges[a.index].word.text, &edges[b.index].word.text);
            if a_word != b_word {
                return a_word < b_word;
            }
            a.history.older.unwrap_or(0) < b.history.older.unwrap_or(0)
        };
        for index in 0..edges.len() {
            let (value, next) = edges[index].transition(history, model);
            let target = edges[index].word.range.end;
            let rank = value + if target == end { model.end(next) } else { 0.0 };
            let length = edges[index].word.range.len();
            let candidate = Ranked { index, rank, history: next };
            let group = &mut groups[length];
            if group.len() == DECODING_BEAM_WIDTH
                && let Some(worst) = group.last()
                && better(edges, worst, &candidate)
            {
                continue;
            }
            if let Some(old) = group.iter().position(|r| r.history == next) {
                if rank.partial_cmp(&group[old].rank) != Some(std::cmp::Ordering::Greater) {
                    continue;
                }
                group.remove(old);
            }
            let insertion = group.iter().position(|r| better(edges, &candidate, r)).unwrap_or(group.len());
            group.insert(insertion, candidate);
            if group.len() > DECODING_BEAM_WIDTH {
                group.pop();
            }
        }
        let mut indices: Vec<usize> = groups.iter().flatten().map(|r| r.index).collect();
        indices.sort_unstable();
        self.best.insert(history, indices.clone());
        indices
    }
}

/// What a composer keeps between conversions so typing at the end reuses the decoded prefix.
pub struct ConversionCache {
    tokens: Vec<CompositionToken>,
    fixed: Vec<FixedWord>,
    typo: TypoTolerance,
    revision: u64,
    group: Option<Range<usize>>,
    start: LanguageModelHistory,
    nodes: Vec<PathNode>,
    states: Vec<Vec<u32>>,
    edges: Vec<Option<DecodingPosition>>,
}

/// The inputs of one decode.
pub struct Request<'a> {
    /// The composition.
    pub tokens: &'a [CompositionToken],
    /// Words that stay as they are.
    pub fixed: &'a [FixedWord],
    /// Typing-correction strength.
    pub typo: TypoTolerance,
    /// Words may not cross these token boundaries.
    pub group: Option<Range<usize>>,
    /// The language-model history before the first token.
    pub start: LanguageModelHistory,
    /// Character-model scores.
    pub characters: Option<&'a CharacterScores>,
}

impl<'a> Request<'a> {
    /// A request with no group, the sentence start and no character scores.
    #[must_use]
    pub fn new(tokens: &'a [CompositionToken], fixed: &'a [FixedWord], typo: TypoTolerance) -> Self {
        Self { tokens, fixed, typo, group: None, start: LanguageModelHistory::default(), characters: None }
    }
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    !a.is_empty() && !b.is_empty() && a.start < b.end && b.start < a.end
}

impl Engine {
    /// The best conversion.  `cache` carries the decoded prefix from the composer's previous conversion.
    pub fn convert(&self, request: &Request<'_>, cache: &mut Option<ConversionCache>) -> Vec<ConvertedWord> {
        let ends = self.decode(request, cache, 1);
        let Some(cache) = cache.as_ref() else { return vec![] };
        ends.first().map(|&end| path(&cache.nodes, end)).unwrap_or_default()
    }

    /// The `k` best distinct sentences.
    pub fn sentences(&self, request: &Request<'_>, k: usize) -> Vec<ScoredSentence> {
        if k == 0 {
            return vec![];
        }
        let mut cache = None;
        let ends = self.decode(request, &mut cache, k);
        let Some(cache) = cache else { return vec![] };
        let mut seen = std::collections::HashSet::new();
        ends.iter()
            .filter_map(|&end| {
                let words = path(&cache.nodes, end);
                let text: String = words.iter().map(|w| &*w.text).collect();
                if !seen.insert(text) {
                    return None;
                }
                Some(ScoredSentence {
                    words: words
                        .iter()
                        .map(|w| SentenceWord {
                            range: w.range.clone(),
                            text: w.text.to_string(),
                            reading: w.reading.to_string(),
                            corrected: w.corrected,
                        })
                        .collect(),
                    score: cache.nodes[end as usize].beam_score,
                })
            })
            .take(k)
            .collect()
    }

    /// Decodes into a new cache and returns the final beam (node indices).
    // One function, as `decode` in Conversion.swift, so the two can be compared line by line.
    #[allow(clippy::too_many_lines)]
    fn decode(&self, request: &Request<'_>, cache: &mut Option<ConversionCache>, k: usize) -> Vec<u32> {
        let tokens = request.tokens;
        if tokens.is_empty() {
            return vec![];
        }
        // Character scores depend on every syllable (the model reads both directions): nothing carries over.
        if request.characters.is_some() {
            *cache = None;
        }
        let distinct = k > 1;
        let width = DECODING_BEAM_WIDTH.max(k);
        let revision = self.conversion_revision();
        let model = self.language_model();
        let fixed = request.fixed;
        let group = request.group.clone();
        let mut reused = 0;
        if !distinct
            && let Some(old) = cache.as_ref()
            && old.fixed == fixed
            && old.typo == request.typo
            && old.revision == revision
            && old.start == request.start
            && (old.group == group || (old.group.is_none() && group.is_some()))
        {
            let common = old.tokens.iter().zip(tokens).take_while(|(a, b)| a == b).count();
            // Final-position beams include end scores; only reuse strictly interior positions.
            reused = common.min(old.tokens.len() - 1).min(tokens.len() - 1);
            if old.group != group {
                reused = reused.min(group.as_ref().map_or(0, |g| g.start));
            }
        }
        let mut nodes: Vec<PathNode> = Vec::new();
        let mut states: Vec<Vec<u32>> = vec![Vec::new(); tokens.len() + 1];
        let mut edge_cache: Vec<Option<DecodingPosition>> = (0..tokens.len()).map(|_| None).collect();
        let old = cache.take();
        if let Some(mut old) = old {
            if old.fixed == fixed && old.typo == request.typo && old.revision == revision {
                if old.tokens == tokens {
                    edge_cache = std::mem::take(&mut old.edges);
                } else {
                    let common = old.tokens.iter().zip(tokens).take_while(|(a, b)| a == b).count();
                    let interior = common.min(old.tokens.len() - 1).min(tokens.len() - 1);
                    for (slot, old) in edge_cache.iter_mut().zip(old.edges.iter_mut()).take(interior.saturating_sub(5))
                    {
                        *slot = old.take();
                    }
                }
            }
            if reused > 0 {
                // Copy the reused positions' nodes; every node's predecessor is in an earlier reused position.
                let mut remap: HashMap<u32, u32> = HashMap::new();
                for (state, old_state) in states.iter_mut().zip(&old.states).take(reused + 1) {
                    for &id in old_state {
                        let mut node = old.nodes[id as usize].clone();
                        node.previous = node.previous.map(|p| remap[&p]);
                        remap.insert(id, nodes.len() as u32);
                        state.push(nodes.len() as u32);
                        nodes.push(node);
                    }
                }
            }
        }
        if reused == 0 {
            nodes.push(PathNode {
                score: 0.0,
                word: None,
                previous: None,
                history: request.start,
                beam_score: 0.0,
                has_last_word: false,
                text: String::new(),
            });
            states[0] = vec![0];
        }
        // Only words of at most six syllables can cross the reusable prefix boundary.
        for start in reused.saturating_sub(5)..tokens.len() {
            if states[start].is_empty() {
                continue;
            }
            let beam = states[start].clone();
            if edge_cache[start].is_none() {
                edge_cache[start] = Some(self.position(request, start));
            }
            let position = edge_cache[start].as_mut().expect("built");
            for &previous_id in &beam {
                let previous = nodes[previous_id as usize].clone();
                let expansions: Vec<usize> = if distinct {
                    (0..position.edges.len()).collect()
                } else {
                    position.expansions(previous.history, model, tokens.len())
                };
                for edge_index in expansions {
                    let edge = &mut position.edges[edge_index];
                    let word = edge.word.clone();
                    if let Some(group) = &group
                        && [group.start, group.end].iter().any(|&b| word.range.start < b && b < word.range.end)
                    {
                        continue;
                    }
                    let end = word.range.end;
                    if end <= reused {
                        continue;
                    }
                    let is_literal = edge.entry.is_none();
                    let (value, state_key) = edge.transition(previous.history, model);
                    let total = previous.score + value;
                    let rank = total + if end == tokens.len() { model.end(state_key) } else { 0.0 };
                    if states[end].len() == width
                        && let Some(&worst_id) = states[end].last()
                    {
                        let worst = &nodes[worst_id as usize];
                        let last: &str = if is_literal { "<s>" } else { &word.text };
                        if rank < worst.beam_score
                            || (rank == worst.beam_score
                                && (last > worst.last_word()
                                    || (last == worst.last_word()
                                        && state_key.older.unwrap_or(0) > worst.history.older.unwrap_or(0))))
                        {
                            continue;
                        }
                    }
                    let text = if distinct { format!("{}{}", previous.text, word.text) } else { String::new() };
                    let final_distinct = distinct && end == tokens.len();
                    let existing = states[end].iter().position(|&id| {
                        let node = &nodes[id as usize];
                        if final_distinct {
                            node.text == text
                        } else {
                            node.history == state_key && (!distinct || node.text == text)
                        }
                    });
                    if let Some(index) = existing {
                        let old = &nodes[states[end][index] as usize];
                        let (value, old_value) =
                            if final_distinct { (rank, old.beam_score) } else { (total, old.score) };
                        let old_previous = old.previous.map_or("", |p| nodes[p as usize].last_word());
                        if !(value > old_value || (value == old_value && previous.last_word() < old_previous)) {
                            continue;
                        }
                        states[end].remove(index);
                    } else if total == f64::NEG_INFINITY {
                        continue;
                    }
                    let node = PathNode {
                        score: total,
                        has_last_word: !word.reading.is_empty(),
                        word: Some(word),
                        previous: Some(previous_id),
                        history: state_key,
                        beam_score: rank,
                        text,
                    };
                    let insertion = states[end]
                        .iter()
                        .position(|&id| nodes[id as usize].precedes(&node))
                        .unwrap_or(states[end].len());
                    let id = nodes.len() as u32;
                    nodes.push(node);
                    states[end].insert(insertion, id);
                    if states[end].len() > width {
                        states[end].pop();
                    }
                }
            }
        }
        let result = states[tokens.len()].clone();
        *cache = Some(ConversionCache {
            tokens: tokens.to_vec(),
            fixed: fixed.to_vec(),
            typo: request.typo,
            revision,
            group,
            start: request.start,
            nodes,
            states,
            edges: edge_cache,
        });
        result
    }

    /// The lattice edges starting at token `start`.
    // One function, as the edge construction in Conversion.swift's `decode`, for side-by-side comparison.
    #[allow(clippy::too_many_lines)]
    fn position(&self, request: &Request<'_>, start: usize) -> DecodingPosition {
        let tokens = request.tokens;
        let fixed = request.fixed;
        let mut edges: Vec<(ConvertedWord, Option<WordEntry>, Vec<WordEntry>)> = Vec::new();
        match &tokens[start] {
            CompositionToken::Literal(text) => edges.push((
                ConvertedWord {
                    range: start..start + 1,
                    text: text.clone(),
                    reading: "".into(),
                    word_id: None,
                    history: None,
                    corrected: false,
                },
                None,
                vec![],
            )),
            CompositionToken::Syllable { reading, .. } => {
                let fixed_here = fixed.iter().find(|w| w.range.start == start);
                if let Some(word) = fixed_here {
                    let lookup = |text: &str, reading: &str, word_id: Option<u32>| {
                        self.entries(reading)
                            .iter()
                            .find(|e| &*e.word == text)
                            .cloned()
                            .unwrap_or_else(|| WordEntry::new(text, -7.0).with_word_id(word_id))
                    };
                    let entry = lookup(&word.text, &word.reading, word.word_id);
                    let components =
                        word.components.iter().map(|part| lookup(&part.text, &part.reading, part.word_id)).collect();
                    edges.push((
                        ConvertedWord {
                            range: word.range.clone(),
                            text: word.text.clone(),
                            reading: word.reading.clone(),
                            word_id: None,
                            history: None,
                            corrected: false,
                        },
                        Some(entry),
                        components,
                    ));
                } else {
                    for m in self.matches(tokens, start, request.typo) {
                        let corrected = m.annotation.is_some();
                        edges.push((
                            ConvertedWord {
                                range: m.range.clone(),
                                text: m.entry.word.clone(),
                                reading: m.typed_reading.clone(),
                                word_id: None,
                                history: None,
                                corrected,
                            },
                            Some(m.entry),
                            vec![],
                        ));
                    }
                }
                // Keep completed tokens if a user removes a phrase that supplied their only dictionary entry.
                if !edges.iter().any(|e| e.0.range.len() == 1) {
                    edges.push((
                        ConvertedWord {
                            range: start..start + 1,
                            text: reading.clone(),
                            reading: reading.clone(),
                            word_id: None,
                            history: None,
                            corrected: false,
                        },
                        Some(WordEntry::new(reading.clone(), -100.0)),
                        vec![],
                    ));
                }
            }
        }
        let edges = edges
            .into_iter()
            .filter(|(word, _, _)| {
                fixed.iter().all(|f| !overlaps(&f.range, &word.range) || (f.range == word.range && f.text == word.text))
            })
            .map(|(mut word, entry, parts)| {
                let values = entry.as_ref().map(|e| self.scoring(e, &word.reading));
                let components: Vec<Component> = parts
                    .into_iter()
                    .map(|part| {
                        let (unigram, adjustment) = self.scoring(&part, part.reading.as_deref().unwrap_or(""));
                        Component { entry: part, unigram, adjustment }
                    })
                    .collect();
                let characters = if word.reading.is_empty() {
                    0.0
                } else {
                    request.characters.map_or(0.0, |c| c.score(&word.text, word.range.start))
                };
                word.word_id =
                    components.last().and_then(|c| c.entry.word_id).or_else(|| entry.as_ref().and_then(|e| e.word_id));
                DecodingEdge {
                    word: Arc::new(word),
                    entry,
                    values,
                    components,
                    characters,
                    transitions: HashMap::new(),
                }
            })
            .collect();
        DecodingPosition { edges, best: HashMap::new() }
    }
}

/// The words of the path ending at node `end`, each with the history after it.
fn path(nodes: &[PathNode], end: u32) -> Vec<ConvertedWord> {
    let mut result = Vec::new();
    let mut node = Some(end);
    while let Some(id) = node {
        let current = &nodes[id as usize];
        let Some(word) = &current.word else { break };
        let mut word = (**word).clone();
        word.history = Some(current.history);
        result.push(word);
        node = current.previous;
    }
    result.reverse();
    result
}

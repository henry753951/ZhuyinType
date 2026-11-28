//! A run of keys typed at the end of the composition, split into syllables by a Viterbi search over key positions
//! (port of `KeyDecoder.swift`).
//!
//! A syllable is built in the composer's three slots (initial, medial, final), so the order of keys inside one
//! syllable never matters.  What the search adds is where syllables end: after a tone key or Space (explicit), or
//! before the next initial when the syllable is complete as a first tone (implicit); and, for `Reorder` and `Timed`,
//! repairs across syllables: two adjacent keys swapped (`ji3ap7` typed `jia3p7`) and a key pressed by mistake.
//! Closing a syllable costs nothing when it is valid and the keyboard channel's cheapest correction otherwise.
//!
//! States with equal cost are ranked by when their signature was first reached, as in the Swift engine (`DESIGN.md`
//! §4).

use std::rc::Rc;

use crate::api::KeyDecoderMethod;
use crate::fast_hash::FastMap;
use crate::key_channel;
use crate::keyboard;

/// One typed key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// The key's unshifted ASCII character.
    pub character: char,
    /// Key-down time in seconds, when known.
    pub time: Option<f64>,
}

impl Key {
    /// A key with an optional key-down time.
    #[must_use]
    pub fn new(character: char, time: Option<f64>) -> Self {
        Self { character, time }
    }
}

/// One finished syllable of a split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Syllable {
    /// Zhuyin symbols in slot order plus the tone mark: the token's reading.
    pub reading: String,
    /// The keys behind it, in the order taken, ending with the tone key or Space.
    pub keys: String,
    /// Ended before the next initial without a tone key or Space.
    pub implicit: bool,
}

/// One split of the run.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodeResult {
    /// The finished syllables.
    pub syllables: Vec<Syllable>,
    /// The unfinished last syllable's slots.
    pub buffer: [Option<char>; 3],
    /// The unfinished last syllable's keys.
    pub buffer_keys: String,
    /// The split's cost.
    pub cost: f64,
    /// The part of `cost` that corrects syllables no reading has (`closing_cost`).  The engine charges its own
    /// correction when a sentence reads such a syllable as another, so a sentence's score already holds it.
    pub corrections: f64,
}

/// Costs on the keyboard channel's scale (transposition 9, extra or missing key 12.5, neighbour 11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Costs {
    /// First tone without Space before the next initial.
    pub implicit_boundary: f64,
    /// For `Timed`, on top of `implicit_boundary`: the boundary falls between keys pressed together.
    pub boundary_together: f64,
    /// First tone without Space before a syllable that has no initial, where the next key's slot is filled
    /// (`ㄧㄣ ㄩㄝˋ`).  Above `overwrite`, so it is taken when the language model prefers it.
    pub bare_boundary: f64,
    /// A filled slot typed again (the slot keeps the later key).
    pub overwrite: f64,
    /// Two keys in the other order (`Reorder`).
    pub swap: f64,
    /// An extra key (`Reorder`).
    pub skip: f64,
    /// For `Timed`: keys pressed within `together_gap` of each other.
    pub swap_together: f64,
    /// For `Timed`: an extra key pressed together with a neighbouring key.
    pub skip_together: f64,
    /// For `Timed`: keys typed apart.
    pub swap_apart: f64,
    /// For `Timed`: an extra key typed apart.
    pub skip_apart: f64,
    /// For `Timed`: an initial typed apart after the medial or final of its own syllable.  Below the implicit
    /// boundary, so `ㄧㄉㄥˋ` is `ㄉㄧㄥˋ` rather than `ㄧ` and `ㄉㄥˋ` (the commonest slip in the app's correction log).
    pub late_initial: f64,
    /// For `Timed`: seconds within which two key-downs count as pressed together.
    pub together_gap: f64,
}

impl Default for Costs {
    fn default() -> Self {
        // Chosen with `zt eval typing-errors` on both dev sets (docs/engine/MODEL.md §3).
        Self {
            implicit_boundary: 4.0,
            boundary_together: 1.0,
            bare_boundary: 12.0,
            overwrite: 10.0,
            swap: 10.0,
            skip: 13.0,
            swap_together: 2.0,
            skip_together: 4.0,
            swap_apart: 12.0,
            skip_apart: 15.0,
            late_initial: 3.0,
            together_gap: 0.035,
        }
    }
}

/// Which repairs `Reorder` and `Timed` make, so each kind of correction can be turned off alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repairs {
    /// Keys of a syllable in another order: two neighbouring keys swapped, an initial typed after its final.  Off,
    /// an initial after its final is priced as `Boundaries` prices it.
    pub order: bool,
    /// An extra key skipped.
    pub extra_keys: bool,
}

impl Default for Repairs {
    fn default() -> Self {
        Self { order: true, extra_keys: true }
    }
}

/// States kept per key position.
pub const BEAM_WIDTH: usize = 24;

/// A finished syllable in a state's history, shared between the states that continue from it.
struct Node {
    reading: Rc<str>,
    keys: Box<str>,
    implicit: bool,
    previous: Option<Rc<Node>>,
}

#[derive(Clone)]
struct State {
    slots: [Option<char>; 3],
    keys: String,
    last: Option<Rc<Node>>,
    cost: f64,
    corrections: f64,
    /// The syllable began at a bare boundary (a medial or final after a first tone without Space) and has no initial
    /// yet: an initial typed late refunds the boundary's extra cost.
    bare: bool,
}

/// Finished syllables a state's signature holds: the longest word (lexicon and custom words alike).  Splits that
/// differ inside the last word's span are told apart only by the language model, so they must reach it; merging on
/// fewer syllables dropped the dearer one before any word was read (`ㄒㄩˊ ㄧ ㄧㄡˋ` gave way to `ㄒㄩˊ ㄧㄡˋ` once a
/// syllable followed).
pub const HISTORY_SYLLABLES: usize = 6;

/// States agreeing on the unfinished syllable and the last `HISTORY_SYLLABLES` syllables compete; earlier differences
/// are settled, so the search stays a beam while every split a word could tell apart survives for the language model.
#[derive(PartialEq, Eq, Hash)]
struct Signature {
    slots: [Option<char>; 3],
    keys: String,
    history: [Option<Rc<str>>; HISTORY_SYLLABLES],
    bare: bool,
}

impl State {
    fn signature(&self) -> Signature {
        let mut history: [Option<Rc<str>>; HISTORY_SYLLABLES] = Default::default();
        let mut node = self.last.as_deref();
        for reading in &mut history {
            let Some(current) = node else { break };
            *reading = Some(current.reading.clone());
            node = current.previous.as_deref();
        }
        Signature { slots: self.slots, keys: self.keys.clone(), history, bare: self.bare }
    }

    /// Every key typed is still in its slot.
    fn replaced_none(&self) -> bool {
        self.keys.chars().count() == self.slots.iter().flatten().count()
    }

    fn has_symbols(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }
}

/// The best states at one key position, in the order their signatures were first reached.
#[derive(Default)]
struct Level {
    states: Vec<State>,
    index: FastMap<Signature, usize>,
}

impl Level {
    fn offer(&mut self, state: State) {
        let signature = state.signature();
        if let Some(&i) = self.index.get(&signature) {
            if self.states[i].cost <= state.cost {
                return;
            }
            self.states[i] = state;
        } else {
            self.index.insert(signature, self.states.len());
            self.states.push(state);
        }
    }

    /// Indices of the states by cost, cheapest first; equal costs keep their order.
    fn ranked(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.states.len()).collect();
        order.sort_by(|&a, &b| self.states[a].cost.total_cmp(&self.states[b].cost));
        order
    }
}

/// Splits key runs into syllables.  `closing_cost(reading, keys)` is the cost of closing a syllable from these slots
/// and keys: 0 when valid, a correction cost, or `None` when impossible.
pub struct KeyDecoder<F> {
    /// How keys become syllables.
    pub method: KeyDecoderMethod,
    /// The decoder's costs (the Swift engine's shared `KeyDecoder.costs`).
    pub costs: Costs,
    /// The repairs made (`Reorder` and `Timed`).
    pub repairs: Repairs,
    closing_cost: F,
}

impl<F: Fn(&str, &str) -> Option<f64>> KeyDecoder<F> {
    /// A decoder with the given method, costs and closing cost.
    pub fn new(method: KeyDecoderMethod, costs: Costs, closing_cost: F) -> Self {
        Self { method, costs, repairs: Repairs::default(), closing_cost }
    }

    /// The same decoder making only `repairs`.
    #[must_use]
    pub fn with_repairs(mut self, repairs: Repairs) -> Self {
        self.repairs = repairs;
        self
    }

    /// The `limit` cheapest splits, cheapest first (each a different unfinished syllable or a different history).
    pub fn decode(&self, keys: &[Key], limit: usize) -> Vec<DecodeResult> {
        let mut levels: Vec<Level> = (0..=keys.len()).map(|_| Level::default()).collect();
        levels[0].offer(State {
            slots: [None; 3],
            keys: String::new(),
            last: None,
            cost: 0.0,
            corrections: 0.0,
            bare: false,
        });
        let reorders = matches!(self.method, KeyDecoderMethod::Reorder | KeyDecoderMethod::Timed);
        let mut next = Vec::with_capacity(2);
        let mut swapped_first = Vec::with_capacity(2);
        for index in 0..keys.len() {
            let level = std::mem::take(&mut levels[index]);
            let mut order = level.ranked();
            order.truncate(BEAM_WIDTH);
            for &i in &order {
                let state = &level.states[i];
                next.clear();
                self.take(state, keys[index], self.together(keys, index, index.checked_sub(1)), &mut next);
                for successor in next.drain(..) {
                    levels[index + 1].offer(successor);
                }
                // A tone key right after a syllable ended fits nothing, and the composer ignores it.  Without the
                // extra-key repair it is ignored here as well (free), so no swap reads the syllables again around it
                // (情人 and a stray ˋ as 其仍嗯).  With it, the same tone key again (a tone pressed twice) is skipped at
                // an extra key's cost: two swaps no longer read 會議 and one more ˋ as 護愛意, while one swap of a tone
                // key still wins.
                if reorders
                    && !state.has_symbols()
                    && state.last.is_some()
                    && keyboard::tone(keys[index].character).is_some()
                {
                    let cost = if !self.repairs.extra_keys {
                        Some(0.0)
                    } else if index > 0 && keys[index - 1].character == keys[index].character {
                        Some(self.skip_cost(keys, index))
                    } else {
                        None
                    };
                    if let Some(cost) = cost {
                        let mut skipped = state.clone();
                        skipped.cost += cost;
                        levels[index + 1].offer(skipped);
                    }
                }
                if !reorders {
                    continue;
                }
                // Skip a Zhuyin key pressed by mistake.  A tone key or Space that fits nothing is ignored by the
                // composer instead, as it always was.
                if self.repairs.extra_keys && keyboard::symbol(keys[index].character).is_some() {
                    let mut skipped = state.clone();
                    skipped.cost += self.skip_cost(keys, index);
                    levels[index + 1].offer(skipped);
                }
                // Take the next two keys in the other order.  Not a Zhuyin key past the tone key or Space typed last:
                // that leaves the key alone in a syllable no later key confirms, and ties with the same key read out
                // of order inside its own syllable (`ㄚ ㄅ ˙` as `ㄚ˙` and `ㄅ` instead of `ㄅㄚ˙`).  The next key
                // decodes the run again, so the swap stays possible once something follows it.
                if self.repairs.order
                    && index + 1 < keys.len()
                    && keys[index].character != keys[index + 1].character
                    && !(index + 2 == keys.len() && closes_syllable(keys[index + 1].character))
                {
                    swapped_first.clear();
                    self.take(state, keys[index + 1], false, &mut swapped_first);
                    for first in &swapped_first {
                        next.clear();
                        self.take(first, keys[index], false, &mut next);
                        let cost = self.swap_cost(keys, index);
                        for mut second in next.drain(..) {
                            second.cost += cost;
                            levels[index + 2].offer(second);
                        }
                    }
                }
            }
        }
        let last = levels.pop().unwrap_or_default();
        let order = last.ranked();
        order.into_iter().take(limit).map(|i| result(&last.states[i])).collect()
    }

    /// The cheapest split, or an empty one of infinite cost when there is none.
    pub fn decode_best(&self, keys: &[Key]) -> DecodeResult {
        self.decode(keys, 1).into_iter().next().unwrap_or(DecodeResult {
            syllables: vec![],
            buffer: [None; 3],
            buffer_keys: String::new(),
            cost: f64::INFINITY,
            corrections: 0.0,
        })
    }

    /// The states reached by taking one more key.  `pressed_with_previous`: this key went down almost together with
    /// the one before it (for `Timed`, ending a syllable between them then costs more).
    fn take(&self, state: &State, key: Key, pressed_with_previous: bool, out: &mut Vec<State>) {
        let character = key.character;
        let tone = keyboard::tone(character).or(if character == ' ' { Some("") } else { None });
        if let Some(tone) = tone {
            if let Some(closed) = self.close(state, tone, character, false) {
                out.push(closed);
            }
            return;
        }
        let Some(symbol) = keyboard::symbol(character) else { return };
        let Some(slot) = keyboard::slot(symbol) else { return };
        let mut placed = state.clone();
        // A filled slot takes the later key, and every key stays in `keys` for the channel's correction; replacing
        // a key costs.  An initial after the medial or final is a key out of order: cheap when pressed together
        // with the key before it, otherwise priced like a swap.
        if placed.slots[slot].is_some() {
            placed.cost += self.costs.overwrite;
        } else if slot == 0 && (state.slots[1].is_some() || state.slots[2].is_some()) {
            placed.cost += match self.method {
                KeyDecoderMethod::Slots => 0.0,
                _ if !self.repairs.order => self.costs.swap,
                KeyDecoderMethod::Timed => {
                    if pressed_with_previous {
                        self.costs.swap_together
                    } else {
                        self.costs.late_initial
                    }
                }
                _ => self.costs.swap,
            };
            // A syllable typed final first after a first tone without Space (`ㄔㄠ ㄢㄗˋ`) had no initial when it
            // began; with one now, the boundary was an ordinary one.
            if state.bare && self.repairs.order && self.method != KeyDecoderMethod::Boundaries {
                placed.cost -= self.costs.bare_boundary - self.costs.implicit_boundary;
            }
            placed.bare = false;
        }
        placed.slots[slot] = Some(symbol);
        placed.keys.push(character);
        out.push(placed);
        // The syllable so far is a whole first-tone syllable and this key starts the next one: an initial, or a medial
        // or final whose slot is filled (and nothing typed again before it).  Keys pressed together are less likely to be in two syllables, not never: a
        // fast typist who leaves out Space rolls from one syllable into the next.
        if self.method != KeyDecoderMethod::Slots
            && state.has_symbols()
            && (slot == 0 || (state.slots[slot].is_some() && state.replaced_none()))
            && let Some(mut closed) = self.close(state, "", ' ', true)
        {
            if slot != 0 {
                closed.cost += self.costs.bare_boundary - self.costs.implicit_boundary;
                closed.bare = true;
            }
            if self.method == KeyDecoderMethod::Timed && pressed_with_previous {
                closed.cost += self.costs.boundary_together;
            }
            closed.slots[slot] = Some(symbol);
            closed.keys = character.to_string();
            out.push(closed);
        }
    }

    fn close(&self, state: &State, tone: &str, tone_key: char, implicit: bool) -> Option<State> {
        if !state.has_symbols() {
            return None;
        }
        let mut reading = String::with_capacity(12);
        reading.extend(state.slots.iter().flatten());
        reading.push_str(tone);
        let mut keys = String::with_capacity(state.keys.len() + 1);
        keys.push_str(&state.keys);
        keys.push(tone_key);
        let cost = (self.closing_cost)(&reading, &keys)?;
        let node = Node { reading: reading.into(), keys: keys.into(), implicit, previous: state.last.clone() };
        Some(State {
            slots: [None; 3],
            keys: String::new(),
            last: Some(Rc::new(node)),
            cost: state.cost + (cost + if implicit { self.costs.implicit_boundary } else { 0.0 }),
            corrections: state.corrections + cost,
            bare: false,
        })
    }

    /// Keys `index` and `other` went down within the together gap of each other.
    fn together(&self, keys: &[Key], index: usize, other: Option<usize>) -> bool {
        let Some(other) = other.filter(|&other| other < keys.len()) else { return false };
        let (Some(a), Some(b)) = (keys[index].time, keys[other].time) else { return false };
        (a - b).abs() < self.costs.together_gap
    }

    fn swap_cost(&self, keys: &[Key], index: usize) -> f64 {
        if self.method == KeyDecoderMethod::Timed {
            if self.together(keys, index, Some(index + 1)) { self.costs.swap_together } else { self.costs.swap_apart }
        } else {
            self.costs.swap
        }
    }

    fn skip_cost(&self, keys: &[Key], index: usize) -> f64 {
        if self.method != KeyDecoderMethod::Timed {
            return self.costs.skip;
        }
        let character = keys[index].character;
        let near = (self.together(keys, index, index.checked_sub(1))
            && key_channel::are_neighbours(keys[index - 1].character, character))
            || (self.together(keys, index, Some(index + 1))
                && key_channel::are_neighbours(keys[index + 1].character, character));
        if near { self.costs.skip_together } else { self.costs.skip_apart }
    }
}

/// A tone key or Space: it ends the syllable being typed.
fn closes_syllable(character: char) -> bool {
    character == ' ' || keyboard::tone(character).is_some()
}

/// A state as a result: its syllables materialized in order.
fn result(state: &State) -> DecodeResult {
    let mut syllables = Vec::new();
    let mut node = state.last.as_deref();
    while let Some(current) = node {
        syllables.push(Syllable {
            reading: current.reading.to_string(),
            keys: current.keys.to_string(),
            implicit: current.implicit,
        });
        node = current.previous.as_deref();
    }
    syllables.reverse();
    DecodeResult {
        syllables,
        buffer: state.slots,
        buffer_keys: state.keys.clone(),
        cost: state.cost,
        corrections: state.corrections,
    }
}

impl Drop for Node {
    /// Unlinks long histories iteratively, so dropping a state never recurses deeply.
    fn drop(&mut self) {
        let mut previous = self.previous.take();
        while let Some(node) = previous {
            match Rc::try_unwrap(node) {
                Ok(mut node) => previous = node.previous.take(),
                Err(_) => break,
            }
        }
    }
}

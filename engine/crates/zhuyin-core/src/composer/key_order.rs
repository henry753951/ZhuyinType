//! Keys typed in another order, offered in the candidate window (`key_order` set to list them only).
//!
//! The composition keeps the keys as typed.  When the window opens, the syllables around it are decoded again from
//! their keys with the order repaired; where that reads differently, the window offers the repaired words, and
//! picking them replaces those syllables.

use std::sync::Arc;

use super::{Composer, overlaps};
use crate::conversion::{CompositionToken, FixedWord};
use crate::key_decoder::{Key, Syllable};

/// Syllables on each side of the window's syllable decoded again.
const REACH: usize = 6;

/// The repaired words the candidate window offers, and what picking them puts in the composition.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OrderChoice {
    /// The candidate: the syllables as typed it replaces, the repaired words and their readings.
    pub word: FixedWord,
    /// The repaired syllables, in place of `word.range`.
    pub syllables: Vec<Syllable>,
}

impl Composer {
    /// The words the syllables around `target` give with their keys' order repaired, when they differ from the
    /// composition there.  Picks and literals bound the syllables decoded again.
    pub(super) fn order_choice(&self, target: usize) -> Option<OrderChoice> {
        let free = |i: usize| {
            matches!(self.tokens.get(i), Some(CompositionToken::Syllable { .. }))
                && !self.fixed.iter().any(|f| !f.frozen && f.range.contains(&i))
        };
        if !free(target) {
            return None;
        }
        let mut start = target;
        while start > 0 && target - start < REACH && free(start - 1) {
            start -= 1;
        }
        let mut end = target + 1;
        while end - target <= REACH && free(end) {
            end += 1;
        }
        let typed = &self.tokens[start..end];
        let keys: Vec<Key> = typed
            .iter()
            .flat_map(|token| {
                let CompositionToken::Syllable { keys, implicit, .. } = token else { unreachable!() };
                // An implicit syllable records the Space no one typed.
                let keys = if *implicit { keys.strip_suffix(' ').unwrap_or(keys) } else { keys };
                keys.chars().map(|c| Key::new(c, None)).collect::<Vec<_>>()
            })
            .collect();
        let repaired = self.split(&keys, &self.tokens[..start], true)?;
        if !repaired.buffer_keys.is_empty() {
            return None;
        }
        let same = |token: &CompositionToken, syllable: &Syllable| token.reading() == Some(syllable.reading.as_str());
        let new = &repaired.syllables;
        let prefix = typed.iter().zip(new).take_while(|(t, s)| same(t, s)).count();
        if prefix == typed.len() && prefix == new.len() {
            return None;
        }
        let suffix =
            typed[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(t, s)| same(t, s)).count();
        // Words of the repaired reading over what changed, whole.
        let mut tokens = self.tokens[..start].to_vec();
        tokens.extend(new.iter().map(|s| CompositionToken::syllable(&s.reading, &s.keys, s.implicit)));
        tokens.extend_from_slice(&self.tokens[end..]);
        let base = start;
        let changed = base + prefix..base + new.len() - suffix;
        let mut request = self.request(&tokens);
        request.fixed = &[];
        let words = self.engine.convert(&request, &mut None);
        let covering: Vec<_> = words.iter().filter(|w| overlaps(&w.range, &changed) && !w.reading.is_empty()).collect();
        let (first, last) = (covering.first()?, covering.last()?);
        let (from, to) = (first.range.start.min(changed.start), last.range.end.max(changed.end));
        if from < base || to > base + new.len() {
            return None;
        }
        let before = from - base;
        let after = base + new.len() - to;
        if before > prefix || after > suffix {
            return None;
        }
        let range = start + before..end - after;
        if !range.contains(&target) {
            return None;
        }
        let syllables = new[before..new.len() - after].to_vec();
        let reading: Vec<&str> = syllables.iter().map(|s| s.reading.as_str()).collect();
        let reading: Arc<str> = reading.join(" ").into();
        let text: String = covering.iter().map(|w| &*w.text).collect();
        Some(OrderChoice {
            word: FixedWord {
                range,
                text: text.into(),
                reading: reading.clone(),
                word_id: None,
                annotation: Some(reading),
                components: vec![],
                frozen: false,
            },
            syllables,
        })
    }

    /// Puts the repaired syllables in place of those typed and keeps the words they read as shown (frozen, not a
    /// pick: nothing is learnt, as when a correction is put back).
    pub(super) fn apply_order_choice(&mut self, choice: &OrderChoice) {
        self.settle_run();
        let range = choice.word.range.clone();
        let (cursor, from_end) = (self.cursor, self.tokens.len() - self.cursor);
        self.delete(range.clone());
        self.cursor = range.start;
        for syllable in &choice.syllables {
            self.insert(CompositionToken::syllable(&syllable.reading, &syllable.keys, syllable.implicit));
        }
        let span = range.start..range.start + choice.syllables.len();
        self.cursor = if cursor <= range.start {
            cursor
        } else if cursor >= range.end {
            self.tokens.len() - from_end
        } else {
            span.end
        };
        let mut request = self.request(&self.tokens);
        request.characters = self.character_scores.as_ref();
        let words = self.engine.convert(&request, &mut None);
        for word in words.iter().filter(|w| overlaps(&w.range, &span) && !w.reading.is_empty()) {
            if self.fixed.iter().any(|f| overlaps(&f.range, &word.range)) {
                continue;
            }
            let mut pinned = self.choice(&word.range, &word.text, &word.reading, Some(&words));
            pinned.frozen = true;
            self.fixed.push(pinned);
        }
        self.conversion_dirty = true;
    }
}

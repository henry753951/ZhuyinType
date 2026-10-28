use super::{
    CHARACTER_CANDIDATES, CharacterModelSyllable, CharacterScores, Composer, CompositionToken, HashMap,
    VariantCharacter, VariantScores, cost_bucket, graphemes, model_syllable, swift_max,
};

pub(super) fn valid_scores(scores: &[f64], count: usize) -> bool {
    scores.len() == count && scores.iter().all(|score| score.is_finite())
}

fn valid_rows(scores: &[Vec<f64>], lists: &[Vec<String>]) -> bool {
    scores.len() == lists.len() && scores.iter().zip(lists).all(|(row, list)| valid_scores(row, list.len()))
}

impl Composer {
    #[allow(clippy::too_many_lines)]
    pub(super) fn score_characters(&mut self) -> Option<CharacterScores> {
        self.character_scorer.as_ref()?;
        if !self.tokens.iter().any(|t| t.reading().is_some()) {
            return None;
        }
        let count = self.tokens.len();
        // Each character's best prior, and whether some match puts it there without a reading-channel variant.
        let mut candidates: Vec<HashMap<String, (f64, bool)>> = vec![HashMap::new(); count];
        let mut variants: Vec<HashMap<String, VariantCharacter>> = vec![HashMap::new(); count];
        let mut plain_best: Vec<f64> = vec![f64::NEG_INFINITY; count];
        for start in 0..count {
            if self.tokens[start].reading().is_none() {
                continue;
            }
            for m in self.engine.matches(&self.tokens, start, self.options.typo_tolerance) {
                let prior = m.entry.logp + m.entry.reading_prior - m.entry.penalty;
                for (offset, character) in graphemes(&m.entry.word).into_iter().enumerate() {
                    if start + offset >= count {
                        continue;
                    }
                    let value = prior;
                    let through = m.channel.filter(|c| c.position == offset);
                    if let Some(channel) = through {
                        // Ranked without the variant's cost, if it is competitive (below).
                        let free = prior + channel.cost;
                        let syllable = m
                            .entry
                            .reading
                            .as_deref()
                            .and_then(|r| r.split(' ').nth(offset))
                            .unwrap_or_default()
                            .to_owned();
                        let seen = variants[start + offset].entry(character.to_owned()).or_insert(VariantCharacter {
                            prior: f64::NEG_INFINITY,
                            syllable: String::new(),
                            correction: false,
                            cost: 0.0,
                        });
                        if free > seen.prior {
                            *seen = VariantCharacter {
                                prior: free,
                                syllable,
                                correction: channel.correction,
                                cost: channel.cost,
                            };
                        }
                    }
                    let slot =
                        candidates[start + offset].entry(character.to_owned()).or_insert((f64::NEG_INFINITY, false));
                    slot.0 = swift_max(slot.0, value);
                    slot.1 |= through.is_none();
                    if through.is_none() {
                        plain_best[start + offset] = plain_best[start + offset].max(value);
                    }
                }
            }
        }
        for word in &self.fixed {
            for (offset, character) in graphemes(&word.text).into_iter().enumerate() {
                if word.range.start + offset < count {
                    candidates[word.range.start + offset].insert(character.to_owned(), (0.0, true));
                    plain_best[word.range.start + offset] = plain_best[word.range.start + offset].max(0.0);
                }
            }
        }
        // Characters only a variant brings, within the window of the best other character: ranked without the
        // variant's cost, so the cut keeps them for the model (rule 8); the others keep their cost and the model's
        // score as typed.  As typed, a correction outside the window still carries its cost bucket, as in v3's
        // training noise (`VariantScores::Typed`).
        let typed = self.tuning.variant_scores == VariantScores::Typed;
        let mut outside: Vec<HashMap<String, VariantCharacter>> = vec![HashMap::new(); count];
        for (position, seen) in variants.iter_mut().enumerate() {
            let floor = plain_best[position] - self.tuning.variant_window;
            let kept = &mut outside[position];
            seen.retain(|character, v| {
                let only = candidates[position].get(character).is_none_or(|c| !c.1);
                if only && typed && v.correction && v.prior < floor {
                    kept.insert(character.clone(), v.clone());
                }
                only && v.prior >= floor
            });
            for (character, v) in seen.iter() {
                if let Some(slot) = candidates[position].get_mut(character) {
                    slot.0 = swift_max(slot.0, v.prior);
                }
            }
        }
        for (seen, kept) in variants.iter_mut().zip(outside) {
            seen.extend(kept);
        }
        // Equal priors by character, so the cut does not depend on the table's order.
        let lists: Vec<Vec<String>> = candidates
            .into_iter()
            .map(|position| {
                let mut entries: Vec<(String, f64)> = position.into_iter().map(|(c, (v, _))| (c, v)).collect();
                entries.sort_by(|a, b| {
                    if a.1 == b.1 {
                        a.0.as_bytes().cmp(b.0.as_bytes())
                    } else if a.1 > b.1 {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                });
                entries.into_iter().take(CHARACTER_CANDIDATES).map(|e| e.0).collect()
            })
            .collect();
        let syllables: Vec<CharacterModelSyllable> = self
            .tokens
            .iter()
            .map(|token| match token {
                CompositionToken::Literal(_) => {
                    CharacterModelSyllable { base: None, tone: 0, cost_bucket: 0, implicit: false }
                }
                CompositionToken::Syllable { reading, keys, implicit } => {
                    let mut bucket = 0;
                    if !self.engine.is_valid_syllable(reading) {
                        let cost = self.engine.alternatives(keys).first().map_or(f64::INFINITY, |a| a.cost);
                        bucket = cost_bucket(cost);
                    }
                    model_syllable(reading, bucket, *implicit)
                }
            })
            .collect();
        let model = self.character_scorer.as_mut()?;
        let scores = model(&syllables, &lists).filter(|scores| valid_rows(scores, &lists))?;
        let mut positions: Vec<HashMap<String, f64>> = vec![HashMap::new(); count];
        for (index, list) in lists.iter().enumerate() {
            let Some(row) = scores.get(index) else { continue };
            for (column, character) in list.iter().enumerate() {
                if let Some(&score) = row.get(column) {
                    positions[index].insert(character.clone(), score);
                }
            }
        }
        if variants.iter().any(|v| !v.is_empty()) {
            self.score_variants(&syllables, &lists, &variants, &mut positions);
        }
        Some(CharacterScores::new(positions, self.tuning.character_weight))
    }

    /// Replaces the scores of characters only a reading-channel variant put in the lists ([`VariantScores`]).
    fn score_variants(
        &mut self,
        syllables: &[CharacterModelSyllable],
        lists: &[Vec<String>],
        variants: &[HashMap<String, VariantCharacter>],
        positions: &mut [HashMap<String, f64>],
    ) {
        let count = lists.len();
        let best: Vec<f64> = positions.iter().map(|p| p.values().copied().fold(f64::NEG_INFINITY, swift_max)).collect();
        // The listed variant characters at each position, best prior first (the list's order).
        let listed: Vec<Vec<(&String, &VariantCharacter)>> =
            (0..count).map(|i| lists[i].iter().filter_map(|c| variants[i].get(c).map(|v| (c, v))).collect()).collect();
        let mode = self.tuning.variant_scores;
        let mut again: Option<(Vec<CharacterModelSyllable>, Vec<Vec<String>>)> = None;
        if matches!(mode, VariantScores::Standard | VariantScores::Typed) {
            let mut changed = syllables.to_vec();
            let mut asked: Vec<Vec<String>> = vec![vec![]; count];
            for (i, characters) in listed.iter().enumerate() {
                if mode == VariantScores::Standard {
                    // The lexicon syllable of the best of them; the others with that syllable are asked too.
                    let Some((_, first)) = characters.first() else { continue };
                    changed[i] = model_syllable(&first.syllable, 0, false);
                    asked[i] = characters
                        .iter()
                        .filter(|(_, v)| v.syllable == first.syllable)
                        .map(|(c, _)| (*c).clone())
                        .collect();
                } else {
                    let corrections: Vec<&(&String, &VariantCharacter)> =
                        characters.iter().filter(|(_, v)| v.correction).collect();
                    let Some(cheapest) = corrections.iter().map(|(_, v)| v.cost).reduce(f64::min) else { continue };
                    changed[i].cost_bucket = cost_bucket(cheapest);
                    asked[i] = corrections.iter().map(|(c, _)| (*c).clone()).collect();
                }
            }
            if asked.iter().any(|a| !a.is_empty()) {
                again = Some((changed, asked));
            }
        }
        let extra = match (&again, self.character_scorer.as_mut()) {
            (Some((changed, asked)), Some(model)) => model(changed, asked).filter(|scores| valid_rows(scores, asked)),
            _ => None,
        };
        for (i, characters) in listed.iter().enumerate() {
            for (character, variant) in characters {
                let from_extra = again.as_ref().zip(extra.as_ref()).and_then(|((_, asked), rows)| {
                    let column = asked[i].iter().position(|c| c == *character)?;
                    rows.get(i)?.get(column).copied()
                });
                let score = match mode {
                    VariantScores::Standard => from_extra.unwrap_or(best[i]),
                    VariantScores::Best => best[i],
                    VariantScores::Zero => 0.0,
                    VariantScores::Typed => {
                        if variant.correction {
                            from_extra.unwrap_or(best[i])
                        } else {
                            continue;
                        }
                    }
                };
                positions[i].insert((*character).clone(), score);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{valid_rows, valid_scores};

    #[test]
    fn rejects_bad_shapes_and_non_finite_scores() {
        let lists = vec![vec!["a".to_owned()], vec![]];
        assert!(valid_rows(&[vec![1.0], vec![]], &lists));
        assert!(!valid_rows(&[vec![1.0]], &lists));
        assert!(!valid_rows(&[vec![], vec![]], &lists));
        assert!(!valid_rows(&[vec![f64::NAN], vec![]], &lists));
        assert!(!valid_rows(&[vec![f64::INFINITY], vec![]], &lists));
        assert!(!valid_scores(&[f64::NEG_INFINITY], 1));
    }
}

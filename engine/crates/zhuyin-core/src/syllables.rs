//! Which syllables exist: the part of the Swift `Engine` that `isValidSyllable` and the keyboard channel's
//! `validCodes` read (`rebuildReadingIndex`, `rebuildValidCodes`, `validSyllable`).  The engine (phase 2) owns one
//! and rebuilds it when user phrases or learned readings change.

use std::collections::{HashMap, HashSet};

use crate::key_channel::code_for_reading;

/// A reading without its tone marks (ˊ ˇ ˋ ˙).
#[must_use]
pub fn without_tones(reading: &str) -> String {
    reading.chars().filter(|&c| !matches!(c, 'ˊ' | 'ˇ' | 'ˋ' | '˙')).collect()
}

/// The toned syllables of the lexicon and personal data, their toneless bases, and the reading codes the keyboard
/// channel accepts.
#[derive(Clone, Debug, Default)]
pub struct SyllableSet {
    syllables: HashSet<String>,
    learned: HashSet<String>,
    bases: HashSet<String>,
    neutral_bases: HashSet<String>,
    valid_codes: HashMap<u32, String>,
}

impl SyllableSet {
    /// The set of these syllables (each one toned syllable, e.g. `ㄅㄚ˙`).
    pub fn new<S: Into<String>>(syllables: impl IntoIterator<Item = S>) -> Self {
        let mut set = SyllableSet { syllables: syllables.into_iter().map(Into::into).collect(), ..Default::default() };
        for syllable in &set.syllables {
            let base = without_tones(syllable);
            if syllable.ends_with('˙') {
                set.neutral_bases.insert(base.clone());
            }
            set.bases.insert(base);
        }
        set.rebuild_valid_codes();
        set
    }

    /// The set of every syllable of these readings (syllables separated by spaces).
    pub fn from_readings<'a>(readings: impl IntoIterator<Item = &'a str>) -> Self {
        Self::new(readings.into_iter().flat_map(|reading| reading.split(' ').filter(|part| !part.is_empty())))
    }

    /// Replaces the syllables of learned readings (accepted, but not tone-tolerant bases).
    pub fn set_learned<S: Into<String>>(&mut self, learned: impl IntoIterator<Item = S>) {
        self.learned = learned.into_iter().map(Into::into).collect();
        self.rebuild_valid_codes();
    }

    /// Whether the set has this exact syllable (not counting learned ones).
    #[must_use]
    pub fn contains(&self, syllable: &str) -> bool {
        self.syllables.contains(syllable)
    }

    /// Every syllable (not counting learned ones).
    #[must_use]
    pub fn syllables(&self) -> &HashSet<String> {
        &self.syllables
    }

    /// Whether the composer accepts `reading` as a syllable: a known or learned syllable, any tone of a base that
    /// has a neutral-tone syllable, or the neutral tone of any base.
    #[must_use]
    pub fn is_valid(&self, reading: &str) -> bool {
        if self.syllables.contains(reading) || self.learned.contains(reading) {
            return true;
        }
        let base = without_tones(reading);
        self.neutral_bases.contains(&base) || (reading.ends_with('˙') && self.bases.contains(&base))
    }

    /// The valid reading with this keyboard-channel code.
    #[inline]
    pub fn valid_reading(&self, code: u32) -> Option<&str> {
        self.valid_codes.get(&code).map(String::as_str)
    }

    /// Every (code, reading) the keyboard channel accepts.
    #[must_use]
    pub fn valid_codes(&self) -> &HashMap<u32, String> {
        &self.valid_codes
    }

    /// Rebuilds the code table in three steps, each overriding the one before.  Within a step, readings are visited
    /// in sorted order, so of two readings with one code the greater in UTF-8 order stays, as in the Swift engine
    /// (canonical readings never share a code).
    fn rebuild_valid_codes(&mut self) {
        self.valid_codes.clear();
        let mut readings: Vec<&String> = self.syllables.union(&self.learned).collect();
        readings.sort_unstable();
        for reading in readings {
            self.valid_codes.insert(code_for_reading(reading), reading.clone());
        }
        let mut bases: Vec<&String> = self.bases.iter().collect();
        bases.sort_unstable();
        for base in bases {
            let reading = format!("{base}˙");
            self.valid_codes.insert(code_for_reading(&reading), reading);
        }
        let mut neutral: Vec<&String> = self.neutral_bases.iter().collect();
        neutral.sort_unstable();
        for base in neutral {
            for tone in ["", "ˊ", "ˇ", "ˋ", "˙"] {
                let reading = format!("{base}{tone}");
                self.valid_codes.insert(code_for_reading(&reading), reading);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity() {
        // ZhuyinCoreChecks: neutral-tone variants and tolerant tones.
        let set = SyllableSet::from_readings(["ㄒㄧㄝˋ ㄒㄧㄝ˙", "ㄅㄚˋ"]);
        assert!(set.is_valid("ㄒㄧㄝˋ") && set.is_valid("ㄒㄧㄝ˙"));
        assert!(set.is_valid("ㄒㄧㄝ") && set.is_valid("ㄒㄧㄝˊ"), "a neutral base accepts every tone");
        assert!(set.is_valid("ㄅㄚ˙"), "typed neutral accepts another tone");
        assert!(!set.is_valid("ㄅㄚˇ"), "different non-neutral tones never match");
        assert_eq!(set.valid_reading(code_for_reading("ㄅㄚ˙")), Some("ㄅㄚ˙"));
        assert_eq!(set.valid_reading(code_for_reading("ㄒㄧㄝˇ")), Some("ㄒㄧㄝˇ"));
        assert_eq!(set.valid_reading(code_for_reading("ㄅㄚˇ")), None);
        assert_eq!(without_tones("ㄅㄚˋ ㄅㄚ˙"), "ㄅㄚ ㄅㄚ");
    }
}

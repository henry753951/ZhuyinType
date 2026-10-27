//! Golden tests on the real data: every sweep builds the same queries, in the same order, as the Swift engine's
//! measurement tool did (removed with the Swift engine in phase 5; in git history as `engine/parity/Sources`), and its
//! digest (FNV-1a over the result lines, floats as bit patterns) must equal the one the Swift engine produced
//! (`parity/golden/digests.tsv`, made on macOS).  Equal digests mean results bit-identical to the Swift engine's; a
//! change of behaviour shows up here, and records new digests only on purpose.
//!
//! Inputs: `ZHUYIN_DATA` (lexicon.bin, lexicon.tsv, lm/, `lm_chat`/), `ZHUYIN_PARITY_TEXT` (sentences for the
//! language-model sweeps), `ZHUYIN_TRIGRAM_LM` (a trigram model directory).  A sweep whose inputs are missing, or
//! whose input files differ in size from the ones the digests were made with, is skipped with a message.  On a
//! mismatch, the first differing line is shown when a dump of the expected lines is in `parity/out/`.

mod common;

use common::parity::{EXTRAS, Golden, Inputs, byte_sorted, hex, with_inputs};

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;

use zhuyin_core::api::KeyDecoderMethod;
use zhuyin_core::key_channel::{self, ReadingAlternative};
use zhuyin_core::key_decoder::{Costs, DecodeResult, Key, KeyDecoder};
use zhuyin_core::language_model::{BigramLanguageModel, BoundaryFreeLanguageModel, MixtureLanguageModel};
use zhuyin_core::lexicon::TextLexicon;
use zhuyin_core::syllables::{SyllableSet, without_tones};
use zhuyin_core::tsv::scan_tsv;
use zhuyin_core::{LanguageModel, LanguageModelHistory, MappedLexicon, NGramLanguageModel};

#[test]
fn lexicon_matches_swift() {
    with_inputs(|golden, inputs| {
        let lexicon = MappedLexicon::open(inputs.data.join("lexicon.bin")).unwrap();
        let with_extras =
            |list: &[String]| list.iter().cloned().chain(EXTRAS.iter().map(|s| (*s).to_owned())).collect::<Vec<_>>();
        let lines: Vec<String> = with_extras(&inputs.readings)
            .iter()
            .map(|reading| {
                let mut line = reading.clone();
                for e in lexicon.entry_refs(reading) {
                    write!(line, "\t{},{},{}", e.word, hex(e.logp), hex(e.reading_prior)).unwrap();
                }
                line
            })
            .collect();
        golden.check("lexicon.entries", &lines);
        let lines: Vec<String> = with_extras(&inputs.words)
            .iter()
            .map(|word| {
                let index = lexicon.word_index(word).map_or("-".to_owned(), |i| i.to_string());
                let readings: Vec<String> =
                    lexicon.readings_of(word).iter().map(|(r, s)| format!("{r},{}", hex(*s))).collect();
                let best = lexicon.best_reading(word).map_or("-".to_owned(), |(r, s)| format!("{r},{}", hex(s)));
                format!("{word}\t{index}\t{}\t{best}", readings.join(" "))
            })
            .collect();
        golden.check("lexicon.words", &lines);
        let bases = with_extras(&byte_sorted(inputs.readings.iter().map(|r| without_tones(r))));
        let lines: Vec<String> =
            bases.iter().map(|base| format!("{base}\t{}", lexicon.readings_for_base(base).join(","))).collect();
        golden.check("lexicon.bases", &lines);
        let mut prefixes = vec![];
        for base in &bases {
            let mut prefix = String::new();
            for part in base.split(' ').filter(|p| !p.is_empty()) {
                if !prefix.is_empty() {
                    prefix.push(' ');
                }
                prefix.push_str(part);
                prefixes.push(prefix.clone());
            }
            prefixes.push(format!("{base} ㄅ"));
            prefixes.push(format!("{base}ㄅ"));
        }
        let lines: Vec<String> = byte_sorted(prefixes)
            .into_iter()
            .map(|p| format!("{p}\t{}", u8::from(lexicon.contains_prefix(&p))))
            .collect();
        golden.check("lexicon.prefixes", &lines);
        golden.check("lexicon.syllables", &lexicon.syllables());

        // The engine's TSV path: personal IDs from 0x40000000 in order of first appearance.
        let mut ids: HashMap<String, u32> = HashMap::new();
        let text = TextLexicon::parse(&inputs.tsv, |word| {
            let next = 0x4000_0000 + ids.len() as u32;
            Some(*ids.entry(word.to_owned()).or_insert(next))
        });
        let lines: Vec<String> = inputs
            .readings
            .iter()
            .map(|reading| {
                let mut line = reading.clone();
                for e in text.entries(reading) {
                    let id = e.word_id.map_or("-".to_owned(), |id| id.to_string());
                    write!(line, "\t{},{},{},{id}", e.word, hex(e.logp), hex(e.reading_prior)).unwrap();
                }
                line
            })
            .collect();
        golden.check("textlexicon.entries", &lines);
        let lines: Vec<String> = inputs
            .words
            .iter()
            .map(|word| {
                let mut line = word.clone();
                for (reading, score) in text.readings_of(word) {
                    write!(line, "\t{reading},{}", hex(*score)).unwrap();
                }
                line
            })
            .collect();
        golden.check("textlexicon.reverse", &lines);
    });
}

/// The sweeps of `modelSweep` in the Swift tool.
fn model_sweep(golden: &Golden, name: &str, model: &dyn LanguageModel, inputs: &Inputs, ids: bool) {
    if ids {
        let lines: Vec<String> = inputs
            .words
            .iter()
            .map(String::as_str)
            .chain(["</s>", "<s>", "<unk>", ""])
            .map(|w| format!("{w}\t{}", model.word_id(w).map_or("-".to_owned(), |id| id.to_string())))
            .collect();
        golden.check(&format!("{name}.ids"), &lines);
    }
    if inputs.sequences.is_empty() {
        eprintln!("skipped {name}.scores: no text");
        return;
    }
    let mut lines = vec![];
    for sequence in &inputs.sequences {
        let word_ids: Vec<Option<u32>> = sequence.iter().map(|w| model.word_id(w)).collect();
        for (j, word) in sequence.iter().enumerate() {
            let history = LanguageModelHistory::new(
                match j {
                    0 => None,
                    1 => Some(1),
                    _ => Some(word_ids[j - 2].unwrap_or(2)),
                },
                if j == 0 { 1 } else { word_ids[j - 1].unwrap_or(2) },
            );
            lines.push(format!("{word}\t{}", hex(model.score(word_ids[j], history, -9.25))));
        }
        let n = sequence.len();
        let end = LanguageModelHistory::new(
            Some(if n > 1 { word_ids[n - 2].unwrap_or(2) } else { 1 }),
            word_ids[n - 1].unwrap_or(2),
        );
        lines.push(format!("</s>\t{}\t{}", hex(model.end_score(end, 1.0)), hex(model.end_score(end, 0.7))));
    }
    let n = inputs.words.len();
    for i in (0..n).step_by(7) {
        let (word, previous, older) =
            (&inputs.words[i], &inputs.words[(i * 7919 + 13) % n], &inputs.words[(i * 104_729 + 7) % n]);
        let history = LanguageModelHistory::new(model.word_id(older), model.word_id(previous).unwrap_or(2));
        lines.push(format!("{word}\t{previous}\t{older}\t{}", hex(model.score(model.word_id(word), history, -9.25))));
    }
    let max = u32::MAX;
    let specials: [(Option<u32>, Option<u32>, u32); 10] = [
        (None, None, 1),
        (Some(max), None, max),
        (Some(3), None, max),
        (Some(3), None, 2),
        (Some(0), None, 1),
        (Some(2), None, 2),
        (Some(1), None, 0),
        (Some(5), Some(1), 1),
        (Some(4), Some(3), 5),
        (None, Some(4), 3),
    ];
    for (word, older, previous) in specials {
        lines.push(hex(model.score(word, LanguageModelHistory::new(older, previous), -9.25)));
    }
    golden.check(&format!("{name}.scores"), &lines);
}

#[test]
fn language_models_match_swift() {
    with_inputs(|golden, inputs| {
        let lm_dir = inputs.data.join("lm");
        if !golden.matches("lm/lm.bin", &lm_dir.join("lm.bin")) {
            return;
        }
        let lm = NGramLanguageModel::open(&lm_dir).unwrap();
        model_sweep(golden, "lm", &lm, inputs, true);
        let chat_dir = inputs.data.join("lm_chat");
        if golden.matches("lm_chat/lm.bin", &chat_dir.join("lm.bin")) {
            let chat = NGramLanguageModel::open(&chat_dir).unwrap();
            model_sweep(golden, "lm_chat", &chat, inputs, true);
            let mixture = MixtureLanguageModel::new(chat, NGramLanguageModel::open(&lm_dir).unwrap(), 0.3);
            model_sweep(golden, "mixture", &mixture, inputs, true);
            if !inputs.sequences.is_empty() {
                let mut parts = vec![];
                for sequence in &inputs.sequences {
                    let mut previous = 1;
                    for word in sequence {
                        let id = mixture.word_id(word);
                        let (c, e) = mixture.scores(id, previous, -14.0);
                        parts.push(format!("{word}\t{}\t{}", hex(c), hex(e)));
                        previous = id.unwrap_or(2);
                    }
                }
                golden.check("mixture.parts", &parts);
            }
            mixture.set_chat_weight(0.7);
            model_sweep(golden, "mixture07", &mixture, inputs, false);
        }
        let boundary_free = BoundaryFreeLanguageModel { base: NGramLanguageModel::open(&lm_dir).unwrap() };
        model_sweep(golden, "boundaryfree", &boundary_free, inputs, false);
        if let Some(trigram) = std::env::var_os("ZHUYIN_TRIGRAM_LM").map(PathBuf::from)
            && golden.matches("trigram", &trigram.join("lm.bin"))
        {
            let lm2 = NGramLanguageModel::open(&trigram).unwrap();
            assert_eq!(lm2.order(), 3);
            model_sweep(golden, "lm2", &lm2, inputs, true);
        }
        if golden.digests.contains_key("lmtsv.ids") && lm_dir.join("bigram.tsv").exists() {
            let tsv = BigramLanguageModel::open(&lm_dir).unwrap();
            model_sweep(golden, "lmtsv", &tsv, inputs, true);
        }
    });
}

/// Key strings near every syllable (`channelKeys` in the Swift tool).
fn channel_keys(syllables: &[String]) -> Vec<String> {
    let mut result = vec![];
    for syllable in syllables {
        let Some(keys) = key_channel::keys_for_reading(syllable) else { continue };
        let k: Vec<char> = keys.chars().collect();
        result.push(keys.clone());
        let mut deleted = k.clone();
        deleted.remove(k.len() / 2);
        result.push(deleted.iter().collect());
        if k.len() > 1 {
            let mut swapped = k.clone();
            swapped.swap(0, 1);
            result.push(swapped.iter().collect());
        }
        if let Some(&neighbour) = key_channel::neighbours(k[0]).first() {
            let mut replaced = k.clone();
            replaced[0] = neighbour;
            result.push(replaced.iter().collect());
        }
        result.push(format!("u{keys}"));
    }
    result
}

fn channel_line(keys: &str, alternatives: &[ReadingAlternative]) -> String {
    let mut line = keys.to_owned();
    for a in alternatives {
        write!(line, "\t{},{}", a.reading, hex(a.cost)).unwrap();
    }
    line
}

#[test]
fn keyboard_channel_matches_swift() {
    with_inputs(|golden, inputs| {
        let lexicon = MappedLexicon::open(inputs.data.join("lexicon.bin")).unwrap();
        let set = SyllableSet::new(lexicon.syllables());
        let mut codes: Vec<(&u32, &String)> = set.valid_codes().iter().collect();
        codes.sort();
        let lines: Vec<String> = codes.iter().map(|(code, reading)| format!("{code}\t{reading}")).collect();
        golden.check("channel.codes", &lines);
        let syllables = byte_sorted(set.syllables().iter().cloned());
        let keys = channel_keys(&syllables);
        let valid = |code| set.valid_reading(code);
        let lines: Vec<String> =
            keys.iter().map(|k| channel_line(k, &key_channel::alternatives(k, false, |_, _| 0.0, valid))).collect();
        golden.check("channel.alternatives", &lines);
        let discount = |typed: u8, intended: u8| {
            if (u32::from(typed) + u32::from(intended)) % 5 == 0 {
                1.5 * (1.0 + f64::from(typed % 7)).ln()
            } else {
                0.0
            }
        };
        let subset: Vec<&String> = keys.iter().step_by(10).collect();
        let lines: Vec<String> =
            subset.iter().map(|k| channel_line(k, &key_channel::alternatives(k, false, discount, valid))).collect();
        golden.check("channel.discount", &lines);
        let lines: Vec<String> =
            subset.iter().map(|k| channel_line(k, &key_channel::alternatives(k, true, |_, _| 0.0, valid))).collect();
        golden.check("channel.hotspot", &lines);
    });
}

/// Key runs typing lexicon phrases (`decoderRuns` in the Swift tool).
fn decoder_runs(inputs: &Inputs) -> Vec<(&'static str, Vec<Key>)> {
    let timed = |keys: &[char], gap_at: Option<usize>| -> Vec<Key> {
        let mut time = 100.0;
        keys.iter()
            .enumerate()
            .map(|(index, &c)| {
                time += if Some(index) == gap_at { 0.006 } else { 0.12 };
                Key::new(c, Some(time))
            })
            .collect()
    };
    let mut runs = vec![];
    for phrase in inputs.readings.iter().filter(|r| r.contains(' ')).step_by(50) {
        let parts: Vec<String> =
            phrase.split(' ').filter(|p| !p.is_empty()).filter_map(key_channel::keys_for_reading).collect();
        let keys: Vec<char> = parts.concat().chars().collect();
        runs.push(("exact", timed(&keys, None)));
        let implicit: Vec<char> = parts
            .iter()
            .enumerate()
            .map(|(i, p)| if i < parts.len() - 1 { p.strip_suffix(' ').unwrap_or(p) } else { p.as_str() })
            .collect::<String>()
            .chars()
            .collect();
        runs.push(("implicit", timed(&implicit, None)));
        let middle = keys.len() / 2;
        if middle + 1 < keys.len() {
            let mut swapped = keys.clone();
            swapped.swap(middle, middle + 1);
            runs.push(("swapped", timed(&swapped, None)));
            runs.push(("together", timed(&swapped, Some(middle + 1))));
        }
        if keys.len() > 1
            && let Some(&neighbour) = key_channel::neighbours(keys[1]).first()
        {
            let mut extra = keys.clone();
            extra.insert(2, neighbour);
            runs.push(("extra", timed(&extra, None)));
            runs.push(("extra-together", timed(&extra, Some(2))));
        }
    }
    runs
}

fn decode_line(results: &[DecodeResult]) -> String {
    results
        .iter()
        .map(|r| {
            let syllables: Vec<String> =
                r.syllables.iter().map(|s| format!("{}/{}/{}", s.reading, s.keys, u8::from(s.implicit))).collect();
            let buffer: String = r.buffer.iter().map(|s| s.unwrap_or('_')).collect();
            format!("{}|{buffer}|{}|{}", syllables.join(" "), r.buffer_keys, hex(r.cost))
        })
        .collect::<Vec<_>>()
        .join("\t")
}

#[test]
fn key_decoder_matches_swift() {
    with_inputs(|golden, inputs| {
        let lexicon = MappedLexicon::open(inputs.data.join("lexicon.bin")).unwrap();
        let set = SyllableSet::new(lexicon.syllables());
        let cache: RefCell<HashMap<String, Option<f64>>> = RefCell::default();
        let closing = |reading: &str, keys: &str| {
            if set.is_valid(reading) {
                return Some(0.0);
            }
            *cache.borrow_mut().entry(keys.to_owned()).or_insert_with(|| {
                key_channel::alternatives(keys, false, |_, _| 0.0, |code| set.valid_reading(code))
                    .first()
                    .map(|a| a.cost)
            })
        };
        let decoders: Vec<_> =
            KeyDecoderMethod::ALL.iter().map(|&method| KeyDecoder::new(method, Costs::default(), &closing)).collect();
        let mut lines = vec![];
        for (label, keys) in decoder_runs(inputs) {
            let typed: String = keys.iter().map(|k| k.character).collect();
            for (method, decoder) in KeyDecoderMethod::ALL.iter().zip(&decoders) {
                lines.push(format!(
                    "{label}\t{typed}\t{}\t{}",
                    method.raw_value(),
                    decode_line(&decoder.decode(&keys, 6))
                ));
            }
        }
        // Equal-cost states rank in the order first reached, in the Swift engine too.
        golden.check("decoder", &lines);
    });
}

#[test]
fn large_model_maps_every_lexicon_word() {
    // ZhuyinCoreChecks: "150 MB model maps every real lexicon word", "150 MB model load and lexicon ID mapping under
    // 80 ms" (the time is reported, not asserted: debug builds are slower).
    let Some(data) = common::data_directory() else { return };
    let Ok(tsv) = std::fs::read(data.join("lexicon.tsv")) else { return };
    let mut words = BTreeSet::new();
    scan_tsv(&tsv, false, |fields| {
        if fields.len() == 3 {
            words.insert(fields[1].to_string());
        }
    });
    let temp = common::TempDir::new("large");
    std::fs::write(temp.path().join("lm.bin"), common::large_lm_fixture(&words)).unwrap();
    let start = std::time::Instant::now();
    let model = NGramLanguageModel::open(temp.path()).unwrap();
    let resolved = words.iter().filter(|w| model.word_id(w).is_some()).count();
    eprintln!("150 MB synthetic LM + {resolved} lexicon IDs: {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(resolved, words.len());
    assert_eq!(model.order(), 3);
}

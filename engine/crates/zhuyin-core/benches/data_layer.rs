//! Data-layer benchmarks on the real data, the twins of `parity/Sources/SwiftParity/main.swift bench` (same queries,
//! same batches): lexicon lookups, language-model scoring, the keyboard channel and key-run decoding of 10, 25 and
//! 40 syllables.  Needs `ZHUYIN_DATA` and `ZHUYIN_PARITY_TEXT` (see `tests/golden.rs`); `ZHUYIN_TRIGRAM_LM` adds the
//! trigram model.

// `criterion_group!` and `criterion_main!` generate undocumented public items.
#![allow(missing_docs)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use zhuyin_core::api::{ComposerOptions, KeyDecoderMethod, KeyInput, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::conversion::{CompositionToken, Request};
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::key_channel;
use zhuyin_core::key_decoder::{Costs, Key, KeyDecoder};
use zhuyin_core::keyboard;
use zhuyin_core::language_model::MixtureLanguageModel;
use zhuyin_core::syllables::{SyllableSet, without_tones};
use zhuyin_core::tsv::{parse_double, scan_tsv};
use zhuyin_core::{LanguageModel, LanguageModelHistory, MappedLexicon, NGramLanguageModel};

struct Inputs {
    data: PathBuf,
    readings: Vec<String>,
    words: Vec<String>,
    sequences: Vec<Vec<String>>,
}

fn inputs() -> Option<Inputs> {
    let data = PathBuf::from(std::env::var_os("ZHUYIN_DATA")?);
    let text = PathBuf::from(std::env::var_os("ZHUYIN_PARITY_TEXT")?);
    let tsv = std::fs::read(data.join("lexicon.tsv")).ok()?;
    let (mut readings, mut words) = (BTreeSet::new(), BTreeSet::new());
    scan_tsv(&tsv, false, |fields| {
        if fields.len() == 3
            && parse_double(&fields[2]).is_some_and(f64::is_finite)
            && !fields[0].is_empty()
            && !fields[1].is_empty()
        {
            readings.insert(fields[0].to_string());
            words.insert(fields[1].to_string());
        }
    });
    let known: HashSet<&str> = words.iter().map(String::as_str).collect();
    let content = std::fs::read_to_string(text).ok()?;
    let mut sequences = vec![];
    for line in content.split('\n').filter(|line| !line.is_empty()).take(2000) {
        let scalars: Vec<char> = line.chars().collect();
        let mut sequence = vec![];
        let mut i = 0;
        while i < scalars.len() {
            for length in (1..=6.min(scalars.len() - i)).rev() {
                let word: String = scalars[i..i + length].iter().collect();
                if length == 1 || known.contains(word.as_str()) {
                    sequence.push(word);
                    i += length;
                    break;
                }
            }
        }
        if !sequence.is_empty() {
            sequences.push(sequence);
        }
    }
    Some(Inputs { data, readings: readings.into_iter().collect(), words: words.into_iter().collect(), sequences })
}

fn scoring_batch(model: &dyn LanguageModel, sequences: &[Vec<String>]) -> Vec<(Option<u32>, LanguageModelHistory)> {
    let mut batch = vec![];
    for sequence in sequences.iter().take(200) {
        let ids: Vec<Option<u32>> = sequence.iter().map(|w| model.word_id(w)).collect();
        for j in 0..ids.len() {
            let older = match j {
                0 => None,
                1 => Some(1),
                _ => Some(ids[j - 2].unwrap_or(2)),
            };
            batch.push((ids[j], LanguageModelHistory::new(older, if j == 0 { 1 } else { ids[j - 1].unwrap_or(2) })));
        }
    }
    batch
}

/// Key strings near every syllable (`channelKeys`).
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

/// Runs of `count` syllables typing the text's words, each by its most common reading (`syllableRuns`).
fn syllable_runs(inputs: &Inputs, lexicon: &MappedLexicon, count: usize, runs: usize) -> Vec<Vec<Key>> {
    let mut syllables: Vec<String> = vec![];
    for sequence in &inputs.sequences {
        for word in sequence {
            if let Some((reading, _)) = lexicon.best_reading(word) {
                syllables.extend(reading.split(' ').filter(|s| !s.is_empty()).map(str::to_owned));
            }
        }
        if syllables.len() > count * runs * 2 {
            break;
        }
    }
    (0..runs)
        .map(|run| {
            let mut time = 100.0;
            syllables[run * count..run * count + count]
                .iter()
                .flat_map(|s| key_channel::keys_for_reading(s).unwrap_or_default().chars().collect::<Vec<_>>())
                .map(|c| {
                    time += 0.12;
                    Key::new(c, Some(time))
                })
                .collect()
        })
        .collect()
}

fn benchmarks(c: &mut Criterion) {
    let Some(inputs) = inputs() else {
        eprintln!("skipped: set ZHUYIN_DATA and ZHUYIN_PARITY_TEXT");
        return;
    };
    let lexicon_path = inputs.data.join("lexicon.bin");
    let lexicon = MappedLexicon::open(&lexicon_path).unwrap();
    let readings: Vec<&String> = inputs.readings.iter().step_by(50).collect();
    let words: Vec<&String> = inputs.words.iter().step_by(50).collect();
    c.bench_function(&format!("lexicon.entries (batch of {})", readings.len()), |b| {
        b.iter(|| {
            for r in &readings {
                black_box(lexicon.entries(r));
            }
        });
    });
    c.bench_function(&format!("lexicon.entry_refs zero-copy (batch of {})", readings.len()), |b| {
        b.iter(|| {
            for r in &readings {
                black_box(lexicon.entry_refs(r).count());
            }
        });
    });
    c.bench_function(&format!("lexicon.word_index (batch of {})", words.len()), |b| {
        b.iter(|| {
            for w in &words {
                black_box(lexicon.word_index(w));
            }
        });
    });
    c.bench_function(&format!("lexicon.contains_prefix (batch of {})", readings.len()), |b| {
        b.iter(|| {
            for r in &readings {
                black_box(lexicon.contains_prefix(&without_tones(r)));
            }
        });
    });
    c.bench_function("lexicon.open", |b| b.iter(|| black_box(MappedLexicon::open(&lexicon_path).unwrap())));
    model_benchmarks(c, &inputs);
    conversion_benchmarks(c, &inputs, &lexicon);
    decoder_benchmarks(c, &inputs, &lexicon);
}

/// Syllable tokens of a key run, as the composer builds them (slots, then the tone key or Space).
fn run_tokens(run: &[Key]) -> Vec<CompositionToken> {
    let mut tokens = vec![];
    let mut keys = String::new();
    for key in run {
        keys.push(key.character);
        let tone = keyboard::tone(key.character);
        if tone.is_some() || key.character == ' ' {
            let mut slots: [Option<char>; 3] = [None; 3];
            for k in keys.chars() {
                if let Some(symbol) = keyboard::symbol(k)
                    && let Some(slot) = keyboard::slot(symbol)
                {
                    slots[slot] = Some(symbol);
                }
            }
            let reading: String = slots.iter().flatten().collect::<String>() + tone.unwrap_or("");
            tokens.push(CompositionToken::syllable(&reading, &keys, false));
            keys.clear();
        }
    }
    tokens
}

/// Conversion as the composer runs it while typing at the end: one conversion per syllable with the cache, and the
/// 16 best sentences the reranker chooses among (the app's engine: mapped lexicon, mixture with the chat model).
fn conversion_benchmarks(c: &mut Criterion, inputs: &Inputs, lexicon: &MappedLexicon) {
    let config = EngineConfig { chat_model_directory: Some(inputs.data.join("lm_chat")), ..EngineConfig::default() };
    let engine = std::sync::Arc::new(
        Engine::open_with_model(&inputs.data.join("lexicon.tsv"), None, Some(&inputs.data.join("lm")), config).unwrap(),
    );
    for count in [10, 25, 40] {
        let runs: Vec<Vec<CompositionToken>> =
            syllable_runs(inputs, lexicon, count, 10).iter().map(|run| run_tokens(run)).collect();
        c.bench_function(
            &format!("convert per syllable typed, {count} syllables (batch of {} runs)", runs.len()),
            |b| {
                b.iter(|| {
                    for tokens in &runs {
                        let mut cache = None;
                        for end in 1..=tokens.len() {
                            black_box(
                                engine.convert(&Request::new(&tokens[..end], &[], TypoTolerance::Standard), &mut cache),
                            );
                        }
                    }
                });
            },
        );
        let key_runs = syllable_runs(inputs, lexicon, count, 10);
        let keys: usize = key_runs.iter().map(Vec::len).sum();
        let mut composer = Composer::new(engine.clone(), ComposerOptions::default());
        c.bench_function(
            &format!("composer typing, {count} syllables (batch of {} runs, {keys} keys)", key_runs.len()),
            |b| {
                b.iter(|| {
                    for run in &key_runs {
                        composer.reset();
                        for key in run {
                            let input =
                                if key.character == ' ' { KeyInput::Space } else { KeyInput::Char(key.character) };
                            black_box(composer.handle_at(input, key.time.unwrap_or(0.0)));
                        }
                    }
                });
            },
        );
        c.bench_function(&format!("16-best, {count} syllables (batch of {} runs)", runs.len()), |b| {
            b.iter(|| {
                for tokens in &runs {
                    black_box(engine.sentences(&Request::new(tokens, &[], TypoTolerance::Standard), 16));
                }
            });
        });
    }
}

fn model_benchmarks(c: &mut Criterion, inputs: &Inputs) {
    let lm_dir = inputs.data.join("lm");
    let lm = NGramLanguageModel::open(&lm_dir).unwrap();
    let chat_dir = inputs.data.join("lm_chat");
    let mixture = MixtureLanguageModel::new(
        NGramLanguageModel::open(&chat_dir).unwrap(),
        NGramLanguageModel::open(&lm_dir).unwrap(),
        0.3,
    );
    let trigram = std::env::var_os("ZHUYIN_TRIGRAM_LM").and_then(|path| NGramLanguageModel::open(path).ok());
    let mut models: Vec<(&str, &dyn LanguageModel)> = vec![("lm", &lm), ("mixture", &mixture)];
    if let Some(trigram) = &trigram {
        models.push(("lm2-trigram", trigram));
    }
    for (name, model) in models {
        let batch = scoring_batch(model, &inputs.sequences);
        c.bench_function(&format!("score {name} (batch of {})", batch.len()), |b| {
            b.iter(|| {
                let mut total = 0.0;
                for &(id, history) in &batch {
                    total += model.score(id, history, -9.25);
                }
                black_box(total)
            });
        });
    }
    let id_words: Vec<&String> = inputs.sequences.iter().take(200).flatten().collect();
    c.bench_function(&format!("lm.word_id (batch of {})", id_words.len()), |b| {
        b.iter(|| {
            for w in &id_words {
                black_box(lm.word_id(w));
            }
        });
    });
    c.bench_function("lm.open", |b| b.iter(|| black_box(NGramLanguageModel::open(&lm_dir).unwrap())));
    c.bench_function("mixture.new", |b| {
        b.iter(|| {
            black_box(MixtureLanguageModel::new(
                NGramLanguageModel::open(&chat_dir).unwrap(),
                NGramLanguageModel::open(&lm_dir).unwrap(),
                0.3,
            ))
        });
    });
}

fn decoder_benchmarks(c: &mut Criterion, inputs: &Inputs, lexicon: &MappedLexicon) {
    let set = SyllableSet::new(lexicon.syllables());
    let syllables: BTreeSet<String> = set.syllables().iter().cloned().collect();
    let keys: Vec<String> = channel_keys(&syllables.into_iter().collect::<Vec<_>>()).into_iter().step_by(20).collect();
    c.bench_function(&format!("channel.alternatives (batch of {})", keys.len()), |b| {
        b.iter(|| {
            for k in &keys {
                black_box(key_channel::alternatives(k, false, |_, _| 0.0, |code| set.valid_reading(code)));
            }
        });
    });
    // The engine's closing cost: a valid syllable is free, otherwise the cached cheapest correction (under a lock, as
    // the Swift engine's `alternatives(for:)`).
    let cache: Mutex<HashMap<String, Option<f64>>> = Mutex::default();
    let closing = |reading: &str, keys: &str| {
        if set.is_valid(reading) {
            return Some(0.0);
        }
        let mut cache = cache.lock().unwrap();
        if let Some(&cost) = cache.get(keys) {
            return cost;
        }
        let cost =
            key_channel::alternatives(keys, false, |_, _| 0.0, |code| set.valid_reading(code)).first().map(|a| a.cost);
        cache.insert(keys.to_owned(), cost);
        cost
    };
    let decoder = KeyDecoder::new(KeyDecoderMethod::Timed, Costs::default(), closing);
    for count in [10, 25, 40] {
        let runs = syllable_runs(inputs, lexicon, count, 10);
        let total: usize = runs.iter().map(Vec::len).sum();
        c.bench_function(
            &format!("decode timed {count} syllables (batch of {} runs, {total} keys)", runs.len()),
            |b| {
                b.iter(|| {
                    for run in &runs {
                        black_box(decoder.decode(run, 6));
                    }
                });
            },
        );
    }
}

fn configured() -> Criterion {
    Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
}

criterion_group! {
    name = data_layer;
    config = configured();
    targets = benchmarks
}
criterion_main!(data_layer);

//! Golden tests of the engine against the Swift engine's digests (its tool's `EngineSweeps.swift`, see `golden.rs`):
//! Swift's number formatting, conversions of dev-set key scripts (best, 16-best with scores, incremental typing with
//! the cache, fixed words and groups, character scores, the text before the cursor), and a learning session with
//! its personal data files, export and import.  Needs `ZHUYIN_PARITY_DEV` (colloquial:encyclopedic:chat dev sets)
//! besides the inputs of `golden.rs`.

mod common;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::parity::{Golden, Inputs, hex, with_inputs};
use zhuyin_core::LanguageModelHistory;
use zhuyin_core::api::{
    CandidateAnchor, CharacterForms, ComposerOptions, FuzzySounds, KeyDecoderMethod, KeyInput, Presentation,
    PunctuationKeys, ScoredSentence, SpaceAction, TypoTolerance,
};
use zhuyin_core::composer::Composer;
use zhuyin_core::conversion::{CharacterScores, CompositionToken, ConversionCache, ConvertedWord, FixedWord, Request};
use zhuyin_core::engine::{CommittedWord, Engine, EngineConfig};
use zhuyin_core::key_channel;
use zhuyin_core::keyboard;
use zhuyin_core::swift_format::{format_double, graphemes};
use zhuyin_core::tsv::{parse_double, scan_tsv};

/// The 64-bit LCG of the Swift tool.
struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

#[test]
fn double_descriptions_match_swift() {
    let Some(golden) = Golden::load() else { return };
    let mut generator = Generator(7);
    let mut lines = vec![];
    for i in 0..60_000 {
        let bits = generator.next();
        let value = match i % 6 {
            0 => f64::from_bits(bits),
            1 => (bits >> 11) as f64 / 1000.0,
            2 => 1_700_000_000.0 + (bits >> 40) as f64 / 1000.0,
            3 => (bits >> 30) as f64 * (-((bits % 64) as f64)).exp2(),
            4 => {
                (bits % 100_000) as f64 / 100_000.0 * format!("1e{}", (bits >> 58) as i32 - 20).parse::<f64>().unwrap()
            }
            _ => 0.05 * (bits % 40) as f64 * (-((bits >> 54) as f64 / 3600.0)).exp2(),
        };
        if value.is_finite() {
            lines.push(format!("{}\t{}", hex(value), format_double(value)));
        }
    }
    golden.check("format.doubles", &lines);
}

fn dev_files(golden: &Golden) -> Option<Vec<PathBuf>> {
    let files: Vec<PathBuf> = std::env::var("ZHUYIN_PARITY_DEV").ok()?.split(':').map(PathBuf::from).collect();
    files.iter().enumerate().all(|(i, f)| golden.matches(&format!("dev{i}"), f)).then_some(files)
}

/// Lines split at LF (CR LF does not split, as Swift's `Character`), empty lines dropped.
fn swift_split_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let (mut lines, mut start) = (vec![], 0);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && !(i > 0 && bytes[i - 1] == b'\r') {
            lines.push(&text[start..i]);
            start = i + 1;
        }
    }
    lines.push(&text[start..]);
    lines.into_iter().filter(|l| !l.is_empty()).collect()
}

/// The dev-set Han runs (4–20 characters) with the text before them (`devRuns`).
fn dev_runs(path: &Path, limit: usize) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
    let han = |c: char| ('\u{4E00}'..='\u{9FFF}').contains(&c);
    let mut result = vec![];
    for line in swift_split_lines(&text) {
        let scalars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < scalars.len() {
            if !han(scalars[i]) {
                i += 1;
                continue;
            }
            let mut end = i;
            while end < scalars.len() && end - i < 20 && han(scalars[end]) {
                end += 1;
            }
            if end - i >= 4 {
                result.push((scalars[i..end].iter().collect(), scalars[i.saturating_sub(32)..i].iter().collect()));
                if result.len() >= limit {
                    return result;
                }
            }
            i = end;
        }
    }
    result
}

fn best_readings(tsv: &[u8]) -> HashMap<String, (String, f64)> {
    let mut best: HashMap<String, (String, f64)> = HashMap::new();
    scan_tsv(tsv, false, |fields| {
        if fields.len() != 3 || fields[0].is_empty() || fields[1].is_empty() {
            return;
        }
        let Some(score) = parse_double(&fields[2]).filter(|s| s.is_finite()) else { return };
        if best.get(fields[1].as_ref()).is_some_and(|old| old.1 >= score) {
            return;
        }
        best.insert(fields[1].to_string(), (fields[0].to_string(), score));
    });
    best
}

fn run_tokens(run: &str, best: &HashMap<String, (String, f64)>) -> Option<Vec<CompositionToken>> {
    let scalars: Vec<char> = run.chars().collect();
    let mut readings: Vec<String> = vec![];
    let mut i = 0;
    'outer: while i < scalars.len() {
        for length in (1..=6.min(scalars.len() - i)).rev() {
            let piece: String = scalars[i..i + length].iter().collect();
            if let Some(reading) = best.get(&piece) {
                readings.extend(reading.0.split(' ').filter(|p| !p.is_empty()).map(str::to_owned));
                i += length;
                continue 'outer;
            }
        }
        return None;
    }
    readings
        .iter()
        .map(|r| key_channel::keys_for_reading(r).map(|keys| CompositionToken::syllable(r, &keys, false)))
        .collect()
}

fn typo_tokens(tokens: &[CompositionToken]) -> Option<Vec<CompositionToken>> {
    let middle = tokens.len() / 2;
    let CompositionToken::Syllable { keys, .. } = &tokens[middle] else { return None };
    let first = keys.chars().next()?;
    let neighbour = *key_channel::neighbours(first).first()?;
    let typed: String = std::iter::once(neighbour).chain(keys.chars().skip(1)).collect();
    let mut slots: [Option<char>; 3] = [None; 3];
    let mut tone = "";
    for key in typed.chars() {
        if let Some(symbol) = keyboard::symbol(key)
            && let Some(slot) = keyboard::slot(symbol)
        {
            slots[slot] = Some(symbol);
        } else if let Some(mark) = keyboard::tone(key) {
            tone = mark;
        }
    }
    if slots.iter().all(Option::is_none) {
        return None;
    }
    let reading: String = slots.iter().flatten().collect::<String>() + tone;
    let mut result = tokens.to_vec();
    result[middle] = CompositionToken::syllable(&reading, &typed, false);
    Some(result)
}

fn history_text(history: Option<LanguageModelHistory>) -> String {
    history.map_or("-".to_owned(), |h| format!("{},{}", h.older.map_or("-".to_owned(), |o| o.to_string()), h.previous))
}

fn words_line(words: &[ConvertedWord]) -> String {
    words
        .iter()
        .map(|w| {
            format!(
                "{}/{}/{}-{}/{}/{}/{}",
                w.text,
                w.reading,
                w.range.start,
                w.range.end,
                w.word_id.map_or("-".to_owned(), |id| id.to_string()),
                history_text(w.history),
                u8::from(w.corrected)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentences_line(sentences: &[ScoredSentence]) -> String {
    sentences
        .iter()
        .map(|s| {
            let ranges: Vec<String> = s.words.iter().map(|w| format!("{}-{}", w.range.start, w.range.end)).collect();
            format!("{},{},{},{}", s.text(), hex(s.score), u8::from(s.corrected()), ranges.join("."))
        })
        .collect::<Vec<_>>()
        .join("\t")
}

fn synthetic_characters(sentences: &[ScoredSentence], count: usize) -> CharacterScores {
    let mut positions: Vec<HashMap<String, f64>> = vec![HashMap::new(); count];
    for (rank, sentence) in sentences.iter().enumerate() {
        for word in sentence.words.iter().filter(|w| !w.reading.is_empty()) {
            for (offset, character) in graphemes(&word.text).into_iter().enumerate() {
                if word.range.start + offset < count {
                    positions[word.range.start + offset]
                        .entry(character.to_owned())
                        .or_insert(-0.25 - rank as f64 / 4.0);
                }
            }
        }
    }
    CharacterScores::new(positions, 0.5)
}

fn app_config(data: &Path) -> EngineConfig {
    EngineConfig { chat_model_directory: Some(data.join("lm_chat")), chat_prior: 0.3, ..EngineConfig::default() }
}

fn app_engine(data: &Path) -> Engine {
    Engine::open_with_model(&data.join("lexicon.tsv"), None, Some(&data.join("lm")), app_config(data)).unwrap()
}

type Script = (usize, String, String, Vec<CompositionToken>);

fn dev_scripts(inputs: &Inputs, files: &[PathBuf], limit: usize) -> Vec<Script> {
    let best = best_readings(&inputs.tsv);
    let mut result: Vec<Script> = vec![];
    for (index, file) in files.iter().enumerate() {
        let mut taken = 0;
        for (run, before) in dev_runs(file, limit * 2) {
            let Some(tokens) = run_tokens(&run, &best) else { continue };
            result.push((index, run, before, tokens));
            taken += 1;
            if taken >= limit {
                break;
            }
        }
    }
    result
}

fn request<'a>(tokens: &'a [CompositionToken], fixed: &'a [FixedWord], start: LanguageModelHistory) -> Request<'a> {
    let mut request = Request::new(tokens, fixed, TypoTolerance::Standard);
    request.start = start;
    request
}

#[test]
// One test per Swift sweep function, step for step, so the two stay comparable.
#[allow(clippy::too_many_lines)]
fn conversions_match_swift() {
    with_inputs(|golden, inputs| {
        let Some(files) = dev_files(golden) else { return };
        let engine = app_engine(&inputs.data);
        let (mut plain, mut nbest, mut incremental, mut fixed_lines, mut character_lines) =
            (vec![], vec![], vec![], vec![], vec![]);
        for (index, (file, run, before, tokens)) in dev_scripts(inputs, &files, 150).into_iter().enumerate() {
            let context = if file == 1 { before.as_str() } else { "" };
            let weight = engine.adapt_domain(context);
            let start = engine.trailing_words(context).last().and_then(|w| w.history).unwrap_or_default();
            let mut variants = vec![("plain", tokens.clone())];
            if index % 3 == 1
                && let Some(typo) = typo_tokens(&tokens)
            {
                variants.push(("typo", typo));
            }
            for (label, tokens) in variants {
                let head = format!("{file}\t{run}\t{label}\t{}\t", weight.map_or("-".to_owned(), hex));
                let best = engine.convert(&request(&tokens, &[], start), &mut None);
                plain.push(head.clone() + &words_line(&best));
                let sentences = engine.sentences(&request(&tokens, &[], start), 16);
                nbest.push(head.clone() + &sentences_line(&sentences));
                let mut steps = vec![];
                let mut growing: Option<ConversionCache> = None;
                for count in 1..=tokens.len() {
                    let words = engine.convert(&request(&tokens[..count], &[], start), &mut growing);
                    steps.push(words.iter().map(|w| &*w.text).collect::<String>());
                }
                if tokens.len() > 3 {
                    let mut edited = tokens.clone();
                    edited.remove(1);
                    for t in [&edited, &tokens] {
                        let words = engine.convert(&request(t, &[], start), &mut growing);
                        steps.push(words.iter().map(|w| &*w.text).collect::<String>());
                    }
                }
                incremental.push(head.clone() + &steps.join("|"));
                if best.len() >= 2 {
                    let word = &best[1];
                    let fixed = vec![FixedWord {
                        range: word.range.clone(),
                        text: word.text.clone(),
                        reading: word.reading.clone(),
                        word_id: word.word_id,
                        annotation: None,
                        components: vec![],
                        frozen: false,
                    }];
                    let mut cache = None;
                    let mut line =
                        head.clone() + &words_line(&engine.convert(&request(&tokens, &fixed, start), &mut cache));
                    if tokens.len() > 2 {
                        let mut grouped = request(&tokens, &fixed, start);
                        grouped.group = Some(1..tokens.len() - 1);
                        write!(line, "\t{}", words_line(&engine.convert(&grouped, &mut cache))).unwrap();
                    }
                    write!(line, "\t{}", sentences_line(&engine.sentences(&request(&tokens, &fixed, start), 8)))
                        .unwrap();
                    fixed_lines.push(line);
                }
                let characters = synthetic_characters(&sentences, tokens.len());
                let mut scored = request(&tokens, &[], start);
                scored.characters = Some(&characters);
                character_lines.push(format!(
                    "{head}{}\t{}",
                    words_line(&engine.convert(&scored, &mut None)),
                    sentences_line(&engine.sentences(&scored, 8))
                ));
            }
        }
        golden.check("conversion.best", &plain);
        let mut option_lines = vec![];
        engine.adapt_domain("");
        let all = FuzzySounds::Z_ZH | FuzzySounds::C_CH | FuzzySounds::S_SH | FuzzySounds::EN_ENG | FuzzySounds::AN_ANG;
        for (index, (_, run, _, tokens)) in dev_scripts(inputs, &files, 150).into_iter().enumerate() {
            if index % 5 != 2 {
                continue;
            }
            let mut line = run.clone();
            for (fuzzy, typo) in [
                (all, TypoTolerance::Standard),
                (FuzzySounds::NONE, TypoTolerance::Off),
                (FuzzySounds::NONE, TypoTolerance::Aggressive),
                (FuzzySounds::Z_ZH | FuzzySounds::EN_ENG, TypoTolerance::Aggressive),
            ] {
                engine.set_fuzzy_sounds(fuzzy);
                let request = Request::new(&tokens, &[], typo);
                write!(
                    line,
                    "\t{}\t{}",
                    words_line(&engine.convert(&request, &mut None)),
                    sentences_line(&engine.sentences(&request, 4))
                )
                .unwrap();
            }
            engine.set_fuzzy_sounds(FuzzySounds::NONE);
            option_lines.push(line);
        }
        golden.check("conversion.options", &option_lines);
        let text = Engine::from_text(std::str::from_utf8(&inputs.tsv).unwrap(), None, None, EngineConfig::default());
        let mut text_lines = vec![];
        for (_, run, _, tokens) in dev_scripts(inputs, &files, 40) {
            let plain = Request::new(&tokens, &[], TypoTolerance::Standard);
            let prefix: String = graphemes(&run).into_iter().take(2).collect();
            text_lines.push(format!(
                "{run}\t{}\t{}\t{}",
                words_line(&text.convert(&plain, &mut None)),
                sentences_line(&text.sentences(&plain, 8)),
                text.readings(&prefix).join(",")
            ));
        }
        golden.check("conversion.text", &text_lines);
        golden.check("conversion.nbest", &nbest);
        golden.check("conversion.incremental", &incremental);
        golden.check("conversion.fixed", &fixed_lines);
        golden.check("conversion.characters", &character_lines);
    });
}

fn split_keep_empty(text: &str) -> Vec<String> {
    text.split('\n').map(str::to_owned).collect()
}

#[test]
// One test per Swift sweep function, step for step, so the two stay comparable.
#[allow(clippy::too_many_lines)]
fn learning_matches_swift() {
    with_inputs(|golden, inputs| {
        let Some(files) = dev_files(golden) else { return };
        let directory = std::env::temp_dir().join(format!("zhuyin-rust-parity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let data = &inputs.data;
        let engine = Engine::open_with_model(
            &data.join("lexicon.tsv"),
            Some(&directory),
            Some(&data.join("lm")),
            app_config(data),
        )
        .unwrap();
        let step = Arc::new(Mutex::new(0.0f64));
        let clock = step.clone();
        engine.set_clock(move || 1_800_000_000.0 + *clock.lock().unwrap() * 5.0 * 3600.0);
        let mut lines: Vec<String> = vec![];
        let mut context: Vec<CommittedWord> = vec![];
        for (index, (_, run, _, tokens)) in dev_scripts(inputs, &files, 60).into_iter().enumerate() {
            *step.lock().unwrap() = index as f64;
            let plain = request(&tokens, &[], LanguageModelHistory::default());
            let before = engine.convert(&plain, &mut None);
            let sentences = engine.sentences(&plain, 2);
            let chosen: Vec<(std::ops::Range<usize>, String, String, bool)> = if index % 4 == 0 && sentences.len() > 1 {
                sentences[1]
                    .words
                    .iter()
                    .map(|w| {
                        let kept = before.iter().any(|b| b.range == w.range && *b.text == *w.text);
                        (w.range.clone(), w.text.clone(), w.reading.clone(), !kept)
                    })
                    .collect()
            } else {
                before.iter().map(|w| (w.range.clone(), w.text.to_string(), w.reading.to_string(), false)).collect()
            };
            let committed: Vec<CommittedWord> = chosen
                .into_iter()
                .map(|(range, text, reading, picked)| {
                    let word_id = before.iter().find(|b| b.range == range && *b.text == *text).and_then(|b| b.word_id);
                    CommittedWord { range, text, reading, word_id, picked }
                })
                .collect();
            engine.learn(&committed, tokens.len(), &context);
            context = committed[committed.len().saturating_sub(2)..]
                .iter()
                .map(|w| CommittedWord { range: 0..0, picked: false, ..w.clone() })
                .collect();
            if index % 7 == 3
                && let Some(word) = before.first()
            {
                engine.weaken(&word.text, &word.reading, word.word_id, Some(LanguageModelHistory::default()));
            }
            if index % 11 == 5 && tokens.len() >= 2 {
                let reading = tokens[..2].iter().filter_map(CompositionToken::reading).collect::<Vec<_>>().join(" ");
                let word: String = graphemes(&run).into_iter().take(2).collect();
                let _ = engine.add_user_phrase(&word, &reading);
            }
            if index % 13 == 6
                && let CompositionToken::Syllable { reading, .. } = &tokens[0]
                && let Some(slip) = typo_tokens(&tokens[..1])
                && let CompositionToken::Syllable { keys: typed, .. } = &slip[0]
            {
                engine.record_typos(&[(typed.to_string(), reading.to_string())]);
            }
            let converted = engine.convert(&plain, &mut None);
            let mut history = LanguageModelHistory::default();
            let remembered: String = converted
                .iter()
                .map(|w| {
                    let r = if engine.remembers(w, history) { '1' } else { '0' };
                    history = w.history.unwrap_or(history);
                    r
                })
                .collect();
            lines.push(format!("{run}\t{}\t{remembered}\t{}", words_line(&converted), engine.learned_entries().len()));
            if let Some(typo) = typo_tokens(&tokens)
                && let CompositionToken::Syllable { keys, .. } = &typo[tokens.len() / 2]
            {
                let alternatives: Vec<String> =
                    engine.typo_alternatives(keys).iter().map(|(r, c)| format!("{r},{}", hex(*c))).collect();
                lines.push(format!("typo\t{}", alternatives.join(" ")));
            }
        }
        lines.extend(engine.learned_entries().iter().map(|(k, c)| format!("{}\t{}\t{}", k.reading, k.word, hex(*c))));
        lines.extend(engine.readings("什麼"));
        lines.extend(engine.readings("不存在字"));
        lines.extend(engine.user_phrases().iter().map(|k| format!("{}\t{}", k.reading, k.word)));
        golden.check("learning.session", &lines);
        engine.flush_personal_data();
        let mut file_lines = vec![];
        for name in ["user_phrases.tsv", "learned.tsv", "excluded.tsv", "typos.tsv", "personal_ngrams.tsv"] {
            file_lines.push(format!("== {name}"));
            match std::fs::read_to_string(directory.join(name)) {
                Ok(text) => file_lines.extend(split_keep_empty(&text)),
                Err(_) => file_lines.push("(missing)".to_owned()),
            }
        }
        golden.check("learning.files", &file_lines);
        let export = engine.export_personal_data();
        golden.check("learning.export", &split_keep_empty(&export));
        let fresh = app_engine(data);
        fresh.set_clock(|| 1_800_000_000.0 + 100.0 * 5.0 * 3600.0);
        let rows = fresh.import_personal_data(export.as_bytes()).map_or("-1".to_owned(), |r| r.to_string());
        let mut imported = vec![rows];
        imported.extend(split_keep_empty(&fresh.export_personal_data()));
        golden.check("learning.import", &imported);
        drop(engine);
        let reloaded = Engine::open_with_model(
            &data.join("lexicon.tsv"),
            Some(&directory),
            Some(&data.join("lm")),
            EngineConfig::default(),
        )
        .unwrap();
        reloaded.set_clock(|| 1_800_000_000.0 + 100.0 * 5.0 * 3600.0);
        let reload: Vec<String> =
            reloaded.learned_entries().iter().map(|(k, c)| format!("{}\t{}\t{}", k.reading, k.word, hex(*c))).collect();
        golden.check("learning.reload", &reload);
        let _ = std::fs::remove_dir_all(&directory);
    });
}

/// zhuyin-cli's key syntax (`parseKeys` in the Swift tool): a key with its label, or a delay in seconds.
fn parse_keys(input: &str) -> Vec<(String, Option<KeyInput>, Option<f64>)> {
    let special = |name: &str| -> Option<KeyInput> {
        Some(match name {
            "space" => KeyInput::Space,
            "bs" => KeyInput::Backspace,
            "obs" => KeyInput::OptionBackspace,
            "del" => KeyInput::ForwardDelete,
            "left" => KeyInput::Left,
            "right" => KeyInput::Right,
            "home" => KeyInput::Home,
            "end" => KeyInput::End,
            "up" => KeyInput::Up,
            "down" => KeyInput::Down,
            "enter" => KeyInput::Enter,
            "senter" => KeyInput::ShiftEnter,
            "esc" => KeyInput::Escape,
            "tab" => KeyInput::Tab,
            "stab" => KeyInput::ShiftTab,
            "sspace" => KeyInput::ShiftSpace,
            "pgup" => KeyInput::PageUp,
            "pgdown" => KeyInput::PageDown,
            "sleft" => KeyInput::ShiftLeft,
            "sright" => KeyInput::ShiftRight,
            "sdown" => KeyInput::ShiftDown,
            "sup" => KeyInput::ShiftUp,
            _ => return None,
        })
    };
    let chars: Vec<char> = input.chars().collect();
    let mut result = vec![];
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if c == '<'
            && let Some(end) = chars[index..].iter().position(|&x| x == '>').map(|p| p + index)
        {
            let name: String = chars[index + 1..end].iter().collect();
            let last = name.chars().last().unwrap_or(' ');
            let single = name.chars().count() == 3;
            if let Some(key) = special(&name) {
                result.push((format!("<{name}>"), Some(key), None));
            } else if name.starts_with("S-") && single {
                result.push((format!("<{name}>"), Some(KeyInput::ShiftedChar(last)), None));
            } else if name.starts_with("O-") && single {
                result.push((format!("<{name}>"), Some(KeyInput::Printed(last)), None));
            } else if name.starts_with("C-") && single {
                result.push((format!("<{name}>"), Some(KeyInput::ControlChar(last)), None));
            } else if let Some(ms) = name.strip_prefix("dt:").and_then(parse_double) {
                result.push(("dt".to_owned(), None, Some(ms / 1000.0)));
            } else {
                panic!("Unknown key escape <{name}>");
            }
            index = end + 1;
        } else {
            let key = if c == ' ' { KeyInput::Space } else { KeyInput::Char(c) };
            result.push((if c == ' ' { "<space>".to_owned() } else { c.to_string() }, Some(key), None));
            index += 1;
        }
    }
    result
}

fn presentation_line(p: &Presentation) -> String {
    let clauses: Vec<String> = p.clauses.iter().map(|c| format!("{}-{}", c.start, c.end)).collect();
    let mut parts = vec![
        p.text.clone(),
        p.cursor.to_string(),
        clauses.join(","),
        p.focused_clause.map_or("-".to_owned(), |i| i.to_string()),
        p.target_clause.map_or("-".to_owned(), |i| i.to_string()),
    ];
    match &p.candidates {
        Some(list) => {
            let tolerant = list.tolerant.map_or("-", |t| if t { "1" } else { "0" });
            let items: Vec<String> = list
                .items
                .iter()
                .map(|i| format!("{}{}", i.text, i.annotation.as_ref().map_or(String::new(), |a| format!("*{a}"))))
                .collect();
            parts.push(format!(
                "{}/{}/{tolerant}/{}/{}",
                list.selection,
                list.page_size,
                u8::from(list.single_character),
                items.join(",")
            ));
        }
        None => parts.push("-".to_owned()),
    }
    parts.push(p.notice.clone().unwrap_or_else(|| "-".to_owned()));
    parts.push(p.zhuyin_hint.clone().unwrap_or_else(|| "-".to_owned()));
    parts.join("|")
}

fn token_keys(tokens: &[CompositionToken]) -> String {
    tokens
        .iter()
        .map(|t| match t {
            CompositionToken::Syllable { keys, .. } => keys.to_string(),
            CompositionToken::Literal(_) => String::new(),
        })
        .collect()
}

/// `composerScripts` of the Swift tool.
fn composer_scripts(inputs: &Inputs, files: &[PathBuf]) -> Vec<(String, String)> {
    let endings = [
        "<enter>",
        "<home><down><down><enter><enter>",
        "<down><sup><down><enter><enter>",
        "<enter><bs><bs>{keys}<enter>",
        "<S-,><C-.>18 <enter>",
        "<sleft><sleft><enter><enter>",
        "<left><down><right><down><enter><esc><esc>",
        "<home><sdown><down><enter><end><bs><obs><enter>",
        "<down><space><tab><stab><pgdown><pgup><up>`<enter>",
        "``<enter>",
        "<left>`<esc><enter>",
    ];
    let mut scripts = vec![];
    for (index, (file, _, before, tokens)) in dev_scripts(inputs, files, 60).into_iter().enumerate() {
        let keys = token_keys(&tokens);
        let context = if file == 1 { before } else { String::new() };
        scripts.push((context.clone(), keys.clone() + &endings[index % endings.len()].replace("{keys}", &keys)));
        if index % 3 == 0
            && let Some(typo) = typo_tokens(&tokens)
        {
            scripts.push((context.clone(), token_keys(&typo) + "<down><sup><enter>"));
        }
        if index % 4 == 1 && keys.chars().count() > 3 {
            let mut swapped: Vec<char> = keys.chars().collect();
            swapped.swap(1, 2);
            let head: String = swapped[..2].iter().collect();
            let tail: String = swapped[2..].iter().collect();
            scripts.push((context, format!("{head}<dt:8>{tail}<enter>")));
        }
    }
    let targeted = [
        "18 q8 a8 <left><left><bs>18 <home><del><end><bs><S-,>18 ek4`<bs><enter>".to_owned(),
        "18 q8 ``<enter>".to_owned(),
        "18 q8 `<esc>a8 <enter>".to_owned(),
        "18 q8`<esc> <enter>".to_owned(),
        "18 q8 <down>``<enter>".to_owned(),
        "18 q8 `a8 <esc><esc>".to_owned(),
        "su3cl3`<bs>`<esc><enter>".to_owned(),
        "18 q8 <down>`<esc><enter>".to_owned(),
        "su3cl38 <sleft><sleft><enter><end>18 <enter>".to_owned(),
        "su3cl38 <left><sleft><sright><sright><esc><enter>".to_owned(),
        "rup wu0 wu0 fu4cp3cl3<home><down><down><down><enter><right><down><sdown><down><enter><enter>".to_owned(),
        "ji3ap7g4<enter><bs>g4<enter><bs><bs><bs>ji3ap7<enter>".to_owned(),
        "jia3p7g4<enter>".to_owned(),
        "suc3<dt:6>l3<enter>".to_owned(),
        "1q8iujm<bs><bs><obs>18 <down>`<enter>".to_owned(),
        "18 q8<home><end><del>a8<sspace><O-.><C-?><C-[>".to_owned(),
        "<S-a><S-1>18 <S-->q8 <down><down><enter><enter>".to_owned(),
        "vu,4vu,7<down><sup><sup><sdown><sdown><enter><enter>".to_owned(),
        "rupwu0 <left><down><left><left><right><enter><enter>".to_owned(),
        "18 <tab><senter>".to_owned(),
        "<C-,>18 <C-[>".to_owned(),
        "ek4wk3<down><sup><down><down><enter><enter>".to_owned(),
        "hl4su3a8 <down><down><enter><home><down><down><enter><enter>".to_owned(),
        "wu0 ".repeat(45) + "<enter>",
        "vu,4".repeat(30) + "<down><enter>",
    ];
    scripts.extend(targeted.into_iter().map(|k| (String::new(), k)));
    scripts
}

fn composer_configurations() -> Vec<(&'static str, ComposerOptions)> {
    let standard = ComposerOptions { character_forms: CharacterForms::Standard, ..ComposerOptions::default() };
    let legacy = ComposerOptions {
        typo_tolerance: TypoTolerance::Off,
        punctuation_keys: PunctuationKeys::Mac,
        ..ComposerOptions::default()
    };
    let other = ComposerOptions {
        candidate_anchor: CandidateAnchor::BeforeCursor,
        vertical_list: false,
        selection_keys: "asdfghjkl".chars().collect(),
        space_action: SpaceAction::CommitAndSpace,
        escape_clears_composition: false,
        key_decoder: KeyDecoderMethod::Reorder,
        typo_tolerance: TypoTolerance::Aggressive,
        ..ComposerOptions::default()
    };
    let commit = ComposerOptions {
        space_action: SpaceAction::Commit,
        key_decoder: KeyDecoderMethod::Slots,
        max_syllables: 12,
        ..ComposerOptions::default()
    };
    vec![
        ("app", ComposerOptions::default()),
        ("standard-forms", standard),
        ("legacy", legacy),
        ("other", other),
        ("commit", commit),
    ]
}

#[test]
fn composer_matches_swift() {
    with_inputs(|golden, inputs| {
        let Some(files) = dev_files(golden) else { return };
        let scripts = composer_scripts(inputs, &files);
        for (name, options) in composer_configurations() {
            let engine = Arc::new(app_engine(&inputs.data));
            let step = Arc::new(Mutex::new(0.0f64));
            let clock = step.clone();
            engine.set_clock(move || 1_800_000_000.0 + *clock.lock().unwrap());
            let mut composer = Composer::new(engine.clone(), options);
            let mut lines = vec![];
            let mut time = 1000.0;
            for (context, keys) in &scripts {
                composer.reset();
                composer.break_context();
                composer.set_preceding_text(context);
                lines.push(format!("== {context}\t{keys}"));
                let mut gap = 0.12;
                for (label, key, delay) in parse_keys(keys) {
                    if let Some(delay) = delay {
                        gap = delay;
                        continue;
                    }
                    time += gap;
                    gap = 0.12;
                    *step.lock().unwrap() += 1.0;
                    let result = composer.handle_at(key.unwrap(), time);
                    lines.push(format!(
                        "{label}\t{}\t{}\t{}",
                        u8::from(result.handled),
                        result.commit.as_deref().unwrap_or("-"),
                        presentation_line(composer.presentation())
                    ));
                }
                let nbest: Vec<String> =
                    composer.n_best(4).iter().map(|s| format!("{},{}", s.text(), hex(s.score))).collect();
                lines.push(format!("nbest\t{}", nbest.join(" ")));
            }
            let learned: Vec<String> =
                engine.learned_entries().iter().map(|(k, c)| format!("{}/{}/{}", k.reading, k.word, hex(*c))).collect();
            lines.push(format!("learned\t{}", learned.join(" ")));
            golden.check(&format!("composer.{name}"), &lines);
        }
    });
}

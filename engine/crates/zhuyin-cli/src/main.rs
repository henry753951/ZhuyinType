//! `zhuyin-cli`: types keys into the composer from a terminal; every evaluation runs through it.  A port of
//! `engine/swift/Sources/zhuyin-cli/main.swift` with the same flags, messages, key syntax, output and `--batch`
//! protocol.
//!
//! The neural models (`--reranker`, `--v3`, `--model-server`, `--check-v3`, and `--models` when its directory has
//! them) come in phase 4: those flags are parsed and validated as in Swift, then refused with a message.

mod usage;

use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use zhuyin_core::api::{
    CandidateAnchor, CharacterForms, ComposerOptions, FuzzySounds, KeyDecoderMethod, KeyInput, KeyResult,
    PunctuationKeys, TypoTolerance,
};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::key_decoder::Costs;
use zhuyin_core::swift_format::{format_double, graphemes};
use zhuyin_core::tsv::{parse_double, parse_int};

const USAGE: &str = "Usage: zhuyin-cli [--models DIR] [--model-server PATH] [--lexicon PATH] [--typo off|standard|aggressive] [--punctuation windows|mac] [--anchor after|before] [--nbest K] [--sentence-boundaries on|off] [--fuzzy zh,ch,sh,en,an] [--reranker DIR [--rerank-weight W] [--exact-margin M] [--context TEXT] [--timing]] [--batch [--keep-memory] [--report-nbest K] [--decoder slots|boundaries|reorder|timed] [--hotspot on|off] | \"<keys>\"]";

fn fail(message: &str) -> ! {
    let mut error = std::io::stderr();
    let _ = writeln!(error, "{message}");
    std::process::exit(1);
}

/// Everything the flags set.
// Each flag is an independent command-line switch.
#[allow(clippy::struct_excessive_bools)]
struct Settings {
    lexicon: String,
    batch: bool,
    keep_memory: bool,
    reranker: Option<String>,
    model_server: Option<String>,
    rerank_count: usize,
    report_nbest: usize,
    v3: Option<String>,
    report_list: bool,
    freeze_on_pick: bool,
    nbest: usize,
    context: String,
    timing: bool,
    fuzzy: FuzzySounds,
    options: ComposerOptions,
    config: EngineConfig,
    costs: Costs,
    models: Option<String>,
    given: HashSet<String>,
}

/// A positive integer with Swift `Int` syntax.
fn positive(text: &str) -> Option<usize> {
    parse_int(text).filter(|&c| c > 0).and_then(|c| usize::try_from(c).ok())
}

fn next_value(arguments: &[String]) -> &str {
    if arguments.len() < 2 {
        fail(USAGE);
    }
    &arguments[1]
}

fn on_off(value: &str, message: &str) -> bool {
    match value {
        "on" => true,
        "off" => false,
        _ => fail(message),
    }
}

#[allow(clippy::too_many_lines)]
fn parse_arguments(mut arguments: Vec<String>) -> (Settings, Vec<String>) {
    // The engine's own forms by default, so results compare with the evaluation text; the app writes 台.
    let options = ComposerOptions { character_forms: CharacterForms::Standard, ..ComposerOptions::default() };
    let mut s = Settings {
        lexicon: "data/build/lexicon.tsv".to_owned(),
        batch: false,
        keep_memory: false,
        reranker: None,
        model_server: None,
        rerank_count: 8,
        report_nbest: 0,
        v3: None,
        report_list: false,
        freeze_on_pick: true,
        nbest: 0,
        context: String::new(),
        timing: false,
        fuzzy: FuzzySounds::NONE,
        options,
        config: EngineConfig::default(),
        costs: Costs::default(),
        models: None,
        given: HashSet::new(),
    };
    while let Some(option) = arguments.first().cloned().filter(|a| a.starts_with("--")) {
        s.given.insert(option.clone());
        let mut take = 2;
        match option.as_str() {
            "--models" => s.models = Some(next_value(&arguments).to_owned()),
            "--lexicon" => next_value(&arguments).clone_into(&mut s.lexicon),
            "--typo" => {
                s.options.typo_tolerance =
                    TypoTolerance::from_raw(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
            }
            "--punctuation" => {
                s.options.punctuation_keys =
                    PunctuationKeys::from_raw(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
            }
            "--batch" => {
                s.batch = true;
                take = 1;
            }
            "--keep-memory" => {
                s.keep_memory = true;
                take = 1;
            }
            "--model-server" => s.model_server = Some(next_value(&arguments).to_owned()),
            "--reranker" => s.reranker = Some(next_value(&arguments).to_owned()),
            "--rerank-count" => {
                let count = positive(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
                s.rerank_count = count;
            }
            "--forms" => {
                let value = arguments.get(1).map(String::as_str).unwrap_or_default();
                s.options.character_forms =
                    CharacterForms::from_raw(value).filter(|_| arguments.len() >= 2).unwrap_or_else(|| {
                        fail(&format!(
                            "--forms needs one of: {}",
                            CharacterForms::ALL.map(CharacterForms::raw_value).join(", ")
                        ))
                    });
            }
            "--decoder" => {
                let value = arguments.get(1).map(String::as_str).unwrap_or_default();
                s.options.key_decoder =
                    KeyDecoderMethod::from_raw(value).filter(|_| arguments.len() >= 2).unwrap_or_else(|| {
                        fail(&format!(
                            "--decoder needs one of: {}",
                            KeyDecoderMethod::ALL.map(KeyDecoderMethod::raw_value).join(", ")
                        ))
                    });
            }
            "--decoder-costs" => {
                // implicit,overwrite,swap,skip,swapTogether,skipTogether,swapApart,skipApart
                let values: Vec<f64> = if arguments.len() >= 2 {
                    arguments[1].split(',').filter(|p| !p.is_empty()).filter_map(parse_double).collect()
                } else {
                    vec![]
                };
                if values.len() != 8 {
                    fail("--decoder-costs needs 8 comma-separated numbers");
                }
                s.costs.implicit_boundary = values[0];
                s.costs.overwrite = values[1];
                s.costs.swap = values[2];
                s.costs.skip = values[3];
                s.costs.swap_together = values[4];
                s.costs.skip_together = values[5];
                s.costs.swap_apart = values[6];
                s.costs.skip_apart = values[7];
            }
            "--check-v3" => {
                next_value(&arguments);
                fail("zhuyin-cli (Rust): --check-v3 is not supported yet; the neural models come in phase 4");
            }
            "--v3" => s.v3 = Some(next_value(&arguments).to_owned()),
            "--rerank-gate" | "--rerank-switch" | "--freeze-on-pick" => {
                // τ and δ of the reranker; freezing the other words after a pick (on/off).
                if arguments.len() < 2 {
                    fail(&format!("{option} needs a value"));
                }
                if option == "--freeze-on-pick" {
                    s.freeze_on_pick = arguments[1] != "off";
                } else if parse_double(&arguments[1]).is_none() {
                    fail(&format!("{option} needs a number"));
                }
            }
            "--report-list" => {
                s.report_list = true;
                take = 1;
            }
            "--v3-list" => {
                let value = arguments.get(1).map(String::as_str).unwrap_or_default();
                if arguments.len() < 2 || !matches!(value, "on" | "off") {
                    fail("--v3-list needs on or off");
                }
            }
            "--v3-boundaries" => {
                // Whether the character model is told which syllables ended without a tone key or Space.
                let value = arguments.get(1).map(String::as_str).unwrap_or_default();
                if arguments.len() < 2 || !matches!(value, "on" | "off") {
                    fail("--v3-boundaries needs on or off");
                }
            }
            "--v3-weight" | "--domain-scale" | "--domain-centre" | "--exact-margin" | "--rerank-weight" => {
                let value = parse_double(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
                match option.as_str() {
                    "--domain-scale" => s.config.domain_evidence_scale = value,
                    "--domain-centre" => s.config.domain_evidence_centre = value,
                    _ => {}
                }
            }
            "--chat-lm" => s.config.chat_model_directory = Some(PathBuf::from(next_value(&arguments))),
            "--end-share" => {
                let value = parse_double(next_value(&arguments)).filter(|&v| v > 0.0 && v <= 1.0);
                s.config.sentence_end_share = value.unwrap_or_else(|| fail(USAGE));
            }
            "--chat-prior" => {
                let value = parse_double(next_value(&arguments)).filter(|&v| v > 0.0 && v < 1.0);
                s.config.chat_prior = value.unwrap_or_else(|| fail(USAGE));
            }
            "--hotspot" => s.config.hotspot = on_off(next_value(&arguments), USAGE),
            "--report-nbest" => {
                let count = positive(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
                s.report_nbest = count;
            }
            "--sentence-boundaries" => s.config.sentence_boundaries = on_off(next_value(&arguments), USAGE),
            "--anchor" => {
                s.options.candidate_anchor = match next_value(&arguments) {
                    "after" => CandidateAnchor::AfterCursor,
                    "before" => CandidateAnchor::BeforeCursor,
                    _ => fail(USAGE),
                };
            }
            "--context" => next_value(&arguments).clone_into(&mut s.context),
            "--timing" => {
                s.timing = true;
                take = 1;
            }
            "--fuzzy" => {
                for name in next_value(&arguments).split(',').filter(|p| !p.is_empty()) {
                    s.fuzzy.insert(match name {
                        "zh" => FuzzySounds::Z_ZH,
                        "ch" => FuzzySounds::C_CH,
                        "sh" => FuzzySounds::S_SH,
                        "en" => FuzzySounds::EN_ENG,
                        "an" => FuzzySounds::AN_ANG,
                        _ => fail(USAGE),
                    });
                }
            }
            "--nbest" => {
                let count = positive(next_value(&arguments)).unwrap_or_else(|| fail(USAGE));
                s.nbest = count;
            }
            _ => fail(USAGE),
        }
        arguments.drain(..take);
    }
    if arguments.len() != usize::from(!s.batch) {
        fail(USAGE);
    }
    (s, arguments)
}

/// `--models DIR`: everything the app loads from a data directory, set up the way the app sets it up; explicit
/// options still win.
fn apply_models(s: &mut Settings) {
    let Some(models) = s.models.clone() else { return };
    let root = Path::new(&models);
    let has = |directory: &str, file: &str| root.join(directory).join(file).exists();
    if !s.given.contains("--lexicon") {
        s.lexicon = root.join("lexicon.tsv").to_string_lossy().into_owned();
    }
    if !s.given.contains("--chat-lm") && (has("lm_chat", "bigram.tsv") || has("lm_chat", "lm.bin")) {
        s.config.chat_model_directory = Some(root.join("lm_chat"));
    }
    if !s.given.contains("--reranker") && has("reranker", "vocab.json") {
        s.reranker = Some(root.join("reranker").to_string_lossy().into_owned());
    }
    if let Some(reranker) = &s.reranker
        && let Ok(data) = std::fs::read(Path::new(reranker).join("config.json"))
        && let Ok(serde_json::Value::Object(config)) = serde_json::from_slice::<serde_json::Value>(&data)
        && !s.given.contains("--rerank-count")
        && let Some(count) = config.get("count").and_then(serde_json::Value::as_u64).filter(|&c| c > 0)
    {
        s.rerank_count = usize::try_from(count).unwrap_or(usize::MAX);
    }
    if !s.given.contains("--v3") && has("v3", "chars.json") {
        s.v3 = Some(root.join("v3").to_string_lossy().into_owned());
    }
}

/// One parsed key: its label for the single-run printout, and the key or a delay before the next key.
enum Typed {
    Key(String, KeyInput),
    /// `<dt:MS>`: the gap before the next key, in seconds.
    Delay(f64),
}

fn special(name: &str) -> Option<KeyInput> {
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
}

/// Keys and `<name>` escapes; an unknown escape is an error.
fn parse_keys(input: &str) -> Result<Vec<Typed>, String> {
    let characters = graphemes(input);
    let mut result = vec![];
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == "<"
            && let Some(end) = characters[index..].iter().position(|&c| c == ">").map(|p| p + index)
        {
            let name: String = characters[index + 1..end].concat();
            let rest: Vec<char> = name.chars().skip(2).collect();
            let single = graphemes(&name).len() == 3;
            if let Some(key) = special(&name) {
                result.push(Typed::Key(format!("<{name}>"), key));
            } else if name.starts_with("S-") && single {
                result.push(Typed::Key(format!("<{name}>"), KeyInput::ShiftedChar(*rest.last().unwrap())));
            } else if name.starts_with("O-") && single {
                result.push(Typed::Key(format!("<{name}>"), KeyInput::Printed(*rest.last().unwrap())));
            } else if name.starts_with("C-") && single {
                result.push(Typed::Key(format!("<{name}>"), KeyInput::ControlChar(*rest.last().unwrap())));
            } else if let Some(ms) = name.strip_prefix("dt:").and_then(parse_double) {
                // A delay, not a key; Swift labels it with the number's description and reads it back.
                result.push(Typed::Delay(parse_double(&format_double(ms)).unwrap_or(120.0) / 1000.0));
            } else {
                return Err(format!("Unknown key escape <{name}>"));
            }
            index = end + 1;
        } else {
            let c = character.chars().next().unwrap_or(' ');
            if character == " " {
                result.push(Typed::Key("<space>".to_owned(), KeyInput::Space));
            } else {
                result.push(Typed::Key(character.to_owned(), KeyInput::Char(c)));
            }
            index += 1;
        }
    }
    Ok(result)
}

/// The run's key-down clock: 0.12 s between keys unless a `<dt:MS>` sets the gap; it continues across batch lines.
struct Typist {
    clock: f64,
    timing: bool,
    key_times: Vec<f64>,
}

impl Typist {
    /// Types the keys; returns what was committed.
    fn typed(
        &mut self,
        keys: &[Typed],
        composer: &mut Composer,
        mut each: impl FnMut(&str, &KeyResult, &Composer),
    ) -> String {
        let mut committed = String::new();
        let mut gap = 0.12;
        for typed in keys {
            let (label, key) = match typed {
                Typed::Delay(seconds) => {
                    gap = *seconds;
                    continue;
                }
                Typed::Key(label, key) => (label, *key),
            };
            self.clock += gap;
            gap = 0.12;
            let start = Instant::now();
            let result = composer.handle_at(key, self.clock);
            if self.timing {
                self.key_times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            committed.push_str(result.commit.as_deref().unwrap_or(""));
            each(label, &result, composer);
        }
        committed
    }
}

/// The presentation with `[` `]` around the focused clause, `⟨` `⟩` around the target and `|` at the caret.
fn display(composer: &Composer) -> String {
    let p = composer.presentation();
    let units: Vec<u16> = p.text.encode_utf16().collect();
    let focus = p.focused_clause.map(|i| p.clauses[i].clone());
    let target = p.target_clause.map(|i| p.clauses[i].clone());
    let mut cuts: Vec<usize> = vec![0, p.cursor, units.len()];
    for range in focus.iter().chain(target.iter()) {
        cuts.push(range.start);
        cuts.push(range.end);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut text = String::new();
    for (index, &offset) in cuts.iter().enumerate() {
        if focus.as_ref().is_some_and(|f| f.end == offset) {
            text.push(']');
        }
        if target.as_ref().is_some_and(|t| t.end == offset) {
            text.push('⟩');
        }
        if focus.as_ref().is_some_and(|f| f.start == offset) {
            text.push('[');
        }
        if offset == p.cursor {
            text.push('|');
        }
        if target.as_ref().is_some_and(|t| t.start == offset) {
            text.push('⟨');
        }
        if let Some(&next) = cuts.get(index + 1) {
            text.push_str(&String::from_utf16_lossy(&units[offset.min(units.len())..next.min(units.len())]));
        }
    }
    text
}

fn print_step(label: &str, result: &KeyResult, composer: &Composer, committed: &mut String) {
    let p = composer.presentation();
    println!("{label} → {}{}", display(composer), if result.handled { "" } else { " [pass]" });
    if let Some(list) = &p.candidates {
        let items: Vec<String> = list
            .page_range()
            .map(|index| {
                format!(
                    "{}{}:{}{}",
                    if index == list.selection { ">" } else { "" },
                    index - list.page() * list.page_size + 1,
                    list.items[index].text,
                    if list.items[index].annotation.is_none() { "" } else { "*" }
                )
            })
            .collect();
        println!("  candidates {}/{}: {}", list.page() + 1, list.page_count(), items.join("  "));
    }
    if let Some(hint) = &p.zhuyin_hint {
        println!("  ` {hint}");
    }
    if let Some(notice) = &p.notice {
        println!("  {notice}");
    }
    if let Some(text) = &result.commit {
        committed.push_str(text);
        println!("  committed: {text}");
    }
}

fn refuse_models(s: &Settings) {
    let unsupported =
        [("--model-server", s.model_server.is_some()), ("--reranker", s.reranker.is_some()), ("--v3", s.v3.is_some())];
    for (flag, used) in unsupported {
        if used {
            let source = if s.given.contains(flag) { flag.to_owned() } else { format!("--models (which sets {flag})") };
            fail(&format!("zhuyin-cli (Rust): {source} is not supported yet; the neural models come in phase 4"));
        }
    }
}

fn main() {
    let (mut s, rest) = parse_arguments(std::env::args().skip(1).collect());
    apply_models(&mut s);
    refuse_models(&s);
    let engine_start = Instant::now();
    let engine = Engine::open(Path::new(&s.lexicon), None, s.config.clone()).unwrap_or_else(|e| fail(&e.to_string()));
    if s.timing {
        let _ = writeln!(std::io::stderr(), "engine load: {:.3} ms", engine_start.elapsed().as_secs_f64() * 1000.0);
    }
    engine.set_fuzzy_sounds(s.fuzzy);
    let engine = Arc::new(engine);
    let mut composer = Composer::new(engine.clone(), s.options.clone());
    composer.freezes_on_pick = s.freeze_on_pick;
    composer.decoder_costs = s.costs;
    let mut typist = Typist { clock: 1000.0, timing: s.timing, key_times: vec![] };
    let stdout = std::io::stdout();
    if s.batch {
        // One line is one stretch of typing: what it commits is the context of the rest of the line.  A line
        // "TEXT<tab>KEYS" types KEYS after TEXT, as if TEXT were in the document before the insertion point.
        let base_context = s.context.clone();
        let usage_start = usage::Usage::now();
        let wall_start = Instant::now();
        let mut out = std::io::BufWriter::new(stdout.lock());
        for line in std::io::stdin().lock().split(b'\n') {
            let Ok(line) = line else { break };
            let mut line = String::from_utf8_lossy(&line).into_owned();
            if line.ends_with('\r') {
                line.pop();
            }
            composer.reset();
            composer.break_context();
            if !s.keep_memory {
                engine.forget_all_learned();
            }
            let (context, keys) = match line.split_once('\t') {
                Some((context, keys)) => (context.to_owned(), keys.to_owned()),
                None => (base_context.clone(), line.clone()),
            };
            composer.set_preceding_text(&context);
            let parsed = parse_keys(&keys).unwrap_or_else(|e| {
                let _ = out.flush();
                fail(&e)
            });
            let committed = typist.typed(&parsed, &mut composer, |_, _, _| {});
            let printed = if s.report_list {
                let text = composer
                    .presentation()
                    .candidates
                    .as_ref()
                    .map(|l| l.items.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\t"))
                    .unwrap_or_default();
                composer.reset();
                text
            } else if s.report_nbest > 0 {
                let text = composer
                    .n_best(s.report_nbest)
                    .iter()
                    .map(zhuyin_core::api::ScoredSentence::text)
                    .collect::<Vec<_>>()
                    .join("\t");
                composer.reset();
                text
            } else {
                committed
            };
            let _ = writeln!(out, "{printed}");
        }
        let _ = out.flush();
        // On standard error, so batch output stays one line per input.
        if s.timing {
            let summary = usage::summary(
                &typist.key_times,
                &usage_start,
                &usage::Usage::now(),
                wall_start.elapsed().as_secs_f64(),
            );
            let _ = write!(std::io::stderr(), "{summary}");
        }
        std::process::exit(0);
    }
    composer.set_preceding_text(&s.context);
    let usage_start = usage::Usage::now();
    let wall_start = Instant::now();
    let keys = parse_keys(&rest[0]).unwrap_or_else(|e| fail(&e));
    let mut committed = String::new();
    typist.typed(&keys, &mut composer, |label, result, composer| print_step(label, result, composer, &mut committed));
    for (index, sentence) in composer.n_best(s.nbest).iter().enumerate() {
        let words: Vec<String> =
            sentence.words.iter().map(|w| format!("{}:{}..<{}", w.text, w.range.start, w.range.end)).collect();
        println!(
            "  nbest {}: {:.6} {}{} [{}]",
            index + 1,
            sentence.score,
            sentence.text(),
            if sentence.corrected() { "*" } else { "" },
            words.join(", ")
        );
    }
    if s.timing {
        print!(
            "{}",
            usage::summary(&typist.key_times, &usage_start, &usage::Usage::now(), wall_start.elapsed().as_secs_f64())
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_syntax() {
        let keys = parse_keys(
            "a<space><bs><obs><del><left><right><home><end><up><down><enter><senter><esc><tab><S-x><sleft><sright>",
        )
        .unwrap();
        let inputs: Vec<KeyInput> = keys
            .iter()
            .filter_map(|t| match t {
                Typed::Key(_, k) => Some(*k),
                Typed::Delay(_) => None,
            })
            .collect();
        assert_eq!(
            inputs,
            [
                KeyInput::Char('a'),
                KeyInput::Space,
                KeyInput::Backspace,
                KeyInput::OptionBackspace,
                KeyInput::ForwardDelete,
                KeyInput::Left,
                KeyInput::Right,
                KeyInput::Home,
                KeyInput::End,
                KeyInput::Up,
                KeyInput::Down,
                KeyInput::Enter,
                KeyInput::ShiftEnter,
                KeyInput::Escape,
                KeyInput::Tab,
                KeyInput::ShiftedChar('x'),
                KeyInput::ShiftLeft,
                KeyInput::ShiftRight
            ]
        );
        assert_eq!(parse_keys("<unknown>").err().as_deref(), Some("Unknown key escape <unknown>"));
        assert!(matches!(parse_keys("<dt:6>").unwrap()[0], Typed::Delay(d) if d == 0.006));
        assert!(matches!(parse_keys("<C-,>").unwrap()[0], Typed::Key(_, KeyInput::ControlChar(','))));
        assert!(matches!(parse_keys("a<b").unwrap()[1], Typed::Key(_, KeyInput::Char('<'))));
    }
}

//! The golden-test harness: Swift digests (`parity/golden/digests.tsv`, or the file `ZHUYIN_GOLDEN` names), the shared
//! inputs, and line hashing.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use zhuyin_core::tsv::{parse_double, scan_tsv};

pub(crate) const EXTRAS: [&str; 5] = ["", "ㄅ", "不存在的讀音", "x", "ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ"];

pub(crate) fn hex(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

pub(crate) fn fnv(lines: &[String]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for line in lines {
        for &byte in line.as_bytes().iter().chain(b"\n") {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

pub(crate) fn byte_sorted(strings: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = strings.into_iter().collect();
    set.into_iter().collect()
}

pub(crate) fn parity_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../parity")
}

/// The digests and input sizes the Swift engine's tool recorded.
pub(crate) struct Golden {
    pub(crate) sizes: HashMap<String, u64>,
    pub(crate) digests: HashMap<String, (usize, String)>,
}

impl Golden {
    pub(crate) fn load() -> Option<Golden> {
        // ZHUYIN_GOLDEN: digests regenerated elsewhere (parity/gate.sh on another machine's data).
        let path = std::env::var_os("ZHUYIN_GOLDEN")
            .map_or_else(|| parity_directory().join("golden/digests.tsv"), PathBuf::from);
        let text = std::fs::read_to_string(path).ok()?;
        let mut golden = Golden { sizes: HashMap::new(), digests: HashMap::new() };
        for line in text.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["#size", name, size] => {
                    golden.sizes.insert((*name).to_owned(), size.parse().unwrap());
                }
                // The C libraries' math functions differ in the last bit between systems (glibc and Apple's libm
                // round a few results of `log` and `exp` differently), so digests hold for the system they were made
                // on; parity/gate.sh regenerates them on the Linux host.
                ["#system", system] if *system != std::env::consts::OS => {
                    eprintln!("skipped: the golden digests were made on {system}, this is {}", std::env::consts::OS);
                    return None;
                }
                [name, count, digest] => {
                    golden.digests.insert((*name).to_owned(), (count.parse().unwrap(), (*digest).to_owned()));
                }
                _ => {}
            }
        }
        Some(golden)
    }

    /// Whether `path` is the input the digests were made with (by size).
    pub(crate) fn matches(&self, name: &str, path: &Path) -> bool {
        let Some(&expected) = self.sizes.get(name) else { return false };
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.len() == expected => true,
            Ok(metadata) => {
                eprintln!(
                    "skipped {name}: {} is {} bytes, the digests were made with {expected}",
                    path.display(),
                    metadata.len()
                );
                false
            }
            Err(_) => {
                eprintln!("skipped {name}: {} is missing", path.display());
                false
            }
        }
    }

    pub(crate) fn check(&self, name: &str, lines: &[String]) {
        let (count, digest) = self.digests.get(name).unwrap_or_else(|| panic!("no Swift digest for {name}"));
        let actual = format!("{:016x}", fnv(lines));
        if lines.len() == *count && actual == *digest {
            eprintln!("{name}: {count} lines identical to Swift");
            return;
        }
        let dump = parity_directory().join("out").join(format!("{name}.txt"));
        let detail = match std::fs::read_to_string(&dump) {
            Ok(text) => {
                let expected: Vec<&str> = text.lines().collect();
                let differing = lines.iter().zip(&expected).filter(|(a, b)| a != b).count();
                match lines.iter().zip(&expected).position(|(a, b)| a != b) {
                    Some(i) => format!(
                        "{differing} lines differ; first at {i}:\n  rust:  {}\n  swift: {}",
                        lines[i], expected[i]
                    ),
                    None => format!("lengths differ: rust {}, swift {}", lines.len(), expected.len()),
                }
            }
            Err(_) => format!("(no Swift dump at {})", dump.display()),
        };
        panic!("{name}: {} lines, digest {actual}; Swift: {count} lines, {digest}. {detail}", lines.len());
    }
}

pub(crate) struct Inputs {
    pub(crate) data: PathBuf,
    pub(crate) tsv: Vec<u8>,
    pub(crate) readings: Vec<String>,
    pub(crate) words: Vec<String>,
    pub(crate) sequences: Vec<Vec<String>>,
}

impl Inputs {
    pub(crate) fn load(golden: &Golden) -> Option<Inputs> {
        let data = super::data_directory()?;
        if !golden.matches("lexicon.bin", &data.join("lexicon.bin"))
            || !golden.matches("lexicon.tsv", &data.join("lexicon.tsv"))
        {
            return None;
        }
        let tsv = std::fs::read(data.join("lexicon.tsv")).ok()?;
        let (mut readings, mut words) = (vec![], vec![]);
        scan_tsv(&tsv, false, |fields| {
            if fields.len() == 3
                && parse_double(&fields[2]).is_some_and(f64::is_finite)
                && !fields[0].is_empty()
                && !fields[1].is_empty()
            {
                readings.push(fields[0].to_string());
                words.push(fields[1].to_string());
            }
        });
        let (readings, words) = (byte_sorted(readings), byte_sorted(words));
        let mut sequences = vec![];
        let text = std::env::var_os("ZHUYIN_PARITY_TEXT").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/build/lm/test_sentences.txt"),
            PathBuf::from,
        );
        if golden.matches("text", &text) {
            let known: HashSet<&str> = words.iter().map(String::as_str).collect();
            let content = std::fs::read_to_string(&text).unwrap();
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
        }
        Some(Inputs { data, tsv, readings, words, sequences })
    }
}

pub(crate) fn with_inputs(body: impl FnOnce(&Golden, &Inputs)) {
    let Some(golden) = Golden::load() else {
        eprintln!("skipped: no parity/golden/digests.tsv");
        return;
    };
    if let Some(inputs) = Inputs::load(&golden) {
        body(&golden, &inputs);
    }
}

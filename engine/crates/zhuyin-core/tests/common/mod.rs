//! Fixture writers shared by the integration tests: independent writers for the ZTLM/1 and ZTLX/1 formats (ports of
//! `ZhuyinCoreChecks/LMFixture.swift` and `LexiconFixture.swift`), temporary directories and the real-data lookup.

#![allow(dead_code)]

pub(crate) mod parity;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// One bigram of a fixture model.
pub(crate) struct LmBigram {
    pub(crate) first: &'static str,
    pub(crate) second: &'static str,
    pub(crate) logp: f32,
    pub(crate) backoff: f32,
}

pub(crate) fn bigram(first: &'static str, second: &'static str, logp: f32) -> LmBigram {
    LmBigram { first, second, logp, backoff: 0.0 }
}

pub(crate) fn bigram_backoff(first: &'static str, second: &'static str, logp: f32, backoff: f32) -> LmBigram {
    LmBigram { first, second, logp, backoff }
}

/// One trigram of a fixture model.
pub(crate) struct LmTrigram {
    pub(crate) first: &'static str,
    pub(crate) second: &'static str,
    pub(crate) third: &'static str,
    pub(crate) logp: f32,
}

/// Writes the resources/formats/lm.md arrays, including alignment and little-endian fields.
pub(crate) fn lm_fixture(
    order: u32,
    unigrams: &[(&str, f32)],
    backoffs: &[(&str, f32)],
    bigrams: &[LmBigram],
    trigrams: &[LmTrigram],
) -> Vec<u8> {
    let reserved = ["</s>", "<s>", "<unk>"];
    let mut rest: Vec<&str> =
        unigrams.iter().map(|u| u.0).filter(|w| !reserved.contains(w)).collect::<BTreeSet<_>>().into_iter().collect();
    rest.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let words: Vec<&str> = reserved.iter().copied().chain(rest).collect();
    let ids: HashMap<&str, u32> = words.iter().enumerate().map(|(i, w)| (*w, i as u32)).collect();
    let unigram: HashMap<&str, f32> = unigrams.iter().copied().collect();
    let backoff: HashMap<&str, f32> = backoffs.iter().copied().collect();
    let mut pairs: Vec<&LmBigram> = bigrams.iter().collect();
    pairs.sort_by_key(|p| (ids[p.first], ids[p.second]));
    let mut data = vec![0u8; 64];
    let uint = |data: &mut Vec<u8>, v: u32| data.extend_from_slice(&v.to_le_bytes());
    let float = |data: &mut Vec<u8>, v: f32| data.extend_from_slice(&v.to_bits().to_le_bytes());
    let align = |data: &mut Vec<u8>| {
        while !data.len().is_multiple_of(8) {
            data.push(0);
        }
    };
    let vocabulary = data.len();
    let mut size = 0u32;
    uint(&mut data, 0);
    for word in &words {
        size += word.len() as u32;
        uint(&mut data, size);
    }
    for word in &words {
        data.extend_from_slice(word.as_bytes());
    }
    align(&mut data);
    let unigram_offset = data.len();
    for word in &words {
        float(&mut data, unigram.get(word).copied().unwrap_or(-5.0));
    }
    for word in &words {
        float(&mut data, backoff.get(word).copied().unwrap_or(0.0));
    }
    align(&mut data);
    let bigram_offset = data.len();
    let mut position = 0u32;
    uint(&mut data, 0);
    for word in &words {
        position += pairs.iter().filter(|p| p.first == *word).count() as u32;
        uint(&mut data, position);
    }
    for pair in &pairs {
        uint(&mut data, ids[pair.second]);
    }
    for pair in &pairs {
        float(&mut data, pair.logp);
    }
    for pair in &pairs {
        float(&mut data, pair.backoff);
    }
    align(&mut data);
    let trigram_offset = if order == 3 { data.len() } else { 0 };
    if order == 3 {
        let groups: Vec<Vec<&LmTrigram>> = pairs
            .iter()
            .map(|pair| {
                let mut group: Vec<&LmTrigram> =
                    trigrams.iter().filter(|t| t.first == pair.first && t.second == pair.second).collect();
                group.sort_by_key(|t| ids[t.third]);
                group
            })
            .collect();
        assert_eq!(groups.iter().map(Vec::len).sum::<usize>(), trigrams.len());
        position = 0;
        uint(&mut data, 0);
        for group in &groups {
            position += group.len() as u32;
            uint(&mut data, position);
        }
        for group in &groups {
            for triple in group {
                uint(&mut data, ids[triple.third]);
            }
        }
        for group in &groups {
            for triple in group {
                float(&mut data, triple.logp);
            }
        }
        align(&mut data);
    }
    let mut header = b"ZTLM".to_vec();
    for value in [1, order, words.len() as u32, pairs.len() as u32, trigrams.len() as u32] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    for value in [vocabulary, unigram_offset, bigram_offset, trigram_offset, data.len()] {
        header.extend_from_slice(&(value as u64).to_le_bytes());
    }
    data[..64].copy_from_slice(&header);
    data
}

/// A three-entry lexicon: neutral tone, two readings, and a shared word.
pub(crate) fn lexicon_fixture() -> Vec<u8> {
    let mut data = vec![0u8; 128];
    let mut blob = Vec::new();
    let mut string = |text: &str| -> [u32; 2] {
        let result = [blob.len() as u32, text.len() as u32];
        blob.extend_from_slice(text.as_bytes());
        result
    };
    let words = [string("八"), string("吧")];
    let readings = [string("ㄅㄚ"), string("ㄅㄚ˙")];
    let base = string("ㄅㄚ");
    let u32s = |data: &mut Vec<u8>, values: &[u32]| {
        for v in values {
            data.extend_from_slice(&v.to_le_bytes());
        }
    };
    let mut offsets: Vec<u64> = vec![];
    let section = |data: &mut Vec<u8>, offsets: &mut Vec<u64>| {
        while !data.len().is_multiple_of(8) {
            data.push(0);
        }
        offsets.push(data.len() as u64);
    };
    section(&mut data, &mut offsets);
    u32s(&mut data, &[words[0][0], words[0][1], 0, 1, 0, 0, words[1][0], words[1][1], 1, 2, 2, 0]);
    section(&mut data, &mut offsets);
    u32s(&mut data, &[readings[0][0], readings[0][1], 0, 2, readings[1][0], readings[1][1], 2, 1]);
    section(&mut data, &mut offsets);
    for (word, reading, logp, prior) in [(0u32, 0u32, -3.0f64, 0.0f64), (1, 0, -2.0, -8.0), (1, 1, -2.0, 0.0)] {
        u32s(&mut data, &[word, reading]);
        data.extend_from_slice(&logp.to_bits().to_le_bytes());
        data.extend_from_slice(&prior.to_bits().to_le_bytes());
    }
    section(&mut data, &mut offsets);
    u32s(&mut data, &[0, 1, 2]);
    section(&mut data, &mut offsets);
    u32s(&mut data, &[base[0], base[1], 0, 2]);
    section(&mut data, &mut offsets);
    u32s(&mut data, &[0, 1]);
    section(&mut data, &mut offsets);
    u32s(&mut data, &base);
    section(&mut data, &mut offsets);
    u32s(&mut data, &[readings[0][0], readings[0][1], readings[1][0], readings[1][1]]);
    section(&mut data, &mut offsets);
    data.extend_from_slice(&blob);
    let size = data.len() as u64;
    let mut header = Vec::new();
    u32s(&mut header, &[0x584c_545a, 1, 2, 2, 3, 1, 1, 2]);
    for value in offsets.iter().copied().chain([size, 0, 0]) {
        header.extend_from_slice(&value.to_le_bytes());
    }
    data[..128].copy_from_slice(&header);
    data
}

/// A 150 MB structural benchmark with the given vocabulary and dense, sorted trigram groups.  Its probabilities are
/// synthetic; it measures loading and lookups, never conversion quality.
pub(crate) fn large_lm_fixture(words: &BTreeSet<String>) -> Vec<u8> {
    let mut all: Vec<&str> = words.iter().map(String::as_str).collect();
    all.extend(["</s>", "<s>", "<unk>"]);
    all.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    all.dedup();
    assert_eq!(&all[..3], ["</s>", "<s>", "<unk>"]);
    let v = all.len();
    let b = v * 2;
    let align = |n: usize| (n + 7) & !7;
    let vocabulary = 64;
    let blob = vocabulary + (v + 1) * 4;
    let unigram = align(blob + all.iter().map(|w| w.len()).sum::<usize>());
    let bigram = align(unigram + v * 8);
    let trigram = align(bigram + (v + 1) * 4 + b * 12);
    let t = (150_000_000usize.saturating_sub(trigram + (b + 1) * 4)) / 8;
    assert!(t / b + 1 < v);
    let size = align(trigram + (b + 1) * 4 + t * 8);
    let mut data = vec![0u8; size];
    let put = |data: &mut [u8], at: usize, value: u32| data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    let put64 = |data: &mut [u8], at: usize, value: u64| data[at..at + 8].copy_from_slice(&value.to_le_bytes());
    let minus5 = (-5f32).to_bits();
    put(&mut data, 0, 0x4d4c_545a);
    put(&mut data, 4, 1);
    put(&mut data, 8, 3);
    put(&mut data, 12, v as u32);
    put(&mut data, 16, b as u32);
    put(&mut data, 20, t as u32);
    for (i, value) in [vocabulary, unigram, bigram, trigram, size].into_iter().enumerate() {
        put64(&mut data, 24 + i * 8, value as u64);
    }
    let mut offset = 0;
    for (i, word) in all.iter().enumerate() {
        put(&mut data, vocabulary + i * 4, offset as u32);
        data[blob + offset..blob + offset + word.len()].copy_from_slice(word.as_bytes());
        offset += word.len();
        put(&mut data, unigram + i * 4, minus5);
        put(&mut data, bigram + i * 4, (i * 2) as u32);
    }
    put(&mut data, vocabulary + v * 4, offset as u32);
    put(&mut data, bigram + v * 4, b as u32);
    for i in 0..b {
        put(&mut data, bigram + (v + 1 + i) * 4, (i % 2) as u32);
        put(&mut data, bigram + (v + 1 + b + i) * 4, minus5);
        let start = i * t / b;
        let end = (i + 1) * t / b;
        put(&mut data, trigram + i * 4, start as u32);
        for j in start..end {
            put(&mut data, trigram + (b + 1 + j) * 4, (j - start) as u32);
            put(&mut data, trigram + (b + 1 + t + j) * 4, minus5);
        }
    }
    put(&mut data, trigram + b * 4, t as u32);
    data
}

/// A fresh directory under the system temporary directory, removed when dropped.
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    pub(crate) fn new(label: &str) -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "zhuyin-core-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// A subdirectory, created.
    pub(crate) fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The directory holding `lexicon.bin`, `lexicon.tsv`, `lm/` and `lm_chat/`: `$ZHUYIN_DATA`, else the repository's
/// `data/build`.  `None` (and the real-data tests skip) when it has no `lexicon.bin`.
pub(crate) fn data_directory() -> Option<PathBuf> {
    let path = std::env::var_os("ZHUYIN_DATA")
        .map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/build"), PathBuf::from);
    if path.join("lexicon.bin").exists() {
        Some(path)
    } else {
        eprintln!("skipped: no lexicon.bin in {} (set ZHUYIN_DATA)", path.display());
        None
    }
}

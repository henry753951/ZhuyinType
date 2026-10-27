//! The ONNX Runtime backend against the saved Core ML reference outputs (feature `onnx`, models from
//! `ZHUYIN_MODELS`, default `data/build`; skipped without them):
//! - v3: `v3/test_vectors.json` through `SyllableScorer` + `OnnxSyllableModel`, every value within 0.1 of Core ML
//!   (the bound `zt models parity` accepts for fp32 ONNX against Core ML; Core ML itself is checked at 0.002);
//! - the reranker: the cases of `remote_eval/reranker-coreml.json` (`zt models parity --coreml`) through
//!   `OnnxReranker`, every target within 0.1; and the same sentences through `NeuralReranker::scores`, whose sums
//!   must equal the per-target values the batched model gives for them.
#![cfg(feature = "onnx")]
#![allow(missing_docs)]

use std::path::PathBuf;

use zhuyin_models::onnx::{OnnxReranker, OnnxSyllableModel};
use zhuyin_models::{NeuralReranker, RerankerModel, SyllableScorer, check_vectors};

fn models() -> Option<PathBuf> {
    let path = std::env::var_os("ZHUYIN_MODELS")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/build"), PathBuf::from);
    path.join("v3/SyllableEncoder.onnx").exists().then_some(path)
}

#[test]
fn v3_vectors() {
    let Some(models) = models() else {
        eprintln!("skipped: no ONNX models");
        return;
    };
    let directory = models.join("v3");
    let scorer = SyllableScorer::new(&directory, Box::new(OnnxSyllableModel::new(&directory).unwrap())).unwrap();
    let check = check_vectors(&scorer, &directory).unwrap();
    eprintln!(
        "v3 test vectors: {}/{} within the Core ML tolerance; largest difference {}",
        check.passed, check.total, check.worst
    );
    assert!(check.total > 0 && check.worst < 0.1, "{check:?}");
}

#[test]
fn reranker_cases() {
    let Some(models) = models() else {
        eprintln!("skipped: no ONNX models");
        return;
    };
    let Ok(data) = std::fs::read(models.join("remote_eval/reranker-coreml.json")) else {
        eprintln!("skipped: no reranker-coreml.json");
        return;
    };
    let cases: serde_json::Value = serde_json::from_slice(&data).unwrap();
    let directory = models.join("reranker");
    let mut model = OnnxReranker::new(&directory).unwrap();
    let rows = |value: &serde_json::Value| -> Vec<Vec<i32>> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect())
            .collect()
    };
    let mut worst: f64 = 0.0;
    for case in cases.as_array().unwrap() {
        let ids = rows(&case["ids"]);
        let targets = rows(&case["targets"]);
        let (count, length) = (ids.len(), ids[0].len());
        let flat_ids: Vec<i32> = ids.concat();
        let flat_targets: Vec<i32> = targets.concat();
        let logprob = model.logprob(&flat_ids, &flat_targets, count, length).unwrap();
        for (row, expected) in case["coreml"].as_array().unwrap().iter().enumerate() {
            for (column, value) in expected.as_array().unwrap().iter().enumerate() {
                if flat_targets[row * length + column] != 0 {
                    worst = worst.max((logprob[row * length + column] - value.as_f64().unwrap()).abs());
                }
            }
        }
        // The decision code builds the same tensors from the text.
        let reranker = NeuralReranker::new(&directory, 1.0, Box::new(OnnxReranker::new(&directory).unwrap())).unwrap();
        let sentences: Vec<String> =
            case["sentences"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect();
        let context = case["context"].as_str().unwrap();
        let scores = reranker.scores(context, &sentences).unwrap();
        // The candidate's tokens follow <bos> and the last 24 context characters.
        let start = context.chars().count().min(24);
        for (row, sentence) in sentences.iter().enumerate() {
            let tokens = sentence.chars().count();
            let sum: f64 = (start..start + tokens).map(|c| logprob[row * length + c]).sum();
            let unknown = (0..tokens).filter(|&i| flat_targets[row * length + start + i] == 2).count() as f64;
            // The prefix-tree model, when present, computes the same values in another order.
            let expected = sum - unknown * 5465f64.ln();
            assert!((scores[row] - expected).abs() < 1e-2, "{sentence}: {} vs {expected}", scores[row]);
        }
    }
    eprintln!("reranker: largest difference from Core ML {worst}");
    assert!(worst < 0.1, "{worst}");
}

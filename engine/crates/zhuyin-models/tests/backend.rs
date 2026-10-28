//! The Swift engine's `BackendTests` (`ZhuyinMLTests`): the scorers' inputs, limits, batching and decisions
//! with fake numeric backends.
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use zhuyin_core::api::{CharacterModelSyllable, ScoredSentence, SentenceWord};
use zhuyin_models::{
    ModelError, NeuralReranker, PackedTree, RerankerModel, SyllableInput, SyllableModel, SyllableScorer,
};

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("zhuyin-models-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        for (file, tokens) in [
            ("vocab", vec!["<pad>", "<bos>", "<unk>", "甲", "乙"]),
            ("chars", vec!["<pad>", "<bos>", "<unk>", "<sep>", "甲", "乙"]),
            ("syllables", vec!["<pad>", "<literal>", "ㄐㄧㄚ"]),
        ] {
            std::fs::write(path.join(format!("{file}.json")), serde_json::to_vec(&tokens).unwrap()).unwrap();
        }
        Directory(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Shapes = Arc<Mutex<Vec<(usize, usize)>>>;

/// Answers −1 for every target and records each call's (rows, length).
struct Constant {
    shapes: Shapes,
    tree: Option<Arc<Mutex<Vec<PackedTree>>>>,
}

impl RerankerModel for Constant {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError> {
        self.shapes.lock().unwrap().push((rows, length));
        assert_eq!(ids.len(), rows * length);
        assert_eq!(targets.len(), ids.len());
        assert_eq!(ids[0], 1);
        Ok(vec![-1.0; ids.len()])
    }

    fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    fn tree_logprob(&mut self, tree: &PackedTree) -> Result<Vec<f64>, ModelError> {
        self.tree.as_ref().unwrap().lock().unwrap().push(tree.clone());
        Ok(vec![-1.0; tree.targets.len()])
    }
}

fn sentence(text: &str, score: f64, corrected: bool) -> ScoredSentence {
    ScoredSentence {
        words: vec![SentenceWord {
            range: 0..1,
            text: text.to_owned(),
            reading: String::new(),
            corrected,
            reading_corrected: false,
            channel: None,
        }],
        score,
    }
}

#[test]
fn reranker_inputs_penalty_batching_and_margin() {
    let directory = Directory::new("reranker");
    let shapes = Shapes::default();
    let model =
        NeuralReranker::new(directory.path(), 2.0, Box::new(Constant { shapes: shapes.clone(), tree: None })).unwrap();
    assert_eq!(model.scores("", &["甲"]), Some(vec![-1.0]));
    assert_eq!(shapes.lock().unwrap().last().unwrap().1, 2);
    assert!((model.scores("", &["丙"]).unwrap()[0] - (-1.0 - 5465f64.ln())).abs() < 1e-10);
    let _ = model.scores(&"甲".repeat(100), &["甲乙"]);
    // The last 24 context characters; <bos> predicts the first.
    assert_eq!(shapes.lock().unwrap().last().unwrap().1, 26);
    assert_eq!(model.candidate_scores("", &vec!["甲"; 65]), Some(vec![-2.0; 65]));
    let last: Vec<usize> = shapes.lock().unwrap().iter().rev().take(3).rev().map(|s| s.0).collect();
    assert_eq!(last, [32, 32, 1]);
    assert_eq!(model.choose(&[sentence("甲", 0.0, false), sentence("乙", 7.9, true)], ""), 0);
    assert_eq!(model.choose(&[sentence("甲", 0.0, false), sentence("乙", 8.0, true)], ""), 1);
    // Corrected only by the reading channel: the reading margin (4), not the keys' (8).
    let reading = |text: &str, score: f64| {
        let mut s = sentence(text, score, true);
        s.words[0].reading_corrected = true;
        s
    };
    assert_eq!(model.choose(&[sentence("甲", 0.0, false), reading("乙", 3.9)], ""), 0);
    assert_eq!(model.choose(&[sentence("甲", 0.0, false), reading("乙", 4.0)], ""), 1);
    assert_eq!(model.scores("", &["甲".repeat(64)]), None);
}

#[test]
fn reranker_gate_cache_switch_and_tree() {
    let directory = Directory::new("gate");
    let shapes = Shapes::default();
    let trees = Arc::new(Mutex::new(vec![]));
    let mut model = NeuralReranker::new(
        directory.path(),
        1.0,
        Box::new(Constant { shapes: shapes.clone(), tree: Some(trees.clone()) }),
    )
    .unwrap();
    // τ: far apart, the model is not consulted.
    assert_eq!(model.choose(&[sentence("甲", 9.0, false), sentence("乙", 0.0, false)], ""), 0);
    assert!(trees.lock().unwrap().is_empty());
    // Tree scoring: "甲" and "甲乙" share the first node.
    let scores = model.scores("乙", &["甲", "甲乙"]).unwrap();
    assert_eq!(scores, [-1.0, -2.0]);
    let tree = trees.lock().unwrap().last().unwrap().clone();
    assert_eq!(tree.targets, [1, 4, 3, 4]);
    assert!(shapes.lock().unwrap().is_empty());
    // More than 32 candidates use the batched model.
    let _ = model.scores("", &vec!["甲"; 33]);
    assert_eq!(shapes.lock().unwrap().len(), 1);
    // The same sentences and context are answered from the last choice.
    let calls = trees.lock().unwrap().len();
    let pair = [sentence("甲", 1.0, false), sentence("乙乙", 0.5, false)];
    assert_eq!(model.choose(&pair, "x"), 0);
    assert_eq!(model.choose(&pair, "x"), 0);
    assert_eq!(trees.lock().unwrap().len(), calls + 1);
    // δ: a switch must win by the switch margin.
    let flip = [sentence("乙乙", 0.5, false), sentence("甲", 0.0, false)];
    assert_eq!(model.choose(&flip, ""), 1);
    model.switch_margin = 1.0;
    assert_eq!(model.choose(&flip, ""), 0);
}

#[test]
fn configured_reads_weight_and_count() {
    let directory = Directory::new("configured");
    let shapes = Shapes::default();
    let plain = NeuralReranker::configured(directory.path(), Box::new(Constant { shapes: shapes.clone(), tree: None }))
        .unwrap();
    assert_eq!((plain.weight, plain.candidate_count), (0.5, 16));
    std::fs::write(directory.path().join("config.json"), r#"{"weight": 1.0, "count": 12}"#).unwrap();
    let configured = NeuralReranker::configured(directory.path(), Box::new(Constant { shapes, tree: None })).unwrap();
    assert_eq!((configured.weight, configured.candidate_count), (1.0, 12));
}

struct Recorder<F: FnMut(&SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> + Send>(F);

impl<F: FnMut(&SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> + Send> SyllableModel for Recorder<F> {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        (self.0)(input)
    }
}

#[test]
fn syllable_mapping_limits_and_output_shape() {
    let directory = Directory::new("syllable");
    let calls = Arc::new(Mutex::new(0));
    let counter = calls.clone();
    let model = SyllableScorer::new(
        directory.path(),
        Box::new(Recorder(move |input: &SyllableInput<'_>| {
            *counter.lock().unwrap() += 1;
            assert_eq!(input.context_ids.len(), 256);
            assert_eq!(*input.context_ids.last().unwrap(), 3);
            assert_eq!(input.syllable_ids, [2, 1]);
            assert_eq!(input.tones, [1, 0]);
            assert_eq!(input.costs, [2, 0]);
            assert_eq!(input.boundaries, [1, 0]);
            assert_eq!(input.candidates[0].len(), 64);
            assert_eq!(input.candidates[1][0], 2);
            assert_eq!(input.candidates[1][1], 0);
            Ok(input.candidates.iter().map(|row| vec![-0.5; row.len()]).collect())
        })),
    )
    .unwrap();
    let syllables = [
        CharacterModelSyllable { base: Some("ㄐㄧㄚ".to_owned()), tone: 1, cost_bucket: 2, implicit: true },
        CharacterModelSyllable { base: None, tone: 0, cost_bucket: 0, implicit: false },
    ];
    let context = "甲".repeat(300) + "\n";
    let candidates = vec![vec!["甲".to_owned(); 70], vec![]];
    let result = model.scores(&context, &syllables, &candidates);
    assert_eq!(result.map(|r| r.iter().map(Vec::len).collect::<Vec<_>>()), Some(vec![64, 0]));
    // The same request is answered from the last one.
    let _ = model.scores(&context, &syllables, &candidates);
    assert_eq!(*calls.lock().unwrap(), 1);
    assert_eq!(model.scores("", &[], &[]), None);
    let bad =
        SyllableScorer::new(directory.path(), Box::new(Recorder(|_: &SyllableInput<'_>| Ok(vec![vec![f64::NAN]]))))
            .unwrap();
    assert_eq!(bad.scores("", &syllables[..1], &[vec!["甲".to_owned()]]), None);
}

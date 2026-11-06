use super::*;
use crate::api::SentenceWord;
use crate::engine::EngineConfig;

fn composer_with_contextual_pick() -> Composer {
    let engine = Arc::new(Engine::from_text(
        "ㄇㄚ\t媽\t-1\nㄅㄚ\t八\t-1\nㄅㄚ\t巴\t-2\nㄋㄧˇ ㄏㄠˇ\t你好\t-1\n",
        None,
        None,
        EngineConfig::default(),
    ));
    engine.learn(
        &[CommittedWord { range: 0..1, text: "巴".into(), reading: "ㄅㄚ".into(), word_id: None, picked: true }],
        1,
        &[CommittedWord { range: 0..0, text: "媽".into(), reading: "ㄇㄚ".into(), word_id: None, picked: false }],
    );
    let mut composer = Composer::new(engine, ComposerOptions::default());
    composer.set_preceding_text("媽");
    composer
}

fn sentence(head: &str, tail: &str) -> ScoredSentence {
    let word = |range, text: &str, reading: &str| SentenceWord {
        range,
        text: text.into(),
        reading: reading.into(),
        corrected: false,
        reading_corrected: false,
        channel: None,
    };
    ScoredSentence { words: vec![word(0..1, head, "ㄅㄚ"), word(1..3, tail, "ㄋㄧˇ ㄏㄠˇ")], score: -1.0 }
}

fn remembered(range: Range<usize>, text: &str) -> ConvertedWord {
    ConvertedWord {
        range,
        text: text.into(),
        reading: "ㄋㄧˇ ㄏㄠˇ".into(),
        word_id: None,
        history: None,
        corrected: false,
        channel: None,
        reading_corrected: false,
    }
}

#[test]
fn contextual_pick_keeps_an_independent_remembered_range_before_counting() {
    let composer = composer_with_contextual_pick();
    let kept =
        composer.reranking_sentences(&[remembered(1..3, "你好")], vec![sentence("八", "你好"), sentence("巴", "你郝")]);
    assert_eq!(kept.iter().map(ScoredSentence::text).collect::<Vec<_>>(), ["八你好"]);
}

#[test]
fn unrelated_contextual_pick_does_not_bypass_an_unsatisfied_remembered_range() {
    let composer = composer_with_contextual_pick();
    let kept =
        composer.reranking_sentences(&[remembered(1..3, "你好")], vec![sentence("八", "你郝"), sentence("巴", "你郝")]);
    assert!(kept.is_empty(), "no compatible candidate leaves the original conversion in place");
}

#[test]
fn same_range_contextual_pick_can_replace_a_remembered_word() {
    let composer = composer_with_contextual_pick();
    let kept =
        composer.reranking_sentences(&[remembered(0..1, "八")], vec![sentence("八", "你好"), sentence("巴", "你好")]);
    assert_eq!(kept.iter().map(ScoredSentence::text).collect::<Vec<_>>(), ["巴你好"]);
}

#[test]
fn partial_contextual_pick_does_not_release_a_whole_recalled_phrase() {
    let composer = composer_with_contextual_pick();
    let kept = composer.reranking_sentences(&[remembered(0..3, "八你好")], vec![sentence("巴", "你郝")]);
    assert!(kept.is_empty(), "a partial pick cannot authorize changes elsewhere in the whole recall");
}

#[test]
fn no_contextual_pick_keeps_the_original_remembered_filter() {
    let mut composer = composer_with_contextual_pick();
    composer.set_preceding_text("");
    let protected = [remembered(1..3, "你好")];
    let kept = composer.reranking_sentences(&protected, vec![sentence("八", "你好"), sentence("巴", "你郝")]);
    assert_eq!(kept.iter().map(ScoredSentence::text).collect::<Vec<_>>(), ["八你好"]);
    assert!(composer.reranking_sentences(&protected, vec![sentence("巴", "你郝")]).is_empty());
    assert!(composer.reranking_sentences(&protected, Vec::new()).is_empty());
}

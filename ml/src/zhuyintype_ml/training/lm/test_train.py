"""Regression checks for the Wikipedia bigram: extraction, exhaustive Viterbi, pruned MKN mass, holdout exclusion,
reading ratios and JSON-serializable evaluation.  Skipped without the train dependency group.

    python -m unittest discover -s ml/src/zhuyintype_ml/training/lm -t ml/src
"""

from __future__ import annotations

import argparse
import json
import math
import tempfile
import unittest
from collections.abc import Iterator
from pathlib import Path

try:
    import numpy as np
    from opencc import OpenCC

    from zhuyintype_ml.training.lm import mkn, train
except ImportError:  # numpy, opencc, pyahocorasick, mwparserfromhell: the train dependency group
    train = None  # type: ignore[assignment]


@unittest.skipIf(train is None, "needs the train dependency group")
class TrainTests(unittest.TestCase):
    def test_taiwan_conversion_and_extraction(self) -> None:
        converter = OpenCC("s2twp")
        self.assertEqual(converter.convert("软件、鼠标、数据库、计算机"), "軟體、滑鼠、資料庫、計算機")
        text = (
            "== 标题 ==\n[[台湾|台湾地区]]的软件使用鼠标。\n<ref>应当移除的引用资料。</ref>\n"
            "{{模板|不应该保留模板内容}}\n{|\n| 表格内容不应该保留\n|}\n"
            "<syntaxhighlight>不应该保留的代码。</syntaxhighlight>\n"
            "-{zh-cn:软件使用鼠标;zh-tw:軟體使用滑鼠;}-！"
        )
        cleaned = train.clean(text, converter)
        self.assertEqual(cleaned, ["臺灣地區的軟體使用滑鼠", "軟體使用滑鼠"])
        self.assertEqual(train.clean("含有网址www.example.com的文字。太短。abcdefgh。", converter), [])

    def test_viterbi_matches_exhaustive_segmentation(self) -> None:
        words = ["<s>", "</s>", "<unk>", "甲", "乙", "丙", "甲乙", "乙丙", "甲乙丙"]
        ids = {w: i for i, w in enumerate(words)}
        scores = np.array([-10.0, -10.0, -12.0, -3.0, -3.5, -4.0, -4.0, -5.0, -9.0])
        trie = mkn.make_trie(words, set(words[3:]))
        sentence = "甲乙丙甲乙"

        def possibilities(s: str) -> Iterator[list[int]]:
            if not s:
                yield []
            for word in words[3:]:
                if s.startswith(word):
                    for rest in possibilities(s[len(word) :]):
                        yield [ids[word], *rest]

        expected = max(possibilities(sentence), key=lambda tokens: sum(scores[t] for t in tokens))
        self.assertEqual(mkn.segment(sentence, trie, ids, scores, -12.0), expected)
        self.assertEqual(mkn.segment("外", trie, ids, scores, -12.0), [2])

    def test_mkn_formula_pruning_and_unseen_contexts(self) -> None:
        # BOS=0, EOS=1, UNK=2, A=3, B=4; every count bin has support.
        triples = [(0, 3, 4), (0, 4, 2), (3, 1, 1), (3, 4, 3), (4, 1, 2), (4, 3, 1)]
        keys = np.array([(a << 32) | b for a, b, c in triples], dtype=np.uint64)
        counts = np.array([c for a, b, c in triples], dtype=np.uint64)
        u, kept, probs, alpha, stats = mkn.fit_mkn(keys, counts, 6, 3)
        self.assertEqual(stats["prune_count_threshold"], 3)
        self.assertEqual(len(kept), 2)
        self.assertGreater(u[5], 0)
        self.assertGreater(u[2], 0)
        self.assertAlmostEqual(float(u.sum()), 1.0)
        for a in range(6):
            total = 0.0
            for b in range(1, 6):
                key = (a << 32) | b
                index = np.flatnonzero(kept == key)
                total += float(probs[index[0]]) if len(index) else alpha[a] * u[b]
            self.assertAlmostEqual(total, 1.0, places=12)
        d, _ = mkn.discounts(counts)
        lam = (d[0] + d[2]) / 4  # A has counts 1 and 3.
        expected = (3 - d[2]) / 4 + lam * u[4]
        self.assertAlmostEqual(float(probs[np.flatnonzero(kept == ((3 << 32) | 4))[0]]), expected)

    def test_holdout_exclusion_includes_eight_character_substrings(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            sentences = [
                "這是訓練用的長句文字甲乙丙丁",
                "前綴這是訓練用的長句文字甲乙丙丁後綴",
                "完全不同的另一條句子內容",
                "短句文字",
            ]
            (work / "unique.txt").write_text("\n".join(sentences) + "\n")
            args = argparse.Namespace(work=work, output=work, seed=1, holdout=2, workers=2)
            stats = train.split_corpus(args)
            test = set((work / "test_sentences.txt").read_text().splitlines())
            training = [s for path in (work / "sentences").glob("*.txt") for s in path.read_text().splitlines()]
            self.assertFalse(test.intersection(training))
            forbidden = {s[i : i + 8] for s in test for i in range(len(s) - 7)}
            self.assertFalse(any(s[i : i + 8] in forbidden for s in training for i in range(len(s) - 7)))
            self.assertEqual(len(training), stats["training_sentences"])

    def test_reading_ratios_and_unseen_shift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            words = ["<s>", "</s>", "<unk>", "甲", "乙"]
            rows = [("a", "甲", math.log(0.06)), ("b", "甲", math.log(0.02)), ("c", "乙", math.log(0.001))]
            old = {"甲": 0.08, "乙": 0.001}
            u = np.array([0.0, 0.1, 0.01, 0.88, 0.01])
            model: mkn.Fitted = (u, np.array([3], dtype=np.uint64), np.array([0.5]), np.ones(5), {})
            args = argparse.Namespace(output=Path(directory))
            result = train.export(args, words, np.array([0, 0, 0, 100, 0]), old, rows, "# seed", model)
            lines = (args.output / "lexicon_tw.tsv").read_text().splitlines()[1:]
            values = [float(line.split("\t")[-1]) for line in lines]
            self.assertAlmostEqual(values[0] - values[1], math.log(3), places=10)
            self.assertAlmostEqual(math.exp(values[0]) + math.exp(values[1]), 0.88, places=10)
            self.assertAlmostEqual(values[2], rows[2][2] + result["unseen_lexicon_logp_shift"], places=10)

    def test_evaluation_is_json_serializable_with_unseen_tokens(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            words = ["<s>", "</s>", "<unk>", "甲", "乙", "外"]
            np.save(root / "segmentation_scores.npy", np.array([-10.0, -10.0, -10.0, -3.0, -4.0, -9.0]))
            (root / "test_sentences.txt").write_text("甲乙外\n")
            keys = np.array([(0 << 32) | 3, (3 << 32) | 4, (4 << 32) | 1], dtype=np.uint64)
            model = mkn.fit_mkn(keys, np.array([4, 2, 1], dtype=np.uint64), 6, 10)
            args = argparse.Namespace(work=root, output=root)
            result = train.evaluate(args, words, np.array([0, 0, 0, 4, 2, 0]), {"甲": 0.2, "乙": 0.1}, model)
            json.dumps(result)
            self.assertEqual(result["test_unseen_training_tokens"], 1)

    def test_serialized_model_round_trip(self) -> None:
        words = ["<s>", "</s>", "<unk>", "今天", "下雨"]
        keys = np.array([(0 << 32) | 3, (3 << 32) | 1, (0 << 32) | 4, (4 << 32) | 1], dtype=np.uint64)
        model = mkn.fit_mkn(keys, np.array([3, 3, 2, 2], dtype=np.uint64), 5, 10)
        with tempfile.TemporaryDirectory() as directory:
            mkn.write_tsv(Path(directory), words, model)
            self.assertLess(mkn.TsvModel(Path(directory)).verify()["all_contexts_max_error"], 1e-9)


if __name__ == "__main__":
    unittest.main()

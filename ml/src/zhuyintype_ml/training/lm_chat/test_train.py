"""lm_chat regression checks: untrusted TSV filtering, seeded disjoint holdouts, serialized MKN and unknown lookups.
Skipped without the train dependency group.

    python -m unittest discover -s ml/src/zhuyintype_ml/training/lm_chat -t ml/src
"""

from __future__ import annotations

import csv
import math
import tempfile
import unittest
from pathlib import Path

try:
    import numpy as np

    from zhuyintype_ml.training.lm import mkn
    from zhuyintype_ml.training.lm_chat import train
except ImportError:  # numpy, pyahocorasick: the train dependency group
    train = None  # type: ignore[assignment]


@unittest.skipIf(train is None, "needs the train dependency group")
class ChatTests(unittest.TestCase):
    def test_untrusted_tsv_filter_and_dedup(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            p = Path(directory) / "input.tsv"
            with p.open("w", newline="") as f:
                writer = csv.writer(f, delimiter="\t")
                writer.writerow(["sentence", "locale", "path"])
                for sentence, locale in [
                    ("今天 OK 嗎", "zh-TW"),
                    ("今天 OK 嗎", "zh-TW"),
                    ("今天hello嗎", "zh-TW"),
                    ("你", "zh-TW"),
                    ("明天再來", "zh-CN"),
                ]:
                    writer.writerow([sentence, locale, "$(touch forbidden); __import__('os')"])
            sentences, counts = train.read_tt(p)
            self.assertEqual(sentences, {"今天 OK 嗎"})
            self.assertEqual(counts["duplicate_rows"], 1)

    def test_seeded_disjoint_holdouts(self) -> None:
        rows = {
            "一二三四五六七八甲",
            "一二三四五六七八乙",
            "九十百千萬億兆京",
            "春夏秋冬東西南北",
            "風雨雷電山川湖海",
            "花草樹木鳥獸蟲魚",
            "今天下雨",
            "明天晴天",
        }
        result = train.select_holdouts(rows, {"明天晴天"}, {"今天下雨"}, size=2)
        self.assertEqual(result, train.select_holdouts(rows, {"明天晴天"}, {"今天下雨"}, size=2))
        test, dev, _ = result
        self.assertEqual(len(set(test + dev)), 4)
        self.assertFalse(set(test + dev) & {"今天下雨", "明天晴天"})
        for i, a in enumerate(test + dev):
            for b in (test + dev)[i + 1 :]:
                self.assertFalse(train.grams(a) & train.grams(b))

    def test_serialized_mkn_and_unknown_lookup(self) -> None:
        words = ["<s>", "</s>", "<unk>", "我", "你", "好"]
        pairs = [(0, 3, 5), (0, 4, 2), (3, 5, 5), (4, 5, 2), (5, 1, 7)]
        keys = np.array([(a << 32) | b for a, b, _ in pairs], dtype=np.uint64)
        counts = np.array([n for _, _, n in pairs], dtype=np.uint64)
        fitted = mkn.fit_mkn(keys, counts, len(words), 3)
        with tempfile.TemporaryDirectory() as directory:
            mkn.write_tsv(Path(directory), words, fitted)
            model = mkn.TsvModel(Path(directory))
            model.verify()
            self.assertAlmostEqual(model.score("missing", "missing"), model.score("<unk>", "<unk>"))
            self.assertAlmostEqual(model.score("我", "missing"), model.score("我", "<unk>"))
            self.assertTrue(all(math.isfinite(model.score(a, b)) for a in words for b in words[1:]))


if __name__ == "__main__":
    unittest.main()

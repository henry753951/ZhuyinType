"""corpus regression checks: cleaning rules, held-out exclusion, deduplication and the prompt grid.  Skipped without
the train dependency group (OpenCC, pyahocorasick, datasketch).

    python -m unittest discover -s ml/src/zhuyintype_ml/training/corpus -t ml/src
"""

from __future__ import annotations

import unittest

try:
    import datasketch  # noqa: F401  (Deduper imports it lazily)
    from opencc import OpenCC

    from zhuyintype_ml.training.corpus.clean import Deduper, normalized
    from zhuyintype_ml.training.corpus.prepare import exclusion
except ImportError:
    OpenCC = None

try:
    from zhuyintype_ml.training.corpus.generate import TOPICS, prompt
except ImportError:  # aiohttp: the train dependency group
    TOPICS = []


@unittest.skipIf(OpenCC is None, "needs the train dependency group")
class CleaningTest(unittest.TestCase):
    def test_cleaning_and_leakage(self) -> None:
        cc = OpenCC("s2t")
        tw = OpenCC("s2twp")
        self.assertEqual(normalized("1. 等一下要不要去買便當？", cc, tw)[0], "等一下要不要去買便當？")
        self.assertEqual(normalized("这个软件真的很好用", cc, tw)[1], "simplified")
        self.assertEqual(normalized("這個視頻真的很好看", cc, tw)[1], "mainland")
        blocked = exclusion(["我們明天一起去吃牛肉麵吧", "四字短句"])
        self.assertTrue(blocked("明天一起去吃牛肉麵吧，如何？"))
        self.assertTrue(blocked("四字短句"))
        self.assertFalse(blocked("晚上來我家打電動嗎"))

    def test_dedup(self) -> None:
        d = Deduper()
        self.assertTrue(d.add("今天晚上要不要去吃火鍋"))
        self.assertFalse(d.add("今天晚上要不要去吃火鍋"))
        self.assertTrue(d.add("這件事情我需要想一下"))


@unittest.skipIf(not TOPICS, "needs the train dependency group")
class GridTest(unittest.TestCase):
    def test_grid(self) -> None:
        self.assertEqual(len(TOPICS), 200)
        self.assertNotEqual(prompt(0), prompt(200))


if __name__ == "__main__":
    unittest.main()

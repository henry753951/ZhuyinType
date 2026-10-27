"""Tests of the loss categoriser (ml/src/zhuyintype_ml/eval/losses.py)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.eval import losses
from zhuyintype_ml.eval.rows import normalizer

LEXICON = """# test
ㄨㄛˇ\t我\t-3
ㄗㄞˋ\t在\t-3
ㄗㄞˋ\t再\t-4
ㄐㄧㄚ\t家\t-4
ㄐㄧㄚ\t加\t-4
ㄕˋ\t試\t-5
ㄐㄧㄥˋ\t鏡\t-6
ㄐㄧㄥˋ\t境\t-6
ㄅㄨˋ\t布\t-6
ㄅㄨˋ\t不\t-3
ㄧ\t一\t-3
ㄧˊ ㄍㄜˋ\t一個\t-4
ㄕㄣˊ ㄇㄜ˙\t什麼\t-4
ㄕㄣˊ\t神\t-5
ㄇㄛˊ\t魔\t-6
ㄍㄜˋ\t個\t-4
ㄍㄜˋ\t各\t-5
ㄎㄜˋ\t客\t-5
ㄖㄣˊ\t人\t-4
ㄎㄜˋ ㄖㄣˊ\t客人\t-6
ㄎㄜˋ ㄖㄣˊ\t課人\t-9
"""


class Losses(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        path = Path(self.dir.name) / "lexicon.tsv"
        path.write_text(LEXICON, encoding="utf-8")
        reference = Path(self.dir.name) / "reference.tsv"
        reference.write_text("ㄕˋ-ㄐㄧㄥˋ-ㄅㄨˋ\t拭鏡布\t-7\nㄕˋ\t拭\t-9\nㄐㄧㄥˋ-ㄅㄨˋ\t鏡布\t-8\n", encoding="utf-8")
        self.lexicon = losses.Vocabulary([path])
        self.reference = losses.Vocabulary([path, reference])

    def tearDown(self) -> None:
        self.dir.cleanup()

    def cause(self, truth: str, syllables: str, prediction: str, whole: bool = False) -> str:
        syl = losses.row_syllables(syllables)
        result = losses.classify(truth, syl, prediction, self.lexicon, self.reference, normalizer(), whole)
        return str(result["cause"])

    def test_causes(self) -> None:
        self.assertEqual(self.cause("我在家", "ㄨㄛˇ ㄗㄞˋ ㄐㄧㄚ", "我再家"), "char")
        self.assertEqual(self.cause("鏡布", "ㄐㄧㄥˋ ㄅㄨˋ", "境不"), "oov")
        self.assertEqual(self.cause("拭鏡布", "ㄕˋ ㄐㄧㄥˋ ㄅㄨˋ", "試鏡布"), "reading")
        self.assertEqual(self.cause("一個", "ㄧˋ ㄍㄜˋ", "億個"), "sandhi")
        self.assertEqual(self.cause("什麼", "ㄕㄣˊ ㄇㄛˊ", "神魔"), "reading")
        self.assertEqual(self.cause("什麼", "ㄕㄣˊ ㄇㄜ", "神魔"), "neutral")
        self.assertEqual(self.cause("我在", "ㄨㄛˋ ㄗㄞˋ", "握在"), "tone")
        self.assertEqual(self.cause("客人", "ㄎㄜˋ ㄖㄣˊ", "課人"), "word")
        self.assertEqual(self.cause("在加", "ㄗㄞˋ ㄐㄧㄚ", "再家"), "phrase")
        self.assertEqual(self.cause("在加", "ㄗㄞˋ ㄐㄧㄚ", "再家", whole=True), "oov")
        self.assertEqual(self.cause("我在", "ㄨㄛˇ ㄗㄞˋ", "我在家"), "length")

    def test_stages(self) -> None:
        fold = normalizer()
        row = {"truth": "我在家", "prediction": "我再家", "engine": "我再家", "nbest": ["我在家", "我再家"]}
        self.assertEqual(losses.stage(row, fold, 64), "reranker")
        row = {**row, "engine": "我在家", "nbest": ["我再家", "我在家"]}
        self.assertEqual(losses.stage(row, fold, 64), "v3")
        row = {**row, "engine": "我再家", "nbest": ["我再家"] * 20 + ["我在家"]}
        self.assertEqual(losses.stage(row, fold, 64), "poolK")
        self.assertEqual(losses.stage(row, fold, 16), "missing")


if __name__ == "__main__":
    unittest.main()

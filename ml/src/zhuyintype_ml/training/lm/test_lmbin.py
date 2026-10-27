"""ZTLM/1 writer and reader on small fixtures: UTF-8 ids, pruning, fallback, corruption rejection.  Skipped without
NumPy.

    python -m unittest discover -s ml/src/zhuyintype_ml/training/lm -t ml/src
"""

from __future__ import annotations

import math
import tempfile
import unittest
from pathlib import Path

try:
    from zhuyintype_ml.training.lm import lmbin
except ImportError:  # numpy: the train dependency group
    lmbin = None  # type: ignore[assignment]


@unittest.skipIf(lmbin is None, "needs the train dependency group")
class BinaryTest(unittest.TestCase):
    def test_unigram_fallback_in_order_two(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            arpa = Path(d) / "empty.arpa"
            binary = Path(d) / "lm.bin"
            arpa.write_text(
                "\\data\\\nngram 1=3\nngram 2=0\n\n\\1-grams:\n-0.301029995664\t</s>\t0\n-30\t<s>\t0\n"
                "-0.301029995664\t<unk>\t0\n\n\\2-grams:\n\n\\end\\\n"
            )
            result = lmbin.write(arpa, binary)
            self.assertEqual(result["bigrams"], 0)
            self.assertEqual(result["order"], 2)
            self.assertAlmostEqual(math.exp(lmbin.Model(binary).score("new", ["<s>"])), 0.5, places=6)
            with self.assertRaises(ValueError):
                lmbin.write(arpa, binary, max_trigrams=-1)

    def test_roundtrip_prune_and_corruption(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            arpa = Path(d) / "tiny.arpa"
            binary = Path(d) / "lm.bin"
            # P(甲)=.4, P(乙)=.3; P(乙|甲)=.6; P(乙|甲甲)=.8.
            uni = [("</s>", 0.2), ("<s>", 1e-30), ("<unk>", 0.1), ("乙", 0.3), ("甲", 0.4)]
            rows = ["\\data\\", "ngram 1=5", "ngram 2=2", "ngram 3=1", "", "\\1-grams:"]
            rows += [f"{math.log10(p)}\t{w}\t0" for w, p in uni]
            rows += ["", "\\2-grams:", f"{math.log10(0.2)}\t甲 甲\t0", f"{math.log10(0.6)}\t甲 乙\t0"]
            rows += ["", "\\3-grams:", f"{math.log10(0.8)}\t甲 甲 乙", "", "\\end\\"]
            arpa.write_text("\n".join(rows) + "\n")
            result = lmbin.write(arpa, binary)
            self.assertEqual(result["trigrams"], 1)
            m = lmbin.Model(binary)
            self.assertAlmostEqual(math.exp(m.score("乙", ["甲", "甲"])), 0.8, places=6)
            self.assertAlmostEqual(math.exp(m.score("乙", ["甲"])), 0.6, places=6)
            self.assertAlmostEqual(math.exp(m.score("新詞", [])), 0.1, places=6)
            lmbin.write(arpa, binary, max_trigrams=0)
            self.assertAlmostEqual(math.exp(lmbin.Model(binary).score("乙", ["甲", "甲"])), 0.6, places=6)
            data = bytearray(binary.read_bytes())
            data[0] = 0
            binary.write_bytes(data)
            with self.assertRaises(ValueError):
                lmbin.verify(binary)


if __name__ == "__main__":
    unittest.main()

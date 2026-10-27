"""Unit tests for the v3 readings, typing noise and exported model interfaces (no data directory needed).

    python -m unittest nlp.training.v3.test_v3

The model tests need PyTorch and are skipped without it; `zt train v3 selfcheck` runs the same checks on real data.
"""

from __future__ import annotations

import importlib.util
import random
import tempfile
import unittest
from pathlib import Path

from nlp.training.v3.noise import KEYS, Noise
from nlp.training.v3.readings import Lexicon, split_reading

HAS_TORCH = importlib.util.find_spec("torch") is not None

LEXICON = """# reading\tword\tlog prior
ㄐㄧㄣ ㄊㄧㄢ\t今天\t-3
ㄐㄧㄣ\t今\t-5
ㄐㄧㄣ\t金\t-4
ㄊㄧㄢ\t天\t-4
ㄊㄧㄢˊ\t田\t-6
ㄏㄜˊ\t和\t-3
ㄏㄢˋ\t和\t-5
ㄏㄜˊ\t河\t-6
ㄏㄢˋ\t漢\t-5
"""


class Readings(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        path = Path(self.directory.name) / "lexicon.tsv"
        path.write_text(LEXICON, encoding="utf-8")
        self.lexicon = Lexicon(path)

    def tearDown(self) -> None:
        self.directory.cleanup()

    def test_split_reading(self) -> None:
        self.assertEqual(split_reading("ㄓㄨㄥ"), ("ㄓㄨㄥ", 1))
        self.assertEqual(split_reading("ㄉㄜ˙"), ("ㄉㄜ", 5))

    def test_keys_in_keyboard_order(self) -> None:
        self.assertEqual(len(KEYS), 37)
        self.assertEqual("".join(KEYS), "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-")

    def test_longest_match_and_literals(self) -> None:
        segments = list(self.lexicon.segments("今天和X"))
        self.assertEqual([s[2] for s in segments], ["今天", "和", "X"])
        self.assertEqual(segments[-1][3], (("<literal>", 0),))
        readings, low = self.lexicon.label("今天和")
        self.assertEqual(readings, [("ㄐㄧㄣ", 1), ("ㄊㄧㄢ", 1), ("ㄏㄜ", 2)])
        self.assertTrue(low, "a polyphonic single character without a classifier is uncertain")
        self.assertEqual(self.lexicon.poly, {"和"})

    def test_noise_stays_valid(self) -> None:
        noise = Noise(self.lexicon.candidates)
        rng = random.Random(7)
        for reading in noise.encode:
            for _ in range(20):
                typed, cost = noise.corrupt(reading, rng)
                self.assertIn(typed, noise.encode)
                self.assertTrue(0 <= cost <= 3)
                self.assertEqual(cost == 0, typed == reading)
        for neighbours in noise.neighbours.values():
            self.assertLessEqual(len(neighbours), 3)
            self.assertLessEqual(set(neighbours), set(noise.encode))


@unittest.skipUnless(HAS_TORCH, "needs PyTorch")
class Interfaces(unittest.TestCase):
    def test_export_semantics(self) -> None:
        import torch

        from nlp.training.v3.models import ContextEncoder, ContextExport, SyllableEncoder, SyllableExport

        torch.manual_seed(0)
        context = ContextEncoder(40).eval()
        syllable = SyllableEncoder(40, 20).eval()
        syllable.initialize(context)
        with torch.no_grad():
            ids = torch.tensor([[1, 4, 5]])
            h = context.hidden(ids)
            other = context.hidden(torch.tensor([[1, 4, 9]]))
            torch.testing.assert_close(h[:, :2], other[:, :2])
            k, v = ContextExport(context, syllable)(ids)
            self.assertEqual(tuple(k.shape), (6, 3, 512))
            self.assertEqual(k.dtype, torch.float16)
            x = torch.ones(1, 2, dtype=torch.int64)
            candidates = torch.tensor([[[4, 5, 0], [6, 7, 8]]])
            wrapper = SyllableExport(syllable)
            args = (x, x, x * 0, x * 0, candidates, k, v)
            y = wrapper(*args)
            self.assertEqual(tuple(y.shape), (1, 2, 3))
            self.assertLess(y[0, 0, 2].item(), -9990)
            torch.testing.assert_close(y.exp().sum(-1), torch.ones(1, 2))
            padded = tuple(torch.cat((t, torch.zeros_like(t[:, :1])), 1) for t in args[:4])
            pad_candidates = torch.cat((candidates, torch.tensor([[[2, 0, 0]]])), 1)
            torch.testing.assert_close(wrapper(*padded, pad_candidates, k, v)[:, :2], y, atol=1e-5, rtol=1e-5)
            pk, pv = ContextExport(context, syllable)(torch.tensor([[1, 4, 5, 0, 0]]))
            torch.testing.assert_close(wrapper(*args[:5], pk, pv), y, atol=1e-5, rtol=1e-5)
            traced = torch.jit.trace(wrapper, args, check_trace=False)
            torch.testing.assert_close(y, traced(*args), atol=1e-5, rtol=1e-5)
            short = (x[:, :1], x[:, :1], x[:, :1] * 0, x[:, :1] * 0, candidates[:, :1, :2], k[:, :1], v[:, :1])
            torch.testing.assert_close(wrapper(*short), traced(*short), atol=1e-5, rtol=1e-5)


if __name__ == "__main__":
    unittest.main()

"""Unit tests for the reranker's data filter and the prefix-tree scorer (no data directory needed).

    python -m unittest nlp.training.reranker.test_reranker

The model tests need PyTorch and are skipped without it.
"""

from __future__ import annotations

import importlib.util
import unittest

HAS_NUMPY = importlib.util.find_spec("numpy") is not None
HAS_TORCH = importlib.util.find_spec("torch") is not None


@unittest.skipUnless(HAS_NUMPY, "needs NumPy")
class Filter(unittest.TestCase):
    def test_held_out_grams_and_length(self) -> None:
        from nlp.training.reranker import prepare

        prepare.init(prepare.grams("甲乙丙丁戊己庚辛"))
        self.assertIsNone(prepare.clean("前面甲乙丙丁戊己庚辛後面"))
        self.assertEqual(prepare.clean(" 甲乙丙丁戊己庚壬 \n"), "甲乙丙丁戊己庚壬")
        self.assertIsNone(prepare.clean("太短"))
        self.assertIsNone(prepare.clean("abcdefgh中文"))


@unittest.skipUnless(HAS_TORCH, "needs PyTorch")
class Tree(unittest.TestCase):
    def test_tree_matches_batched_model(self) -> None:
        import torch

        from nlp.training.reranker.export_tree import TreeScorer, example
        from nlp.training.reranker.train import CharLM, Config

        torch.manual_seed(0)
        model = CharLM(Config(vocab=16, layers=2, width=32, heads=2, ffn=64)).eval()
        scorer = TreeScorer(model).eval()
        with torch.no_grad():
            actual = scorer(*example())
            for token, node in [(4, 2), (5, 3)]:
                expected = model(torch.tensor([[1, 3]]))[0, -1].log_softmax(-1)[token]
                torch.testing.assert_close(actual[node], expected, atol=2e-5, rtol=2e-5)
            for branch, token, node in [(4, 6, 4), (5, 7, 5)]:
                expected = model(torch.tensor([[1, 3, branch]]))[0, -1].log_softmax(-1)[token]
                torch.testing.assert_close(actual[node], expected, atol=2e-5, rtol=2e-5)


if __name__ == "__main__":
    unittest.main()

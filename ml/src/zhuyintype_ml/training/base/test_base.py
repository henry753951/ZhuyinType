"""A1 regression checks: exact lattice posteriors against enumeration, the additive lexicon merge, the McBopomofo
parser, the frequency prior and the fallback renormalization.  Skipped without the train dependency group.

    python -m unittest discover -s ml/src/zhuyintype_ml/training/base -t ml/src
"""

from __future__ import annotations

import math
import tempfile
import unittest
from collections import Counter
from pathlib import Path

try:
    import numpy as np

    from zhuyintype_ml.training.base.fallback import correction
    from zhuyintype_ml.training.base.prior import distribution
    from zhuyintype_ml.training.base.train import expected, lattice, mcbopomofo, merge
    from zhuyintype_ml.training.lm import mkn
except ImportError:  # numpy, opencc, pyahocorasick: the train dependency group
    mkn = None  # type: ignore[assignment]


def walk(edges: list, end: int, i: int, tokens: list[int], p: float, paths: list) -> None:
    """Every segmentation path of a lattice with its product of probabilities."""
    if i == end:
        paths.append((tokens, p))
        return
    for j, token, score in edges[i]:
        walk(edges, end, j, tokens + [token], p * math.exp(score), paths)


@unittest.skipIf(mkn is None, "needs the train dependency group")
class BaseChecks(unittest.TestCase):
    def test_posterior_matches_enumeration(self) -> None:
        words = ["<s>", "</s>", "<unk>", "好", "吧", "好吧", "啊"]
        ids = {w: i for i, w in enumerate(words)}
        scores = np.log([0.01, 0.01, 0.01, 0.3, 0.2, 0.25, 0.22])
        trie = mkn.make_trie(words, set(words[3:]))
        for text in ("好吧", "好吧啊", "好好吧", "外好吧", ""):
            paths: list[tuple[list[int], float]] = []
            walk(lattice(text, trie, ids, scores), len(text), 0, [], 1.0, paths)
            z = sum(p for _, p in paths)
            uni: Counter[int] = Counter()
            bi: Counter[int] = Counter()
            for tokens, p in paths:
                for token in tokens:
                    uni[token] += p / z  # type: ignore[assignment]
                seq = [0, *tokens, 1]
                for a, b in zip(seq, seq[1:]):
                    bi[(a << 32) | b] += p / z  # type: ignore[assignment]
            got_u, got_b = expected(text, trie, ids, scores)
            self.assertEqual(uni.keys(), got_u.keys())
            self.assertEqual(bi.keys(), got_b.keys())
            self.assertTrue(all(abs(p - got_u[k]) < 1e-12 for k, p in uni.items()))
            self.assertTrue(all(abs(p - got_b[k]) < 1e-12 for k, p in bi.items()))
            self.assertLess(abs(sum(got_b.values()) - sum(got_u.values()) - 1), 1e-12)
        u, b = expected("好吧", trie, ids, scores)
        self.assertLess(abs(b[(ids["好"] << 32) | ids["吧"]] - (0.3 * 0.2) / (0.25 + 0.3 * 0.2)), 1e-12)

    def test_merge_and_parser(self) -> None:
        seed = [
            ("ㄧ", "一", -2.0),
            ("ㄧˊ", "一", -4.5),
            ("ㄅㄨˋ", "不", -2.0),
            ("ㄅㄨˊ", "不", -4.5),
            ("ㄏㄠˇ", "好", -3.0),
            ("ㄏㄠˋ", "好", -8.0),
            ("ㄧ ㄉㄧㄥˋ", "一定", -5.0),
        ]
        extra = {
            ("ㄏㄠˇ", "好"): -1.0,
            ("ㄏㄠ", "好"): -2.0,
            ("ㄅㄨˋ ㄧㄠˋ", "不要"): -6.0,
            ("ㄅㄨˊ ㄧㄠˋ", "不要"): -6.1,
            ("ㄉㄧˋ ㄧ ㄊㄞ", "第一胎"): -9.0,
        }
        result, _ = merge(seed, extra)
        self.assertTrue(all(result[r, w] == p for r, w, p in seed))
        self.assertLessEqual(result["ㄏㄠ", "好"], -4.0)
        self.assertIn(("ㄅㄨˊ ㄧㄠˋ", "不要"), result)
        self.assertNotIn(("ㄉㄧˋ ㄧˋ ㄊㄞ", "第一胎"), result)
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / "source"
            p.write_text(
                "# comment\nㄏㄠˇ-ㄅㄚ˙ 好吧 -2.0\n_punctuation_! ！ 0.0\n_punctuation_list 　 0.0\nㄐㄧˊ 𠮷 -5.0\n"
            )
            rows, rejected = mcbopomofo(p)
            self.assertEqual(rows, {("ㄏㄠˇ ㄅㄚ˙", "好吧"): -2 * math.log(10), ("ㄐㄧˊ", "𠮷"): -5 * math.log(10)})
            self.assertEqual(rejected["control_or_punctuation"], 2)

    def test_prior_distribution(self) -> None:
        p = distribution({"好": -1000.0, "吧": -1001.0}, 0.1)
        self.assertLess(abs(sum(math.exp(v) for v in p.values()) - 1), 1e-12)
        self.assertLess(abs((p["好"] - p["吧"]) - 1), 1e-12)

    def test_fallback_renormalization(self) -> None:
        # Original conditional: stored A=.5, backed-off B=.2, deleted C=.3.
        mass = 0.2
        alpha = 1.5
        scale = math.exp(correction(alpha, mass))
        restricted_alpha = alpha * (1 - mass) * scale
        self.assertLess(abs(0.5 * scale + restricted_alpha * (0.2 / alpha) / (1 - mass) - 1), 1e-12)
        self.assertEqual(correction(1.0, 0.0), 0.0)


if __name__ == "__main__":
    unittest.main()

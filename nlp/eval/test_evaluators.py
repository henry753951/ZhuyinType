"""End-to-end plumbing of the evaluators with a stand-in zhuyin-cli (nlp/eval/fake_cli.py), and the sampling every
evaluator shares: the same runs as the original tools picked."""

from __future__ import annotations

import os
import random
import re
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from nlp.common.zhuyin import Lexicon, keys, sample_runs
from nlp.eval import registry
from nlp.eval.fake_cli import fake_cli

LEXICON = "ㄅㄚ\t甲\t-1\nㄆㄚ\t乙\t-1\nㄇㄚ\t丙\t-1\nㄈㄚ\t丁\t-1\nㄉㄚ\t戊\t-1\nㄅㄚ ㄆㄚ\t甲乙\t-0.5\n"
# Commits 甲乙丙戊 until a pick, lists 戊 and 丁 at the cursor, and gives 甲乙 and 甲乙丙丁 as the two best.
ENGINE = """\
if '--report-list' in argv:
    return '戊\\t丁'
if '--report-nbest' in argv:
    return '甲乙丙戊\\t甲乙丙丁'
return '甲乙丙丁' if '<down>' in line else '甲乙丙戊'"""


class Evaluators(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        (self.root / "lexicon.tsv").write_text(LEXICON, encoding="utf-8")
        self.text = self.root / "text.txt"
        self.text.write_text("前文，甲乙丙丁。\n甲乙丙丁甲乙丙丁\n", encoding="utf-8")
        cli = fake_cli(self.root, ENGINE)
        patch = unittest.mock.patch.dict(os.environ, {"ZT_CLI": str(cli)})
        patch.start()
        self.addCleanup(patch.stop)
        self.addCleanup(self.directory.cleanup)

    def run_evaluator(self, name: str, *options: str):
        evaluator = registry.get(name)
        argv = ["--config", "engine", "--data", str(self.root), "--set", str(self.text), "--jobs", "2", *options]
        return evaluator.run(evaluator.arguments(argv), quiet=True)

    def test_conversion(self):
        report = self.run_evaluator("conversion", "--json", str(self.root / "report.json"))
        section = report.sections[0]
        self.assertEqual(section.metrics["runs"], 2)
        self.assertEqual(section.metrics["whole"], 0)
        self.assertEqual(section.metrics["character"], 6 / 12)
        self.assertTrue((self.root / "report.json").exists())

    def test_candidates(self):
        m = self.run_evaluator("candidates").sections[0].metrics
        # Only the 4-character run comes out with the right length; its one wrong character is first in the list.
        self.assertEqual((m["runs"], m["wrong"], m["first"], m["mean_rank"]), (2, 1, 1.0, 1.0))

    def test_repick(self):
        section = self.run_evaluator("repick", "--relearn").sections[0]
        self.assertEqual(section.metrics["runs"], 2)
        self.assertEqual(section.metrics["picks0"], 0)
        self.assertEqual(section.metrics["picks1"], 0.5)  # 甲乙丙丁 is fixed with one pick, the 8-character run is not
        self.assertEqual(section.items[0]["picks"], 1)

    def test_errors(self):
        m = self.run_evaluator("errors").sections[0].metrics
        self.assertEqual(m["habits"]["clean"], {"runs": 2, "shown": 0.0, "recall": 0.5})

    def test_typo(self):
        m = self.run_evaluator("typo", "-n", "2", "--rates", "0", "0.5").sections[0].metrics
        self.assertEqual(
            [(r["rate"], r["typo"]) for r in m["rows"]][:3], [(0, "off"), (0, "standard"), (0, "aggressive")]
        )
        self.assertEqual(m["rows"][0]["false_correction_rate"], 0)

    def test_typing_errors(self):
        section = self.run_evaluator("typing-errors", "-n", "2", "--conditions", "exact", "tone").sections[0]
        rows = section.metrics["rows"]
        self.assertEqual([(r["condition"], r["runs"]) for r in rows], [("exact", 2), ("tone", 0)])
        self.assertEqual(rows[0]["recall_at_16"], 0.5)


def old_selection(path: Path, lexicon: Lexicon, n: int, seed: int) -> list[tuple[str, str]]:
    """How the former candidate_eval.py, repick_eval.py and error_analysis.py picked runs and their keys."""
    with open(path, encoding="utf-8") as f:
        found = [m.group() for line in f for m in re.finditer(r"[一-鿿]{4,20}", line)]
    random.Random(seed).shuffle(found)
    runs = []
    for run in found:
        if len(runs) >= n:
            break
        readings = lexicon.readings(run)
        if readings and len(readings) == len(run):
            runs.append((run, keys(readings)))
    return runs


class Sampling(unittest.TestCase):
    def test_same_runs_as_the_original_tools(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "lexicon.tsv").write_text(LEXICON, encoding="utf-8")
            rng = random.Random(1)
            lines = ["".join(rng.choice("甲乙丙丁戊己，") for _ in range(rng.randint(3, 40))) for _ in range(200)]
            (root / "text.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
            lexicon = Lexicon(root / "lexicon.tsv")
            for n in (1, 10, 50, 1000):
                runs = sample_runs(root / "text.txt", lexicon, n, 7, whole_readings=True)
                self.assertEqual([(r.text, r.keys) for r in runs], old_selection(root / "text.txt", lexicon, n, 7))


if __name__ == "__main__":
    unittest.main()

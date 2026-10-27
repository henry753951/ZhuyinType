"""`zt eval corrections` end to end with a stand-in zhuyin-cli (zhuyintype_ml/eval/fake_cli.py)."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from zhuyintype_ml.core import corrections
from zhuyintype_ml.eval import registry
from zhuyintype_ml.eval.fake_cli import fake_cli

FIXTURE = Path(corrections.__file__).parent / "testdata" / "corrections.jsonl"
# Settings come from the record unless given; the typo in 1j6 is now corrected; everything else as before.
ENGINE = """\
assert argv.count('--forms') == 1, argv
keys = line.split('\\t')[1]
if '1<dt:150>j<dt:150>6' in keys:
    return '你步'
if keys == 's<dt:150>u<dt:150>3<enter>':
    return '你'
return '你好'"""


class CorrectionsEvaluator(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        (self.root / "lexicon.tsv").write_text("ㄅㄚ\t八\t-1\n", encoding="utf-8")
        patch = unittest.mock.patch.dict(os.environ, {"ZT_CLI": str(fake_cli(self.root, ENGINE))})
        patch.start()
        self.addCleanup(patch.stop)
        self.addCleanup(self.directory.cleanup)

    def run_evaluator(self, *options: str):
        evaluator = registry.get("corrections")
        argv = [str(FIXTURE), "--config", "engine", "--data", str(self.root), "--jobs", "2", *options]
        return evaluator.run(evaluator.arguments(argv), quiet=True)

    def test_outcomes_per_kind(self) -> None:
        report = self.run_evaluator("--json", str(self.root / "report.json"))
        metrics = report.sections[0].metrics
        self.assertEqual((metrics["records"], metrics["replayed"]), (5, 4))
        kinds = metrics["kinds"]
        self.assertEqual(kinds["pick"]["unchanged"], 1)
        self.assertEqual(kinds["backspace"]["right"], 1)
        self.assertEqual(kinds["escape"]["unchanged"], 1)
        self.assertEqual(kinds["delete_after_commit"]["unchanged"], 1)
        self.assertEqual((kinds["cursor_edit"]["records"], kinds["cursor_edit"]["replayed"]), (1, 0))
        items = json.loads((self.root / "report.json").read_text(encoding="utf-8"))["sections"][0]["items"]
        self.assertEqual(items[1]["id"], "00000000-0000-0000-0000-000000000002:backspace")
        self.assertEqual((items[1]["reference"], items[1]["output"]), ("你步", "你步"))

    def test_given_options_win_and_n(self) -> None:
        report = self.run_evaluator("--cli-args", "--forms standard", "-n", "1")
        self.assertEqual(report.sections[0].metrics["replayed"], 1)

    def test_files_required(self) -> None:
        evaluator = registry.get("corrections")
        with self.assertRaises(SystemExit):
            evaluator.run(evaluator.arguments(["--config", "engine", "--data", str(self.root)]), quiet=True)


if __name__ == "__main__":
    unittest.main()

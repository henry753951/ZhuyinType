"""Tests of the paired statistics (zhuyintype_ml/eval/stats.py): the checks of the former stats.py --self-test, and
pairing reports, row results and dumps."""

from __future__ import annotations

import argparse
import json
import tempfile
import unittest
import unittest.mock
from pathlib import Path

from zhuyintype_ml.eval import stats


class Statistics(unittest.TestCase):
    def test_mcnemar(self):
        self.assertEqual(stats.mcnemar(6, 0), 0.03125)
        self.assertEqual(stats.mcnemar(9, 1), 0.021484375)
        self.assertEqual(stats.mcnemar(0, 0), 1)
        self.assertEqual(stats.mcnemar(3, 3), 1)

    def test_poisson_upper(self):
        self.assertAlmostEqual(stats.poisson_upper(0), 2.99573227355, places=9)
        self.assertAlmostEqual(stats.poisson_upper(1), 4.74386451839, places=9)

    def test_minimum_detectable(self):
        self.assertEqual(stats.minimum_detectable(61, 0.05), 23)
        self.assertEqual(stats.minimum_detectable(61, 0.01), 27)
        self.assertIsNone(stats.minimum_detectable(0))
        self.assertIsNone(stats.minimum_detectable(5))

    def test_bootstrap(self):
        base = [(str(i), 1, 0, 4) for i in range(10)]
        result = stats.compare(base, base, resamples=100, jobs=1)
        self.assertEqual(result["delta_top1_ci95"], [0, 0])
        self.assertEqual(result["delta_cer_ci95"], [0, 0])
        changed = [(str(i), int(i >= 3), int(i < 3), 4) for i in range(10)]
        differences = [(c[1] - b[1], c[2] - b[2], 4, 1) for b, c in zip(base, changed)]
        self.assertEqual(stats.bootstrap(differences, 100, 7, 1), stats.bootstrap(differences, 100, 7, 2))
        self.assertEqual(
            stats.bootstrap(differences, 100, 7, 1),
            stats.bootstrap(differences, 100, 7, 1, [str(i) for i in range(10)]),
        )
        result = stats.compare(base, changed, resamples=100)
        self.assertEqual((result["fixed"], result["broken"], result["net"]), (0, 3, -3))

    def test_misordered_pairs(self):
        base = [(str(i), 1, 0, 4) for i in range(10)]
        with self.assertRaises(ValueError):
            stats.compare(base, list(reversed(base)), resamples=10)


class Inputs(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)

    def tearDown(self):
        self.directory.cleanup()

    def report(self, name: str, outputs: list[str]) -> Path:
        items = [{"id": i, "reference": "台灣你好", "output": out} for i, out in enumerate(outputs)]
        path = self.root / name
        path.write_text(json.dumps({"tool": "conversion", "sections": [{"name": "wiki-dev", "items": items}]}))
        return path

    def test_report_items_pair_by_id(self):
        base = self.report("base.json", ["台灣你好", "臺灣你好", "台灣妳好"])
        data = json.loads(base.read_text())
        data["sections"][0]["items"].reverse()
        new = self.root / "new.json"
        new.write_text(json.dumps(data))
        raw = stats.load(base, "raw")["wiki-dev"]
        self.assertEqual([o[1] for o in raw], [1, 0, 0])
        self.assertEqual(raw[1][2], 1)  # one character differs
        self.assertEqual([o[1] for o in stats.load(base, "variants")["wiki-dev"]], [1, 1, 1])
        paired = stats.pair(raw, stats.load(new, "raw")["wiki-dev"], "wiki-dev")
        self.assertEqual(paired, raw)

    def test_repeated_ids_in_order(self):
        base = [("a", 1, 0, 2), ("a", 0, 1, 2)]
        self.assertEqual(stats.pair(base, list(base), "rows"), base)
        with self.assertRaises(ValueError):
            stats.pair(base, list(reversed(base[:1])) + [("b", 1, 0, 2)], "rows")

    def test_unpaired_items(self):
        base = stats.load(self.report("a.json", ["x", "y"]), "raw")["wiki-dev"]
        new = stats.load(self.report("b.json", ["x"]), "raw")["wiki-dev"]
        with self.assertRaises(ValueError):
            stats.pair(base, new, "wiki-dev")

    def test_tsv_and_jsonl(self):
        tsv = self.root / "dump.tsv"
        tsv.write_text("你好\t你好\n台灣\t臺灣\n", encoding="utf-8")
        self.assertEqual(stats.load(tsv, "raw")[""], [("0", 1, 0, 2), ("1", 0, 1, 2)])
        jsonl = self.root / "rows.jsonl"
        rows = [{"row": 1, "id": "a", "ok_shanjie": 1, "errors_shanjie": 0, "gold_len": 3}]
        jsonl.write_text("".join(json.dumps(r) + "\n" for r in rows))
        self.assertEqual(stats.load(jsonl, "shanjie")[""], [("a", 1, 0, 3)])
        self.assertTrue(stats.has_rows(jsonl))
        self.assertFalse(stats.has_rows(tsv))

    def test_command(self):
        base, new = self.report("a.json", ["台灣你好", "x"]), self.report("b.json", ["台灣你好", "台灣你好"])
        parser = argparse.ArgumentParser()
        stats.add_arguments(parser)
        output = self.root / "out.json"
        args = parser.parse_args([str(base), str(new), "--resamples", "100", "--output", str(output)])
        with unittest.mock.patch("builtins.print"):
            self.assertEqual(stats.run(args), 0)
        result = json.loads(output.read_text())["wiki-dev"]
        self.assertEqual((result["fixed"], result["broken"], result["normalization"]), (1, 0, "raw"))


if __name__ == "__main__":
    unittest.main()

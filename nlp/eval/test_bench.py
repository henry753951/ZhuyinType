"""Tests of ZhuyinBench: the record keeps the schema and rounding of earlier records, and the pages render the
existing records exactly as committed."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from nlp.common import paths
from nlp.common.results import Report, Section
from nlp.eval.bench import history, suite

RECORD = paths.DOCS / "benchmarks" / "v0.4.0.json"


class Records(unittest.TestCase):
    def test_pages_render_existing_records(self):
        record = json.loads(RECORD.read_text(encoding="utf-8"))
        mac = history.load_mac(RECORD.with_name("v0.4.0-mac.json"))
        self.assertEqual(history.page(record, mac), RECORD.with_suffix(".md").read_text(encoding="utf-8"))

    def test_mac_report(self):
        """A `zt eval latency --json` report renders like the older metrics-mac.json."""
        record = json.loads(RECORD.read_text(encoding="utf-8"))
        old = history.load_mac(RECORD.with_name("v0.4.0-mac.json"))
        assert old is not None
        report = Report("latency", {}, {}, environment={"commit": "x"})
        report.add(Section("latency", {"machine": old["machine"], "macos": old["macos"], "median": old["median"]}))
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "v0.4.0-mac.json"
            report.save(path)
            self.assertEqual(history.page(record, history.load_mac(path)), history.page(record, old))
            self.assertIsNone(history.load_mac(Path(d) / "missing.json"))

    def test_rounding(self):
        self.assertEqual(suite.pct(0.8125), 81.2)
        self.assertEqual(suite.pct(0.83), 83.0)
        self.assertEqual(suite.pct(97 / 400), 24.2)
        self.assertEqual(suite.two(2.2551), 2.26)
        self.assertEqual(suite.key("wiki-test"), "test_sentences.txt")
        self.assertEqual(suite.key("smoke"), "colloquial_smoke.txt")

    def test_schema(self):
        record = json.loads(RECORD.read_text(encoding="utf-8"))["results"]
        report = Report("x", {}, {}, environment={"commit": "x"})
        report.add(Section("wiki-test", {"runs": 400, "character": 0.976, "whole": 0.8125, "variants": 0.8125}))
        self.assertEqual(
            suite.conversion(report),
            {"test_sentences.txt": {"runs": 400, "character": 97.6, "whole": 81.2, "variant_equal": 81.2}},
        )
        habits = Report("errors", {}, {}, environment={"commit": "x"})
        habits.add(Section("colloquial-test", {"habits": {"clean": {"runs": 400, "shown": 0.835, "recall": 0.98}}}))
        self.assertEqual(
            suite.habits(habits)["test_colloquial.txt"]["clean"], record["habits/app"]["test_colloquial.txt"]["clean"]
        )
        candidates = Report("candidates", {}, {}, environment={"commit": "x"})
        metrics = {"runs": 68, "wrong": 0, "first": 0, "first_page": 0, "listed": 0, "mean_rank": 0}
        candidates.add(Section("smoke", metrics))
        self.assertEqual(
            suite.candidates(candidates), {"colloquial_smoke.txt": record["candidates/app"]["colloquial_smoke.txt"]}
        )
        rows = Report("rows", {}, {}, environment={"commit": "x"})
        rows.add(Section("shanjie-typing76", {"n": 49, "shanjie": {"top1": 47 / 49, "cer": 0.0042, "oracle16": 1.0}}))
        self.assertEqual(suite.rows(rows), {"typing76": {"n": 49, "top1": 95.92, "cer": 0.42, "oracle16": 100.0}})
        repick = Report("repick", {}, {}, environment={"commit": "x"})
        values = {"runs": 68, "picks0": 1, "picks1": 1, "picks2": 1, "picks3": 1, "mean_picks": 0, "broke": 0}
        repick.add(Section("smoke", values))
        self.assertEqual(suite.repick(repick)["colloquial_smoke.txt"], record["repick/app"]["colloquial_smoke.txt"])


if __name__ == "__main__":
    unittest.main()

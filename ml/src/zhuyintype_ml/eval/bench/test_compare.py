"""The comparison's bookkeeping with stand-in systems: identical requests converted once, cells saved and resumed,
N/A cells, the record's metrics under both normalizations, the paired tests, and the page rendered from it."""

from __future__ import annotations

import os
import tempfile
import unittest
import unittest.mock
from pathlib import Path
from typing import Any

from zhuyintype_ml.eval.bench import compare, competition
from zhuyintype_ml.eval.systems.base import Case, Outcome, Segment, System, Task


class Fake(System):
    family = "shanjie"

    def __init__(self, name: str, answers: dict[str, str], context: bool = True) -> None:
        self.name, self.label, self.answers = name, name, answers
        self.capabilities = frozenset({"context"} if context else set())
        self.seen: list[str] = []

    def convert(self, task: Task, cases: list[Case]) -> list[Outcome]:
        self.seen += [c.id for c in cases]
        return [Outcome(self.answers.get(c.reference, c.reference)) for c in cases]

    def describe(self) -> dict[str, Any]:
        return {}


def case(i: int, text: str, context: str = "") -> Case:
    """A case whose keys depend on its text only (the same text: the same request)."""
    return Case(str(i), text, (Segment(text.translate(str.maketrans("臺", "台")), ("ㄅ",) * len(text)),), context)


class Bookkeeping(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.store = compare.Store(Path(directory.name))

    def test_identical_requests_once_and_context_only_where_taken(self):
        system = Fake("zhuyintype-app", {})
        cases = [case(0, "臺北", "前"), case(1, "臺北", "後"), case(2, "台北", "前")]
        plain = Task("x", "plain", "zhuyintype", cases)
        cell = compare.run_cell(system, plain, {}, self.store)
        # Without context all three are the same request (臺 and 台 are typed the same).
        self.assertEqual((cell["converted"], system.seen), (1, ["0"]))
        with_context = Task("x", "context", "zhuyintype", cases, uses_context=True)
        cache = self.store.cache(system.name)
        cell = compare.run_cell(system, with_context, cache, self.store)
        self.assertEqual(cell["converted"], 2)
        self.assertEqual(len(self.store.cache(system.name)), 3)

    def test_na_and_dropped_context(self):
        system = Fake("0c-full", {}, context=False)
        cases = [case(0, "甲", "前"), case(1, "乙")]
        self.assertEqual(
            compare.run_cell(system, Task("x", "context", "zhuyintype", cases, uses_context=True), {}, self.store),
            {"status": "n/a", "reason": "no-context"},
        )
        cell = compare.run_cell(system, Task("x", "rows", "shanjie", cases, uses_context=True), {}, self.store)
        self.assertEqual(cell["context_dropped"], 1)

    def test_unsupported_tasks_run_a_sample(self):
        system = Fake("shanjie-chat", {})
        cases = [Case(str(i), f"字{i}", (Segment(f"k{i}"),)) for i in range(compare.SAMPLE + 20)]
        cell = compare.run_cell(system, Task("x", "typing/hotspot", "zhuyintype", cases), {}, self.store)
        self.assertEqual((len(cell["outputs"]), cell["unsupported"]), (compare.SAMPLE, ["key-errors"]))
        cell = compare.run_cell(system, Task("x", "typing/exact", "zhuyintype", cases), {}, self.store)
        self.assertEqual(len(cell["outputs"]), len(cases))
        self.assertNotIn("unsupported", cell)

    def test_resume_requires_matching_fingerprint(self):
        inputs = {"tasks": [{"id": "plain/x", "cases": ["one"]}], "n": 1, "typing_n": 2, "version": "v1"}
        compare.check_run_fingerprint(self.store, inputs)
        compare.check_run_fingerprint(self.store, inputs)
        for key, value in (("tasks", []), ("n", 3), ("typing_n", 4), ("version", "v2")):
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, "new label"):
                compare.check_run_fingerprint(self.store, {**inputs, key: value})
        self.assertEqual(self.store.read(self.store.root / "run.json")["fingerprint"].__len__(), 64)

    def test_run_inputs_capture_task_lexicon_and_system_configuration(self):
        root = self.store.root / "inputs"
        root.mkdir()
        (root / "lexicon.tsv").write_text("first", encoding="utf-8")
        (root / "assets.json").write_text("{}", encoding="utf-8")
        tasks = [Task("x", "plain", "zhuyintype", [case(0, "甲")])]
        system = Fake("example", {})
        with (
            unittest.mock.patch.object(compare.paths, "RESOURCES", root),
            unittest.mock.patch.object(compare, "EngineConfig") as config,
            unittest.mock.patch.object(compare.registry, "create", return_value=system),
            unittest.mock.patch.object(compare, "commit", return_value="c1"),
            unittest.mock.patch.object(compare, "app_version", return_value="v1"),
        ):
            config.return_value.reading_lexicon = root / "lexicon.tsv"
            inputs = compare.run_inputs(tasks, ["example"], 1, 2, 4, {})
            store = compare.Store(self.store.root / "run")
            compare.check_run_fingerprint(store, inputs)
            (root / "lexicon.tsv").write_text("second", encoding="utf-8")
            changed = compare.run_inputs(tasks, ["example"], 1, 2, 4, {})
            with self.assertRaisesRegex(SystemExit, "new label"):
                compare.check_run_fingerprint(store, changed)
            self.assertEqual(inputs["tasks"][0]["cases"][0]["reference"], "甲")
            self.assertEqual(inputs["systems"][0]["name"], "example")

    def test_legacy_run_is_rejected(self):
        self.store.write(self.store.root / "cells" / "old.json", {})
        with self.assertRaisesRegex(SystemExit, "no fingerprint"):
            compare.check_run_fingerprint(self.store, {})

    def test_budget(self):
        budget = compare.Budget(16, ["zhuyintype-app", "shanjie-chat", "0c-full"])
        # 0c's model servers count once they run, two processes each; the others share the rest by weight.
        self.assertEqual(
            (budget.share("0c-full"), budget.share("zhuyintype-app"), budget.share("shanjie-chat")), (2, 12, 4)
        )
        budget.begin("0c-full")
        self.assertEqual((budget.share("zhuyintype-app"), budget.share("shanjie-chat")), (9, 3))
        budget.release("shanjie-chat")
        self.assertEqual(budget.share("zhuyintype-app"), 12)
        budget.release("0c-full")
        self.assertEqual(budget.share("zhuyintype-app"), 16)
        self.assertEqual(compare.Budget(16, ["a", "b"], {"a": 3}).share("a"), 3)

    def test_metrics_and_pairs(self):
        folds = compare.folds(None)
        references = ["臺北", "她們", "甲乙", "丙丁"]
        base = ["台北", "他們", "甲乙", "丙乙"]
        other = ["臺北", "她們", "甲丁", "丙丁"]
        m = compare.metrics(references, base, {"3": "no-path"}, folds)
        self.assertEqual((m["raw"], m["S"], m["V"], m["no-path"]), (0.25, 0.75, 0.5, 1))
        self.assertAlmostEqual(m["character"], 1 - 3 / 8)
        pairs = compare.paired(references, base, other, folds)
        self.assertEqual((pairs["V"]["only_base"], pairs["V"]["only_other"]), (1, 2))
        self.assertEqual((pairs["S"]["only_base"], pairs["S"]["only_other"]), (1, 1))
        self.assertEqual(pairs["S"]["p"], 1.0)

    def test_record_and_page(self):
        tasks = [
            Task("wiki-test", "plain", "zhuyintype", [case(0, "臺北"), case(1, "甲乙")]),
            Task("wiki-test", "context", "zhuyintype", [case(0, "臺北", "前"), case(1, "甲乙")], uses_context=True),
            Task("sj/dev/names/stores", "rows", "shanjie", [case(0, "店名", "前")], uses_context=True),
            Task("colloquial-test", "typing/hotspot", "zhuyintype", [Case("0", "丙", (Segment("x"),))]),
        ]
        systems = {
            "zhuyintype-app": Fake("zhuyintype-app", {"甲乙": "甲已"}),
            "0c-full": Fake("0c-full", {"臺北": "颱北"}, context=False),
            "shanjie-chat": Fake("shanjie-chat", {"店名": "店明"}),
        }
        for name, system in systems.items():
            for task in tasks:
                self.store.write(self.store.cell(name, task.id), compare.run_cell(system, task, {}, self.store))
            describe = {"label": name, "family": system.family, "capabilities": sorted(system.capabilities)}
            self.store.write(self.store.root / "systems" / f"{name}.json", describe)
        with unittest.mock.patch.dict(os.environ, {"ZT_SYSTEMS_DIR": str(self.store.root / "none")}):
            record = compare.assemble("t", list(systems), tasks, self.store, 2, 1)
        stores = record["tasks"]["rows/sj/dev/names/stores"]
        self.assertEqual(stores["cells"]["shanjie-chat"]["metrics"]["S"], 0)
        self.assertEqual(stores["cells"]["0c-full"]["context_dropped"], 1)
        self.assertEqual(record["tasks"]["context/wiki-test"]["cells"]["0c-full"]["status"], "n/a")
        self.assertEqual(record["tasks"]["plain/wiki-test"]["paired"]["0c-full"]["V"]["only_other"], 1)
        page = competition.render(record)
        # The tracked record keeps numbers only, and gives the same page.
        slim = competition.slim(record)
        self.assertNotIn("outputs", slim["tasks"]["plain/wiki-test"]["cells"]["0c-full"])
        self.assertNotIn("references", slim["tasks"]["plain/wiki-test"])
        self.assertEqual(competition.render(slim), page)
        self.assertIn("## 總覽", page)
        self.assertIn("| 店名 | 1 | **100.0**／**100.0** | **100.0**／**100.0** | 0.0／0.0 |", page)
        self.assertIn("N/A", page)
        for word in ("領先", "落後", "超越", "打敗", "對手"):
            self.assertNotIn(word, page)


if __name__ == "__main__":
    unittest.main()

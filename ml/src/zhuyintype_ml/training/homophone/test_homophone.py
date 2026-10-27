"""Regression tests for exclusions, categories, splits, shard filtering, scoring and the training sampler.

    python -m unittest zhuyintype_ml.training.homophone.test_homophone

Tests that need NumPy, PyTorch or the prepared data (GPU host) are skipped without them.
"""

from __future__ import annotations

import importlib.util
import json
import random
import tempfile
import unittest
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from zhuyintype_ml.training.homophone.common import BASE, CATEGORIES, HELD, RUN, Exclusions, holdouts, positions
from zhuyintype_ml.training.homophone.compatible import blocked_fields
from zhuyintype_ml.training.homophone.evaluate import align, metrics
from zhuyintype_ml.training.v3.readings import Lexicon

HAS_NUMPY = importlib.util.find_spec("numpy") is not None
HAS_TORCH = importlib.util.find_spec("torch") is not None
HAS_AHOCORASICK = importlib.util.find_spec("ahocorasick") is not None

LEXICON = """ㄨㄛˇ\t我\t-2
ㄒㄧㄢˋ ㄗㄞˋ\t現在\t-3
ㄗㄞˋ\t在\t-2
ㄗㄞˋ\t再\t-3
ㄇㄤˊ\t忙\t-4
ㄔ\t吃\t-3
ㄧ\t一\t-2
ㄨㄢˇ\t碗\t-5
ㄉㄥˇ\t等\t-4
ㄨˇ\t五\t-4
ㄈㄣ ㄓㄨㄥ\t分鐘\t-4
ㄐㄧㄥˇ ㄈㄤ\t警方\t-5
ㄉㄧㄠˋ ㄔㄚˊ\t調查\t-5
"""


class Exclusion(unittest.TestCase):
    def test_exclusions(self) -> None:
        held = Exclusions(["甲乙丙丁戊己庚辛壬", "我在家"])
        self.assertIn("前甲乙丙丁戊己庚辛後", held)
        self.assertIn("我在家", held)
        self.assertIn("今天我在家吃飯", held)
        self.assertNotIn("甲乙丙丁戊己庚壬", held)

    @unittest.skipUnless(HAS_AHOCORASICK, "needs pyahocorasick")
    def test_compiled_exclusions(self) -> None:
        held = Exclusions(["一二三四五六七八", "我在家"]).compile()
        self.assertIn("零一二三四五六七八九", held)
        self.assertIn("他說我在家呢", held)
        self.assertNotIn("再試一次", held)

    def test_short_context_is_not_a_forbidden_sentence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "inputs").mkdir()
            (root / "smoke.txt").write_text("")
            for path in HELD:
                (root / "inputs" / Path(path).name).write_text("")
            (root / "dev_zai.txt").write_text("，\t甲乙丙丁戊己庚辛\n無關短文\t壬癸子丑寅卯辰巳\n")
            held = holdouts(new=True, directory=root)
            self.assertNotIn("這是無關，文字", held)
            self.assertNotIn("前無關短文後", held)
            self.assertIn("前甲乙丙丁戊己庚辛後", held)
            self.assertIn("今天我在家吃飯", held)

    def test_evaluation_columns_and_join(self) -> None:
        self.assertEqual(
            blocked_fields("前文前文前文前文|目標|ㄇㄨˋ ㄅㄧㄠ"), ["目標", "前文前文前文前文", "前文前文前文前文目標"]
        )
        self.assertEqual(blocked_fields("短\t測試句子"), ["測試句子", "短測試句子"])
        self.assertEqual(blocked_fields("# comment"), [])


class Scoring(unittest.TestCase):
    def test_alignment(self) -> None:
        self.assertEqual(align("我再吃一碗", "我再吃一碗"), dict(enumerate("我再吃一碗")))
        self.assertEqual(align("我再吃一碗", "我又再吃一碗")[1], "再")
        self.assertEqual(align("我再吃一碗", "我吃一碗")[1], "")

    def test_metrics(self) -> None:
        rows = [
            dict(
                text="我再吃一碗",
                context="",
                positions=[dict(index=1, char="再", group="在再", category="after_subject")],
            )
        ]
        result = metrics(rows, ["我又再吃一碗"])
        self.assertEqual(result["positions"]["all"]["accuracy"], 1)
        self.assertEqual(result["whole_accuracy"], 0)

    def test_evaluator_with_a_stand_in_engine(self) -> None:
        import os
        import sys
        from unittest.mock import patch

        from zhuyintype_ml.training.homophone.evaluate import Homophone, run

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "lexicon.tsv").write_text(LEXICON, encoding="utf-8")
            (root / "inventory.json").write_text(json.dumps(dict(kept=[dict(chars="在再", readings=[["ㄗㄞ", 4]])])))
            (root / "smoke.txt").write_text("我再吃一碗\n我在忙\n")
            # Answers 在 for every 在／再 key sequence, as an engine that always prefers 在 would.
            answers = {"ji3y94t<space>u<space>j03<enter>": "我在吃一碗", "ji3y94a;6<enter>": "我在忙"}
            cli = root / "zhuyin-cli"
            cli.write_text(
                f"#!{sys.executable}\nimport sys\nanswers = {answers!r}\n"
                "for line in sys.stdin: print(answers[line.strip()])\n"
            )
            cli.chmod(0o755)
            with patch.dict(os.environ, ZT_CLI=str(cli)):
                run(
                    Homophone(),
                    [
                        "--config",
                        "engine",
                        "--data",
                        str(root),
                        "--set",
                        str(root / "smoke.txt"),
                        "--jobs",
                        "1",
                        "--json",
                        str(root / "report.json"),
                    ],
                    quiet=True,
                )
            section = json.loads((root / "report.json").read_text())["sections"][0]
            self.assertEqual(section["metrics"]["positions"]["all"], dict(correct=1, total=2, accuracy=0.5))
            self.assertEqual(section["metrics"]["whole_correct"], 1)
            self.assertEqual([p["hit"] for item in section["items"] for p in item["positions"]], [False, True])

    def test_categories(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "lexicon.tsv"
            path.write_text(LEXICON, encoding="utf-8")
            lex = Lexicon(path)
        inventory = [dict(chars="在再", readings=[["ㄗㄞ", 4]])]

        def category(text: str, index: int) -> str:
            return next(p for p in positions(text, lex, inventory) if p["index"] == index)["category"]

        self.assertEqual(category("我現在在忙", 2), "fixed")
        self.assertEqual(category("我再吃一碗", 1), "after_subject")
        self.assertEqual(category("再等我五分鐘", 0), "initial")
        self.assertEqual(category("警方在調查", 2), "after_subject")
        native = [dict(chars="在再", readings=[("ㄗㄞ", 4)])]
        self.assertEqual(positions("我再吃一碗", lex, native), positions("我再吃一碗", lex, inventory))


@unittest.skipUnless(HAS_NUMPY, "needs NumPy")
class Data(unittest.TestCase):
    def test_context_filter(self) -> None:
        import numpy as np

        from zhuyintype_ml.training.homophone import filter as filtering

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "encoded").mkdir()
            (root / "filtered").mkdir()
            vocab = ["<pad>", "<bos>", "<unk>", "<sep>"] + list("甲乙丙丁戊己庚辛在我再吃")
            ids = [1] + list(range(4, 13)) + [1, 13, 14, 15]
            np.asarray(ids, np.uint16).tofile(root / "encoded/t.ids")
            np.save(root / "encoded/t.documents.npy", np.asarray([[0, 10], [10, 14]], np.int64))
            np.save(root / "encoded/t.examples.npy", np.asarray([[9, 10, 0], [11, 14, 10]], np.int64))
            filtering.BASE = filtering.OUT = root
            filtering.HELD = Exclusions(["甲乙丙丁戊己庚辛"])
            filtering.VOCAB = vocab
            filtering.GROUPS = [dict(chars="在再")]
            filtering.CHAR_GROUPS = {i: ["在再"] if c in "在再" else [] for i, c in enumerate(vocab)}
            result = filtering.filter_shard(dict(name="t"))
            self.assertEqual((result["allowed"], result["rejected"]), (1, 1))
            self.assertEqual(np.load(root / "filtered/t.normal.npy").tolist(), [1])
            self.assertEqual(np.load(root / "filtered/t.primary-再.npy").tolist(), [1])

    def test_balanced_splits_do_not_exhaust_rare_side(self) -> None:
        from zhuyintype_ml.training.homophone import split

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            rng = random.Random(42)
            alphabet = [chr(n) for n in range(0x4E00, 0x9FFF) if chr(n) not in "在再"]
            rows = []
            for c in "在再":
                for _ in range(1000):
                    text = c + "".join(rng.choices(alphabet, k=10))
                    rows.append(
                        dict(
                            text=text,
                            context="",
                            source="taiwan_tongues_train",
                            offset=0,
                            positions=[dict(index=0, char=c, group="在再", category="initial")],
                            judge=dict(accepted=True),
                        )
                    )
            (root / "inventory.json").write_text(json.dumps(dict(kept=[dict(chars="在再")])))
            (root / "lexicon.tsv").write_text("".join("ㄗㄞˋ\t" + c + "\t-1\n" for c in alphabet + list("在再")))
            (root / "judged.jsonl").write_text("\n".join(map(json.dumps, rows)))
            split.main([], out=root, held=Exclusions)
            stats = json.loads((root / "splits.json").read_text())["sets"]
            for name in ["dev_zai", "test_zai"]:
                self.assertEqual(stats[name]["characters"], {"在": 500, "再": 500})
                self.assertEqual(stats[name]["sources"], {"taiwan_tongues_train": 1000})
            self.assertEqual(
                set(CATEGORIES), {"fixed", "after_subject", "initial", "before_verb", "before_place_time", "other"}
            )


@unittest.skipUnless(
    HAS_TORCH and (RUN / "filtered.json").exists() and (BASE / "encoded").exists(),
    "needs PyTorch, the prepared homophone2/ and the v3_030 shards (GPU host)",
)
class Sampler(unittest.TestCase):
    def test_sampler_reads_only_real_sources(self) -> None:
        from unittest.mock import patch

        from zhuyintype_ml.training.homophone import train
        from zhuyintype_ml.training.homophone.common import jsonl

        opened = []

        def checked(path: Path) -> Iterator[dict[str, Any]]:
            self.assertEqual(Path(path).name, "real-train.jsonl")
            opened.append(str(path))
            return jsonl(path)

        with patch.object(train, "jsonl", checked):
            stream = iter(train.Batches(2, RUN))
            for _ in range(12):
                tensors = next(stream)
                self.assertEqual(len(tensors), 7)
                self.assertEqual(tensors[0].shape[0], 2)
        self.assertEqual(len(opened), 1)


if __name__ == "__main__":
    unittest.main()

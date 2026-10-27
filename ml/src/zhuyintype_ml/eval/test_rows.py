"""Tests of row sets (ml/src/zhuyintype_ml/eval/rows.py): the checks of the former rows_eval.py --self-test."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.core.engine import EngineConfig, EnginePool
from zhuyintype_ml.core.zhuyin import syllable_keys
from zhuyintype_ml.eval import rows
from zhuyintype_ml.eval.fake_cli import fake_cli


class Rows(unittest.TestCase):
    def test_keys(self):
        self.assertEqual(syllable_keys("ㄋㄧˇ ㄏㄠˇ ㄇㄚ˙"), "su3cl3a87")
        self.assertEqual(syllable_keys("ㄊㄚ ˙ㄉㄜ"), "w8<space>2k7")
        for invalid in ("", "abc", "˙", "ㄋㄧˇˋ", "ㄋˊㄧ"):
            with self.assertRaises(ValueError, msg=invalid):
                syllable_keys(invalid)

    def test_scoring(self):
        self.assertEqual(rows.edit_distance("𠮷你好", "你好啊"), 2)
        self.assertEqual(rows.normalizer()("她妳它牠嘗周臺裏"), "他你他他嚐週台裡")
        r = rows.score_row({"truth": "她在臺灣"}, "他在台灣", ["她在臺灣"], rows.normalizer())
        self.assertEqual((r["ok_shanjie"], r["errors_shanjie"], r["ok_variants"], r["errors_raw"]), (1, 0, 0, 2))
        self.assertEqual((r["oracle16_raw"], r["gold_len"]), (1, 4))
        summary = rows.summary([r])
        self.assertEqual(summary["shanjie"], {"top1": 1.0, "cer": 0.0, "oracle16": 1.0})
        self.assertEqual(summary["raw"]["cer"], 0.5)

    def test_files_and_engine(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            variants = root / "variants.tsv"
            variants.write_text("甲乙\t丙\n甲\t丁\n", encoding="utf-8")
            self.assertEqual(rows.normalizer(variants)("甲乙甲"), "丙丁")
            path = root / "rows.txt"
            path.write_text("# comment\n前文|你好|ㄋㄧˇ ㄏㄠˇ\n|他|ㄊㄚ\n", encoding="utf-8")
            loaded = rows.load_rows(path)
            self.assertEqual([r["row"] for r in loaded], [1, 2])
            self.assertEqual(loaded[1]["keys"], "w8<space>")
            cli = fake_cli(
                root,
                "oracle = '--report-nbest' in argv\n"
                "if line.startswith('前文\\t'):\n"
                "    return '你好\\t你號' if oracle else '你好'\n"
                "return '他' if oracle else ''",
            )
            pool = EnginePool(EngineConfig("engine", root), jobs=2, cli=cli)
            got = rows.evaluate_rows(loaded, pool, rows.normalizer())
            self.assertEqual((got[0]["ok_raw"], got[1]["prediction"], got[1]["oracle16_raw"]), (1, "", 1))
            path.write_text("bad row\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                rows.load_rows(path)
            failing = fake_cli(root, "raise SystemExit(3)")
            with self.assertRaises(RuntimeError):
                EnginePool(EngineConfig("engine", root), cli=failing).run(["one"])


if __name__ == "__main__":
    unittest.main()

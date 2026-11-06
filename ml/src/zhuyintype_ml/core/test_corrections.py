"""Unit tests for zhuyintype_ml.core.corrections, on a small correction log written by the app's recorder."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.core import corrections

FIXTURE = Path(__file__).parent / "testdata" / "corrections.jsonl"


class CorrectionLogTest(unittest.TestCase):
    def setUp(self) -> None:
        self.records = corrections.load(FIXTURE)

    def test_load(self) -> None:
        self.assertEqual(len(self.records), 5)
        self.assertEqual(self.records[0]["corrections"], ["pick"])
        with tempfile.TemporaryDirectory() as directory:
            bad = Path(directory) / "bad.jsonl"
            bad.write_text('{"schema": 2}\n', encoding="utf-8")
            with self.assertRaises(ValueError):
                corrections.load(bad)
            bad.write_text("not json\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                corrections.load(bad)
            # Text the user typed may hold U+2028, which JSON leaves unescaped: still one record per line.
            separated = Path(directory) / "separated.jsonl"
            first = FIXTURE.read_text(encoding="utf-8").split("\n")[0]
            separated.write_text(first.rstrip()[:-1] + ', "note": "a\u2028b"}\n', encoding="utf-8")
            self.assertEqual(corrections.load(separated)[0]["note"], "a\u2028b")

    def test_optional_candidate_identity_is_backward_compatible(self) -> None:
        record = json.loads(json.dumps(self.records[0]))
        pick = next(e for e in record["events"] if e["type"] == "pick")
        pick.update(candidate_text="你郝", candidate_reading="ㄋㄧˇ ㄏㄠˇ", candidate_start=0, candidate_end=2)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "mixed.jsonl"
            path.write_text(
                "\n".join(json.dumps(r, ensure_ascii=False) for r in [self.records[0], record]), encoding="utf-8"
            )
            old, new = corrections.load(path)
        self.assertNotIn("candidate_text", next(e for e in old["events"] if e["type"] == "pick"))
        new_pick = next(e for e in new["events"] if e["type"] == "pick")
        self.assertEqual(
            (new_pick["candidate_text"], new_pick["candidate_start"], new_pick["candidate_end"]), ("你郝", 0, 2)
        )
        self.assertEqual(corrections.replay_items(old), corrections.replay_items(new))
        self.assertEqual(corrections.summarize([old]), corrections.summarize([new]))

    def test_replay_items(self) -> None:
        pick, backspace, escape, cursor, retype = (corrections.replay_items(r) for r in self.records)
        self.assertEqual(
            [(i.group, corrections.plain(i.keys), i.truth, i.shown) for i in pick], [("pick", "su3cl3", "你郝", "你好")]
        )
        self.assertEqual(corrections.plain(backspace[0].keys), "su31j6")
        self.assertEqual(backspace[0].line, "我們\t" + backspace[0].keys + "<enter>")
        self.assertEqual((escape[0].truth, corrections.plain(escape[0].keys)), ("逆", "su3"))
        self.assertEqual(cursor, [])
        self.assertEqual((retype[0].group, retype[0].truth, retype[0].shown), ("delete_after_commit", "逆", "你"))
        self.assertIn("--forms", pick[0].options)

    def test_cli_options(self) -> None:
        options = corrections.cli_options({"typo_tolerance": "off", "fuzzy_sounds": ["zh", "an", "xx"]})
        self.assertEqual(options[:2], ["--typo", "off"])
        self.assertEqual(options[-2:], ["--fuzzy", "zh,an"])

    def test_punctuation_width_and_historical_recordings(self) -> None:
        self.assertNotIn("--punctuation-width", corrections.cli_options({}))
        for width in ("legacy", "full", "half"):
            options = corrections.cli_options({"punctuation_width": width})
            self.assertEqual(options[options.index("--punctuation-width") + 1], width)
        with self.assertRaises(ValueError):
            corrections.cli_options({"punctuation_width": "unknown"})
        record = {
            **self.records[-1],
            "settings": {"punctuation_width": "half"},
            "events": [{**event, "previous_punctuation_width": "full"} for event in self.records[-1]["events"]],
        }
        options = corrections.replay_items(record)[0].options
        self.assertEqual(options[options.index("--punctuation-width") + 1], "full")

    def test_syllables(self) -> None:
        self.assertEqual(corrections.syllables("s<dt:90>u3cl3"), ["ㄋㄧˇ", "ㄏㄠˇ"])
        self.assertEqual(corrections.syllables("1j<space>a87"), ["ㄅㄨ", "ㄇㄚ˙"])
        self.assertEqual(corrections.syllables("j16"), ["ㄅㄨˊ"])
        for keys in ("su3<bs>", "su", "3", "su3,", "1q6"):
            self.assertIsNone(corrections.syllables(keys), keys)

    def test_rows(self) -> None:
        self.assertEqual(corrections.rows(self.records), ["|你郝|ㄋㄧˇ ㄏㄠˇ", "我們|你步|ㄋㄧˇ ㄅㄨˊ", "|逆|ㄋㄧˇ"])

    def test_summary(self) -> None:
        summary = corrections.summarize(self.records)
        self.assertEqual(summary["corrections"]["backspace"], 1)
        self.assertEqual({(e["typed"], e["retyped"]) for e in summary["keys"]}, {("1j6", "1j4"), ("su3", "su4")})
        self.assertEqual(summary["words"][0], {"shown": "你", "final": "逆", "count": 2})
        self.assertIn({"shown": "好", "final": "郝", "count": 1}, summary["words"])


if __name__ == "__main__":
    unittest.main()

"""The engine's reading channel against its Python reference (engine_channel.py) on a small table.

python -m unittest zhuyintype_ml.training.lexicon.test_engine_channel

The Rust half builds zhuyin-cli from the current sources and is skipped where cargo is missing; the full table is
checked with `python -m zhuyintype_ml.training.lexicon.engine_channel --channel DIR`.
"""

from __future__ import annotations

import json
import math
import shutil
import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.training.lexicon import engine_channel
from zhuyintype_ml.training.lexicon.channel import CHANNEL_HEADER, TAIWAN_HEADER, load
from zhuyintype_ml.training.lm.compile_data import compile_channel

CHANNEL = [
    ("什", "ㄕㄣˊ", "lexicon", 1.0, "cedict-unihan"),
    ("喙", "ㄏㄨㄟˋ", "lexicon", 0.9, "cedict-unihan"),
    ("喙", "ㄓㄡˋ", "series", 0.1, "ids"),
    ("悉", "ㄒㄧ", "lexicon", 1.0, "cedict-unihan"),
    ("期", "ㄑㄧ", "lexicon", 0.7, "cedict-unihan"),
    ("期", "ㄑㄧˊ", "taiwan", 0.2, "wiktionary,cns"),
    ("期", "ㄐㄧ", "word", 0.1, "cedict-unihan"),
    ("熟", "ㄕㄨˊ", "lexicon", 0.9, "cedict-unihan"),
    ("熟", "ㄕㄡˊ", "taiwan", 0.1, "wiktionary,cns"),
    ("蚵", "ㄜˊ", "lexicon", 0.6, "cedict-unihan"),
    ("蚵", "ㄎㄜ", "word", 0.3, "wiktionary"),
    ("蚵", "ㄏㄜˊ", "unihan", 0.1, "cedict-unihan"),
    ("麼", "ㄇㄛˊ", "lexicon", 0.7, "cedict-unihan"),
    ("麼", "ㄇㄚ", "attested", 0.2, "wiktionary"),
    ("麼", "ㄇㄜ", "cns", 0.1, "cns"),
    ("鮮", "ㄒㄧㄢ", "lexicon", 1.0, "cedict-unihan"),
]
TAIWAN = [("期", "ㄑㄧ", "ㄑㄧˊ", 0.95, 1), ("熟", "ㄕㄨˊ", "ㄕㄡˊ", 0.4, 0)]
LEXICON = [
    ("ㄕㄨˊ", "熟", -8.0),
    ("ㄕㄨˊ ㄒㄧ", "熟悉", -9.0),
    ("ㄜˊ", "蚵", -12.0),
    ("ㄒㄧㄢ ㄜˊ", "鮮蚵", -14.0),
    ("ㄒㄧㄢ ㄎㄜ", "鮮蚵", -16.0),
    ("ㄑㄧ ㄓㄨㄥ", "期中", -10.0),
    ("ㄑㄧ", "期", -7.0),
    ("ㄑㄧˊ", "期", -7.5),
    ("ㄕㄣˊ ㄇㄜ˙", "什麼", -6.0),
    ("ㄏㄨㄟˋ", "喙", -13.0),
    ("ㄋㄧㄠˇ ㄏㄨㄟˋ", "鳥喙", -14.0),
    ("ㄊ ㄒㄩˋ", "T恤", -11.0),
]


def write_fixture(directory: Path) -> None:
    with open(directory / "channel.tsv", "w", encoding="utf-8") as handle:
        handle.write(CHANNEL_HEADER)
        for char, reading, kind, p, groups in CHANNEL:
            handle.write(f"{char}\t{reading}\t{kind}\t{math.log(p):.4f}\t{groups}\n")
    with open(directory / "taiwan.tsv", "w", encoding="utf-8") as handle:
        handle.write(TAIWAN_HEADER)
        for char, a, b, strength, standard in TAIWAN:
            handle.write(f"{char}\t{a}\t{b}\t{math.log(strength):.4f}\t{standard}\n")
    (directory / "familiar.txt").write_text("什悉期熟麼鮮\n", encoding="utf-8")
    (directory / "channel.json").write_text(json.dumps({"misread": 0.005, "neutral_full_tone": 0.01}), encoding="utf-8")
    with open(directory / "lexicon.tsv", "w", encoding="utf-8") as handle:
        handle.write("# test lexicon\n")
        for reading, word, logp in LEXICON:
            handle.write(f"{reading}\t{word}\t{logp}\n")


class Reference(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp())
        write_fixture(self.tmp)
        self.channel = load(self.tmp)
        self.lexicon = engine_channel.read_lexicon(self.tmp / "lexicon.tsv")

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp)

    def rows(
        self, settings: engine_channel.Settings | None = None
    ) -> dict[tuple[str, int, str], tuple[str, str, float]]:
        table = engine_channel.table(self.channel, self.lexicon, settings or engine_channel.Settings.every_class())
        return {(word, position, typed): (kind, state, cost) for _, word, position, typed, kind, state, cost in table}

    def test_classes_and_costs(self):
        rows = self.rows()
        p = {(c, r): math.exp(round(math.log(q), 4)) for c, r, _, q, _ in CHANNEL}
        # A single character: P(typed) / P(the lexicon's reading).
        self.assertAlmostEqual(rows[("熟", 0, "ㄕㄡˊ")][2], -math.log(p[("熟", "ㄕㄡˊ")] / p[("熟", "ㄕㄨˊ")]))
        # taiwan.tsv in a longer word: the standard costs 0, another Taiwan reading −log strength.
        self.assertEqual(rows[("期中", 0, "ㄑㄧˊ")], ("taiwan-word", "accepted", 0.0))
        self.assertAlmostEqual(rows[("熟悉", 0, "ㄕㄡˊ")][2], -round(math.log(0.4), 4))
        # A neutral tone typed with another reading of its character.
        kind, state, cost = rows[("什麼", 1, "ㄇㄛˊ")]
        others = p[("麼", "ㄇㄛˊ")] + p[("麼", "ㄇㄚ")]
        self.assertEqual((kind, state), ("neutral", "accepted"))
        self.assertAlmostEqual(cost, -math.log(0.01) - math.log(p[("麼", "ㄇㄛˊ")] / others))
        # An unfamiliar character in a longer word; a correction for the phonetic series.
        self.assertEqual(rows[("鳥喙", 1, "ㄓㄡˋ")][:2], ("series", "correction"))
        self.assertAlmostEqual(
            rows[("鳥喙", 1, "ㄓㄡˋ")][2],
            -math.log(0.005) - math.log(p[("喙", "ㄓㄡˋ")] / (1 - p[("喙", "ㄏㄨㄟˋ")])),
        )
        # Familiar characters have no misreadings in longer words.
        self.assertNotIn(("期中", 0, "ㄐㄧ"), rows)

    def test_rows_win_and_alignment(self):
        rows = self.rows()
        self.assertNotIn(("鮮蚵", 1, "ㄎㄜ"), rows)  # the lexicon has 鮮蚵 ㄒㄧㄢ ㄎㄜ itself
        self.assertIn(("鮮蚵", 1, "ㄏㄜˊ"), rows)
        self.assertNotIn(("期", 0, "ㄑㄧˊ"), rows)  # 期 ㄑㄧˊ is a row
        self.assertFalse(any(word == "T恤" for word, _, _ in rows))

    def test_settings(self):
        settings = engine_channel.Settings.every_class()
        settings.offsets, settings.max_cost = {"taiwan": 1.5}, 6.0
        settings.classes.discard("series")
        rows = self.rows(settings)
        self.assertNotIn(("鳥喙", 1, "ㄓㄡˋ"), rows)
        base = self.rows()[("熟", 0, "ㄕㄡˊ")][2]
        self.assertAlmostEqual(rows[("熟", 0, "ㄕㄡˊ")][2], base + 1.5)
        self.assertTrue(all(cost <= 6.0 for _, _, cost in rows.values()))


class AgainstRust(unittest.TestCase):
    """Python's table and zhuyin-cli --channel-table agree row for row, costs bit for bit."""

    def test_same_table(self):
        if shutil.which("cargo") is None and not (Path.home() / ".cargo" / "bin" / "cargo").exists():
            self.skipTest("no cargo")
        from zhuyintype_ml.core import paths

        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            write_fixture(directory)
            compile_channel(directory, directory / "compiled")
            channel = load(directory)
            lexicon = engine_channel.read_lexicon(directory / "lexicon.tsv")
            for settings in (
                engine_channel.Settings(),
                engine_channel.Settings.every_class(),
                engine_channel.Settings(offsets={"word": 2.0, "series": -1.0}, max_cost=7.0),
                engine_channel.Settings(single_classes={"taiwan"}),
            ):
                python = engine_channel.table(channel, lexicon, settings)
                rust = engine_channel.rust_table(
                    paths.cli(), directory / "lexicon.tsv", directory / "compiled" / "channel.bin", settings
                )
                self.assertEqual(engine_channel.compare(python, rust), [])
                self.assertGreater(len(python), 2)


if __name__ == "__main__":
    unittest.main()

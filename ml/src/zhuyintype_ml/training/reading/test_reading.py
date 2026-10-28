"""Unit tests for the reading noise and the gap rows (no data directory needed).

python -m unittest zhuyintype_ml.training.reading.test_reading
"""

from __future__ import annotations

import json
import random
import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.core.zhuyin import Lexicon as TypingLexicon
from zhuyintype_ml.training.reading.gap import pairs
from zhuyintype_ml.training.reading.noise import HEADER, ReadingNoise
from zhuyintype_ml.training.v3.noise import Noise
from zhuyintype_ml.training.v3.readings import Lexicon

LEXICON = """# reading\tword\tlog prior
ㄒㄧㄢ\t鮮\t-6
ㄜˊ\t蚵\t-12
ㄎㄜ\t科\t-7
ㄑㄧ ㄓㄨㄥ\t期中\t-9
ㄑㄧ\t期\t-8
ㄑㄧˊ\t其\t-6
ㄓㄨㄥ\t中\t-5
ㄐㄧ\t機\t-7
ㄕㄣˊ ㄇㄜ˙\t什麼\t-6
ㄇㄛˊ\t麼\t-12
ㄋㄧˇ\t你\t-5
ㄏㄠˇ\t好\t-5
ㄋㄧˊ\t泥\t-9
ㄕㄣˊ\t神\t-7
ㄇㄜ\t么\t-12
ㄏㄠˊ\t毫\t-9
"""
NOISE = [
    "蚵\t*\tㄎㄜ\tword\t0.4\taccepted\t0",
    "期\tㄑㄧ\tㄑㄧˊ\ttaiwan\t0.99\taccepted\t0",
    "期\tㄑㄧ\tㄐㄧ\tmisread\t0.003\tcorrection\t1",
]
CHANNEL = """# ZTRC/1 char\treading\tclass\tlog P(reading | char)\tsource groups
麼\tㄇㄜ˙\tlexicon\t-0.1\t
麼\tㄇㄛˊ\tlexicon\t-2.4\t
"""


class Noise_(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        root = Path(self.directory.name)
        (root / "lexicon.tsv").write_text(LEXICON, encoding="utf-8")
        (root / "v3_noise.tsv").write_text(HEADER + "\n" + "\n".join(NOISE) + "\n", encoding="utf-8")
        (root / "channel.tsv").write_text(CHANNEL, encoding="utf-8")
        (root / "channel.json").write_text(json.dumps({"misread": 0.005, "neutral_full_tone": 0.01}))
        self.lexicon = Lexicon(root / "lexicon.tsv")
        self.keys = Noise(self.lexicon.candidates)
        self.noise = ReadingNoise(root, scale=1, valid=lambda r: r in self.keys.encode)

    def tearDown(self) -> None:
        self.directory.cleanup()

    def options(self, text: str) -> list[list[tuple[tuple[str, int], int, str]]]:
        readings, _ = self.lexicon.label(text)
        single = self.noise.singles(text, self.lexicon)
        return [
            sorted((o.typed, o.bucket, o.state) for o in self.noise.options(text, readings, i, single[i]))
            for i in range(len(text))
        ]

    def test_character_rows_only_for_single_character_words(self) -> None:
        self.assertEqual(self.options("鮮蚵")[1], [(("ㄎㄜ", 1), 0, "accepted")])

    def test_rows_for_the_reading_in_the_text_apply_inside_words(self) -> None:
        options = self.options("期中")[0]
        self.assertEqual(options, [(("ㄐㄧ", 1), 1, "correction"), (("ㄑㄧ", 2), 0, "accepted")])

    def test_rule_level_variants_are_accepted(self) -> None:
        self.assertIn((("ㄇㄛ", 2), 0, "accepted"), self.options("什麼")[1])
        self.assertEqual(self.options("你好")[0], [(("ㄋㄧ", 2), 0, "accepted")])

    def test_state_and_bucket_reach_v3(self) -> None:
        readings, _ = self.lexicon.label("期中")
        self.noise.scale = 1000
        typed = self.keys.variants("期中", readings, self.lexicon, random.Random(3), 0, False, self.noise)
        self.assertNotEqual(typed[0][0], ("ㄑㄧ", 1))
        self.assertEqual(typed[0][1], 1 if typed[0][0] == ("ㄐㄧ", 1) else 0)

    def test_cap_keeps_the_text_reading(self) -> None:
        readings, _ = self.lexicon.label("期中")
        self.noise.scale = 1000
        rng = random.Random(5)
        changed = sum(0 in self.noise("期中", readings, self.lexicon, rng) for _ in range(2000))
        self.assertAlmostEqual(changed / 2000, 0.5, delta=0.05)

    def test_bad_state_is_rejected(self) -> None:
        root = Path(self.directory.name)
        (root / "v3_noise.tsv").write_text(HEADER + "\n蚵\t*\tㄎㄜ\tword\t0.4\taccepted\t2\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            ReadingNoise(root)


class Gap(unittest.TestCase):
    def test_pairs_keep_rows_that_differ(self) -> None:
        lexicon = TypingLexicon(rows=[("ㄑㄧˊ ㄓㄨㄥ", "期中", -9.0), ("ㄎㄠˇ", "考", -6.0)])
        found = pairs(["|期中考|ㄑㄧ ㄓㄨㄥ ㄎㄠˇ", "|期中考|ㄑㄧˊ ㄓㄨㄥ ㄎㄠˇ", "|未知|ㄨㄟˋ ㄓ"], lexicon)
        self.assertEqual(found, [("|期中考|ㄑㄧ ㄓㄨㄥ ㄎㄠˇ", "|期中考|ㄑㄧˊ ㄓㄨㄥ ㄎㄠˇ")])


if __name__ == "__main__":
    unittest.main()

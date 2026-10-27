"""Unit tests of the lexicon coverage pipeline: readings, alignment, the channel, merging and pack handling.

python -m unittest zhuyintype_ml.training.lexicon.test_lexicon
"""

from __future__ import annotations

import importlib.util
import math
import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.training.lexicon import channel, merge, phonology, wiktionary

SYLLABLES = {"ㄚ", "ㄜ", "ㄐㄧㄢ", "ㄗ", "ㄎㄜ", "ㄒㄧㄢ", "ㄏㄜ", "ㄍㄜ", "ㄕㄣ", "ㄇㄜ", "ㄇㄛ", "ㄉㄨㄥ", "ㄒㄧ"}


class Phonology(unittest.TestCase):
    def test_bopomofo(self):
        self.assertEqual(phonology.bopomofo("ㄕㄣˊ ˙ㄇㄜ"), "ㄕㄣˊ ㄇㄜ˙")
        self.assertEqual(phonology.bopomofo("ㄎㄜ"), "ㄎㄜ")
        self.assertIsNone(phonology.bopomofo("ㄕㄣˊ me"))

    def test_tailo_syllables(self):
        self.assertEqual(phonology.tailo_syllables("ô-á-tsian"), [("ô", False), ("á", False), ("tsian", False)])
        self.assertEqual(phonology.tailo_syllables("o--á"), [("o", False), ("á", True)])

    def test_tailo_to_zhuyin(self):
        convert = lambda s, n=False: phonology.tailo_to_zhuyin(s, n, SYLLABLES)
        self.assertEqual(convert("á"), "ㄚˇ")  # 上 -> third tone
        self.assertEqual(convert("ô"), "ㄜˊ")  # 陽平 -> second tone
        self.assertEqual(convert("tsian"), "ㄐㄧㄢ")  # ts before i is palatal
        self.assertEqual(convert("á", True), "ㄚ˙")
        self.assertIsNone(convert("tsut"))  # checked syllable
        self.assertIsNone(convert("gû"))  # no Mandarin g
        self.assertIsNone(convert("kiánn"))  # no velar before i


class Wiktionary(unittest.TestCase):
    def test_facts(self):
        entry = {
            "word": "蚵",
            "pos": "character",
            "etymology_text": "Phono-semantic compound: semantic 虫 + phonetic 可 (OC)",
            "sounds": [
                {"zh_pron": "ㄎㄜ", "tags": ["Mandarin", "Standard-Chinese", "Taiwan", "Bopomofo"]},
                {"zh_pron": "ô", "tags": ["Min-Nan", "Hokkien", "Taipei", "Tai-lo"]},
            ],
        }
        rows = wiktionary.facts(entry)
        self.assertIn(("蚵", "mandarin", "ㄎㄜ", "Taiwan"), rows)
        self.assertIn(("蚵", "hokkien", "ô", "Taiwan"), rows)
        self.assertIn(("蚵", "phonetic", "可", ""), rows)
        self.assertEqual(wiktionary.facts({"word": "ABC", "pos": "noun"}), [])

    def test_mandarin_readings_join_labels(self):
        facts = [
            ("蚵仔煎", "mandarin", "ㄎㄜˋ ㄗˇ ㄐㄧㄢ", []),
            ("蚵仔煎", "mandarin", "ㄎㄜˋ ㄗˇ ㄐㄧㄢ", ["Mainland-China"]),
            ("蚵仔煎", "mandarin", "ㄜˊ ㄗˇ ㄐㄧㄢ", ["Taiwan"]),
            ("什麼", "mandarin", "ㄕㄣˊ ㄇㄜ˙", ["Taiwan", "standard"]),
            ("什麼", "mandarin", "ㄕㄜˊ ㄇㄜ˙", ["common", "variant"]),
        ]
        readings = channel.mandarin_readings(facts)
        self.assertEqual(set(readings["蚵仔煎"]), {"ㄜˊ ㄗˇ ㄐㄧㄢ"})  # the Mainland-only reading is dropped
        self.assertEqual(channel.variants(readings["什麼"]), [("ㄕㄣˊ ㄇㄜ˙", "ㄕㄜˊ ㄇㄜ˙")])


def toy_channel() -> channel.Channel:
    c = channel.Channel(misread=0.01, neutral=0.02)
    c.share = {
        "蚵": {"ㄜˊ": 0.9, "ㄎㄜ": 0.1},
        "仔": {"ㄗˇ": 0.95, "ㄚˇ": 0.05},
        "煎": {"ㄐㄧㄢ": 1.0},
        "麼": {"ㄇㄜ˙": 0.98, "ㄇㄛˊ": 0.02},
        "什": {"ㄕㄣˊ": 1.0},
        "鮮": {"ㄒㄧㄢ": 1.0},
    }
    c.mechanism = {(ch, r): "lexicon" for ch, rs in c.share.items() for r in rs}
    return c


class Merge(unittest.TestCase):
    def setUp(self):
        self.base = [
            ("ㄜˊ", "蚵", -15.0),
            ("ㄗˇ", "仔", -11.0),
            ("ㄜˊ ㄗˇ ㄐㄧㄢ", "蚵仔煎", -19.0),
            ("ㄕㄣˊ ㄇㄜ˙", "什麼", -6.0),
        ]

    def test_alternatives_cost(self):
        c = toy_channel()
        c.familiar = {"仔"}
        self.assertEqual(c.alternatives("仔", "ㄗˇ"), [])  # a familiar character keeps its word reading
        ((reading, cost),) = c.alternatives("蚵", "ㄜˊ")
        self.assertEqual(reading, "ㄎㄜ")
        self.assertAlmostEqual(cost, math.log(0.01) + math.log(0.1 / 0.1))

    def test_base_rows_unchanged_and_variants_below(self):
        builder = merge.Builder(self.base, toy_channel(), max_cost=8.0)
        builder.characters()
        builder.words([])
        rows = builder.rows
        for reading, word, logp in self.base:
            self.assertEqual(rows[(reading, word)], logp)
        self.assertAlmostEqual(rows[("ㄎㄜ", "蚵")], -15.0 + math.log(0.1 / 0.9))
        self.assertAlmostEqual(rows[("ㄎㄜ ㄗˇ ㄐㄧㄢ", "蚵仔煎")], -19.0 + math.log(0.01))
        self.assertIn(("ㄕㄣˊ ㄇㄛˊ", "什麼"), rows)  # neutral syllable typed with another reading
        for (_, word), logp in rows.items():
            self.assertLessEqual(logp, max(p for r, w, p in self.base if w == word))

    def test_max_cost_bounds_generated_variants(self):
        builder = merge.Builder(self.base, toy_channel(), max_cost=3.0)
        builder.words([])
        self.assertNotIn(("ㄎㄜ ㄗˇ ㄐㄧㄢ", "蚵仔煎"), builder.rows)

    def test_attested_loan_variant(self):
        builder = merge.Builder(self.base, toy_channel(), max_cost=3.0)
        builder.syllables |= {"ㄚ"}
        facts = [("蚵仔煎", "hokkien", "ô-á-tsian", ["Taiwan"]), ("蚵仔煎", "class", "taiwanese-mandarin", [])]
        builder.words(facts)
        # The Mandarin tone of a Hokkien syllable is unknown: every tone at log m - log 5.
        for tone in ("", "ˊ", "ˇ", "ˋ", "˙"):
            self.assertAlmostEqual(
                builder.rows[(f"ㄜˊ ㄚ{tone} ㄐㄧㄢ", "蚵仔煎")], -19.0 + math.log(0.01) - math.log(5)
            )

    def test_taiwan_reading_propagates_into_words(self):
        c = toy_channel()
        c.share["期"] = {"ㄑㄧ": 0.5, "ㄑㄧˊ": 0.5}
        c.share["中"] = {"ㄓㄨㄥ": 1.0}
        c.taiwan[("期", "ㄑㄧ")] = ("ㄑㄧˊ", 0.9)
        builder = merge.Builder([("ㄑㄧ ㄓㄨㄥ", "期中", -12.0)], c, max_cost=8.0)
        builder.words([])
        self.assertAlmostEqual(builder.rows[("ㄑㄧˊ ㄓㄨㄥ", "期中")], -12.0 + math.log(0.9) - merge.TIE)
        builder = merge.Builder([("ㄑㄧ ㄓㄨㄥ", "期中", -12.0)], c, offsets={"taiwan": 2.0})
        builder.words([])
        self.assertAlmostEqual(builder.rows[("ㄑㄧˊ ㄓㄨㄥ", "期中")], -14.0 + math.log(0.9) - merge.TIE)

    def test_third_tone_sandhi_and_mcbopomofo(self):
        base = [("ㄋㄧˇ ㄏㄠˇ", "你好", -9.0), ("ㄨㄟ ㄒㄧㄢˇ", "危險", -10.0)]
        mcb = {"危險": {"ㄨㄟˊ ㄒㄧㄢˇ": 1.0}}
        builder = merge.Builder(base, toy_channel(), mcbopomofo=mcb)
        builder.words([])
        self.assertAlmostEqual(builder.rows[("ㄋㄧˊ ㄏㄠˇ", "你好")], -9.0 + math.log(0.01))
        self.assertAlmostEqual(builder.rows[("ㄨㄟˊ ㄒㄧㄢˇ", "危險")], -10.0 - merge.TIE)
        self.assertAlmostEqual(builder.rows[("ㄨㄟˇ ㄒㄧㄢˇ", "危險")], -10.0 - merge.TIE + math.log(0.01))

    def test_pack_adds_new_words_only(self):
        builder = merge.Builder(self.base, toy_channel(), max_cost=8.0)
        with tempfile.TemporaryDirectory() as tmp:
            pack = Path(tmp) / "food.tsv"
            pack.write_text("# pack\nㄒㄧㄢ ㄜˊ\t鮮蚵\t-17.0\nㄜˊ ㄗˇ ㄐㄧㄢ\t蚵仔煎\t-10.0\n", encoding="utf-8")
            self.assertEqual(builder.pack(pack), 1)
        self.assertEqual(builder.rows[("ㄒㄧㄢ ㄜˊ", "鮮蚵")], -17.0)
        self.assertEqual(builder.rows[("ㄜˊ ㄗˇ ㄐㄧㄢ", "蚵仔煎")], -19.0)  # an existing word keeps its base row
        self.assertIn(("ㄒㄧㄢ ㄎㄜ", "鮮蚵"), builder.rows)  # the pack word gets the channel's variants


class Format(unittest.TestCase):
    """channel.tsv and taiwan.tsv as resources/formats/reading-channel.md defines them (the engine reads them too)."""

    def test_written_tables_validate_and_load_back(self):
        c = toy_channel()
        c.taiwan[("期", "ㄑㄧ")] = ("ㄑㄧˊ", 0.99)
        c.standard.add(("期", "ㄑㄧ"))
        c.evidence[("蚵", "ㄎㄜ")] = "wiktionary,cns"
        with tempfile.TemporaryDirectory() as tmp:
            c.write(Path(tmp))
            self.assertEqual(channel.validate(Path(tmp)), [])
            again = channel.load(Path(tmp))
        self.assertAlmostEqual(again.share["蚵"]["ㄎㄜ"], 0.1, places=4)
        self.assertEqual(again.evidence[("蚵", "ㄎㄜ")], "wiktionary,cns")
        self.assertIn(("期", "ㄑㄧ"), again.standard)

    def test_noise_states_and_cost_buckets(self):
        c = toy_channel()
        c.mechanism[("蚵", "ㄎㄜ")] = "series"
        c.mechanism[("麼", "ㄇㄛˊ")] = "attested"
        c.taiwan[("期", "ㄑㄧ")] = ("ㄑㄧˊ", 0.99)
        with tempfile.TemporaryDirectory() as tmp:
            c.write(Path(tmp))
            self.assertEqual(channel.validate(Path(tmp)), [])
            rows = [line.split("\t") for line in Path(tmp, "v3_noise.tsv").read_text(encoding="utf-8").splitlines()]
        by = {(r[0], r[1], r[2]): (r[5], r[6]) for r in rows[1:]}
        self.assertEqual(by[("麼", "*", "ㄇㄛˊ")], ("accepted", "0"))
        self.assertEqual(by[("期", "ㄑㄧ", "ㄑㄧˊ")], ("accepted", "0"))
        self.assertEqual(by[("蚵", "*", "ㄎㄜ")], ("correction", "1"))  # −log 0.1 ≤ 6
        self.assertEqual(by[("蚵", "ㄜˊ", "ㄎㄜ")], ("correction", "1"))  # −log 0.01 ≤ 6
        self.assertEqual(channel.cost_bucket("series", math.exp(-7)), 2)
        self.assertEqual(channel.cost_bucket("loan", math.exp(-12)), 3)

    def test_validate_reports_problems(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "channel.tsv").write_text(
                channel.CHANNEL_HEADER + "蚵\tㄎㄜ\tseries\t0.5\tids\n蚵\tx\tnope\t-1\tother\n", encoding="utf-8"
            )
            Path(tmp, "taiwan.tsv").write_text(channel.TAIWAN_HEADER + "期\tㄑㄧ\tㄑㄧˊ\t-0.01\t2\n", encoding="utf-8")
            problems = channel.validate(Path(tmp))
        self.assertTrue(any("at most 0" in p for p in problems))
        self.assertTrue(any("bad character" in p for p in problems))
        self.assertTrue(any("standard flag" in p for p in problems))

    def test_built_table_validates(self):
        from zhuyintype_ml.core import paths

        directory = paths.data("lexicon_coverage", "channel")
        if not (directory / "channel.tsv").exists():
            self.skipTest("no built table (zt train lexicon channel)")
        self.assertEqual(channel.validate(directory), [])


class Ids(unittest.TestCase):
    def test_first_level(self):
        self.assertEqual(channel.first_level("虫可"), ["虫", "可"])
        self.assertEqual(channel.first_level("⿱艹口可"), ["⿱艹口", "可"])
        self.assertEqual(channel.first_level("彳⿰山攵"), ["彳", "⿰山攵"])


@unittest.skipUnless(importlib.util.find_spec("numpy"), "numpy is in the train dependency group")
class Mining(unittest.TestCase):
    def test_cohesion(self):
        from zhuyintype_ml.training.lexicon.mine import cohesion

        counts = {"鮮蚵": 10, "鮮": 20, "蚵": 10}
        self.assertAlmostEqual(cohesion("鮮蚵", counts, 1000), math.log(10 * 1000 / (20 * 10)))

    def test_eight_gram_exclusion(self):
        from zhuyintype_ml.training.lexicon.mine import contained, eight_grams

        grams = eight_grams(["今天晚上去夜市吃蚵仔煎"])
        self.assertTrue(contained("我說今天晚上去夜市吃東西", grams))
        self.assertFalse(contained("今天晚上去吃蚵仔煎", grams))


if __name__ == "__main__":
    unittest.main()

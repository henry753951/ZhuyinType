"""Tests of the simulated typing: keyboard slips (nlp/eval/typo.py), typing habits (nlp/eval/errors.py) and timed
typing errors (nlp/eval/typing_errors.py); the checks of the former tools' self-checks."""

from __future__ import annotations

import random
import re
import unittest

from nlp.common.engine import strip_options
from nlp.common.sets import BY_NAME, EvalSet
from nlp.eval import errors, typing_errors, typo
from nlp.eval.typing_errors import CONDITIONS, FINGERS, POSITIONS, Stroke, canonical, condition, mutation


class Slips(unittest.TestCase):
    def test_neighbours(self):
        n = typo.NEIGHBOURS
        self.assertTrue("4" in n["3"] and "7" in n["6"] and "6" not in n["4"])
        self.assertTrue("w" in n["e"] and {"2", "3", "a", "s"} <= n["w"])
        self.assertTrue(all(a in n[b] for a, neighbours in n.items() for b in neighbours))

    def test_mutations_change_keys(self):
        for keys in ("su3", "fu4", "186", "187", "wu0 "):
            self.assertTrue(all(variant != keys for group in typo.mutations(keys) for variant in group))

    def test_scripts_and_metrics(self):
        self.assertEqual(typo.noisy_script(["ㄋㄧˇ", "ㄏㄠˇ"], 0, random.Random(7)), "su3cl3<enter>")
        noisy = typo.noisy_script(["ㄋㄧˇ", "ㄏㄠˇ", "ㄊㄧㄢ"], 1, random.Random(7))
        self.assertNotEqual(noisy, "su3cl3wu0 <enter>")
        self.assertEqual(typo.metrics(["甲乙丙"], ["甲丙"])["character_accuracy"], 1 - 1 / 3)
        self.assertEqual(typo.metrics(["甲乙"], ["甲乙"])["whole_run_accuracy"], 1)


class Habits(unittest.TestCase):
    def test_habits(self):
        self.assertEqual(errors.sandhi("一直", ["ㄧ", "ㄓˊ"]), ["ㄧˋ", "ㄓˊ"])
        self.assertEqual(errors.sandhi("一定", ["ㄧ", "ㄉㄧㄥˋ"]), ["ㄧˊ", "ㄉㄧㄥˋ"])
        self.assertEqual(errors.sandhi("不要", ["ㄅㄨˋ", "ㄧㄠˋ"]), ["ㄅㄨˊ", "ㄧㄠˋ"])
        self.assertIsNone(errors.sandhi("你好", ["ㄋㄧˇ", "ㄏㄠˇ"]))
        self.assertEqual(errors.neutral(["ㄇㄚ˙"]), ["ㄇㄚ"])
        self.assertEqual(errors.nospace(["ㄊㄚ", "ㄊㄚ"]), [False, True])
        self.assertIsNone(errors.nospace(["ㄋㄧˇ", "ㄊㄚ"]))
        self.assertEqual(errors.habit_keys(["ㄊㄚ", "ㄊㄚ"], [False, True]), "w8w8 ")

    def test_scripts(self):
        habits = errors.habit_scripts([("一直", ["ㄧ", "ㄓˊ"]), ("你好", ["ㄋㄧˇ", "ㄏㄠˇ"])], 7)
        self.assertEqual([len(habits[h]) for h in errors.HABITS], [2, 1, 0, 1, 2])
        self.assertEqual(habits["clean"][0], ("一直", "u 56"))
        self.assertEqual(habits, errors.habit_scripts([("一直", ["ㄧ", "ㄓˊ"]), ("你好", ["ㄋㄧˇ", "ㄏㄠˇ"])], 7))


class TimedErrors(unittest.TestCase):
    readings = ["ㄋㄧˇ", "ㄏㄠˇ", "ㄊㄧㄢ", "ㄑㄧˋ"]

    def test_layout(self):
        self.assertEqual("".join(s.key for s in canonical(self.readings)), "su3cl3wu0 fu4")
        self.assertEqual(POSITIONS["q"], (0.25, 1))
        self.assertEqual(FINGERS["u"], ("R", 0))

    def test_conditions(self):
        original = canonical(self.readings)
        for name in CONDITIONS:
            for seed in range(30):
                result = condition(self.readings, name, random.Random(seed))
                if result is None:
                    self.assertEqual(name, "nospace-30")
                    continue
                strokes, fast, edits = result
                self.assertTrue(all(s.key in POSITIONS for s in strokes))
                if name not in ("exact", "exact-fast", "mixed"):
                    self.assertNotEqual([s.key for s in strokes], [s.key for s in original])
                if name.endswith("-2"):
                    self.assertEqual(edits, 2)
                if name.startswith("rollover"):
                    self.assertEqual(sum(s.gap is not None and 0 <= s.gap <= 15 for s in strokes), edits)
                if name == "extra-key":
                    self.assertEqual(len(strokes), len(original) + 1)
                    self.assertEqual(sum(s.gap is not None and 0 <= s.gap <= 20 for s in strokes), 1)
                if name == "missing-key":
                    self.assertEqual(len(strokes), len(original) - 1)
                script = typing_errors.timed_script(strokes, random.Random(seed), fast)
                self.assertEqual(script.count("<dt:"), len(strokes))
                self.assertNotIn("<enter>", script)

    def test_mutation_sites(self):
        original = canonical(self.readings)
        for kind in ("swap-in-syllable", "swap-across", "rollover", "hotspot"):
            result = mutation(original, kind, random.Random(7), 2)
            assert result is not None
            changed, sites = result
            self.assertEqual(len({i for site in sites for i in site}), sum(map(len, sites)))
            self.assertEqual(sum(a.key != b.key for a, b in zip(original, changed)), 2 if kind == "hotspot" else 4)
            for site in sites:
                if kind.startswith("swap-"):
                    across = original[site[0]].syllable != original[site[1]].syllable
                    self.assertEqual(across, kind == "swap-across")
        self.assertIsNone(condition(["ㄚ", "ㄚ"], "swap-across-2", random.Random(7)))
        self.assertIsNone(condition(["ㄚ", "ㄚ"], "tone", random.Random(7)))
        self.assertEqual(typing_errors.omit_spaces(canonical(["ㄚ", "ㄚ"]), random.Random(7), 1, 1)[-1].key, " ")
        self.assertEqual(typing_errors.omit_spaces([Stroke(" ", 0)], random.Random(7), 1, 1), [Stroke(" ", 0)])

    def test_timing_and_seeds(self):
        original = canonical(self.readings)
        normal = typing_errors.timed_script(original * 100, random.Random(7))
        fast = typing_errors.timed_script(original * 100, random.Random(7), True)

        def gaps(script: str) -> list[float]:
            return [float(x) for x in re.findall(r"<dt:([\d.]+)>", script)]

        self.assertTrue(all(abs(a - 2 * b) <= 0.002 for a, b in zip(gaps(normal), gaps(fast))))
        self.assertEqual(typing_errors.seeded(7, "x").random(), typing_errors.seeded(7, "x").random())
        # The scripts are seeded by the path the original tool was given, whatever the data directory is.
        self.assertEqual(typing_errors.seed_key(BY_NAME["colloquial-dev"]), "data/build/corpus/dev_colloquial.txt")
        self.assertEqual(typing_errors.seed_key(BY_NAME["smoke"]), "nlp/eval/colloquial_smoke.txt")
        self.assertEqual(typing_errors.seed_key(EvalSet("x", "/tmp/x.txt", "text", "local", "file", "")), "/tmp/x.txt")

    def test_strip_reranker(self):
        options = ["--decoder", "future", "--reranker", "dir", "--rerank-weight=1", "--rerank-count", "16"]
        self.assertEqual(strip_options(options, typing_errors.RERANK_OPTIONS), ["--decoder", "future"])


if __name__ == "__main__":
    unittest.main()

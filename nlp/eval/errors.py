"""Where conversion fails: the same runs typed with the habits real typists have, through the real composer.

    zt eval errors --set colloquial-dev wiki-dev
    zt eval errors --config engine -k 32 --examples 3

Each run is typed several ways (only runs a habit applies to count for it):

    clean     the most common reading of every word, every first tone with Space
    sandhi    一 and 不 typed as pronounced: 一直 ㄧˋ ㄓˊ, 一定 ㄧˊ ㄉㄧㄥˋ, 不要 ㄅㄨˊ ㄧㄠˋ
    neutral   neutral-tone syllables typed with Space (first tone), as many people do
    nospace   first-tone syllables followed by another syllable typed without Space: syllable boundaries are left
              for the input method to find
    typo5     5% of syllables get one keyboard slip (the mix of `zt eval typo`)

For each it reports whole-run accuracy of what is shown and recall@K: how often the right sentence is among the
engine's K best (zhuyin-cli --report-nbest), the most any reranker could reach.  Low recall means the engine (lexicon,
segmentation, language model) loses the answer; high recall with low accuracy means the choice is wrong.  Runs need
one reading per character.  Each item is one run typed with one habit (item group = the habit).
"""

from __future__ import annotations

import argparse
import random
from typing import Any

from nlp.common.engine import EnginePool
from nlp.common.results import Section
from nlp.common.sets import EvalSet
from nlp.common.zhuyin import KEYS, TONES, Lexicon, sample_runs
from nlp.eval.base import Evaluator
from nlp.eval.typo import noisy_script

HABITS = ["clean", "sandhi", "neutral", "nospace", "typo5"]


def habit_keys(syllables: list[str], space_after: list[bool] | None = None) -> str:
    """Keys for syllables, a first tone typed as a space character; `space_after[i]` False leaves that space out."""
    out = []
    for i, syllable in enumerate(syllables):
        keys = "".join(KEYS[c] for c in syllable)
        if syllable[-1] not in TONES and (space_after is None or space_after[i]):
            keys += " "
        out.append(keys)
    return "".join(out)


def sandhi(run: str, syllables: list[str]) -> list[str] | None:
    """Readings with 一 and 不 as pronounced before the next syllable's tone, or None when nothing changes."""
    changed = list(syllables)
    for i, ch in enumerate(run[:-1]):
        tone = syllables[i + 1][-1] if syllables[i + 1][-1] in TONES else ""
        if ch == "一" and syllables[i] == "ㄧ":
            changed[i] = "ㄧˊ" if tone in "ˋ˙" and tone else "ㄧˋ"
        elif ch == "不" and syllables[i] == "ㄅㄨˋ" and tone in ("ˋ", "˙"):
            changed[i] = "ㄅㄨˊ"
    return changed if changed != syllables else None


def neutral(syllables: list[str]) -> list[str] | None:
    """Readings with neutral tones typed as first tones, or None when there is none."""
    changed = [s[:-1] if s.endswith("˙") else s for s in syllables]
    return changed if changed != syllables else None


def nospace(syllables: list[str]) -> list[bool] | None:
    """Which syllables keep their Space when every first tone followed by another syllable drops it; None when no
    syllable has one to drop."""
    flags = [True] * len(syllables)
    candidates = [i for i in range(len(syllables) - 1) if syllables[i][-1] not in TONES]
    if not candidates:
        return None
    for i in candidates:
        flags[i] = False
    return flags


def habit_scripts(runs: list[tuple[str, list[str]]], seed: int) -> dict[str, list[tuple[str, str]]]:
    """(reference, keys without Enter) per habit, for the runs each habit applies to."""
    rng = random.Random(seed)
    habits: dict[str, list[tuple[str, str]]] = {name: [] for name in HABITS}
    for run, syllables in runs:
        habits["clean"].append((run, habit_keys(syllables)))
        if (s := sandhi(run, syllables)) is not None:
            habits["sandhi"].append((run, habit_keys(s)))
        if (s := neutral(syllables)) is not None:
            habits["neutral"].append((run, habit_keys(s)))
        if (flags := nospace(syllables)) is not None:
            habits["nospace"].append((run, habit_keys(syllables, flags)))
        habits["typo5"].append((run, noisy_script(syllables, 0.05, rng).removesuffix("<enter>")))
    return habits


class Errors(Evaluator):
    name = "errors"
    help = "accuracy and recall@K per typing habit"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument("-k", type=int, default=16, help="engine sentences for recall (default: 16)")
        parser.add_argument("--examples", type=int, default=0, help="print this many failures per habit")

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        runs = sample_runs(eval_set.file, Lexicon(pool.config.reading_lexicon), args.n, args.seed, whole_readings=True)
        habits = habit_scripts([(run.text, run.readings) for run in runs], args.seed)
        rows: dict[str, dict[str, Any]] = {}
        items: list[dict[str, Any]] = []
        examples: list[tuple[str, str]] = []
        for name, pairs in habits.items():
            if not pairs:
                continue
            scripts = [keys for _, keys in pairs]
            shown = pool.run([k + "<enter>" for k in scripts])
            best = pool.run(scripts, "--report-nbest", str(args.k))
            right = sum(got == run for (run, _), got in zip(pairs, shown))
            recalled = sum(run in line.split("\t") for (run, _), line in zip(pairs, best))
            rows[name] = {"runs": len(pairs), "shown": right / len(pairs), "recall": recalled / len(pairs)}
            items += [
                {
                    "id": f"{name}/{i}",
                    "group": name,
                    "reference": run,
                    "output": got,
                    "keys": keys,
                    "recalled": run in line.split("\t"),
                }
                for i, ((run, keys), got, line) in enumerate(zip(pairs, shown, best))
            ]
            misses = [(run, got, line.split("\t")[:3]) for (run, _), got, line in zip(pairs, shown, best) if got != run]
            examples += [(run, f"{got}   前三：{' / '.join(top)}") for run, got, top in misses[: args.examples]]
        return Section(eval_set.name, {"runs": len(runs), "k": args.k, "habits": rows}, items=items, examples=examples)

    def describe(self, m: dict[str, Any]) -> str:
        lines = [f"{m['runs']} runs", f"  {'habit':<9}{'runs':>6}{'shown':>9}{'recall@' + str(m['k']):>11}"]
        for name, row in m["habits"].items():
            lines.append(f"  {name:<9}{row['runs']:>6}{row['shown']:>9.1%}{row['recall']:>11.1%}")
        return "\n".join(lines)

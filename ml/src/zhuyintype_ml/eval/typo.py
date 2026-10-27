"""Keyboard slips at a fixed rate, through the real composer, with each tolerance mode of the key decoder.

    zt eval typo --set colloquial-dev wiki-dev
    zt eval typo --typo standard --rates 0 0.05 0.1 --config engine

Each syllable gets one slip with probability `rate`.  A slip is drawn from the types that apply to the syllable, with
weights 30/15/30/10/8/7: a 3↔4 tone swap, a 6↔7 tone swap, a neighbouring key, two adjacent keys swapped, a key left
out, a key added.  Every slip changes the keys.  The runs and their slips are the same for every mode and setting
(seeded), so modes compare on the same keys.

Reported per rate and mode: whole-run accuracy and character accuracy (Levenshtein alignment, so a dropped syllable
costs one character).  At rate 0, the false-correction rate counts runs that `--typo off` gets right and the mode gets
wrong, and changed-vs-off counts runs whose output differs from off at all.  Each item is one run under one rate and
mode (item group "RATE/MODE").
"""

from __future__ import annotations

import argparse
import random
from typing import Any

from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import KEYS, TONES, Lexicon, sample_runs
from zhuyintype_ml.eval.base import Evaluator

MODES = ["off", "standard", "aggressive"]
ROWS = ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"]
NEIGHBOURS: dict[str, set[str]] = {key: set() for row in ROWS for key in row}
for _row, _keys in enumerate(ROWS):
    for _index, _key in enumerate(_keys):
        _adjacent = [_keys[_index + 1]] if _index + 1 < len(_keys) else []
        if _row:
            _adjacent += [ROWS[_row - 1][i] for i in (_index, _index + 1) if i < len(ROWS[_row - 1])]
        for _other in _adjacent:
            NEIGHBOURS[_key].add(_other)
            NEIGHBOURS[_other].add(_key)
INPUT_KEYS = "".join(dict.fromkeys(KEYS.values())) + " "
WEIGHTS = (30, 15, 30, 10, 8, 7)


def mutations(keys: str) -> list[list[str]]:
    """Candidates per slip type; punctuation neighbours remain real physical slips."""
    return [
        [keys[:-1] + {"3": "4", "4": "3"}[keys[-1]]] if keys[-1] in "34" else [],
        [keys[:-1] + {"6": "7", "7": "6"}[keys[-1]]] if keys[-1] in "67" else [],
        [keys[:i] + other + keys[i + 1 :] for i, key in enumerate(keys) for other in sorted(NEIGHBOURS.get(key, ()))],
        [keys[:i] + keys[i + 1] + keys[i] + keys[i + 2 :] for i in range(len(keys) - 1) if keys[i] != keys[i + 1]],
        [keys[:i] + keys[i + 1 :] for i in range(len(keys))],
        [keys[:i] + key + keys[i:] for i in range(len(keys) + 1) for key in INPUT_KEYS],
    ]


def noisy_script(readings: list[str], rate: float, rng: random.Random) -> str:
    """Keys for `readings` (a first tone typed as a space character), each syllable slipped with probability `rate`,
    then Enter."""
    result = []
    for syllable in readings:
        keys = "".join(KEYS[c] for c in syllable)
        if syllable[-1] not in TONES:
            keys += " "
        if rng.random() < rate:
            choices = [(weight, variants) for weight, variants in zip(WEIGHTS, mutations(keys)) if variants]
            variants = rng.choices([v for _, v in choices], weights=[w for w, _ in choices])[0]
            changed = rng.choice(variants)
            assert changed != keys
            keys = changed
        result.append(keys)
    return "".join(result) + "<enter>"


def edit_distance(want: str, got: str) -> int:
    previous = list(range(len(got) + 1))
    for i, a in enumerate(want, 1):
        current = [i]
        for j, b in enumerate(got, 1):
            current.append(min(current[-1] + 1, previous[j] + 1, previous[j - 1] + (a != b)))
        previous = current
    return previous[-1]


def metrics(wanted: list[str], outputs: list[str]) -> dict[str, float]:
    """Character accuracy from the Levenshtein alignment (useful after a dropped syllable) and whole-run accuracy."""
    errors = sum(edit_distance(want, got) for want, got in zip(wanted, outputs))
    return {
        "character_accuracy": max(0, 1 - errors / sum(map(len, wanted))),
        "whole_run_accuracy": sum(a == b for a, b in zip(wanted, outputs)) / len(wanted),
    }


class Typo(Evaluator):
    name = "typo"
    help = "accuracy with keyboard slips at fixed rates, per tolerance mode"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "--typo", choices=[*MODES, "all"], default="all", help="tolerance mode to measure (default: all)"
        )
        parser.add_argument(
            "--rates", nargs="+", type=float, default=[0, 0.1, 0.2], help="slip rates per syllable (default: 0 .1 .2)"
        )

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        if args.n < 1 or any(not 0 <= rate <= 1 for rate in args.rates):
            raise SystemExit("zt eval typo: -n must be positive and rates between 0 and 1")
        runs = sample_runs(eval_set.file, Lexicon(pool.config.reading_lexicon), args.n, args.seed)
        if len(runs) < args.n:
            raise SystemExit(f"zt eval typo: {eval_set.name} has only {len(runs)} readable runs, need {args.n}")
        wanted = [run.text for run in runs]
        clean = [noisy_script(run.readings, 0, random.Random(args.seed)) for run in runs]
        modes = MODES if args.typo == "all" else [args.typo]
        scripts_at: dict[float, list[str]] = {}
        for rate in args.rates:
            rng = random.Random(args.seed)
            scripts_at[rate] = [noisy_script(run.readings, rate, rng) for run in runs]
        # Every (rate, mode) at once over one pool, so no run waits for the slowest lines of the one before.
        jobs = [(clean, ("--typo", "off"))] + [
            (scripts_at[rate], ("--typo", mode)) for rate in args.rates for mode in modes if mode != "off" or rate
        ]
        results = iter(pool.run_many(jobs))
        baseline = next(results)
        rows: list[dict[str, Any]] = []
        items: list[dict[str, Any]] = []
        for rate in args.rates:
            scripts = scripts_at[rate]
            for mode in modes:
                outputs = baseline if mode == "off" and rate == 0 else next(results)
                row: dict[str, Any] = {"rate": rate, "typo": mode, "runs": len(runs), **metrics(wanted, outputs)}
                if rate == 0:
                    row["false_correction_rate"] = sum(
                        a == w and b != w for a, b, w in zip(baseline, outputs, wanted)
                    ) / len(wanted)
                    row["changed_vs_off"] = sum(a != b for a, b in zip(baseline, outputs)) / len(wanted)
                rows.append(row)
                group = f"{rate:g}/{mode}"
                items += [
                    {"id": f"{group}/{i}", "group": group, "reference": want, "output": got, "keys": script}
                    for i, (want, got, script) in enumerate(zip(wanted, outputs, scripts))
                ]
        return Section(eval_set.name, {"runs": len(runs), "rows": rows}, items=items)

    def describe(self, m: dict[str, Any]) -> str:
        lines = [f"{m['runs']} runs"]
        for row in m["rows"]:
            line = (
                f"  rate {row['rate']:<4g} {row['typo']:<10} whole {percent(row['whole_run_accuracy'])}, "
                f"character {percent(row['character_accuracy'])}"
            )
            if "false_correction_rate" in row:
                line += (
                    f", false corrections {percent(row['false_correction_rate'])}, "
                    f"changed vs off {percent(row['changed_vs_off'])}"
                )
            lines.append(line)
        return "\n".join(lines)

"""Timed keyboard errors: real typing slips with key timing, through the real composer (method: TYPING_ERRORS.md).

    zt eval typing-errors --set colloquial-dev wiki-dev -n 300
    zt eval typing-errors --config engine --variant current "" \\
        --variant current+reranker "--reranker data/build/reranker --rerank-weight 1 --rerank-count 16"
    zt eval typing-errors --conditions exact hotspot nospace-30 --examples 2

Every key carries a `<dt:MS>` gap drawn from a log-normal around 120 ms (60 ms when fast), shorter across hands and
longer on the same finger.  Conditions: exact, exact-fast, swaps within and across syllables, rollover (a swap typed
almost at once), hotspot (a neighbouring key from a Gaussian landing point), an extra key, a missing key, a tone
swap, first tones without Space (all or 30%), and a mixed condition with all of them at low rates.  Runs a condition
does not apply to are left out of it.

Reported per condition and variant: whole-run and character accuracy of what is committed, recall@16 of the engine
alone (reranker options removed), and at exact/exact-fast the false-correction rate: output different from the
variant's exact output.  Each variant adds its zhuyin-cli options to the configuration; all variants type the same
scripts (a fixed generator per run and condition).  Each item is one run under one condition and variant (item group
"CONDITION/VARIANT").
"""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
import os
import random
import shlex
from dataclasses import dataclass, replace
from typing import Any

from nlp.common.engine import EnginePool, strip_options
from nlp.common.results import Section
from nlp.common.sets import EvalSet
from nlp.common.zhuyin import Lexicon, keys, sample_runs
from nlp.eval.base import Evaluator
from nlp.eval.typo import metrics

CONDITIONS = (
    "exact",
    "exact-fast",
    "swap-in-syllable",
    "swap-across",
    "swap-across-2",
    "rollover",
    "rollover-2",
    "hotspot",
    "hotspot-2",
    "extra-key",
    "missing-key",
    "tone",
    "nospace-all",
    "nospace-30",
    "mixed",
)
RERANK_OPTIONS = {"--reranker", "--rerank-weight", "--rerank-count", "--exact-margin"}
# Options that would break the isolation of batch lines or change what they print.
FORBIDDEN = {"--keep-memory", "--report-nbest", "--batch", "--timing"}
TONE_SWAP = dict(zip("3467", "4376"))
RECALL = 16

# Key centres in key-width units: number row is the origin, then real US stagger.
POSITIONS: dict[str, tuple[float, float]] = {}
FINGERS: dict[str, tuple[str, int]] = {}
for _y, (_row, _offset) in enumerate(
    (("1234567890-=", 0), ("qwertyuiop[]\\", 0.25), ("asdfghjkl;'", 0.5), ("zxcvbnm,./", 1))
):
    for _x, _key in enumerate(_row):
        POSITIONS[_key] = (_x + _offset, _y)
        FINGERS[_key] = ("L", min(_x, 3)) if _x < 5 else ("R", min(max(_x - 6, 0), 3))
POSITIONS["`"] = (-1, 0)
FINGERS["`"] = ("L", 0)
POSITIONS[" "] = (5.5, 4.25)
FINGERS[" "] = ("R", 4)  # right thumb


@dataclass(frozen=True)
class Stroke:
    key: str
    syllable: int
    gap: float | None = None
    first_tone: bool = False


Edit = tuple[int, ...]


def canonical(readings: list[str]) -> list[Stroke]:
    return [
        Stroke(key, i, first_tone=key == " ")
        for i, reading in enumerate(readings)
        for key in keys([reading]).replace("<space>", " ")
    ]


def neighbour(key: str, rng: random.Random) -> str:
    """Sample a Gaussian landing point, snap to a key centre, condition on a local slip."""
    x, y = POSITIONS[key]
    while True:
        px, py = rng.gauss(x, 0.45), rng.gauss(y, 0.3)
        other = min(POSITIONS, key=lambda k: (POSITIONS[k][0] - px) ** 2 + (POSITIONS[k][1] - py) ** 2)
        ox, oy = POSITIONS[other]
        if other != key and math.hypot(ox - x, oy - y) <= 1.6:
            return other


def mutation(
    strokes: list[Stroke], kind: str, rng: random.Random, count: int = 1, syllable: int | None = None
) -> tuple[list[Stroke], tuple[Edit, ...]] | None:
    """Return changed strokes and disjoint edit sites, or None when inapplicable."""
    sites: list[Edit] = []
    for i, stroke in enumerate(strokes):
        if syllable is not None and stroke.syllable != syllable:
            continue
        if kind in ("swap-in-syllable", "swap-across", "rollover"):
            if i + 1 == len(strokes) or stroke.key == strokes[i + 1].key:
                continue
            across = stroke.syllable != strokes[i + 1].syllable
            if kind == "swap-in-syllable" and across or kind == "swap-across" and not across:
                continue
            sites.append((i, i + 1))
        elif kind in ("hotspot", "extra-key"):
            # Space spans several key widths; the unit-key Gaussian is inappropriate there.
            if stroke.key != " ":
                sites.append((i,))
        elif kind == "missing-key" or kind == "tone" and stroke.key in TONE_SWAP:
            sites.append((i,))
    choices = [
        group
        for group in itertools.combinations(sites, count)
        if len({i for site in group for i in site}) == sum(map(len, group))
    ]
    if not choices:
        return None
    chosen = rng.choice(choices)
    result = list(strokes)
    for site in sorted(chosen, reverse=True):
        i = site[0]
        if len(site) == 2:
            result[i], result[i + 1] = result[i + 1], result[i]
            if kind == "rollover":
                result[i + 1] = replace(result[i + 1], gap=rng.uniform(0, 15))
        elif kind == "hotspot":
            result[i] = replace(result[i], key=neighbour(result[i].key, rng))
        elif kind == "extra-key":
            extra = replace(result[i], key=neighbour(result[i].key, rng), gap=None)
            pair = [extra, result[i]] if rng.random() < 0.5 else [result[i], extra]
            pair[1] = replace(pair[1], gap=rng.uniform(0, 20))
            result[i : i + 1] = pair
        elif kind == "missing-key":
            del result[i]
        elif kind == "tone":
            result[i] = replace(result[i], key=TONE_SWAP[result[i].key])
    assert [s.key for s in result] != [s.key for s in strokes]
    return result, chosen


def omit_spaces(strokes: list[Stroke], rng: random.Random, rate: float, last_syllable: int) -> list[Stroke]:
    return [
        s for s in strokes if not (s.first_tone and s.key == " " and s.syllable < last_syllable and rng.random() < rate)
    ]


def condition(readings: list[str], name: str, rng: random.Random) -> tuple[list[Stroke], bool, int] | None:
    """Strokes for `readings` typed under a condition, whether typed fast, and how many errors were made; None when
    the condition does not apply."""
    strokes = canonical(readings)
    fast = name == "exact-fast"
    edits = 0
    if name.startswith("nospace-"):
        changed = omit_spaces(strokes, rng, 1 if name == "nospace-all" else 0.3, len(readings) - 1)
        edits = len(strokes) - len(changed)
        if not edits:
            return None
        strokes = changed
    elif name == "mixed":
        fast = rng.random() < 0.3
        for sid in range(len(readings)):
            draw = rng.random()
            kind = next(
                (
                    kind
                    for threshold, kind in (
                        (0.03, "hotspot"),
                        (0.05, "rollover"),
                        (0.06, "extra-key"),
                        (0.07, "missing-key"),
                        (0.08, "tone"),
                    )
                    if draw < threshold
                ),
                None,
            )
            if kind and (mutated := mutation(strokes, kind, rng, syllable=sid)):
                strokes = mutated[0]
                edits += 1
        changed = omit_spaces(strokes, rng, 0.3, len(readings) - 1)
        edits += len(strokes) - len(changed)
        strokes = changed
    elif name not in ("exact", "exact-fast"):
        count = 2 if name.endswith("-2") else 1
        kind = name.removesuffix("-2")
        mutated = mutation(strokes, kind, rng, count)
        if mutated is None:
            return None
        strokes, _ = mutated
        edits = count
    return strokes, fast, edits


def timed_script(strokes: list[Stroke], rng: random.Random, fast: bool = False) -> str:
    result = []
    previous = None
    for stroke in strokes:
        hand, finger = FINGERS[stroke.key]
        factor = 1.0
        if previous is not None:
            if previous[0] != hand:
                factor = 0.75
            elif previous == (hand, finger):
                factor = 1.4
        gap = (
            stroke.gap if stroke.gap is not None else rng.lognormvariate(math.log((60 if fast else 120) * factor), 0.35)
        )
        result.append(f"<dt:{gap:.3f}>" + ("<space>" if stroke.key == " " else stroke.key))
        previous = (hand, finger)
    return "".join(result)


def seeded(seed: int, *parts: object) -> random.Random:
    """Stable across Python versions and processes; adding a variant never changes the typed inputs."""
    return random.Random(int.from_bytes(hashlib.sha256(json.dumps([seed, *parts]).encode()).digest(), "big"))


def seed_key(eval_set: EvalSet) -> str:
    """The set's path as the original tool was given it (data/build/...), which seeds its scripts, so the typed
    inputs stay identical to earlier results whatever the data directory is."""
    if eval_set.source == "file" or eval_set.path.startswith("nlp/"):
        return eval_set.path
    return f"data/build/{eval_set.path}"


def table(rows: list[dict[str, Any]]) -> str:
    lines = [
        "| Set | Condition | Variant | N | Whole | Char | Recall@16 | False corr. |",
        "|---|---|---|---:|---:|---:|---:|---:|",
    ]
    for row in rows:
        values = [
            "—" if row.get(k) is None else f"{row[k]:.1%}"
            for k in ("whole_run_accuracy", "character_accuracy", "recall_at_16", "false_correction_rate")
        ]
        lines.append(
            f"| {row['dataset']} | {row['condition']} | {row['config']} | {row['runs']} | " + " | ".join(values) + " |"
        )
    return "\n".join(lines)


class TypingErrors(Evaluator):
    name = "typing-errors"
    help = "timed typing errors (swaps, rollover, hotspots, missing Space) per condition"
    default_n = 300
    n_help = "runs per set before leaving out runs a condition does not apply to (default: 300)"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "--variant",
            nargs=2,
            action="append",
            metavar=("LABEL", "OPTIONS"),
            help='a labelled set of extra zhuyin-cli options, e.g. timed "--decoder timed" (repeatable; '
            'default: current "")',
        )
        parser.add_argument(
            "--conditions", nargs="+", choices=CONDITIONS, default=list(CONDITIONS), help="conditions (default: all)"
        )
        parser.add_argument("--examples", type=int, default=0, metavar="N", help="failures to keep per row")

    def variants(self, args: argparse.Namespace, base: list[str]) -> list[tuple[str, list[str]]]:
        variants = [(label, shlex.split(extra)) for label, extra in (args.variant or [("current", "")])]
        if len({label for label, _ in variants}) != len(variants):
            raise SystemExit("zt eval typing-errors: variant labels must be unique")
        for _, extra in variants:
            if any(x.split("=", 1)[0] in FORBIDDEN for x in base + extra):
                raise SystemExit(f"zt eval typing-errors: options must not include {', '.join(sorted(FORBIDDEN))}")
        return variants

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        if args.n < 1 or args.examples < 0:
            raise SystemExit("zt eval typing-errors: -n must be positive and --examples nonnegative")
        base = pool.config.options
        variants = self.variants(args, base)
        runs = sample_runs(eval_set.file, Lexicon(pool.config.reading_lexicon), args.n, args.seed, whole_readings=True)
        if len(runs) < args.n:
            raise SystemExit(
                f"zt eval typing-errors: {eval_set.name} has only {len(runs)} readable runs, need {args.n}"
            )
        key = seed_key(eval_set)
        commit_pools = {
            label: EnginePool(pool.config.with_options(base + extra), pool.jobs, pool.cli) for label, extra in variants
        }
        recall_options = {label: tuple(strip_options(base + extra, RERANK_OPTIONS)) for label, extra in variants}
        recall_pools = {
            options: EnginePool(pool.config.with_options(list(options)), pool.jobs, pool.cli)
            for options in recall_options.values()
        }
        clean: dict[str, dict[int, str]] = {}
        rows: list[dict[str, Any]] = []
        items: list[dict[str, Any]] = []
        for name in args.conditions:
            cases = []  # run index, reference, script, errors, fast
            for index, run in enumerate(runs):
                result = condition(run.readings, name, seeded(args.seed, key, index, name, "errors"))
                if result is not None:
                    strokes, fast, edits = result
                    timing_name = "exact" if name == "exact-fast" else name
                    script = timed_script(strokes, seeded(args.seed, key, index, timing_name, "timing"), fast)
                    cases.append((index, run.text, script, edits, fast))
            scripts = [case[2] for case in cases]
            recalls = {
                options: [line.split("\t") for line in recall_pool.run(scripts, "--report-nbest", str(RECALL))]
                for options, recall_pool in recall_pools.items()
            }
            for label, _ in variants:
                outputs = commit_pools[label].run([s + "<enter>" for s in scripts])
                best = recalls[recall_options[label]]
                if name == "exact":
                    clean[label] = {case[0]: got for case, got in zip(cases, outputs)}
                wanted = [case[1] for case in cases]
                row: dict[str, Any] = {
                    "dataset": eval_set.name,
                    "condition": name,
                    "config": label,
                    "runs": len(cases),
                    "injected_errors": sum(case[3] for case in cases),
                    "fast_runs": sum(case[4] for case in cases),
                    **(metrics(wanted, outputs) if cases else {"character_accuracy": None, "whole_run_accuracy": None}),
                    "recall_at_16": (
                        sum(case[1] in candidates for case, candidates in zip(cases, best)) / len(cases)
                        if cases
                        else None
                    ),
                }
                if name in ("exact", "exact-fast") and label in clean and cases:
                    row["false_correction_rate"] = sum(
                        got != clean[label][case[0]] for case, got in zip(cases, outputs)
                    ) / len(cases)
                row["examples"] = [
                    {"wanted": case[1], "committed": got, "nbest": candidates, "script": case[2]}
                    for case, got, candidates in zip(cases, outputs, best)
                    if case[1] != got
                ][: args.examples]
                rows.append(row)
                group = f"{name}/{label}"
                items += [
                    {
                        "id": f"{group}/{case[0]}",
                        "group": group,
                        "reference": case[1],
                        "output": got,
                        "nbest": candidates,
                        "keys": case[2],
                        "errors": case[3],
                        "fast": case[4],
                    }
                    for case, got, candidates in zip(cases, outputs, best)
                ]
        metrics_ = {
            "runs": len(runs),
            "variants": {label: shlex.join(extra) for label, extra in variants},
            "coreml_units": os.environ.get("ZHUYIN_UNITS", "cpuOnly"),
            "rows": rows,
        }
        return Section(eval_set.name, metrics_, items=items)

    def describe(self, m: dict[str, Any]) -> str:
        return f"{m['runs']} runs\n" + table(m["rows"])

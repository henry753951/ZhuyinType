"""The app's correction log replayed: for each kind of correction, does the engine now get the composition right
without it?

    zt eval corrections ~/Library/Application\\ Support/ZhuyinType/corrections.jsonl
    zt eval corrections log.jsonl --config engine -n 50 --json data/build/eval/corrections.json
    zt eval stats before.json after.json                 # paired by record and kind of correction

Each record (zhuyintype_ml.core.corrections) is typed again with its recorded settings (typo tolerance, punctuation
keys, character forms, fuzzy sounds; options given with --cli-args win) and its context when it was recorded:

    pick and the other in-place corrections   the keys of the final composition, without the list
    backspace, escape                         the keys as first typed, before they were deleted and typed again
    delete_after_commit                       the previous composition's keys, against the text the user retyped

Outcomes: right (the text the user ended with), unchanged (the text shown before the first correction) or other.
Records whose edits cannot be replayed (inside the composition, keys zhuyin-cli cannot write) are counted as not
replayable.  The replay has no personal memory, so a record the user's learned words decided can differ.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from typing import Any

from zhuyintype_ml.core import corrections
from zhuyintype_ml.core.engine import EnginePool, strip_options
from zhuyintype_ml.core.results import Report, Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.eval.base import Evaluator

OUTCOMES = ("right", "unchanged", "other")


def replay(items: list[corrections.Item], pool: EnginePool) -> list[str]:
    """What the engine commits for each item, with the item's recorded settings unless --cli-args gives them."""
    given = {option.partition("=")[0] for option in pool.config.options if option.startswith("--")}
    groups: dict[tuple[str, ...], list[int]] = defaultdict(list)
    for index, item in enumerate(items):
        groups[item.options].append(index)
    outputs = [""] * len(items)
    for options, indices in groups.items():
        extra = strip_options(list(options), given)
        config = pool.config.with_options(pool.config.options + extra)
        lines = EnginePool(config, pool.jobs, pool.cli).run([items[i].line for i in indices])
        for index, line in zip(indices, lines):
            outputs[index] = line
    return outputs


def outcome(item: corrections.Item, output: str) -> str:
    if output == item.truth:
        return "right"
    return "unchanged" if output == item.shown else "other"


class Corrections(Evaluator):
    name = "corrections"
    help = "replay the app's correction log (corrections.jsonl): right without the correction, per kind"
    kind = "corrections"
    default_sets: list[str] = []
    default_n = 0
    n_help = "records per file, from the start (default: 0, every record)"

    def configure(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument("files", nargs="*", metavar="FILE", help="correction logs (corrections.jsonl)")
        super().configure(parser)

    def run(self, args: argparse.Namespace, quiet: bool = False) -> Report:
        args.set = [*args.files, *args.set]
        if not args.set:
            raise SystemExit("zt eval corrections: give one or more correction logs (corrections.jsonl)")
        return super().run(args, quiet)

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        records = corrections.load(eval_set.file)
        if args.n > 0:
            records = records[: args.n]
        items = [item for record in records for item in corrections.replay_items(record)]
        outputs = replay(items, pool)
        groups: dict[str, dict[str, int]] = {
            kind: {"records": 0, "replayed": 0, **dict.fromkeys(OUTCOMES, 0)} for kind in corrections.CORRECTIONS
        }
        for record in records:
            for kind in record.get("corrections", []):
                if kind in groups:
                    groups[kind]["records"] += 1
        results: list[dict[str, Any]] = []
        for item, output in zip(items, outputs):
            result = outcome(item, output)
            groups[item.group]["replayed"] += 1
            groups[item.group][result] += 1
            results.append(
                {
                    "id": item.id,
                    "group": item.group,
                    "reference": item.truth,
                    "output": output,
                    "shown": item.shown,
                    "keys": item.keys,
                    "outcome": result,
                }
            )
        metrics = {
            "records": len(records),
            "replayed": len(items),
            **{name: sum(1 for r in results if r["outcome"] == name) for name in OUTCOMES},
            "kinds": {kind: counts for kind, counts in groups.items() if counts["records"]},
        }
        examples = [(r["reference"], r["output"]) for r in results if r["outcome"] != "right"][:8]
        return Section(eval_set.name, metrics, items=results, examples=examples)

    def describe(self, m: dict[str, Any]) -> str:
        lines = [
            f"{m['records']} records, {m['replayed']} replays: right {m['right']}, unchanged {m['unchanged']}, "
            f"other {m['other']}"
        ]
        for kind, c in m["kinds"].items():
            share = percent(c["right"] / c["replayed"]) if c["replayed"] else "-"
            lines.append(
                f"  {kind:20} {c['records']:4} records, {c['replayed']:4} replayed: right {c['right']} ({share}), "
                f"unchanged {c['unchanged']}, other {c['other']}"
            )
        return "\n".join(lines)

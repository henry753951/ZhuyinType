"""Whole-sentence conversion: each run is typed key by key and committed with Enter; how much of it comes out right.

    zt eval conversion --set colloquial-dev wiki-dev
    zt eval conversion --config engine --set wiki-test -n 400
    zt eval conversion --context line --set wiki-dev          # with the text before the run on its line
    zt eval conversion --split continue                       # committed in two parts, the second continuing

Runs: Han runs of 4–20 characters, seeded shuffle, the first n with readings (zhuyintype_ml.core.zhuyin.sample_runs).
Metrics: character accuracy, whole-run accuracy, and whole-run accuracy counting interchangeable forms as equal.
"""

from __future__ import annotations

import argparse
from typing import Any

from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import Lexicon, Run, keys, sample_runs, score
from zhuyintype_ml.eval.base import Evaluator


class Conversion(Evaluator):
    name = "conversion"
    help = "whole-sentence conversion accuracy"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "--context",
            choices=["none", "line"],
            default="none",
            help="line: type each run after the text before it on its line (default: none)",
        )
        parser.add_argument(
            "--split",
            choices=["none", "continue", "restart"],
            default="none",
            help="commit each run in two parts cut at the word boundary nearest its middle; continue: "
            "the second part follows the first; restart: a key goes to the app in between",
        )
        parser.add_argument(
            "--typing-lexicon",
            metavar="FILE",
            help="derive typed readings from this lexicon, to compare two lexicons on the same keys",
        )

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        lexicon = Lexicon(args.typing_lexicon or pool.config.reading_lexicon)
        runs = sample_runs(eval_set.file, lexicon, args.n if args.split == "none" else 10**9, args.seed)
        selected: list[Run] = []
        scripts: list[str] = []
        for run in runs:
            if len(selected) >= args.n:
                break
            prefix = run.before.replace("\t", " ").replace("\n", " ") + "\t" if args.context == "line" else ""
            if args.split == "none":
                selected.append(run)
                scripts.append(prefix + run.keys + "<enter>")
                continue
            cuts = [c for c in run.cuts[:-1] if 2 <= c <= len(run.text) - 2]
            if not cuts:
                continue
            cut = min(cuts, key=lambda c: abs(c - len(run.text) / 2))
            between = "<enter><left>" if args.split == "restart" else "<enter>"
            selected.append(run)
            scripts.append(prefix + keys(run.readings[:cut]) + between + keys(run.readings[cut:]) + "<enter>")
        outputs = [line.strip() for line in pool.run(scripts)]
        pairs = [(run.text, got) for run, got in zip(selected, outputs)]
        return Section(
            eval_set.name,
            score(pairs),
            items=[{"id": i, "reference": want, "output": got} for i, (want, got) in enumerate(pairs)],
            examples=[(want, got) for want, got in pairs if want != got][:8],
        )

    def describe(self, m: dict[str, Any]) -> str:
        return (
            f"{m['runs']} runs: whole {percent(m['whole'])}, character {percent(m['character'])}, "
            f"forms equal {percent(m['variants'])}"
        )

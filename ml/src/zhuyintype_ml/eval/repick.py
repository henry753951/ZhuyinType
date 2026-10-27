"""How many picks a sentence needs: a simulated user types each run, and while the result is wrong, moves the cursor
to the first wrong character, opens the candidate list and picks the first entry that matches the reference there.

    zt eval repick --set colloquial-dev wiki-dev
    zt eval repick --relearn --set colloquial-dev -n 200       # also: is a picked sentence remembered?

Reports the share of runs right with 0, ≤1, ≤2, ≤3 picks (0 picks is whole-run accuracy: typing without picking),
the mean picks over runs that end right, and how often a pick made another, already right, character wrong (a pick
that breaks what the user has read).  Runs are sampled as for `zt eval candidates`.

--relearn also types every run that needed picks a second time in the same zhuyin-cli process (--keep-memory),
after the picked version was committed, and reports how often the second typing shows what was learned.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from typing import Any

from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import Lexicon, sample_runs
from zhuyintype_ml.eval.base import Evaluator

MAX_PICKS = 3


def pick_keys(position: int, rank: int) -> str:
    """Cursor to the character, open the list, page to the entry and select it."""
    page, slot = divmod(rank - 1, 9)
    return "<home>" + "<right>" * position + "<down>" + "<pgdown>" * page + str(slot + 1)


@dataclass
class Typing:
    """One simulated run: what it should become, the keys typed so far (with picks) and how it ended."""

    target: str
    keys: str
    original: str
    # Picks it took to be right; -1 when the list has no right entry; None while unresolved.
    done: int | None = None
    broke: bool = False


def summary(typings: list[Typing]) -> dict[str, Any]:
    total = max(1, len(typings))
    within = [sum(1 for t in typings if t.done is not None and 0 <= t.done <= k) / total for k in range(MAX_PICKS + 1)]
    fixed = [t.done for t in typings if t.done is not None and t.done >= 0]
    return {
        "runs": len(typings),
        **{f"picks{k}": within[k] for k in range(MAX_PICKS + 1)},
        "mean_picks": sum(fixed) / max(1, len(fixed)),
        "broke": sum(t.broke for t in typings) / total,
    }


class Repick(Evaluator):
    name = "repick"
    help = "picks a simulated user needs until each run is right"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument("--relearn", action="store_true", help="retype runs fixed by picks after committing them")

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        runs = sample_runs(eval_set.file, Lexicon(pool.config.reading_lexicon), args.n, args.seed, whole_readings=True)
        typings = [Typing(run.text, run.keys, run.keys) for run in runs]
        first = pool.run([t.keys + "<enter>" for t in typings])
        shown = list(first)
        for typing, got in zip(typings, shown):
            if got == typing.target:
                typing.done = 0
        for picks in range(1, MAX_PICKS + 1):
            active = [(t, got) for t, got in zip(typings, shown) if t.done is None and len(got) == len(t.target)]
            if not active:
                break
            probes = []
            for typing, got in active:
                position = next(i for i, (a, b) in enumerate(zip(typing.target, got)) if a != b)
                probes.append((typing, got, position))
            lists = pool.run([t.keys + "<home>" + "<right>" * p + "<down>" for t, _, p in probes], "--report-list")
            retyped = []
            for (typing, got, position), line in zip(probes, lists):
                entries = line.split("\t") if line else []
                found = next(
                    (i + 1 for i, item in enumerate(entries) if item and typing.target[position:].startswith(item)),
                    None,
                )
                if found is None:
                    typing.done = -1  # cannot be fixed from the list
                    continue
                typing.keys += pick_keys(position, found)
                retyped.append((typing, got))
            outputs = pool.run([t.keys + "<enter>" for t, _ in retyped]) if retyped else []
            for (typing, before), after in zip(retyped, outputs):
                if len(after) == len(typing.target) and any(
                    b == t and a != t for b, a, t in zip(before, after, typing.target)
                ):
                    typing.broke = True
                if after == typing.target:
                    typing.done = picks
            latest = {id(t): out for (t, _), out in zip(retyped, outputs)}
            shown = [latest.get(id(t), s) for t, s in zip(typings, shown)]
        metrics = summary(typings)
        if args.relearn:
            picked = [t for t in typings if t.done is not None and t.done > 0]
            sessions = pool.sessions([[f"{t.keys}<enter>", f"{t.original}<enter>"] for t in picked])
            kept = sum(out[1] == t.target for t, out in zip(picked, sessions))
            metrics.update(relearn_runs=len(picked), relearned=kept / max(1, len(picked)))
        items = [
            {"id": i, "reference": t.target, "output": got, "picks": t.done, "broke": t.broke}
            for i, (t, got) in enumerate(zip(typings, first))
        ]
        examples = [(t.target, got) for t, got in zip(typings, shown) if t.done == -1][:8]
        return Section(eval_set.name, metrics, items=items, examples=examples)

    def describe(self, m: dict[str, Any]) -> str:
        line = (
            f"{m['runs']} runs: right with 0 picks {percent(m['picks0'])}, ≤1 {percent(m['picks1'])}, "
            f"≤2 {percent(m['picks2'])}, ≤3 {percent(m['picks3'])}; mean picks {m['mean_picks']:.2f}; "
            f"picks that broke a right character in {percent(m['broke'])} of runs"
        )
        if "relearned" in m:
            line += f"; retyped after picking: {percent(m['relearned'])} of {m['relearn_runs']} show what was learned"
        return line

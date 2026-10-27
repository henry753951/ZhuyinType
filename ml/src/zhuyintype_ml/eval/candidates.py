"""How well the candidate list fixes a wrong conversion: for every character the conversion got wrong, the cursor is
put on it, the list is opened, and the rank of the first right entry is recorded.

    zt eval candidates --set colloquial-dev wiki-dev
    zt eval candidates --config engine --set chat-dev -n 200 --json data/build/eval/candidates.json

A list entry is right when it equals the reference text from the cursor on (any length).  Entries that are what is
already shown there are skipped: the rank is among the alternatives the user would scan.  Reported: how often the
first right entry is first, on the first page (9), anywhere in the list, and its mean rank where found.  Runs are
sampled as for `zt eval conversion` (same seed) but need one reading per character, so the cursor can be put on any
character.  Each item is one run: its reference, the conversion and the rank for each wrong character.
"""

from __future__ import annotations

import argparse
from typing import Any

from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import Lexicon, sample_runs
from zhuyintype_ml.eval.base import Evaluator


def probe_keys(keys: str, position: int) -> str:
    """Typed keys, then the cursor to `position` and the list opened."""
    return keys + "<home>" + "<right>" * position + "<down>"


def rank(target: str, shown: str, line: str) -> int | None:
    """Rank of the first list entry that is right for `target`, among the entries that differ from what is shown."""
    # The list starts with what is shown there (wrong by construction); rank among the alternatives.
    items = [item for item in (line.split("\t") if line else []) if item and not shown.startswith(item)]
    return next((i + 1 for i, item in enumerate(items) if target.startswith(item)), None)


def summary(runs: int, ranks: list[int | None]) -> dict[str, Any]:
    count = max(1, len(ranks))
    found = [r for r in ranks if r]
    return {
        "runs": runs,
        "wrong": len(ranks),
        "first": sum(r == 1 for r in found) / count,
        "first_page": sum(r <= 9 for r in found) / count,
        "listed": len(found) / count,
        "mean_rank": sum(found) / max(1, len(found)),
    }


class Candidates(Evaluator):
    name = "candidates"
    help = "rank of the right entry in the candidate list at each wrong character"

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        runs = sample_runs(eval_set.file, Lexicon(pool.config.reading_lexicon), args.n, args.seed, whole_readings=True)
        converted = pool.run([run.keys + "<enter>" for run in runs])
        probes: list[tuple[int, str, str, str]] = []  # run index, target, shown, keys
        for index, (run, got) in enumerate(zip(runs, converted)):
            want = run.text
            if got == want or len(got) != len(want):
                continue
            for position in range(len(want)):
                if got[position] != want[position]:
                    probes.append((index, want[position:], got[position:], probe_keys(run.keys, position)))
        lists = pool.run([keys for *_, keys in probes], "--report-list") if probes else []
        ranks = [rank(target, shown, line) for (_, target, shown, _), line in zip(probes, lists)]
        by_run: dict[int, list[int | None]] = {}
        for (index, *_), found in zip(probes, ranks):
            by_run.setdefault(index, []).append(found)
        items = [
            {"id": i, "reference": run.text, "output": got, "ranks": by_run.get(i, [])}
            for i, (run, got) in enumerate(zip(runs, converted))
        ]
        examples = [
            (target[:1], f"{shown[:1]} (rank {r or '—'})") for (_, target, shown, _), r in zip(probes, ranks) if r != 1
        ][:8]
        return Section(eval_set.name, summary(len(runs), ranks), items=items, examples=examples)

    def describe(self, m: dict[str, Any]) -> str:
        return (
            f"{m['runs']} runs, {m['wrong']} wrong characters: first {percent(m['first'])}, "
            f"first page {percent(m['first_page'])}, listed {percent(m['listed'])}, mean rank {m['mean_rank']:.2f}"
        )

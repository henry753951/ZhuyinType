#!/usr/bin/env python3
"""How many picks a sentence needs: a simulated user types each run, and while the result is wrong, moves the cursor
to the first wrong character, opens the candidate list and picks the first entry that matches the reference there.

    python3 nlp/eval/repick_eval.py --cli .build/release/zhuyin-cli -n 400 --jobs 8 \
        --cli-arg=--reranker --cli-arg=data/build/reranker --test data/build/corpus/dev_colloquial.txt

Reports the share of runs right with 0, ≤1, ≤2, ≤3 picks (0 picks is whole-run accuracy: "no-selection typing"),
the mean picks over runs that end right, and how often a pick made another, already right, character wrong
(a pick that breaks what the user has read).  Runs are chosen as in conversion_eval.py (seed 7).
"""
import argparse
import os
import random
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from candidate_eval import run_cli  # noqa: E402
from conversion_eval import keys_for, load_lexicon, readings_for  # noqa: E402

MAX_PICKS = 3


def pick_keys(position, rank):
    """Cursor to the character, open the list, page to the entry and select it."""
    page, slot = divmod(rank - 1, 9)
    return "<home>" + "<right>" * position + "<down>" + "<pgdown>" * page + str(slot + 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cli", default=".build/release/zhuyin-cli")
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("-n", type=int, default=400)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--cli-arg", action="append", default=[])
    parser.add_argument("--test", nargs="+", required=True)
    args = parser.parse_args()
    lexicon = load_lexicon(args.lexicon)
    base = [args.cli, "--lexicon", args.lexicon, "--batch"] + args.cli_arg
    for test in args.test:
        with open(test, encoding="utf-8") as f:
            found = [m.group() for line in f for m in re.finditer(r"[一-鿿]{4,20}", line)]
        random.Random(args.seed).shuffle(found)
        runs = []
        for run in found:
            if len(runs) >= args.n:
                break
            readings = readings_for(run, lexicon)
            if readings and len(readings) == len(run):
                runs.append([run, keys_for(readings), 0, None, False])  # target, keys, picks, done-at, broke
        shown = run_cli(base, [r[1] + "<enter>" for r in runs], args.jobs)
        for run, got in zip(runs, shown):
            if got == run[0]:
                run[3] = 0
        for picks in range(1, MAX_PICKS + 1):
            active = [(r, got) for r, got in zip(runs, shown) if r[3] is None and len(got) == len(r[0])]
            if not active:
                break
            probes = []
            for run, got in active:
                position = next(i for i, (a, b) in enumerate(zip(run[0], got)) if a != b)
                probes.append((run, got, position))
            lists = run_cli(base + ["--report-list"],
                            [r[1] + "<home>" + "<right>" * p + "<down>" for r, _, p in probes], args.jobs)
            retyped = []
            for (run, got, position), line in zip(probes, lists):
                items = line.split("\t") if line else []
                rank = next((i + 1 for i, item in enumerate(items) if item and run[0][position:].startswith(item)), None)
                if rank is None:
                    run[3] = -1  # cannot be fixed from the list
                    continue
                run[1] += pick_keys(position, rank)
                retyped.append((run, got))
            outputs = run_cli(base, [r[1] + "<enter>" for r, _ in retyped], args.jobs) if retyped else []
            for (run, before), after in zip(retyped, outputs):
                if len(after) == len(run[0]) and any(b == t and a != t for b, a, t in zip(before, after, run[0])):
                    run[4] = True
                if after == run[0]:
                    run[3] = picks
            shown = [next((o for (r2, _), o in zip(retyped, outputs) if r2 is r), s) for r, s in zip(runs, shown)]
        total = max(1, len(runs))
        within = [sum(1 for r in runs if r[3] is not None and 0 <= r[3] <= k) / total for k in range(MAX_PICKS + 1)]
        fixed = [r[3] for r in runs if r[3] is not None and r[3] >= 0]
        print(f"[{os.path.basename(test)}] {len(runs)} runs: right with 0 picks {within[0]:.1%}, ≤1 {within[1]:.1%}, "
              f"≤2 {within[2]:.1%}, ≤3 {within[3]:.1%}; mean picks {sum(fixed) / max(1, len(fixed)):.2f}; "
              f"picks that broke a right character in {sum(r[4] for r in runs) / total:.1%} of runs")


if __name__ == "__main__":
    main()

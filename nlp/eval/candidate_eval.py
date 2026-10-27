#!/usr/bin/env python3
"""How well the candidate list fixes a wrong conversion: for every character the conversion got wrong, the cursor is
put on it, the list is opened, and the rank of the first right entry is recorded.

    python3 nlp/eval/candidate_eval.py --cli .build-dev/release/zhuyin-cli -n 400 --jobs 8 \
        --cli-arg=--reranker --cli-arg=data/build/reranker \
        --test data/build/corpus/dev_colloquial.txt data/build/corpus/dev_wikipedia.txt

A list entry is right when it equals the reference text from the cursor on (any length).  Entries that are what is
already shown there are skipped: the rank is among the alternatives the user would scan.  Reported: how often the
first right entry is first, on the first page (9), anywhere in the list, and its mean rank where found.  The runs and
their typed readings are chosen exactly as in conversion_eval.py (same seed), so the two tools see the same runs.
"""
import argparse
import os
import random
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from conversion_eval import keys_for, load_lexicon, readings_for  # noqa: E402


def run_cli(command, lines, jobs):
    """One output line per input line, over `jobs` CLI processes (batch lines are independent)."""
    jobs = max(1, min(jobs, len(lines)))
    size = -(-len(lines) // jobs)
    processes = [subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  text=True) for _ in range(jobs)]
    with ThreadPoolExecutor(jobs) as pool:
        results = list(pool.map(lambda job: processes[job].communicate(
            "".join(line + "\n" for line in lines[job * size:(job + 1) * size])), range(jobs)))
    for process, (_, error) in zip(processes, results):
        if process.returncode:
            raise RuntimeError(f"zhuyin-cli failed: {error[-500:]}")
    outputs = [line for out, _ in results for line in out.split("\n")[:-1]]
    if len(outputs) != len(lines):
        raise RuntimeError(f"zhuyin-cli returned {len(outputs)} lines for {len(lines)} inputs")
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cli", default=".build/release/zhuyin-cli")
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("-n", type=int, default=400)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--cli-arg", action="append", default=[], help="extra zhuyin-cli argument (repeatable)")
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
                runs.append((run, keys_for(readings)))
        converted = run_cli(base, [keys + "<enter>" for _, keys in runs], args.jobs)
        probes = []
        for (want, keys), got in zip(runs, converted):
            if got == want or len(got) != len(want):
                continue
            for position in range(len(want)):
                if got[position] != want[position]:
                    probes.append((want[position:], got[position:], keys + "<home>" + "<right>" * position + "<down>"))
        lists = run_cli(base + ["--report-list"], [keys for _, _, keys in probes], args.jobs) if probes else []
        ranks = []
        for (target, shown, _), line in zip(probes, lists):
            # The list starts with what is shown there (wrong by construction); rank among the alternatives.
            items = [item for item in (line.split("\t") if line else []) if item and not shown.startswith(item)]
            ranks.append(next((i + 1 for i, item in enumerate(items) if target.startswith(item)), None))
        count = max(1, len(ranks))
        found_ranks = [r for r in ranks if r]
        print(f"[{os.path.basename(test)}] {len(runs)} runs, {len(ranks)} wrong characters: first "
              f"{sum(r == 1 for r in found_ranks) / count:.1%}, first page {sum(r <= 9 for r in found_ranks) / count:.1%}, "
              f"listed {len(found_ranks) / count:.1%}, mean rank {sum(found_ranks) / max(1, len(found_ranks)):.2f}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""ZhuyinBench: the fixed evaluation run for every release, so versions can be compared line by line.

    python3 training/eval/bench/suite.py --version v0.4.0 --cli .build/release/zhuyin-cli --jobs 60 \
        --out docs/benchmarks/v0.4.0.json

Runs every tool through training/remote_eval/evaluate.py (parallel CLI processes) with the app configuration and
the engine alone, and writes a JSON record plus a Markdown page next to it.  docs/benchmarks/README.md is rebuilt
from all records by history.py.  The sets and their sizes are fixed per suite version; changing them means a new
SUITE name, so old and new numbers are never mixed.
"""
import argparse
import datetime
import json
import os
import platform
import re
import subprocess
import sys

SUITE = "zhuyinbench-1"
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
EVALUATE = os.path.join(ROOT, "training", "remote_eval", "evaluate.py")

SETS = {
    "wikipedia": "data/build/lm/test_sentences.txt",
    "colloquial": "data/build/corpus/test_colloquial.txt",
    "smoke": "training/eval/colloquial_smoke.txt",
    "chat_tt": "data/build/lm_chat/test_chat_tt.txt",
}
APP = ["--reranker", "data/build/reranker", "--rerank-count", "16", "--v3", "data/build/v3", "--v3-weight", "0.5",
       "--chat-lm", "data/build/lm_chat", "--chat-prior", "0.3"]
CONFIGS = {"app": APP, "engine": []}
# 善解's reusable rows (CC0; rows overlapping our training text removed).  Holdout shards are not here: they are kept
# for one-off confirmations, not tracked every release.
EXTERNAL = "training/eval/sets/external/shanjie/eval"
ROWS = {"dev302": f"{EXTERNAL}/bench/suite-v1/dev302.clean.rows.txt",
        "typing76": f"{EXTERNAL}/bench/suite-v1/typing76.clean.rows.txt",
        "reported": f"{EXTERNAL}/bench/suite-v2/user-reported.clean.rows.txt",
        "fw": f"{EXTERNAL}/bench/suite-v2/fw.clean.rows.txt",
        "trap": f"{EXTERNAL}/sets/trap.clean.rows.txt",
        "daily": f"{EXTERNAL}/sets/daily.clean.rows.txt"}


def run(tool, args, cli_args, jobs, cli):
    command = [sys.executable, EVALUATE, "--jobs", str(jobs)] + [f"--cli-arg={a}" for a in cli_args] + \
              [tool, "--cli", cli] + args
    print("$", " ".join(command[2:]), flush=True)
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if result.returncode:
        print(result.stdout[-2000:], result.stderr[-4000:], file=sys.stderr)
        raise RuntimeError(f"{tool} failed")
    return result.stdout


def conversion(output):
    """[name] N runs, character accuracy X%, whole-run accuracy Y%, counting variants as equal Z%"""
    rows = {}
    for match in re.finditer(r"\[([^\]]+)\] (\d+) runs, character accuracy ([\d.]+)%, whole-run accuracy ([\d.]+)%"
                             r"(?:, counting variants as equal ([\d.]+)%)?", output):
        name, runs, char, whole, variant = match.groups()
        rows[name] = {"runs": int(runs), "character": float(char), "whole": float(whole),
                      "variant_equal": float(variant) if variant else None}
    return rows


def habits(output):
    rows, current = {}, None
    for line in output.splitlines():
        header = re.match(r"\[([^\]]+)\] \d+ runs", line)
        if header:
            current = header.group(1)
            continue
        match = re.match(r"\s+(\w+)\s+(\d+)\s+([\d.]+)%\s+([\d.]+)%", line)
        if match and current:
            habit, runs, shown, recall = match.groups()
            rows.setdefault(current, {})[habit] = {"runs": int(runs), "shown": float(shown), "recall16": float(recall)}
    return rows


def candidates(output):
    rows = {}
    for match in re.finditer(r"\[([^\]]+)\] (\d+) runs, (\d+) wrong characters: first ([\d.]+)%, first page ([\d.]+)%, "
                             r"listed ([\d.]+)%, mean rank ([\d.]+)", output):
        name, runs, wrong, first, page, listed, rank = match.groups()
        rows[name] = {"runs": int(runs), "wrong": int(wrong), "first": float(first), "first_page": float(page),
                      "listed": float(listed), "mean_rank": float(rank)}
    return rows


def repicks(output):
    rows = {}
    for match in re.finditer(r"\[([^\]]+)\] (\d+) runs: right with 0 picks ([\d.]+)%, ≤1 ([\d.]+)%, ≤2 ([\d.]+)%, "
                             r"≤3 ([\d.]+)%; mean picks ([\d.]+); picks that broke a right character in ([\d.]+)%", output):
        name, runs, p0, p1, p2, p3, mean, broke = match.groups()
        rows[name] = {"runs": int(runs), "picks0": float(p0), "picks1": float(p1), "picks2": float(p2),
                      "picks3": float(p3), "mean_picks": float(mean), "broke": float(broke)}
    return rows


def rows_sets(output):
    """rows_eval prints one JSON object per rows file: {"source": ..., "n": ..., "shanjie": {"top1": ...}, ...}."""
    rows = {}
    for line in output.splitlines():
        if line.startswith('{"source"'):
            record = json.loads(line)
            name = next((k for k, v in ROWS.items() if v == record["source"]), record["source"])
            rows[name] = {"n": record["n"], "top1": round(record["shanjie"]["top1"] * 100, 2),
                          "cer": round(record["shanjie"]["cer"] * 100, 2),
                          "oracle16": round(record["shanjie"]["oracle16"] * 100, 2)}
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--cli", default=".build/release/zhuyin-cli")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 8)
    parser.add_argument("-n", type=int, default=400)
    parser.add_argument("--out", required=True)
    parser.add_argument("--quick", action="store_true", help="n=50, for checking the harness")
    args = parser.parse_args()
    n = "50" if args.quick else str(args.n)
    tests = list(SETS.values())
    record = {"suite": SUITE, "version": args.version, "date": datetime.date.today().isoformat(),
              "commit": subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True,
                                       text=True).stdout.strip(),
              "host": f"{platform.system()} {platform.machine()}", "n": int(n), "results": {}}
    results = record["results"]
    for label, config in CONFIGS.items():
        results[f"conversion/{label}"] = conversion(run("conversion_eval", ["-n", n, "--test", *tests], config,
                                                        args.jobs, args.cli))
    results["context/app"] = conversion(run("conversion_eval", ["-n", n, "--line-context", "--test", SETS["wikipedia"]],
                                            APP, args.jobs, args.cli))
    results["split/app"] = conversion(run("conversion_eval", ["-n", n, "--split", "continue", "--test",
                                                              SETS["colloquial"], SETS["wikipedia"]], APP, args.jobs,
                                          args.cli))
    results["habits/app"] = habits(run("error_analysis", ["-n", n, "--test", SETS["colloquial"], SETS["wikipedia"]],
                                       APP, args.jobs, args.cli))
    results["candidates/app"] = candidates(run("candidate_eval", ["-n", n, "--test", *tests], APP, args.jobs, args.cli))
    present = [path for path in ROWS.values() if os.path.exists(os.path.join(ROOT, path))]
    if present:
        for label, config in CONFIGS.items():
            results[f"external/{label}"] = rows_sets(run("rows_eval", ["--rows", *present, "--output",
                                                                       f"data/build/bench/rows-{label}.jsonl"],
                                                         config, args.jobs, args.cli))
    results["repick/app"] = repicks(run("repick_eval", ["-n", n, "--test", *tests], APP, args.jobs, args.cli))
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(record, f, ensure_ascii=False, indent=1)
        f.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()

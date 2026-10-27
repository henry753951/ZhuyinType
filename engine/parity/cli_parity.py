#!/usr/bin/env python3
"""Compare two zhuyin-cli builds (the Swift reference and the Rust port) line by line on the evaluation inputs.

    PYTHONPATH=ml/src python3 engine/parity/cli_parity.py --swift .build/release/zhuyin-cli \
        --rust engine/target/release/zhuyin-cli --jobs 8 --output data/build/remote_eval/cli-parity.json

Inputs are built exactly as `zt eval conversion` builds them (zhuyintype_ml.core.zhuyin.sample_runs: 400 runs per dev
set, seed 7; the encyclopedic set also with the text before each run) and as `zt eval rows` builds the shanjie rows.
Every engine-only configuration (no neural models) runs over every input set with both builds; the outputs must be
identical.  Runs on the GPU host (docs: AGENTS.md); Python standard library only.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "ml" / "src"))
from zhuyintype_ml.core.zhuyin import Lexicon, sample_runs  # noqa: E402
from zhuyintype_ml.eval.rows import load_rows  # noqa: E402

DEV = {
    "colloquial": ("data/build/corpus/dev_colloquial.txt", False),
    "wikipedia": ("data/build/corpus/dev_wikipedia.txt", False),
    "chat": ("data/build/lm_chat/dev_chat_tt.txt", False),
    "context": ("data/build/corpus/dev_wikipedia.txt", True),
}


def dev_scripts(path, line_context, lexicon, n=400, seed=7):
    """`zt eval conversion`'s selection with --split none (and --context line when `line_context`)."""
    scripts = []
    for run in sample_runs(path, lexicon, n, seed):
        prefix = run.before.replace("\t", " ").replace("\n", " ") + "\t" if line_context else ""
        scripts.append(prefix + run.keys)
    return scripts


def row_scripts():
    scripts = {}
    for path in sorted((ROOT / "ml/eval-sets/external/shanjie/eval/bench").glob("*/*.clean.rows.txt")):
        rows = load_rows(path)
        scripts["shanjie-" + path.name.split(".")[0]] = [r["context"] + "\t" + r["keys"] for r in rows]
    return scripts


# Each configuration: CLI flags, and how a typed script ends (what the batch line reports); `{keys}` repeats the
# script's keys.
CONFIGS = [
    ("default", [], "<enter>"),
    ("typo-off", ["--typo", "off"], "<enter>"),
    ("typo-aggressive", ["--typo", "aggressive"], "<enter>"),
    ("decoder-slots", ["--decoder", "slots"], "<enter>"),
    ("decoder-boundaries", ["--decoder", "boundaries"], "<enter>"),
    ("decoder-reorder", ["--decoder", "reorder"], "<enter>"),
    ("fuzzy-all", ["--fuzzy", "zh,ch,sh,en,an"], "<enter>"),
    ("chat-mixture", ["--chat-lm", "data/build/lm_chat", "--chat-prior", "0.3"], "<enter>"),
    ("sentence-boundaries-off", ["--sentence-boundaries", "off"], "<enter>"),
    ("end-share", ["--end-share", "0.5"], "<enter>"),
    ("hotspot", ["--hotspot", "on"], "<enter>"),
    ("punctuation-mac", ["--punctuation", "mac"], "<S-,><enter>"),
    ("forms-common", ["--forms", "common"], "<enter>"),
    ("nbest-16", ["--report-nbest", "16"], ""),
    ("list-home", ["--report-list"], "<home><down>"),
    ("list-before", ["--report-list", "--anchor", "before"], "<left><down>"),
    ("list-corrections", ["--report-list"], "<home><sup>"),
    ("list-single", ["--report-list"], "<end><sdown>"),
    ("pick-second", [], "<home><down><down><enter><enter>"),
    ("pick-and-learn", ["--keep-memory"], "<home><down><down><enter><enter>"),
    ("delete-and-retype", ["--keep-memory"], "<enter><bs><bs>{keys}<enter>"),
]


def run(cli, flags, lines, jobs, lexicon):
    command = [cli, "--lexicon", lexicon, "--batch", *flags]
    sequential = "--keep-memory" in flags
    jobs = 1 if sequential else max(1, min(jobs, len(lines)))
    size = -(-len(lines) // jobs)

    def part(start):
        chunk = lines[start:start + size]
        result = subprocess.run(command, input="".join(line + "\n" for line in chunk), capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(f"{cli} {' '.join(flags)} exited {result.returncode}: {result.stderr[-1000:]}")
        outputs = result.stdout.split("\n")[:-1]
        if len(outputs) != len(chunk):
            raise RuntimeError(f"{cli} returned {len(outputs)} lines for {len(chunk)}")
        return outputs

    with ThreadPoolExecutor(jobs) as pool:
        return [line for chunk in pool.map(part, range(0, len(lines), size)) for line in chunk]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--swift", required=True)
    parser.add_argument("--rust", required=True)
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--jobs", type=int, default=8, help="processes per build (both builds run at once)")
    parser.add_argument("--output", required=True)
    parser.add_argument("--limit", type=int, help="only the first N lines of each set (a quick local check)")
    args = parser.parse_args()
    lexicon = Lexicon(args.lexicon)
    inputs = {name: dev_scripts(path, context, lexicon) for name, (path, context) in DEV.items()}
    inputs.update(row_scripts())
    if args.limit:
        inputs = {name: lines[:args.limit] for name, lines in inputs.items()}
    report = {"inputs": {name: len(lines) for name, lines in inputs.items()}, "configurations": [], "differences": []}
    total = differing = 0
    for name, flags, ending in CONFIGS:
        for set_name, scripts in inputs.items():
            lines = [script + ending.replace("{keys}", script.split("\t")[-1]) for script in scripts]
            with ThreadPoolExecutor(2) as pool:
                swift, rust = pool.map(lambda cli: run(cli, flags, lines, args.jobs, args.lexicon), [args.swift, args.rust])
            different = [i for i, (a, b) in enumerate(zip(swift, rust)) if a != b]
            total += len(lines)
            differing += len(different)
            report["configurations"].append({"configuration": name, "set": set_name, "lines": len(lines),
                                             "different": len(different)})
            for i in different[:5]:
                report["differences"].append({"configuration": name, "set": set_name, "input": lines[i],
                                              "swift": swift[i], "rust": rust[i]})
            print(f"{name:24} {set_name:28} {len(lines):5} lines, {len(different)} different", flush=True)
    report["total"] = {"lines": total, "different": differing}
    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    Path(args.output).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"total: {total} lines, {differing} different")
    return 1 if differing else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Compare two zhuyin-cli builds (the Swift reference and the Rust port) line by line on the evaluation inputs.

    python3 engine/parity/cli_parity.py --swift engine/swift/.build/release/zhuyin-cli \
        --rust engine/target/release/zhuyin-cli --output cli-parity.json            # the full gate
    python3 engine/parity/cli_parity.py ... --quick                                    # the quick gate

Inputs are built exactly as `zt eval conversion` builds them (zhuyintype_ml.core.zhuyin.sample_runs: 400 runs per dev
set, seed 7; the encyclopedic set also with the text before each run) and as `zt eval rows` builds the shanjie rows.
Two suites, both by default: every engine-only configuration (no neural models), and the app's setup with the models
in `--models DIR` (`zhuyin-cli --models DIR`: reranker, v3 character model and chat model, run with ONNX Runtime by
both builds).  Every configuration runs over every input set with both builds; the outputs must be identical.

All (configuration, set) pairs share one pool of `--jobs` processes: a pair is split into one process per 50 lines
(at most `--jobs`), so model loading is paid where it pays off, and small sets do not leave cores idle.  A pair with
`--keep-memory` runs as one process, its lines in order.  `--quick` takes the first 50 lines of each set.  Runs on
the GPU host (docs: AGENTS.md); Python standard library only.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

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


def dev_scripts(path: str, line_context: bool, lexicon: Lexicon, n: int = 400, seed: int = 7) -> list[str]:
    """`zt eval conversion`'s selection with --split none (and --context line when `line_context`)."""
    scripts = []
    for run in sample_runs(path, lexicon, n, seed):
        prefix = run.before.replace("\t", " ").replace("\n", " ") + "\t" if line_context else ""
        scripts.append(prefix + run.keys)
    return scripts


def row_scripts() -> dict[str, list[str]]:
    scripts: dict[str, list[str]] = {}
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


def model_configs(models: str) -> list[tuple[str, list[str], str]]:
    """The app's setup: what is committed, the 16 best, candidate lists (the reranker orders them) and learning."""
    app = ["--models", models]
    return [
        ("app", app, "<enter>"),
        ("app-nbest-16", [*app, "--report-nbest", "16"], ""),
        ("app-list-home", [*app, "--report-list"], "<home><down>"),
        ("app-list-corrections", [*app, "--report-list"], "<home><sup>"),
        ("app-pick-second", app, "<home><down><down><enter><enter>"),
    ]


def command(cli: str, flags: list[str], lexicon: str) -> list[str]:
    return [cli, "--batch", *flags] if "--models" in flags else [cli, "--lexicon", lexicon, "--batch", *flags]


def batch(cli: list[str], lines: list[str]) -> list[str]:
    """One zhuyin-cli process over `lines`; one output line per input line."""
    result = subprocess.run(cli, input="".join(line + "\n" for line in lines), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{' '.join(cli)} exited {result.returncode}: {result.stderr[-1000:]}")
    outputs = result.stdout.split("\n")[:-1]
    if len(outputs) != len(lines):
        raise RuntimeError(f"{cli[0]} returned {len(outputs)} lines for {len(lines)}")
    return outputs


def chunks(lines: list[str], flags: list[str], jobs: int) -> list[list[str]]:
    """The processes of one pair: one per 50 lines, at most `jobs`; one for --keep-memory (lines depend on each
    other)."""
    count = 1 if "--keep-memory" in flags else max(1, min(jobs, len(lines) // 50))
    size = -(-len(lines) // count)
    return [lines[start : start + size] for start in range(0, len(lines), size)]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--swift", required=True)
    parser.add_argument("--rust", required=True)
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--jobs", type=int, default=24, help="zhuyin-cli processes at once, both builds together")
    parser.add_argument("--output", required=True)
    parser.add_argument("--limit", type=int, help="only the first N lines of each set")
    parser.add_argument("--quick", action="store_true", help="the quick gate: the first 50 lines of each set")
    parser.add_argument(
        "--models", metavar="DIR", default="data/build", help="the app setup's data (default: data/build)"
    )
    parser.add_argument("--suite", choices=["engine", "models", "both"], default="both")
    parser.add_argument("--only", nargs="+", metavar="NAME", help="only these configurations")
    args = parser.parse_args()
    limit = args.limit or (50 if args.quick else None)
    lexicon = Lexicon(args.lexicon)
    inputs = {name: dev_scripts(path, context, lexicon) for name, (path, context) in DEV.items()}
    inputs.update(row_scripts())
    if limit:
        inputs = {name: lines[:limit] for name, lines in inputs.items()}
    configs = CONFIGS if args.suite != "models" else []
    if args.suite != "engine":
        configs = configs + model_configs(args.models)
    if args.only:
        configs = [c for c in configs if c[0] in args.only]
    pairs = []
    with ThreadPoolExecutor(max(1, args.jobs)) as pool:
        # Submitted in report order, so the first pairs finish first and print as they complete.
        for name, flags, ending in configs:
            for set_name, scripts in inputs.items():
                lines = [script + ending.replace("{keys}", script.split("\t")[-1]) for script in scripts]
                parts = chunks(lines, flags, args.jobs)
                futures = {
                    build: [pool.submit(batch, command(cli, flags, args.lexicon), part) for part in parts]
                    for build, cli in (("swift", args.swift), ("rust", args.rust))
                }
                pairs.append((name, set_name, lines, futures))
        configurations: list[dict[str, object]] = []
        differences: list[dict[str, object]] = []
        report: dict[str, object] = {
            "inputs": {name: len(lines) for name, lines in inputs.items()},
            "configurations": configurations,
            "differences": differences,
        }
        total = differing = 0
        for name, set_name, lines, futures in pairs:
            swift = [line for future in futures["swift"] for line in future.result()]
            rust = [line for future in futures["rust"] for line in future.result()]
            different = [i for i, (a, b) in enumerate(zip(swift, rust)) if a != b]
            total += len(lines)
            differing += len(different)
            configurations.append(
                {"configuration": name, "set": set_name, "lines": len(lines), "different": len(different)}
            )
            for i in different[:5]:
                differences.append(
                    {"configuration": name, "set": set_name, "input": lines[i], "swift": swift[i], "rust": rust[i]}
                )
            print(f"{name:24} {set_name:28} {len(lines):5} lines, {len(different)} different", flush=True)
    report["total"] = {"lines": total, "different": differing}
    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    Path(args.output).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"total: {total} lines, {differing} different")
    return 1 if differing else 0


if __name__ == "__main__":
    sys.exit(main())

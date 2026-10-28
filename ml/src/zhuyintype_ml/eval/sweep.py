"""Named configurations × evaluation sets: every combination measured with one evaluator, one report each, one table.

    zt eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json
    zt eval sweep zhuyintype_ml/eval/matrices/validation.json -n 200 --jobs 32 --output data/build/eval/sweeps/try
    zt --remote eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json      # on the GPU host

A matrix is a JSON file (examples in ml/src/zhuyintype_ml/eval/matrices/):

    {"evaluator": "conversion",
     "configurations": [{"name": "plain", "config": "engine"},
                        {"name": "reranker", "config": "engine",
                         "cli_args": ["--reranker", "{data}/reranker", "--rerank-count", "16", "--rerank-weight", "1"]},
                        {"name": "app", "config": "app"}],
     "sets": [{"name": "colloquial", "set": "colloquial-dev"},
              {"name": "context", "set": "wiki-dev", "options": ["--context", "line"]}]}

`evaluator` defaults to conversion.  A configuration is a `--config` (app or engine, default engine) plus zhuyin-cli
options; `{data}` in an option is the data directory.  A set is a set name (`zt eval sets`) or a file, plus options of
the evaluator.  Names may contain letters, digits, `_` and `-`.  Each combination's report (with every item) is saved
as CONFIGURATION-SET.json in the output directory (default data/build/eval/sweeps/MATRIX); the table gives the runs,
whole-run accuracy and wall seconds of each.
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import time
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.eval import registry

NAME = re.compile(r"[A-Za-z0-9_-]+")


def load_matrix(path: Path) -> dict[str, Any]:
    matrix = json.loads(path.read_text(encoding="utf-8"))
    for entry in matrix.get("configurations", []) + matrix.get("sets", []):
        if not NAME.fullmatch(entry.get("name", "")):
            raise SystemExit(f"zt eval sweep: {path}: names may contain only letters, digits, _ and -")
    if not matrix.get("configurations") or not matrix.get("sets"):
        raise SystemExit(f"zt eval sweep: {path}: needs configurations and sets")
    return matrix


def argv(configuration: dict[str, Any], entry: dict[str, Any], args: argparse.Namespace, output: Path) -> list[str]:
    """Options of one combination, as `zt eval EVALUATOR` takes them."""
    options = [o.replace("{data}", str(args.data)) for o in configuration.get("cli_args", [])]
    result = ["--config", configuration.get("config", "engine"), "--data", str(args.data), "--set", entry["set"]]
    if options:
        result += ["--cli-args", shlex.join(options)]
    for extra in args.cli_args:
        result += ["--cli-args", extra]
    if args.n is not None:
        result += ["-n", str(args.n)]
    if args.jobs is not None:
        result += ["--jobs", str(args.jobs)]
    return result + ["--json", str(output)] + entry.get("options", [])


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("matrix", type=Path, help="matrix JSON (see ml/src/zhuyintype_ml/eval/matrices/)")
    parser.add_argument("-n", type=int, default=None, help="items per set (default: the evaluator's)")
    parser.add_argument("--jobs", type=int, default=None, help="parallel engine processes (default: CPU count)")
    parser.add_argument(
        "--cli-args", action="append", default=[], metavar="OPTIONS", help="zhuyin-cli options for every configuration"
    )
    parser.add_argument(
        "--data",
        type=lambda p: Path(p).resolve(),
        default=paths.data_dir(),
        help="data directory (default: data/build)",
    )
    parser.add_argument(
        "--output", type=Path, help="directory for the reports (default: data/build/eval/sweeps/MATRIX)"
    )


def run(args: argparse.Namespace) -> int:
    matrix = load_matrix(args.matrix)
    evaluator = registry.get(matrix.get("evaluator", "conversion"))
    output = args.output or paths.data("eval", "sweeps", args.matrix.stem)
    targets = [
        output / f"{configuration['name']}-{entry['name']}.json"
        for configuration in matrix["configurations"]
        for entry in matrix["sets"]
    ]
    if len(targets) != len(set(targets)):
        raise SystemExit("zt eval sweep: report paths collide")
    output.mkdir(parents=True, exist_ok=True)
    table = []
    for configuration in matrix["configurations"]:
        for entry in matrix["sets"]:
            target = output / f"{configuration['name']}-{entry['name']}.json"
            start = time.monotonic()
            report = evaluator.run(evaluator.arguments(argv(configuration, entry, args, target)), quiet=True)
            seconds = time.monotonic() - start
            section = report.sections[0]
            table.append((configuration["name"], entry["name"], len(section.items), section.metrics, seconds))
            print(f"finished {configuration['name']} / {entry['name']}: {seconds:.1f} s", flush=True)
    print("| Configuration | Set | Runs | Whole-run accuracy | Wall seconds |")
    print("|---|---|---:|---:|---:|")
    for name, set_name, count, metrics, seconds in table:
        whole = metrics.get("whole")
        accuracy = "—" if whole is None else f"{100 * whole:.1f}%"
        print(f"| {name} | {set_name} | {count} | {accuracy} | {seconds:.2f} |")
    return 0

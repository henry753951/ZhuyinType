"""ZhuyinBench: the fixed evaluation run for every release, so versions can be compared line by line.

    zt bench run v0.4.0                 # on the GPU host: docs/benchmarks/v0.4.0.json, then the pages
    zt bench run check-2 --quick        # 50 runs per set, to check the harness

Every evaluation runs in this process through the registry (zhuyintype_ml/eval/registry.py), the same code as `zt eval
NAME`, in the app configuration (`--config app`: the models in the data directory, set up as the app sets them up) and,
for conversion and the row sets, the engine alone (`--config engine`).  The record keeps the schema of the earlier
records (docs/benchmarks/v0.4.0.json): sets are keyed by their file names, percentages are rounded as printed.  Every
report, with its items, is also saved in data/build/bench/VERSION/ for paired comparisons (`zt eval stats`).

The sets and their sizes are fixed per suite version; changing them means a new SUITE name, so old and new numbers
are never mixed.
"""

from __future__ import annotations

import datetime
import platform
import shlex
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.results import Report
from zhuyintype_ml.core.sets import BY_NAME
from zhuyintype_ml.eval import registry

SUITE = "zhuyinbench-1"
# The test sets, in the order of the record.
TEXT_SETS = ["wiki-test", "colloquial-test", "smoke", "chat-test"]
CONFIGS = ("app", "engine")
# 善解's reusable rows (CC0; rows overlapping our training text removed).  Holdout shards are not here: they are kept
# for one-off confirmations, not tracked every release.
ROW_SETS = ["shanjie-dev302", "shanjie-typing76", "shanjie-reported", "shanjie-fw", "shanjie-trap", "shanjie-daily"]


def key(set_name: str) -> str:
    """The record's key for a text set: its file name, as in earlier records (test_sentences.txt)."""
    return Path(BY_NAME[set_name].path).name if set_name in BY_NAME else set_name


def pct(value: float) -> float:
    """A fraction as the percentage printed with one decimal (81.2), as earlier records hold it."""
    return float(f"{value:.1%}"[:-1])


def two(value: float) -> float:
    return float(f"{value:.2f}")


def conversion(report: Report) -> dict[str, Any]:
    return {
        key(s.name): {
            "runs": s.metrics["runs"],
            "character": pct(s.metrics["character"]),
            "whole": pct(s.metrics["whole"]),
            "variant_equal": pct(s.metrics["variants"]),
        }
        for s in report.sections
    }


def habits(report: Report) -> dict[str, Any]:
    return {
        key(s.name): {
            name: {"runs": row["runs"], "shown": pct(row["shown"]), "recall16": pct(row["recall"])}
            for name, row in s.metrics["habits"].items()
        }
        for s in report.sections
    }


def candidates(report: Report) -> dict[str, Any]:
    result = {}
    for s in report.sections:
        m = s.metrics
        result[key(s.name)] = {
            "runs": m["runs"],
            "wrong": m["wrong"],
            "first": pct(m["first"]),
            "first_page": pct(m["first_page"]),
            "listed": pct(m["listed"]),
            "mean_rank": two(m["mean_rank"]),
        }
    return result


def rows(report: Report) -> dict[str, Any]:
    result = {}
    for s in report.sections:
        shanjie = s.metrics["shanjie"]
        result[s.name.removeprefix("shanjie-")] = {
            "n": s.metrics["n"],
            "top1": round(shanjie["top1"] * 100, 2),
            "cer": round(shanjie["cer"] * 100, 2),
            "oracle16": round(shanjie["oracle16"] * 100, 2),
        }
    return result


def repick(report: Report) -> dict[str, Any]:
    result = {}
    for s in report.sections:
        m = s.metrics
        result[key(s.name)] = {
            "runs": m["runs"],
            **{f"picks{k}": pct(m[f"picks{k}"]) for k in range(4)},
            "mean_picks": two(m["mean_picks"]),
            "broke": pct(m["broke"]),
        }
    return result


def commit() -> str:
    done = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=paths.ROOT, capture_output=True, text=True)
    return done.stdout.strip()


def run_suite(
    version: str,
    n: int = 400,
    jobs: int | None = None,
    data: Path | None = None,
    cli_args: list[str] | None = None,
    reports: Path | None = None,
) -> dict[str, Any]:
    """The ZhuyinBench record for `version`.  `cli_args` adds zhuyin-cli options to every configuration (for checks
    only: a release record uses none)."""
    data = data or paths.data_dir()
    reports = reports or paths.data("bench", version)
    shared = ["--data", str(data)] + [a for text in cli_args or [] for a in ("--cli-args", text)]
    if jobs:
        shared += ["--jobs", str(jobs)]

    def measure(label: str, name: str, config: str, sets: list[str], *options: str) -> Report:
        evaluator = registry.get(name)
        argv = ["--config", config, *shared, "--set", *sets, *options, "--json", str(reports / f"{label}.json")]
        print(f"$ zt eval {name} {shlex.join(argv)}", flush=True)
        return evaluator.run(evaluator.arguments(argv), quiet=True)

    record: dict[str, Any] = {
        "suite": SUITE,
        "version": version,
        "date": datetime.date.today().isoformat(),
        "commit": commit(),
        "host": f"{platform.system()} {platform.machine()}",
        "n": n,
    }
    if cli_args:
        record["cli_args"] = cli_args
    results: dict[str, Any] = {}
    record["results"] = results
    count = ["-n", str(n)]
    for config in CONFIGS:
        results[f"conversion/{config}"] = conversion(
            measure(f"conversion-{config}", "conversion", config, TEXT_SETS, *count)
        )
    results["context/app"] = conversion(
        measure("context-app", "conversion", "app", ["wiki-test"], *count, "--context", "line")
    )
    results["split/app"] = conversion(
        measure("split-app", "conversion", "app", ["colloquial-test", "wiki-test"], *count, "--split", "continue")
    )
    results["habits/app"] = habits(measure("habits-app", "errors", "app", ["colloquial-test", "wiki-test"], *count))
    results["candidates/app"] = candidates(measure("candidates-app", "candidates", "app", TEXT_SETS, *count))
    present = [name for name in ROW_SETS if BY_NAME[name].present]
    if present:
        for config in CONFIGS:
            results[f"external/{config}"] = rows(measure(f"external-{config}", "rows", config, present))
    results["repick/app"] = repick(measure("repick-app", "repick", "app", TEXT_SETS, *count))
    return record


# Section converters by result group, for tests and for reading saved reports.
CONVERTERS: dict[str, Callable[[Report], dict[str, Any]]] = {
    "conversion": conversion,
    "context": conversion,
    "split": conversion,
    "habits": habits,
    "candidates": candidates,
    "external": rows,
    "repick": repick,
}

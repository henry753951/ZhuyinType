"""Per-key latency, CPU time, energy and memory of the engine on this machine (meant for the Mac; energy needs macOS).

    zt eval latency                                                # the app setup in data/build
    zt eval latency --data dist/ZhuyinType.app/Contents/Resources --json dist/release/metrics-mac.json
    zt eval latency -n 3                                           # a quick check of the harness

Types each sentence of the latency set (ml/eval-sets/latency_sentences.txt, one per line) from start to end in one
composition with `--timing`, one sentence at a time so they do not compete for the CPU, and reports the median over
sentences of each summary figure.  On macOS it measures the app's engine path, EngineBench (platforms/macos: the
engine and Core ML models set up as the app sets them up); elsewhere zhuyin-cli.  This is the one evaluation that runs
on the development Mac: the others run on the GPU host.  On Linux, energy is not reported.  Each item is one sentence
with its own figures.
"""

from __future__ import annotations

import argparse
import platform
import re
import statistics
import sys
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import Lexicon, keys
from zhuyintype_ml.eval.base import Evaluator

# Summary lines of --timing (engine/crates/zhuyin-cli/src/usage.rs; EngineBench prints the same).
FIGURES = {
    "p50_ms": r"keys \d+: p50 ([\d.]+) ms",
    "p95_ms": r"p95 ([\d.]+) ms",
    "max_ms": r"max ([\d.]+) ms\]",
    "cpu_ms_per_key": r"([\d.]+) ms per key",
    "energy_mj_per_key": r"([\d.]+) mJ per key",
    "memory_mb": r"memory (\d+) MB now",
}


def figures(output: str) -> dict[str, float | None]:
    """The summary figures in zhuyin-cli --timing output (None when a figure is not reported)."""
    found = {name: re.search(pattern, output) for name, pattern in FIGURES.items()}
    return {name: float(match.group(1)) if match else None for name, match in found.items()}


def medians(rows: list[dict[str, Any]]) -> dict[str, float | None]:
    def median(name: str) -> float | None:
        values = [r[name] for r in rows if r[name] is not None]
        return round(statistics.median(values), 2) if values else None

    return {name: median(name) for name in FIGURES}


class Latency(Evaluator):
    name = "latency"
    help = "per-key latency, CPU, energy and memory on this machine (Mac)"
    default_sets = ["latency"]
    default_n = 10
    n_help = "sentences per set (default: 10, the whole latency set)"

    def cli(self) -> Path:
        return paths.bench() if sys.platform == "darwin" else paths.cli()

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        lexicon = Lexicon(pool.config.reading_lexicon)
        rows: list[dict[str, Any]] = []
        for sentence in eval_set.file.read_text(encoding="utf-8").split():
            if len(rows) >= args.n:
                break
            readings = lexicon.readings(sentence)
            if not readings:
                continue
            output = pool.single(keys(readings) + "<enter>", "--timing")
            rows.append({"sentence": sentence, "syllables": len(readings), **figures(output)})
        metrics = {
            "machine": platform.machine(),
            "macos": platform.mac_ver()[0],
            "configuration": pool.config.arguments(),
            "median": medians(rows),
        }
        items = [{"id": i, **row} for i, row in enumerate(rows)]
        return Section(eval_set.name, metrics, items=items)

    def describe(self, m: dict[str, Any]) -> str:
        median = m["median"]
        energy = median["energy_mj_per_key"]
        return (
            f"per key p50 {median['p50_ms']} ms, p95 {median['p95_ms']} ms, max {median['max_ms']} ms; "
            f"CPU {median['cpu_ms_per_key']} ms; energy {'—' if energy is None else f'{energy} mJ'}; "
            f"memory {median['memory_mb']} MB ({m['machine']}, macOS {m['macos'] or '—'})"
        )

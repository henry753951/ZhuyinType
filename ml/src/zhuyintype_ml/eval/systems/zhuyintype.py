"""ZhuyinType through zhuyin-cli's batch mode (the EnginePool every evaluation uses): `app` is the app's setup
(`--config app`, zhuyin-cli --models), `engine` the lexicon and language model alone.

Each case is one batch line: the text before the run and a tab when the situation has context, then the keys of each
segment with Enter after each, timing marks included.  Lines are independent (the CLI resets the composer and its
learning per line).  The pool keeps its processes from start to close, so each loads the models once.
Latency: zhuyin-cli --timing, one sentence per process, the CLI's own per-key figures.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.engine import EngineConfig, EnginePool
from zhuyintype_ml.eval.latency import figures
from zhuyintype_ml.eval.systems.base import Case, Outcome, System, Task


def batch_line(case: Case, with_context: bool) -> str:
    prefix = case.context.replace("\t", " ").replace("\n", " ") + "\t" if with_context and case.context else ""
    return prefix + case.keys


class ZhuyinType(System):
    family = "zhuyintype"
    capabilities = frozenset({"context", "key-errors", "no-space"})

    def __init__(
        self, config: str, data: Path | None = None, jobs: int | None = None, options: list[str] | None = None
    ) -> None:
        self.name = f"zhuyintype-{config}"
        self.label = "ZhuyinType" if config == "app" else "ZhuyinType（只用引擎）"
        self.config = EngineConfig(config, data or paths.data_dir(), list(options or []))
        self.pool = EnginePool(self.config, jobs)

    def start(self) -> None:
        self.pool.__enter__()  # keep the engine processes (and their loaded models) from one task to the next

    def close(self) -> None:
        self.pool.__exit__()

    def resize(self, processes: int) -> None:
        self.pool.jobs = max(1, processes)

    def convert(self, task: Task, cases: list[Case]) -> list[Outcome]:
        lines = [batch_line(case, task.uses_context) for case in cases]
        return [Outcome(line.strip()) for line in self.pool.run(lines)]

    def latency(self, scripts: list[str]) -> dict[str, Any] | None:
        rows = []
        for script in scripts:
            found = figures(self.pool.single(script, "--timing"))
            rows.append({"keys": None, **{k: found[k] for k in ("p50_ms", "p95_ms", "max_ms")}})

        def median(name: str) -> float | None:
            values: list[float] = sorted(float(v) for r in rows if (v := r[name]) is not None)
            if not values:
                return None
            middle = len(values) // 2
            return round(values[middle] if len(values) % 2 else (values[middle - 1] + values[middle]) / 2, 2)

        return {
            "sentences": len(rows),
            "median": {name: median(name) for name in ("p50_ms", "p95_ms", "max_ms")},
            "measured": "zhuyin-cli --timing: time to handle each key inside the engine process",
            "items": rows,
        }

    def describe(self) -> dict[str, Any]:
        return {
            "config": self.config.describe(),
            "arguments": [paths.relative(a) if os.sep in a else a for a in self.config.arguments()],
            "cli": paths.relative(self.pool.cli),
            "processes": self.pool.jobs,
        }

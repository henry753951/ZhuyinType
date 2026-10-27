"""Running the engine: a configuration (which models, which options) and a pool of zhuyin-cli batch processes.

A configuration is named, so results say what they measured:

    app      everything in the data directory, set up exactly as the app sets it up (zhuyin-cli --models)
    engine   the lexicon and language model only, no neural models and no chat model

Extra zhuyin-cli options vary one setting against these.  Batch lines are independent (the CLI resets the composer
and its learning per line unless --keep-memory), so a batch is split over parallel processes, one core each.

The engine breaks every score tie in a fixed order (engine/DESIGN.md §4), so a result depends only on the input line,
never on the process or the order of the lines.
"""

from __future__ import annotations

import os
import subprocess
from collections.abc import Iterable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any

from . import paths

CONFIGURATIONS = ("app", "engine")
# One core per process (the pool provides the parallelism).
ENVIRONMENT = {"OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1"}


@dataclass
class EngineConfig:
    name: str = "app"
    data: Path = field(default_factory=paths.data_dir)
    options: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        if self.name not in CONFIGURATIONS:
            raise ValueError(f"unknown configuration {self.name!r}; use one of {', '.join(CONFIGURATIONS)}")

    @property
    def lexicon(self) -> Path:
        return self.data / "lexicon.tsv"

    @property
    def reading_lexicon(self) -> Path:
        """The lexicon TSV the evaluations derive typed readings from: this data directory's, or the default data
        directory's when this one has only the compiled lexicon.bin (an app bundle's Resources)."""
        return self.lexicon if self.lexicon.exists() else paths.data("lexicon.tsv")

    def arguments(self) -> list[str]:
        """zhuyin-cli options for this configuration."""
        base = ["--models", str(self.data)] if self.name == "app" else ["--lexicon", str(self.lexicon)]
        return base + self.options

    def describe(self) -> dict[str, Any]:
        return {"name": self.name, "data": paths.relative(self.data), "options": self.options}

    def with_options(self, options: list[str]) -> EngineConfig:
        """The same configuration with other extra options."""
        return replace(self, options=list(options))


def strip_options(options: list[str], names: Iterable[str]) -> list[str]:
    """`options` without the named options and their values (`--name value` or `--name=value`)."""
    names = set(names)
    result: list[str] = []
    i = 0
    while i < len(options):
        name, equals, _ = options[i].partition("=")
        if name in names:
            i += 1 if equals else 2
        else:
            result.append(options[i])
            i += 1
    return result


class EnginePool:
    """Runs batch lines through zhuyin-cli over `jobs` processes and returns one output line per input line."""

    def __init__(self, config: EngineConfig, jobs: int | None = None, cli: Path | None = None) -> None:
        self.config = config
        self.jobs = max(1, jobs or os.cpu_count() or 1)
        self.cli = cli or paths.cli()

    def command(self, *extra: str) -> list[str]:
        return [str(self.cli), *self.config.arguments(), "--batch", *extra]

    def run(self, lines: list[str], *extra: str) -> list[str]:
        if not lines:
            return []
        command = self.command(*extra)
        jobs = min(self.jobs, len(lines))
        size = -(-len(lines) // jobs)
        chunks = [lines[i * size : (i + 1) * size] for i in range(jobs)]
        env = dict(os.environ, **ENVIRONMENT)

        def one(chunk: list[str]) -> list[str]:
            done = subprocess.run(
                command, input="".join(line + "\n" for line in chunk), capture_output=True, text=True, env=env
            )
            if done.returncode:
                raise RuntimeError(f"zhuyin-cli failed: {done.stderr[-500:]}")
            return done.stdout.split("\n")[:-1]

        with ThreadPoolExecutor(jobs) as pool:
            outputs = [line for chunk in pool.map(one, chunks) for line in chunk]
        if len(outputs) != len(lines):
            raise RuntimeError(f"zhuyin-cli returned {len(outputs)} lines for {len(lines)} inputs")
        return outputs

    def sessions(self, sessions: list[list[str]], *extra: str) -> list[list[str]]:
        """Each session in its own process with --keep-memory, so what one line teaches the next is kept."""
        env = dict(os.environ, **ENVIRONMENT)
        command = self.command("--keep-memory", *extra)

        def one(lines: list[str]) -> list[str]:
            done = subprocess.run(
                command,
                input="".join(line + "\n" for line in lines),
                capture_output=True,
                text=True,
                env=env,
                check=True,
            )
            return done.stdout.split("\n")[:-1]

        with ThreadPoolExecutor(self.jobs) as pool:
            return list(pool.map(one, sessions))

    def single(self, keys: str, *extra: str) -> str:
        """Types `keys` in one composition outside batch mode and returns what zhuyin-cli prints (with --timing, the
        latency summary)."""
        command = [str(self.cli), *self.config.arguments(), *extra, keys]
        done = subprocess.run(command, capture_output=True, text=True, env=dict(os.environ, **ENVIRONMENT))
        if done.returncode:
            raise RuntimeError(f"zhuyin-cli failed: {done.stderr[-500:]}")
        return done.stdout

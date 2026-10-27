"""Running the engine: a configuration (which models, which options) and a pool of zhuyin-cli batch processes.

A configuration is named, so results say what they measured:

    app      everything in the data directory, set up exactly as the app sets it up (zhuyin-cli --models)
    engine   the lexicon and language model only, no neural models and no chat model

Extra zhuyin-cli options vary one setting against these.  Batch lines are independent (the CLI resets the composer
and its learning per line unless --keep-memory), so a batch is spread over parallel processes, one core each, which
take the next line whenever they finish one.  This needs zhuyin-cli --batch to flush each answer line as it writes it
(the Swift CLI calls fflush; Rust's stdout is line-buffered); a stand-in CLI must do the same (print(..., flush=True)).

Every process gets the same Swift hash seed (SWIFT_DETERMINISTIC_HASHING): the engine breaks some score ties in hash
order, which is otherwise random per process, so the same keys could convert differently from run to run.  With a
fixed seed, a result depends only on the input line, never on the process or the order of the lines.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import threading
from collections.abc import Iterable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any

from . import paths

CONFIGURATIONS = ("app", "engine")
# One core per process (the pool provides the parallelism) and a fixed hash seed (see above).
ENVIRONMENT = {"OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "SWIFT_DETERMINISTIC_HASHING": "1"}


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


class EngineProcess:
    """One zhuyin-cli --batch process answering lines one at a time (zhuyin-cli flushes after each answer)."""

    def __init__(self, command: list[str], env: dict[str, str]) -> None:
        self.command = command
        self.errors = tempfile.TemporaryFile()
        self.process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.errors,
            env=env,
            text=True,
            encoding="utf-8",
        )

    def failure(self) -> RuntimeError:
        self.process.kill()
        self.process.wait()
        self.errors.seek(0)
        return RuntimeError(f"zhuyin-cli failed: {self.errors.read().decode(errors='replace')[-500:]}")

    def ask(self, line: str) -> str:
        assert self.process.stdin and self.process.stdout
        try:
            self.process.stdin.write(line + "\n")
            self.process.stdin.flush()
        except BrokenPipeError:
            raise self.failure() from None
        answer = self.process.stdout.readline()
        if not answer.endswith("\n"):
            raise self.failure()
        return answer[:-1]

    def close(self) -> None:
        if self.process.poll() is None and self.process.stdin:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
        code = self.process.wait()
        if self.process.stdout:
            self.process.stdout.close()
        if code:
            raise self.failure()
        self.errors.close()


@dataclass
class _Batch:
    """One run_many batch: its command, lines and outputs, the next line to hand out, the processes working on it."""

    command: list[str]
    lines: list[str]
    outputs: list[str]
    next: int = 0
    workers: int = 0


class EnginePool:
    """Runs batch lines through zhuyin-cli over `jobs` processes and returns one output line per input line.

    Lines are handed out one at a time to whichever process is free, so a few long lines never leave the other
    processes idle at the end of a run.  `run_many` runs several batches with different options at once over the same
    `jobs` processes (a process that finishes its batch moves to the one with the most lines left).  Used as a context
    manager, the pool keeps its processes between runs, so each process loads the models once; otherwise every run
    starts and stops its own."""

    def __init__(self, config: EngineConfig, jobs: int | None = None, cli: Path | None = None) -> None:
        self.config = config
        self.jobs = max(1, jobs or os.cpu_count() or 1)
        self.cli = cli or paths.cli()
        self.keep = False
        self.idle: list[EngineProcess] = []

    def __enter__(self) -> EnginePool:
        self.keep = True
        return self

    def __exit__(self, *_: object) -> None:
        self.keep = False
        self.close()

    def close(self) -> None:
        idle, self.idle = self.idle, []
        for process in idle:
            process.close()

    def command(self, *extra: str) -> list[str]:
        return [str(self.cli), *self.config.arguments(), "--batch", *extra]

    def run(self, lines: list[str], *extra: str) -> list[str]:
        return self.run_many([(lines, extra)])[0]

    def run_many(self, batches: Sequence[tuple[list[str], Sequence[str]]]) -> list[list[str]]:
        """The outputs of several batches (lines, extra zhuyin-cli options), run at the same time."""
        work = [_Batch(self.command(*extra), list(lines), [""] * len(lines)) for lines, extra in batches]
        total = sum(len(b.lines) for b in work)
        if not total:
            return [[] for _ in work]
        env = dict(os.environ, **ENVIRONMENT)
        lock = threading.Lock()
        failures: list[BaseException] = []

        def choose(current: _Batch | None) -> _Batch | None:
            """The batch with the most lines left per process; None when moving would not pay (under a line each)."""
            if current is not None and current.next < len(current.lines):
                return current
            open_batches = [b for b in work if b.next < len(b.lines)]
            if not open_batches:
                return None
            best = max(open_batches, key=lambda b: (len(b.lines) - b.next) / (b.workers + 1))
            return best if (len(best.lines) - best.next) / (best.workers + 1) >= 1 else None

        def process_for(command: list[str]) -> EngineProcess:
            with lock:
                for i, process in enumerate(self.idle):
                    if process.command == command:
                        return self.idle.pop(i)
                stale = self.idle.pop(0) if self.idle else None
            if stale is not None:
                stale.close()
            return EngineProcess(command, env)

        def worker() -> None:
            batch: _Batch | None = None
            process: EngineProcess | None = None
            try:
                while not failures:
                    with lock:
                        chosen = choose(batch)
                        if chosen is not batch:
                            if batch is not None:
                                batch.workers -= 1
                            if chosen is not None:
                                chosen.workers += 1
                        if chosen is None:
                            break
                        index = chosen.next
                        chosen.next += 1
                    if chosen is not batch or process is None:
                        if process is not None:
                            process.close()
                        process = process_for(chosen.command)
                        batch = chosen
                    chosen.outputs[index] = process.ask(chosen.lines[index])
            except BaseException as error:
                failures.append(error)
                process = None
            if process is not None:
                if self.keep:
                    with lock:
                        self.idle.append(process)
                else:
                    process.close()

        threads = [threading.Thread(target=worker, daemon=True) for _ in range(min(self.jobs, total))]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        if failures:
            self.close()
            raise failures[0]
        return [b.outputs for b in work]

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

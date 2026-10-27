"""The shape every training pipeline has: named steps run in order, each a resumable unit (template method).

A model directory (training/<model>/) defines `PIPELINE = Pipeline(...)` in its `pipeline.py`.  `zt train` lists
pipelines, runs all steps or chosen ones, and resumes after the last finished step.  A step records when it finished
in `<out>/steps/<step>.json`, so a run interrupted on the GPU host continues where it stopped.

    zt train list
    zt train v3                       # every step, skipping finished ones
    zt train v3 export --force        # one step again
    zt --remote train v3              # the same on the GPU host
"""

from __future__ import annotations

import json
import subprocess
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from zhuyintype_ml.core import paths


@dataclass
class Context:
    """What a step gets: where to write, the data directory, and the options given to `zt train`."""

    out: Path
    data: Path
    options: dict[str, object] = field(default_factory=dict)

    def log(self, message: str) -> None:
        print(f"[{time.strftime('%H:%M:%S')}] {message}", flush=True)

    def python(self, module: str, *args: str, group: str | None = None, threads: int | None = None) -> None:
        """Runs `python -m module args` in the project environment and waits for it, with a pyproject.toml
        dependency group (such as "train" for PyTorch) when the step needs one; zt itself stays standard-library only.
        `threads` sets the OpenMP thread count of the child (BLAS stays single-threaded), as the training runs used."""
        subprocess.run(self._command(module, args, group), check=True, env=self._env(threads))

    def spawn(
        self, module: str, *args: str, group: str | None = None, threads: int | None = None, log: str | None = None
    ) -> subprocess.Popen[bytes]:
        """`python` without waiting: the child starts in the background, its output appended to `out/log` when
        given.  The caller waits for it or stops it."""
        output = open(self.out / log, "ab") if log else None  # noqa: SIM115 (owned by the child process)
        return subprocess.Popen(
            self._command(module, args, group),
            env=self._env(threads),
            stdout=output,
            stderr=subprocess.STDOUT if output else None,
        )

    @staticmethod
    def _command(module: str, args: tuple[str, ...], group: str | None) -> list[str]:
        return paths.python_command(module, *args, groups=(group,) if group else ())

    @staticmethod
    def _env(threads: int | None) -> dict[str, str]:
        env = dict(paths.python_env(), PYTHONDONTWRITEBYTECODE="1")
        if threads is not None:
            env.update(OMP_NUM_THREADS=str(threads), OPENBLAS_NUM_THREADS="1")
        return env


@dataclass(frozen=True)
class Step:
    name: str
    help: str
    run: Callable[[Context], None]


@dataclass(frozen=True)
class Pipeline:
    name: str
    help: str
    # Output directory under the data directory.
    output: str
    steps: tuple[Step, ...]

    def step(self, name: str) -> Step:
        for step in self.steps:
            if step.name == name:
                return step
        raise SystemExit(f"zt: {self.name} has no step {name!r}; steps: {', '.join(s.name for s in self.steps)}")

    def context(self, options: dict[str, object] | None = None) -> Context:
        out = paths.data(self.output)
        out.mkdir(parents=True, exist_ok=True)
        return Context(out, paths.data_dir(), options or {})

    def run(
        self, names: list[str] | None = None, force: bool = False, options: dict[str, object] | None = None
    ) -> None:
        context = self.context(options)
        for step in [self.step(n) for n in names] if names else self.steps:
            marker = context.out / "steps" / f"{step.name}.json"
            if marker.exists() and not force and not names:
                context.log(f"{self.name}/{step.name}: done earlier, skipped")
                continue
            context.log(f"{self.name}/{step.name}: {step.help}")
            started = time.time()
            step.run(context)
            marker.parent.mkdir(parents=True, exist_ok=True)
            marker.write_text(
                json.dumps({"finished": time.strftime("%Y-%m-%dT%H:%M:%S"), "seconds": round(time.time() - started, 1)})
                + "\n"
            )

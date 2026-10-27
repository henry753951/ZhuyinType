"""The interface every evaluation implements, and the options they share.

An Evaluator declares its own options, then measures one set at a time and returns a Section.  `run` builds the
engine configuration and the process pool once, goes through the requested sets, and collects a Report.  Commands
(`zt eval NAME`), ZhuyinBench and sweeps all call `run`, so a number is computed the same way wherever it appears:

    evaluator = registry.get("conversion")
    report = evaluator.run(evaluator.arguments(["--config", "engine", "--set", "wiki-dev", "-n", "50"]), quiet=True)
"""

from __future__ import annotations

import argparse
import shlex
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.engine import CONFIGURATIONS, EngineConfig, EnginePool
from zhuyintype_ml.core.results import Report, Section, print_report
from zhuyintype_ml.core.sets import EvalSet, resolve

# Shared options that describe how a report was made rather than what it measured; Report.config records them.
NOT_OPTIONS = ("func", "config", "data", "cli_args", "jobs", "json", "remote", "command", "tool")


class Evaluator(ABC):
    name: str
    help: str
    # "text" sets (sentences to sample runs from) or "rows" sets (context|truth|syllables).
    kind = "text"
    default_sets = ["colloquial-dev", "wiki-dev"]
    default_n = 400
    # Help for -n when "items per set" does not describe it.
    n_help: str | None = None

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:  # noqa: B027 (optional hook, empty by default)
        """Options of this evaluation, besides the shared ones."""

    @abstractmethod
    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        """Measures one set."""

    @abstractmethod
    def describe(self, metrics: dict[str, Any]) -> str:
        """One line (or a few) for people."""

    def configure(self, parser: argparse.ArgumentParser) -> None:
        add_shared_arguments(parser, self.default_sets, self.default_n, self.n_help)
        self.add_arguments(parser)

    def arguments(self, argv: list[str]) -> argparse.Namespace:
        """Parsed options for a call from code (ZhuyinBench, sweeps), exactly as `zt eval NAME ARGV` parses them."""
        parser = argparse.ArgumentParser(prog=f"zt eval {self.name}")
        self.configure(parser)
        return parser.parse_args(argv)

    def cli(self) -> Path:
        """The engine command line this evaluation runs (zhuyin-cli, built from the current sources)."""
        return paths.cli()

    def config(self, args: argparse.Namespace) -> EngineConfig:
        return EngineConfig(args.config, args.data, [a for text in args.cli_args for a in shlex.split(text)])

    def run(self, args: argparse.Namespace, quiet: bool = False) -> Report:
        config = self.config(args)
        pool = EnginePool(config, args.jobs, self.cli())
        options = {k: v for k, v in vars(args).items() if k not in NOT_OPTIONS and not callable(v)}
        report = Report(self.name, config.describe(), options)
        for eval_set in resolve(args.set, self.kind):
            report.add(self.evaluate(eval_set, pool, args))
        if not quiet:
            print_report(report, self.describe)
        if args.json:
            report.save(args.json)
        return report


def add_shared_arguments(
    parser: argparse.ArgumentParser, default_sets: list[str], default_n: int, n_help: str | None = None
) -> None:
    group = parser.add_argument_group("shared options")
    group.add_argument(
        "--config",
        choices=CONFIGURATIONS,
        default="app",
        help="app: every model, set up as the app does (default); engine: lexicon and LM only",
    )
    group.add_argument(
        "--data",
        type=lambda p: Path(p).resolve(),
        default=paths.data_dir(),
        help="data directory with the lexicon and models (default: data/build or $ZT_DATA)",
    )
    group.add_argument(
        "--cli-args",
        action="append",
        default=[],
        metavar="OPTIONS",
        help='extra zhuyin-cli options, e.g. "--rerank-weight 0.5" (repeatable)',
    )
    group.add_argument(
        "--set",
        nargs="+",
        default=default_sets,
        metavar="SET",
        help=f"set names, groups or files (default: {' '.join(default_sets)}; list: zt eval sets)",
    )
    group.add_argument("-n", type=int, default=default_n, help=n_help or f"items per set (default: {default_n})")
    group.add_argument("--seed", type=int, default=7, help="sampling seed (default: 7)")
    group.add_argument("--jobs", type=int, default=None, help="parallel engine processes (default: CPU count)")
    group.add_argument("--json", metavar="FILE", help="also save the report as JSON")

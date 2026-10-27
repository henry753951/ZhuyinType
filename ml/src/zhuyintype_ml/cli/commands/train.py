"""zt train: run model training pipelines (ml/src/zhuyintype_ml/training/<model>/pipeline.py)."""

from __future__ import annotations

import argparse
import importlib
import pkgutil

import zhuyintype_ml.training
from zhuyintype_ml.training.pipeline import Pipeline


def pipelines() -> dict[str, Pipeline]:
    found: dict[str, Pipeline] = {}
    for module in pkgutil.iter_modules(zhuyintype_ml.training.__path__):
        if not module.ispkg:
            continue
        try:
            pipeline = importlib.import_module(f"zhuyintype_ml.training.{module.name}.pipeline").PIPELINE
        except ModuleNotFoundError:
            continue
        found[pipeline.name] = pipeline
    return found


def register(sub: argparse._SubParsersAction) -> None:
    parser = sub.add_parser("train", description="Run model training pipelines.", help="run model training pipelines")
    parser.add_argument("model", nargs="?", help="pipeline name; `zt train list` lists them")
    parser.add_argument("steps", nargs="*", help="steps to run (default: every step not finished yet)")
    parser.add_argument("--force", action="store_true", help="run steps again even when finished")
    parser.add_argument(
        "--option",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="pipeline option, passed to its steps (repeatable)",
    )
    parser.set_defaults(func=run)


def run(args: argparse.Namespace) -> int:
    found = pipelines()
    if not args.model or args.model == "list":
        for name, pipeline in sorted(found.items()):
            print(f"{name:12} {pipeline.help}")
            for step in pipeline.steps:
                print(f"  {step.name:16} {step.help}")
        return 0
    if args.model not in found:
        raise SystemExit(f"zt: no pipeline {args.model!r}; known: {', '.join(sorted(found))}")
    options = dict(item.split("=", 1) for item in args.option)
    found[args.model].run(args.steps or None, args.force, options)
    return 0

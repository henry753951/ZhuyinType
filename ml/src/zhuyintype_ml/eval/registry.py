"""Every evaluation, by name.  `zt eval NAME`, ZhuyinBench and sweeps look them up here."""

from __future__ import annotations

import importlib

from zhuyintype_ml.eval.base import Evaluator

# name → "module:Class", imported when used.
EVALUATORS = {
    "conversion": "zhuyintype_ml.eval.conversion:Conversion",
    "candidates": "zhuyintype_ml.eval.candidates:Candidates",
    "repick": "zhuyintype_ml.eval.repick:Repick",
    "errors": "zhuyintype_ml.eval.errors:Errors",
    "typo": "zhuyintype_ml.eval.typo:Typo",
    "typing-errors": "zhuyintype_ml.eval.typing_errors:TypingErrors",
    "rows": "zhuyintype_ml.eval.rows:Rows",
    "losses": "zhuyintype_ml.eval.losses:Losses",
    "corrections": "zhuyintype_ml.eval.corrections:Corrections",
    "latency": "zhuyintype_ml.eval.latency:Latency",
}


def get(name: str) -> Evaluator:
    if name not in EVALUATORS:
        raise SystemExit(f"zt: no evaluation {name!r}; known: {', '.join(EVALUATORS)}")
    module, cls = EVALUATORS[name].split(":")
    evaluator: Evaluator = getattr(importlib.import_module(module), cls)()
    return evaluator


def all() -> list[Evaluator]:
    return [get(name) for name in EVALUATORS]

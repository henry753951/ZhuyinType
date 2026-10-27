"""Every evaluation, by name.  `zt eval NAME`, ZhuyinBench and sweeps look them up here."""

from __future__ import annotations

import importlib

from nlp.eval.base import Evaluator

# name → "module:Class", imported when used.
EVALUATORS = {
    "conversion": "nlp.eval.conversion:Conversion",
    "candidates": "nlp.eval.candidates:Candidates",
    "repick": "nlp.eval.repick:Repick",
    "errors": "nlp.eval.errors:Errors",
    "typo": "nlp.eval.typo:Typo",
    "typing-errors": "nlp.eval.typing_errors:TypingErrors",
    "rows": "nlp.eval.rows:Rows",
    "latency": "nlp.eval.latency:Latency",
}


def get(name: str) -> Evaluator:
    if name not in EVALUATORS:
        raise SystemExit(f"zt: no evaluation {name!r}; known: {', '.join(EVALUATORS)}")
    module, cls = EVALUATORS[name].split(":")
    evaluator: Evaluator = getattr(importlib.import_module(module), cls)()
    return evaluator


def all() -> list[Evaluator]:
    return [get(name) for name in EVALUATORS]

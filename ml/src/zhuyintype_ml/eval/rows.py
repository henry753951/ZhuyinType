"""Row sets (`context|truth|syllables`, the format of 善解's evaluation sets): each row typed exactly as given after
its context, committed, and the engine's 16 best checked.

    zt eval rows                                   # the shanjie group
    zt eval rows --set shanjie-dev302 --config engine --json data/build/eval/rows-engine.json
    zt eval rows --set my-rows.txt --shanjie-variants path/to/variants.tsv

Three ways to count a row right: raw (the same characters), shanjie (善解's folds of eight common characters, plus
its variants dictionary when --shanjie-variants is given; an unmodified evaluation-time file, never training data)
and variants (our interchangeable forms, as in `zt eval conversion`).  For each: top-1 (the committed text is right),
CER (善解's rowstats rule: zero for a row right under the normalization, otherwise the normalized Levenshtein distance,
over raw truth characters) and oracle@16 (a right sentence is among the engine's 16 best).  Each item is one row,
with the per-normalization outcomes `zt eval stats` pairs (id: SHA-256 of the row).
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

from zhuyintype_ml.core.engine import EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import VARIANTS, syllable_keys
from zhuyintype_ml.eval.base import Evaluator

COMMON = str.maketrans("她妳它牠嘗周臺裏", "他你他他嚐週台裡")
NORMALIZATIONS = ("raw", "shanjie", "variants")
ORACLE = 16
# zhuyin-cli options that would make rows depend on each other or change what is printed.
FORBIDDEN = ("--keep-memory", "--report-nbest", "--report-list")


def normalizer(variants: Path | str | None = None) -> Callable[[str], str]:
    """善解's normalization: its common-character folds, then the longest match in its variants file (when given)."""
    table: dict[str, str] = {}
    if variants:
        for i, line in enumerate(Path(variants).read_text(encoding="utf-8").splitlines(), 1):
            if not line or line.startswith("#"):
                continue
            pair = line.split("\t")
            if len(pair) != 2 or not all(pair) or pair[0] in table:
                raise ValueError(f"{variants}:{i}: invalid variant pair")
            table[pair[0]] = pair[1]
    longest = max(map(len, table), default=0)

    def apply(text: str) -> str:
        text = text.translate(COMMON)
        out, i = [], 0
        while i < len(text):
            for size in range(min(longest, len(text) - i), 0, -1):
                replacement = table.get(text[i : i + size])
                if replacement is not None:
                    out.append(replacement)
                    i += size
                    break
            else:
                out.append(text[i])
                i += 1
        return "".join(out)

    return apply


def load_rows(path: Path | str) -> list[dict[str, Any]]:
    """Rows of a row file; blank lines and # comments are skipped."""
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(Path(path).read_text(encoding="utf-8-sig").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("|")
        if len(parts) != 3 or not parts[1] or any(ord(c) < 32 for c in line):
            raise ValueError(f"{path}:{line_number}: expected context|truth|syllables without controls")
        context, truth, syllables = parts
        keys = syllable_keys(syllables)
        identity = json.dumps(parts, ensure_ascii=False, separators=(",", ":"))
        rows.append(
            {
                "row": len(rows) + 1,
                "source_line": line_number,
                "context": context,
                "truth": truth,
                "syllables": syllables,
                "keys": keys,
                "id": hashlib.sha256(identity.encode()).hexdigest(),
            }
        )
    if not rows:
        raise ValueError(f"{path}: no rows")
    return rows


def edit_distance(truth: str, predicted: str) -> int:
    previous = list(range(len(predicted) + 1))
    for i, expected in enumerate(truth, 1):
        current = [i]
        for j, actual in enumerate(predicted, 1):
            current.append(min(current[-1] + 1, previous[j] + 1, previous[j - 1] + (actual != expected)))
        previous = current
    return previous[-1]


def score_row(
    row: dict[str, Any], predicted: str, candidates: list[str], normalize: Callable[[str], str]
) -> dict[str, Any]:
    """The row with its prediction, 16 best and, per normalization, ok_*, errors_* and oracle16_*."""
    result = {**row, "prediction": predicted, "nbest": candidates, "gold_len": len(row["truth"])}
    folds: tuple[tuple[str, Callable[[str], str]], ...] = (
        ("raw", str),
        ("shanjie", normalize),
        ("variants", lambda s: s.translate(VARIANTS)),
    )
    for name, fold in folds:
        gold = fold(row["truth"])
        prediction = fold(predicted)
        ok = prediction == gold
        result["ok_" + name] = int(ok)
        result["errors_" + name] = edit_distance(gold, prediction)
        result["oracle16_" + name] = int(any(fold(s) == gold for s in candidates[:ORACLE]))
    return result


def evaluate_rows(
    rows: list[dict[str, Any]], pool: EnginePool, normalize: Callable[[str], str]
) -> list[dict[str, Any]]:
    """Commits and 16 best for each row (typed after its context), scored."""
    scripts = [row["context"] + "\t" + row["keys"] for row in rows]
    predictions = pool.run([s + "<enter>" for s in scripts])
    nbest = pool.run(scripts, "--report-nbest", str(ORACLE))
    return [
        score_row(row, predicted, line.split("\t") if line else [], normalize)
        for row, predicted, line in zip(rows, predictions, nbest)
    ]


def summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    n, chars = len(rows), sum(r["gold_len"] for r in rows)
    return {
        "n": n,
        "characters": chars,
        **{
            norm: {
                "top1": sum(r["ok_" + norm] for r in rows) / n,
                "cer": sum(r["errors_" + norm] for r in rows) / chars,
                "oracle16": sum(r["oracle16_" + norm] for r in rows) / n,
            }
            for norm in NORMALIZATIONS
        },
    }


class Rows(Evaluator):
    name = "rows"
    help = "row sets (context|truth|syllables): top-1, CER and oracle@16"
    kind = "rows"
    default_sets = ["shanjie"]
    default_n = 0
    n_help = "rows per set, from the start (default: 0, every row)"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "--shanjie-variants",
            metavar="FILE",
            help="善解's eval/variants.tsv, for its full normalization (default: common characters only)",
        )

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        if any(option in pool.config.options for option in FORBIDDEN):
            raise SystemExit(f"zt eval rows: options must not include {', '.join(FORBIDDEN)}")
        rows = load_rows(eval_set.file)
        if args.n > 0:
            rows = rows[: args.n]
        scored = evaluate_rows(rows, pool, normalizer(args.shanjie_variants))
        metrics = summary(scored)
        metrics["normalization"] = (
            "common characters + dictionary" if args.shanjie_variants else "common characters only"
        )
        items = [{**row, "reference": row["truth"], "output": row["prediction"]} for row in scored]
        examples = [(r["truth"], r["prediction"]) for r in scored if not r["ok_shanjie"]][:8]
        return Section(eval_set.name, metrics, items=items, examples=examples)

    def describe(self, m: dict[str, Any]) -> str:
        s = m["shanjie"]
        return (
            f"{m['n']} rows (shanjie normalization): top-1 {percent(s['top1'])}, CER {percent(s['cer'])}, "
            f"oracle@16 {percent(s['oracle16'])}; raw top-1 {percent(m['raw']['top1'])}"
        )

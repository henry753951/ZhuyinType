"""Confusion-position accuracy on the homophone sets, and the dev/test comparison of the original and continued v3.

    zt train homophone evaluate                     # dev, original v3 (baseline) and the continuation (chosen)
    zt train homophone evaluate-test                # test sets, once, after the dev decision
    python -m nlp.training.homophone.evaluate sets --set data/build/homophone2/dev_zai.txt --context line
    python -m nlp.training.homophone.evaluate compare --phase dev

`sets` types every clause of a homophone set (`context<TAB>text`, with positions from the .jsonl beside it, or found
with the lexicon) through zhuyin-cli, key by key, with the set group's shared reading at each confusion position, and
scores those positions after an edit-distance alignment (a deletion is wrong); whole-clause accuracy needs an exact
match.  Positions are counted overall, per character, group, category and group × category; `interesting` leaves
out lexicon words.  The *_zai sets and smoke score 在／再, the *_pairs sets the other groups.

`compare` runs, for the original v3 (v3_030) and the continuation (v3_homophone2), with the app configuration and
the sentence weight of the 0.4.0 comparison (0.5): the homophone sets with and without context, and on dev also the
general sets (conversion: colloquial, chat, smoke; Wikipedia with line context; 400 runs) and the 善解 rows.  Reports
go to homophone2/reports/<role>-<name>.json; `zt eval stats` pairs two of them.  The test phase needs the dev
decision (homophone2/dev-decision.json) and runs once.
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from nlp.common.engine import EnginePool
from nlp.common.results import Section, percent
from nlp.common.sets import EvalSet
from nlp.common.zhuyin import Lexicon, keys
from nlp.eval.base import Evaluator
from nlp.training.v3.readings import Lexicon as ReadingLexicon

from .common import BASE, DATA, MODEL, RUN, jsonl, positions

TONE_MARKS = {1: "", 2: "ˊ", 3: "ˇ", 4: "ˋ", 5: "˙"}
# The sentence weight the 0.4.0 comparison fixed (the reranker's config.json says 1.0); everything else is the app's.
COMPARISON = ["--rerank-weight", "0.5"]


def align(want: str, got: str) -> dict[int, str]:
    """For each character of `want`, what `got` has there after a minimal edit alignment ('' when deleted)."""
    costs = [[0] * (len(got) + 1) for _ in range(len(want) + 1)]
    for i in range(len(want) + 1):
        costs[i][0] = i
    for j in range(len(got) + 1):
        costs[0][j] = j
    for i, a in enumerate(want, 1):
        for j, b in enumerate(got, 1):
            costs[i][j] = min(costs[i - 1][j - 1] + (a != b), costs[i - 1][j] + 1, costs[i][j - 1] + 1)
    mapping, i, j = {}, len(want), len(got)
    while i or j:
        if i and j and costs[i][j] == costs[i - 1][j - 1] + (want[i - 1] != got[j - 1]):
            mapping[i - 1] = got[j - 1]
            i -= 1
            j -= 1
        elif i and costs[i][j] == costs[i - 1][j] + 1:
            mapping[i - 1] = ""
            i -= 1
        else:
            j -= 1
    return mapping


def metrics(rows: Sequence[dict[str, Any]], predictions: Sequence[str]) -> dict[str, Any]:
    groups: defaultdict[str, list[int]] = defaultdict(lambda: [0, 0])
    errors = []
    for row, got in zip(rows, predictions, strict=True):
        text = row["text"]
        mapping = align(text, got)
        for p in row["positions"]:
            hit = mapping[p["index"]] == p["char"]
            names = [
                "all",
                "char:" + p["char"],
                "group:" + p["group"],
                "category:" + p["category"],
                "group_category:" + p["group"] + ":" + p["category"],
            ]
            if p["category"] != "fixed":
                names.append("interesting")
                names.append("group_interesting:" + p["group"])
            for name in names:
                groups[name][0] += hit
                groups[name][1] += 1
        if got != text:
            errors.append(dict(text=text, got=got, context=row["context"]))
    return dict(
        sentences=len(rows),
        whole_correct=len(rows) - len(errors),
        whole_accuracy=(len(rows) - len(errors)) / max(1, len(rows)),
        positions={k: dict(correct=c, total=n, accuracy=c / n) for k, (c, n) in sorted(groups.items())},
        errors=errors,
    )


def load_rows(path: Path, lex: ReadingLexicon, inventory: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """A set's clauses with their confusion positions, restricted to the groups the set is about."""
    metadata = path.with_suffix(".jsonl")
    if metadata.exists():
        rows = list(jsonl(metadata))
    else:
        rows = []
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            context, text = fields if len(fields) == 2 else ("", fields[0])
            rows.append(dict(text=text, context=context, positions=positions(text, lex, inventory)))
    for row in rows:
        if path.stem.endswith("_zai") or path.stem == "smoke":
            row["positions"] = [p for p in row["positions"] if p["group"] == "在再"]
        elif path.stem.endswith("_pairs"):
            row["positions"] = [p for p in row["positions"] if p["group"] != "在再"]
    return rows


class Homophone(Evaluator):
    name = "homophone"
    help = "confusion-position accuracy on the homophone sets"
    default_sets = [str(RUN / "dev_zai.txt"), str(RUN / "dev_pairs.txt"), str(RUN / "smoke.txt")]
    default_n = 100000

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument(
            "--context",
            choices=["none", "line"],
            default="none",
            help="line: type each clause after its context (default: none)",
        )

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        path = eval_set.file
        lexicon = Lexicon(pool.config.lexicon)
        lex = ReadingLexicon(pool.config.lexicon)
        inventory_path = path.parent / "inventory.json"
        inventory = json.loads((inventory_path if inventory_path.exists() else DATA / "inventory.json").read_text())
        rows = load_rows(path, lex, inventory["kept"])[: args.n]
        scripts = []
        for row in rows:
            readings = lexicon.readings(row["text"])
            if readings is None or len(readings) != len(row["text"]):
                raise ValueError(f"Unaligned or untypable evaluation text: {row['text']}")
            # Type each confusion with the group's shared toned reading (including neutral 的／得).
            for p in row["positions"]:
                base, tone = p["reading"]
                readings[p["index"]] = base + TONE_MARKS[tone]
            prefix = row["context"].replace("\n", " ").replace("\t", " ") + "\t" if args.context == "line" else ""
            scripts.append(prefix + keys(readings) + "<enter>")
        outputs = pool.run(scripts)
        result = metrics(rows, outputs)
        items = []
        for i, (row, got) in enumerate(zip(rows, outputs)):
            mapping = align(row["text"], got)
            items.append(
                dict(
                    id=i,
                    reference=row["text"],
                    output=got,
                    context=row["context"],
                    cluster=f"{row.get('source', 'row')}:{row.get('line', i)}",
                    positions=[
                        dict(
                            index=p["index"],
                            char=p["char"],
                            group=p["group"],
                            category=p["category"],
                            hit=mapping[p["index"]] == p["char"],
                        )
                        for p in row["positions"]
                    ],
                )
            )
        errors = result.pop("errors")
        return Section(eval_set.name, result, items=items, examples=[(e["text"], e["got"]) for e in errors][:8])

    def describe(self, m: dict[str, Any]) -> str:
        positions = m["positions"].get("all", dict(correct=0, total=0, accuracy=0.0))
        return (
            f"{m['sentences']} clauses: positions {positions['correct']}/{positions['total']} "
            f"({percent(positions['accuracy'])}), whole {percent(m['whole_accuracy'])}"
        )


def run(evaluator: Evaluator, arguments: list[str], quiet: bool = False) -> None:
    parser = argparse.ArgumentParser(prog=evaluator.name)
    evaluator.configure(parser)
    evaluator.run(parser.parse_args(arguments), quiet)


def compare(phase: str, roles: list[str], jobs: int | None) -> None:
    reports = RUN / "reports"
    if phase == "test":
        assert (RUN / "dev-decision.json").exists(), "Decide on dev (homophone2/dev-decision.json) before test."
    for role in roles:
        model = BASE if role == "baseline" else MODEL
        shared = ["--config", "app", "--cli-args", " ".join([*COMPARISON, "--v3", str(model)])]
        shared += ["--jobs", str(jobs)] if jobs else []
        for stem in ["dev_zai", "dev_pairs", "smoke"] if phase == "dev" else ["test_zai", "test_pairs"]:
            for context in ["none"] if stem == "smoke" else ["none", "line"]:
                target = reports / f"{role}-{stem}-ctx{int(context == 'line')}.json"
                assert phase == "dev" or not target.exists(), f"Test already scored: {target}"
                run(
                    Homophone(),
                    [*shared, "--set", str(RUN / f"{stem}.txt"), "--context", context, "--json", str(target)],
                )
        if phase == "dev":
            from nlp.eval import registry

            conversion = registry.get("conversion")
            run(
                conversion,
                [
                    *shared,
                    "--set",
                    "colloquial-dev",
                    "chat-dev",
                    "smoke",
                    "-n",
                    "400",
                    "--json",
                    str(reports / f"{role}-general.json"),
                ],
            )
            run(
                conversion,
                [
                    *shared,
                    "--set",
                    "wiki-dev",
                    "--context",
                    "line",
                    "-n",
                    "400",
                    "--json",
                    str(reports / f"{role}-wikipedia.json"),
                ],
            )
            run(
                registry.get("rows"),
                [*shared, "--set", "shanjie-dev302", "shanjie-typing76", "--json", str(reports / f"{role}-rows.json")],
            )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    Homophone().configure(sub.add_parser("sets", help="score homophone sets with one configuration"))
    comparison = sub.add_parser("compare", help="original v3 against the continuation, app configuration")
    comparison.add_argument("--phase", choices=["dev", "test"], default="dev")
    comparison.add_argument(
        "--role", choices=["baseline", "chosen"], action="append", help="only this side (default: both)"
    )
    comparison.add_argument("--jobs", type=int, help="parallel engine processes (default: CPU count)")
    args = parser.parse_args(argv)
    if args.command == "sets":
        Homophone().run(args)
    else:
        compare(args.phase, args.role or ["baseline", "chosen"], args.jobs)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

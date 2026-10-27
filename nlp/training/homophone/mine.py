"""Count the seed confusion groups in the corpus and reservoir-sample source-attributed candidate clauses.

    zt train homophone inventory mine
    python -m nlp.training.homophone.mine                     # full counts: inventory.json (and candidates)
    python -m nlp.training.homophone.mine --reuse-inventory   # the sample used: every 20th wiki line + Taiwan Tongues

Sources: the v3 Wikipedia and colloquial text (v3_030/wiki.txt, colloquial.txt; counts only for colloquial) and the
Taiwan Tongues zh-TW train.tsv (homophone/inputs/train.tsv; its clauses are used only for evaluation sets).  A group
is kept when every character occurs at least 2,000 times as text and as a standalone or word-initial word.  Keeps
whole 4–31-character Han clauses not touching Latin letters or digits, with up to 32 characters of preceding text,
reservoir-sampled per (group, character, category, source): 800 for 在再, 160 otherwise.  Writes
homophone/{inventory.json, candidates.jsonl, mining.json}.
"""

from __future__ import annotations

import argparse
import csv
import json
import random
import re
import time
from collections import Counter, defaultdict
from collections.abc import Iterator
from typing import Any

from nlp.training.v3.readings import Lexicon

from .common import BASE, DATA, SEEDS, digest, holdouts, positions, save


def sources() -> Iterator[tuple[str, int, str, str]]:
    """(source, line number, text, previous wiki line) for every line of every source."""
    for name in ["wiki", "colloquial"]:
        previous = ""
        with (BASE / f"{name}.txt").open() as f:
            for number, line in enumerate(f, 1):
                text = line.strip()
                yield name, number, text, previous if name == "wiki" else ""
                previous = text if text else ""
    seen = set()
    with (DATA / "inputs/train.tsv").open(newline="") as f:
        for number, row in enumerate(csv.DictReader(f, delimiter="\t"), 2):
            if row.get("locale") != "zh-TW":
                continue
            text = " ".join(row["sentence"].split())
            if text in seen or len(re.findall("[一-鿿]", text)) < 2 or re.search("[A-Za-z]{4}", text):
                continue
            seen.add(text)
            yield "taiwan_tongues_train", number, text, ""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--reuse-inventory",
        action="store_true",
        help="Reuse completed corpus counts; sample every twentieth wiki line and all Taiwan Tongues rows.",
    )
    args = parser.parse_args(argv)
    lex = Lexicon(DATA / "lexicon.tsv")
    inventory: list[dict[str, Any]] = []
    for chars in SEEDS:
        readings = set.intersection(*[{r for _, r in lex.single[c]} for c in chars])
        if readings:
            inventory.append(dict(chars=chars, readings=sorted(readings)))
    frozen: dict[str, Any] | None = None
    if args.reuse_inventory:
        frozen = json.loads((DATA / "inventory.json").read_text())
        assert frozen["lexicon_sha256"] == digest(DATA / "lexicon.tsv")
        inventory = frozen["kept"]
    target = set("".join(g["chars"] for g in inventory))
    held = holdouts().compile()
    counts: Counter[str] = Counter()
    initial: Counter[str] = Counter()
    source_counts: Counter[str] = Counter()
    pools: defaultdict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    seen_counts: Counter[tuple[str, str, str, str]] = Counter()
    rng = random.Random(20261027)
    start = time.monotonic()
    for source, number, text, previous in sources():
        source_counts[source] += 1
        if frozen and (source == "colloquial" or (source == "wiki" and number % 20)):
            continue
        if not text or text in held:
            continue
        hits = target.intersection(text)
        if not hits:
            continue
        if not frozen:
            for c in hits:
                counts[c] += text.count(c)
            for _, _, word, _ in lex.segments(text):
                if word[0] in target:
                    initial[word[0]] += 1
        if source == "colloquial":
            continue
        for match in re.finditer("[一-鿿]+", text):
            sentence = match.group()
            if not 4 <= len(sentence) < 32 or not target.intersection(sentence) or sentence in held:
                continue
            if (match.start() and text[match.start() - 1].isalnum()) or (
                match.end() < len(text) and text[match.end()].isalnum()
            ):
                continue
            context = (previous + "\n" + text[: match.start()])[-32:].strip()
            if context in held or (context + sentence) in held:
                context = ""
            pos = positions(sentence, lex, inventory)
            if not pos:
                continue
            row: dict[str, Any] = dict(
                text=sentence, context=context, source=source, line=number, offset=match.start(), positions=pos
            )
            for key in {(p["group"], p["char"], p["category"], source) for p in pos}:
                seen_counts[key] += 1
                pool = pools[key]
                if len(pool) < (800 if key[0] == "在再" else 160):
                    pool.append(row)
                else:
                    j = rng.randrange(seen_counts[key])
                    if j < len(pool):
                        pool[j] = row
        if source_counts[source] % 1000000 == 0:
            print(source, source_counts[source], round(time.monotonic() - start, 1), flush=True)
    rejected: list[dict[str, Any]] = []
    kept: list[dict[str, Any]] = []
    for group in inventory:
        if not frozen:
            group.update(
                counts={c: counts[c] for c in group["chars"]},
                standalone_or_initial={c: initial[c] for c in group["chars"]},
            )
        (
            kept
            if min(group["counts"].values()) >= 2000 and min(group["standalone_or_initial"].values()) >= 2000
            else rejected
        ).append(group)
    rejected += [
        dict(chars=c, reason="no common toned standalone lexicon reading")
        for c in SEEDS
        if c not in {g["chars"] for g in inventory}
    ]
    if not frozen:
        save(
            DATA / "inventory.json",
            dict(
                kept=kept,
                rejected=rejected,
                sources=source_counts,
                seconds=time.monotonic() - start,
                lexicon_sha256=digest(DATA / "lexicon.tsv"),
                counts_protocol=(
                    "filtered text occurrences; longest-match standalone/word-initial counts; sources once each"
                ),
            ),
        )
    allowed = {g["chars"] for g in kept}
    unique: dict[str, dict[str, Any]] = {}
    for key, pool in sorted(pools.items()):
        if key[0] in allowed:
            for row in pool:
                row["positions"] = [p for p in row["positions"] if p["group"] in allowed]
                unique.setdefault(row["text"], row)
    with (DATA / "candidates.jsonl").open("w") as f:
        for text in sorted(unique):
            f.write(json.dumps(unique[text], ensure_ascii=False) + "\n")
    print("done", len(unique), "candidates", len(kept), "groups", flush=True)
    assert unique, "No candidates: inspect reading matching and filters before judging."
    save(
        DATA / "mining.json",
        dict(
            candidates=len(unique),
            seconds=time.monotonic() - start,
            reused_counts=bool(frozen),
            wiki_stride=20 if frozen else 1,
            protocol="Whole 4-31 Han clauses; never truncate long clauses; reservoir by character/category/source.",
        ),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

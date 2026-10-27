"""Leak-free example indexes into the immutable v3 shards: a document goes whole when it touches held-out text.

Used by compatible.py in worker processes: `init(out, held)` once per process, then `filter_shard(row)` per shard
of v3_030/encode.json.  A document (with its context) is rejected when it contains a held-out 8-gram or a short
held-out sentence, as raw text and with characters outside the v3 vocabulary as U+FFFD (how the shards store them);
臺 and 台 count as one.  Writes <out>/filtered/<shard>.normal.npy (allowed example indexes) and one index file per
confusion group and per primary character (primary-在, primary-再).
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import numpy as np

from .common import BASE, Exclusions

OUT: Path
VOCAB: list[str]
HELD: Exclusions
GROUPS: list[dict[str, Any]]
CHAR_GROUPS: dict[int, list[str]]


def init(out: Path, held: Exclusions, base: Path = BASE) -> None:
    global OUT, VOCAB, HELD, GROUPS, CHAR_GROUPS, BASE
    vocab = json.loads((base / "chars.json").read_text())
    known = set(vocab[4:]) | {"\n"}
    for text in list(held.exact):
        held.add("".join(c if c in known else "�" for c in text))
    held.compile()
    GROUPS = json.loads((out / "inventory.json").read_text())["kept"]
    CHAR_GROUPS = {i: [g["chars"] for g in GROUPS if c in g["chars"]] for i, c in enumerate(vocab)}
    # The held-out sets and the corpus both write 臺 and 台.
    VOCAB = [c.replace("臺", "台") for c in vocab]
    OUT, HELD, BASE = out, held, base


def filter_shard(row: dict[str, Any]) -> dict[str, Any]:
    name = row["name"]
    base = BASE / "encoded" / name
    ids = np.memmap(str(base) + ".ids", np.uint16, "r")
    documents = np.load(str(base) + ".documents.npy")
    examples = np.load(str(base) + ".examples.npy")
    bad = []
    for start, end in documents:
        text = "".join(VOCAB[n] if n >= 4 else "\n" if n == 3 else "�" if n == 2 else "" for n in ids[start:end])
        if text in HELD:
            bad.append(start)
    allowed = ~np.isin(examples[:, 2], bad)
    indexes = np.flatnonzero(allowed)
    np.save(OUT / "filtered" / f"{name}.normal.npy", indexes)
    groups: dict[str, list[int]] = {g["chars"]: [] for g in GROUPS}
    groups.update({"primary-在": [], "primary-再": []})
    for index in indexes:
        start, end, _ = examples[index]
        chars = {VOCAB[n] for n in np.unique(ids[start:end])}
        found = {g for n in np.unique(ids[start:end]) for g in CHAR_GROUPS[int(n)]}
        found.update("primary-" + c for c in chars.intersection("在再"))
        for group in found:
            groups[group].append(index)
    counts = {}
    for group, indices in groups.items():
        np.save(OUT / "filtered" / f"{name}.{group}.npy", np.asarray(indices, np.int64))
        counts[group] = len(indices)
    print(name, len(indexes), "rejected", int((~allowed).sum()), flush=True)
    return dict(name=name, allowed=len(indexes), rejected=int((~allowed).sum()), groups=counts)

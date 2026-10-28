"""Training batches for v3: mmap shard samplers, and the candidate builder shared by training and the dev checks.

`Builder.example` turns a text, its readings and its context into model inputs: typed readings with noise
(zhuyintype_ml/training/v3/noise.py), and for each position the candidate pool of the typed reading and its three
nearest neighbours, always including the right character (the full union, which may exceed the exported limit of 64).
`Batches("context" | "syllable", batch)` streams batches from v3_030/encoded, colloquial shards weighted twice.

    from zhuyintype_ml.training.v3.dataset import Batches, Builder, collate
"""

from __future__ import annotations

import json
import random
from collections.abc import Iterator
from functools import lru_cache
from typing import TYPE_CHECKING, Any

import numpy as np
import torch
from torch.utils.data import IterableDataset

from .common import OUT
from .noise import Noise
from .readings import Lexicon, Reading

if TYPE_CHECKING:
    from .noise import ReadingChannel


class Shard:
    def __init__(self, name: str) -> None:
        p = OUT / "encoded" / name
        self.ids = np.memmap(str(p) + ".ids", dtype=np.uint16, mode="r")
        self.documents = np.load(str(p) + ".documents.npy")
        self.examples = np.load(str(p) + ".examples.npy")
        self.syllables = np.memmap(str(p) + ".syllables", dtype=np.uint16, mode="r")
        self.tones = np.memmap(str(p) + ".tones", dtype=np.uint8, mode="r")


class Builder:
    def __init__(self) -> None:
        self.vocab = json.load(open(OUT / "chars.json"))
        self.chars = {c: i for i, c in enumerate(self.vocab)}
        self.chars["\n"] = 3
        self.syllables = json.load(open(OUT / "syllables.json"))
        self.sids = {s: i for i, s in enumerate(self.syllables)}
        self.lex = Lexicon(OUT / "lexicon.tsv")
        self.noise = Noise(self.lex.candidates)
        # Readings typed another way (the reading channel), when a training run sets it.
        self.reading: ReadingChannel | None = None
        self.pool = {
            r: [self.chars.get(c, 2) for c in sorted(rows, key=lambda c: (-rows[c], c))]
            for r, rows in self.lex.candidates.items()
        }
        priors: dict[Reading, tuple[float, int]] = {}
        for char, rows in self.lex.single.items():
            for prior, reading in rows:
                if reading not in priors or prior > priors[reading][0]:
                    priors[reading] = (prior, self.chars.get(char, 2))
        self.baseline = {r: value[1] for r, value in priors.items()}

    def candidates(self, reading: Reading, truth: int, rng: random.Random) -> tuple[list[int], int]:
        found = set(self.pool.get(reading, []))
        for neighbour in self.noise.neighbours.get(reading, []):
            found.update(self.pool.get(neighbour, []))
        found.add(truth)
        found.discard(0)
        result = sorted(found)
        rng.shuffle(result)
        return result, result.index(truth)

    def example(
        self,
        text: str,
        readings: list[Reading],
        context: str,
        rng: random.Random,
        rate: float | None = None,
        augment: bool = True,
    ) -> dict[str, Any]:
        variants = self.noise.variants(text, readings, self.lex, rng, rate, augment, self.reading)
        candidates: list[list[int]] = []
        labels: list[int] = []
        for char, (reading, _cost, _boundary) in zip(text, variants):
            pool, label = self.candidates(reading, self.chars.get(char, 2), rng)
            candidates.append(pool)
            labels.append(label)
        return {
            "text": text,
            "context": context[-255:],
            "readings": readings,
            "syllable_ids": [self.sids.get(r[0], 1) for r, c, b in variants],
            "tone_ids": [r[1] for r, c, b in variants],
            "cost_bucket": [c for r, c, b in variants],
            "boundary": [b for r, c, b in variants],
            "candidates": candidates,
            "labels": labels,
            "baseline": [self.baseline.get(r, -1) for r, c, b in variants],
            "truth": [self.chars.get(c, 2) for c in text],
        }


def collate(examples: list[dict[str, Any]], builder: Builder, drop_context: bool = False) -> tuple[torch.Tensor, ...]:
    batch = len(examples)
    length = max(len(e["text"]) for e in examples)
    context_length = 1 if drop_context else max(len(e["context"]) + 1 for e in examples)
    count = max(len(pool) for e in examples for pool in e["candidates"])
    inputs = {n: np.zeros((batch, length), np.int64) for n in ["syllable_ids", "tone_ids", "cost_bucket", "boundary"]}
    contexts = np.zeros((batch, context_length), np.int64)
    contexts[:, 0] = 1
    candidates = np.zeros((batch, length, count), np.int64)
    labels = np.full((batch, length), -100, np.int64)
    for i, e in enumerate(examples):
        n = len(e["text"])
        for name in inputs:
            inputs[name][i, :n] = e[name]
        if not drop_context and e["context"]:
            ctx = [builder.chars.get(c, 2) for c in e["context"]]
            contexts[i, 1 : len(ctx) + 1] = ctx
        for j, pool in enumerate(e["candidates"]):
            candidates[i, j, : len(pool)] = pool
        labels[i, :n] = e["labels"]
        # Padded positions still need a nonempty candidate set to avoid undefined softmax.
        candidates[i, n:, 0] = 2
    return tuple(torch.from_numpy(inputs[n]) for n in inputs) + (
        torch.from_numpy(candidates),
        torch.from_numpy(contexts),
        torch.from_numpy(labels),
    )


class Batches(IterableDataset):
    def __init__(self, phase: str, batch: int, seed: int = 31) -> None:
        self.phase, self.batch, self.seed = phase, batch, seed

    def __iter__(self) -> Iterator[tuple[torch.Tensor, ...]]:
        worker = torch.utils.data.get_worker_info()
        rng = random.Random(self.seed + (worker.id if worker else 0))
        manifest = [r for r in json.load(open(OUT / "encode.json"))["shards"] if r["tokens"] > 2]
        weights = [row["tokens"] * (2 if "colloquial" in row["name"] else 1) for row in manifest]
        load = lru_cache(maxsize=8)(Shard)
        builder = Builder() if self.phase == "syllable" else None
        if self.phase == "syllable":
            weights = [
                row["confident_examples"] * (2 if row["name"].startswith("colloquial") else 1) for row in manifest
            ]
        while True:
            s = load(rng.choices(manifest, weights)[0]["name"])
            if self.phase == "context":
                x = np.zeros((self.batch, 257), np.int64)
                for i in range(self.batch):
                    doc = int(np.searchsorted(s.documents[:, 1], rng.randrange(len(s.ids)), side="right"))
                    start, end = s.documents[doc]
                    start = rng.randrange(start + 1, max(start + 2, end - 1))
                    text = s.ids[start : min(end, start + 256)]
                    x[i, 0] = 1
                    x[i, 1 : len(text) + 1] = text
                yield torch.from_numpy(x[:, :-1].copy()), torch.from_numpy(x[:, 1:].copy())
            else:
                assert builder is not None
                examples = []
                for _ in range(self.batch):
                    start, end, doc = s.examples[rng.randrange(len(s.examples))]
                    text = "".join(builder.vocab[n] for n in s.ids[start:end])
                    readings = [
                        (builder.syllables[n], int(t)) for n, t in zip(s.syllables[start:end], s.tones[start:end])
                    ]
                    context = "".join(
                        builder.vocab[n] if n >= 4 else "\n" if n == 3 else ""
                        for n in s.ids[max(doc, start - 255) : start]
                    )
                    examples.append(builder.example(text, readings, context, rng))
                yield collate(examples, builder, drop_context=rng.random() < 0.3)

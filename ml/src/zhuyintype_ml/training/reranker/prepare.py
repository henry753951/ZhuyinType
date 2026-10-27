"""Build the character vocabulary and token streams for the reranker language model.

    zt train reranker prepare
    python -m zhuyintype_ml.training.reranker.prepare --workers 60 [--out data/build/reranker]

Training text: Wikipedia sentences (the lm pipeline's deduplicated lm/work/unique.txt), generated Taiwan colloquial
sentences and Common Voice zh-TW sentences (training/corpus).  Every sentence sharing an 8-character substring with a
test or development sentence is dropped, so the evaluation sets stay unseen.  Colloquial text is repeated
`--colloquial-repeat` times.

Writes vocab.json (id → character; 0 = <pad>, 1 = <bos>, 2 = <unk>), train.bin and val_{wiki,colloquial}.bin
(uint16 streams, each sentence preceded by <bos>) and prepare.json.
"""

from __future__ import annotations

import argparse
import json
import multiprocessing as mp
import os
import random
import re
from collections import Counter
from collections.abc import Iterator
from pathlib import Path

import numpy as np

from zhuyintype_ml.core import paths

HAN = re.compile(r"[㐀-鿿]")
# Relative to the data directory.
HELD_OUT = [
    "lm/test_sentences.txt",
    "corpus/test_colloquial.txt",
    "corpus/dev_wikipedia.txt",
    "corpus/dev_colloquial.txt",
]
COLLOQUIAL = ["synthetic_phase1.txt", "synthetic_background.txt", "train_commonvoice.txt"]
GRAMS: set[str] = set()


def grams(text: str) -> set[str]:
    return {text[i : i + 8] for i in range(len(text) - 7)}


def init(held: set[str]) -> None:
    global GRAMS
    GRAMS = held


def clean(line: str) -> str | None:
    """The sentence when it is 4–200 characters, at least half Han, and shares no 8-gram with held-out text."""
    line = line.strip()
    if not 4 <= len(line) <= 200 or len(HAN.findall(line)) < 0.5 * len(line):
        return None
    if any(line[i : i + 8] in GRAMS for i in range(len(line) - 7)):
        return None
    return line


def read(path: Path | str, limit: int | None = None) -> Iterator[str]:
    with open(path, encoding="utf-8", errors="ignore") as f:
        for i, line in enumerate(f):
            if limit and i >= limit:
                break
            yield line


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=paths.data("reranker"))
    parser.add_argument(
        "--wiki",
        type=Path,
        default=paths.data("lm", "work", "unique.txt"),
        help="deduplicated Wikipedia sentences (default: the lm pipeline's lm/work/unique.txt)",
    )
    parser.add_argument("--workers", type=int, default=32)
    parser.add_argument("--vocab", type=int, default=12000)
    parser.add_argument("--colloquial-repeat", type=int, default=2)
    args = parser.parse_args(argv)
    os.makedirs(args.out, exist_ok=True)

    held: set[str] = set()
    for path in HELD_OUT:
        for line in read(paths.data(path)):
            held |= grams(line.strip())
    print(f"held-out 8-grams: {len(held)}", flush=True)

    sources = {"wiki": [args.wiki], "colloquial": [paths.data("corpus", name) for name in COLLOQUIAL]}
    kept: dict[str, list[str]] = {}
    with mp.Pool(args.workers, initializer=init, initargs=(held,)) as pool:
        for name, files in sources.items():
            lines = [line for path in files for line in read(path)]
            kept[name] = [s for s in pool.imap(clean, lines, chunksize=20000) if s]
            print(f"{name}: {len(lines)} lines, kept {len(kept[name])}", flush=True)

    counts: Counter[str] = Counter()
    for sentences in kept.values():
        for sentence in sentences:
            counts.update(sentence)
    vocab = ["<pad>", "<bos>", "<unk>"] + [c for c, _ in counts.most_common(args.vocab - 3)]
    index = {c: i for i, c in enumerate(vocab)}
    coverage = sum(counts[c] for c in vocab[3:]) / max(1, sum(counts.values()))
    print(f"vocab {len(vocab)}, coverage {coverage:.5f}", flush=True)
    with open(os.path.join(args.out, "vocab.json"), "w", encoding="utf-8") as f:
        json.dump(vocab, f, ensure_ascii=False)

    def encode(sentences: list[str]) -> np.ndarray:
        out: list[int] = []
        for s in sentences:
            out.append(1)
            out.extend(index.get(c, 2) for c in s)
        return np.array(out, dtype=np.uint16)

    train = kept["wiki"] + kept["colloquial"] * args.colloquial_repeat
    random.Random(7).shuffle(train)
    encode(train).tofile(os.path.join(args.out, "train.bin"))
    for name, path in [("wiki", "corpus/dev_wikipedia.txt"), ("colloquial", "corpus/dev_colloquial.txt")]:
        encode([line.strip() for line in read(paths.data(path)) if line.strip()]).tofile(
            os.path.join(args.out, f"val_{name}.bin")
        )
    stats: dict[str, object] = {name: len(v) for name, v in kept.items()}
    stats.update(
        vocab=len(vocab),
        coverage=coverage,
        train_tokens=int(os.path.getsize(os.path.join(args.out, "train.bin")) // 2),
    )
    with open(os.path.join(args.out, "prepare.json"), "w") as f:
        json.dump(stats, f, indent=2)
    print(stats, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

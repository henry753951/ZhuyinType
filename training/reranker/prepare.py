#!/usr/bin/env python3
"""Build the character vocabulary and token streams for the reranker language model.

    python prepare.py --root ~/zhuyintype --out ~/zhuyintype/reranker --workers 60

Training text: Wikipedia sentences (work/unique.txt), generated Taiwan colloquial sentences and Common Voice zh-TW
sentences.  Every sentence sharing an 8-character substring with a test or development sentence is dropped, so the
evaluation sets stay unseen.  Colloquial text is repeated `--colloquial-repeat` times.

Writes vocab.json (id → character; 0 = <pad>, 1 = <bos>, 2 = <unk>), train.bin and val_{wiki,colloquial}.bin
(uint16 streams, each sentence preceded by <bos>).
"""
import argparse
import json
import multiprocessing as mp
import os
import random
import re
from collections import Counter

import numpy as np

HAN = re.compile(r"[㐀-鿿]")
HELD_OUT = ["data/build/lm/test_sentences.txt", "data/build/corpus/test_colloquial.txt",
            "data/build/corpus/dev_wikipedia.txt", "data/build/corpus/dev_colloquial.txt"]
GRAMS = None


def grams(text):
    return {text[i:i + 8] for i in range(len(text) - 7)}


def init(held):
    global GRAMS
    GRAMS = held


def clean(line):
    line = line.strip()
    if not 4 <= len(line) <= 200 or len(HAN.findall(line)) < 0.5 * len(line):
        return None
    if any(line[i:i + 8] in GRAMS for i in range(len(line) - 7)):
        return None
    return line


def read(path, limit=None):
    with open(path, encoding="utf-8", errors="ignore") as f:
        for i, line in enumerate(f):
            if limit and i >= limit:
                break
            yield line


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--workers", type=int, default=32)
    parser.add_argument("--vocab", type=int, default=12000)
    parser.add_argument("--colloquial-repeat", type=int, default=2)
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    root = args.root

    held = set()
    for path in HELD_OUT:
        for line in read(os.path.join(root, path)):
            held |= grams(line.strip())
    print(f"held-out 8-grams: {len(held)}", flush=True)

    sources = {
        "wiki": [os.path.join(root, "work/unique.txt")],
        "colloquial": [os.path.join(root, "data/build/corpus", name) for name in
                       ["synthetic_phase1.txt", "synthetic_background.txt", "train_commonvoice.txt"]],
    }
    kept = {}
    with mp.Pool(args.workers, initializer=init, initargs=(held,)) as pool:
        for name, paths in sources.items():
            lines = [line for path in paths for line in read(path)]
            kept[name] = [s for s in pool.imap(clean, lines, chunksize=20000) if s]
            print(f"{name}: {len(lines)} lines, kept {len(kept[name])}", flush=True)

    counts = Counter()
    for name, sentences in kept.items():
        for sentence in sentences:
            counts.update(sentence)
    vocab = ["<pad>", "<bos>", "<unk>"] + [c for c, _ in counts.most_common(args.vocab - 3)]
    index = {c: i for i, c in enumerate(vocab)}
    coverage = sum(counts[c] for c in vocab[3:]) / max(1, sum(counts.values()))
    print(f"vocab {len(vocab)}, coverage {coverage:.5f}", flush=True)
    with open(os.path.join(args.out, "vocab.json"), "w", encoding="utf-8") as f:
        json.dump(vocab, f, ensure_ascii=False)

    def encode(sentences):
        out = []
        for s in sentences:
            out.append(1)
            out.extend(index.get(c, 2) for c in s)
        return np.array(out, dtype=np.uint16)

    train = kept["wiki"] + kept["colloquial"] * args.colloquial_repeat
    random.Random(7).shuffle(train)
    encode(train).tofile(os.path.join(args.out, "train.bin"))
    for name, path in [("wiki", "data/build/corpus/dev_wikipedia.txt"), ("colloquial", "data/build/corpus/dev_colloquial.txt")]:
        encode([l.strip() for l in read(os.path.join(root, path)) if l.strip()]).tofile(
            os.path.join(args.out, f"val_{name}.bin"))
    stats = {name: len(v) for name, v in kept.items()}
    stats.update(vocab=len(vocab), coverage=coverage, train_tokens=int(os.path.getsize(os.path.join(args.out, "train.bin")) // 2))
    with open(os.path.join(args.out, "prepare.json"), "w") as f:
        json.dump(stats, f, indent=2)
    print(stats, flush=True)


if __name__ == "__main__":
    main()

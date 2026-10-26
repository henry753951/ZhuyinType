"""Longest lexicon match and a small context-window polyphone classifier."""
import argparse
from collections import defaultdict, Counter
import json
import math
import random
import re
import time

import numpy as np
import torch
from torch import nn
from torch.nn import functional as F

from common import OUT, save

TONES = {"ˊ": 2, "ˇ": 3, "ˋ": 4, "˙": 5}


def split_reading(text):
    tone = next((v for k, v in TONES.items() if k in text), 1)
    return re.sub("[ˊˇˋ˙]", "", text), tone


class Lexicon:
    def __init__(self, path):
        self.words = defaultdict(list)
        self.single = defaultdict(list)
        self.candidates = defaultdict(dict)
        self.trie = {}
        for line in open(path):
            if line.startswith("#"):
                continue
            raw, word, prior = line.rstrip().split("\t")
            reading = tuple(map(split_reading, raw.split()))
            if len(reading) != len(word):
                continue
            prior = float(prior)
            self.words[word].append((prior, reading))
            if len(word) == 1:
                self.single[word].append((prior, reading[0]))
            for char, item in zip(word, reading):
                self.candidates[item][char] = max(prior, self.candidates[item].get(char, -math.inf))
        for word, rows in self.words.items():
            rows.sort(reverse=True)
            node = self.trie
            for char in word:
                node = node.setdefault(char, {})
            node[None] = word
        for rows in self.single.values():
            rows.sort(reverse=True)
        self.poly = {c for c, rows in self.single.items() if len({r for _, r in rows}) > 1}

    def segments(self, text):
        i = 0
        while i < len(text):
            node, found = self.trie, None
            j = i
            while j < len(text) and text[j] in node:
                node = node[text[j]]
                j += 1
                if None in node:
                    found = (j, node[None])
            if found:
                j, word = found
                yield i, j, word, self.words[word][0][1]
                i = j
            else:
                yield i, i + 1, text[i], (("<literal>", 0),)
                i += 1

    def label(self, text, classifier=None, chars=None, labels=None):
        readings, low = [], False
        for start, end, word, reading in self.segments(text):
            if len(word) == 1 and word in self.poly:
                chosen = reading[0]
                if classifier is not None:
                    ids = window(text, start, chars)
                    with torch.no_grad():
                        logits = classifier(torch.tensor([ids]))[0]
                        allowed = [labels.index(r) for _, r in self.single[word] if r in labels]
                        probs = logits[sorted(set(allowed))].softmax(0)
                        p, index = probs.max(0)
                    if p.item() >= .9:
                        chosen = tuple(labels[sorted(set(allowed))[index.item()]])
                    else:
                        low = True
                else:
                    low = True
                readings.append(chosen)
            else:
                readings.extend(reading)
        assert len(readings) == len(text)
        return readings, low


def window(text, position, chars):
    return [chars.get(text[i], 2) if 0 <= i < len(text) else 0 for i in range(position - 4, position + 5)]


class Polyphone(nn.Module):
    def __init__(self, vocab, labels):
        super().__init__()
        self.embed = nn.Embedding(vocab, 48, padding_idx=0)
        self.net = nn.Sequential(nn.Linear(9 * 48, 192), nn.GELU(), nn.Linear(192, labels))

    def forward(self, ids):
        return self.net(self.embed(ids).flatten(1))


def load_poly():
    ckpt = torch.load(OUT / "polyphone.pt", map_location="cpu", weights_only=False)
    model = Polyphone(ckpt["vocab"], len(ckpt["labels"]))
    model.load_state_dict(ckpt["model"])
    model.eval()
    return model, ckpt["labels"]


def train(limit=1500000):
    torch.set_num_threads(8)
    torch.manual_seed(19)
    rng = random.Random(19)
    lex = Lexicon(OUT / "lexicon.tsv")
    chars = {c: i for i, c in enumerate(json.load(open(OUT / "chars.json")))}
    labels = sorted({r for c in lex.poly for _, r in lex.single[c]})
    label_ids = {r: i for i, r in enumerate(labels)}
    xs, ys = [], []
    counts = Counter()
    start_time = time.monotonic()
    # Supervision is only from words with one distinct reading (ignoring 一/不 sandhi).
    for name in ["colloquial.txt", "wiki.txt"]:
        for line in (OUT / name).open():
            text = line.strip()
            for start, end, word, reading in lex.segments(text):
                if len(word) < 2 or len({r for _, r in lex.words[word]}) != 1:
                    continue
                for offset, (char, r) in enumerate(zip(word, reading)):
                    if char in lex.poly and r in label_ids and counts[(char, r)] < 15000:
                        xs.append(window(text, start + offset, chars))
                        ys.append(label_ids[r])
                        counts[(char, r)] += 1
            if len(xs) >= limit:
                break
        if len(xs) >= limit:
            break
    assert len(xs) > 1000, "insufficient word-supervised polyphone labels"
    order = list(range(len(xs)))
    rng.shuffle(order)
    nval = max(1, len(order) // 10)
    x = torch.tensor(xs, device="cuda")
    y = torch.tensor(ys, device="cuda")
    model = Polyphone(len(chars), len(labels)).cuda()
    optimizer = torch.optim.AdamW(model.parameters(), lr=.001)
    train_ids = torch.tensor(order[nval:], device="cuda")
    val_ids = torch.tensor(order[:nval], device="cuda")
    history = []
    for epoch in range(8):
        model.train()
        for ids in train_ids[torch.randperm(len(train_ids), device="cuda")].split(2048):
            loss = F.cross_entropy(model(x[ids]), y[ids])
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            optimizer.step()
        model.eval()
        with torch.no_grad():
            logits = model(x[val_ids])
            probs = logits.softmax(-1)
            p, pred = probs.max(-1)
            row = {"epoch": epoch + 1, "val_accuracy": (pred == y[val_ids]).float().mean().item(),
                   "confident_fraction": (p >= .9).float().mean().item(),
                   "confident_accuracy": (pred[p >= .9] == y[val_ids][p >= .9]).float().mean().item()}
        history.append(row)
        print(row, flush=True)
    torch.save({"model": model.cpu().state_dict(), "vocab": len(chars), "labels": labels}, OUT / "polyphone.pt")
    save(OUT / "polyphone.json", {"examples": len(xs), "reading_classes": len(labels), "supervised_pairs": len(counts),
         "seconds": time.monotonic() - start_time, "history": history,
         "validation": "seeded 10% of training-only word-supervised examples; not independent human annotation"})


if __name__ == "__main__":
    train()

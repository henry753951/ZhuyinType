"""The polyphone classifier: picks the reading of a polyphonic single character from a 9-character window.

Supervision comes only from words with one distinct reading (each of their characters is labelled by the word), so
no hand labels are needed.  A 48-dimensional embedding and a 192-unit MLP, trained on the GPU for 8 epochs; the
encode step keeps a prediction only at p >= 0.9.  Writes polyphone.pt and polyphone.json to v3_030/.

    zt train v3 polyphone
    python -m nlp.training.v3.polyphone
"""

from __future__ import annotations

import json
import random
import time
from collections import Counter

import torch
from torch import nn
from torch.nn import functional as F

from .common import OUT, save
from .readings import Lexicon, Reading, window


class Polyphone(nn.Module):
    def __init__(self, vocab: int, labels: int) -> None:
        super().__init__()
        self.embed = nn.Embedding(vocab, 48, padding_idx=0)
        self.net = nn.Sequential(nn.Linear(9 * 48, 192), nn.GELU(), nn.Linear(192, labels))

    def forward(self, ids: torch.Tensor) -> torch.Tensor:
        return self.net(self.embed(ids).flatten(1))


def load_poly() -> tuple[Polyphone, list[Reading]]:
    """The trained classifier (CPU, eval mode) and its reading labels."""
    ckpt = torch.load(OUT / "polyphone.pt", map_location="cpu", weights_only=False)
    model = Polyphone(ckpt["vocab"], len(ckpt["labels"]))
    model.load_state_dict(ckpt["model"])
    model.eval()
    return model, ckpt["labels"]


def train(limit: int = 1500000) -> None:
    torch.set_num_threads(8)
    torch.manual_seed(19)
    rng = random.Random(19)
    lex = Lexicon(OUT / "lexicon.tsv")
    chars = {c: i for i, c in enumerate(json.load(open(OUT / "chars.json")))}
    labels = sorted({r for c in lex.poly for _, r in lex.single[c]})
    label_ids = {r: i for i, r in enumerate(labels)}
    xs: list[list[int]] = []
    ys: list[int] = []
    counts: Counter[tuple[str, Reading]] = Counter()
    start_time = time.monotonic()
    # Supervision is only from words with one distinct reading (ignoring 一/不 sandhi).
    for name in ["colloquial.txt", "wiki.txt"]:
        for line in (OUT / name).open():
            text = line.strip()
            for start, _end, word, reading in lex.segments(text):
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
    optimizer = torch.optim.AdamW(model.parameters(), lr=0.001)
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
            row = {
                "epoch": epoch + 1,
                "val_accuracy": (pred == y[val_ids]).float().mean().item(),
                "confident_fraction": (p >= 0.9).float().mean().item(),
                "confident_accuracy": (pred[p >= 0.9] == y[val_ids][p >= 0.9]).float().mean().item(),
            }
        history.append(row)
        print(row, flush=True)
    torch.save({"model": model.cpu().state_dict(), "vocab": len(chars), "labels": labels}, OUT / "polyphone.pt")
    save(
        OUT / "polyphone.json",
        {
            "examples": len(xs),
            "reading_classes": len(labels),
            "supervised_pairs": len(counts),
            "seconds": time.monotonic() - start_time,
            "history": history,
            "validation": "seeded 10% of training-only word-supervised examples; not independent human annotation",
        },
    )


def main(argv: list[str] | None = None) -> int:
    import argparse

    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    train()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

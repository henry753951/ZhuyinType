"""Invariant checks on the prepared v3 data and the model code, run once before training.

    zt train v3 selfcheck
    python -m zhuyintype_ml.training.v3.selfcheck

Needs the snapshot lexicon, the held-out sets, chars.json/syllables.json and polyphone.pt in v3_030/: lexicon
alignment, valid noisy syllables, held-out matching, the UNK candidate bucket, the polyphone labels, and the
exported interfaces (causal prefixes, padding, normalisation, tracing with shorter dynamic shapes).  The model-only
part also runs as a unit test (test_v3.py).
"""

from __future__ import annotations

import random

import torch

from .common import OUT, excluded, holdouts
from .dataset import Builder
from .models import ContextEncoder, ContextExport, SyllableEncoder, SyllableExport
from .noise import KEYS, Noise
from .polyphone import load_poly
from .readings import Lexicon, split_reading


def main(argv: list[str] | None = None) -> int:
    import argparse

    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    torch.set_num_threads(2)
    assert len(KEYS) == 37 and len(set(KEYS.values())) == 37
    assert split_reading("ㄓㄨㄥ") == ("ㄓㄨㄥ", 1)
    assert split_reading("ㄉㄜ˙") == ("ㄉㄜ", 5)
    lex = Lexicon(OUT / "lexicon.tsv")
    text = "今天不要一直工作"
    readings, low = lex.label(text)
    assert len(readings) == len(text)
    noise = Noise(lex.candidates)
    rng = random.Random(7)
    changed = 0
    for reading in list(noise.encode)[:1000]:
        for _ in range(10):
            typed, cost = noise.corrupt(reading, rng)
            assert typed in noise.encode and 0 <= cost <= 3
            changed += typed != reading
    assert changed > 0
    held = holdouts()
    assert excluded(next(iter(held[0])), held)
    builder = Builder()
    unseen_reading = next(r for r, rows in lex.candidates.items() if any(c not in builder.chars for c in rows))
    pool, label = builder.candidates(unseen_reading, 4, rng)
    assert 2 in pool and pool[label] == 4
    poly, labels = load_poly()
    labelled, low = lex.label("和", poly, builder.chars, labels)
    assert len(labelled) == 1 and labelled[0] in {r for _, r in lex.single["和"]}
    context = ContextEncoder(40).eval()
    syllable = SyllableEncoder(40, 20).eval()
    syllable.initialize(context)
    with torch.no_grad():
        ids = torch.tensor([[1, 4, 5]])
        h = context.hidden(ids)
        other = context.hidden(torch.tensor([[1, 4, 9]]))
        torch.testing.assert_close(h[:, :2], other[:, :2])
        k, v = ContextExport(context, syllable)(ids)
        assert k.shape == (6, 3, 512) and k.dtype == torch.float16
        x = torch.ones(1, 2, dtype=torch.int64)
        candidates = torch.tensor([[[4, 5, 0], [6, 7, 8]]])
        wrapper = SyllableExport(syllable)
        args = (x, x, x * 0, x * 0, candidates, k, v)
        y = wrapper(*args)
        assert y.shape == (1, 2, 3) and y[0, 0, 2] < -9990
        torch.testing.assert_close(y.exp().sum(-1), torch.ones(1, 2))
        padded = tuple(torch.cat((t, torch.zeros_like(t[:, :1])), 1) for t in args[:4])
        pad_candidates = torch.cat((candidates, torch.tensor([[[2, 0, 0]]])), 1)
        torch.testing.assert_close(wrapper(*padded, pad_candidates, k, v)[:, :2], y, atol=1e-5, rtol=1e-5)
        pk, pv = ContextExport(context, syllable)(torch.tensor([[1, 4, 5, 0, 0]]))
        torch.testing.assert_close(wrapper(*args[:5], pk, pv), y, atol=1e-5, rtol=1e-5)
        traced = torch.jit.trace(wrapper, args, check_trace=False)
        torch.testing.assert_close(y, traced(*args), atol=1e-5, rtol=1e-5)
        # Variable lengths and candidate counts must survive tracing.
        short = (x[:, :1], x[:, :1], x[:, :1] * 0, x[:, :1] * 0, candidates[:, :1, :2], k[:, :1], v[:, :1])
        torch.testing.assert_close(wrapper(*short), traced(*short), atol=1e-5, rtol=1e-5)
    print("v3 invariant self-check passed; valid noisy syllables", changed)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

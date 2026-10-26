"""Small invariant checks for leakage, lexicon alignment, noise and export semantics."""
import random
import tempfile
from pathlib import Path

import torch
from common import OUT, holdouts, excluded
from readings import Lexicon, split_reading, load_poly
from noise import Noise, KEYS
from models import ContextEncoder, SyllableEncoder, ContextExport, SyllableExport
from dataset import Builder


def main():
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
    reading, low = lex.label("和", poly, builder.chars, labels)
    assert len(reading) == 1 and reading[0] in {r for _, r in lex.single["和"]}
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
        short = (x[:, :1], x[:, :1], x[:, :1] * 0, x[:, :1] * 0,
                 candidates[:, :1, :2], k[:, :1], v[:, :1])
        torch.testing.assert_close(wrapper(*short), traced(*short), atol=1e-5, rtol=1e-5)
    print("v3 invariant self-check passed; valid noisy syllables", changed)


if __name__ == "__main__":
    main()

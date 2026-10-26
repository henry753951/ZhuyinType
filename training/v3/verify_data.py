"""Full leakage audit and encoded-reading spot checks before training."""
from collections import Counter
import json
import random
import time

import numpy as np

from common import OUT, excluded, holdouts, save
from readings import Lexicon


def main():
    held = holdouts()
    stats = {}
    start = time.monotonic()
    for name in ["wiki", "colloquial"]:
        lines = chars = blanks = 0
        for line in (OUT / (name + ".txt")).open():
            text = line.strip()
            if not text:
                blanks += 1
                continue
            assert not excluded(text, held), f"leakage in {name}: {text[:40]}"
            lines += 1; chars += len(text)
        stats[name] = {"nonempty_lines": lines, "characters": chars, "blank_context_resets": blanks}
    vocab = json.load(open(OUT / "chars.json"))
    syllables = json.load(open(OUT / "syllables.json"))
    assert vocab[:4] == ["<pad>", "<bos>", "<unk>", "<sep>"]
    assert syllables[:2] == ["<pad>", "<literal>"]
    assert len(vocab) == len(set(vocab)) and len(syllables) == len(set(syllables))
    lex = Lexicon(OUT / "lexicon.tsv")
    corpus_chars = set(vocab[4:])
    absent = set()
    for name in ["wiki", "colloquial"]:
        for line in (OUT / (name + ".txt")).open():
            absent.update(c for c in line.rstrip("\n") if c not in corpus_chars)
    assert not absent
    rng = random.Random(53)
    manifest = json.load(open(OUT / "encode.json"))["shards"]
    for row in manifest:
        if row["name"].startswith("colloquial"):
            ids = np.memmap(OUT / "encoded" / (row["name"] + ".ids"), np.uint16, mode="r")
            text = "".join(vocab[c] if c >= 4 else "\n" for c in ids)
            assert next(held[1].iter(text), None) is None, "encoded preceding-line boundary leaks a heldout 8-gram"
    stats["encoded_colloquial_cross_boundary_8gram_matches"] = 0
    samples = []
    for row in rng.sample(manifest, min(20, len(manifest))):
        p = OUT / "encoded" / row["name"]
        ids = np.memmap(str(p) + ".ids", np.uint16, mode="r")
        sids = np.memmap(str(p) + ".syllables", np.uint16, mode="r")
        tones = np.memmap(str(p) + ".tones", np.uint8, mode="r")
        examples = np.load(str(p) + ".examples.npy")
        assert len(ids) == len(sids) == len(tones)
        assert int(ids.max()) < len(vocab) and int(sids.max()) < len(syllables) and int(tones.max()) <= 5
        if len(examples):
            a, b, doc = examples[rng.randrange(len(examples))]
            assert 0 <= doc < a < b <= len(ids) and b - a <= 64 and ids[doc] == 1
            text = "".join(vocab[c] for c in ids[a:b])
            assert not excluded(text, held)
            samples.append({"shard": row["name"], "text": text,
                            "readings": [[syllables[s], int(t)] for s, t in zip(sids[a:b], tones[a:b])],
                            "context_tokens": min(255, int(a - doc - 1))})
    stats.update(seconds=time.monotonic() - start, vocabulary=len(vocab), syllables=len(syllables),
                 lexicon_single_characters_in_corpus=len(set(lex.single) & corpus_chars), samples=samples)
    save(OUT / "data-audit.json", stats)
    print(json.dumps(stats, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

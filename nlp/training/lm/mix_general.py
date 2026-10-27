#!/usr/bin/env python3
"""Mix the corpus language model with the lexicon's general word frequencies.

The corpus model knows context but is skewed to its corpus (an encyclopedia makes 獲釋 likelier than 或是); the
lexicon's frequencies come from a broad mix of everyday text and rank common words the way people type them.  The
mixed model is

    P(w | h) = λ · P_corpus(w | h) + (1 − λ) · P_general(w)

applied to every bigram entry and to the unigram the engine backs off to.  Small λ: frequency decides, context nudges.

    python3 nlp/training/lm/mix_general.py SRC_LM_DIR LEXICON OUT_LM_DIR --weight 0.3
"""
import argparse
import math
import os
from collections import defaultdict


def mix(a, b, weight):
    """log(weight·e^a + (1−weight)·e^b), stably."""
    x, y = math.log(weight) + a, math.log(1 - weight) + b
    m = max(x, y)
    return m + math.log(math.exp(x - m) + math.exp(y - m))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("src")
    parser.add_argument("lexicon")
    parser.add_argument("out")
    parser.add_argument("--weight", type=float, default=0.3, help="λ, the share of the corpus model")
    args = parser.parse_args()

    general = defaultdict(float)
    with open(args.lexicon, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            _, word, logp = line.rstrip("\n").split("\t")
            general[word] += math.exp(float(logp))
    total = sum(general.values())
    general = {w: math.log(p / total) for w, p in general.items()}
    floor = min(general.values())

    os.makedirs(args.out, exist_ok=True)
    unigram = {}
    with open(os.path.join(args.src, "unigram.tsv"), encoding="utf-8") as f:
        for line in f:
            word, logp = line.rstrip("\n").split("\t")
            unigram[word] = float(logp)
    corpus_floor = unigram.get("<unk>", min(unigram.values()))
    with open(os.path.join(args.out, "unigram.tsv"), "w", encoding="utf-8") as f:
        for word in set(unigram) | set(general):
            value = mix(unigram.get(word, corpus_floor), general.get(word, floor), args.weight)
            f.write(f"{word}\t{value:.6f}\n")
    with open(os.path.join(args.src, "bigram.tsv"), encoding="utf-8") as src, \
            open(os.path.join(args.out, "bigram.tsv"), "w", encoding="utf-8") as out:
        for line in src:
            w1, w2, logp = line.rstrip("\n").split("\t")
            out.write(f"{w1}\t{w2}\t{mix(float(logp), general.get(w2, floor), args.weight):.6f}\n")
    # Backoff weights scale only the corpus share; keeping them as they are leaves the general share intact for
    # unseen pairs, which is the behaviour wanted (frequency decides when the corpus has no evidence).
    with open(os.path.join(args.src, "backoff.tsv"), encoding="utf-8") as src, \
            open(os.path.join(args.out, "backoff.tsv"), "w", encoding="utf-8") as out:
        for line in src:
            word, logp = line.rstrip("\n").split("\t")
            out.write(f"{word}\t{0.0 if float(logp) < 0 else float(logp):.6f}\n")


if __name__ == "__main__":
    main()

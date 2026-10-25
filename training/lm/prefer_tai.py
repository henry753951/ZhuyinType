#!/usr/bin/env python3
"""Prefer 台 over 臺 in the language model, as everyday Taiwanese writing does.

The training text went through OpenCC s2twp, which writes 臺 almost everywhere (臺灣 349,339 times, 台灣 64).  For
every word containing 臺 whose 台 spelling is also in the lexicon, the 台 spelling takes the combined probability and
the 臺 spelling keeps its own, lowered by DEMOTE, so both stay typeable and 台 comes first.

    python3 training/lm/prefer_tai.py data/build/lm data/build/lexicon.tsv
"""
import math
import os
import sys

DEMOTE = 1.5


def logaddexp(a, b):
    if a is None:
        return b
    m = max(a, b)
    return m + math.log(math.exp(a - m) + math.exp(b - m))


def main(lm_dir, lexicon_path):
    words = set()
    with open(lexicon_path, encoding="utf-8") as f:
        for line in f:
            if not line.startswith("#"):
                words.add(line.split("\t")[1])

    def tai(word):
        if "臺" not in word:
            return None
        other = word.replace("臺", "台")
        return other if other in words else None

    def rewrite(name, key_fields):
        path = os.path.join(lm_dir, name)
        rows = {}
        with open(path, encoding="utf-8") as f:
            for line in f:
                fields = line.rstrip("\n").split("\t")
                rows[tuple(fields[:key_fields])] = float(fields[key_fields])
        merged = dict(rows)
        for key, value in rows.items():
            mapped = tuple(tai(w) or w for w in key)
            if mapped == key:
                continue
            merged[mapped] = logaddexp(merged.get(mapped) if mapped in rows else None, value)
            merged[key] = value - DEMOTE
        with open(path + ".tmp", "w", encoding="utf-8") as f:
            for key, value in merged.items():
                f.write("\t".join(key) + f"\t{value:.12f}\n")
        os.replace(path + ".tmp", path)
        return sum(1 for k in rows if any(tai(w) for w in k))

    for name, fields in [("unigram.tsv", 1), ("bigram.tsv", 2)]:
        print(f"{name}: {rewrite(name, fields)} rows with 臺 spellings")
    # Backoff weights follow their context word.
    path = os.path.join(lm_dir, "backoff.tsv")
    rows = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            word, value = line.rstrip("\n").split("\t")
            rows[word] = value
    for word, value in list(rows.items()):
        other = tai(word)
        if other and other not in rows:
            rows[other] = value
    with open(path, "w", encoding="utf-8") as f:
        f.writelines(f"{w}\t{v}\n" for w, v in rows.items())


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

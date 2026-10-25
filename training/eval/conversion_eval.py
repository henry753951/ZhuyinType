#!/usr/bin/env python3
"""Character accuracy of whole-sentence conversion, typed key by key through zhuyin-cli.

    python3 training/eval/conversion_eval.py --lexicon data/build/lexicon.tsv [--no-lm] [-n 200]

Sentences come from data/build/lm/test_sentences.txt (held out from LM training).  Each sentence is cut into its Han
runs; each run is given readings by longest-match against the lexicon (the most likely reading of each word), turned
into standard-layout keys, typed, and committed with Enter.
"""
import argparse
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile

KEYS = {
    "ㄅ": "1", "ㄆ": "q", "ㄇ": "a", "ㄈ": "z", "ㄉ": "2", "ㄊ": "w", "ㄋ": "s", "ㄌ": "x", "ㄍ": "e", "ㄎ": "d",
    "ㄏ": "c", "ㄐ": "r", "ㄑ": "f", "ㄒ": "v", "ㄓ": "5", "ㄔ": "t", "ㄕ": "g", "ㄖ": "b", "ㄗ": "y", "ㄘ": "h",
    "ㄙ": "n", "ㄧ": "u", "ㄨ": "j", "ㄩ": "m", "ㄚ": "8", "ㄛ": "i", "ㄜ": "k", "ㄝ": ",", "ㄞ": "9", "ㄟ": "o",
    "ㄠ": "l", "ㄡ": ".", "ㄢ": "0", "ㄣ": "p", "ㄤ": ";", "ㄥ": "/", "ㄦ": "-",
    "ˊ": "6", "ˇ": "3", "ˋ": "4", "˙": "7",
}


def load_lexicon(path):
    best = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            reading, word, logp = line.rstrip("\n").split("\t")
            logp = float(logp)
            if word not in best or best[word][1] < logp:
                best[word] = (reading, logp)
    return {w: r for w, (r, _) in best.items()}


def readings_for(text, lexicon):
    out, i = [], 0
    while i < len(text):
        for length in range(min(6, len(text) - i), 0, -1):
            word = text[i:i + length]
            if word in lexicon:
                out += lexicon[word].split(" ")
                i += length
                break
        else:
            return None
    return out


def keys_for(readings):
    keys = []
    for syllable in readings:
        typed = "".join(KEYS[c] for c in syllable)
        keys.append(typed if syllable[-1] in "ˊˇˋ˙" else typed + "<space>")
    return "".join(keys)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--no-lm", action="store_true", help="hide the lm/ directory next to the lexicon")
    parser.add_argument("-n", type=int, default=200)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--cli", default=".build/release/zhuyin-cli")
    args = parser.parse_args()

    lexicon_path = args.lexicon
    if args.no_lm:
        temp = tempfile.mkdtemp()
        lexicon_path = os.path.join(temp, "lexicon.tsv")
        shutil.copy(args.lexicon, lexicon_path)
    lexicon = load_lexicon(lexicon_path)

    with open("data/build/lm/test_sentences.txt", encoding="utf-8") as f:
        runs = [r for line in f for r in re.findall(r"[一-鿿]{4,20}", line)]
    random.Random(args.seed).shuffle(runs)
    total = correct = whole = tested = 0
    misses = []
    for run in runs:
        if tested >= args.n:
            break
        readings = readings_for(run, lexicon)
        if not readings:
            continue
        out = subprocess.run([args.cli, "--lexicon", lexicon_path, keys_for(readings) + "<enter>"],
                             capture_output=True, text=True).stdout
        match = re.search(r"committed: (.*)", out)
        got = match.group(1).strip() if match else ""
        tested += 1
        total += len(run)
        correct += sum(a == b for a, b in zip(run, got))
        whole += got == run
        if got != run and len(misses) < 12:
            misses.append((run, got))
    print(f"{tested} runs, character accuracy {correct / total:.1%}, whole-run accuracy {whole / tested:.1%}")
    for want, got in misses:
        print(f"  {want}  →  {got}")


if __name__ == "__main__":
    sys.exit(main())

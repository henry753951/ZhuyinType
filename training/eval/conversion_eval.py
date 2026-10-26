#!/usr/bin/env python3
"""Character accuracy of whole-sentence conversion, typed key by key through zhuyin-cli.

    python3 training/eval/conversion_eval.py [--lexicon data/build/lexicon.tsv] [--test FILE ...] [--lm-dir DIR | --no-lm] [-n 200]

Sentences come from held-out test files (default: data/build/lm/test_sentences.txt, and
data/build/corpus/test_colloquial.txt when it exists), each reported separately.  Each sentence is cut into its Han
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
    model = parser.add_mutually_exclusive_group()
    model.add_argument("--no-lm", action="store_true", help="hide the lm/ directory next to the lexicon")
    model.add_argument("--lm-dir", help="evaluate DIR as lm/, using DIR/lexicon.tsv when present")
    parser.add_argument("-n", type=int, default=200)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--cli", default=".build-engine/release/zhuyin-cli")
    parser.add_argument("--typo", choices=["off", "standard", "aggressive"], default="standard")
    parser.add_argument("--test", nargs="*", default=None)
    args = parser.parse_args()
    if args.lm_dir and not os.path.isdir(args.lm_dir):
        parser.error(f"model directory does not exist: {args.lm_dir}")

    tests = args.test or [p for p in ["data/build/lm/test_sentences.txt", "data/build/corpus/test_colloquial.txt"]
                          if os.path.exists(p)]
    with tempfile.TemporaryDirectory(prefix="zhuyin-eval-") as temp:
        lexicon_path = args.lexicon
        if args.no_lm or args.lm_dir:
            candidate = os.path.join(args.lm_dir, "lexicon.tsv") if args.lm_dir else args.lexicon
            source = candidate if os.path.isfile(candidate) else args.lexicon
            lexicon_path = os.path.join(temp, "lexicon.tsv")
            shutil.copyfile(source, lexicon_path)
            if args.lm_dir:
                os.symlink(os.path.abspath(args.lm_dir), os.path.join(temp, "lm"), target_is_directory=True)
        lexicon = load_lexicon(lexicon_path)
        for test in tests:
            evaluate(test, args, lexicon, lexicon_path)


def evaluate(test, args, lexicon, lexicon_path):
    with open(test, encoding="utf-8") as f:
        runs = [r for line in f for r in re.findall(r"[\u4e00-\u9fff]{4,20}", line)]
    random.Random(args.seed).shuffle(runs)
    total = correct = whole = tested = 0
    misses = []
    selected, scripts = [], []
    for run in runs:
        if len(selected) >= args.n:
            break
        readings = readings_for(run, lexicon)
        if not readings:
            continue
        selected.append(run)
        scripts.append(keys_for(readings) + "<enter>")
    result = subprocess.run([args.cli, "--lexicon", lexicon_path, "--typo", args.typo, "--batch"],
                            input="".join(script + "\n" for script in scripts),
                            capture_output=True, text=True, check=True)
    outputs = result.stdout.splitlines()
    if len(outputs) != len(selected):
        raise RuntimeError(f"zhuyin-cli returned {len(outputs)} lines for {len(selected)} inputs")
    for run, got in zip(selected, outputs):
        got = got.strip()
        tested += 1
        total += len(run)
        correct += sum(a == b for a, b in zip(run, got))
        whole += got == run
        if got != run and len(misses) < 8:
            misses.append((run, got))
    print(f"[{os.path.basename(test)}] {tested} runs, character accuracy {correct / max(1, total):.1%}, "
          f"whole-run accuracy {whole / max(1, tested):.1%}")
    for want, got in misses:
        print(f"  {want}  →  {got}")


if __name__ == "__main__":
    sys.exit(main())

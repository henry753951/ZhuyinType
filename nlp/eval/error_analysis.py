#!/usr/bin/env python3
"""Where conversion fails: the same runs typed with the habits real typists have, through the real composer.

    python3 nlp/eval/error_analysis.py --cli .build-me/release/zhuyin-cli --reranker data/build/reranker \
        --test data/build/corpus/dev_colloquial.txt data/build/corpus/dev_wikipedia.txt -n 400

Each run is typed several ways (only runs a habit applies to count for it):
  clean       the most common reading of every word, every first tone with Space
  sandhi      一 and 不 typed as pronounced: 一直 ㄧˋ ㄓˊ, 一定 ㄧˊ ㄉㄧㄥˋ, 不要 ㄅㄨˊ ㄧㄠˋ
  neutral     neutral-tone syllables typed with Space (first tone), as many people do
  nospace     first-tone syllables followed by another syllable typed without Space: syllable boundaries
              are left for the input method to find
  typo5       5% of syllables get one keyboard slip (typo_eval.py's error mix)

For each it reports whole-run accuracy of what is displayed (with the reranker when given) and recall@K: how often
the right sentence is among the engine's K best, the most any reranker could reach.  Low recall means the engine
(lexicon, segmentation, language model) loses the answer; high recall with low accuracy means the choice is wrong.
"""
import argparse
import os
import random
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from conversion_eval import KEYS, load_lexicon, readings_for  # noqa: E402
from typo_eval import noisy_script  # noqa: E402

TONES = "ˊˇˋ˙"


def keys_for(syllables, space_after=None):
    """Keys for syllables; `space_after[i]` False omits the Space that ends a first-tone syllable."""
    out = []
    for i, syllable in enumerate(syllables):
        keys = "".join(KEYS[c] for c in syllable)
        if syllable[-1] not in TONES and (space_after is None or space_after[i]):
            keys += " "
        out.append(keys)
    return "".join(out)


def sandhi(run, syllables):
    changed = list(syllables)
    for i, ch in enumerate(run[:-1]):
        tone = syllables[i + 1][-1] if syllables[i + 1][-1] in TONES else ""
        if ch == "一" and syllables[i] == "ㄧ":
            changed[i] = "ㄧˊ" if tone in "ˋ˙" and tone else "ㄧˋ"
        elif ch == "不" and syllables[i] == "ㄅㄨˋ" and tone in ("ˋ", "˙"):
            changed[i] = "ㄅㄨˊ"
    return changed if changed != syllables else None


def neutral(syllables):
    changed = [s[:-1] if s.endswith("˙") else s for s in syllables]
    return changed if changed != syllables else None


def nospace(syllables, rng):
    flags = [True] * len(syllables)
    candidates = [i for i in range(len(syllables) - 1) if syllables[i][-1] not in TONES]
    if not candidates:
        return None
    for i in candidates:
        flags[i] = False
    return flags


def run_cli(args, scripts, nbest=0):
    command = [args.cli, "--typo", "standard", "--batch"]
    if nbest:
        command += ["--report-nbest", str(nbest)]
    elif args.reranker:
        command += ["--reranker", args.reranker, "--rerank-weight", "1.0", "--rerank-count", "16"]
    result = subprocess.run(command, input="".join(s + "\n" for s in scripts), capture_output=True, text=True, check=True)
    lines = result.stdout.splitlines()
    assert len(lines) == len(scripts), (len(lines), len(scripts))
    return lines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", default=".build-me/release/zhuyin-cli")
    parser.add_argument("--reranker")
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--test", nargs="+", required=True)
    parser.add_argument("-n", type=int, default=400)
    parser.add_argument("-k", type=int, default=16)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--examples", type=int, default=0, help="print this many failures per habit")
    args = parser.parse_args()
    lexicon = load_lexicon(args.lexicon)
    for test in args.test:
        with open(test, encoding="utf-8") as f:
            runs = [r for line in f for r in re.findall(r"[一-鿿]{4,20}", line)]
        random.Random(args.seed).shuffle(runs)
        selected = []
        for run in runs:
            syllables = readings_for(run, lexicon)
            if syllables and len(syllables) == len(run):
                selected.append((run, syllables))
            if len(selected) == args.n:
                break
        rng = random.Random(args.seed)
        habits = {name: [] for name in ["clean", "sandhi", "neutral", "nospace", "typo5"]}
        for run, syllables in selected:
            habits["clean"].append((run, keys_for(syllables)))
            if (s := sandhi(run, syllables)) is not None:
                habits["sandhi"].append((run, keys_for(s)))
            if (s := neutral(syllables)) is not None:
                habits["neutral"].append((run, keys_for(s)))
            if (flags := nospace(syllables, rng)) is not None:
                habits["nospace"].append((run, keys_for(syllables, flags)))
            habits["typo5"].append((run, noisy_script(syllables, 0.05, rng).removesuffix("<enter>")))
        print(f"[{os.path.basename(test)}] {len(selected)} runs")
        print(f"  {'habit':<9}{'runs':>6}{'shown':>9}{'recall@' + str(args.k):>11}")
        for name, items in habits.items():
            if not items:
                continue
            scripts = [keys for _, keys in items]
            shown = run_cli(args, [k + "<enter>" for k in scripts])
            best = run_cli(args, scripts, nbest=args.k)
            right = sum(got == run for (run, _), got in zip(items, shown))
            recalled = sum(run in line.split("\t") for (run, _), line in zip(items, best))
            print(f"  {name:<9}{len(items):>6}{right / len(items):>9.1%}{recalled / len(items):>11.1%}")
            if args.examples:
                misses = [(run, got, line.split("\t")[:3]) for (run, _), got, line in zip(items, shown, best) if got != run]
                for run, got, top in misses[:args.examples]:
                    print(f"      {run} → {got}   前三：{' / '.join(top)}")


if __name__ == "__main__":
    main()

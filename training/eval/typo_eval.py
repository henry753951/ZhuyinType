#!/usr/bin/env python3
"""Reproducible keyboard-noise evaluation through the real composer, with learning reset between runs.

python training/eval/typo_eval.py -n 400 --typo all
Use --test dev_wikipedia.txt dev_colloquial.txt for calibration; keep test sets held out.
False corrections count clean runs that off gets right and the selected mode gets wrong.
Changed-vs-off also counts differing outputs regardless of reference correctness.
Errors are sampled from applicable types with weights 30/15/30/10/8/7; tone swaps
are applicable only to that tone pair. Every injected error changes the raw keys.
"""
import argparse
import json
import os
import random
import re
import shutil
import subprocess
import tempfile
from conversion_eval import KEYS, load_lexicon, readings_for

ROWS = ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"]
NEIGHBOURS = {key: set() for row in ROWS for key in row}
for row, keys in enumerate(ROWS):
    for index, key in enumerate(keys):
        adjacent = ([keys[index + 1]] if index + 1 < len(keys) else [])
        if row:
            adjacent += [ROWS[row - 1][i] for i in (index, index + 1) if i < len(ROWS[row - 1])]
        for other in adjacent:
            NEIGHBOURS[key].add(other)
            NEIGHBOURS[other].add(key)
INPUT_KEYS = "".join(dict.fromkeys(KEYS.values())) + " "


def mutations(keys):
    """Candidates per error type; punctuation neighbours remain real physical slips."""
    return [
        [keys[:-1] + {"3": "4", "4": "3"}[keys[-1]]] if keys[-1] in "34" else [],
        [keys[:-1] + {"6": "7", "7": "6"}[keys[-1]]] if keys[-1] in "67" else [],
        [keys[:i] + other + keys[i + 1:] for i, key in enumerate(keys)
         for other in sorted(NEIGHBOURS.get(key, ()))],
        [keys[:i] + keys[i + 1] + keys[i] + keys[i + 2:]
         for i in range(len(keys) - 1) if keys[i] != keys[i + 1]],
        [keys[:i] + keys[i + 1:] for i in range(len(keys))],
        [keys[:i] + key + keys[i:] for i in range(len(keys) + 1) for key in INPUT_KEYS],
    ]


def noisy_script(readings, rate, rng):
    result = []
    for syllable in readings:
        keys = "".join(KEYS[c] for c in syllable)
        if syllable[-1] not in "ˊˇˋ˙":
            keys += " "
        if rng.random() < rate:
            choices = [(weight, variants) for weight, variants in zip((30, 15, 30, 10, 8, 7), mutations(keys)) if variants]
            variants = rng.choices([v for _, v in choices], weights=[w for w, _ in choices])[0]
            changed = rng.choice(variants)
            assert changed != keys
            keys = changed
        result.append(keys)
    return "".join(result) + "<enter>"


def select_runs(path, lexicon, count, seed):
    with open(path, encoding="utf-8") as source:
        runs = [r for line in source for r in re.findall(r"[\u4e00-\u9fff]{4,20}", line)]
    random.Random(seed).shuffle(runs)
    selected = []
    for run in runs:
        readings = readings_for(run, lexicon)
        if readings:
            selected.append((run, readings))
            if len(selected) == count:
                return selected
    raise ValueError(f"{path}: only {len(selected)} readable runs, need {count}")


def convert(cli, lexicon, mode, scripts):
    result = subprocess.run([cli, "--lexicon", lexicon, "--typo", mode, "--batch"],
                            input="\n".join(scripts) + "\n", capture_output=True, text=True, check=True)
    outputs = result.stdout.splitlines()
    if len(outputs) != len(scripts):
        raise RuntimeError(f"expected {len(scripts)} outputs, got {len(outputs)}")
    return outputs


def metrics(wanted, outputs):
    # Levenshtein alignment gives useful character accuracy after a deleted syllable.
    errors = 0
    for want, got in zip(wanted, outputs):
        previous = list(range(len(got) + 1))
        for i, a in enumerate(want, 1):
            current = [i]
            for j, b in enumerate(got, 1):
                current.append(min(current[-1] + 1, previous[j] + 1, previous[j - 1] + (a != b)))
            previous = current
        errors += previous[-1]
    return dict(character_accuracy=max(0, 1 - errors / sum(map(len, wanted))),
                whole_run_accuracy=sum(a == b for a, b in zip(wanted, outputs)) / len(wanted))


def self_check():
    assert "4" in NEIGHBOURS["3"] and "7" in NEIGHBOURS["6"] and "6" not in NEIGHBOURS["4"]
    assert "w" in NEIGHBOURS["e"] and {"2", "3", "a", "s"} <= NEIGHBOURS["w"]
    assert all(a in NEIGHBOURS[b] for a, neighbours in NEIGHBOURS.items() for b in neighbours)
    rng = random.Random(7)
    for keys in ("su3", "fu4", "186", "187", "wu0 "):
        assert all(variant != keys for group in mutations(keys) for variant in group)
    assert noisy_script(["ㄋㄧˇ", "ㄏㄠˇ"], 0, rng) == "su3cl3<enter>"
    assert metrics(["甲乙丙"], ["甲丙"])["character_accuracy"] == 1 - 1 / 3


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    model = parser.add_mutually_exclusive_group()
    model.add_argument("--no-lm", action="store_true")
    model.add_argument("--lm-dir")
    parser.add_argument("--cli", default=".build-typo/release/zhuyin-cli")
    parser.add_argument("--typo", choices=["off", "standard", "aggressive", "all"], default="all")
    parser.add_argument("--test", nargs="+", default=["data/build/lm/test_sentences.txt", "data/build/corpus/test_colloquial.txt"])
    parser.add_argument("--rates", nargs="+", type=float, default=[0, .1, .2])
    parser.add_argument("-n", type=int, default=400)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--json", help="write full numerical results")
    args = parser.parse_args()
    if args.n < 1 or any(not 0 <= rate <= 1 for rate in args.rates):
        parser.error("n must be positive and rates must be between 0 and 1")
    if args.lm_dir and not os.path.isdir(args.lm_dir):
        parser.error("model directory does not exist")
    self_check()
    rows = []
    with tempfile.TemporaryDirectory(prefix="zhuyin-typo-") as temp:
        lexicon_path = args.lexicon
        if args.no_lm or args.lm_dir:
            candidate = os.path.join(args.lm_dir, "lexicon.tsv") if args.lm_dir else args.lexicon
            source = candidate if os.path.isfile(candidate) else args.lexicon
            lexicon_path = os.path.join(temp, "lexicon.tsv")
            shutil.copyfile(source, lexicon_path)
            if args.lm_dir:
                os.symlink(os.path.abspath(args.lm_dir), os.path.join(temp, "lm"), target_is_directory=True)
        lexicon = load_lexicon(lexicon_path)
        for path in args.test:
            selected = select_runs(path, lexicon, args.n, args.seed)
            wanted = [run for run, _ in selected]
            clean = [noisy_script(readings, 0, random.Random(args.seed)) for _, readings in selected]
            baseline = convert(args.cli, lexicon_path, "off", clean)
            for rate in args.rates:
                rng = random.Random(args.seed)
                scripts = [noisy_script(readings, rate, rng) for _, readings in selected]
                for mode in (["off", "standard", "aggressive"] if args.typo == "all" else [args.typo]):
                    outputs = baseline if mode == "off" and rate == 0 else convert(args.cli, lexicon_path, mode, scripts)
                    row = dict(dataset=path, runs=len(selected), seed=args.seed, rate=rate, typo=mode,
                               **metrics(wanted, outputs))
                    if rate == 0:
                        row["false_correction_rate"] = sum(a == w and b != w for a, b, w in zip(baseline, outputs, wanted)) / len(wanted)
                        row["changed_vs_off"] = sum(a != b for a, b in zip(baseline, outputs)) / len(wanted)
                    rows.append(row)
                    print(json.dumps(row, ensure_ascii=False), flush=True)
    if args.json:
        with open(args.json, "w", encoding="utf-8") as output:
            json.dump(rows, output, ensure_ascii=False, indent=2)


if __name__ == "__main__":
    main()

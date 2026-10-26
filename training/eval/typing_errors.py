#!/usr/bin/env python3
"""Timed keyboard-error evaluation. Run from the repository root; rebuild first (see TYPING_ERRORS.md)."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, replace
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import random
import re
import shlex
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from conversion_eval import KEYS, keys_for, load_lexicon, readings_for
from typo_eval import metrics

CONDITIONS = (
    "exact", "exact-fast", "swap-in-syllable", "swap-across", "swap-across-2",
    "rollover", "rollover-2", "hotspot", "hotspot-2", "extra-key", "missing-key",
    "tone", "nospace-all", "nospace-30", "mixed",
)
DEFAULT_TESTS = ["data/build/corpus/dev_colloquial.txt", "data/build/corpus/dev_wikipedia.txt"]
RERANK_OPTIONS = {"--reranker", "--rerank-weight", "--rerank-count", "--exact-margin"}
TONE_SWAP = dict(zip("3467", "4376"))

# Key centres in key-width units: number row is the origin, then real US stagger.
POSITIONS = {}
FINGERS = {}
for y, (row, offset) in enumerate((("1234567890-=", 0), ("qwertyuiop[]\\", .25),
                                     ("asdfghjkl;'", .5), ("zxcvbnm,./", 1))):
    for x, key in enumerate(row):
        POSITIONS[key] = (x + offset, y)
        FINGERS[key] = ("L", min(x, 3)) if x < 5 else ("R", min(max(x - 6, 0), 3))
POSITIONS["`"] = (-1, 0)
FINGERS["`"] = ("L", 0)
POSITIONS[" "] = (5.5, 4.25)
FINGERS[" "] = ("R", 4)  # right thumb


@dataclass(frozen=True)
class Stroke:
    key: str
    syllable: int
    gap: float | None = None
    first_tone: bool = False


def canonical(readings):
    return [Stroke(key, i, first_tone=key == " ") for i, reading in enumerate(readings)
            for key in keys_for([reading]).replace("<space>", " ")]


def neighbour(key, rng):
    """Sample a Gaussian landing point, snap to a key centre, condition on a local slip."""
    x, y = POSITIONS[key]
    while True:
        px, py = rng.gauss(x, .45), rng.gauss(y, .3)
        other = min(POSITIONS, key=lambda k: (POSITIONS[k][0] - px) ** 2 + (POSITIONS[k][1] - py) ** 2)
        ox, oy = POSITIONS[other]
        if other != key and math.hypot(ox - x, oy - y) <= 1.6:
            return other


def mutation(strokes, kind, rng, count=1, syllable=None):
    """Return changed strokes and disjoint edit sites, or None when inapplicable."""
    sites = []
    for i, stroke in enumerate(strokes):
        if syllable is not None and stroke.syllable != syllable:
            continue
        if kind in ("swap-in-syllable", "swap-across", "rollover"):
            if i + 1 == len(strokes) or stroke.key == strokes[i + 1].key:
                continue
            across = stroke.syllable != strokes[i + 1].syllable
            if kind == "swap-in-syllable" and across or kind == "swap-across" and not across:
                continue
            sites.append((i, i + 1))
        elif kind in ("hotspot", "extra-key"):
            # Space spans several key widths; the unit-key Gaussian is inappropriate there.
            if stroke.key != " ":
                sites.append((i,))
        elif kind == "missing-key" or kind == "tone" and stroke.key in TONE_SWAP:
            sites.append((i,))
    choices = [group for group in itertools.combinations(sites, count)
               if len({i for site in group for i in site}) == sum(map(len, group))]
    if not choices:
        return None
    chosen = rng.choice(choices)
    result = list(strokes)
    for site in sorted(chosen, reverse=True):
        i = site[0]
        if len(site) == 2:
            result[i], result[i + 1] = result[i + 1], result[i]
            if kind == "rollover":
                result[i + 1] = replace(result[i + 1], gap=rng.uniform(0, 15))
        elif kind == "hotspot":
            result[i] = replace(result[i], key=neighbour(result[i].key, rng))
        elif kind == "extra-key":
            extra = replace(result[i], key=neighbour(result[i].key, rng), gap=None)
            pair = [extra, result[i]] if rng.random() < .5 else [result[i], extra]
            pair[1] = replace(pair[1], gap=rng.uniform(0, 20))
            result[i:i + 1] = pair
        elif kind == "missing-key":
            del result[i]
        elif kind == "tone":
            result[i] = replace(result[i], key=TONE_SWAP[result[i].key])
    assert [s.key for s in result] != [s.key for s in strokes]
    return result, chosen


def omit_spaces(strokes, rng, rate, last_syllable):
    return [s for s in strokes if not (s.first_tone and s.key == " " and s.syllable < last_syllable and rng.random() < rate)]


def condition(readings, name, rng):
    strokes = canonical(readings)
    fast = name == "exact-fast"
    edits = 0
    if name.startswith("nospace-"):
        changed = omit_spaces(strokes, rng, 1 if name == "nospace-all" else .3, len(readings) - 1)
        edits = len(strokes) - len(changed)
        if not edits:
            return None
        strokes = changed
    elif name == "mixed":
        fast = rng.random() < .3
        for sid in range(len(readings)):
            draw = rng.random()
            kind = next((kind for threshold, kind in ((.03, "hotspot"), (.05, "rollover"),
                        (.06, "extra-key"), (.07, "missing-key"), (.08, "tone")) if draw < threshold), None)
            if kind and (changed := mutation(strokes, kind, rng, syllable=sid)):
                strokes = changed[0]
                edits += 1
        changed = omit_spaces(strokes, rng, .3, len(readings) - 1)
        edits += len(strokes) - len(changed)
        strokes = changed
    elif name not in ("exact", "exact-fast"):
        count = 2 if name.endswith("-2") else 1
        kind = name.removesuffix("-2")
        changed = mutation(strokes, kind, rng, count)
        if changed is None:
            return None
        strokes, _ = changed
        edits = count
    return strokes, fast, edits


def timed_script(strokes, rng, fast=False):
    result = []
    previous = None
    for stroke in strokes:
        hand, finger = FINGERS[stroke.key]
        factor = 1
        if previous is not None:
            if previous[0] != hand:
                factor = .75
            elif previous == (hand, finger):
                factor = 1.4
        gap = stroke.gap if stroke.gap is not None else rng.lognormvariate(math.log((60 if fast else 120) * factor), .35)
        result.append(f"<dt:{gap:.3f}>" + ("<space>" if stroke.key == " " else stroke.key))
        previous = (hand, finger)
    return "".join(result)


def seeded(seed, *parts):
    # Stable across Python versions/processes; adding a config never changes its test inputs.
    return random.Random(int.from_bytes(hashlib.sha256(json.dumps([seed, *parts]).encode()).digest(), "big"))


def select_runs(path, lexicon, count, seed):
    found = re.findall(r"[\u4e00-\u9fff]{4,20}", Path(path).read_text(encoding="utf-8"))
    random.Random(seed).shuffle(found)
    selected = []
    for run in found:
        readings = readings_for(run, lexicon)
        if readings and len(readings) == len(run):
            selected.append((run, readings))
            if len(selected) == count:
                return selected
    raise ValueError(f"{path}: only {len(selected)} readable runs; requested {count}")


def without_reranker(extra):
    result = []
    i = 0
    while i < len(extra):
        if extra[i].split("=", 1)[0] in RERANK_OPTIONS:
            i += 1 if "=" in extra[i] else 2
        else:
            result.append(extra[i])
            i += 1
    return result


def convert(cli, lexicon, extra, scripts, nbest=False):
    if not scripts:
        return []
    command = [cli, "--lexicon", lexicon, "--batch", *extra]
    if nbest:
        command += ["--report-nbest", "16"]
    start = time.monotonic()
    result = subprocess.run(command, input="\n".join(scripts) + "\n", capture_output=True,
                            text=True, timeout=600)
    if result.returncode:
        raise RuntimeError(f"{shlex.join(command)} exited {result.returncode}: {result.stderr[-4000:]}")
    outputs = result.stdout.splitlines()
    if len(outputs) != len(scripts):
        raise RuntimeError(f"{shlex.join(command)}: expected {len(scripts)} lines, got {len(outputs)}")
    print(f"  {'recall' if nbest else 'commit'}: {len(scripts)} runs, {time.monotonic() - start:.1f}s", file=sys.stderr, flush=True)
    return outputs


def condition_outputs(cli, lexicon, configs, scripts):
    # Three isolated batch processes overlap CPU work; shared engine recall runs only once.
    with ThreadPoolExecutor(max_workers=3) as pool:
        commits, recalls = {}, {}
        for label, extra in configs:
            commits[label] = pool.submit(convert, cli, lexicon, extra, [s + "<enter>" for s in scripts])
            engine_args = tuple(without_reranker(extra))
            if engine_args not in recalls:
                recalls[engine_args] = pool.submit(convert, cli, lexicon, list(engine_args), scripts, True)
        return {label: (commits[label].result(), recalls[tuple(without_reranker(extra))].result())
                for label, extra in configs}


def digest(path):
    with open(path, "rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def artifacts(args, configs):
    paths = {Path(args.cli), Path(args.lexicon), *(Path(p) for p in args.test)}
    roots = {Path(args.lexicon).parent / "lm"}
    for _, extra in configs:
        for i, option in enumerate(extra):
            if option == "--reranker" and i + 1 < len(extra):
                roots.add(Path(extra[i + 1]))
            elif option.startswith("--reranker="):
                roots.add(Path(option.split("=", 1)[1]))
    for root in roots:
        if root.is_dir():
            paths.update(p for p in root.rglob("*") if p.is_file())
    return {str(p): digest(p) for p in sorted(paths)}


def table(rows):
    lines = ["| Dataset | Condition | Config | N | Whole | Char | Recall@16 | False corr. |",
             "|---|---|---|---:|---:|---:|---:|---:|"]
    for row in rows:
        def percent(key):
            value = row.get(key)
            return "—" if value is None else f"{value:.1%}"
        lines.append(f"| {Path(row['dataset']).stem.removeprefix('dev_')} | {row['condition']} | {row['config']} | {row['runs']} | "
                     + " | ".join(percent(k) for k in ("whole_run_accuracy", "character_accuracy", "recall_at_16", "false_correction_rate")) + " |")
    return "\n".join(lines)


def self_check():
    readings = ["ㄋㄧˇ", "ㄏㄠˇ", "ㄊㄧㄢ", "ㄑㄧˋ"]
    original = canonical(readings)
    assert "".join(s.key for s in original) == "su3cl3wu0 fu4"
    assert POSITIONS["q"] == (.25, 1) and FINGERS["u"] == ("R", 0)
    for name in CONDITIONS:
        for seed in range(30):
            result = condition(readings, name, random.Random(seed))
            if result is None:
                assert name == "nospace-30"
                continue
            strokes, fast, edits = result
            assert all(s.key in POSITIONS for s in strokes)
            if name not in ("exact", "exact-fast", "mixed"):
                assert [s.key for s in strokes] != [s.key for s in original]
            if name.endswith("-2"):
                assert edits == 2
            if name.startswith("rollover"):
                assert sum(s.gap is not None and 0 <= s.gap <= 15 for s in strokes) == edits
            if name == "extra-key":
                assert len(strokes) == len(original) + 1
                assert sum(s.gap is not None and 0 <= s.gap <= 20 for s in strokes) == 1
            if name == "missing-key":
                assert len(strokes) == len(original) - 1
            script = timed_script(strokes, random.Random(seed), fast)
            assert script.count("<dt:") == len(strokes) and "<enter>" not in script
    for kind in ("swap-in-syllable", "swap-across", "rollover", "hotspot"):
        changed, sites = mutation(original, kind, random.Random(7), 2)
        assert len({i for site in sites for i in site}) == sum(map(len, sites))
        assert sum(a.key != b.key for a, b in zip(original, changed)) == (2 if kind == "hotspot" else 4)
        for site in sites:
            if kind.startswith("swap-"):
                across = original[site[0]].syllable != original[site[1]].syllable
                assert across == (kind == "swap-across")
    assert condition(["ㄚ", "ㄚ"], "swap-across-2", random.Random(7)) is None
    assert condition(["ㄚ", "ㄚ"], "tone", random.Random(7)) is None
    assert omit_spaces(canonical(["ㄚ", "ㄚ"]), random.Random(7), 1, 1)[-1].key == " "
    assert omit_spaces([Stroke(" ", 0)], random.Random(7), 1, 1) == [Stroke(" ", 0)]
    assert without_reranker(["--decoder", "future", "--reranker", "dir", "--rerank-weight=1", "--rerank-count", "16"]) == ["--decoder", "future"]
    assert metrics(["甲乙丙"], ["甲丙"])["character_accuracy"] == 1 - 1 / 3
    normal = timed_script(original * 100, random.Random(7))
    fast = timed_script(original * 100, random.Random(7), True)
    gaps = lambda s: [float(x) for x in re.findall(r"<dt:([\d.]+)>", s)]
    assert all(abs(a - 2 * b) <= .002 for a, b in zip(gaps(normal), gaps(fast)))
    assert seeded(7, "x").random() == seeded(7, "x").random()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", nargs=2, action="append", metavar=("LABEL", "EXTRA CLI ARGS"))
    parser.add_argument("--test", nargs="+", default=DEFAULT_TESTS, help="corpus paths (default: both dev sets)")
    parser.add_argument("-n", type=int, default=300, help="base runs per dataset, before applicability filtering")
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--json", metavar="OUT")
    parser.add_argument("--examples", type=int, default=0, metavar="N")
    parser.add_argument("--cli", default=".build-eval/release/zhuyin-cli")
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--self-test", action="store_true", help="assert-based checks; no build/data/CLI needed")
    args = parser.parse_args()
    if args.n < 1 or args.examples < 0:
        parser.error("-n must be positive and --examples nonnegative")
    self_check()
    if args.self_test:
        print("typing_errors self-checks passed")
        return
    configs = [(label, shlex.split(extra)) for label, extra in (args.config or [("current", "")])]
    if len({label for label, _ in configs}) != len(configs):
        parser.error("configuration labels must be unique")
    for _, extra in configs:
        if any(x.split("=", 1)[0] in {"--keep-memory", "--report-nbest", "--batch", "--timing"} for x in extra):
            parser.error("configs must not override batch isolation/output (--keep-memory/--report-nbest/--batch/--timing)")
    started = time.monotonic()
    before = artifacts(args, configs)
    report = dict(started_utc=datetime.now(timezone.utc).isoformat(), command=shlex.join(sys.argv),
                  seed=args.seed, requested_per_dataset=args.n, configs=dict(configs),
                  coreml_units=os.environ.get("ZHUYIN_UNITS", "cpuOnly"),
                  artifact_sha256=before, rows=[])
    lexicon = load_lexicon(args.lexicon)
    datasets = {path: select_runs(path, lexicon, args.n, args.seed) for path in args.test}
    clean = {}
    for name in CONDITIONS:
        items = []
        for path, selected in datasets.items():
            for index, (run, readings) in enumerate(selected):
                result = condition(readings, name, seeded(args.seed, path, index, name, "errors"))
                if result is not None:
                    strokes, fast, edits = result
                    timing_name = "exact" if name == "exact-fast" else name
                    script = timed_script(strokes, seeded(args.seed, path, index, timing_name, "timing"), fast)
                    items.append((path, index, run, script, edits, fast))
        print(f"[{name}] {len(items)} applicable", file=sys.stderr, flush=True)
        scripts = [item[3] for item in items]
        batches = condition_outputs(args.cli, args.lexicon, configs, scripts)
        for label, extra in configs:
            print(f" {label}", file=sys.stderr, flush=True)
            outputs, recalled = batches[label]
            best = [line.split("\t") for line in recalled]
            if name == "exact":
                clean[label] = {(item[0], item[1]): got for item, got in zip(items, outputs)}
            for path in datasets:
                subset = [(item, got, candidates) for item, got, candidates in zip(items, outputs, best) if item[0] == path]
                wanted = [item[2] for item, _, _ in subset]
                committed = [got for _, got, _ in subset]
                row = dict(dataset=path, condition=name, config=label, runs=len(subset),
                           injected_errors=sum(i[4] for i, _, _ in subset), fast_runs=sum(i[5] for i, _, _ in subset),
                           **(metrics(wanted, committed) if subset else dict(character_accuracy=None, whole_run_accuracy=None)),
                           recall_at_16=sum(item[2] in candidates for item, _, candidates in subset) / len(subset) if subset else None)
                if name in ("exact", "exact-fast"):
                    row["false_correction_rate"] = sum(got != clean[label][item[0], item[1]] for item, got, _ in subset) / len(subset)
                row["examples"] = [dict(wanted=w, committed=o, nbest=b, script=s)
                                   for (_, _, w, s, _, _), o, b in subset if w != o][:args.examples]
                row["cases"] = [dict(index=item[1], wanted=item[2], script=item[3], errors=item[4], fast=item[5], committed=o, nbest=b)
                                for item, o, b in subset]
                report["rows"].append(row)
                if args.json:
                    Path(args.json).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
                for example in row["examples"]:
                    print(f"  {example['wanted']} → {example['committed']}", file=sys.stderr)
    report["elapsed_seconds"] = time.monotonic() - started
    after = artifacts(args, configs)
    report["artifacts_changed_during_run"] = before != after
    if before != after:
        print("WARNING: CLI/model/corpus artifacts changed during evaluation; rerun before comparing configs.", file=sys.stderr)
    if args.json:
        Path(args.json).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(table(report["rows"]), flush=True)


if __name__ == "__main__":
    main()

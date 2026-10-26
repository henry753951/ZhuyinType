#!/usr/bin/env python3
"""Run the remaining v2 experiment after the nohup v2a/v2b training queue starts.

Uses frozen evaluation code/binary and existing remote environments; never edits app sources.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data/build/reranker_v2_experiment"
REPORT = ROOT / "training/reranker/REPORT_v2.md"
HOST = "$ZHUYIN_REMOTE_HOST"
KEY = str(Path.home() / ".ssh/<remote-key>")
REMOTE = "~/zhuyintype"
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-i", KEY, HOST]
DEV = ["data/build/corpus/dev_colloquial.txt", "data/build/corpus/dev_wikipedia.txt"]
TEST = ["data/build/lm/test_sentences.txt", "data/build/corpus/test_colloquial.txt",
        "training/eval/colloquial_smoke.txt"]
WEIGHTS = [0.7, 1.0, 1.4]
ENV = {k: v for k, v in os.environ.items() if k != "ZHUYIN_UNITS"}
ENV["DEVELOPER_DIR"] = "/Applications/Xcode.app/Contents/Developer"
STATE = {"dev": {}, "latency": {}, "training": {}, "test": {}, "events": [], "status": "running"}


def event(message):
    print(time.strftime("%Y-%m-%d %H:%M:%S"), message, flush=True)
    STATE["events"].append(message)
    (OUT / "results.json").write_text(json.dumps(STATE, indent=2) + "\n")


def run(command, log):
    event("$ " + shlex.join(command))
    with (OUT / log).open("w") as stream:
        result = subprocess.run(command, cwd=ROOT, env=ENV, stdout=stream, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f"exit {result.returncode}: {shlex.join(command)}; see {OUT / log}")
    return (OUT / log).read_text()


def remote(code):
    command = SSH + [shlex.join([REMOTE + "/.venv-gen/bin/python", "-c", code])]
    for attempt in range(3):
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode == 0:
            return json.loads(result.stdout)
        event(f"Remote probe failed ({attempt + 1}/3): {result.stderr.strip()}")
        if attempt < 2:
            time.sleep(15)
    raise RuntimeError("Remote probe failed three times; stopped without guessing.")


def probe(variant):
    return remote(f"""
import json
from pathlib import Path
r=Path({REMOTE!r}); v={variant!r}
result={{}}
for phase in ['train','export']:
 p=r/'logs'/f'{{v}}-{{phase}}.exit'
 result[phase]=int(p.read_text()) if p.exists() else None
p=r/'logs'/f'{{v}}-train.log'
lines=p.read_text().splitlines() if p.exists() else []
rows=[json.loads(line) for line in lines if line.startswith('{{')]
result['last']=rows[-1] if rows else {{}}
result['tail']=lines[-3:]
p=r/v/'train-log.json'
result['history']=json.loads(p.read_text()) if p.exists() else None
print(json.dumps(result))
""")


def parse_eval(text):
    matches = re.findall(r"\[([^\]]+)\] (\d+) runs, character accuracy ([\d.]+)%, whole-run accuracy ([\d.]+)%", text)
    result = {}
    for name, n, char, whole in matches:
        n = int(n)
        assert 0 < n <= 400
        # With <=400 runs, the printed 0.1% uniquely identifies the whole-run count.
        count = round(float(whole) * n / 100)
        assert abs(100 * count / n - float(whole)) <= 0.051
        result[name] = {"runs": n, "char": float(char), "whole": 100 * count / n,
                        "whole_correct": count}
    return result


def mean(rows, metric):
    return sum(row[metric] for row in rows.values()) / len(rows)


def eval_model(variant, weight, heldout=False):
    label = f"{variant}-w{weight:.1f}-{'test' if heldout else 'dev'}"
    command = [sys.executable, str(OUT / "conversion_eval.py"), "--cli", str(OUT / "zhuyin-cli"),
               "--lexicon", str(OUT / "lexicon.tsv"),
               "-n", "400", "--reranker", "data/build/" + ("reranker" if variant == "v1" else variant),
               "--rerank-weight", str(weight), "--rerank-count", "16", "--test"] + [
                   str(OUT / ("test" if heldout else "dev") / Path(p).name) for p in (TEST if heldout else DEV)]
    output = OUT / (label + ".txt")
    rows = parse_eval(output.read_text()) if output.exists() else {}
    if len(rows) == (3 if heldout else 2):
        event(f"Reuse completed evaluation: {output.name}")
    else:
        assert not (heldout and output.exists()), "Partial held-out evaluation exists; do not rerun or tune."
        rows = parse_eval(run(command, label + ".txt"))
    assert len(rows) == (3 if heldout else 2), f"Incomplete evaluation: {label}"
    STATE["test" if heldout else "dev"][label] = rows
    event(f"{label}: {rows}")
    return rows


def measure(variant):
    directory = "data/build/" + ("reranker" if variant == "v1" else variant)
    text = run([str(OUT / "zhuyin-cli"), "--lexicon", str(OUT / "lexicon.tsv"), "--reranker", directory, "--rerank-count", "16", "--timing",
                "ji3ap6vu35j/ru04xu4vu/5j4u4ek75k4cl3vu04xu4cl3wu4"], variant + "-latency.txt")
    calls = [float(ms) for ms in re.findall(r"\[(?:rerank|candidates) \d+ ([\d.]+) ms\]", text)]
    assert len(calls) > 1, "No steady-state latency samples"
    STATE["latency"][variant] = {"first": calls[0], "calls": calls[1:], "max": max(calls[1:])}
    event(f"{variant} CPU calls after first: {calls[1:]}")


def finish_variant(variant):
    while True:
        status = probe(variant)
        event(f"{variant}: train={status['train']}, export={status['export']}, last={status['last']}")
        if any(status[phase] not in (None, 0) for phase in ("train", "export")):
            raise RuntimeError(f"Remote {variant} failed: {status}")
        if status["train"] == 0 and status["export"] == 0:
            assert status["last"]["step"] == 90000
            STATE["training"][variant] = status["history"][-1]
            break
        time.sleep(45)
    destination = ROOT / "data/build" / variant
    destination.mkdir(exist_ok=False)
    for name in ["Reranker.mlpackage", "vocab.json", "train-log.json"]:
        run(["scp", "-r", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-i", KEY,
             f"{HOST}:{REMOTE}/{variant}/{name}", str(destination) + "/"], f"{variant}-copy-{name}.log")
    assert (destination / "vocab.json").read_bytes() == (ROOT / "data/build/reranker/vocab.json").read_bytes()
    run(["xcrun", "coremlcompiler", "compile", str(destination / "Reranker.mlpackage"), str(destination)],
        variant + "-compile.log")
    (destination / "config.json").write_text('{"weight": 1.0, "count": 16}\n')
    for weight in WEIGHTS:
        eval_model(variant, weight)
    measure(variant)
    write_report()


def write_report():
    prefix = REPORT.read_text().split("## Results", 1)[0]
    rows = ["## Results", "", f"Status: {STATE['status']}.", "", "### Training", "",
            "| Model | Steps | Minutes | Wiki perplexity | Colloquial perplexity |",
            "|---|---:|---:|---:|---:|", "| v1 (historical) | 30000 | 16.71 | 29.5594 | 75.2983 |"]
    for name, row in STATE["training"].items():
        rows.append(f"| {name} | {row['step']} | {row['minutes']:.2f} | {row['val_wiki_ppl']:.4f} | {row['val_colloquial_ppl']:.4f} |")
    rows += ["", "### Dev (400 runs per set)", "", "| Model / weight | Colloquial char % | Colloquial whole % | Wiki char % | Wiki whole % | Whole mean % |",
             "|---|---:|---:|---:|---:|---:|"]
    for name, result in STATE["dev"].items():
        c, w = result['dev_colloquial.txt'], result['dev_wikipedia.txt']
        label = name.removesuffix('-dev').replace('reranker_', '').replace('-w', ' / ')
        rows.append(f"| {label} | {c['char']:.1f} | {c['whole']:.2f} | {w['char']:.1f} | {w['whole']:.2f} | {mean(result, 'whole'):.3f} |")
    rows += ["", "### CPU latency", "", "| Model | First ms (excluded) | Every subsequent call, ms | Maximum ms | Pass <25 ms |", "|---|---:|---|---:|---|"]
    for name, row in STATE["latency"].items():
        calls = ", ".join(f"{x:.1f}" for x in row['calls'])
        rows.append(f"| {name} | {row['first']:.1f} | {calls} | {row['max']:.1f} | {'yes' if row['max'] < 25 else 'no'} |")
    rows += ["", "### Held-out test (once per model)", "", "| Model / weight | Set | Runs | Character % | Whole-run % |", "|---|---|---:|---:|---:|"]
    for name, result in STATE["test"].items():
        for name_set, row in result.items():
            rows.append(f"| {name.removesuffix('-test')} | {name_set} | {row['runs']} | {row['char']:.1f} | {row['whole']:.2f} |")
        rows.append(f"| {name.removesuffix('-test')} | **Equal-weight mean** | — | {mean(result, 'char'):.2f} | **{mean(result, 'whole'):.3f}** |")
    rows += ["", "### Decision", "", STATE.get("decision", "Pending completed dev and latency measurements. No held-out results used for tuning."),
             "", "Raw evaluation output, timings, copy/compile logs, manifest and machine details: `data/build/reranker_v2_experiment/`. Training logs: remote `logs/reranker_v2*-train.log` and local variant `train-log.json`.",
             "", "Character accuracy is printed by the existing evaluator to 0.1 percentage point. Whole-run counts are recovered unambiguously from the printed percentage and sample count (at most 400 runs), so whole-run means and the adoption gate use exact fractions. Latency uses the specified single typing sequence and excludes its first timed call. No broader workload, app UI or installation test was run.",
             "", "The full chronological execution log, including the recovered first-pass interruption, is `data/build/reranker_v2_experiment/experiment.log`. Only the clean frozen-resource measurements above are used for selection and adoption."]
    REPORT.write_text(prefix + "\n".join(rows) + "\n")


def launch_c():
    script = f"""set -eu
cd {REMOTE}
mkdir reranker_v2c
cp reranker/vocab.json reranker_v2c/vocab.json
for name in train.bin val_wiki.bin val_colloquial.bin; do ln -s ../reranker/$name reranker_v2c/$name; done
nohup sh -c {shlex.quote(f'''cd {REMOTE}
.venv-gen/bin/python -u training/reranker/train.py --data reranker_v2c --steps 90000 --layers 10 --width 640 --heads 10 --ffn 2560 > logs/reranker_v2c-train.log 2>&1
rc=$?; echo "$rc" > logs/reranker_v2c-train.exit
if [ "$rc" -ne 0 ]; then exit "$rc"; fi
.venv-gen/bin/python -u training/reranker/export_coreml.py --data reranker_v2c --out reranker_v2c/Reranker.mlpackage > logs/reranker_v2c-export.log 2>&1
rc=$?; echo "$rc" > logs/reranker_v2c-export.exit
exit "$rc"''')} > logs/reranker_v2c-jobs.log 2>&1 < /dev/null &
echo $! > logs/reranker_v2c-jobs.pid
"""
    run(SSH + [script], "v2c-launch.log")


def main():
    os.chdir(ROOT)
    for weight in [1.0, 0.7]:
        path = OUT / f"v1-w{weight:.1f}-dev.txt"
        while len(parse_eval(path.read_text())) != 2:
            event(f"Waiting for initial v1 weight {weight} dev evaluation")
            time.sleep(45)
        STATE["dev"][path.stem] = parse_eval(path.read_text())
    eval_model("v1", 1.4)
    # Initial v1 timing was run alone, before any local accuracy evaluations.
    calls = [float(x) for x in re.findall(r"\[rerank \d+ ([\d.]+) ms\]", (OUT / "v1-latency.txt").read_text())]
    assert len(calls) > 1
    STATE["latency"]["v1"] = {"first": calls[0], "calls": calls[1:], "max": max(calls[1:])}
    write_report()
    finish_variant("reranker_v2a")
    finish_variant("reranker_v2b")
    a, b = STATE["training"]["reranker_v2a"], STATE["training"]["reranker_v2b"]
    if all(b[f"val_{domain}_ppl"] <= 0.98 * a[f"val_{domain}_ppl"] for domain in ["wiki", "colloquial"]):
        event("v2b improves perplexity >=2% on both streams; training v2c.")
        launch_c()
        finish_variant("reranker_v2c")
    else:
        event("Skipped v2c: v2b does not improve perplexity >=2% on both streams.")
    select_and_test()


def select_and_test():
    candidates = []
    for label, rows in STATE["dev"].items():
        variant, weight = label.removesuffix("-dev").split("-w")
        if variant != "v1" and STATE["latency"][variant]["max"] < 25:
            candidates.append((mean(rows, "whole"), mean(rows, "char"), -STATE["latency"][variant]["max"],
                               -abs(float(weight) - 1), variant, float(weight)))
    if not candidates:
        STATE["decision"] = "Do not adopt: no v2 variant passes the CPU latency limit. Held-out evaluation was skipped because there is no acceptable dev winner."
    else:
        *_, winner, weight = max(candidates)
        STATE["winner"] = {"variant": winner, "weight": weight}
        event(f"Locked dev winner: {winner}, weight {weight}; held-out evaluation starts now.")
        manifest = json.loads((OUT / "manifest.json").read_text())
        for path, digest in manifest["frozen_evaluation_sha256"].items():
            assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest, f"Evaluation resource changed: {path}"
        winner_result = eval_model(winner, weight, heldout=True)
        baseline_result = eval_model("v1", 1.0, heldout=True)
        delta = mean(winner_result, "whole") - mean(baseline_result, "whole")
        if delta > 0:
            target = ROOT / "data/build/reranker_v2"
            target.mkdir(exist_ok=False)
            for name in ["Reranker.mlpackage", "Reranker.mlmodelc", "vocab.json"]:
                source = ROOT / "data/build" / winner / name
                if source.is_dir():
                    shutil.copytree(source, target / name)
                else:
                    shutil.copy2(source, target / name)
            (target / "config.json").write_text(json.dumps({"weight": weight, "count": 16}) + "\n")
            STATE["decision"] = f"Adopt {winner}, weight {weight}: held-out whole-run mean improves by {delta:.2f} percentage points versus shipped v1; slowest post-first CPU call {STATE['latency'][winner]['max']:.1f} ms. Winner artifacts are in data/build/reranker_v2/."
        else:
            STATE["decision"] = f"Do not adopt {winner}, weight {weight}: held-out whole-run mean difference is {delta:+.2f} percentage points versus shipped v1. No data/build/reranker_v2/ promotion."
    STATE["status"] = "complete"
    event(STATE["decision"])
    write_report()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--evaluate-only", action="store_true", help="Use completed training and frozen resources; do not retrain")
    args = parser.parse_args()
    if args.self_test:
        parsed = parse_eval("[dev_colloquial.txt] 400 runs, character accuracy 93.2%, whole-run accuracy 79.0%\n  miss\n[dev_wikipedia.txt] 400 runs, character accuracy 90.1%, whole-run accuracy 76.5%")
        assert len(parsed) == 2 and parsed["dev_colloquial.txt"]["runs"] == 400
        assert mean(parsed, "whole") == 77.75
        rounded = parse_eval("[dev.txt] 400 runs, character accuracy 96.3%, whole-run accuracy 76.2%")
        assert rounded['dev.txt']['whole_correct'] == 305 and rounded['dev.txt']['whole'] == 76.25
        assert not parse_eval("incomplete subprocess output")
        assert (max([24.9, 25.0]) < 25) is False
        print("self-test passed")
    else:
        try:
            if args.evaluate_only:
                STATE = json.loads((OUT / "results.json").read_text())
                STATE.update(status="running with frozen evaluation resources", decision="Pending clean dev re-evaluation.")
                STATE.pop("winner", None)
                assert not STATE["test"], "Held-out tests have already been evaluated."
                for variant in ["v1", "reranker_v2a", "reranker_v2b", "reranker_v2c"]:
                    for weight in WEIGHTS:
                        eval_model(variant, weight)
                    measure(variant)
                    write_report()
                select_and_test()
            else:
                main()
        except Exception as exc:
            STATE["status"] = "stopped after failure"
            STATE["decision"] = f"Experiment stopped without guessing: {type(exc).__name__}: {exc}. No adoption recommendation."
            event(STATE["decision"])
            write_report()
            raise

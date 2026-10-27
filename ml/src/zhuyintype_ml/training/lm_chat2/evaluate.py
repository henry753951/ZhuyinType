"""chat2 measurements on the dev sets: word perplexity of four models, and a conversion matrix of chat priors.

    perplexity   Wikipedia (lm/), the 0.3.0 chat model (lm_chat_v1/), chat2 mixed and chat2 real (no_generated/), all
                 on the same tokens from the frozen 0.3.0 chat tokenizer (inputs/tokenizer); EOS counted, BOS and
                 context not.  Dialogue lines score the target turn only, seeded with the previous turn's last word
                 when it ends in Han.  Writes perplexity.json and perplexity-lines.tsv.
    conversion   400 runs per dev set (seed 7) typed through zhuyin-cli with v2b top-16 and v3 at 0.5, for the
                 0.3.0 chat model and chat2 at chat priors 0.3/0.5/0.7.  One JSON per row in conversion/ (resumed;
                 a row whose command or inputs changed is refused), conversion-summary.json at the end.

Test splits are never scored.  GPU host only (refuses to run elsewhere).

    zt --remote train lm_chat2 perplexity
    python -m zhuyintype_ml.training.lm_chat2.evaluate --phase conversion --jobs 60 -n 400
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import random
import re
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

import numpy as np

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.zhuyin import VARIANTS, Lexicon, keys
from zhuyintype_ml.training.lm import mkn
from zhuyintype_ml.training.lm.files import digest, save_json
from zhuyintype_ml.training.lm_chat.train import normalized

from .prepare import OUT

# (name, file, type the text before the run as context)
SETS = [
    ("dev_colloquial", paths.data("corpus", "dev_colloquial.txt"), False),
    ("dev_chat_tt", paths.data("lm_chat", "dev_chat_tt.txt"), False),
    ("dev_wikipedia", paths.data("corpus", "dev_wikipedia.txt"), True),
    ("dev_dialogue", OUT / "dev_dialogue.txt", True),
]
# The chat models compared: the 0.3.0 model (data/build/lm_chat when this ran) and chat2 mixed.
CHAT_MODELS = {"lm_chat": paths.data("lm_chat_v1"), "lm_chat2": OUT}


def candidates(path: Path, context: bool) -> list[tuple[str, str]]:
    """Han runs of 4–20 characters with the 32 characters before them, shuffled with seed 7.  Dialogue runs come from
    the target turn, with the previous turn as the text before."""
    found = []
    for line in path.read_text().splitlines():
        previous, target = line.split("\t", 1) if path.name == "dev_dialogue.txt" else ("", line)
        for m in re.finditer(r"[一-鿿]{4,20}", target):
            before = (
                (previous + (" " if m.start() else "") + target[: m.start()])[-32:]
                if previous
                else target[: m.start()][-32:]
            )
            found.append((m.group(), before if context else ""))
    random.Random(7).shuffle(found)
    if path.name != "dev_dialogue.txt":
        # Match the conversion evaluation exactly, including its duplicate-run context lookup.
        last = dict(found)
        found = [(run, last[run]) for run, _ in found]
    return found


def selected_inputs(path: Path, context: bool, lexicon: Lexicon, n: int) -> tuple[list[str], list[str]]:
    """The first n runs with readings, and their key scripts (context, a tab, keys, Enter)."""
    selected, scripts = [], []
    for run, previous in candidates(path, context):
        readings = lexicon.readings(run)
        if not readings:
            continue
        selected.append(run)
        prefix = previous.replace("\t", " ").replace("\n", " ") + "\t" if context else ""
        scripts.append(prefix + keys(readings) + "<enter>")
        if len(selected) == n:
            break
    if not selected:
        raise ValueError(f"no eligible runs: {path}")
    return selected, scripts


def run_batch(command: list[str], scripts: list[str], failure: Path) -> subprocess.CompletedProcess[str]:
    """One zhuyin-cli batch; on failure the command, inputs and output are saved to `failure`."""
    result = subprocess.run(command, input="".join(s + "\n" for s in scripts), text=True, capture_output=True)
    if result.returncode:
        failure.parent.mkdir(parents=True, exist_ok=True)
        save_json(
            failure,
            {
                "command": command,
                "inputs": scripts,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            },
        )
        raise RuntimeError(f"CLI returncode={result.returncode}; batch and diagnostics saved to {failure}")
    return result


def run_parallel(
    command: list[str], scripts: list[str], jobs: int, failures: Path
) -> list[subprocess.CompletedProcess[str]]:
    """The scripts split over `jobs` batches in order; a failed batch i is saved as `failures`-i.json."""
    jobs = min(jobs, len(scripts))
    size = -(-len(scripts) // jobs)

    def run(i: int) -> subprocess.CompletedProcess[str]:
        return run_batch(command, scripts[i : i + size], failures.with_name(f"{failures.name}-{i}.json"))

    with ThreadPoolExecutor(jobs) as pool:
        return list(pool.map(run, range(0, len(scripts), size)))


def conversion(a: argparse.Namespace) -> None:
    lexicon_path = paths.data("lexicon.tsv")
    lexicon = Lexicon(lexicon_path)
    cases = {name: selected_inputs(path, context, lexicon, a.n) for name, path, context in SETS}
    out = OUT / "conversion"
    out.mkdir(exist_ok=True)
    rows = []
    base = [a.cli, "--lexicon", str(lexicon_path), "--typo", "standard", "--batch"]
    base += ["--reranker", str(paths.data("reranker")), "--rerank-count", "16"]
    base += ["--v3", str(paths.data("v3")), "--v3-weight", ".5"]
    base += a.cli_arg
    for model, directory in CHAT_MODELS.items():
        for prior in (0.3, 0.5, 0.7):
            for name, _, _ in SETS:
                target = out / f"{model}-{prior}-{name}.json"
                wanted, scripts = cases[name]
                command = base + ["--chat-lm", str(directory), "--chat-prior", str(prior)]
                config = {
                    "command": command,
                    "input_sha256": hashlib.sha256(("\n".join(scripts) + "\n").encode()).hexdigest(),
                    "cli_sha256": digest(Path(a.cli)),
                    "lexicon_sha256": digest(lexicon_path),
                }
                if target.exists():
                    record = json.loads(target.read_text())
                    if record["config"] != config:
                        raise ValueError(f"stale result: {target}")
                    rows.append(record)
                    continue
                started = time.monotonic()
                results = run_parallel(command, scripts, a.jobs, out / "failures" / f"{model}-{prior}-{name}")
                outputs = [s for result in results for s in result.stdout.splitlines()]
                if len(outputs) != len(wanted):
                    raise ValueError("CLI output count mismatch")
                exact = sum(w == g.strip() for w, g in zip(wanted, outputs))
                variant = sum(w.translate(VARIANTS) == g.strip().translate(VARIANTS) for w, g in zip(wanted, outputs))
                characters = sum(sum(x == y for x, y in zip(w, g.strip())) for w, g in zip(wanted, outputs))
                record = {
                    "model": model,
                    "prior": prior,
                    "set": name,
                    "runs": len(wanted),
                    "whole_correct": exact,
                    "whole_percent": 100 * exact / len(wanted),
                    "variant_correct": variant,
                    "variant_percent": 100 * variant / len(wanted),
                    "character_percent": 100 * characters / sum(map(len, wanted)),
                    "seconds": time.monotonic() - started,
                    "config": config,
                    "targets": wanted,
                    "inputs": scripts,
                    "outputs": outputs,
                    "stderr": [r.stderr for r in results if r.stderr],
                }
                save_json(target, record)
                rows.append(record)
                print(
                    f"{model} P={prior} {name}: {exact}/{len(wanted)} ({record['whole_percent']:.2f}%), "
                    f"variants {record['variant_percent']:.2f}%, {record['seconds']:.1f}s",
                    flush=True,
                )
    save_json(
        OUT / "conversion-summary.json",
        [{k: v for k, v in row.items() if k not in ("targets", "inputs", "outputs", "stderr")} for row in rows],
    )


def perplexity() -> None:
    # Freeze the OLD chat final-E tokenizer for all models to preserve historical comparability.
    tok = OUT / "inputs/tokenizer"
    words = json.loads((tok / "vocabulary.json").read_text())
    scores = np.load(tok / "segmentation_scores.npy")
    _, _, old = mkn.read_lexicon(OUT / "inputs/lexicon.tsv")
    ids = {w: i for i, w in enumerate(words)}
    trie = mkn.make_trie(words, old)

    def tokens(text: str) -> list[str]:
        return [words[i] for i in mkn.segment(normalized(text), trie, ids, scores, float(scores[ids["<unk>"]]))]

    tokenized: dict[str, list[tuple[str, list[tuple[str, str]]]]] = {}
    for name, path, _ in SETS:
        tokenized[name] = []
        for line in path.read_text().splitlines():
            previous, target = line.split("\t", 1) if name == "dev_dialogue" else ("", line)
            prior = tokens(previous) if previous and re.search(r"[一-鿿]$", previous) else []
            target_words = tokens(target)
            seq = [prior[-1] if prior else "<s>", *target_words, "</s>"]
            tokenized[name].append((target, list(zip(seq, seq[1:]))))
    models = {
        "wiki": paths.data("lm"),
        "lm_chat": CHAT_MODELS["lm_chat"],
        "lm_chat2": OUT,
        "no_generated": OUT / "no_generated",
    }
    metrics: dict[str, dict[str, Any]] = {
        name: {"lines": len(rows), "predictions_with_eos": sum(len(events) for _, events in rows), "models": {}}
        for name, rows in tokenized.items()
    }
    with (OUT / "perplexity-lines.tsv").open("w") as lines:
        lines.write("model\tset\tline\tpredictions_with_eos\tlog_likelihood\n")
        for model_name, directory in models.items():
            model = mkn.TsvModel(directory)
            for name, rows in tokenized.items():
                total = 0.0
                unknown = 0
                for i, (_, events) in enumerate(rows, 1):
                    ll = math.fsum(model.score(a, b) for a, b in events)
                    total += ll
                    unknown += sum(b not in model.uni or b == "<unk>" for _, b in events)
                    lines.write(f"{model_name}\t{name}\t{i}\t{len(events)}\t{ll:.12f}\n")
                metrics[name]["models"][model_name] = {
                    "perplexity": math.exp(-total / metrics[name]["predictions_with_eos"]),
                    "log_likelihood": total,
                    "unknown_targets_with_eos": unknown,
                }
            del model
    save_json(
        OUT / "perplexity.json",
        {
            "protocol": "Same old chat final E-step tokens; natural logs, EOS included, BOS/context not scored; "
            "dialogue target only, previous final word seeds context only if no trailing boundary punctuation.",
            "tokenizer_sha256": {p.name: digest(p) for p in tok.iterdir()},
            "sets": metrics,
        },
    )
    print(json.dumps(metrics, ensure_ascii=False, indent=2), flush=True)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--phase", choices=["perplexity", "conversion", "all"], default="all")
    p.add_argument("--cli", default=None, help="zhuyin-cli (default: the release build, built when missing)")
    p.add_argument("--jobs", type=int, default=60)
    p.add_argument("--cli-arg", action="append", default=[], help="extra zhuyin-cli argument (repeatable)")
    p.add_argument("-n", type=int, default=400)
    a = p.parse_args(argv)
    if platform.system() != "Linux":
        raise SystemExit("Evaluation must run on the remote Linux host, never on the Mac.")
    if a.phase in ("perplexity", "all"):
        perplexity()
    if a.phase in ("conversion", "all"):
        a.cli = a.cli or str(paths.cli())
        conversion(a)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

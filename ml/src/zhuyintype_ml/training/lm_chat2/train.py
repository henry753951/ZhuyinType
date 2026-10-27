"""chat2 word bigram: the Wikipedia model's three-pass hard EM and modified Kneser-Ney, plus turn-boundary bigrams.

Each clean adjacent turn pair adds one bigram from the previous turn's last word to the next turn's first word;
sentences inside a turn keep their own BOS/EOS events.  Writes unigram.tsv, bigram.tsv, backoff.tsv (at most
--max-bytes together), validation.json (normalization and a leakage rescan) and stats.json into data/build/lm_chat2
(--variant mixed) or lm_chat2/no_generated (--variant real, the model 0.4.0 ships).  Refuses changed inputs or code
in an existing work directory.  GPU host (24 workers).

    zt --remote train lm_chat2 real mixed
    python -m zhuyintype_ml.training.lm_chat2.train --variant real --workers 24
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import time
from collections import Counter
from pathlib import Path
from typing import Any

import numpy as np

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.lm import mkn
from zhuyintype_ml.training.lm.files import digest, save_json
from zhuyintype_ml.training.lm_chat import train as chat

from .prepare import OUT, blocked_text, evaluation_files, excluded, normalize


def audit(work: Path) -> dict[str, Any]:
    """Rescans every training sentence and turn pair against all holdouts."""
    files = evaluation_files() + [OUT / "dev_dialogue.txt", OUT / "test_dialogue.txt"]
    held = blocked_text(files)
    held.update(normalize(s).replace("臺", "台") for s in list(held))
    machine = mkn.automaton(held)
    counts: Counter[str] = Counter()
    for path in sorted((work / "sentences").glob("*.txt")) + [work / "context_pairs.tsv"]:
        with path.open() as f:
            for line in f:
                for text in line.rstrip("\n").split("\t"):
                    assert not excluded(text, held, machine), (path, text)
                    assert not excluded(normalize(text).replace("臺", "台"), held, machine), (path, text)
                    counts["texts_rescanned"] += 1
    return {
        **counts,
        "exact_or_eight_character_overlap": 0,
        "evaluation_files": {paths.relative(p): digest(p) for p in files},
    }


def boundary_counts(
    work: Path, words: list[str], old: dict[str, float], keys: np.ndarray, counts: np.ndarray
) -> tuple[np.ndarray, np.ndarray, int]:
    """The pair counts plus one (last word of the previous turn, first word of the next) event per turn pair."""
    ids = {w: i for i, w in enumerate(words)}
    trie = mkn.make_trie(words, old)
    scores = np.load(work / "segmentation_scores.npy")
    extra: Counter[int] = Counter()
    for line in (work / "context_pairs.tsv").open():
        previous, target = line.rstrip("\n").split("\t")
        a = re.findall(r"[\u4e00-\u9fff]+", previous)
        b = re.findall(r"[\u4e00-\u9fff]+", target)
        if not a or not b:
            continue
        left = mkn.segment(a[-1], trie, ids, scores, float(scores[2]))
        right = mkn.segment(b[0], trie, ids, scores, float(scores[2]))
        if left and right:
            extra[(left[-1] << 32) | right[0]] += 1
    all_keys = np.r_[keys, np.array(list(extra), dtype=np.uint64)]
    all_counts = np.r_[counts, np.array(list(extra.values()), dtype=np.uint64)]
    order = np.argsort(all_keys)
    all_keys, all_counts = all_keys[order], all_counts[order]
    starts = np.r_[0, np.flatnonzero(all_keys[1:] != all_keys[:-1]) + 1]
    return all_keys[starts], np.add.reduceat(all_counts, starts), sum(extra.values())


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--variant", choices=["mixed", "real"], default="mixed")
    p.add_argument("--workers", type=int, default=24)
    p.add_argument("--max-bytes", type=int, default=60_000_000)
    a = p.parse_args(argv)
    started = time.monotonic()
    a.output = OUT if a.variant == "mixed" else OUT / "no_generated"
    a.work = a.output / "work"
    prepared = json.loads((a.work / "prepared.json").read_text())
    files = sorted((a.work / "sentences").glob("*.txt")) + [a.work / "context_pairs.tsv", OUT / "inputs/lexicon.tsv"]
    config = {
        "workers": a.workers,
        "variant": a.variant,
        "max_bytes": a.max_bytes,
        "inputs": {paths.relative(p): digest(p) for p in files},
        "code": {
            paths.relative(p): digest(p)
            for p in [*Path(__file__).parent.glob("*.py"), Path(mkn.__file__), Path(chat.__file__)]
        },
    }
    checkpoint = a.work / "train-config.json"
    if checkpoint.exists() and json.loads(checkpoint.read_text()) != config:
        raise ValueError("frozen inputs/code changed; choose a fresh work tree")
    save_json(checkpoint, config)
    _, _, old = mkn.read_lexicon(OUT / "inputs/lexicon.tsv")
    words, values, timings = mkn.em(a, old, prepared)
    keys, counts = mkn.merge_pairs(a)
    keys, counts, boundaries = boundary_counts(a.work, words, old, keys, counts)
    cap = 1_600_000
    while True:
        model = mkn.fit_mkn(keys, counts, len(words), cap)
        mkn.write_tsv(a.output, words, model)
        size = sum((a.output / f"{n}.tsv").stat().st_size for n in ("unigram", "bigram", "backoff"))
        if size <= a.max_bytes:
            break
        cap = int(len(model[1]) * a.max_bytes / size * 0.9)
        if cap < 1:
            raise ValueError("vocabulary exceeds budget")
    validation = mkn.TsvModel(a.output).verify()
    validation["leakage"] = audit(a.work)
    save_json(a.output / "validation.json", validation)
    save_json(
        a.output / "stats.json",
        {
            "status": "validated",
            "created_utc": datetime.datetime.now(datetime.UTC).isoformat(),
            "corpus": {k: v for k, v in prepared.items() if k != "training_characters"},
            "model": {
                **model[4],
                "tsv_bytes": size,
                "unigram_types": len(words) - 1,
                "word_tokens": int(values.sum()),
                "supplemental_turn_boundary_events": boundaries,
            },
            "segmentation": timings,
            "config": config,
            "python": platform.python_version(),
            "numpy": np.__version__,
            "seconds": time.monotonic() - started,
            "files": {n: digest(a.output / n) for n in ("unigram.tsv", "bigram.tsv", "backoff.tsv")},
        },
    )
    print("VALIDATED", a.variant, size, "bytes", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

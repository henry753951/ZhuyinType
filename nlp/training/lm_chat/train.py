"""The 0.3.0 chat word bigram: Taiwan Tongues holdouts, the v3 colloquial corpus, the Wikipedia estimator.

Selects the Taiwan Tongues dev/test holdouts (dev_chat_tt.txt, test_chat_tt.txt), trains on the 0.3.0 v3 colloquial
corpus (Qwen3-8B sentences and Common Voice) plus the remaining Taiwan Tongues text with the same three-pass hard EM
and modified Kneser-Ney as the Wikipedia model, then scores both models on the dev sets (line_scores.tsv) and
compares sentence starts.  Frozen inputs are read from --inputs (default OUTPUT/inputs); checkpoints go to
OUTPUT/work.  GPU host (CPU only, 48 workers).

Retired: 0.4.0 ships the chat2 model in lm_chat/ (zt train lm_chat2 ship), and the 0.3.0 TSVs were archived as
lm_chat_v1/.  Training again writes into --output (default data/build/lm_chat), so point it elsewhere.

    zt --remote train lm_chat train --option output=lm_chat_rerun
    python -m nlp.training.lm_chat.train --output data/build/lm_chat_rerun --inputs data/build/lm_chat/inputs
"""

from __future__ import annotations

import argparse
import csv
import datetime
import importlib.metadata
import json
import math
import platform
import random
import re
import time
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import numpy as np

from nlp.common import paths
from nlp.training.lm import mkn
from nlp.training.lm.files import digest, log, save_json

SEED = 20261027
HELD = ("test_sentences.txt", "test_colloquial.txt", "dev_colloquial.txt", "dev_wikipedia.txt")
SPLITS = ("test_chat_tt.txt", "dev_chat_tt.txt")


def normalized(text: str) -> str:
    """NFC with runs of whitespace collapsed to one space."""
    return " ".join(unicodedata.normalize("NFC", text).split())


def read_tt(path: Path) -> tuple[set[str], dict[str, int]]:
    """Unique zh-TW Taiwan Tongues sentences (at least two Han characters, no Latin run over three letters).  The TSV
    is read only through csv.DictReader; audio paths and participant metadata are ignored."""
    counts: Counter[str] = Counter()
    sentences = set()
    with path.open(encoding="utf-8", newline="") as f:
        rows = csv.DictReader(f, delimiter="\t")
        if not {"sentence", "locale"} <= set(rows.fieldnames or []):
            raise ValueError(f"missing TSV columns: {path}")
        for row in rows:
            counts["rows"] += 1
            if None in row or row.get("sentence") is None or row.get("locale") is None:
                raise ValueError(f"malformed TSV row {rows.line_num}: {path}")
            if row["locale"] != "zh-TW":
                counts["other_locale"] += 1
                continue
            s = normalized(row["sentence"])
            if re.search(r"[A-Za-z]{4,}", s) or len(mkn.HAN.findall(s)) < 2:
                counts["filtered"] += 1
                continue
            if any(unicodedata.category(c) == "Cc" for c in s):
                counts["control_filtered"] += 1
                continue
            sentences.add(s)
            counts["accepted_rows"] += 1
    counts["unique"] = len(sentences)
    counts["duplicate_rows"] = counts["accepted_rows"] - len(sentences)
    return sentences, dict(counts)


def grams(s: str) -> set[str]:
    """Every 8-character substring."""
    return {s[i : i + 8] for i in range(len(s) - 7)}


def select_holdouts(
    sentences: set[str], commonvoice: set[str], held: set[str], size: int = 1500
) -> tuple[list[str], list[str], int]:
    """Seeded test and dev holdouts of `size` sentences that share no 8-gram with each other or with `held`."""
    candidates = sorted(sentences - commonvoice - held)
    random.Random(SEED).shuffle(candidates)
    used = set().union(*(grams(s) for s in held))
    selected: list[str] = []
    for s in candidates:
        g = grams(s)
        if g & used:
            continue
        selected.append(s)
        used.update(g)
        if len(selected) == 2 * size:
            return sorted(selected[:size]), sorted(selected[size:]), len(candidates)
    raise ValueError(f"only {len(selected)} independent holdouts; need {2 * size}")


def prepare(args: argparse.Namespace) -> dict[str, Any]:
    """Freezes the holdouts, then shards the training sentences with every holdout and its 8-grams removed."""
    inputs, work, out = args.inputs, args.work, args.output
    marker = work / "prepared.json"
    if marker.exists():
        return dict(json.loads(marker.read_text()))
    held = {normalized(s) for name in HELD for s in (inputs / name).read_text().splitlines() if s.strip()}
    cv = {normalized(s) for s in (inputs / "train_commonvoice.txt").read_text().splitlines()}
    tt: set[str] = set()
    sources = {}
    for name in ("train.tsv", "test.tsv"):
        sentences, sources[name] = read_tt(inputs / name)
        tt.update(sentences)
    test, dev, candidates = select_holdouts(tt, cv, held)
    for name, rows in zip(SPLITS, (test, dev)):
        (out / name).write_text("\n".join(rows) + "\n")
    save_json(
        out / "splits.json",
        {
            "seed": SEED,
            "eligible_unique": candidates,
            "tt_unique": len(tt),
            "sources": sources,
            "test": len(test),
            "dev": len(dev),
            "excluded_train_commonvoice_exact": len(tt & cv),
            "policy": "NFC + whitespace; zh-TW; >=2 Han; no Latin run >3; unique; exclude train_commonvoice "
            "equality; disjoint 8-grams within/between new holdouts and against old dev/test",
            "paths": {name: paths.relative(out / name) for name in SPLITS},
            "sha256": {name: digest(out / name) for name in SPLITS},
        },
    )
    held.update(test + dev)
    machine = mkn.automaton(held)
    directory = work / "sentences"
    directory.mkdir(exist_ok=True)
    handles = [(directory / f"{i:03d}.txt").open("w") for i in range(args.workers)]
    seen: set[str] = set()
    chars: set[str] = set()
    counts: Counter[str] = Counter()
    try:
        with args.colloquial.open() as colloquial:
            for source, rows in (("v3", colloquial), ("tt", sorted(tt))):
                for row in rows:
                    s = normalized(row)
                    counts[source + "_input"] += 1
                    if not s or s in held or next(machine.iter(s), None) is not None:
                        counts[source + "_heldout_removed"] += 1
                        continue
                    if s in seen:
                        counts[source + "_duplicates_removed"] += 1
                        continue
                    if "\t" in s or any(unicodedata.category(c) == "Cc" for c in s):
                        raise ValueError("control character in training sentence")
                    seen.add(s)
                    handles[counts["training_sentences"] % len(handles)].write(s + "\n")
                    counts["training_sentences"] += 1
                    counts[source + "_kept"] += 1
                    counts["training_chars"] += len(s)
                    chars.update(s)
    finally:
        for f in handles:
            f.close()
    result = {**counts, "training_characters": sorted(chars), "tt_sources": sources}
    save_json(marker, result)
    log(f"prepared: {dict(counts)}")
    return result


def distribution(values: list[float]) -> dict[str, Any]:
    v = np.asarray(values)
    return {
        "count": len(v),
        "mean": float(v.mean()),
        "std": float(v.std()),
        "quantiles": dict(
            zip(
                ("min", "p05", "p25", "p50", "p75", "p95", "max"),
                map(float, np.quantile(v, [0, 0.05, 0.25, 0.5, 0.75, 0.95, 1])),
            )
        ),
        "positive_fraction": float(np.mean(v > 0)),
    }


def evaluate(
    args: argparse.Namespace, old: dict[str, float], words: list[str], models: dict[str, mkn.TsvModel]
) -> dict[str, Any]:
    """Dev perplexity of both models on the same frozen chat tokens, and the per-line log-likelihood difference."""
    ids = {w: i for i, w in enumerate(words)}
    trie = mkn.make_trie(words, old)
    scores = np.load(args.work / "segmentation_scores.npy")
    metrics = {}
    with (args.output / "line_scores.tsv").open("w") as f:
        f.write(
            "dataset\tline\twords\tpredictions_with_eos\twikipedia_logp\tchat_logp\tdelta_chat_minus_wiki"
            "\tdelta_per_prediction\tsentence\n"
        )
        for name in ("dev_wikipedia", "dev_colloquial", "dev_chat_tt"):
            path = (args.output if name == "dev_chat_tt" else args.inputs) / (name + ".txt")
            totals: Counter[str] = Counter()
            likelihood: defaultdict[str, float] = defaultdict(float)
            unknown: Counter[str] = Counter()
            deltas, per_word = [], []
            with path.open() as lines:
                for n, line in enumerate(lines, 1):
                    sentence = normalized(line)
                    unk = float(scores[ids["<unk>"]])
                    tokens = [words[i] for i in mkn.segment(sentence, trie, ids, scores, unk)]
                    sequence = ["<s>", *tokens, "</s>"]
                    ll = {}
                    for domain, model in models.items():
                        ll[domain] = math.fsum(model.score(a, b) for a, b in zip(sequence, sequence[1:]))
                        likelihood[domain] += ll[domain]
                        unknown[domain] += sum(w not in model.uni or w == "<unk>" for w in tokens)
                    predictions = len(tokens) + 1
                    delta = ll["chat"] - ll["wikipedia"]
                    deltas.append(delta)
                    per_word.append(delta / predictions)
                    totals["words"] += len(tokens)
                    totals["predictions"] += predictions
                    f.write(
                        f"{name}\t{n}\t{len(tokens)}\t{predictions}\t{ll['wikipedia']:.12f}\t{ll['chat']:.12f}"
                        f"\t{delta:.12f}\t{delta / predictions:.12f}\t{sentence}\n"
                    )
            metrics[name] = {
                "sentences": len(deltas),
                "words": totals["words"],
                "predictions_with_eos": totals["predictions"],
                "models": {
                    domain: {
                        "perplexity": math.exp(-likelihood[domain] / totals["predictions"]),
                        "log_likelihood": likelihood[domain],
                        "unknown_words": unknown[domain],
                        "unknown_rate": unknown[domain] / totals["words"],
                    }
                    for domain in models
                },
                "line_delta_chat_minus_wikipedia": distribution(deltas),
                "delta_per_prediction": distribution(per_word),
            }
    return metrics


def reading_comparison(
    args: argparse.Namespace,
    rows: list[tuple[str, str, float]],
    old: dict[str, float],
    words: list[str],
    counts: np.ndarray,
    models: dict[str, mkn.TsvModel],
) -> list[dict[str, Any]]:
    """Each model's most probable sentence-initial word for the 30 most frequent two-syllable readings."""
    word_counts = dict(zip(words, map(int, counts)))
    frequencies: Counter[str] = Counter()
    candidates: defaultdict[str, set[str]] = defaultdict(set)
    for reading, w, logp in rows:
        if len(reading.split()) != 2:
            continue
        candidates[reading].add(w)
        frequencies[reading] += word_counts.get(w, 0) * math.exp(logp) / old[w]  # type: ignore[assignment]
    chosen = [r for r, _ in frequencies.most_common(30)]
    example = "ㄏㄨㄛˋ ㄕˋ"
    if example in candidates and example not in chosen:
        chosen.append(example)
    result = []
    for rank, r in enumerate(chosen, 1):
        tops = {}
        for domain, model in models.items():
            w = max(sorted(candidates[r]), key=lambda w: model.score("<s>", w))
            tops[domain] = {"word": w, "logp_bos": model.score("<s>", w)}
        result.append(
            {
                "rank": rank if rank <= 30 else None,
                "reading": r,
                "estimated_training_occurrences": frequencies[r],
                **tops,
            }
        )
    save_json(args.output / "sentence_starts.json", result)
    return result


def audit(args: argparse.Namespace) -> dict[str, int]:
    """Rescans every training sentence against all holdouts after export."""
    held = {normalized(s) for name in HELD for s in (args.inputs / name).read_text().splitlines()}
    test, dev = [set((args.output / name).read_text().splitlines()) for name in SPLITS]
    cv = {normalized(s) for s in (args.inputs / "train_commonvoice.txt").read_text().splitlines()}
    assert len(test) == len(dev) == 1500 and not test & dev and not (test | dev) & cv
    test_grams = set().union(*(grams(s) for s in test))
    assert all(not grams(s) & test_grams for s in dev)
    held.update(test | dev)
    machine = mkn.automaton(held)
    seen = set()
    for path in sorted((args.work / "sentences").glob("*.txt")):
        with path.open() as lines:
            for line in lines:
                s = line.rstrip("\n")
                assert s not in held and next(machine.iter(s), None) is None
                assert s not in seen
                seen.add(s)
    return {
        "training_sentences_rescanned": len(seen),
        "exact_overlap": 0,
        "eight_character_overlap": 0,
        "training_duplicates": 0,
        "holdouts_each": 1500,
        "new_test_dev_eight_character_overlap": 0,
    }


PROVENANCE = (
    "NOTICE.md",
    "Taiwan-Tongues-LICENSE",
    "Taiwan-Tongues-README.md",
    "Qwen3-8B-LICENSE",
    "source_receipts.json",
    "transfer_receipt.json",
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, default=paths.data("lm_chat"))
    parser.add_argument("--inputs", type=Path, help="frozen inputs (default: OUTPUT/inputs)")
    parser.add_argument(
        "--colloquial",
        type=Path,
        default=paths.data("v3_030", "colloquial.txt"),
        help="the 0.3.0 v3 colloquial corpus (data/build/v3/ when this model was trained)",
    )
    parser.add_argument("--workers", type=int, default=48)
    parser.add_argument("--max-bytes", type=int, default=60_000_000, help="budget of the three TSVs")
    args = parser.parse_args(argv)
    start = time.monotonic()
    args.output = args.output.resolve()
    args.work = args.output / "work"
    args.inputs = (args.inputs or args.output / "inputs").resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    args.work.mkdir(exist_ok=True)
    inputs = sorted(p for p in args.inputs.rglob("*") if p.is_file()) + [args.colloquial]
    config = {
        "seed": SEED,
        "workers": args.workers,
        "max_tsv_bytes": args.max_bytes,
        "inputs": {paths.relative(p): {"sha256": digest(p), "bytes": p.stat().st_size} for p in inputs},
        "code": {paths.relative(p): digest(p) for p in (Path(__file__).resolve(), Path(mkn.__file__).resolve())},
    }
    checkpoint = args.work / "config.json"
    if checkpoint.exists() and json.loads(checkpoint.read_text()) != config:
        raise ValueError("inputs/code/config changed; use a fresh work directory (no stale checkpoints)")
    save_json(checkpoint, config)
    prepared = prepare(args)
    _, rows, old = mkn.read_lexicon(args.inputs / "lexicon.tsv")
    words, counts, timings = mkn.em(args, old, prepared)
    keys, pair_counts = mkn.merge_pairs(args)
    cap = 1_600_000
    while True:
        model = mkn.fit_mkn(keys, pair_counts, len(words), cap)
        mkn.write_tsv(args.output, words, model)
        size = sum((args.output / name).stat().st_size for name in ("unigram.tsv", "bigram.tsv", "backoff.tsv"))
        if size <= args.max_bytes:
            break
        cap = int(len(model[1]) * args.max_bytes / size * 0.9)
        if cap < 1:
            raise ValueError("TSV budget too small for vocabulary")
    model_stats = model[4]
    del model, keys, pair_counts
    models = {"wikipedia": mkn.TsvModel(args.inputs / "wiki"), "chat": mkn.TsvModel(args.output)}
    verification = models["chat"].verify()
    evaluation = evaluate(args, old, words, models)
    comparisons = reading_comparison(args, rows, old, words, counts, models)
    verification["leakage"] = audit(args)
    save_json(args.output / "validation.json", verification)
    for name in PROVENANCE:
        (args.output / name).write_bytes((args.inputs / name).read_bytes())
    stats = {
        "status": "validated",
        "created_utc": datetime.datetime.now(datetime.UTC).isoformat(),
        "config": config,
        "corpus": {k: v for k, v in prepared.items() if k != "training_characters"},
        "model": {**model_stats, "unigram_types": len(words) - 1, "tsv_bytes": size},
        "segmentation": {
            "method": "pipeline.py initial lexicon Viterbi + two hard-EM updates; freeze final E step for BOTH models",
            "passes": timings,
        },
        "evaluation_protocol": "Natural logs; word perplexity includes EOS, excludes BOS. Same frozen chat word "
        "tokens for both models; per-line delta = logP(chat)-logP(wikipedia). Test held out, never scored/tuned. "
        "Novel targets AND contexts map to <unk>; absent known context backoff=0.",
        "evaluation": evaluation,
        "sentence_start_readings": comparisons,
        "validation": verification,
        "reading_frequency_protocol": "Final chat segmentation word counts, apportioned among readings by "
        "normalized seed reading mass; top word maximizes word P(w|<s>) among exact-reading lexicon entries, "
        "without reading-prior multiplication.",
        "runtime": {
            "python": platform.python_version(),
            "platform": platform.platform(),
            "packages": {
                p: importlib.metadata.version(p) for p in ("numpy", "opencc", "mwparserfromhell", "pyahocorasick")
            },
        },
        "seconds": time.monotonic() - start,
    }
    stats["files"] = {
        p.name: {"bytes": p.stat().st_size, "sha256": digest(p)}
        for p in args.output.iterdir()
        if p.is_file() and p.name != "stats.json"
    }
    save_json(args.output / "stats.json", stats)
    log("VALIDATED")
    print("Dataset | Lines | Wikipedia PPL | Chat PPL | Mean line delta", flush=True)
    for name, v in evaluation.items():
        print(
            f"{name} | {v['sentences']} | {v['models']['wikipedia']['perplexity']:.3f} | "
            f"{v['models']['chat']['perplexity']:.3f} | {v['line_delta_chat_minus_wikipedia']['mean']:.3f}",
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Chat MKN bigram; reuse the existing Viterbi, hard EM and MKN implementation."""
import argparse
from collections import Counter, defaultdict
import csv
import datetime
import importlib.metadata
import json
import math
from pathlib import Path
import platform
import random
import re
import sys
import time
import unicodedata

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lm"))
import pipeline as lm
import numpy as np

SEED = 20261027
HELD = ("test_sentences.txt", "test_colloquial.txt", "dev_colloquial.txt", "dev_wikipedia.txt")
SPLITS = ("test_chat_tt.txt", "dev_chat_tt.txt")


def normalized(text):
    return " ".join(unicodedata.normalize("NFC", text).split())


def read_tt(path):
    counts = Counter()
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
            if re.search(r"[A-Za-z]{4,}", s) or len(lm.HAN.findall(s)) < 2:
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


def grams(s):
    return {s[i:i + 8] for i in range(len(s) - 7)}


def select_holdouts(sentences, commonvoice, held, size=1500):
    candidates = sorted(sentences - commonvoice - held)
    random.Random(SEED).shuffle(candidates)
    used = set().union(*(grams(s) for s in held))
    selected = []
    for s in candidates:
        g = grams(s)
        if g & used:
            continue
        selected.append(s)
        used.update(g)
        if len(selected) == 2 * size:
            return sorted(selected[:size]), sorted(selected[size:]), len(candidates)
    raise ValueError(f"only {len(selected)} independent holdouts; need {2 * size}")


def prepare(args):
    inputs, work, out = args.inputs, args.work, args.output
    marker = work / "prepared.json"
    if marker.exists():
        return json.loads(marker.read_text())
    held = {normalized(s) for name in HELD for s in (inputs / name).read_text().splitlines() if s.strip()}
    cv = {normalized(s) for s in (inputs / "train_commonvoice.txt").read_text().splitlines()}
    tt = set()
    sources = {}
    for name in ("train.tsv", "test.tsv"):
        sentences, sources[name] = read_tt(inputs / name)
        tt.update(sentences)
    test, dev, candidates = select_holdouts(tt, cv, held)
    for name, rows in zip(SPLITS, (test, dev)):
        (out / name).write_text("\n".join(rows) + "\n")
    lm.save_json(out / "splits.json", {
        "seed": SEED, "eligible_unique": candidates, "tt_unique": len(tt), "sources": sources,
        "test": len(test), "dev": len(dev), "excluded_train_commonvoice_exact": len(tt & cv),
        "policy": "NFC + whitespace; zh-TW; >=2 Han; no Latin run >3; unique; exclude train_commonvoice equality; disjoint 8-grams within/between new holdouts and against old dev/test",
        "paths": {name: str((out / name).relative_to(args.root)) for name in SPLITS},
        "sha256": {name: lm.digest(out / name) for name in SPLITS},
    })
    held.update(test + dev)
    machine = lm.automaton(held)
    directory = work / "sentences"
    directory.mkdir(exist_ok=True)
    handles = [(directory / f"{i:03d}.txt").open("w") for i in range(args.workers)]
    seen, chars, counts = set(), set(), Counter()
    try:
        for source, rows in (("v3", args.colloquial.open()), ("tt", sorted(tt))):
            try:
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
                if hasattr(rows, "close"):
                    rows.close()
    finally:
        for f in handles:
            f.close()
    result = {**counts, "training_characters": sorted(chars), "tt_sources": sources}
    lm.save_json(marker, result)
    lm.log(f"prepared: {dict(counts)}")
    return result


def export(out, words, model):
    uni, keys, probs, alpha, _ = model
    with (out / "unigram.tsv").open("w") as f:
        for i, w in enumerate(words):
            if w != "<s>":
                f.write(f"{w}\t{math.log(uni[i]):.12f}\n")
    with (out / "bigram.tsv").open("w") as f:
        for key, p in zip(keys, probs):
            a, b = int(key) >> 32, int(key) & 0xffffffff
            f.write(f"{words[a]}\t{words[b]}\t{math.log(p):.12f}\n")
    with (out / "backoff.tsv").open("w") as f:
        for a in np.unique(keys >> np.uint64(32)):
            f.write(f"{words[int(a)]}\t{math.log(alpha[int(a)]):.12f}\n")


class Model:
    def __init__(self, directory):
        self.uni, self.backoff, self.bi = {}, {}, defaultdict(dict)
        for name, target in (("unigram", self.uni), ("backoff", self.backoff)):
            with (directory / (name + ".tsv")).open() as f:
                for line in f:
                    w, p = line.rstrip("\n").split("\t")
                    if w in target or not math.isfinite(float(p)):
                        raise ValueError(f"invalid {name} row")
                    target[w] = float(p)
        with (directory / "bigram.tsv").open() as f:
            for line in f:
                a, b, p = line.rstrip("\n").split("\t")
                if b in self.bi[a] or b not in self.uni or not math.isfinite(float(p)):
                    raise ValueError("invalid bigram row")
                self.bi[a][b] = float(p)

    def score(self, a, b):
        a = a if a == "<s>" or a in self.uni else "<unk>"
        b = b if b in self.uni else "<unk>"
        return self.bi.get(a, {}).get(b, self.backoff.get(a, 0.) + self.uni[b])

    def verify(self):
        assert "<s>" not in self.uni and {"</s>", "<unk>"} <= self.uni.keys()
        assert set(self.bi) == set(self.backoff)
        mass = {w: math.exp(p) for w, p in self.uni.items()}
        assert all(p > 0 for p in mass.values())
        uni_error = abs(math.fsum(mass.values()) - 1)
        error = 0.
        for a, successors in self.bi.items():
            assert a == "<s>" or a in self.uni
            total = math.fsum(math.exp(p) for p in successors.values())
            total += math.exp(self.backoff[a]) * (1 - math.fsum(mass[b] for b in successors))
            error = max(error, abs(total - 1))
        assert max(uni_error, error) < 1e-9, (uni_error, error)
        # Independently enumerate full distributions for frequent and fallback contexts.
        contexts = sorted(self.bi, key=lambda a: -len(self.bi[a]))[:8] + ["<s>", "<unk>", "never-observed-context"]
        sampled_error = max(abs(math.fsum(math.exp(self.score(a, b)) for b in self.uni) - 1) for a in contexts)
        assert sampled_error < 1e-9
        return {"unigram_error": uni_error, "all_contexts_max_error": error,
                "enumerated_max_error": sampled_error, "contexts": len(self.bi), "enumerated_contexts": contexts}


def distribution(values):
    v = np.asarray(values)
    return {"count": len(v), "mean": float(v.mean()), "std": float(v.std()),
            "quantiles": dict(zip(("min", "p05", "p25", "p50", "p75", "p95", "max"),
                                  map(float, np.quantile(v, [0, .05, .25, .5, .75, .95, 1])))),
            "positive_fraction": float(np.mean(v > 0))}


def evaluate(args, old, words, models):
    ids = {w: i for i, w in enumerate(words)}
    trie = lm.make_trie(words, old)
    scores = np.load(args.work / "segmentation_scores.npy")
    metrics = {}
    with (args.output / "line_scores.tsv").open("w") as f:
        f.write("dataset\tline\twords\tpredictions_with_eos\twikipedia_logp\tchat_logp\tdelta_chat_minus_wiki\tdelta_per_prediction\tsentence\n")
        for name in ("dev_wikipedia", "dev_colloquial", "dev_chat_tt"):
            path = (args.output if name == "dev_chat_tt" else args.inputs) / (name + ".txt")
            totals, unknown, deltas, per_word = Counter(), Counter(), [], []
            for n, line in enumerate(path.open(), 1):
                sentence = normalized(line)
                tokens = [words[i] for i in lm.segment(sentence, trie, ids, scores, float(scores[ids["<unk>"]]))]
                sequence = ["<s>", *tokens, "</s>"]
                ll = {}
                for domain, model in models.items():
                    ll[domain] = math.fsum(model.score(a, b) for a, b in zip(sequence, sequence[1:]))
                    totals[domain] += ll[domain]
                    unknown[domain] += sum(w not in model.uni or w == "<unk>" for w in tokens)
                predictions = len(tokens) + 1
                delta = ll["chat"] - ll["wikipedia"]
                deltas.append(delta)
                per_word.append(delta / predictions)
                totals["words"] += len(tokens)
                totals["predictions"] += predictions
                f.write(f"{name}\t{n}\t{len(tokens)}\t{predictions}\t{ll['wikipedia']:.12f}\t{ll['chat']:.12f}\t{delta:.12f}\t{delta/predictions:.12f}\t{sentence}\n")
            metrics[name] = {"sentences": len(deltas), "words": totals["words"], "predictions_with_eos": totals["predictions"],
                "models": {domain: {"perplexity": math.exp(-totals[domain] / totals["predictions"]),
                    "log_likelihood": totals[domain], "unknown_words": unknown[domain],
                    "unknown_rate": unknown[domain] / totals["words"]} for domain in models},
                "line_delta_chat_minus_wikipedia": distribution(deltas), "delta_per_prediction": distribution(per_word)}
    return metrics


def reading_comparison(args, rows, old, words, counts, models):
    word_counts = dict(zip(words, map(int, counts)))
    frequencies, candidates = Counter(), defaultdict(set)
    for reading, w, logp in rows:
        if len(reading.split()) != 2:
            continue
        candidates[reading].add(w)
        frequencies[reading] += word_counts.get(w, 0) * math.exp(logp) / old[w]
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
        result.append({"rank": rank if rank <= 30 else None, "reading": r, "estimated_training_occurrences": frequencies[r], **tops})
    lm.save_json(args.output / "sentence_starts.json", result)
    return result


def audit(args):
    held = {normalized(s) for name in HELD for s in (args.inputs / name).read_text().splitlines()}
    test, dev = [set((args.output / name).read_text().splitlines()) for name in SPLITS]
    cv = {normalized(s) for s in (args.inputs / "train_commonvoice.txt").read_text().splitlines()}
    assert len(test) == len(dev) == 1500 and not test & dev and not (test | dev) & cv
    test_grams = set().union(*(grams(s) for s in test))
    assert all(not grams(s) & test_grams for s in dev)
    held.update(test | dev)
    machine = lm.automaton(held)
    seen = set()
    for path in sorted((args.work / "sentences").glob("*.txt")):
        for line in path.open():
            s = line.rstrip("\n")
            assert s not in held and next(machine.iter(s), None) is None
            assert s not in seen
            seen.add(s)
    return {"training_sentences_rescanned": len(seen), "exact_overlap": 0, "eight_character_overlap": 0,
            "training_duplicates": 0, "holdouts_each": 1500, "new_test_dev_eight_character_overlap": 0}


def main(args):
    start = time.monotonic()
    args.root = args.root.resolve()
    args.output = args.root / "data/build/lm_chat"
    args.work = args.output / "work"
    args.inputs = args.output / "inputs"
    args.colloquial = args.root / "data/build/v3/colloquial.txt"
    args.output.mkdir(parents=True, exist_ok=True)
    args.work.mkdir(exist_ok=True)
    inputs = sorted(p for p in args.inputs.rglob("*") if p.is_file()) + [args.colloquial]
    config = {"seed": SEED, "workers": args.workers, "max_tsv_bytes": args.max_bytes,
        "inputs": {str(p.relative_to(args.root)): {"sha256": lm.digest(p), "bytes": p.stat().st_size} for p in inputs},
        "code": {str(p.relative_to(args.root)): lm.digest(p) for p in (Path(__file__).resolve(), Path(lm.__file__).resolve(), args.root / "nlp/training/lm/train2.py")}}
    checkpoint = args.work / "config.json"
    if checkpoint.exists() and json.loads(checkpoint.read_text()) != config:
        raise ValueError("inputs/code/config changed; use a fresh work directory (no stale checkpoints)")
    lm.save_json(checkpoint, config)
    prepared = prepare(args)
    _, rows, old = lm.read_lexicon(args.inputs / "lexicon.tsv")
    words, counts, timings = lm.em(args, old, prepared)
    keys, pair_counts = lm.merge_pairs(args)
    cap = 1_600_000
    while True:
        model = lm.fit_mkn(keys, pair_counts, len(words), cap)
        export(args.output, words, model)
        size = sum((args.output / name).stat().st_size for name in ("unigram.tsv", "bigram.tsv", "backoff.tsv"))
        if size <= args.max_bytes:
            break
        cap = int(len(model[1]) * args.max_bytes / size * .9)
        if cap < 1:
            raise ValueError("TSV budget too small for vocabulary")
    model_stats = model[4]
    del model, keys, pair_counts
    models = {"wikipedia": Model(args.inputs / "wiki"), "chat": Model(args.output)}
    verification = models["chat"].verify()
    evaluation = evaluate(args, old, words, models)
    comparisons = reading_comparison(args, rows, old, words, counts, models)
    verification["leakage"] = audit(args)
    lm.save_json(args.output / "validation.json", verification)
    for name in ("NOTICE.md", "Taiwan-Tongues-LICENSE", "Taiwan-Tongues-README.md", "Qwen3-8B-LICENSE", "source_receipts.json", "transfer_receipt.json"):
        (args.output / name).write_bytes((args.inputs / name).read_bytes())
    stats = {"status": "validated", "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "config": config, "corpus": {k: v for k, v in prepared.items() if k != "training_characters"},
        "model": {**model_stats, "unigram_types": len(words) - 1, "tsv_bytes": size},
        "segmentation": {"method": "pipeline.py initial lexicon Viterbi + two hard-EM updates; freeze final E step for BOTH models", "passes": timings},
        "evaluation_protocol": "Natural logs; word perplexity includes EOS, excludes BOS. Same frozen chat word tokens for both models; per-line delta = logP(chat)-logP(wikipedia). Test held out, never scored/tuned. Novel targets AND contexts map to <unk>; absent known context backoff=0.",
        "evaluation": evaluation, "sentence_start_readings": comparisons, "validation": verification,
        "reading_frequency_protocol": "Final chat segmentation word counts, apportioned among readings by normalized seed reading mass; top word maximizes word P(w|<s>) among exact-reading lexicon entries, without reading-prior multiplication.",
        "runtime": {"python": platform.python_version(), "platform": platform.platform(), "packages": {p: importlib.metadata.version(p) for p in ("numpy", "opencc", "mwparserfromhell", "pyahocorasick")}},
        "seconds": time.monotonic() - start}
    stats["files"] = {p.name: {"bytes": p.stat().st_size, "sha256": lm.digest(p)} for p in args.output.iterdir() if p.is_file() and p.name != "stats.json"}
    lm.save_json(args.output / "stats.json", stats)
    lm.log("VALIDATED")
    print("Dataset | Lines | Wikipedia PPL | Chat PPL | Mean line delta", flush=True)
    for name, v in evaluation.items():
        print(f"{name} | {v['sentences']} | {v['models']['wikipedia']['perplexity']:.3f} | {v['models']['chat']['perplexity']:.3f} | {v['line_delta_chat_minus_wikipedia']['mean']:.3f}", flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--root", type=Path, default=Path("."))
    p.add_argument("--workers", type=int, default=48)
    p.add_argument("--max-bytes", type=int, default=60_000_000)
    main(p.parse_args())

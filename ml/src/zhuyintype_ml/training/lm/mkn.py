"""Word bigram estimation shared by every language model: lexicon Viterbi segmentation, hard EM, modified Kneser-Ney.

`lm` (Wikipedia), `lm_chat`, `lm_chat2` and `base` all segment and smooth with these functions, so their models are
built the same way.  Needs NumPy and pyahocorasick (the train dependency group).

    from zhuyintype_ml.training.lm import mkn
    words, counts, timings = mkn.em(args, lexicon_mass, prepared)   # three segmentation passes over args.work
    keys, pair_counts = mkn.merge_pairs(args)
    model = mkn.fit_mkn(keys, pair_counts, len(words), 3_000_000)
    mkn.write_tsv(output, words, model)                              # unigram.tsv, bigram.tsv, backoff.tsv
    mkn.TsvModel(output).verify()

A fitted model is (unigram, kept pair keys, their probabilities, backoff weights, statistics); a pair key packs the
context id in the high 32 bits and the next word id in the low 32 bits.
"""

from __future__ import annotations

import argparse
import array
import collections
import json
import math
import multiprocessing as mp
import re
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

import ahocorasick
import numpy as np

from .files import log, save_json

HAN = re.compile(r"[㐀-䶿一-鿿豈-﫿\U00020000-\U000323af]")
SPLIT = re.compile(r"[。！？；\n]+")
BAD = re.compile(r"https?://|www\.|://|[{}\[\]<>|=`\\]|\b(?:function|SELECT|INSERT|import)\b", re.I)

Trie = dict[Any, Any]
# (unigram, kept pair keys, pair probabilities, backoff weights, statistics)
Fitted = tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, dict[str, Any]]


def automaton(sentences: Any) -> ahocorasick.Automaton:
    """Every 8-character substring of `sentences`, for leakage checks against held-out text."""
    machine = ahocorasick.Automaton()
    for sentence in sentences:
        for i in range(len(sentence) - 7):
            machine.add_word(sentence[i : i + 8], True)
    if len(machine):
        machine.make_automaton()
    return machine


def read_lexicon(path: Path | str) -> tuple[str, list[tuple[str, str, float]], dict[str, float]]:
    """(header line, (reading, word, logp) rows, word → summed probability over its readings)."""
    rows: list[tuple[str, str, float]] = []
    old: collections.defaultdict[str, float] = collections.defaultdict(float)
    with open(path, encoding="utf-8") as source:
        header = next(source).rstrip("\n")
        if not header.startswith("#"):
            raise ValueError("lexicon must start with a comment")
        for line in source:
            reading, word, value = line.rstrip("\n").split("\t")
            p = float(value)
            if not math.isfinite(p):
                raise ValueError("non-finite lexicon logp")
            rows.append((reading, word, p))
            old[word] += math.exp(p)
    return header, rows, dict(old)


def make_trie(words: list[str], lexicon: Any) -> Trie:
    trie: Trie = {}
    for index, word in enumerate(words):
        if word not in lexicon:
            continue
        node = trie
        for ch in word:
            node = node.setdefault(ch, {})
        node[None] = index
    return trie


def segment(sentence: str, trie: Trie, ids: dict[str, int], scores: Any, unknown_score: float) -> list[int]:
    """The most probable segmentation under unigram `scores`; characters outside the lexicon are single tokens."""
    n = len(sentence)
    best = [-math.inf] * (n + 1)
    previous: list[tuple[int, int] | None] = [None] * (n + 1)
    best[0] = 0.0
    for i, ch in enumerate(sentence):
        token = ids.get(ch, ids["<unk>"])
        value = best[i] + (scores[token] if ch in ids else unknown_score)
        if value > best[i + 1]:
            best[i + 1], previous[i + 1] = value, (i, token)
        node: Trie | None = trie
        for j in range(i, n):
            assert node is not None
            node = node.get(sentence[j])
            if node is None:
                break
            word: int | None = node.get(None)
            if word is not None:
                value = best[i] + scores[word]
                if value > best[j + 1]:
                    best[j + 1], previous[j + 1] = value, (i, word)
    result = []
    position = n
    while position:
        step = previous[position]
        assert step is not None
        position, token = step
        result.append(token)
    result.reverse()
    return result


# The segmenter state of the current pass, set by em() before forking the worker pool.
TRIE: Trie = {}
IDS: dict[str, int] = {}
SCORES: np.ndarray = np.zeros(0)
UNKNOWN_SCORE = 0.0


def segment_shard(task: tuple[str, str, bool]) -> dict[str, float]:
    path, directory, final = task
    start = time.monotonic()
    uni: collections.Counter[int] = collections.Counter()
    pairs = array.array("Q")
    with open(path, encoding="utf-8") as source:
        for line in source:
            tokens = segment(line.rstrip("\n"), TRIE, IDS, SCORES, UNKNOWN_SCORE)
            uni.update(tokens)
            if final:
                sequence = [IDS["<s>"], *tokens, IDS["</s>"]]
                pairs.extend((a << 32) | b for a, b in zip(sequence, sequence[1:]))
    values = np.zeros(len(IDS), dtype=np.uint64)
    for token, count in uni.items():
        values[token] = count
    target = Path(directory) / Path(path).stem
    np.save(str(target) + ".uni.npy", values)
    if final:
        keys, counts = np.unique(np.frombuffer(pairs, dtype=np.uint64), return_counts=True)
        np.save(str(target) + ".keys.npy", keys)
        np.save(str(target) + ".counts.npy", counts.astype(np.uint64))
    return {"tokens": int(values.sum()), "seconds": time.monotonic() - start}


def em(
    args: argparse.Namespace, old: dict[str, float], split: dict[str, Any]
) -> tuple[list[str], np.ndarray, list[dict[str, Any]]]:
    """Segments `args.work/sentences/*.txt` three times: lexicon Viterbi, then two unigram M/E updates.

    Resumable per pass (`args.work/em{0,1,2}/done.json`).  The last pass also counts bigrams.  Returns the
    vocabulary, the final unigram counts and the pass timings; saves the frozen final tokenizer."""
    global TRIE, IDS, SCORES, UNKNOWN_SCORE
    words = ["<s>", "</s>", "<unk>"] + sorted(set(old) | set(split["training_characters"]))
    IDS = {word: index for index, word in enumerate(words)}
    TRIE = make_trie(words, old)
    normalizer = sum(old.values())
    SCORES = np.array([math.log(old.get(word, 1e-10) / normalizer) for word in words])
    UNKNOWN_SCORE = math.log(1e-10 / normalizer)
    timings = []
    values = np.zeros(len(words), dtype=np.uint64)
    # Initial E step, followed by two M/E updates (three segmentation passes).
    for iteration in range(3):
        directory = args.work / f"em{iteration}"
        directory.mkdir(exist_ok=True)
        marker = directory / "done.json"
        start = time.monotonic()
        if not marker.exists():
            log(f"segment pass={iteration} workers={args.workers}")
            tasks = [
                (str(path), str(directory), iteration == 2) for path in sorted((args.work / "sentences").glob("*.txt"))
            ]
            with mp.get_context("fork").Pool(args.workers) as pool:
                results = list(pool.imap_unordered(segment_shard, tasks))
            save_json(
                marker,
                {"wall_seconds": time.monotonic() - start, "worker_seconds": sum(r["seconds"] for r in results)},
            )
        values = sum(
            (np.load(path) for path in sorted(directory.glob("*.uni.npy"))), np.zeros(len(words), dtype=np.uint64)
        )
        timings.append(json.loads(marker.read_text()))
        if iteration < 2:
            probabilities = (values.astype(float) + 0.1) / (float(values.sum()) + 0.1 * len(words))
            SCORES = np.log(probabilities)
            UNKNOWN_SCORE = math.log(0.1 / (float(values.sum()) + 0.1 * len(words)))
        log(f"segment pass={iteration} tokens={int(values.sum()):,} seconds={timings[-1]['wall_seconds']:.1f}")
    # Freeze the final E-step tokenizer for held-out evaluation, without test fitting.
    np.save(args.work / "segmentation_scores.npy", SCORES)
    save_json(args.work / "vocabulary.json", words)
    return words, values, timings


def merge_pairs(args: argparse.Namespace) -> tuple[np.ndarray, np.ndarray]:
    """The final pass's per-shard bigram counts, merged into sorted unique keys (cached in args.work)."""
    marker = args.work / "merged.json"
    if not marker.exists():
        start = time.monotonic()
        directory = args.work / "em2"
        keys = np.concatenate([np.load(path) for path in sorted(directory.glob("*.keys.npy"))])
        counts = np.concatenate([np.load(path) for path in sorted(directory.glob("*.counts.npy"))])
        log(f"merge shard pair types={len(keys):,}")
        order = np.argsort(keys)
        keys, counts = keys[order], counts[order]
        del order
        starts = np.r_[0, np.flatnonzero(keys[1:] != keys[:-1]) + 1]
        counts = np.add.reduceat(counts, starts)
        keys = keys[starts]
        np.save(args.work / "pair_keys.npy", keys)
        np.save(args.work / "pair_counts.npy", counts)
        save_json(marker, {"seconds": time.monotonic() - start, "pair_types": len(keys)})
    return np.load(args.work / "pair_keys.npy"), np.load(args.work / "pair_counts.npy")


def discounts(counts: np.ndarray) -> tuple[np.ndarray, list[int]]:
    """Chen–Goodman discounts D1, D2, D3+ and the count-of-counts n0..n4 (n0 unused)."""
    nr = [0] + [int(np.count_nonzero(counts == r)) for r in range(1, 5)]
    y = nr[1] / (nr[1] + 2 * nr[2]) if nr[1] else 0.5
    result = []
    for r in range(1, 4):
        value = r - (r + 1) * y * nr[r + 1] / nr[r] if nr[r] and nr[r + 1] else 0.75
        result.append(min(r - 1e-6, max(1e-6, value)))
    return np.array(result), nr


def fit_mkn(keys: np.ndarray, counts: np.ndarray, size: int, cap: int) -> Fitted:
    """Interpolated modified Kneser-Ney over all pairs, pruned by the smallest global count threshold (>=2) that keeps
    at most `cap` pairs, with backoff weights renormalized after pruning."""
    left = (keys >> np.uint64(32)).astype(np.int64)
    right = (keys & np.uint64(0xFFFFFFFF)).astype(np.int64)
    d, nr = discounts(counts)
    # KN continuation unigram; reserve a tiny floor only for zero-continuation words.
    continuation = np.bincount(right, minlength=size).astype(float)
    continuation[continuation == 0] = 0.1
    continuation[0] = 0.0  # BOS is context-only.
    continuation[2] = max(1.0, continuation[2])
    unigram = continuation / continuation.sum()
    context_total = np.bincount(left, weights=counts.astype(float), minlength=size)
    removed = d[np.minimum(counts, 3).astype(np.int64) - 1]
    lambdas = np.divide(
        np.bincount(left, weights=removed, minlength=size), context_total, out=np.ones(size), where=context_total > 0
    )
    threshold = 2
    while np.count_nonzero(counts >= threshold) > cap:
        threshold += 1
    keep = counts >= threshold
    selected_keys = keys[keep]
    a, b = left[keep], right[keep]
    probabilities = (counts[keep] - removed[keep]) / context_total[a] + lambdas[a] * unigram[b]
    kept_mass = np.bincount(a, weights=probabilities, minlength=size)
    kept_lower = np.bincount(a, weights=unigram[b], minlength=size)
    alpha = np.divide(1 - kept_mass, 1 - kept_lower, out=np.ones(size), where=kept_lower < 1)
    if not (np.all(probabilities > 0) and np.all(alpha > 0)):
        raise ValueError("invalid probabilities/backoff")
    error = np.max(np.abs(kept_mass + alpha * (1 - kept_lower) - 1))
    assert error < 1e-10
    return (
        unigram,
        selected_keys,
        probabilities,
        alpha,
        {
            "discounts_D1_D2_D3plus": d.tolist(),
            "bigram_count_of_counts_N1_to_N4": nr[1:],
            "prune_count_threshold": threshold,
            "bigram_types_before_pruning": len(keys),
            "bigram_types_exported": len(selected_keys),
            "normalization_error_all_contexts_float64": float(error),
            "unseen_unigram_pseudocontinuations": 0.1,
            "unk_pseudocontinuations": 1.0,
        },
    )


def write_tsv(out: Path, words: list[str], model: Fitted) -> None:
    """unigram.tsv, bigram.tsv and backoff.tsv (natural logs, 12 decimals); <s> is context-only."""
    uni, keys, probs, alpha, _ = model
    with (out / "unigram.tsv").open("w", encoding="utf-8") as f:
        for i, w in enumerate(words):
            if w != "<s>":
                f.write(f"{w}\t{math.log(uni[i]):.12f}\n")
    with (out / "bigram.tsv").open("w", encoding="utf-8") as f:
        for key, p in zip(keys, probs):
            a, b = int(key) >> 32, int(key) & 0xFFFFFFFF
            f.write(f"{words[a]}\t{words[b]}\t{math.log(p):.12f}\n")
    with (out / "backoff.tsv").open("w", encoding="utf-8") as f:
        for a in np.unique(keys >> np.uint64(32)):
            f.write(f"{words[int(a)]}\t{math.log(alpha[int(a)]):.12f}\n")


class TsvModel:
    """A bigram model read back from its TSVs: scoring with backoff, and an independent normalization check."""

    def __init__(self, directory: Path) -> None:
        self.uni: dict[str, float] = {}
        self.backoff: dict[str, float] = {}
        self.bi: defaultdict[str, dict[str, float]] = defaultdict(dict)
        for name, target in (("unigram", self.uni), ("backoff", self.backoff)):
            with (directory / (name + ".tsv")).open(encoding="utf-8") as f:
                for line in f:
                    w, p = line.rstrip("\n").split("\t")
                    if w in target or not math.isfinite(float(p)):
                        raise ValueError(f"invalid {name} row")
                    target[w] = float(p)
        with (directory / "bigram.tsv").open(encoding="utf-8") as f:
            for line in f:
                a, b, p = line.rstrip("\n").split("\t")
                if b in self.bi[a] or b not in self.uni or not math.isfinite(float(p)):
                    raise ValueError("invalid bigram row")
                self.bi[a][b] = float(p)

    def score(self, a: str, b: str) -> float:
        """log P(b | a); unknown words and contexts map to <unk>, a known context without a backoff row uses 0."""
        a = a if a == "<s>" or a in self.uni else "<unk>"
        b = b if b in self.uni else "<unk>"
        return self.bi.get(a, {}).get(b, self.backoff.get(a, 0.0) + self.uni[b])

    def verify(self) -> dict[str, Any]:
        assert "<s>" not in self.uni and {"</s>", "<unk>"} <= self.uni.keys()
        assert set(self.bi) == set(self.backoff)
        mass = {w: math.exp(p) for w, p in self.uni.items()}
        assert all(p > 0 for p in mass.values())
        uni_error = abs(math.fsum(mass.values()) - 1)
        error = 0.0
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
        return {
            "unigram_error": uni_error,
            "all_contexts_max_error": error,
            "enumerated_max_error": sampled_error,
            "contexts": len(self.bi),
            "enumerated_contexts": contexts,
        }

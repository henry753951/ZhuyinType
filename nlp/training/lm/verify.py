"""Independent validation of the exported Wikipedia bigram: TSV normalization, lexicon rows and reading ratios, and a
full rescan of the training sentences against the held-out test set.

Re-reads the serialized files (not the in-memory model), checks every file hash in stats.json, writes
validation.json, and marks stats.json "validated".  GPU host (reads the training shards in --work).

    zt --remote train lm verify
    python -m nlp.training.lm.verify --lexicon data/build/lexicon.tsv --work data/build/lm/work \\
        --output data/build/lm --workers 64
"""

from __future__ import annotations

import argparse
import array
import collections
import json
import math
import multiprocessing as mp
import random
import time
from pathlib import Path

import ahocorasick
import numpy as np

from nlp.common import paths

from .files import digest, log, save_json
from .mkn import automaton, read_lexicon

# The held-out set, shared with the forked workers.
HELDOUT: set[str] = set()
MACHINE: ahocorasick.Automaton | None = None


def verify_shard(path: Path) -> tuple[int, int]:
    count = chars = 0
    assert MACHINE is not None
    with open(path, encoding="utf-8") as f:
        for line in f:
            sentence = line.rstrip("\n")
            assert sentence not in HELDOUT, f"test sentence in training: {path}"
            assert next(MACHINE.iter(sentence), None) is None, f"8-character test overlap: {path}"
            count += 1
            chars += len(sentence)
    return count, chars


def main(argv: list[str] | None = None) -> int:
    global HELDOUT, MACHINE
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, default=paths.data("lm"))
    parser.add_argument("--lexicon", type=Path, default=paths.data("lexicon.tsv"))
    parser.add_argument("--work", type=Path, default=paths.data("lm", "work"))
    parser.add_argument("--workers", type=int, default=64)
    args = parser.parse_args(argv)
    started = time.monotonic()
    stats = json.loads((args.output / "stats.json").read_text())
    for name, meta in stats["files"].items():
        path = args.output / name
        assert path.stat().st_size == meta["bytes"], name
        assert digest(path) == meta["sha256"], name
    words = []
    probabilities = []
    with open(args.output / "unigram.tsv", encoding="utf-8") as f:
        for line in f:
            word, p = line.rstrip("\n").split("\t")
            assert math.isfinite(float(p)) and float(p) <= 0
            words.append(word)
            probabilities.append(math.exp(float(p)))
    assert len(set(words)) == len(words)
    assert "<unk>" in words and "</s>" in words and "<s>" not in words
    ids = {word: i for i, word in enumerate(words)}
    ids["<s>"] = len(ids)
    uni = np.array([*probabilities, 0.0])
    assert abs(uni.sum() - 1) < 1e-10
    backoffs = {}
    with open(args.output / "backoff.tsv", encoding="utf-8") as f:
        for line in f:
            word, p = line.rstrip("\n").split("\t")
            assert word not in backoffs and math.isfinite(float(p))
            backoffs[word] = math.exp(float(p))
    key_array, prob_array = array.array("Q"), array.array("d")
    contexts = set()
    with open(args.output / "bigram.tsv", encoding="utf-8") as f:
        for line in f:
            a, b, p = line.rstrip("\n").split("\t")
            assert b != "<s>" and a != "</s>"
            assert math.isfinite(float(p)) and float(p) <= 0
            key_array.append((ids[a] << 32) | ids[b])
            prob_array.append(math.exp(float(p)))
            contexts.add(a)
    assert len(key_array) <= 3000000
    assert "<s>" in contexts
    assert set(backoffs) == contexts
    keys = np.frombuffer(key_array, dtype=np.uint64)
    probs = np.frombuffer(prob_array, dtype=np.float64)
    assert len(np.unique(keys)) == len(keys)
    left = (keys >> np.uint64(32)).astype(np.int64)
    right = (keys & np.uint64(0xFFFFFFFF)).astype(np.int64)
    assert ids["</s>"] in right
    mass = np.bincount(left, weights=probs, minlength=len(ids))
    lower = np.bincount(left, weights=uni[right], minlength=len(ids))
    alpha = np.ones(len(ids))
    for word, value in backoffs.items():
        alpha[ids[word]] = value
    errors = np.abs(mass + alpha * (1 - lower) - 1)
    assert max(errors) < 1e-9
    # Explicitly enumerate the target vocabulary for 24 contexts, using file values.
    rng = random.Random(20261026)
    sample = ["<s>", "我", "今天", "台灣", "的"]
    sample += rng.sample(sorted(contexts - set(sample)), min(19, len(contexts - set(sample))))
    sample_errors = {}
    for word in sample:
        context = ids[word]
        distribution = alpha[context] * uni.copy()
        positions = np.flatnonzero(left == context)
        distribution[right[positions]] = probs[positions]
        sample_errors[word] = abs(float(distribution.sum()) - 1)
        assert sample_errors[word] < 1e-9
    _, rows, old = read_lexicon(args.lexicon)
    new_rows = []
    with open(args.output / "lexicon_tw.tsv", encoding="utf-8") as f:
        assert next(f).startswith("#")
        for line in f:
            reading, word, logp = line.rstrip("\n").split("\t")
            new_rows.append((reading, word, float(logp)))
    assert len(rows) == len(new_rows)
    deltas: collections.defaultdict[str, list[float]] = collections.defaultdict(list)
    new_word_mass: collections.defaultdict[str, float] = collections.defaultdict(float)
    for (r, w, old_logp), (r2, w2, new_logp) in zip(rows, new_rows):
        assert (r, w) == (r2, w2)
        deltas[w].append(new_logp - old_logp)
        new_word_mass[w] += math.exp(new_logp)
    assert max(max(ds) - min(ds) for ds in deltas.values()) < 2e-10
    training_words = json.loads((args.work / "vocabulary.json").read_text())
    training_ids = {word: i for i, word in enumerate(training_words)}
    translate = np.array([training_ids[word] for word in words] + [training_ids["<s>"]], dtype=np.uint64)
    original_keys = (translate[left] << np.uint64(32)) | translate[right]
    raw_keys = np.load(args.work / "pair_keys.npy", mmap_mode="r")
    raw_counts = np.load(args.work / "pair_counts.npy", mmap_mode="r")
    positions = np.searchsorted(raw_keys, original_keys)
    assert np.all(positions < len(raw_keys))
    assert np.array_equal(raw_keys[positions], original_keys)
    assert stats["model"]["prune_count_threshold"] >= 2
    assert np.all(raw_counts[positions] >= stats["model"]["prune_count_threshold"])
    values = sum(
        (np.load(path) for path in sorted((args.work / "em2").glob("*.uni.npy"))),
        np.zeros(len(training_words), dtype=np.uint64),
    )
    expected_shift = stats["vocabulary"]["unseen_lexicon_logp_shift"]
    for word, count in zip(training_words, values):
        if word not in old:
            continue
        if count:
            assert abs(math.log(new_word_mass[word]) - math.log(uni[ids[word]])) < 2e-10
        else:
            assert abs(deltas[word][0] - expected_shift) < 2e-10
    HELDOUT = set((args.output / "test_sentences.txt").read_text().splitlines())
    assert len(HELDOUT) == stats["corpus"]["test_sentences"]
    MACHINE = automaton(HELDOUT)
    with mp.get_context("fork").Pool(args.workers) as pool:
        separation = list(pool.imap_unordered(verify_shard, sorted((args.work / "sentences").glob("*.txt"))))
    assert sum(c for c, n in separation) == stats["corpus"]["training_sentences"]
    assert sum(n for c, n in separation) == stats["corpus"]["training_chars"]
    result = {
        "serialized_unigram_mass": float(uni.sum()),
        "serialized_max_context_normalization_error": float(max(errors)),
        "explicit_enumeration_errors": sample_errors,
        "contexts_verified": len(contexts),
        "lexicon_rows_identical": len(rows),
        "reading_ratios_preserved": True,
        "all_seen_word_masses_match_exported_unigram": True,
        "all_unseen_word_shifts_verified": True,
        "all_exported_pairs_meet_training_count_threshold": True,
        "full_training_8char_overlap_scan_passed": True,
        "verified_training_sentences": sum(c for c, n in separation),
        "seconds": time.monotonic() - started,
    }
    save_json(args.output / "validation.json", result)
    stats["validation"] = result
    stats["files"]["validation.json"] = {
        "bytes": (args.output / "validation.json").stat().st_size,
        "sha256": digest(args.output / "validation.json"),
    }
    stats["status"] = "validated"
    save_json(args.output / "stats.json", stats)
    log(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

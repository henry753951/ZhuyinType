"""Variant and misspelt spellings among mined words, told apart by where the corpus uses them (GPU host, CPU only).

    python -m zhuyintype_ml.training.lexicon.variants --mined DIR --packs DIR --corpus ptt generated \\
        --lexicon data/lexicon.tsv --unihan data/raw/lexicon/Unihan-18.0.0.zip --out DIR

A mined word can be another spelling of a word the corpus already writes: a variant character (祕書 for 秘書,
臺幣 for 台幣), a misspelling or a pun on the same sounds (喝喝 for 呵呵, 摳憐 for 摳連, 老濕 for 老師). As a lexicon
word it only competes with the usual spelling. Such a spelling is used where the usual one is, while a
new word with the same reading (破億 next to 破譯, 公視 next to 公式) is used elsewhere. So, for each pack word:

  competitors  the lexicon's words with the same reading, and the corpus n-grams (counted by `mine`) that differ from
               it in one character the lexicon reads with that syllable;
  contexts     the character before and the character after every occurrence (a sentence boundary or a non-Han
               character counts as one more character);
  score        how well the competitor's contexts explain the pack word's: the mean over the pack word's occurrences
               of log P(context | competitor) - log P(context | pack word, that occurrence left out), both smoothed
               towards the characters' corpus frequency with weight BETA.  Near 0, the two are used alike; a word
               used elsewhere scores far below 0.

A pack word is a variant when a competitor the corpus uses more often scores at least the threshold.  The threshold
comes from the lexicon: the 95th percentile of the score over pairs of lexicon words with the same reading, both
seen at least 20 times (mostly different words, so at most about 5% of real homophones would look alike; the top
of that set is in fact mostly spellings the lexicon holds twice, 計劃 and 計畫, 宣佈 and 宣布).  The check the other
way is a set of known same-word pairs: a lexicon word and the same word with one character swapped for a Unihan
variant (kZVariant, kSemanticVariant) read with the same syllable, both seen at least 20 times (台灣 and 臺灣, 抽煙
and 抽菸); variants.json gives how many reach the threshold.  On PTT, with the context window and BETA chosen to
separate the two sets best, the area under the ROC curve was 0.90 (docs/research/log.md, `chat-variants`).  A corpus
with fewer than 200 lexicon pairs takes the threshold of the corpus before it.

Writes OUT/<corpus>.variants.tsv (word, reading, competitor, score, counts, verdict; one row per pack word,
its best-scoring more frequent competitor), OUT/<corpus>.calibration.tsv, OUT/<corpus>.contexts.npz with
<corpus>.targets.txt (the context counts, for checking other scores) and OUT/variants.json (threshold,
calibration, counts). `screen --variants` leaves out the words marked `variant`. Needs numpy (the `train`
dependency group).
"""

from __future__ import annotations

import argparse
import collections
import json
import math
import re
import zipfile
from collections.abc import Callable
from pathlib import Path

import numpy as np

from .mine import MULTIPLIER, _hashes, read_ngrams

PERCENTILE = 95
MIN_CALIBRATION_COUNT = 20
MAX_CALIBRATION_PAIRS = 6000
MIN_PAIRS = 200  # a smaller corpus takes the threshold of the corpus before it
BETA = 20.0  # smoothing weight towards the characters' corpus frequency
CHAR_BITS = 21
MASK = (1 << 64) - 1


def read_lexicon(path: Path) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    """(reading → words of 2 to 6 characters, syllable → characters read with it) from lexicon rows."""
    by_reading: dict[str, set[str]] = collections.defaultdict(set)
    chars: dict[str, set[str]] = collections.defaultdict(set)
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("#") or "\t" not in line:
                continue
            reading, word = line.split("\t")[:2]
            if len(word) == 1:
                chars[reading].add(word)
            elif 2 <= len(word) <= 6 and len(reading.split(" ")) == len(word):
                by_reading[reading].add(word)
    return by_reading, chars


def unihan_variants(archive: Path) -> set[tuple[str, str]]:
    """Unordered character pairs Unihan lists as kZVariant or kSemanticVariant of each other."""
    pairs: set[tuple[str, str]] = set()
    with zipfile.ZipFile(archive) as z, z.open("Unihan_Variants.txt") as raw:
        for line in raw.read().decode("utf-8").splitlines():
            parts = line.split("\t")
            if len(parts) < 3 or parts[1] not in ("kZVariant", "kSemanticVariant"):
                continue
            a = chr(int(parts[0][2:], 16))
            for code in re.findall(r"U\+([0-9A-F]+)", parts[2]):
                b = chr(int(code, 16))
                if a != b:
                    pairs.add((min(a, b), max(a, b)))
    return pairs


def is_variant_spelling(a: str, b: str, variants: set[tuple[str, str]]) -> bool:
    differ = [(x, y) for x, y in zip(a, b) if x != y]
    return len(a) == len(b) and bool(differ) and all((min(x, y), max(x, y)) in variants for x, y in differ)


def string_hash(word: str) -> int:
    h = 0
    for c in word:
        h = (h * int(MULTIPLIER) + ord(c)) & MASK
    return h


def contexts(ids: np.ndarray, targets: list[str], chunk: int = 20_000_000) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """(target index, feature, count) rows, sorted by target then feature; feature = slot << 21 | code point, slot 0
    the character before, 1 the one after (0 for a boundary)."""
    n = len(ids)
    keys: list[np.ndarray] = []
    counts: list[np.ndarray] = []
    by_length: dict[int, list[int]] = collections.defaultdict(list)
    for i, word in enumerate(targets):
        by_length[len(word)].append(i)
    for length, members in sorted(by_length.items()):
        hashes = np.array([string_hash(targets[i]) for i in members], dtype=np.uint64)
        order = np.argsort(hashes)
        hashes, index = hashes[order], np.array(members, dtype=np.int64)[order]
        for start in range(0, n, chunk):
            stop = min(n, start + chunk)
            h, valid = _hashes(ids, start, stop, length)
            slot = np.minimum(np.searchsorted(hashes, h), len(hashes) - 1)
            found = np.flatnonzero(valid & (hashes[slot] == h))
            if not len(found):
                continue
            where = start + found
            target = index[slot[found]].astype(np.uint64)
            columns = []
            left = np.where(where >= 1, ids[np.maximum(where - 1, 0)], 0).astype(np.uint64)
            after = where + length
            right = np.where(after < n, ids[np.minimum(after, n - 1)], 0).astype(np.uint64)
            for s, column in enumerate((left, right)):
                columns.append((target << np.uint64(23)) | (np.uint64(s) << np.uint64(CHAR_BITS)) | column)
            key, count = np.unique(np.concatenate(columns), return_counts=True)
            keys.append(key)
            counts.append(count)
    if not keys:
        empty = np.zeros(0, dtype=np.int64)
        return empty, empty, empty
    key, inverse = np.unique(np.concatenate(keys), return_inverse=True)
    total = np.bincount(inverse, weights=np.concatenate(counts)).astype(np.int64)
    return (key >> np.uint64(23)).astype(np.int64), (key & np.uint64((1 << 23) - 1)).astype(np.int64), total


class Contexts:
    """The characters around each target in the corpus, for the score of one target's contexts under another's."""

    def __init__(self, ids: np.ndarray, targets: list[str]) -> None:
        self.target, self.feature, self.count = contexts(ids, targets)
        self.unigram = np.bincount(ids, minlength=1 << CHAR_BITS).astype(np.int64)
        self.p_char = self.unigram / max(1, int(self.unigram.sum()))
        self.starts = np.searchsorted(self.target, np.arange(len(targets) + 1))
        # every occurrence has exactly one feature in slot 0
        slot0 = (self.feature >> CHAR_BITS) == 0
        occurrences = np.bincount(self.target[slot0], weights=self.count[slot0], minlength=len(targets))
        self.occurrences = occurrences.astype(np.int64)

    def score(self, word: int, other: int) -> float:
        """Mean log P(context | other) - log P(context | word, leave one out) over word's occurrences."""
        a, b = self.starts[word], self.starts[word + 1]
        if a == b or self.starts[other] == self.starts[other + 1]:
            return -math.inf
        feature, count = self.feature[a:b], self.count[a:b].astype(np.float64)
        prior = BETA * np.maximum(self.p_char[feature & ((1 << CHAR_BITS) - 1)], 1e-12)
        theirs = self.feature[self.starts[other] : self.starts[other + 1]]
        position = np.minimum(np.searchsorted(theirs, feature), len(theirs) - 1)
        shared = theirs[position] == feature
        other_count = np.where(shared, self.count[self.starts[other] : self.starts[other + 1]][position], 0)
        n_word, n_other = self.occurrences[word], self.occurrences[other]
        p_other = (other_count + prior) / (n_other + BETA)
        p_self = (count - 1 + prior) / (n_word - 1 + BETA)
        return float((count * (np.log(p_other) - np.log(p_self))).sum() / count.sum())


def read_pack(path: Path) -> list[tuple[str, str]]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or "\t" not in line:
            continue
        reading, word = line.split("\t")[:2]
        rows.append((word, reading))
    return rows


def competitors(
    word: str, reading: str, by_reading: dict[str, set[str]], chars: dict[str, set[str]], ngrams: dict[str, int]
) -> dict[str, str]:
    """Other spellings of the reading: lexicon words (`lexicon`) and one-character changes the corpus has (`corpus`)."""
    found = {w: "lexicon" for w in by_reading.get(reading, ()) if w != word}
    syllables = reading.split(" ")
    if len(syllables) == len(word):
        for i, syllable in enumerate(syllables):
            for c in chars.get(syllable, ()):
                spelling = word[:i] + c + word[i + 1 :]
                if c != word[i] and spelling in ngrams and spelling not in found:
                    found[spelling] = "corpus"
    return found


def calibration_pairs(
    by_reading: dict[str, set[str]],
    chars: dict[str, set[str]],
    ngrams: dict[str, int],
    variants: set[tuple[str, str]],
) -> tuple[list[tuple[str, str]], list[tuple[str, str]]]:
    """(different-word pairs, same-word variant pairs), both sides seen at least MIN_CALIBRATION_COUNT times here.

    Different: lexicon words with one reading that are not Unihan variant spellings of each other.  Same: those that
    are, and each lexicon word with one character swapped for a Unihan variant read with the same syllable that the
    corpus has (pairs are ordered rarer first)."""
    others: dict[str, set[str]] = collections.defaultdict(set)
    for a, b in variants:
        others[a].add(b)
        others[b].add(a)
    different: set[tuple[str, str]] = set()
    same: set[tuple[str, str]] = set()
    for reading in sorted(by_reading):
        words = sorted(w for w in by_reading[reading] if ngrams.get(w, 0) >= MIN_CALIBRATION_COUNT)
        for i, a in enumerate(words):
            for b in words[i + 1 :]:
                (same if is_variant_spelling(a, b, variants) else different).add((a, b))
            for k, (c, syllable) in enumerate(zip(a, reading.split(" "))):
                for v in others.get(c, set()) & chars.get(syllable, set()):
                    spelling = a[:k] + v + a[k + 1 :]
                    if ngrams.get(spelling, 0) >= MIN_CALIBRATION_COUNT:
                        same.add((min(a, spelling), max(a, spelling)))
    ordered = sorted(different)
    if len(ordered) > MAX_CALIBRATION_PAIRS:
        step = len(ordered) / MAX_CALIBRATION_PAIRS
        ordered = [ordered[int(i * step)] for i in range(MAX_CALIBRATION_PAIRS)]

    def rarer_first(pair: tuple[str, str]) -> tuple[str, str]:
        a, b = pair
        return (a, b) if (ngrams.get(a, 0), a) <= (ngrams.get(b, 0), b) else (b, a)

    return [rarer_first(p) for p in ordered], [rarer_first(p) for p in sorted(same)]


def _score(found: Contexts, index: dict[str, int]) -> Callable[[str, str], float]:
    return lambda a, b: found.score(index[a], index[b])


def percentile(values: list[float], q: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return 1.0
    k = (len(ordered) - 1) * q / 100
    low, high = math.floor(k), math.ceil(k)
    return ordered[low] + (ordered[high] - ordered[low]) * (k - low)


def run(mined: Path, packs: Path, corpora: list[str], lexicon: Path, unihan: Path, out: Path) -> dict:
    by_reading, chars = read_lexicon(lexicon)
    variants = unihan_variants(unihan)
    out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"percentile": PERCENTILE, "beta": BETA, "unihan_pairs": len(variants), "corpora": {}}
    previous: tuple[float, str] | None = None
    for corpus in corpora:
        _, ngrams, _ = read_ngrams(mined / f"{corpus}.ngrams.tsv")
        pack = read_pack(packs / f"{corpus}.tsv")
        rivals = {word: competitors(word, reading, by_reading, chars, ngrams) for word, reading in pack}
        different, same = calibration_pairs(by_reading, chars, ngrams, variants)
        words = {w for w, _ in pack} | {c for r in rivals.values() for c in r}
        targets = sorted(words | {w for p in different + same for w in p})
        index = {w: i for i, w in enumerate(targets)}
        vectors = Contexts(np.load(mined / f"{corpus}.ids.npy", mmap_mode="r"), targets)
        np.savez_compressed(
            out / f"{corpus}.contexts.npz",
            target=vectors.target,
            feature=vectors.feature,
            count=vectors.count,
            occurrences=vectors.occurrences,
            unigram=vectors.unigram,
        )
        (out / f"{corpus}.targets.txt").write_text("\n".join(targets) + "\n", encoding="utf-8")
        sim = _score(vectors, index)

        different_sims = [sim(a, b) for a, b in different]
        same_sims = [sim(a, b) for a, b in same]
        threshold, source = percentile(different_sims, PERCENTILE), corpus
        if len(different) < MIN_PAIRS and previous is not None:
            threshold, source = previous
        previous = (threshold, source)
        counts: collections.Counter[str] = collections.Counter()
        rows = []
        for word, reading in pack:
            n = int(vectors.occurrences[index[word]])
            best = ("", "", -math.inf, 0)
            for rival, origin in rivals[word].items():
                m = int(vectors.occurrences[index[rival]])
                if m > n:
                    s = sim(word, rival)
                    if s > best[2]:
                        best = (rival, origin, s, m)
            verdict = "variant" if best[0] and best[2] >= threshold else "kept"
            counts[verdict] += 1
            shown = f"{best[2]:.4f}" if best[0] else ""
            rows.append(f"{word}\t{reading}\t{best[0]}\t{best[1]}\t{shown}\t{n}\t{best[3]}\t{verdict}\n")
        header = "# word\treading\tcompetitor\tsource\tscore\tcount\tcompetitor_count\tverdict\n"
        (out / f"{corpus}.variants.tsv").write_text(header + "".join(rows), encoding="utf-8")
        calibration = [f"different\t{a}\t{b}\t{s:.4f}\n" for (a, b), s in zip(different, different_sims)]
        calibration += [f"same\t{a}\t{b}\t{s:.4f}\n" for (a, b), s in zip(same, same_sims)]
        (out / f"{corpus}.calibration.tsv").write_text("".join(calibration), encoding="utf-8")
        summary["corpora"][corpus] = {
            "threshold": round(threshold, 4),
            "threshold_from": source,
            "different_pairs": len(different),
            "same_pairs": len(same),
            "same_pairs_reaching_threshold": sum(s >= threshold for s in same_sims),
            "different_median": round(percentile(different_sims, 50), 4),
            "same_median": round(percentile(same_sims, 50), 4),
            "words": dict(counts),
        }
    (out / "variants.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mined", type=Path, required=True)
    parser.add_argument("--packs", type=Path, required=True)
    parser.add_argument("--corpus", nargs="+", required=True)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--unihan", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    print(json.dumps(run(args.mined, args.packs, args.corpus, args.lexicon, args.unihan, args.out), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Word discovery: frequent Han n-grams that behave like words, mined from licensed corpora (GPU host, CPU only).

    python -m zhuyintype_ml.training.lexicon.mine prepare --corpus wiki --out DIR FILES...  [--exclude EVAL...]
    python -m zhuyintype_ml.training.lexicon.mine count --out DIR --corpus wiki --workers 16 --min-count 20
    python -m zhuyintype_ml.training.lexicon.mine select --out DIR --corpus wiki --lexicon BASE.tsv

prepare  turns a corpus into one array of code points (`<corpus>.ids.npy`; everything that is not a Han character
         is a 0 separator).  Lines sharing any 8-character substring with an evaluation set are dropped first, the
         same rule the training corpora use.  Inputs: text files (one sentence per line), the chat corpus
         (real_dialogues.jsonl; documents reserved for its dev and test sets are skipped) or the business registry
         (BGMOPEN1.zip: the business names, or with --registry-field address the addresses).
count    counts every n-gram of 1 to 7 characters exactly.  Each worker reads the whole array and keeps the
         n-grams whose 64-bit polynomial hash falls in its share, so no counts are merged; n-grams seen fewer than
         --min-count times are dropped.  Then, for the n-grams of 2 to 6 characters, the characters before and after
         each occurrence are counted (branching entropy).  Writes `<corpus>.ngrams.tsv`.
select   keeps an n-gram of 2 to 6 characters as a word when it is as word-like as the dictionary's own words:
           cohesion  the smallest pointwise mutual information over its two-part splits,
           freedom   the smaller of its left and right branching entropies,
           edges     how often its first and last characters are words by themselves (the Wikipedia model's
                     unigram against the character's frequency here; 的人 starts or ends with such a character),
         cohesion and freedom at least the median of the same statistic over the base lexicon's words of that length
         found in the same corpus, edges at most the median (so the thresholds come from the data, per corpus and
         length; at the 10th percentile 的一, 他的 pass).  Words already in the lexicon are skipped.  Writes
         `<corpus>.mined.tsv` (word, count, cohesion, freedom) and `<corpus>.json`.

Needs numpy (the `train` dependency group).
"""

from __future__ import annotations

import argparse
import collections
import json
import math
import re
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

from .corpora import corpus_lines

HAN_LOW, HAN_HIGH = 0x4E00, 0x9FFF
EXT_A_LOW, EXT_A_HIGH = 0x3400, 0x4DBF
MAX_N = 7  # counted; words are at most 6 characters, the 7-grams give the 6-grams' neighbours
WORD_N = (2, 6)
MULTIPLIER = np.uint64(0x9E3779B97F4A7C15)
PERCENTILE = 50  # a mined word must be as word-like as the median dictionary word of its length


def han_runs(text: str) -> list[str]:
    return re.findall(r"[㐀-䶿一-鿿]+", text)


def eight_grams(lines: list[str]) -> set[str]:
    grams: set[str] = set()
    for line in lines:
        for run in han_runs(line):
            grams.update(run[i : i + 8] for i in range(len(run) - 7))
    return grams


def contained(line: str, grams: set[str]) -> bool:
    return any(run[i : i + 8] in grams for run in han_runs(line) for i in range(len(run) - 7))


def prepare(kind: str, files: list[Path], excludes: list[Path], out: Path, registry_field: str) -> dict:
    lines = corpus_lines(kind, files, registry_field)
    grams = eight_grams([line for path in excludes for line in path.read_text(encoding="utf-8").splitlines()])
    kept = [line for line in lines if not contained(line, grams)]
    text = "\n".join(kept) + "\n"
    ids = np.frombuffer(text.encode("utf-32-le"), dtype=np.uint32).copy()
    han = ((ids >= HAN_LOW) & (ids <= HAN_HIGH)) | ((ids >= EXT_A_LOW) & (ids <= EXT_A_HIGH))
    ids[~han] = 0
    out.mkdir(parents=True, exist_ok=True)
    np.save(out / f"{kind}.ids.npy", ids)
    summary = {
        "lines": len(lines),
        "dropped_for_eval_overlap": len(lines) - len(kept),
        "han_characters": int(han.sum()),
        "files": len(files),
        "evaluation_files_excluded_against": len(excludes),
    }
    (out / f"{kind}.prepare.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
    return summary


# MARK: - Counting


def _hashes(ids: np.ndarray, start: int, stop: int, n: int) -> tuple[np.ndarray, np.ndarray]:
    """(hash, valid) of every n-gram starting in [start, stop) (the slice reads n past stop)."""
    size = stop - start
    padded = np.zeros(size + n, dtype=np.uint64)
    x = np.asarray(ids[start : min(len(ids), stop + n)], dtype=np.uint64)
    padded[: len(x)] = x
    h = np.zeros(size, dtype=np.uint64)
    valid = np.ones(size, dtype=bool)
    for k in range(n):
        column = padded[k : k + size]
        h = h * MULTIPLIER + column
        valid &= column != 0
    return h, valid


def _reduce(keys: list[np.ndarray], counts: np.ndarray, extra: np.ndarray | None = None) -> tuple:
    """Sums `counts` over equal key tuples (the last key varies fastest); keeps the first `extra` of each."""
    order = np.lexsort(tuple(reversed(keys)))
    keys = [k[order] for k in keys]
    counts = counts[order]
    change = np.zeros(len(counts), dtype=bool)
    if len(counts):
        change[0] = True
        for k in keys:
            change[1:] |= k[1:] != k[:-1]
    first = np.flatnonzero(change)
    summed = np.add.reduceat(counts, first) if len(first) else counts[:0]
    reduced = [k[first] for k in keys]
    return reduced, summed, (extra[order][first] if extra is not None else None)


def entropies(hashes: np.ndarray, neighbours: np.ndarray, counts: np.ndarray) -> dict[int, float]:
    """Branching entropy of each hash from (hash, neighbour, count) rows; a 0 neighbour (a sentence boundary or a
    non-Han character) counts as a different neighbour each time."""
    result: dict[int, float] = {}
    if not len(hashes):
        return result
    (h, nb), c, _ = _reduce([hashes, neighbours], counts)
    starts = np.flatnonzero(np.concatenate([[True], h[1:] != h[:-1]]))
    totals = np.add.reduceat(c, starts).astype(np.float64)
    group = np.repeat(np.arange(len(starts)), np.diff(np.append(starts, len(h))))
    total = totals[group]
    term = np.where(nb == 0, c * np.log(total), -c * np.log(c / total))
    values = np.add.reduceat(term, starts) / totals
    return dict(zip(h[starts].tolist(), values.tolist()))


def count_share(path: str, worker: int, workers: int, min_count: int, chunk: int = 20_000_000) -> dict:
    ids = np.load(path, mmap_mode="r")
    share, me = np.uint64(workers), np.uint64(worker)
    counts: dict[str, int] = {}
    freedom: dict[str, tuple[float, float]] = {}
    for n in range(1, MAX_N + 1):
        hashes, totals, positions = np.zeros(0, np.uint64), np.zeros(0, np.int64), np.zeros(0, np.int64)
        for start in range(0, len(ids), chunk):
            stop = min(len(ids), start + chunk)
            h, valid = _hashes(ids, start, stop, n)
            index = np.flatnonzero(valid & (h % share == me))
            unique, first, c = np.unique(h[index], return_index=True, return_counts=True)
            (hashes,), totals, positions = _reduce(
                [np.concatenate([hashes, unique])],
                np.concatenate([totals, c.astype(np.int64)]),
                np.concatenate([positions, index[first] + start]),
            )
        keep = totals >= min_count
        names = {}
        for h_value, c_value, p in zip(hashes[keep].tolist(), totals[keep].tolist(), positions[keep].tolist()):
            word = "".join(map(chr, ids[p : p + n]))
            counts[word] = c_value
            names[h_value] = word
        if not WORD_N[0] <= n <= WORD_N[1] or not names:
            continue
        wanted = np.sort(np.fromiter(names, dtype=np.uint64, count=len(names)))
        empty = (np.zeros(0, np.uint64), np.zeros(0, np.int64), np.zeros(0, np.int64))
        sides = {"l": empty, "r": empty}
        for start in range(0, len(ids), chunk):
            stop = min(len(ids), start + chunk)
            h, valid = _hashes(ids, start, stop, n)
            index = np.flatnonzero(valid & (h % share == me))
            index = index[np.isin(h[index], wanted)]
            absolute = index + start
            before = np.asarray(ids[np.maximum(absolute - 1, 0)], dtype=np.int64)
            before[absolute == 0] = 0
            after = np.asarray(ids[np.minimum(absolute + n, len(ids) - 1)], dtype=np.int64)
            for side, neighbour in (("l", before), ("r", after)):
                old_h, old_n, old_c = sides[side]
                (hh, nb), c, _ = _reduce(
                    [np.concatenate([old_h, h[index]]), np.concatenate([old_n, neighbour])],
                    np.concatenate([old_c, np.ones(len(index), np.int64)]),
                )
                sides[side] = (hh, nb, c)
        left = entropies(*sides["l"])
        right = entropies(*sides["r"])
        for h_value, word in names.items():
            freedom[word] = (left.get(h_value, 0.0), right.get(h_value, 0.0))
    return {"counts": counts, "freedom": freedom}


def count(out: Path, kind: str, workers: int, min_count: int) -> dict:
    path = str(out / f"{kind}.ids.npy")
    with ProcessPoolExecutor(workers) as pool:
        parts = list(
            pool.map(count_share, [path] * workers, range(workers), [workers] * workers, [min_count] * workers)
        )
    counts: dict[str, int] = {}
    freedom: dict[str, tuple[float, float]] = {}
    for part in parts:
        counts.update(part["counts"])
        freedom.update(part["freedom"])
    total = int((np.load(path, mmap_mode="r") != 0).sum())
    with open(out / f"{kind}.ngrams.tsv", "w", encoding="utf-8") as handle:
        handle.write(f"# total\t{total}\n")
        for word in sorted(counts):
            left, right = freedom.get(word, (math.nan, math.nan))
            handle.write(f"{word}\t{counts[word]}\t{left:.4f}\t{right:.4f}\n")
    return {"ngrams": len(counts), "han_characters": total}


# MARK: - Selection


def read_ngrams(path: Path) -> tuple[int, dict[str, int], dict[str, tuple[float, float]]]:
    counts: dict[str, int] = {}
    freedom: dict[str, tuple[float, float]] = {}
    total = 0
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("# total"):
                total = int(line.split("\t")[1])
                continue
            word, c, left, right = line.rstrip("\n").split("\t")
            counts[word] = int(c)
            if left != "nan":
                freedom[word] = (float(left), float(right))
    return total, counts, freedom


def cohesion(word: str, counts: dict[str, int], total: int) -> float:
    """The smallest PMI over the two-part splits of `word` (characters, not words)."""
    c = counts[word]
    best = math.inf
    for i in range(1, len(word)):
        a, b = counts.get(word[:i]), counts.get(word[i:])
        if not a or not b:
            continue
        best = min(best, math.log(c * total / (a * b)))
    return best


def standalone(lm: Path, counts: dict[str, int], total: int) -> dict[str, float]:
    """How often each character is a word by itself: its unigram probability as a token in the language model
    (lm/unigram.tsv) over its share of characters in this corpus, scaled by the model's tokens per character
    (lm/stats.json).  的 is about 1; a character mostly inside words (蚵, 澎) is near 0."""
    stats = json.loads((lm / "stats.json").read_text())["corpus"]
    per_char = stats["training_tokens_excluding_boundaries"] / stats["training_chars"]
    result: dict[str, float] = {}
    with open(lm / "unigram.tsv", encoding="utf-8") as lines:
        for line in lines:
            parts = line.split()
            if len(parts) == 2 and len(parts[0]) == 1 and counts.get(parts[0]):
                result[parts[0]] = math.exp(float(parts[1])) * per_char / (counts[parts[0]] / total)
    return result


def select(out: Path, kind: str, known: set[str], lm: Path | None = None) -> dict:
    total, counts, freedom = read_ngrams(out / f"{kind}.ngrams.tsv")
    alone = standalone(lm, counts, total) if lm else {}

    def edge(word: str) -> float:
        return max(alone.get(word[0], 0.0), alone.get(word[-1], 0.0))

    stats: dict[str, tuple[float, float, float]] = {}
    for word in counts:
        if WORD_N[0] <= len(word) <= WORD_N[1] and word in freedom:
            stats[word] = (cohesion(word, counts, total), min(freedom[word]), edge(word))
    thresholds: dict[int, tuple[float, float, float]] = {}
    for n in range(WORD_N[0], WORD_N[1] + 1):
        reference = [stats[w] for w in stats if len(w) == n and w in known and math.isfinite(stats[w][0])]
        if len(reference) < 50:
            continue
        thresholds[n] = (
            float(np.percentile([s[0] for s in reference], PERCENTILE)),
            float(np.percentile([s[1] for s in reference], PERCENTILE)),
            float(np.percentile([s[2] for s in reference], 100 - PERCENTILE)),
        )
    mined = []
    for word, (pmi, free, alone_edge) in stats.items():
        if word in known or len(word) not in thresholds:
            continue
        need_pmi, need_free, max_edge = thresholds[len(word)]
        if pmi >= need_pmi and free >= need_free and alone_edge <= max_edge:
            mined.append((word, counts[word], pmi, free))
    mined.sort(key=lambda x: (-x[1], x[0]))
    with open(out / f"{kind}.mined.tsv", "w", encoding="utf-8") as handle:
        handle.write(f"# total\t{total}\n")
        for word, c, pmi, free in mined:
            handle.write(f"{word}\t{c}\t{pmi:.3f}\t{free:.3f}\n")
    summary = {
        "han_characters": total,
        "ngrams": len(counts),
        "candidates": len(stats),
        "percentile": PERCENTILE,
        "thresholds_by_length": {
            n: {"cohesion": t[0], "freedom": t[1], "standalone_edge": t[2]} for n, t in thresholds.items()
        },
        "reference_words_by_length": {
            n: sum(1 for w in stats if len(w) == n and w in known) for n in range(WORD_N[0], WORD_N[1] + 1)
        },
        "mined": len(mined),
        "mined_by_length": dict(collections.Counter(len(w) for w, *_ in mined)),
    }
    (out / f"{kind}.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
    return summary


def lexicon_words(path: Path) -> set[str]:
    with open(path, encoding="utf-8") as lines:
        return {line.split("\t")[1] for line in lines if not line.startswith("#")}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("action", choices=("prepare", "count", "select"))
    parser.add_argument("files", nargs="*", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--corpus", required=True, help="corpus name: wiki, chat, registry, places")
    parser.add_argument("--exclude", type=Path, action="append", default=[], help="evaluation text (repeatable)")
    parser.add_argument("--registry-field", choices=("name", "address"), default="name")
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--min-count", type=int, default=20)
    parser.add_argument("--lexicon", type=Path, help="base lexicon (select)")
    parser.add_argument(
        "--lm", type=Path, help="the Wikipedia language model directory (select: standalone characters)"
    )
    args = parser.parse_args(argv)
    if args.action == "prepare":
        summary = prepare(args.corpus, args.files, args.exclude, args.out, args.registry_field)
    elif args.action == "count":
        summary = count(args.out, args.corpus, args.workers, args.min_count)
    else:
        summary = select(args.out, args.corpus, lexicon_words(args.lexicon), args.lm)
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

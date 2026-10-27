"""ARPA → ZTLM/1 (FORMAT.md: natural logs, float32, order <= 3), an mmap reader and an independent verifier.

Writes atomically and reserves the UTF-8 sorted </s>, <s>, <unk> ids.  `--max-trigrams` keeps the most probable
explicit trigrams and recomputes the backoff mass.  The shipped bigrams are compiled from TSV by compile_data.py
instead; this tool reads ARPA files (KenLM output) and checks any lm.bin.  Needs NumPy (the train dependency group).

    python -m nlp.training.lm.lmbin write model.arpa lm.bin [--max-trigrams N]
    python -m nlp.training.lm.lmbin verify data/build/lm/lm.bin --samples 256
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import mmap
import os
import struct
from collections.abc import Sequence
from pathlib import Path
from typing import Any

import numpy as np

HEADER = struct.Struct("<4s5I5Q")
SPECIAL = ["</s>", "<s>", "<unk>"]
LN10 = math.log(10)


def read_arpa(path: Path | str) -> tuple[list[str], np.ndarray, np.ndarray, dict[int, np.ndarray]]:
    """(UTF-8 sorted vocabulary, unigram and backoff log scores in that order, n-gram records by order)."""
    counts: dict[int, int] = {}
    order = 0
    rows: dict[int, np.ndarray] = {}
    words: list[str] = []
    uni_scores: list[float] = []
    back_scores: list[float] = []
    ids: dict[str, int] = {}
    used: dict[int, int] = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line.startswith("ngram "):
                n, c = line[6:].split("=")
                counts[int(n)] = int(c)
            elif line.startswith("\\") and line.endswith("-grams:"):
                order = int(line[1:].split("-")[0])
                used[order] = 0
                if order > 3:
                    raise ValueError("ZTLM supports only order <=3")
                if order > 1:
                    rows[order] = np.empty(counts[order], dtype=[("ids", "<u4", (order,)), ("p", "<f8"), ("bo", "<f8")])
            elif line == "\\end\\":
                break
            elif order and line and not line.startswith("\\"):
                fields = line.split()
                if len(fields) not in (order + 1, order + 2):
                    raise ValueError(f"malformed ARPA row: {line}")
                p = float(fields[0]) * LN10
                bo = float(fields[-1]) * LN10 if len(fields) == order + 2 else 0.0
                if not math.isfinite(p) or not math.isfinite(bo) or p > 1e-5:
                    raise ValueError("invalid ARPA probability")
                if order == 1:
                    word = fields[1]
                    if word in ids:
                        raise ValueError("duplicate unigram")
                    ids[word] = len(words)
                    words.append(word)
                    uni_scores.append(p)
                    back_scores.append(bo)
                else:
                    rows[order][used[order]] = (tuple(ids[w] for w in fields[1 : 1 + order]), p, bo)
                used[order] += 1
    if counts != used or max(counts) not in (2, 3):
        raise ValueError(f"ARPA count/order mismatch: {counts} != {used}")
    canonical = sorted(words, key=lambda w: w.encode("utf-8"))
    if canonical[:3] != SPECIAL:
        raise ValueError("ARPA vocabulary violates reserved UTF-8 ordering; filter ASCII fallback tokens upstream")
    newids = {w: i for i, w in enumerate(canonical)}
    mapping = np.array([newids[w] for w in words], dtype=np.uint32)
    perm = np.argsort(mapping)
    uni = np.array(uni_scores)[perm]
    back = np.array(back_scores)[perm]
    for level, records in rows.items():
        records["ids"] = mapping[records["ids"]]
        keys = records["ids"]
        idx = np.lexsort(tuple(keys[:, i] for i in reversed(range(level))))
        rows[level] = records[idx]
        k = rows[level]["ids"]
        if len(k) > 1 and np.any(np.all(k[1:] == k[:-1], axis=1)):
            raise ValueError("duplicate ngram")
    return canonical, uni, back, rows


def starts(groups: np.ndarray, size: int) -> np.ndarray:
    return np.r_[0, np.cumsum(np.bincount(groups, minlength=size), dtype=np.uint64)].astype("<u4")


def renormalize(starts_: np.ndarray, prob: np.ndarray, lower: np.ndarray, size: int) -> np.ndarray:
    """Log backoff weights that give each context's unstored successors exactly the mass the stored ones leave."""
    groups = np.repeat(np.arange(size), np.diff(starts_).astype(np.int64))
    mass = np.bincount(groups, weights=np.exp(prob), minlength=size)
    base = np.bincount(groups, weights=lower, minlength=size)
    if np.any(mass > 1.00002) or np.any(base > 1.00002):
        raise ValueError("stored conditional mass exceeds one")
    alpha = np.ones(size)
    active = np.diff(starts_) > 0
    numerator = np.maximum(0.0, 1.0 - mass[active])
    denominator = np.maximum(0.0, 1.0 - base[active])
    if np.any((denominator < 1e-14) & (numerator > 2e-5)):
        raise ValueError("no lower-order mass to redistribute")
    alpha[active] = np.divide(numerator, denominator, out=np.ones_like(numerator), where=denominator > 1e-14)
    return np.log(np.maximum(alpha, 1e-38))


def lookup_bigram(
    keys: np.ndarray | None, prob: np.ndarray, uni: np.ndarray, back: np.ndarray, first: Any, next_: Any
) -> Any:
    if keys is None or not len(keys):
        return back[first] + uni[next_]
    query = (np.asarray(first, dtype=np.uint64) << np.uint64(32)) | np.asarray(next_, dtype=np.uint64)
    index = np.searchsorted(keys, query)
    safe = np.minimum(index, len(keys) - 1)
    present = (index < len(keys)) & (keys[safe] == query)
    return np.where(present, prob[safe], back[first] + uni[next_])


def write(arpa: Path | str, target: Path | str, max_trigrams: int | None = None) -> dict[str, Any]:
    if max_trigrams is not None and max_trigrams < 0:
        raise ValueError("max_trigrams must be nonnegative")
    words, uni, _, rows = read_arpa(arpa)
    v = len(words)
    b = rows[2]
    t = rows.get(3)
    # Normalize the ARPA's rounded unigram distribution before float32 export.
    uni -= math.log(float(np.exp(uni).sum()))
    if t is not None and max_trigrams is not None and len(t) > max_trigrams:
        chosen = np.argpartition(t["p"], -max_trigrams)[-max_trigrams:] if max_trigrams else np.array([], dtype=int)
        t = t[np.sort(chosen)]
    bs = starts(b["ids"][:, 0], v)
    bn = b["ids"][:, 1].astype("<u4")
    bp = b["p"]
    ub = renormalize(bs, bp, np.exp(uni[bn]), v)
    keys = (b["ids"][:, 0].astype(np.uint64) << np.uint64(32)) | b["ids"][:, 1]
    ts: np.ndarray | None = None
    tn: np.ndarray | None = None
    tp: np.ndarray | None = None
    if t is not None:
        contexts = (t["ids"][:, 0].astype(np.uint64) << np.uint64(32)) | t["ids"][:, 1]
        ci = np.searchsorted(keys, contexts)
        if np.any(ci >= len(keys)) or np.any(keys[np.minimum(ci, len(keys) - 1)] != contexts):
            raise ValueError("missing trigram prefix bigram")
        ts = starts(ci, len(b))
        tn = t["ids"][:, 2].astype("<u4")
        tp = t["p"]
        lower = np.exp(lookup_bigram(keys, bp, uni, ub, t["ids"][:, 1], tn))
        bb = renormalize(ts, tp, lower, len(b))
    else:
        bb = np.zeros(len(b))
    offsets = np.r_[0, np.cumsum([len(w.encode()) for w in words])].astype("<u4")
    blob = "".join(words).encode()
    target = Path(target)
    tmp = target.with_suffix(target.suffix + ".tmp")
    positions: list[int] = []
    with tmp.open("wb") as f:
        f.write(bytes(64))

        def section(parts: list[Any]) -> None:
            f.write(bytes((-f.tell()) % 8))
            positions.append(f.tell())
            for part in parts:
                f.write(part if isinstance(part, bytes) else part.tobytes())

        section([offsets, blob])
        section([uni.astype("<f4"), ub.astype("<f4")])
        section([bs, bn, bp.astype("<f4"), bb.astype("<f4")])
        if t is not None and tp is not None:
            section([ts, tn, tp.astype("<f4")])
        size = f.tell()
        f.seek(0)
        f.write(
            HEADER.pack(
                b"ZTLM",
                1,
                3 if t is not None else 2,
                v,
                len(b),
                len(t) if t is not None else 0,
                *positions[:3],
                positions[3] if t is not None else 0,
                size,
            )
        )
        f.flush()
        os.fsync(f.fileno())
    result = verify(tmp)
    tmp.replace(target)
    return result


class Model:
    """A ZTLM/1 file mapped read-only, with its header and sections validated."""

    def __init__(self, path: Path | str) -> None:
        with open(path, "rb") as file:
            self.mm = mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ)
        if len(self.mm) < 64:
            raise ValueError("truncated header")
        magic, version, self.order, self.v, self.b, self.t, vo, uo, bo, to, size = HEADER.unpack_from(self.mm)
        if magic != b"ZTLM" or version != 1 or self.order not in (2, 3) or size != len(self.mm):
            raise ValueError("invalid header")
        if self.v < 3 or (self.order == 2 and (self.t or to)):
            raise ValueError("invalid counts/order")
        sections = [vo, uo, bo] + ([to] if self.order == 3 else [])
        if sections[0] != 64 or any(x % 8 for x in sections) or sections != sorted(set(sections)):
            raise ValueError("invalid section alignment/order")

        def array(offset: int, n: int, dtype: str = "<u4") -> np.ndarray:
            if offset < 64 or offset + n * 4 > size:
                raise ValueError("array out of bounds")
            return np.ndarray(n, dtype=dtype, buffer=self.mm, offset=offset)

        self.offsets = array(vo, self.v + 1)
        if self.offsets[0] != 0 or np.any(self.offsets[1:] <= self.offsets[:-1]):
            raise ValueError("invalid string offsets")
        start = vo + 4 * (self.v + 1)
        end = start + int(self.offsets[-1])
        if end > uo:
            raise ValueError("vocabulary overlaps unigrams")
        self.words = [
            self.mm[start + int(a) : start + int(b)].decode("utf-8")
            for a, b in zip(self.offsets[:-1], self.offsets[1:])
        ]
        if self.words[:3] != SPECIAL or any(a.encode() >= b.encode() for a, b in zip(self.words, self.words[1:])):
            raise ValueError("invalid vocabulary order")
        self.ids = {w: i for i, w in enumerate(self.words)}
        self.uni = array(uo, self.v, "<f4")
        self.ub = array(uo + 4 * self.v, self.v, "<f4")
        self.bs = array(bo, self.v + 1)
        self.bn = array(bo + 4 * (self.v + 1), self.b)
        self.bp = array(bo + 4 * (self.v + 1 + self.b), self.b, "<f4")
        self.bb = array(bo + 4 * (self.v + 1 + 2 * self.b), self.b, "<f4")
        self.ts: np.ndarray | None = None
        self.tn: np.ndarray | None = None
        self.tp: np.ndarray | None = None
        if self.order == 3:
            self.ts = array(to, self.b + 1)
            self.tn = array(to + 4 * (self.b + 1), self.t)
            self.tp = array(to + 4 * (self.b + 1 + self.t), self.t, "<f4")
        ends = [end, uo + 8 * self.v, bo + 4 * (self.v + 1 + 3 * self.b)] + (
            [to + 4 * (self.b + 1 + 2 * self.t)] if self.order == 3 else []
        )
        for end, next_ in zip(ends, sections[1:] + [size]):
            if end > next_ or next_ - end > 7 or any(self.mm[end:next_]):
                raise ValueError("section overlap/padding/trailing bytes")
        self.keys: np.ndarray | None = (
            (np.repeat(np.arange(self.v, dtype=np.uint64), np.diff(self.bs).astype(np.int64)) << np.uint64(32))
            | self.bn.astype(np.uint64)
            if self.bs[-1] == self.b and np.all(self.bs[1:] >= self.bs[:-1])
            else None
        )

    def p2(self, first: Any, next_: Any) -> Any:
        return lookup_bigram(self.keys, self.bp, self.uni, self.ub, first, next_)

    def score(self, word: str, history: Sequence[str] = ()) -> float:
        """log P(word | the last two words of history); unknown words are <unk>."""
        w = self.ids.get(word, 2)
        context = [self.ids.get(x, 2) for x in history[-2:]]
        if not context:
            return float(self.uni[w])
        first = context[-1]
        left, right = int(self.bs[first]), int(self.bs[first + 1])
        j = left + int(np.searchsorted(self.bn[left:right], w))
        score = float(self.bp[j]) if j < right and self.bn[j] == w else float(self.ub[first] + self.uni[w])
        if self.order == 3 and len(context) == 2:
            assert self.ts is not None and self.tn is not None and self.tp is not None
            a = context[0]
            left, right = int(self.bs[a]), int(self.bs[a + 1])
            j = left + int(np.searchsorted(self.bn[left:right], first))
            if j < right and self.bn[j] == first:
                left, right = int(self.ts[j]), int(self.ts[j + 1])
                k = left + int(np.searchsorted(self.tn[left:right], w))
                score = float(self.tp[k]) if k < right and self.tn[k] == w else float(self.bb[j]) + score
        return score


def verify(path: Path | str, samples: int = 128) -> dict[str, Any]:
    """Structure, sorting, finiteness, unigram mass and sampled conditional normalization of an lm.bin."""
    m = Model(path)

    def group_check(starts_: np.ndarray, next_: np.ndarray, total: int, contexts: int) -> None:
        if starts_[0] != 0 or starts_[-1] != total or np.any(starts_[1:] < starts_[:-1]) or np.any(next_ >= m.v):
            raise ValueError("invalid ngram pointers/ids")
        same = np.ones(max(0, total - 1), dtype=bool)
        boundaries = starts_[1:-1]
        boundaries = boundaries[(boundaries > 0) & (boundaries < total)]
        same[boundaries - 1] = False
        if np.any((next_[1:] <= next_[:-1]) & same):
            raise ValueError("successors not strictly sorted")

    group_check(m.bs, m.bn, m.b, m.v)
    if m.order == 3:
        assert m.ts is not None and m.tn is not None
        group_check(m.ts, m.tn, m.t, m.b)
    for a in (m.uni, m.ub, m.bp, m.bb, m.tp):
        if a is not None and not np.all(np.isfinite(a)):
            raise ValueError("nonfinite scores")
    if np.any(m.uni > 1e-5) or np.any(m.bp > 1e-5) or (m.tp is not None and np.any(m.tp > 1e-5)):
        raise ValueError("probability >1")
    unigram_mass = float(np.exp(m.uni.astype(float)).sum())
    if abs(unigram_mass - 1) > 2e-5:
        raise ValueError(f"unigram mass {unigram_mass}")
    if np.any(m.ub[np.diff(m.bs) == 0] != 0) or (m.ts is not None and np.any(m.bb[np.diff(m.ts) == 0] != 0)):
        raise ValueError("backoff on empty context")
    rng = np.random.default_rng(20261026)
    errors: list[float] = []
    checked = {}
    levels: list[tuple[int, Any, Any, Any, int]] = [(2, m.bs, m.bp, m.ub, m.v)]
    if m.order == 3:
        levels.append((3, m.ts, m.tp, m.bb, m.b))
    for order, starts_, prob, back, _ in levels:
        active = np.flatnonzero(np.diff(starts_) > 0)
        selected = np.unique(
            np.r_[
                active[:8],
                active[np.argsort(np.diff(starts_)[active])[-8:]],
                rng.choice(active, min(samples, len(active)), replace=False),
            ]
        )
        for context in selected:
            lo, hi = int(starts_[context]), int(starts_[context + 1])
            if order == 2:
                lower = np.exp(m.uni[m.bn[lo:hi]].astype(float))
            else:
                assert m.tn is not None
                lower = np.exp(m.p2(np.full(hi - lo, int(m.bn[context])), m.tn[lo:hi]).astype(float))
            mass = float(
                np.exp(prob[lo:hi].astype(float)).sum()
                + math.exp(float(back[context])) * (unigram_mass - float(lower.sum()))
            )
            errors.append(abs(mass - 1))
        checked[str(order)] = len(selected)
    if errors and max(errors) > 5e-5:
        raise ValueError(f"conditional/backoff normalization error {max(errors)}")
    # Missing stored contexts back off directly; compare scalar and vector paths.
    for first, word in zip(rng.integers(0, m.v, 128), rng.integers(0, m.v, 128)):
        if abs(m.score(m.words[word], [m.words[first]]) - float(m.p2(np.array([first]), np.array([word]))[0])) > 2e-6:
            raise ValueError("backoff scoring mismatch")
    return {
        "format": "ZTLM/1",
        "order": m.order,
        "vocabulary": m.v,
        "bigrams": m.b,
        "trigrams": m.t,
        "bytes": len(m.mm),
        "unigram_mass": unigram_mass,
        "max_sample_normalization_error": max(errors, default=0),
        "contexts_checked": checked,
        "sha256": hashlib.sha256(m.mm).hexdigest(),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    w = sub.add_parser("write", help="ARPA to lm.bin")
    w.add_argument("arpa")
    w.add_argument("output")
    w.add_argument("--max-trigrams", type=int)
    v = sub.add_parser("verify", help="check an lm.bin")
    v.add_argument("path")
    v.add_argument("--samples", type=int, default=128)
    args = parser.parse_args(argv)
    result = (
        write(args.arpa, args.output, args.max_trigrams) if args.command == "write" else verify(args.path, args.samples)
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

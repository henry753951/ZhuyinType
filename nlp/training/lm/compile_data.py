"""Compile the shipped data for mmap: lexicon.tsv → lexicon.bin (ZTLX/1), lm/ and lm_chat/ TSVs → lm.bin (ZTLM/1).

Deterministic and offline: the same inputs give byte-identical binaries and JSON on the Mac and on Linux.  Reads
SOURCE/lexicon.tsv, SOURCE/lm/{unigram,bigram,backoff}.tsv and the same three in SOURCE/lm_chat/; writes
OUT/lexicon.{bin,json}, OUT/lm/lm.{bin,json} and OUT/lm_chat/lm.{bin,json}.  Each JSON records the binary's SHA-256,
size, format and the source hashes.  Formats: FORMAT.md, LEXICON_FORMAT.md.  Standard library only.

    python -m nlp.training.lm.compile_data data/build/compiled
    python -m nlp.training.lm.compile_data OUT --source data/build
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
from collections import defaultdict
from collections.abc import Iterable, Iterator
from pathlib import Path
from typing import Any

from nlp.common import paths

SPECIAL = ["</s>", "<s>", "<unk>"]
LM_HEADER = struct.Struct("<4s5I5Q")
LEX_HEADER = struct.Struct("<4s7I12Q")


def rows(path: Path, fields: int, spaces: bool = False) -> Iterator[tuple[Any, ...]]:
    # Match Swift scanTSV, including comments and its optional ASCII-space separator.
    for line in path.read_text(encoding="utf-8").split("\n"):
        if line.startswith("#"):
            continue
        parts = re.split("[\t ]+", line.rstrip("\r")) if spaces else line.rstrip("\r").split("\t")
        if spaces:
            parts = [x for x in parts if x]
        if len(parts) != fields:
            continue
        try:
            score = float(parts[-1])
        except ValueError:
            continue
        if math.isfinite(score):
            yield (*parts[:-1], score)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def publish(path: Path, data: bytearray, metadata: dict[str, Any]) -> None:
    """Replaces `path` atomically and writes its metadata beside it (same stem, .json)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".bin.tmp")
    tmp.write_bytes(data)
    tmp.replace(path)
    metadata.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    path.with_suffix(".json").write_text(json.dumps(metadata, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    print(f"{path}: {len(data):,} bytes")


def section(data: bytearray, content: bytes | bytearray) -> int:
    """Appends `content` at the next 8-byte boundary; returns its offset."""
    data.extend(bytes(-len(data) % 8))
    start = len(data)
    data.extend(content)
    return start


def pack_array(fmt: str, values: Iterable[Any]) -> bytes:
    record = struct.Struct("<" + fmt)
    return b"".join(record.pack(*v) if isinstance(v, tuple) else record.pack(v) for v in values)


def compile_lm(source: Path, target: Path) -> None:
    uni = dict(rows(source / "unigram.tsv", 2, True))
    bi = {(a, b): p for a, b, p in rows(source / "bigram.tsv", 3, True)}
    back = dict(rows(source / "backoff.tsv", 2, True))
    vocabulary = set(uni) | set(back) | {w for pair in bi for w in pair}
    words = SPECIAL + sorted(vocabulary - set(SPECIAL), key=lambda w: w.encode())
    ids = {w: i for i, w in enumerate(words)}
    pairs = sorted((ids[a], ids[b], p) for (a, b), p in bi.items())
    starts = [0] * (len(words) + 1)
    for a, _, _ in pairs:
        starts[a + 1] += 1
    for i in range(len(words)):
        starts[i + 1] += starts[i]
    offsets = [0]
    blob = bytearray()
    for word in words:
        blob.extend(word.encode())
        offsets.append(len(blob))
    data = bytearray(64)
    vo = section(data, pack_array("I", offsets) + blob)
    uo = section(
        data, pack_array("f", (uni.get(w, 0) for w in words)) + pack_array("f", (back.get(w, 0) for w in words))
    )
    bo = section(
        data,
        pack_array("I", starts)
        + pack_array("I", (b for _, b, _ in pairs))
        + pack_array("f", (p for _, _, p in pairs))
        + bytes(4 * len(pairs)),
    )
    data[:64] = LM_HEADER.pack(b"ZTLM", 1, 2, len(words), len(pairs), 0, vo, uo, bo, 0, len(data))
    publish(
        target / "lm.bin",
        data,
        {
            "format": "ZTLM/1",
            "order": 2,
            "vocabulary": len(words),
            "bigrams": len(pairs),
            "tsv_compatibility": True,
            "missing_unigrams": [ids[w] for w in words if w not in uni],
            "unknown_word_shift": 0,
            "vocabulary_order": "reserved-then-utf8",
            "source_hashes": {name: digest(source / name) for name in ["unigram.tsv", "bigram.tsv", "backoff.tsv"]},
        },
    )


def toneless(reading: str) -> str:
    return reading.translate(str.maketrans("", "", "ˊˇˋ˙"))


def compile_lexicon(source: Path, target: Path) -> None:
    original = [(r, w, p) for r, w, p in rows(source, 3) if r and w]
    words = sorted({w for _, w, _ in original}, key=lambda w: w.encode())
    readings = sorted({r for r, _, _ in original}, key=lambda r: r.encode())
    wi = {w: i for i, w in enumerate(words)}
    ri = {r: i for i, r in enumerate(readings)}
    top: dict[str, float] = {}
    for _, w, p in original:
        top[w] = max(top.get(w, -math.inf), p)
    grouped: defaultdict[str, list[tuple[str, float, int]]] = defaultdict(list)
    for ordinal, (r, w, p) in enumerate(original):
        grouped[r].append((w, p, ordinal))
    blob = bytearray()
    strings: dict[str, tuple[int, int]] = {}

    def string(s: str) -> tuple[int, int]:
        if s not in strings:
            b = s.encode()
            strings[s] = (len(blob), len(b))
            blob.extend(b)
        return strings[s]

    entries: list[tuple[int, int, float, float]] = []
    reading_records: list[tuple[int, ...]] = []
    reverse: defaultdict[str, list[tuple[int, int, float]]] = defaultdict(list)
    for r in readings:
        start = len(entries)
        for w, p, ordinal in grouped[r]:
            entry = len(entries)
            entries.append((wi[w], ri[r], top[w], p - top[w]))
            reverse[w].append((ordinal, entry, p))
        reading_records.append((*string(r), start, len(entries) - start))
    refs: list[int] = []
    word_records: list[tuple[int, ...]] = []
    for w in words:
        items = sorted(reverse[w])
        best = max(items, key=lambda x: x[2])[1]
        word_records.append((*string(w), len(refs), len(items), best, 0))
        refs.extend(entry for _, entry, _ in items)
    bases: defaultdict[str, list[int]] = defaultdict(list)
    prefixes: set[str] = set()
    syllables: set[str] = set()
    for r in readings:
        base = toneless(r)
        bases[base].append(ri[r])
        parts = base.split()
        prefixes.update(" ".join(parts[:i]) for i in range(1, len(parts) + 1))
        syllables.update(r.split())
    base_records: list[tuple[int, ...]] = []
    base_refs: list[int] = []
    for b in sorted(bases, key=lambda b: b.encode()):
        base_records.append((*string(b), len(base_refs), len(bases[b])))
        base_refs.extend(bases[b])
    prefix_records = [string(p) for p in sorted(prefixes, key=lambda p: p.encode())]
    syllable_records = [string(s) for s in sorted(syllables, key=lambda s: s.encode())]
    data = bytearray(128)
    tables: list[tuple[str, Iterable[Any]]] = [
        ("6I", word_records),
        ("4I", reading_records),
        ("2I2d", entries),
        ("I", refs),
        ("4I", base_records),
        ("I", base_refs),
        ("2I", prefix_records),
        ("2I", syllable_records),
    ]
    sections = [section(data, pack_array(fmt, values)) for fmt, values in tables]
    sections.append(section(data, blob))
    data[:128] = LEX_HEADER.pack(
        b"ZTLX",
        1,
        len(words),
        len(readings),
        len(entries),
        len(bases),
        len(prefixes),
        len(syllables),
        *sections,
        len(data),
        0,
        0,
    )
    publish(
        target / "lexicon.bin",
        data,
        {
            "format": "ZTLX/1",
            "words": len(words),
            "readings": len(readings),
            "entries": len(entries),
            "source_hashes": {"lexicon.tsv": digest(source)},
        },
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("output", type=Path)
    parser.add_argument("--source", type=Path, default=paths.data_dir(), help="default: the data directory")
    a = parser.parse_args(argv)
    compile_lm(a.source / "lm", a.output / "lm")
    compile_lm(a.source / "lm_chat", a.output / "lm_chat")
    compile_lexicon(a.source / "lexicon.tsv", a.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Clean the generated sentences as they arrive: strict Traditional/Taiwan filtering, held-out exclusion, exact and
MinHash deduplication.

Tails raw/synthetic.jsonl from a checkpointed offset.  The first 1,500,000 accepted lines go to
synthetic_phase1.txt (then PHASE1_READY is written), the rest to synthetic_background.txt, until 6,000,000 or a
STOP_CLEANER file.  Progress: clean-progress.json.  Runs beside the generator (zt train corpus generate).

    python -m nlp.training.corpus.clean --output data/build/corpus --phase1 1500000 --target 6000000
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import time
import unicodedata
from collections import Counter
from pathlib import Path
from typing import Any

from opencc import OpenCC

from nlp.common import paths

from .prepare import exclusion

MAINLAND = re.compile(
    "視頻|軟件|網絡|出租車|地鐵|自行車|打印|鼠標|硬盤|內存|程序員|服務器|信息化|質量很好|公交車|方便麵|土豆|西紅柿|酸奶|"
    "二維碼|充電寶|移動電源|快遞小哥|外賣|靠譜|性價比|小區|聯繫方式|賬號|默認|下載鏈接|優化體驗|點贊"
)
HAN = re.compile(r"[㐀-鿿]")


def normalized(line: str, s2t: OpenCC, s2twp: OpenCC) -> tuple[str | None, str | None]:
    """(cleaned sentence, None) or (None, rejection reason)."""
    s = unicodedata.normalize("NFC", line.strip())
    s = re.sub(r"^\s*(?:[-*•]|\d+[.、)．])\s*", "", s).strip(' "「」')
    if not 4 <= len(s) <= 80 or len(HAN.findall(s)) < 0.6 * len(s):
        return None, "length_or_han"
    if s2t.convert(s) != s:
        return None, "simplified"
    if any(x in s for x in ("http", "<think", "</", "```", "以下是", "批次編號")):
        return None, "markup"
    converted = s2twp.convert(s)
    # Phrase conversion is allowed only when it changes no Han spelling at all.
    if HAN.findall(converted) == HAN.findall(s):
        s = converted
    elif MAINLAND.search(s):
        return None, "mainland"
    if MAINLAND.search(s):
        return None, "mainland"
    return s, None


def shingles(s: str) -> set[str]:
    return {s[i : i + 3] for i in range(max(1, len(s) - 2))}


class Deduper:
    """Exact (BLAKE2b-128) and near-duplicate (64-permutation MinHash LSH over character trigrams, Jaccard >= .85
    confirmed exactly) rejection."""

    def __init__(self) -> None:
        from datasketch import MinHash, MinHashLSH

        self.MinHash = MinHash
        self.exact: set[bytes] = set()
        self.lines: list[str] = []
        self.lsh = MinHashLSH(threshold=0.85, num_perm=64)

    def add(self, s: str) -> bool:
        key = hashlib.blake2b(s.encode(), digest_size=16).digest()
        if key in self.exact:
            return False
        grams = shingles(s)
        mh = self.MinHash(num_perm=64)
        mh.update_batch([g.encode() for g in grams])
        for i in self.lsh.query(mh):
            other = shingles(self.lines[i])
            if len(grams & other) / len(grams | other) >= 0.85:
                return False
        i = len(self.lines)
        self.lines.append(s)
        self.exact.add(key)
        self.lsh.insert(i, mh)
        return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, default=paths.data("corpus"))
    parser.add_argument("--wikipedia-test", type=Path, default=paths.data("lm", "test_sentences.txt"))
    parser.add_argument("--phase1", type=int, default=1500000)
    parser.add_argument("--target", type=int, default=6000000)
    args = parser.parse_args(argv)
    out = args.output
    raw = out / "raw/synthetic.jsonl"
    held: list[str] = []
    for name in ["test_colloquial.txt", "dev_colloquial.txt", "dev_wikipedia.txt"]:
        held += (out / name).read_text().splitlines()
    held += args.wikipedia_test.read_text().splitlines()
    blocked = exclusion(held)
    dedup = Deduper()
    counts: Counter[str] = Counter()
    s2t = OpenCC("s2t")
    tw = OpenCC("s2twp")
    files = [out / "synthetic_phase1.txt", out / "synthetic_background.txt"]
    for file in files:
        if file.exists():
            with file.open() as f:
                for line in f:
                    dedup.add(line.rstrip("\n"))
    counts["accepted"] = len(dedup.lines)
    checkpoint = out / "clean-checkpoint.json"
    offset = json.loads(checkpoint.read_text())["offset"] if checkpoint.exists() else 0
    handles = [f.open("a", buffering=1) for f in files]

    def save() -> None:
        for h in handles:
            h.flush()
        temp = checkpoint.with_suffix(".tmp")
        temp.write_text(json.dumps({"offset": offset}))
        temp.replace(checkpoint)
        progress: dict[str, Any] = {**counts, "raw_offset": offset, "updated": time.time()}
        temp = out / "clean-progress.tmp"
        temp.write_text(json.dumps(progress, indent=2))
        temp.replace(out / "clean-progress.json")

    last = time.time()
    try:
        while counts["accepted"] < args.target:
            if not raw.exists():
                time.sleep(2)
                continue
            with raw.open("rb") as source:
                source.seek(offset)
                while True:
                    line = source.readline()
                    if not line or not line.endswith(b"\n"):
                        break
                    record = json.loads(line)
                    for item in record["text"].splitlines():
                        counts["raw_lines"] += 1
                        s, reason = normalized(item, s2t, tw)
                        if reason or s is None:
                            counts[str(reason)] += 1
                            continue
                        if blocked(s):
                            counts["heldout_overlap"] += 1
                            continue
                        if not dedup.add(s):
                            counts["duplicate"] += 1
                            continue
                        index = 0 if counts["accepted"] < args.phase1 else 1
                        handles[index].write(s + "\n")
                        counts["accepted"] += 1
                        if counts["accepted"] == args.phase1:
                            handles[0].flush()
                            (out / "PHASE1_READY").write_text(str(args.phase1) + "\n")
                    offset = source.tell()
                    if time.time() - last > 20:
                        save()
                        print(json.dumps(dict(counts)), flush=True)
                        last = time.time()
                    if counts["accepted"] >= args.target:
                        break
            save()
            if (out / "STOP_CLEANER").exists():
                break
            time.sleep(2)
    finally:
        save()
        for h in handles:
            h.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

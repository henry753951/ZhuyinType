"""Freeze the colloquial and Wikipedia dev/test sets before any generation: Common Voice zh-TW splits, Wikipedia dev.

Downloads Common Voice's CC0 sentence-collector.txt at the current commit (with the submission policy as licence
evidence), keeps unique Traditional lines of 4–60 characters, and fixes test_colloquial.txt (3,000) and
dev_colloquial.txt (2,000) with seed 20261026; train_commonvoice.txt keeps the rest minus anything sharing 8
characters with them.  dev_wikipedia.txt is a seeded reservoir of 2,000 lines from the Wikipedia model's training
shards (which already exclude its test set).  Does nothing when splits.json exists.

    zt --remote train corpus splits
    python -m zhuyintype_ml.training.corpus.prepare --output data/build/corpus --wikipedia data/build/lm/work/sentences
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import unicodedata
import urllib.request
from collections.abc import Callable, Iterable
from pathlib import Path
from typing import Any

import ahocorasick
from opencc import OpenCC

from zhuyintype_ml.core import paths

SEED = 20261026


def fetch(url: str, path: Path) -> dict[str, Any]:
    data = urllib.request.urlopen(url).read()
    path.write_bytes(data)
    return {"url": url, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def exclusion(sentences: Iterable[str]) -> Callable[[str], bool]:
    """A test for text equal to a held-out sentence or sharing any 8 consecutive characters with one."""
    machine = ahocorasick.Automaton()
    exact = set(sentences)
    for s in exact:
        for i in range(len(s) - 7):
            machine.add_word(s[i : i + 8], True)
    machine.make_automaton()
    return lambda s: s in exact or next(machine.iter(s), None) is not None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, default=paths.data("corpus"))
    parser.add_argument(
        "--wikipedia",
        type=Path,
        default=paths.data("lm", "work", "sentences"),
        help="the Wikipedia model's training shards (zt train lm train)",
    )
    args = parser.parse_args(argv)
    out = args.output
    raw = out / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    if (out / "splits.json").exists():
        print((out / "splits.json").read_text())
        return 0
    commit = json.load(urllib.request.urlopen("https://api.github.com/repos/common-voice/common-voice/commits/main"))[
        "sha"
    ]
    receipt = fetch(
        f"https://raw.githubusercontent.com/common-voice/common-voice/{commit}/server/data/zh-TW/sentence-collector.txt",
        raw / "sentence-collector.txt",
    )
    receipt["commit"] = commit
    receipt["license"] = "CC0-1.0"
    receipt["license_evidence"] = (
        "https://github.com/common-voice/common-voice/blob/" + commit + "/docs/submitting-bulk-sentences.md"
    )
    fetch(
        receipt["license_evidence"].replace("github.com", "raw.githubusercontent.com").replace("/blob/", "/"),
        raw / "common-voice-submission-policy.md",
    )
    cc = OpenCC("s2t")
    lines = sorted(
        {
            s
            for line in (raw / "sentence-collector.txt").read_text().splitlines()
            if 4 <= len(s := unicodedata.normalize("NFC", line.strip())) <= 60
            and len(re.findall(r"[㐀-鿿]", s)) >= 0.6 * len(s)
            and cc.convert(s) == s
        }
    )
    assert len(lines) > 5000, len(lines)
    random.Random(SEED).shuffle(lines)
    test, dev, train = lines[:3000], lines[3000:5000], lines[5000:]
    blocked = exclusion(test + dev)
    train = [s for s in train if not blocked(s)]
    for name, values in [("test_colloquial", test), ("dev_colloquial", dev), ("train_commonvoice", train)]:
        (out / (name + ".txt")).write_text("\n".join(values) + "\n")
    # The old Wikipedia training shards have already excluded the fixed Wikipedia test.
    shards = sorted(args.wikipedia.glob("*.txt"))
    reservoir: list[str] = []
    rng = random.Random(SEED + 1)
    seen = 0
    for shard in shards:
        with shard.open() as f:
            for line in f:
                seen += 1
                if len(reservoir) < 2000:
                    reservoir.append(line.strip())
                else:
                    j = rng.randrange(seen)
                    if j < 2000:
                        reservoir[j] = line.strip()
    assert len(reservoir) == 2000
    (out / "dev_wikipedia.txt").write_text("\n".join(reservoir) + "\n")
    # Also protect Wikipedia dev/test against synthetic leakage.
    stats = {
        "seed": SEED,
        "source": receipt,
        "clean_unique": len(lines),
        "test_colloquial": len(test),
        "dev_colloquial": len(dev),
        "train_commonvoice": len(train),
        "dev_wikipedia": len(reservoir),
        "wikipedia_candidates": seen,
        "policy": "strict OpenCC s2t identity; training excludes held-out equality and every 8-char overlap",
    }
    (out / "splits.json").write_text(json.dumps(stats, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(stats, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

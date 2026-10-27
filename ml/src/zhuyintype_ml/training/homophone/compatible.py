"""Training data of the 0.4.0 continuation: leak-free shard indexes and real-source clauses, no Taiwan Tongues.

    zt train homophone prepare
    python -m zhuyintype_ml.training.homophone.compatible prepare --workers 4    # GPU host; refuses to run twice
    python -m zhuyintype_ml.training.homophone.compatible audit                  # the independent rescan again

prepare copies the homophone sets into homophone2/, records the hashes of every source and held-out file
(inputs.json), filters the v3_030 shards (filter.py) against every frozen evaluation file — the corpus dev/test sets,
the Taiwan Tongues and chat2 dialogue sets, the homophone sets, smoke and every text file under zhuyintype_ml/eval
(including the imported 善解 rows), raw, with 臺 as 台, and with unknown characters as U+FFFD — adds initial-在／再
indexes,
collects real-source clauses (Legislative Yuan gazettes and Wikipedia talk pages from lm_chat2, minus its reserved
documents, and Common Voice sentences) with their real preceding text, and rescans everything (audit).  Writes
homophone2/{inputs.json, filtered/, filtered.json, real-train.jsonl, real-train.json, leakage-audit.json}.
"""

from __future__ import annotations

import argparse
import json
import multiprocessing as mp
import re
import shutil
import sys
import time
from collections import Counter
from collections.abc import Iterator
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

from .common import BASE, DATA, RUN, SMOKE, Exclusions, digest, jsonl, save

OUT = RUN
# The files the homophone sets are copied from (round 1) into homophone2/.
SETS = [
    "inventory.json",
    "dev_zai.txt",
    "dev_zai.jsonl",
    "dev_pairs.txt",
    "dev_pairs.jsonl",
    "test_zai.txt",
    "test_zai.jsonl",
    "test_pairs.txt",
    "test_pairs.jsonl",
    "smoke.txt",
]


def evaluation_files() -> list[Path]:
    """Every frozen evaluation file whose text the continuation must not train on."""
    files = [
        paths.data(p)
        for p in (
            "lm/test_sentences.txt",
            "corpus/test_colloquial.txt",
            "corpus/dev_colloquial.txt",
            "corpus/dev_wikipedia.txt",
            "lm_chat/dev_chat_tt.txt",
            "lm_chat/test_chat_tt.txt",
            "lm_chat2/dev_dialogue.txt",
            "lm_chat2/test_dialogue.txt",
        )
    ]
    files += [p for prefix in ("dev_", "test_") for p in DATA.glob(prefix + "*.txt")]
    files += [DATA / "smoke.txt"] + list((paths.ROOT / "ml/src/zhuyintype_ml/eval").rglob("*.txt"))
    return sorted(set(files))


def blocked_fields(line: str) -> list[str]:
    """The texts of one evaluation line: the target, a context of 8+ characters, and their join.  Lines are
    `context|truth|syllables` rows or `context<TAB>text`."""
    if not line.strip() or line.startswith("#"):
        return []
    fields = line.split("|")[:2] if "|" in line else line.split("\t")
    # Short source contexts are not independent evaluation sentences.
    return [fields[-1]] + [f for f in fields[:-1] if len(f) >= 8] + ["".join(fields)]


def exclusions() -> Exclusions:
    vocab = set(json.loads((BASE / "chars.json").read_text())[4:]) | {"\n"}
    held = Exclusions(SMOKE)
    for path in evaluation_files():
        for line in path.read_text().splitlines():
            for text in blocked_fields(line):
                for variant in (text, text.replace("臺", "台")):
                    held.add(variant)
                    held.add("".join(c if c in vocab else "�" for c in variant))
    return held.compile()


def init_filter() -> None:
    from . import filter as filtering

    filtering.init(OUT, exclusions())


def filter_shard(row: dict[str, Any]) -> dict[str, Any]:
    import numpy as np

    from . import filter as filtering

    result = filtering.filter_shard(row)
    stem = BASE / "encoded" / row["name"]
    ids = np.memmap(str(stem) + ".ids", np.uint16, "r")
    examples = np.load(str(stem) + ".examples.npy", mmap_mode="r")
    vocab = json.loads((BASE / "chars.json").read_text())
    for char in "在再":
        group = "initial-" + char
        choices = np.load(OUT / "filtered" / f"{row['name']}.primary-{char}.npy")
        choices = choices[ids[examples[choices, 0]] == vocab.index(char)]
        np.save(OUT / "filtered" / f"{row['name']}.{group}.npy", choices)
        result["groups"][group] = len(choices)
    return result


def real_examples() -> None:
    from zhuyintype_ml.training.v3.dataset import Builder
    from zhuyintype_ml.training.v3.polyphone import load_poly

    builder = Builder()
    poly, labels = load_poly()
    held = exclusions()
    reserved = json.loads(paths.data("lm_chat2", "splits.json").read_text())["reserved_documents"]
    seen: set[str] = set()
    counts: Counter[str] = Counter()

    def rows() -> Iterator[dict[str, Any]]:
        for record in jsonl(paths.data("lm_chat2", "real_dialogues.jsonl")):
            assert record["source"] in ("gazette", "wiki_talk")
            if record["source"] + ":" + record["document"] in reserved:
                continue
            previous = ""
            for turn in record["turns"]:
                text = turn["text"]
                # Retain complete clauses and their actual preceding text, never synthetic prefixes.
                for match in re.finditer(r"[\u4e00-\u9fff]{4,64}", text):
                    if match.start() and re.match(r"[A-Za-z0-9\u4e00-\u9fff]", text[match.start() - 1]):
                        continue
                    if match.end() < len(text) and re.match(r"[A-Za-z0-9\u4e00-\u9fff]", text[match.end()]):
                        continue
                    yield dict(
                        text=match[0],
                        context=(previous + "\n" + text[: match.start()])[-255:],
                        source=record["source"],
                        document=record["document"],
                        url=record["url"],
                    )
                previous = text if text else ""
        for line in paths.data("corpus", "train_commonvoice.txt").open():
            text = line.strip()
            if 4 <= len(text) <= 64:
                yield dict(text=text, context="", source="commonvoice")

    with (OUT / "real-train.jsonl").open("x") as f:
        for row in rows():
            counts[row["source"] + "_candidates"] += 1
            text, ctx = row["text"], row["context"]
            # Check the whole context-target join, plus each side independently.
            if any(s in held or s.replace("臺", "台") in held for s in (text, ctx, ctx + text)):
                counts["excluded"] += 1
                continue
            if text in seen:
                continue
            readings, low = builder.lex.label(text, poly, builder.chars, labels)
            if low:
                continue
            row["readings"] = readings
            seen.add(text)
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
            counts[row["source"] + "_kept"] += 1
            for char in "在再":
                counts["contains_" + char] += char in text
                counts["initial_" + char] += text.startswith(char)
    save(OUT / "real-train.json", dict(counts))
    print(dict(counts), flush=True)


def audit() -> None:
    """Independent rescan of every selected document and supplemental context join."""
    import numpy as np

    held = exclusions()
    vocab = json.loads((BASE / "chars.json").read_text())
    count = documents = 0
    for row in json.loads((OUT / "filtered.json").read_text())["shards"]:
        stem = BASE / "encoded" / row["name"]
        ids = np.memmap(str(stem) + ".ids", np.uint16, "r")
        examples = np.load(str(stem) + ".examples.npy")
        allowed = np.load(OUT / "filtered" / f"{row['name']}.normal.npy")
        starts = set(examples[allowed, 2].tolist())
        for start, end in np.load(str(stem) + ".documents.npy"):
            if start not in starts:
                continue
            text = "".join(vocab[n] if n >= 4 else "\n" if n == 3 else "�" if n == 2 else "" for n in ids[start:end])
            assert text not in held and text.replace("臺", "台") not in held, row["name"]
            documents += 1
        count += len(allowed)
    real = 0
    for row in jsonl(OUT / "real-train.jsonl"):
        assert row["source"] in ("gazette", "wiki_talk", "commonvoice")
        for s in (row["text"], row["context"], row["context"] + row["text"]):
            assert s not in held and s.replace("臺", "台") not in held
        real += 1
    save(OUT / "leakage-audit.json", dict(examples=count, documents=documents, real=real, violations=0))


def prepare(workers: int) -> None:
    # The pipeline creates homophone2/ for its step records; refuse only when a preparation has started.
    assert not (OUT / "inputs.json").exists() and not (OUT / "filtered").exists(), "Already prepared: inspect it."
    OUT.mkdir(parents=True, exist_ok=True)
    for name in SETS:
        shutil.copy2(DATA / name, OUT / name)
    sources = [BASE / n for n in ("encode.json", "colloquial.json", "provenance.json", "syllable.pt", "context.pt")]
    sources += [paths.data("lm_chat2", n) for n in ("real_dialogues.jsonl", "splits.json", "source-manifest.json")]
    sources += [paths.data("corpus", "train_commonvoice.txt")]
    save(
        OUT / "inputs.json",
        dict(
            sources={paths.relative(p): digest(p) for p in sources},
            holdouts={paths.relative(p): digest(p) for p in evaluation_files()},
            protocol="Source allowlist: Wikipedia, deduplicated Qwen/Common Voice, gazette, wiki talk, Common Voice. "
            "No TT training source.",
        ),
    )
    (OUT / "filtered").mkdir()
    rows = json.loads((BASE / "encode.json").read_text())["shards"]
    assert all(r["name"].startswith(("wiki-", "colloquial-")) for r in rows)
    started = time.monotonic()
    with ProcessPoolExecutor(workers, mp_context=mp.get_context("spawn"), initializer=init_filter) as pool:
        result = list(pool.map(filter_shard, rows, chunksize=4))
    save(
        OUT / "filtered.json",
        dict(
            shards=result,
            allowed=sum(r["allowed"] for r in result),
            rejected=sum(r["rejected"] for r in result),
            seconds=time.monotonic() - started,
        ),
    )
    real_examples()
    audit()


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("phase", choices=["prepare", "audit"])
    p.add_argument("--workers", type=int, default=4)
    a = p.parse_args(argv)
    assert sys.platform == "linux", "Preparation and audits run on GPU host."
    if a.phase == "audit":
        audit()
    else:
        prepare(a.workers)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

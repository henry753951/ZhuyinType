"""v3 training text: ordered Wikipedia paragraphs, deduplicated colloquial lines, and the vocabularies.

    zt train v3 wiki colloquial vocab
    python -m nlp.training.v3.prepare wiki --workers 24        # GPU host; ~12 minutes
    python -m nlp.training.v3.prepare colloquial               # ~18 minutes
    python -m nlp.training.v3.prepare vocab                    # after both

wiki: reads the Wikipedia dump in document order and cleans it with the lm pipeline's markup removal and OpenCC
s2twp; keeps 4–200-character sentences that are at least 60% Han, per source paragraph, with their punctuation.  A
paragraph sharing an 8-character substring with held-out text is dropped and breaks the context (an empty line).
Stops after the batch that reaches `--paragraphs`.  Writes wiki.txt, articles.tsv.gz (page, revision, title of every
kept article: the attribution) and wiki.json.

colloquial: the generated and Common Voice lines of nlp/training/corpus, deduplicated after template normalisation
(numbers, name slots, punctuation), near duplicates removed with 64-permutation MinHash LSH confirmed by exact
5-gram Jaccard >= 0.8, and at most `--family-cap` lines per family (first 6 + last 4 normalised characters).  Writes
colloquial.txt and colloquial.json.

vocab: chars.json (ids 0–3 are <pad> <bos> <unk> <sep>, then corpus characters by frequency), syllables.json and
provenance.json (lexicon and held-out hashes).
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib
import multiprocessing as mp
import re
import time
import unicodedata
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from types import ModuleType
from typing import Any

from nlp.common import paths

from .common import HELD, OUT, Held, digest, excluded, holdouts, save

# The lm module with the Wikipedia reader and cleaner (page_batches, clean, BAD, HAN): v3 text is cleaned the same way.
WIKIPEDIA = "nlp.training.lm.train"
# The dump the lm pipeline downloads (raw/ beside the data directory).
DUMP = paths.data_dir().parent / "raw" / "lm" / "zhwiki-latest-pages-articles.xml.bz2"

LM: ModuleType
CONVERTER: Any
HELDOUT: Held


def wikipedia() -> ModuleType:
    return importlib.import_module(WIKIPEDIA)


def init() -> None:
    global LM, CONVERTER, HELDOUT
    from opencc import OpenCC

    LM = wikipedia()
    CONVERTER = OpenCC("s2twp")
    HELDOUT = holdouts()


class Capture:
    """Stands in for the converter the lm cleaner takes, and keeps the whole converted text."""

    text: str

    def convert(self, text: str) -> str:
        self.text = CONVERTER.convert(text)
        return self.text


def paragraphs(raw: str) -> list[str | None]:
    """Cleaned paragraphs of one article; None for a paragraph dropped as held-out text."""
    capture = Capture()
    LM.clean(raw, capture)
    result: list[str | None] = []
    for line in unicodedata.normalize("NFC", capture.text).splitlines():
        pieces = []
        for match in re.finditer(r"[^。！？；]+[。！？；]?", line):
            sentence = re.sub(r"\s+", " ", match.group()).strip(" \t*#;:：")
            body = sentence.rstrip("。！？；")
            if 4 <= len(body) <= 200 and not LM.BAD.search(body) and len(LM.HAN.findall(body)) / len(body) >= 0.6:
                pieces.append(sentence)
        text = "".join(pieces)
        if text:
            result.append(None if excluded(text, HELDOUT) else text)
    return result


def extract_batch(task: tuple[int, list[tuple[str, str, str, str]], str]) -> dict[str, int]:
    index, pages, directory_name = task
    directory = Path(directory_name)
    stats: Counter[str] = Counter()
    with (
        (directory / f"{index:06d}.txt").open("w") as out,
        gzip.open(directory / f"{index:06d}.articles.tsv.gz", "wt") as attr,
    ):
        for page, revision, title, raw in pages:
            stats["articles_seen"] += 1
            kept = False
            for paragraph in paragraphs(raw):
                if paragraph is None:
                    stats["excluded_paragraphs"] += 1
                    out.write("\n")
                else:
                    out.write(paragraph + "\n")
                    stats["paragraphs"] += 1
                    stats["characters"] += len(paragraph)
                    kept = True
            out.write("\n")
            if kept:
                stats["articles_kept"] += 1
                attr.write(f"{page}\t{revision}\t{title}\n")
    return dict(stats)


def wiki(args: argparse.Namespace) -> None:
    lm = wikipedia()
    directory = OUT / "wiki_shards"
    directory.mkdir(exist_ok=True)
    totals: Counter[str] = Counter()
    start = time.monotonic()
    batches = lm.page_batches(args.dump, directory)
    # Ordered bounded batches preserve dump order without buffering the full dump.
    with ProcessPoolExecutor(args.workers, mp_context=mp.get_context("fork"), initializer=init) as pool:
        while True:
            tasks = []
            for _ in range(args.workers * 2):
                task = next(batches, None)
                if task is None:
                    break
                tasks.append(task)
            if not tasks:
                break
            for result in pool.map(extract_batch, tasks):
                totals.update(result)
            print({**totals, "seconds": round(time.monotonic() - start)}, flush=True)
            if totals["paragraphs"] >= args.paragraphs:
                break
    with (OUT / "wiki.txt").open("wb") as out, (OUT / "articles.tsv.gz").open("wb") as attr:
        for path in sorted(directory.glob("*.txt")):
            with path.open("rb") as source:
                while block := source.read(8 << 20):
                    out.write(block)
            with path.with_suffix(".articles.tsv.gz").open("rb") as source:
                attr.write(source.read())
    save(OUT / "wiki.json", {**totals, "seconds": time.monotonic() - start})


# Names are normalized only in explicit name slots; arbitrary Chinese words stay intact.
NAME = re.compile(
    r"(?:(?<=我叫)|(?<=我是)|(?<=名字是)|(?<=找)|(?<=聯絡))[陳林黃張李王吳劉蔡楊許鄭謝洪郭邱曾廖賴徐周葉蘇莊呂江何蕭羅高潘簡朱鍾游彭]"
    r"[\u4e00-\u9fff]{1,2}(?=[，。！？、：\s]|$)|[小阿][明華美玲芳強豪偉傑婷君志翔]|\b[A-Z][a-z]{1,15}\b"
)


def template(text: str) -> str:
    text = re.sub(r"[0-9０-９一二三四五六七八九十百千萬億]+", "#", text)
    text = NAME.sub("@", text)
    return re.sub(r"[\W_]+", "", text)


def fivegrams(text: str) -> set[str]:
    return {text[i : i + 5] for i in range(max(1, len(text) - 4))}


def colloquial(args: argparse.Namespace) -> None:
    from datasketch import MinHash, MinHashLSH

    held = holdouts()
    lsh = MinHashLSH(threshold=0.8, num_perm=64)
    # Compact exact-confirmation signatures are retained only for accepted lines.
    signatures: dict[int, set[int]] = {}
    exact: set[bytes] = set()
    families: Counter[str] = Counter()
    stats: Counter[str] = Counter()
    sources = sorted(paths.data("corpus").glob("synthetic_*.txt"))
    sources.append(paths.data("corpus", "train_commonvoice.txt"))
    start = time.monotonic()
    with (OUT / "colloquial.txt").open("w") as out:
        for path in sources:
            for line in path.open():
                text = line.strip()
                stats["input_lines"] += 1
                if not text or excluded(text, held):
                    stats["excluded"] += 1
                    continue
                normalized = template(text)
                key = hashlib.blake2b(normalized.encode(), digest_size=16).digest()
                if key in exact:
                    stats["normalized_duplicates"] += 1
                    continue
                grams = fivegrams(normalized)
                # Hash grams to keep the multi-million-line index bounded in memory.
                hashed = {int.from_bytes(hashlib.blake2b(g.encode(), digest_size=8).digest(), "little") for g in grams}
                m = MinHash(num_perm=64)
                m.update_batch([g.encode() for g in sorted(grams)])
                neighbours = lsh.query(m)
                if any(len(hashed & signatures[n]) / len(hashed | signatures[n]) >= 0.8 for n in neighbours):
                    stats["near_duplicates"] += 1
                    continue
                family = normalized[:6] + ":" + normalized[-4:]
                if families[family] >= args.family_cap:
                    stats["family_cap_removed"] += 1
                    continue
                index = stats["kept"]
                lsh.insert(index, m)
                signatures[index] = hashed
                exact.add(key)
                families[family] += 1
                out.write(text + "\n")
                stats["kept"] += 1
                if stats["input_lines"] % 100000 == 0:
                    print(dict(stats), flush=True)
            print(path.name, dict(stats), flush=True)
    save(
        OUT / "colloquial.json",
        {
            **stats,
            "seconds": time.monotonic() - start,
            "family_cap": args.family_cap,
            "max_family": max(families.values(), default=0),
            "families": len(families),
            "sources": [paths.relative(p) for p in sources],
            "limit": "64-permutation MinHash retrieval is probabilistic; exact 5-gram Jaccard confirms retrieved "
            "matches; family is normalized first 6 + last 4 characters",
        },
    )


def vocab() -> None:
    from .readings import Lexicon

    lex = Lexicon(OUT / "lexicon.tsv")
    counts: Counter[str] = Counter()
    for name in ["wiki.txt", "colloquial.txt"]:
        for line in (OUT / name).open():
            counts.update(line.rstrip("\n"))
    chars = ["<pad>", "<bos>", "<unk>", "<sep>"] + sorted(counts, key=lambda c: (-counts[c], c))
    assert len(chars) < 65536
    save(OUT / "chars.json", chars)
    save(OUT / "syllables.json", ["<pad>", "<literal>"] + sorted({s for s, t in lex.candidates}))
    save(
        OUT / "provenance.json",
        {
            "lexicon_sha256": digest(OUT / "lexicon.tsv"),
            "holdout_sha256": {paths.relative(paths.data(p)): digest(paths.data(p)) for p in HELD},
            "vocab": len(chars),
            "single_character_lexicon_in_corpus": len(set(lex.single) & set(counts)),
            "license": "CC BY-SA 4.0; preserve existing data sources and articles.tsv.gz attribution",
        },
    )


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("phase", choices=["wiki", "colloquial", "vocab"])
    p.add_argument("--workers", type=int, default=24)
    p.add_argument("--paragraphs", type=int, default=10000000)
    p.add_argument("--family-cap", type=int, default=100)
    p.add_argument("--dump", type=Path, default=DUMP, help=f"Wikipedia dump (default: {paths.relative(DUMP)})")
    args = p.parse_args(argv)
    OUT.mkdir(exist_ok=True, parents=True)
    if args.phase == "wiki":
        wiki(args)
    elif args.phase == "colloquial":
        colloquial(args)
    else:
        vocab()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

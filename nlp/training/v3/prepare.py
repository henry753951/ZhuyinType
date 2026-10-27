"""Re-extract ordered Wikipedia paragraphs and deduplicate colloquial templates."""
import argparse
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor
import gzip
import importlib.util
import multiprocessing as mp
from pathlib import Path
import re
import sys
import time
import unicodedata

from common import ROOT, OUT, HELD, digest, excluded, holdouts, save


def pipeline():
    # Reuse the installed parser without modifying the existing Python environments.
    for site in (ROOT / ".venv/lib").glob("python*/site-packages"):
        sys.path.append(str(site))
    spec = importlib.util.spec_from_file_location("lm_pipeline", ROOT / "nlp/training/lm/pipeline.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def init():
    global LM, CONVERTER, HELDOUT
    from opencc import OpenCC
    LM = pipeline()
    CONVERTER = OpenCC("s2twp")
    HELDOUT = holdouts()


class Capture:
    def convert(self, text):
        self.text = CONVERTER.convert(text)
        return self.text


def paragraphs(raw):
    capture = Capture()
    LM.clean(raw, capture)
    result = []
    for line in unicodedata.normalize("NFC", capture.text).splitlines():
        pieces = []
        for match in re.finditer(r"[^。！？；]+[。！？；]?", line):
            sentence = re.sub(r"\s+", " ", match.group()).strip(" \t*#;:：")
            body = sentence.rstrip("。！？；")
            if (4 <= len(body) <= 200 and not LM.BAD.search(body)
                    and len(LM.HAN.findall(body)) / len(body) >= .6):
                pieces.append(sentence)
        text = "".join(pieces)
        if text:
            result.append(None if excluded(text, HELDOUT) else text)
    return result


def extract_batch(task):
    index, pages, directory = task
    directory = Path(directory)
    stats = Counter()
    with (directory / f"{index:06d}.txt").open("w") as out, gzip.open(
            directory / f"{index:06d}.articles.tsv.gz", "wt") as attr:
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


def wiki(args):
    lm = pipeline()
    directory = OUT / "wiki_shards"
    directory.mkdir(exist_ok=True)
    totals = Counter()
    start = time.monotonic()
    batches = lm.page_batches(ROOT / "raw/zhwiki-latest-pages-articles.xml.bz2", directory)
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
    totals["seconds"] = time.monotonic() - start
    save(OUT / "wiki.json", dict(totals))


# Names are normalized only in explicit name slots; arbitrary Chinese words stay intact.
NAME = re.compile(r"(?:(?<=我叫)|(?<=我是)|(?<=名字是)|(?<=找)|(?<=聯絡))[陳林黃張李王吳劉蔡楊許鄭謝洪郭邱曾廖賴徐周葉蘇莊呂江何蕭羅高潘簡朱鍾游彭][\u4e00-\u9fff]{1,2}(?=[，。！？、：\s]|$)|[小阿][明華美玲芳強豪偉傑婷君志翔]|\b[A-Z][a-z]{1,15}\b")


def template(text):
    text = re.sub(r"[0-9０-９一二三四五六七八九十百千萬億]+", "#", text)
    text = NAME.sub("@", text)
    return re.sub(r"[\W_]+", "", text)


def fivegrams(text):
    return {text[i:i + 5] for i in range(max(1, len(text) - 4))}


def colloquial(args):
    from datasketch import MinHash, MinHashLSH
    held = holdouts()
    lsh = MinHashLSH(threshold=.8, num_perm=64)
    # Compact exact-confirmation signatures are retained only for accepted lines.
    signatures = {}
    exact = set()
    families = Counter()
    stats = Counter()
    paths = sorted((ROOT / "data/build/corpus").glob("synthetic_*.txt"))
    paths.append(ROOT / "data/build/corpus/train_commonvoice.txt")
    start = time.monotonic()
    with (OUT / "colloquial.txt").open("w") as out:
        for path in paths:
            for line in path.open():
                text = line.strip()
                stats["input_lines"] += 1
                if not text or excluded(text, held):
                    stats["excluded"] += 1
                    continue
                normalized = template(text)
                key = __import__("hashlib").blake2b(normalized.encode(), digest_size=16).digest()
                if key in exact:
                    stats["normalized_duplicates"] += 1
                    continue
                grams = fivegrams(normalized)
                # Hash grams to keep the multi-million-line index bounded in memory.
                hashed = {int.from_bytes(__import__("hashlib").blake2b(g.encode(), digest_size=8).digest(), "little") for g in grams}
                m = MinHash(num_perm=64)
                m.update_batch([g.encode() for g in sorted(grams)])
                neighbours = lsh.query(m)
                if any(len(hashed & signatures[n]) / len(hashed | signatures[n]) >= .8 for n in neighbours):
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
    save(OUT / "colloquial.json", {**stats, "seconds": time.monotonic() - start,
         "family_cap": args.family_cap, "max_family": max(families.values(), default=0),
         "families": len(families), "sources": [str(p.relative_to(ROOT)) for p in paths],
         "limit": "64-permutation MinHash retrieval is probabilistic; exact 5-gram Jaccard confirms retrieved matches; family is normalized first 6 + last 4 characters"})


def vocab():
    from readings import Lexicon
    lex = Lexicon(OUT / "lexicon.tsv")
    counts = Counter()
    for name in ["wiki.txt", "colloquial.txt"]:
        for line in (OUT / name).open():
            counts.update(line.rstrip("\n"))
    chars = ["<pad>", "<bos>", "<unk>", "<sep>"] + sorted(counts, key=lambda c: (-counts[c], c))
    assert len(chars) < 65536
    save(OUT / "chars.json", chars)
    save(OUT / "syllables.json", ["<pad>", "<literal>"] + sorted({s for s, t in lex.candidates}))
    save(OUT / "provenance.json", {"lexicon_sha256": digest(OUT / "lexicon.tsv"),
         "holdout_sha256": {p: digest(ROOT / p) for p in HELD}, "vocab": len(chars),
         "single_character_lexicon_in_corpus": len(set(lex.single) & set(counts)),
         "license": "CC BY-SA 4.0; preserve existing data sources and articles.tsv.gz attribution"})


def main():
    p = argparse.ArgumentParser()
    p.add_argument("phase", choices=["wiki", "colloquial", "vocab"])
    p.add_argument("--workers", type=int, default=24)
    p.add_argument("--paragraphs", type=int, default=10000000)
    p.add_argument("--family-cap", type=int, default=100)
    args = p.parse_args()
    OUT.mkdir(exist_ok=True, parents=True)
    {"wiki": wiki, "colloquial": colloquial, "vocab": lambda _: vocab()}[args.phase](args)


if __name__ == "__main__":
    main()

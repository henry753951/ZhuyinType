"""Wikipedia → Taiwan Traditional sentences → leakage-free hard-EM segmentation → pruned modified Kneser-Ney bigram.

Writes unigram.tsv, bigram.tsv, backoff.tsv, lexicon_tw.tsv, test_sentences.txt, stats.json, NOTICE.md and the
article attribution list into --output; resumable checkpoints go to --work.  GPU host (CPU only, 64 workers, ~50 GB
disk, >=64 GB RAM).  AGPL-3.0-only code; the generated models are CC BY-SA 4.0 (see README.md).

    zt --remote train lm train
    python -m zhuyintype_ml.training.lm.train --dump data/raw/lm/zhwiki-latest-pages-articles.xml.bz2 \\
        --lexicon data/build/lexicon.tsv --work data/build/lm/work --output data/build/lm --workers 64
"""

from __future__ import annotations

import argparse
import bz2
import collections
import concurrent.futures
import datetime
import gzip
import html
import importlib.metadata
import json
import math
import multiprocessing as mp
import os
import platform
import random
import re
import subprocess
import time
import unicodedata
import xml.etree.ElementTree as ET
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import mwparserfromhell
import numpy as np
from opencc import OpenCC

from zhuyintype_ml.core import paths

from .download import DUMP_URL
from .files import digest, log, raw, save_json
from .mkn import (
    BAD,
    HAN,
    SPLIT,
    Fitted,
    automaton,
    em,
    fit_mkn,
    make_trie,
    merge_pairs,
    read_lexicon,
    segment,
    write_tsv,
)

NS = "{http://www.mediawiki.org/xml/export-0.11/}"
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"
# (page id, revision id, title, wikitext)
Page = tuple[str | None, str | None, str | None, str]
DROPPED_TAGS = {
    "ref",
    "references",
    "math",
    "chem",
    "code",
    "pre",
    "source",
    "syntaxhighlight",
    "gallery",
    "table",
    "timeline",
    "score",
    "imagemap",
    "nowiki",
}


def variant(match: re.Match[str]) -> str:
    """The Taiwan choice of a -{zh-tw:...;zh-cn:...}- variant block."""
    body = match.group(1)
    if ":" not in body:
        return body
    choices = {}
    for item in body.split(";"):
        if ":" in item:
            key, value = item.split(":", 1)
            choices[key.strip().lower()] = value
    return next((choices[k] for k in ("zh-tw", "zh-hant", "zh-hk", "zh", "zh-cn") if k in choices), "")


def clean(wikitext: str, converter: OpenCC) -> list[str]:
    """Plain-text sentences of an article: 4–200 characters, at least 60% Han, no markup or URLs."""
    # Templates are deliberately omitted: expanding them needs a MediaWiki runtime.
    text = re.sub(r"-\{(.*?)\}-", variant, wikitext, flags=re.S)
    text = re.split(r"(?m)^==+\s*(?:參考|参考|外部連結|外部链接|注釋|注释|註釋|參見|参见)", text)[0]
    text = re.sub(r"\{\|.*?\|\}", "\n", text, flags=re.S)
    text = "\n".join(line for line in text.splitlines() if not re.match(r"^\s*[|!=]", line))
    code = mwparserfromhell.parse(text)
    for tag in list(code.filter_tags()):
        if str(tag.tag).lower() in DROPPED_TAGS:
            try:
                code.remove(tag)
            except ValueError:
                pass  # A containing tag was already removed.
    for link in list(code.filter_wikilinks()):
        if ":" in str(link.title):
            try:
                code.remove(link)
            except ValueError:
                pass
    for external in list(code.filter_external_links()):
        try:
            code.remove(external)
        except ValueError:
            pass
    text = converter.convert(html.unescape(code.strip_code(normalize=True, collapse=True)))
    result = []
    for sentence in SPLIT.split(unicodedata.normalize("NFC", text)):
        sentence = re.sub(r"\s+", " ", sentence).strip(" \t*#;:：")
        if 4 <= len(sentence) <= 200 and not BAD.search(sentence) and len(HAN.findall(sentence)) / len(sentence) >= 0.6:
            result.append(sentence)
    return result


CONVERTER: OpenCC | None = None


def extract_init() -> None:
    global CONVERTER
    CONVERTER = OpenCC("s2twp")


def extract_batch(task: tuple[int, list[Page], str]) -> dict[str, int]:
    index, pages, directory = task
    assert CONVERTER is not None
    counts: collections.Counter[str] = collections.Counter()
    with (
        open(Path(directory) / f"{index:06d}.txt", "w", encoding="utf-8") as out,
        gzip.open(Path(directory) / f"{index:06d}.articles.tsv.gz", "wt", encoding="utf-8") as attr,
    ):
        for page_id, rev_id, title, text in pages:
            sentences = clean(text, CONVERTER)
            counts["documents_processed"] += 1
            if sentences:
                counts["documents_with_clean_sentences"] += 1
                attr.write(f"{page_id}\t{rev_id}\t{title}\n")
            for sentence in sentences:
                out.write(sentence + "\n")
                counts["sentences_before_dedup"] += 1
                counts["chars_before_dedup"] += len(sentence)
    return dict(counts)


def page_batches(dump: Path, directory: Path) -> Iterator[tuple[int, list[Page], str]]:
    """Namespace-0, non-redirect wikitext pages in batches of 512, streamed from the bzip2 XML."""
    batch: list[Page] = []
    index = 0
    with bz2.open(dump, "rb") as stream:
        iterator = ET.iterparse(stream, events=("start", "end"))
        _, root = next(iterator)
        for event, element in iterator:
            if event != "end" or element.tag != NS + "page":
                continue
            if element.findtext(NS + "ns") == "0" and element.find(NS + "redirect") is None:
                revision = element.find(NS + "revision")
                assert revision is not None
                text = revision.findtext(NS + "text", "")
                if text and revision.findtext(NS + "model", "wikitext") == "wikitext":
                    batch.append(
                        (
                            element.findtext(NS + "id"),
                            revision.findtext(NS + "id"),
                            element.findtext(NS + "title"),
                            text,
                        )
                    )
            root.clear()
            if len(batch) == 512:
                yield index, batch, str(directory)
                index += 1
                batch = []
        if batch:
            yield index, batch, str(directory)


def extract(args: argparse.Namespace) -> dict[str, Any]:
    """Cleans every article over `args.workers` processes, then deduplicates all sentences with GNU sort."""
    work, out = args.work, args.output
    marker = work / "extract.json"
    if marker.exists():
        log("reuse completed extraction")
        return dict(json.loads(marker.read_text()))
    start = time.monotonic()
    directory = work / "extracted"
    directory.mkdir(exist_ok=True)
    totals: collections.Counter[str] = collections.Counter()
    batches = iter(page_batches(args.dump, directory))
    # Bounded submission prevents the XML reader from queuing the full dump in RAM.
    with concurrent.futures.ProcessPoolExecutor(
        args.workers, mp_context=mp.get_context("fork"), initializer=extract_init
    ) as pool:
        pending: set[concurrent.futures.Future[dict[str, int]]] = set()
        exhausted = False
        completed = 0
        while pending or not exhausted:
            while len(pending) < args.workers * 2 and not exhausted:
                task = next(batches, None)
                if task is None:
                    exhausted = True
                else:
                    pending.add(pool.submit(extract_batch, task))
            if pending:
                done, pending = concurrent.futures.wait(pending, return_when=concurrent.futures.FIRST_COMPLETED)
                for future in done:
                    totals.update(future.result())
                    completed += 1
                if completed % 32 == 0:
                    log(
                        f"extract batches={completed} docs={totals['documents_processed']:,} "
                        f"sentences={totals['sentences_before_dedup']:,}"
                    )
    with open(out / "articles.tsv.gz", "wb") as destination:
        for path in sorted(directory.glob("*.articles.tsv.gz")):
            with path.open("rb") as source:
                while block := source.read(1024 * 1024):
                    destination.write(block)
    subprocess.run(
        [
            "sort",
            "--parallel=" + str(args.workers),
            "-S",
            "24G",
            "-T",
            str(work),
            "-u",
            "-o",
            str(work / "unique.txt"),
            *map(str, sorted(directory.glob("*.txt"))),
        ],
        check=True,
        env={**os.environ, "LC_ALL": "C"},
    )
    result: dict[str, Any] = dict(totals)
    result["extraction_dedup_seconds"] = time.monotonic() - start
    save_json(marker, result)
    return result


def split_corpus(args: argparse.Namespace) -> dict[str, Any]:
    """Holds out `args.holdout` uniformly sampled sentences (test_sentences.txt), then shards the training sentences
    that neither equal a test sentence nor share any 8 consecutive characters with one."""
    marker = args.work / "split.json"
    if marker.exists():
        return dict(json.loads(marker.read_text()))
    start = time.monotonic()
    rng = random.Random(args.seed)
    sample: list[str] = []
    unique_count = 0
    with open(args.work / "unique.txt", encoding="utf-8") as source:
        for unique_count, line in enumerate(source, 1):
            sentence = line.rstrip("\n")
            if len(sample) < args.holdout:
                sample.append(sentence)
            else:
                position = rng.randrange(unique_count)
                if position < args.holdout:
                    sample[position] = sentence
    if len(sample) != args.holdout:
        raise ValueError("not enough sentences for the requested holdout")
    sample.sort()
    (args.output / "test_sentences.txt").write_text("\n".join(sample) + "\n", encoding="utf-8")
    heldout = set(sample)
    machine = automaton(sample)
    directory = args.work / "sentences"
    directory.mkdir(exist_ok=True)
    handles = [open(directory / f"{i:03d}.txt", "w", encoding="utf-8") for i in range(args.workers)]
    chars: set[str] = set()
    counts: collections.Counter[str] = collections.Counter()
    try:
        with open(args.work / "unique.txt", encoding="utf-8") as source:
            for line in source:
                sentence = line.rstrip("\n")
                if sentence in heldout:
                    continue
                if next(machine.iter(sentence), None) is not None:
                    counts["removed_for_test_8char_overlap"] += 1
                    continue
                handles[counts["training_sentences"] % len(handles)].write(line)
                counts["training_sentences"] += 1
                counts["training_chars"] += len(sentence)
                chars.update(sentence)
    finally:
        for handle in handles:
            handle.close()
    counts.update(unique_sentences=unique_count, test_sentences=len(sample), test_chars=sum(map(len, sample)))
    result = {
        **counts,
        "training_characters": sorted(chars),
        "split_seconds": time.monotonic() - start,
        "seed": args.seed,
    }
    save_json(marker, result)
    return result


def export(
    args: argparse.Namespace,
    words: list[str],
    uni_counts: np.ndarray,
    old: dict[str, float],
    rows: list[tuple[str, str, float]],
    header: str,
    model: Fitted,
) -> dict[str, Any]:
    """The three model TSVs and lexicon_tw.tsv: every seed row in order, recalibrated to the corpus unigram with the
    reading ratios of each word kept; unseen words shift by the median log ratio of the seen ones."""
    unigram = model[0]
    write_tsv(args.output, words, model)
    ids = {w: i for i, w in enumerate(words)}
    common = [w for w in old if uni_counts[ids[w]] > 0]
    shift = float(np.median([math.log(unigram[ids[w]]) - math.log(old[w]) for w in common]))
    with open(args.output / "lexicon_tw.tsv", "w", encoding="utf-8") as f:
        f.write(
            header + " Taiwan corpus word frequencies: Chinese Wikipedia / OpenCC s2twp; CC BY-SA 4.0. "
            "See ml/src/zhuyintype_ml/training/lm/README.md and stats.json.\n"
        )
        for reading, word, p in rows:
            count = int(uni_counts[ids[word]])
            value = math.log(unigram[ids[word]]) + p - math.log(old[word]) if count else p + shift
            f.write(f"{reading}\t{word}\t{value:.12f}\n")
    return {
        "lexicon_rows": len(rows),
        "lexicon_words": len(old),
        "lexicon_words_seen": len(common),
        "lexicon_type_coverage": len(common) / len(old),
        "unseen_lexicon_logp_shift": shift,
        "lexicon_frequency_estimator": "exported KN corpus unigram; median log-ratio scale shift for unseen words",
    }


def evaluate(
    args: argparse.Namespace, words: list[str], uni_counts: np.ndarray, old: dict[str, float], model: Fitted
) -> dict[str, Any]:
    """Perplexity and OOV rates on test_sentences.txt with the frozen final tokenizer (EOS counted, BOS not)."""
    unigram, keys, probabilities, alpha, _ = model
    scores = np.load(args.work / "segmentation_scores.npy")
    ids = {w: i for i, w in enumerate(words)}
    trie = make_trie(words, old)
    tokens_total = lexical_oov = model_oov = train_oov = 0
    uni_nll = bi_nll = 0.0
    predictions = 0
    hits = 0
    for sentence in (args.output / "test_sentences.txt").read_text().splitlines():
        tokens = segment(sentence, trie, ids, scores, float(scores[ids["<unk>"]]))
        tokens_total += len(tokens)
        lexical_oov += sum(words[t] not in old for t in tokens)
        model_oov += tokens.count(ids["<unk>"])
        train_oov += int(sum(uni_counts[t] == 0 for t in tokens))
        sequence = [ids["<s>"], *tokens, ids["</s>"]]
        for a, b in zip(sequence, sequence[1:]):
            key = np.uint64((a << 32) | b)
            position = int(np.searchsorted(keys, key))
            if position < len(keys) and keys[position] == key:
                p = probabilities[position]
                hits += 1
            else:
                p = alpha[a] * unigram[b]
            uni_nll -= math.log(unigram[b])
            bi_nll -= math.log(p)
            predictions += 1
    return {
        "test_tokens_excluding_boundaries": tokens_total,
        "test_predictions_including_EOS": predictions,
        "test_lexicon_oov_tokens": lexical_oov,
        "test_lexicon_oov_rate": lexical_oov / tokens_total,
        "test_model_unk_tokens": model_oov,
        "test_model_unk_rate": model_oov / tokens_total,
        "test_unseen_training_tokens": train_oov,
        "test_unseen_training_token_rate": train_oov / tokens_total,
        "test_unigram_perplexity": math.exp(uni_nll / predictions),
        "test_bigram_perplexity": math.exp(bi_nll / predictions),
        "test_bigram_hit_rate": hits / predictions,
        "perplexity_protocol": "same frozen final-E-step word tokens; include EOS, exclude BOS; no test tuning",
    }


def notice(receipt: dict[str, Any]) -> str:
    return (
        "# ZhuyinType statistical language model\n\n"
        "Models, recalibrated lexicon, test sentences, and attribution metadata are distributed under "
        f"Creative Commons Attribution-ShareAlike 4.0 International: {LICENSE_URL}\n\n"
        f"Source: Chinese Wikipedia contributors, {receipt['dump_date']} pages-articles snapshot.\n"
        f"Dump: {receipt['dated_url']}\n\n"
        "articles.tsv.gz lists page ID, revision ID, and title for every source article with retained clean text. "
        "For each row, the article is https://zh.wikipedia.org/?curid=PAGE_ID ; the exact revision is "
        "https://zh.wikipedia.org/w/index.php?oldid=REVISION_ID ; contributor history is "
        "https://zh.wikipedia.org/w/index.php?curid=PAGE_ID&action=history . "
        "These links provide contributor attribution under Wikimedia's reuse terms: "
        "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content\n\n"
        "Changes: article text extraction with templates/references/tables/code omitted; OpenCC s2twp "
        "Taiwan Traditional conversion; sentence filtering, exact deduplication, held-out exclusion, "
        "Viterbi hard EM, modified Kneser-Ney estimation, pruning, and probability export.\n\n"
        "Seed lexicon: ZhuyinType data/build/lexicon.tsv, CC BY-SA 4.0; derived from "
        "CC-CEDICT https://www.mdbg.net/chinese/dictionary?page=cc-cedict (CC BY-SA 4.0), "
        "Unicode Unihan https://www.unicode.org/charts/unihan.html (Unicode License v3), and "
        "wordfreq https://github.com/rspeer/wordfreq (data CC BY-SA 4.0). "
        "Original seed notice remains in lexicon_tw.tsv. Scripts are AGPL-3.0-only. "
        "Retain this notice and the article attribution manifest when redistributing outputs.\n"
    )


DECISIONS = [
    "Wikipedia only; no optional web corpus",
    "User-requested 4-200-character sentences override MODEL.md 4-64",
    "Exact deduplication as requested, no MinHash",
    "5000 uniform random unique sentences with fixed seed before all corpus counting",
    "Initial lexicon Viterbi plus two unigram re-estimation/resegmentation updates",
    "All fallback characters remain distinct tokens; novel test characters map to <unk>",
    "Exported KN unigram also supplies seen-word lexicon_tw probabilities; empirical counts supply hard EM",
    "Uniform count threshold selected from training counts only; backoff renormalized after pruning",
    "TSV deliverables only; Swift integration and quantized mmap export outside requested scope",
]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--dump",
        type=Path,
        default=raw("lm") / DUMP_URL.rsplit("/", 1)[1],
        help="pages-articles dump; download.json beside it is the source receipt",
    )
    parser.add_argument("--lexicon", type=Path, default=paths.data("lexicon.tsv"), help="seed lexicon")
    parser.add_argument("--work", type=Path, default=paths.data("lm", "work"), help="resumable checkpoints")
    parser.add_argument("--output", type=Path, default=paths.data("lm"))
    parser.add_argument("--workers", type=int, default=64)
    parser.add_argument("--holdout", type=int, default=5000)
    parser.add_argument("--seed", type=int, default=20261026)
    parser.add_argument("--max-bigrams", type=int, default=3000000)
    args = parser.parse_args(argv)
    for path in (args.work, args.output):
        path.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    config = {key: str(value.resolve()) if isinstance(value, Path) else value for key, value in vars(args).items()}
    config["lexicon_sha256"] = digest(args.lexicon)
    config["dump_sha256"] = digest(args.dump)
    config["dump_bytes"] = args.dump.stat().st_size
    marker = args.work / "config.json"
    if marker.exists() and json.loads(marker.read_text()) != config:
        raise ValueError("work directory config/input hashes changed; choose a new work directory")
    save_json(marker, config)
    extraction = extract(args)
    split = split_corpus(args)
    header, rows, old = read_lexicon(args.lexicon)
    words, uni_counts, em_times = em(args, old, split)
    keys, counts = merge_pairs(args)
    start = time.monotonic()
    model = fit_mkn(keys, counts, len(words), args.max_bigrams)
    vocabulary = export(args, words, uni_counts, old, rows, header, model)
    evaluation = evaluate(args, words, uni_counts, old, model)
    receipt = json.loads((args.dump.parent / "download.json").read_text())
    spotchecks = ["软件、鼠标、数据库、计算机", "台湾的出租车和网络服务器", "里面的信息通过互联网传输"]
    converter = OpenCC("s2twp")
    save_json(
        args.output / "conversion_spotchecks.json",
        [{"input": s, "s2twp": converter.convert(s)} for s in spotchecks],
    )
    (args.output / "NOTICE.md").write_text(notice(receipt), encoding="utf-8")
    examples = {}
    ids = {w: i for i, w in enumerate(words)}
    for context in ("我", "今天", "台灣", "的", "臺灣"):
        a = ids[context]
        positions = np.flatnonzero((model[1] >> np.uint64(32)) == a)
        distribution = model[3][a] * model[0].copy()
        explicit = set()
        for i in positions:
            b = int(model[1][i]) & 0xFFFFFFFF
            distribution[b] = model[2][i]
            explicit.add(b)
        top = np.argsort(-distribution)[:5]
        examples[context] = [
            {"next": words[b], "probability": float(distribution[b]), "stored_bigram": int(b) in explicit} for b in top
        ]
    save_json(args.output / "examples.json", examples)
    corpus = {
        **extraction,
        **{k: v for k, v in split.items() if k != "training_characters"},
        "training_tokens_excluding_boundaries": int(uni_counts.sum()),
        "training_bigram_events_including_boundaries": int(counts.sum()),
        "training_lexicon_oov_tokens": sum(int(uni_counts[i]) for i, w in enumerate(words) if w not in old),
        "training_token_types": int(np.count_nonzero(uni_counts)),
    }
    stats: dict[str, Any] = {
        "status": "trained_pending_independent_validation",
        "created_utc": datetime.datetime.now(datetime.UTC).isoformat(),
        "license": "CC BY-SA 4.0",
        "license_url": LICENSE_URL,
        "corpus": corpus,
        "vocabulary": vocabulary,
        "model": {
            **model[4],
            "unigram_types": len(words) - 1,
            "extra_tokens": "observed fallback characters, </s>, <unk>; <s> context-only",
        },
        "evaluation": evaluation,
        "examples": examples,
        "timings": {
            "segmentation_passes": em_times,
            "merge": json.loads((args.work / "merged.json").read_text()),
            "fit_export_evaluate_seconds": time.monotonic() - start,
            "pipeline_invocation_seconds": time.monotonic() - started,
        },
        "sources": [
            {
                "name": "Chinese Wikipedia pages-articles",
                "requested_url": DUMP_URL,
                "license": "CC BY-SA 4.0",
                "license_url": LICENSE_URL,
                "terms_url": "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use",
                "attribution": "Chinese Wikipedia contributors; articles.tsv.gz page/revision IDs resolve to article "
                "and author-history links",
                "transformations": "namespace 0 nonredirect text; omit templates/references/tables/code; OpenCC "
                "s2twp; clean/exact-dedupe/holdout/8-character exclusion; 2 hard-EM updates; MKN and count pruning",
                "receipt": receipt,
            },
            {
                "name": "ZhuyinType seed lexicon",
                "path": "data/build/lexicon.tsv",
                "sha256": config["lexicon_sha256"],
                "license": "CC BY-SA 4.0",
                "components": [
                    "CC-CEDICT (CC BY-SA 4.0)",
                    "Unicode Unihan (Unicode License v3)",
                    "wordfreq data (CC BY-SA 4.0)",
                ],
                "urls": [
                    "https://www.mdbg.net/chinese/dictionary?page=cc-cedict",
                    "https://www.unicode.org/charts/unihan.html",
                    "https://github.com/rspeer/wordfreq",
                ],
            },
        ],
        "runtime": {
            "python": platform.python_version(),
            "platform": platform.platform(),
            "workers": args.workers,
            "packages": {
                name: importlib.metadata.version(name)
                for name in ("numpy", "opencc", "mwparserfromhell", "pyahocorasick")
            },
        },
        "training_code_sha256": {
            p.name: digest(p) for p in sorted(Path(__file__).parent.iterdir()) if p.suffix in {".py", ".sh", ".txt"}
        },
        "decisions": DECISIONS,
    }
    stats["files"] = {
        p.name: {"bytes": p.stat().st_size, "sha256": digest(p)}
        for p in sorted(args.output.iterdir())
        if p.is_file() and p.name != "stats.json"
    }
    save_json(args.output / "stats.json", stats)
    log(json.dumps({"corpus": corpus, "evaluation": evaluation, "model": model[4]}, ensure_ascii=False))
    log("TRAINING COMPLETE; run zhuyintype_ml.training.lm.verify against serialized exports")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

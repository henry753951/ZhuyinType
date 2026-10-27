#!/usr/bin/env python3
"""Wikipedia -> Taiwan Traditional, leakage-free hard-EM segmentation, MKN bigram.

AGPL-3.0-only code. Generated data/models: CC BY-SA 4.0. See README.md.
"""
import argparse
import array
import bz2
import collections
import concurrent.futures
import datetime
import gzip
import hashlib
import html
import importlib.metadata
import json
import math
import multiprocessing as mp
import os
from pathlib import Path
import platform
import random
import re
import subprocess
import time
import unicodedata
import xml.etree.ElementTree as ET

import ahocorasick
import mwparserfromhell
import numpy as np
from opencc import OpenCC

HAN = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\U00020000-\U000323af]")
SPLIT = re.compile(r"[。！？；\n]+")
BAD = re.compile(r"https?://|www\.|://|[{}\[\]<>|=`\\]|\b(?:function|SELECT|INSERT|import)\b", re.I)
NS = "{http://www.mediawiki.org/xml/export-0.11/}"
DUMP_URL = "https://dumps.wikimedia.org/zhwiki/latest/zhwiki-latest-pages-articles.xml.bz2"
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"


def log(message):
    print(f"{datetime.datetime.now(datetime.timezone.utc).isoformat()} {message}", flush=True)


def save_json(path, value):
    path = Path(path)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    tmp.replace(path)


def digest(path, algorithm="sha256"):
    h = hashlib.new(algorithm)
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(8 * 1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def variant(match):
    body = match.group(1)
    if ":" not in body:
        return body
    choices = {}
    for item in body.split(";"):
        if ":" in item:
            key, value = item.split(":", 1)
            choices[key.strip().lower()] = value
    return next((choices[k] for k in ("zh-tw", "zh-hant", "zh-hk", "zh", "zh-cn") if k in choices), "")


def clean(wikitext, converter):
    # Templates are deliberately omitted: expanding them needs a MediaWiki runtime.
    text = re.sub(r"-\{(.*?)\}-", variant, wikitext, flags=re.S)
    text = re.split(r"(?m)^==+\s*(?:參考|参考|外部連結|外部链接|注釋|注释|註釋|參見|参见)", text)[0]
    text = re.sub(r"\{\|.*?\|\}", "\n", text, flags=re.S)
    text = "\n".join(line for line in text.splitlines() if not re.match(r"^\s*[|!=]", line))
    code = mwparserfromhell.parse(text)
    for tag in list(code.filter_tags()):
        if str(tag.tag).lower() in {"ref", "references", "math", "chem", "code", "pre", "source", "syntaxhighlight", "gallery", "table", "timeline", "score", "imagemap", "nowiki"}:
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
    for link in list(code.filter_external_links()):
        try:
            code.remove(link)
        except ValueError:
            pass
    text = converter.convert(html.unescape(code.strip_code(normalize=True, collapse=True)))
    result = []
    for sentence in SPLIT.split(unicodedata.normalize("NFC", text)):
        sentence = re.sub(r"\s+", " ", sentence).strip(" \t*#;:：")
        if 4 <= len(sentence) <= 200 and not BAD.search(sentence) and len(HAN.findall(sentence)) / len(sentence) >= .6:
            result.append(sentence)
    return result


def extract_init():
    global CONVERTER
    CONVERTER = OpenCC("s2twp")


def extract_batch(task):
    index, pages, directory = task
    counts = collections.Counter()
    with open(Path(directory) / f"{index:06d}.txt", "w", encoding="utf-8") as out, gzip.open(Path(directory) / f"{index:06d}.articles.tsv.gz", "wt", encoding="utf-8") as attr:
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


def page_batches(dump, directory):
    batch = []
    index = 0
    with bz2.open(dump, "rb") as stream:
        iterator = ET.iterparse(stream, events=("start", "end"))
        _, root = next(iterator)
        for event, element in iterator:
            if event != "end" or element.tag != NS + "page":
                continue
            if element.findtext(NS + "ns") == "0" and element.find(NS + "redirect") is None:
                revision = element.find(NS + "revision")
                text = revision.findtext(NS + "text", "")
                if text and revision.findtext(NS + "model", "wikitext") == "wikitext":
                    batch.append((element.findtext(NS + "id"), revision.findtext(NS + "id"), element.findtext(NS + "title"), text))
            root.clear()
            if len(batch) == 512:
                yield index, batch, str(directory)
                index += 1
                batch = []
        if batch:
            yield index, batch, str(directory)


def extract(args):
    work, out = args.work, args.output
    marker = work / "extract.json"
    if marker.exists():
        log("reuse completed extraction")
        return json.loads(marker.read_text())
    start = time.monotonic()
    directory = work / "extracted"
    directory.mkdir(exist_ok=True)
    totals = collections.Counter()
    batches = iter(page_batches(args.dump, directory))
    # Bounded submission prevents the XML reader from queuing the full dump in RAM.
    with concurrent.futures.ProcessPoolExecutor(args.workers, mp_context=mp.get_context("fork"), initializer=extract_init) as pool:
        pending = set()
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
                    log(f"extract batches={completed} docs={totals['documents_processed']:,} sentences={totals['sentences_before_dedup']:,}")
    with open(out / "articles.tsv.gz", "wb") as destination:
        for path in sorted(directory.glob("*.articles.tsv.gz")):
            with path.open("rb") as source:
                while block := source.read(1024 * 1024):
                    destination.write(block)
    subprocess.run(["sort", "--parallel=" + str(args.workers), "-S", "24G", "-T", str(work), "-u", "-o", str(work / "unique.txt"), *map(str, sorted(directory.glob("*.txt")))], check=True, env={**os.environ, "LC_ALL": "C"})
    totals["extraction_dedup_seconds"] = time.monotonic() - start
    save_json(marker, dict(totals))
    return dict(totals)


def automaton(sentences):
    machine = ahocorasick.Automaton()
    for sentence in sentences:
        for i in range(len(sentence) - 7):
            machine.add_word(sentence[i:i + 8], True)
    if len(machine):
        machine.make_automaton()
    return machine


def split_corpus(args):
    marker = args.work / "split.json"
    if marker.exists():
        return json.loads(marker.read_text())
    start = time.monotonic()
    rng = random.Random(args.seed)
    sample = []
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
    chars = set()
    counts = collections.Counter()
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
    result = {**counts, "training_characters": sorted(chars), "split_seconds": time.monotonic() - start, "seed": args.seed}
    save_json(marker, result)
    return result


def read_lexicon(path):
    rows, old = [], collections.defaultdict(float)
    with open(path, encoding="utf-8") as source:
        header = next(source).rstrip("\n")
        if not header.startswith("#"):
            raise ValueError("lexicon must start with a comment")
        for line in source:
            reading, word, value = line.rstrip("\n").split("\t")
            p = float(value)
            if not math.isfinite(p):
                raise ValueError("non-finite lexicon logp")
            rows.append((reading, word, p))
            old[word] += math.exp(p)
    return header, rows, dict(old)


def make_trie(words, lexicon):
    trie = {}
    for index, word in enumerate(words):
        if word not in lexicon:
            continue
        node = trie
        for ch in word:
            node = node.setdefault(ch, {})
        node[None] = index
    return trie


def segment(sentence, trie, ids, scores, unknown_score):
    n = len(sentence)
    best = [-math.inf] * (n + 1)
    previous = [None] * (n + 1)
    best[0] = 0.
    for i, ch in enumerate(sentence):
        token = ids.get(ch, ids["<unk>"])
        value = best[i] + (scores[token] if ch in ids else unknown_score)
        if value > best[i + 1]:
            best[i + 1], previous[i + 1] = value, (i, token)
        node = trie
        for j in range(i, n):
            node = node.get(sentence[j])
            if node is None:
                break
            token = node.get(None)
            if token is not None:
                value = best[i] + scores[token]
                if value > best[j + 1]:
                    best[j + 1], previous[j + 1] = value, (i, token)
    result = []
    position = n
    while position:
        position, token = previous[position]
        result.append(token)
    result.reverse()
    return result


def segment_shard(task):
    path, directory, final = task
    start = time.monotonic()
    uni = collections.Counter()
    pairs = array.array("Q")
    for line in open(path, encoding="utf-8"):
        tokens = segment(line.rstrip("\n"), TRIE, IDS, SCORES, UNKNOWN_SCORE)
        uni.update(tokens)
        if final:
            sequence = [IDS["<s>"], *tokens, IDS["</s>"]]
            pairs.extend((a << 32) | b for a, b in zip(sequence, sequence[1:]))
    values = np.zeros(len(IDS), dtype=np.uint64)
    for token, count in uni.items():
        values[token] = count
    target = Path(directory) / Path(path).stem
    np.save(str(target) + ".uni.npy", values)
    if final:
        keys, counts = np.unique(np.frombuffer(pairs, dtype=np.uint64), return_counts=True)
        np.save(str(target) + ".keys.npy", keys)
        np.save(str(target) + ".counts.npy", counts.astype(np.uint64))
    return {"tokens": int(values.sum()), "seconds": time.monotonic() - start}


def em(args, old, split):
    global TRIE, IDS, SCORES, UNKNOWN_SCORE
    words = ["<s>", "</s>", "<unk>"] + sorted(set(old) | set(split["training_characters"]))
    IDS = {word: index for index, word in enumerate(words)}
    TRIE = make_trie(words, old)
    normalizer = sum(old.values())
    SCORES = np.array([math.log(old.get(word, 1e-10) / normalizer) for word in words])
    UNKNOWN_SCORE = math.log(1e-10 / normalizer)
    timings = []
    # Initial E step, followed by two M/E updates (three segmentation passes).
    for iteration in range(3):
        directory = args.work / f"em{iteration}"
        directory.mkdir(exist_ok=True)
        marker = directory / "done.json"
        start = time.monotonic()
        if not marker.exists():
            log(f"segment pass={iteration} workers={args.workers}")
            tasks = [(str(path), str(directory), iteration == 2) for path in sorted((args.work / "sentences").glob("*.txt"))]
            with mp.get_context("fork").Pool(args.workers) as pool:
                results = list(pool.imap_unordered(segment_shard, tasks))
            save_json(marker, {"wall_seconds": time.monotonic() - start, "worker_seconds": sum(r["seconds"] for r in results)})
        values = sum((np.load(path) for path in sorted(directory.glob("*.uni.npy"))), np.zeros(len(words), dtype=np.uint64))
        timings.append(json.loads(marker.read_text()))
        if iteration < 2:
            probabilities = (values.astype(float) + .1) / (float(values.sum()) + .1 * len(words))
            SCORES = np.log(probabilities)
            UNKNOWN_SCORE = math.log(.1 / (float(values.sum()) + .1 * len(words)))
        log(f"segment pass={iteration} tokens={int(values.sum()):,} seconds={timings[-1]['wall_seconds']:.1f}")
    # Freeze the final E-step tokenizer for held-out evaluation, without test fitting.
    np.save(args.work / "segmentation_scores.npy", SCORES)
    save_json(args.work / "vocabulary.json", words)
    return words, values, timings


def merge_pairs(args):
    marker = args.work / "merged.json"
    if not marker.exists():
        start = time.monotonic()
        directory = args.work / "em2"
        keys = np.concatenate([np.load(path) for path in sorted(directory.glob("*.keys.npy"))])
        counts = np.concatenate([np.load(path) for path in sorted(directory.glob("*.counts.npy"))])
        log(f"merge shard pair types={len(keys):,}")
        order = np.argsort(keys)
        keys, counts = keys[order], counts[order]
        del order
        starts = np.r_[0, np.flatnonzero(keys[1:] != keys[:-1]) + 1]
        counts = np.add.reduceat(counts, starts)
        keys = keys[starts]
        np.save(args.work / "pair_keys.npy", keys)
        np.save(args.work / "pair_counts.npy", counts)
        save_json(marker, {"seconds": time.monotonic() - start, "pair_types": len(keys)})
    return np.load(args.work / "pair_keys.npy"), np.load(args.work / "pair_counts.npy")


def discounts(counts):
    nr = [0] + [int(np.count_nonzero(counts == r)) for r in range(1, 5)]
    y = nr[1] / (nr[1] + 2 * nr[2]) if nr[1] else .5
    result = []
    for r in range(1, 4):
        value = r - (r + 1) * y * nr[r + 1] / nr[r] if nr[r] and nr[r + 1] else .75
        result.append(min(r - 1e-6, max(1e-6, value)))
    return np.array(result), nr


def fit_mkn(keys, counts, size, cap):
    left = (keys >> np.uint64(32)).astype(np.int64)
    right = (keys & np.uint64(0xffffffff)).astype(np.int64)
    d, nr = discounts(counts)
    # KN continuation unigram; reserve a tiny floor only for zero-continuation words.
    continuation = np.bincount(right, minlength=size).astype(float)
    continuation[continuation == 0] = .1
    continuation[0] = 0.  # BOS is context-only.
    continuation[2] = max(1., continuation[2])
    unigram = continuation / continuation.sum()
    context_total = np.bincount(left, weights=counts.astype(float), minlength=size)
    removed = d[np.minimum(counts, 3).astype(np.int64) - 1]
    lambdas = np.divide(np.bincount(left, weights=removed, minlength=size), context_total, out=np.ones(size), where=context_total > 0)
    threshold = 2
    while np.count_nonzero(counts >= threshold) > cap:
        threshold += 1
    keep = counts >= threshold
    selected_keys = keys[keep]
    a, b = left[keep], right[keep]
    probabilities = (counts[keep] - removed[keep]) / context_total[a] + lambdas[a] * unigram[b]
    kept_mass = np.bincount(a, weights=probabilities, minlength=size)
    kept_lower = np.bincount(a, weights=unigram[b], minlength=size)
    alpha = np.divide(1 - kept_mass, 1 - kept_lower, out=np.ones(size), where=kept_lower < 1)
    if not (np.all(probabilities > 0) and np.all(alpha > 0)):
        raise ValueError("invalid probabilities/backoff")
    error = np.max(np.abs(kept_mass + alpha * (1 - kept_lower) - 1))
    assert error < 1e-10
    return unigram, selected_keys, probabilities, alpha, {
        "discounts_D1_D2_D3plus": d.tolist(), "bigram_count_of_counts_N1_to_N4": nr[1:],
        "prune_count_threshold": threshold, "bigram_types_before_pruning": len(keys),
        "bigram_types_exported": len(selected_keys), "normalization_error_all_contexts_float64": float(error),
        "unseen_unigram_pseudocontinuations": .1, "unk_pseudocontinuations": 1.,
    }


def export(args, words, uni_counts, old, rows, header, model):
    unigram, keys, probabilities, alpha, _ = model
    ids = {w: i for i, w in enumerate(words)}
    with open(args.output / "unigram.tsv", "w", encoding="utf-8") as f:
        for i, word in enumerate(words):
            if word != "<s>":
                f.write(f"{word}\t{math.log(unigram[i]):.12f}\n")
    with open(args.output / "bigram.tsv", "w", encoding="utf-8") as f:
        for key, p in zip(keys, probabilities):
            a, b = int(key) >> 32, int(key) & 0xffffffff
            f.write(f"{words[a]}\t{words[b]}\t{math.log(p):.12f}\n")
    with open(args.output / "backoff.tsv", "w", encoding="utf-8") as f:
        for a in np.unique(keys >> np.uint64(32)):
            f.write(f"{words[int(a)]}\t{math.log(alpha[int(a)]):.12f}\n")
    common = [w for w in old if uni_counts[ids[w]] > 0]
    shift = float(np.median([math.log(unigram[ids[w]]) - math.log(old[w]) for w in common]))
    with open(args.output / "lexicon_tw.tsv", "w", encoding="utf-8") as f:
        f.write(header + " Taiwan corpus word frequencies: Chinese Wikipedia / OpenCC s2twp; CC BY-SA 4.0. See nlp/training/lm/README.md and stats.json.\n")
        for reading, word, p in rows:
            count = int(uni_counts[ids[word]])
            value = math.log(unigram[ids[word]]) + p - math.log(old[word]) if count else p + shift
            f.write(f"{reading}\t{word}\t{value:.12f}\n")
    return {"lexicon_rows": len(rows), "lexicon_words": len(old), "lexicon_words_seen": len(common),
            "lexicon_type_coverage": len(common) / len(old), "unseen_lexicon_logp_shift": shift,
            "lexicon_frequency_estimator": "exported KN corpus unigram; median log-ratio scale shift for unseen words"}


def evaluate(args, words, uni_counts, old, model):
    unigram, keys, probabilities, alpha, _ = model
    scores = np.load(args.work / "segmentation_scores.npy")
    ids = {w: i for i, w in enumerate(words)}
    trie = make_trie(words, old)
    tokens_total = lexical_oov = model_oov = train_oov = 0
    uni_nll = bi_nll = 0.
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
    return {"test_tokens_excluding_boundaries": tokens_total, "test_predictions_including_EOS": predictions,
            "test_lexicon_oov_tokens": lexical_oov, "test_lexicon_oov_rate": lexical_oov / tokens_total,
            "test_model_unk_tokens": model_oov, "test_model_unk_rate": model_oov / tokens_total,
            "test_unseen_training_tokens": train_oov, "test_unseen_training_token_rate": train_oov / tokens_total,
            "test_unigram_perplexity": math.exp(uni_nll / predictions), "test_bigram_perplexity": math.exp(bi_nll / predictions),
            "test_bigram_hit_rate": hits / predictions, "perplexity_protocol": "same frozen final-E-step word tokens; include EOS, exclude BOS; no test tuning"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dump", type=Path, required=True)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=64)
    parser.add_argument("--holdout", type=int, default=5000)
    parser.add_argument("--seed", type=int, default=20261026)
    parser.add_argument("--max-bigrams", type=int, default=3000000)
    args = parser.parse_args()
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
    save_json(args.output / "conversion_spotchecks.json", [{"input": s, "s2twp": converter.convert(s)} for s in spotchecks])
    (args.output / "NOTICE.md").write_text(
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
        "Retain this notice and the article attribution manifest when redistributing outputs.\n",
        encoding="utf-8")
    examples = {}
    ids = {w: i for i, w in enumerate(words)}
    for context in ("我", "今天", "台灣", "的", "臺灣"):
        a = ids[context]
        positions = np.flatnonzero((model[1] >> np.uint64(32)) == a)
        distribution = model[3][a] * model[0].copy()
        explicit = set()
        for i in positions:
            b = int(model[1][i]) & 0xffffffff
            distribution[b] = model[2][i]
            explicit.add(b)
        top = np.argsort(-distribution)[:5]
        examples[context] = [{"next": words[b], "probability": float(distribution[b]), "stored_bigram": int(b) in explicit} for b in top]
    save_json(args.output / "examples.json", examples)
    corpus = {**extraction, **{k: v for k, v in split.items() if k != "training_characters"},
              "training_tokens_excluding_boundaries": int(uni_counts.sum()),
              "training_bigram_events_including_boundaries": int(counts.sum()),
              "training_lexicon_oov_tokens": sum(int(uni_counts[i]) for i, w in enumerate(words) if w not in old),
              "training_token_types": int(np.count_nonzero(uni_counts))}
    stats = {"status": "trained_pending_independent_validation", "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
             "license": "CC BY-SA 4.0", "license_url": LICENSE_URL, "corpus": corpus, "vocabulary": vocabulary,
             "model": {**model[4], "unigram_types": len(words) - 1, "extra_tokens": "observed fallback characters, </s>, <unk>; <s> context-only"},
             "evaluation": evaluation, "examples": examples,
             "timings": {"segmentation_passes": em_times, "merge": json.loads((args.work / "merged.json").read_text()),
                         "fit_export_evaluate_seconds": time.monotonic() - start, "pipeline_invocation_seconds": time.monotonic() - started},
             "sources": [{"name": "Chinese Wikipedia pages-articles", "requested_url": DUMP_URL,
                          "license": "CC BY-SA 4.0", "license_url": LICENSE_URL,
                          "terms_url": "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use",
                          "attribution": "Chinese Wikipedia contributors; articles.tsv.gz page/revision IDs resolve to article and author-history links",
                          "transformations": "namespace 0 nonredirect text; omit templates/references/tables/code; OpenCC s2twp; clean/exact-dedupe/holdout/8-character exclusion; 2 hard-EM updates; MKN and count pruning",
                          "receipt": receipt},
                         {"name": "ZhuyinType seed lexicon", "path": "data/build/lexicon.tsv", "sha256": config["lexicon_sha256"],
                          "license": "CC BY-SA 4.0", "components": ["CC-CEDICT (CC BY-SA 4.0)", "Unicode Unihan (Unicode License v3)", "wordfreq data (CC BY-SA 4.0)"],
                          "urls": ["https://www.mdbg.net/chinese/dictionary?page=cc-cedict", "https://www.unicode.org/charts/unihan.html", "https://github.com/rspeer/wordfreq"]}],
             "runtime": {"python": platform.python_version(), "platform": platform.platform(), "workers": args.workers,
                         "packages": {name: importlib.metadata.version(name) for name in ("numpy", "opencc", "mwparserfromhell", "pyahocorasick")}},
             "training_code_sha256": {p.name: digest(p) for p in sorted(Path(__file__).parent.iterdir()) if p.suffix in {".py", ".sh", ".txt"}},
             "decisions": ["Wikipedia only; no optional web corpus", "User-requested 4-200-character sentences override MODEL.md 4-64", "Exact deduplication as requested, no MinHash", "5000 uniform random unique sentences with fixed seed before all corpus counting", "Initial lexicon Viterbi plus two unigram re-estimation/resegmentation updates", "All fallback characters remain distinct tokens; novel test characters map to <unk>", "Exported KN unigram also supplies seen-word lexicon_tw probabilities; empirical counts supply hard EM", "Uniform count threshold selected from training counts only; backoff renormalized after pruning", "TSV deliverables only; Swift integration and quantized mmap export outside requested scope"]}
    stats["files"] = {p.name: {"bytes": p.stat().st_size, "sha256": digest(p)} for p in sorted(args.output.iterdir()) if p.is_file() and p.name != "stats.json"}
    save_json(args.output / "stats.json", stats)
    log(json.dumps({"corpus": corpus, "evaluation": evaluation, "model": model[4]}, ensure_ascii=False))
    log("TRAINING COMPLETE; run verify.py against serialized exports")


if __name__ == "__main__":
    main()

"""Encode ordered documents, supervised readings and confidence into mmap shards."""
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import json
import multiprocessing as mp
from pathlib import Path
import re
import time

import numpy as np
import torch

from common import OUT, save
from readings import Lexicon, load_poly, window


def init():
    global LEX, POLY, LABELS, CHARS, SYLLABLES
    torch.set_num_threads(1)
    LEX = Lexicon(OUT / "lexicon.tsv")
    POLY, LABELS = load_poly()
    CHARS = {c: i for i, c in enumerate(json.load(open(OUT / "chars.json")))}
    SYLLABLES = {s: i for i, s in enumerate(json.load(open(OUT / "syllables.json")))}


def annotate(texts):
    results, windows, positions = [], [], []
    label_index = {tuple(r): i for i, r in enumerate(LABELS)}
    for row, text in enumerate(texts):
        readings = []
        uncertain = [False] * len(text)
        for start, end, word, reading in LEX.segments(text):
            readings.extend(reading)
            if len(word) == 1 and word in LEX.poly:
                allowed = sorted({label_index[r] for _, r in LEX.single[word] if r in label_index})
                if allowed:
                    windows.append(window(text, start, CHARS))
                    positions.append((row, start, allowed))
                else:
                    uncertain[start] = True
        results.append([readings, uncertain])
    with torch.no_grad():
        for start in range(0, len(windows), 4096):
            logits = POLY(torch.tensor(windows[start:start + 4096]))
            for scores, (row, pos, allowed) in zip(logits, positions[start:start + 4096]):
                probs = scores[allowed].softmax(0)
                p, index = probs.max(0)
                if p.item() >= .9:
                    results[row][0][pos] = tuple(LABELS[allowed[index.item()]])
                else:
                    results[row][1][pos] = True
    return results


def encode(task):
    source, name, colloquial = task
    target = OUT / "encoded" / name
    stats = Counter()
    ids, syllables, tones, uncertain = [], [], [], []
    documents, examples = [], []
    document_start = 0
    previous_colloquial = ""

    def finish_document():
        nonlocal document_start
        if len(ids) > document_start:
            documents.append((document_start, len(ids)))
        document_start = len(ids)

    def consume(lines):
        nonlocal previous_colloquial, document_start
        texts = [line.strip() for line in lines]
        annotations = annotate(texts)
        for text, (reading, low) in zip(texts, annotations):
            if not text:
                finish_document()
                previous_colloquial = ""
                continue
            if colloquial:
                finish_document()
                prefix = previous_colloquial[-255:]
                previous_colloquial = text
            else:
                prefix = ""
            if len(ids) == document_start:
                ids.append(1); syllables.append(1); tones.append(0); uncertain.append(0)
            if prefix:
                ids.extend(CHARS.get(c, 2) for c in prefix)
                syllables.extend([1] * len(prefix)); tones.extend([0] * len(prefix)); uncertain.extend([0] * len(prefix))
            start = len(ids)
            ids.extend(CHARS.get(c, 2) for c in text)
            syllables.extend(SYLLABLES.get(r[0], 1) for r in reading)
            tones.extend(r[1] for r in reading)
            uncertain.extend(low)
            stats["characters"] += len(text)
            stats["low_confidence_characters"] += sum(low)
            # Sentence boundaries retain the true preceding paragraph/line as context.
            for match in re.finditer(r"[^。！？；]+[。！？；]?", text):
                for offset in range(match.start(), match.end(), 64):
                    end = min(offset + 64, match.end())
                    if end - offset < 2:
                        continue
                    stats["examples"] += 1
                    if any(low[offset:end]):
                        stats["low_confidence_examples"] += 1
                    else:
                        examples.append((start + offset, start + end, document_start))
            ids.append(3); syllables.append(1); tones.append(0); uncertain.append(0)
    batch = []
    for line in Path(source).open():
        batch.append(line)
        if len(batch) == 128:
            consume(batch)
            batch = []
    consume(batch)
    finish_document()
    np.asarray(ids, np.uint16).tofile(str(target) + ".ids")
    np.asarray(syllables, np.uint16).tofile(str(target) + ".syllables")
    np.asarray(tones, np.uint8).tofile(str(target) + ".tones")
    np.save(str(target) + ".documents.npy", np.asarray(documents, np.int64).reshape(-1, 2))
    np.save(str(target) + ".examples.npy", np.asarray(examples, np.int64).reshape(-1, 3))
    stats["confident_examples"] = len(examples)
    stats["tokens"] = len(ids)
    print(name, dict(stats), flush=True)
    return {"name": name, **stats}


def main():
    directory = OUT / "encoded"
    directory.mkdir(exist_ok=True)
    colloquial = OUT / "colloquial_shards"
    colloquial.mkdir(exist_ok=True)
    handle = None
    for i, line in enumerate((OUT / "colloquial.txt").open()):
        if i % 50000 == 0:
            if handle:
                handle.close()
            handle = (colloquial / f"{i // 50000:06d}.txt").open("w")
        handle.write(line)
    if handle:
        handle.close()
    tasks = [(str(p), "wiki-" + p.stem, False) for p in sorted((OUT / "wiki_shards").glob("*.txt"))]
    tasks += [(str(p), "colloquial-" + p.stem, True) for p in sorted(colloquial.glob("*.txt"))]
    start = time.monotonic()
    with ProcessPoolExecutor(12, mp_context=mp.get_context("spawn"), initializer=init) as pool:
        results = list(pool.map(encode, tasks))
    totals = Counter()
    for result in results:
        totals.update({k: v for k, v in result.items() if k != "name"})
    save(OUT / "encode.json", {"shards": results, "totals": dict(totals), "seconds": time.monotonic() - start})


if __name__ == "__main__":
    main()

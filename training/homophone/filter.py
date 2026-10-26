"""Leakage-free indexes into immutable v3 shards; filter context and target together."""
from collections import Counter
import csv
from concurrent.futures import ProcessPoolExecutor
import json
import multiprocessing as mp
from pathlib import Path
import time

import numpy as np

from common import BASE, OUT, holdouts, save, v3


def init():
    global HELD, VOCAB, GROUPS, CHAR_GROUPS
    VOCAB = json.loads((BASE/'chars.json').read_text())
    HELD = holdouts(new=True)
    known = set(VOCAB[4:]) | {'\n'}
    for text in list(HELD.exact):
        HELD.add(''.join(c if c in known else '\ufffd' for c in text))
    HELD.compile()
    GROUPS = json.loads((OUT/'inventory.json').read_text())['kept']
    CHAR_GROUPS = {i: [g['chars'] for g in GROUPS if c in g['chars']] for i,c in enumerate(VOCAB)}


def filter_shard(row):
    name = row['name']
    base = BASE/'encoded'/name
    ids = np.memmap(str(base)+'.ids', np.uint16, 'r')
    documents = np.load(str(base)+'.documents.npy')
    examples = np.load(str(base)+'.examples.npy')
    bad = []
    for start,end in documents:
        text = ''.join(VOCAB[n] if n >= 4 else '\n' if n == 3 else '\ufffd' if n == 2 else '' for n in ids[start:end])
        if text in HELD:
            bad.append(start)
    allowed = ~np.isin(examples[:,2], bad)
    indexes = np.flatnonzero(allowed)
    np.save(OUT/'filtered'/f'{name}.normal.npy', indexes)
    groups = {g['chars']: [] for g in GROUPS}
    groups.update({'primary-在': [], 'primary-再': []})
    for index in indexes:
        start,end,_ = examples[index]
        chars = {VOCAB[n] for n in np.unique(ids[start:end])}
        found = {g for n in np.unique(ids[start:end]) for g in CHAR_GROUPS[int(n)]}
        found.update('primary-'+c for c in chars.intersection('在再'))
        for group in found:
            groups[group].append(index)
    counts = {}
    for group, indices in groups.items():
        np.save(OUT/'filtered'/f'{name}.{group}.npy', np.asarray(indices, np.int64))
        counts[group] = len(indices)
    print(name, len(indexes), 'rejected', int((~allowed).sum()), flush=True)
    return dict(name=name, allowed=len(indexes), rejected=int((~allowed).sum()), groups=counts)


def main():
    directory = OUT/'filtered'
    directory.mkdir(exist_ok=False)
    start = time.monotonic()
    rows = json.loads((BASE/'encode.json').read_text())['shards']
    with ProcessPoolExecutor(12, mp_context=mp.get_context('spawn'), initializer=init) as pool:
        result = list(pool.map(filter_shard, rows))
    save(OUT/'filtered.json', dict(shards=result, allowed=sum(r['allowed'] for r in result),
         rejected=sum(r['rejected'] for r in result), seconds=time.monotonic()-start,
         protocol='Whole original document removed on any forbidden 8-gram or short exact substring, including context and joins.'))
    v3()
    from dataset import Builder
    from readings import load_poly
    builder = Builder()
    poly,labels = load_poly()
    excluded = holdouts(new=True).compile()
    seen,kept = set(),0
    with (OUT/'inputs/train.tsv').open(newline='') as source, (OUT/'tt-train.jsonl').open('x') as target:
        for row in csv.DictReader(source, delimiter='\t'):
            text = ' '.join(row['sentence'].split())
            if row['locale']!='zh-TW' or text in seen or text in excluded or not 2<=len(text)<=64:
                continue
            seen.add(text)
            if not any(c in text for c in '在再'):
                continue
            reading,low = builder.lex.label(text,poly,builder.chars,labels)
            if low:
                continue
            target.write(json.dumps(dict(text=text,context='',readings=reading),ensure_ascii=False)+'\n')
            kept += 1
    save(OUT/'tt-train.json', dict(sentences=kept,protocol='unique zh-TW train.tsv only; all holdouts excluded; confident v3 labels'))


if __name__ == '__main__':
    main()

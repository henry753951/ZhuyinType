#!/usr/bin/env python3
"""Deterministic, offline TSV -> ZTLM/1 + ZTLX/1. Python standard library only."""
import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import re
import struct

SPECIAL = ['</s>', '<s>', '<unk>']
LM_HEADER = struct.Struct('<4s5I5Q')
LEX_HEADER = struct.Struct('<4s7I12Q')


def rows(path, fields, spaces=False):
    # Match Swift scanTSV, including comments and its optional ASCII-space separator.
    for line in path.read_text(encoding='utf-8').split('\n'):
        if line.startswith('#'):
            continue
        parts = re.split('[\t ]+', line.rstrip('\r')) if spaces else line.rstrip('\r').split('\t')
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


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def publish(path, data, metadata):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix('.bin.tmp')
    tmp.write_bytes(data)
    tmp.replace(path)
    metadata.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    path.with_suffix('.json').write_text(json.dumps(metadata, ensure_ascii=False, sort_keys=True, indent=2) + '\n')
    print(f'{path}: {len(data):,} bytes')


def section(data, content):
    data.extend(bytes(-len(data) % 8))
    start = len(data)
    data.extend(content)
    return start


def pack_array(fmt, values):
    record = struct.Struct('<' + fmt)
    return b''.join(record.pack(*v) if isinstance(v, tuple) else record.pack(v) for v in values)


def compile_lm(source, target):
    uni = dict(rows(source / 'unigram.tsv', 2, True))
    bi = {(a, b): p for a, b, p in rows(source / 'bigram.tsv', 3, True)}
    back = dict(rows(source / 'backoff.tsv', 2, True))
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
    vo = section(data, pack_array('I', offsets) + blob)
    uo = section(data, pack_array('f', (uni.get(w, 0) for w in words)) +
                 pack_array('f', (back.get(w, 0) for w in words)))
    bo = section(data, pack_array('I', starts) + pack_array('I', (b for _, b, _ in pairs)) +
                 pack_array('f', (p for _, _, p in pairs)) + bytes(4 * len(pairs)))
    data[:64] = LM_HEADER.pack(b'ZTLM', 1, 2, len(words), len(pairs), 0, vo, uo, bo, 0, len(data))
    publish(target / 'lm.bin', data, {
        'format': 'ZTLM/1', 'order': 2, 'vocabulary': len(words), 'bigrams': len(pairs),
        'tsv_compatibility': True, 'missing_unigrams': [ids[w] for w in words if w not in uni],
        'unknown_word_shift': 0, 'vocabulary_order': 'reserved-then-utf8',
        'source_hashes': {name: digest(source / name) for name in ['unigram.tsv', 'bigram.tsv', 'backoff.tsv']},
    })


def toneless(reading):
    return reading.translate(str.maketrans('', '', 'ˊˇˋ˙'))


def compile_lexicon(source, target):
    original = [(r, w, p) for r, w, p in rows(source, 3) if r and w]
    words = sorted({w for _, w, _ in original}, key=lambda w: w.encode())
    readings = sorted({r for r, _, _ in original}, key=lambda r: r.encode())
    wi = {w: i for i, w in enumerate(words)}
    ri = {r: i for i, r in enumerate(readings)}
    top = {}
    for _, w, p in original:
        top[w] = max(top.get(w, -math.inf), p)
    grouped = defaultdict(list)
    for ordinal, (r, w, p) in enumerate(original):
        grouped[r].append((w, p, ordinal))
    blob = bytearray()
    strings = {}
    def string(s):
        if s not in strings:
            b = s.encode()
            strings[s] = (len(blob), len(b))
            blob.extend(b)
        return strings[s]
    entries, reading_records = [], []
    reverse = defaultdict(list)
    for r in readings:
        start = len(entries)
        for w, p, ordinal in grouped[r]:
            entry = len(entries)
            entries.append((wi[w], ri[r], top[w], p - top[w]))
            reverse[w].append((ordinal, entry, p))
        reading_records.append((*string(r), start, len(entries) - start))
    refs, word_records = [], []
    for w in words:
        items = sorted(reverse[w])
        best = max(items, key=lambda x: x[2])[1]
        word_records.append((*string(w), len(refs), len(items), best, 0))
        refs.extend(entry for _, entry, _ in items)
    bases = defaultdict(list)
    prefixes, syllables = set(), set()
    for r in readings:
        base = toneless(r)
        bases[base].append(ri[r])
        parts = base.split()
        prefixes.update(' '.join(parts[:i]) for i in range(1, len(parts) + 1))
        syllables.update(r.split())
    base_records, base_refs = [], []
    for b in sorted(bases, key=lambda b: b.encode()):
        base_records.append((*string(b), len(base_refs), len(bases[b])))
        base_refs.extend(bases[b])
    prefix_records = [string(p) for p in sorted(prefixes, key=lambda p: p.encode())]
    syllable_records = [string(s) for s in sorted(syllables, key=lambda s: s.encode())]
    data = bytearray(128)
    sections = [section(data, pack_array(fmt, values)) for fmt, values in [
        ('6I', word_records), ('4I', reading_records), ('2I2d', entries), ('I', refs),
        ('4I', base_records), ('I', base_refs), ('2I', prefix_records), ('2I', syllable_records)]]
    sections.append(section(data, blob))
    data[:128] = LEX_HEADER.pack(b'ZTLX', 1, len(words), len(readings), len(entries), len(bases),
                                len(prefixes), len(syllables), *sections, len(data), 0, 0)
    publish(target / 'lexicon.bin', data, {'format': 'ZTLX/1', 'words': len(words), 'readings': len(readings),
        'entries': len(entries), 'source_hashes': {'lexicon.tsv': digest(source)}})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2] / 'data/build')
    a = parser.parse_args()
    compile_lm(a.source / 'lm', a.output / 'lm')
    compile_lm(a.source / 'lm_chat', a.output / 'lm_chat')
    compile_lexicon(a.source / 'lexicon.tsv', a.output)


if __name__ == '__main__':
    main()

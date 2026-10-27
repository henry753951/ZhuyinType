"""Existing three-pass Viterbi/MKN estimator, with observed turn-boundary bigrams."""
import argparse
from collections import Counter
import datetime
import json
from pathlib import Path
import platform
import time
import numpy as np
from common import ROOT, OUT, lm, chat, save, digest, evaluation_files, blocked_text, excluded
from prepare import normalize


def audit(work):
    paths = evaluation_files() + [OUT / 'dev_dialogue.txt', OUT / 'test_dialogue.txt']
    held = blocked_text(paths)
    held.update(normalize(s).replace('臺', '台') for s in list(held))
    machine = lm.automaton(held)
    counts = Counter()
    for path in sorted((work / 'sentences').glob('*.txt')) + [work / 'context_pairs.tsv']:
        with path.open() as f:
            for line in f:
                for text in line.rstrip('\n').split('\t'):
                    assert not excluded(text, held, machine), (path, text)
                    assert not excluded(normalize(text).replace('臺', '台'), held, machine), (path, text)
                    counts['texts_rescanned'] += 1
    return {**counts, 'exact_or_eight_character_overlap': 0,
            'evaluation_files': {str(p.relative_to(ROOT)): digest(p) for p in paths}}


def boundary_counts(work, words, old, keys, counts):
    ids = {w: i for i, w in enumerate(words)}
    trie = lm.make_trie(words, old)
    scores = np.load(work / 'segmentation_scores.npy')
    import re
    extra = Counter()
    for line in (work / 'context_pairs.tsv').open():
        previous, target = line.rstrip('\n').split('\t')
        a = re.findall(r'[\u4e00-\u9fff]+', previous)
        b = re.findall(r'[\u4e00-\u9fff]+', target)
        if not a or not b: continue
        left = lm.segment(a[-1], trie, ids, scores, float(scores[2]))
        right = lm.segment(b[0], trie, ids, scores, float(scores[2]))
        if left and right: extra[(left[-1] << 32) | right[0]] += 1
    all_keys = np.r_[keys, np.array(list(extra), dtype=np.uint64)]
    all_counts = np.r_[counts, np.array(list(extra.values()), dtype=np.uint64)]
    order = np.argsort(all_keys)
    all_keys, all_counts = all_keys[order], all_counts[order]
    starts = np.r_[0, np.flatnonzero(all_keys[1:] != all_keys[:-1]) + 1]
    return all_keys[starts], np.add.reduceat(all_counts, starts), sum(extra.values())


def main(a):
    started = time.monotonic()
    a.output = OUT if a.variant == 'mixed' else OUT / 'no_generated'
    a.work = a.output / 'work'
    prepared = json.loads((a.work / 'prepared.json').read_text())
    files = sorted((a.work / 'sentences').glob('*.txt')) + [a.work / 'context_pairs.tsv', OUT / 'inputs/lexicon.tsv']
    config = {'workers': a.workers, 'variant': a.variant, 'max_bytes': a.max_bytes,
              'inputs': {str(p.relative_to(ROOT)): digest(p) for p in files},
              'code': {str(p.relative_to(ROOT)): digest(p) for p in
                       [*Path(__file__).parent.glob('*.py'), Path(lm.__file__), Path(chat.__file__)]}}
    checkpoint = a.work / 'train-config.json'
    if checkpoint.exists() and json.loads(checkpoint.read_text()) != config:
        raise ValueError('frozen inputs/code changed; choose a fresh work tree')
    save(checkpoint, config)
    _, _, old = lm.read_lexicon(OUT / 'inputs/lexicon.tsv')
    words, values, timings = lm.em(a, old, prepared)
    keys, counts = lm.merge_pairs(a)
    keys, counts, boundaries = boundary_counts(a.work, words, old, keys, counts)
    cap = 1_600_000
    while True:
        model = lm.fit_mkn(keys, counts, len(words), cap)
        chat.export(a.output, words, model)
        size = sum((a.output / f'{n}.tsv').stat().st_size for n in ('unigram', 'bigram', 'backoff'))
        if size <= a.max_bytes: break
        cap = int(len(model[1]) * a.max_bytes / size * .9)
        if cap < 1: raise ValueError('vocabulary exceeds budget')
    validation = chat.Model(a.output).verify()
    validation['leakage'] = audit(a.work)
    save(a.output / 'validation.json', validation)
    save(a.output / 'stats.json', {'status': 'validated', 'created_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
         'corpus': {k: v for k, v in prepared.items() if k != 'training_characters'},
         'model': {**model[4], 'tsv_bytes': size, 'unigram_types': len(words) - 1,
                   'word_tokens': int(values.sum()), 'supplemental_turn_boundary_events': boundaries},
         'segmentation': timings, 'config': config, 'python': platform.python_version(),
         'numpy': np.__version__, 'seconds': time.monotonic() - started,
         'files': {n: digest(a.output / n) for n in ('unigram.tsv', 'bigram.tsv', 'backoff.tsv')}})
    print('VALIDATED', a.variant, size, 'bytes', flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--variant', choices=['mixed', 'real'], default='mixed')
    p.add_argument('--workers', type=int, default=24)
    p.add_argument('--max-bytes', type=int, default=60_000_000)
    main(p.parse_args())

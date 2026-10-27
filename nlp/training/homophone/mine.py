"""Count curated confusions and reservoir-sample source-attributed candidates."""
from collections import Counter, defaultdict
import argparse
import csv
import json
import random
import re
import time

from common import BASE, OUT, SEEDS, digest, holdouts, positions, save, v3


def sources():
    for name in ['wiki', 'colloquial']:
        previous = ''
        with (BASE / f'{name}.txt').open() as f:
            for number, line in enumerate(f, 1):
                text = line.strip()
                yield name, number, text, previous if name == 'wiki' else ''
                previous = text if text else ''
    seen = set()
    with (OUT / 'inputs/train.tsv').open(newline='') as f:
        for number, row in enumerate(csv.DictReader(f, delimiter='\t'), 2):
            if row.get('locale') != 'zh-TW':
                continue
            text = ' '.join(row['sentence'].split())
            if text in seen or len(re.findall('[一-鿿]', text)) < 2 or re.search('[A-Za-z]{4}', text):
                continue
            seen.add(text)
            yield 'taiwan_tongues_train', number, text, ''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-inventory', action='store_true',
                        help='Reuse completed corpus counts; sample every twentieth wiki line and all Taiwan Tongues rows.')
    args = parser.parse_args()
    v3()
    from readings import Lexicon
    lex = Lexicon(OUT / 'lexicon.tsv')
    inventory = []
    for chars in SEEDS:
        readings = set.intersection(*[{r for _, r in lex.single[c]} for c in chars])
        if readings:
            inventory.append(dict(chars=chars, readings=sorted(readings)))
    frozen = None
    if args.reuse_inventory:
        frozen = json.loads((OUT/'inventory.json').read_text())
        assert frozen['lexicon_sha256'] == digest(OUT/'lexicon.tsv')
        inventory = frozen['kept']
    target = set(''.join(g['chars'] for g in inventory))
    held = holdouts().compile()
    counts, initial, source_counts = Counter(), Counter(), Counter()
    pools, seen_counts = defaultdict(list), Counter()
    rng = random.Random(20261027)
    start = time.monotonic()
    for source, number, text, previous in sources():
        source_counts[source] += 1
        if frozen and (source == 'colloquial' or (source == 'wiki' and number % 20)):
            continue
        if not text or text in held:
            continue
        hits = target.intersection(text)
        if not hits:
            continue
        if not frozen:
            for c in hits:
                counts[c] += text.count(c)
            for _, _, word, _ in lex.segments(text):
                if word[0] in target:
                    initial[word[0]] += 1
        if source == 'colloquial':
            continue
        for match in re.finditer('[一-鿿]+', text):
            sentence = match.group()
            if not 4 <= len(sentence) < 32 or not target.intersection(sentence) or sentence in held:
                continue
            if (match.start() and text[match.start()-1].isalnum()) or (match.end()<len(text) and text[match.end()].isalnum()):
                continue
            context = (previous + '\n' + text[:match.start()])[-32:].strip()
            if context in held or (context + sentence) in held:
                context = ''
            pos = positions(sentence, lex, inventory)
            if not pos:
                continue
            row = dict(text=sentence, context=context, source=source, line=number,
                       offset=match.start(), positions=pos)
            for key in {(p['group'], p['char'], p['category'], source) for p in pos}:
                seen_counts[key] += 1
                pool = pools[key]
                if len(pool) < (800 if key[0]=='在再' else 160):
                    pool.append(row)
                else:
                    j = rng.randrange(seen_counts[key])
                    if j < len(pool):
                        pool[j] = row
        if source_counts[source] % 1000000 == 0:
            print(source, source_counts[source], round(time.monotonic()-start, 1), flush=True)
    rejected, kept = [], []
    for group in inventory:
        if not frozen:
            group.update(counts={c: counts[c] for c in group['chars']},
                         standalone_or_initial={c: initial[c] for c in group['chars']})
        (kept if min(group['counts'].values()) >= 2000 and min(group['standalone_or_initial'].values()) >= 2000 else rejected).append(group)
    rejected += [dict(chars=c, reason='no common toned standalone lexicon reading')
                 for c in SEEDS if c not in {g['chars'] for g in inventory}]
    if not frozen:
        save(OUT / 'inventory.json', dict(kept=kept, rejected=rejected, sources=source_counts,
         seconds=time.monotonic()-start, lexicon_sha256=digest(OUT / 'lexicon.tsv'),
             counts_protocol='filtered text occurrences; longest-match standalone/word-initial counts; sources once each'))
    allowed = {g['chars'] for g in kept}
    unique = {}
    for key, pool in sorted(pools.items()):
        if key[0] in allowed:
            for row in pool:
                row['positions'] = [p for p in row['positions'] if p['group'] in allowed]
                unique.setdefault(row['text'], row)
    with (OUT / 'candidates.jsonl').open('w') as f:
        for text in sorted(unique):
            f.write(json.dumps(unique[text], ensure_ascii=False) + '\n')
    print('done', len(unique), 'candidates', len(kept), 'groups', flush=True)
    assert unique, 'No candidates: inspect reading matching and filters before judging.'
    save(OUT/'mining.json', dict(candidates=len(unique), seconds=time.monotonic()-start,
         reused_counts=bool(frozen), wiki_stride=20 if frozen else 1,
         protocol='Whole 4-31 Han clauses; never truncate long clauses; reservoir by character/category/source.'))


if __name__ == '__main__':
    main()

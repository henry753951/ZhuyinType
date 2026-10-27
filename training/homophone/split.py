"""Lock balanced source-text holdouts before constructing any training sampler."""
from collections import Counter
import json
import random
import re
import sys

from common import CATEGORIES, OUT, ROOT, SMOKE, holdouts, jsonl, save


def main():
    assert not (OUT/'splits.json').exists(), 'Splits are immutable after creation.'
    rows = [r for r in jsonl(OUT/'judged.jsonl') if r['judge']['accepted'] and len(r['text'])<32
            and not r['text'].endswith(('在','再'))
            and not (r['offset']>0 and (not r['context'] or re.search('[一-鿿]$',r['context'])))]
    sys.path.insert(0,str(ROOT/'data/build/homophone'))
    from conversion_eval import load_lexicon, readings_for
    lexicon = load_lexicon(OUT/'lexicon.tsv')
    rows = [r for r in rows if readings_for(r['text'],lexicon) is not None]
    for row in rows:
        row['context'] = row['context'].replace('\n',' ').replace('\t',' ').strip()
    rng = random.Random(20261027)
    rng.shuffle(rows)
    exclusions = holdouts()
    inventory = json.loads((OUT/'inventory.json').read_text())['kept']
    stats, datasets = {}, {}
    for kind in ['zai','pairs']:
        selected = {s:[] for s in ['dev','test']}
        for group in inventory:
            name = group['chars']
            if (name=='在再') != (kind=='zai'):
                continue
            target = 1000 if kind=='zai' else 100
            buckets = {(c,cat):[r for r in rows if any(p['group']==name and p['char']==c and
                       p['category']==cat for p in r['positions'])] for c in name for cat in CATEGORIES}
            for bucket in buckets.values():
                rng.shuffle(bucket)
            chars = {s:Counter() for s in selected}
            cats = {s:Counter() for s in selected}
            chosen = {s:[] for s in selected}
            # Alternate splits so rare characters and categories cannot be consumed by dev alone.
            for turn in range(target):
                for split in selected:
                    found = None
                    for c in sorted(name,key=lambda c:(chars[split][c],c)):
                        for cat in sorted(CATEGORIES,key=lambda cat:(cats[split][cat],cat)):
                            bucket = buckets[c,cat]
                            while bucket:
                                row = bucket.pop()
                                if any(t and t in exclusions for t in [row['text'],row['context'],row['context']+row['text']]):
                                    continue
                                found = {**row,'stratum':dict(group=name,char=c,category=cat)}
                                break
                            if found:
                                break
                        if found:
                            break
                    if found:
                        chosen[split].append(found)
                        chars[split][c] += 1
                        cats[split][cat] += 1
                        for text in [found['text'],found['context'],found['context']+found['text']]:
                            exclusions.add(text)
            for split in selected:
                selected[split].extend(chosen[split])
        for split, records in selected.items():
            rng.shuffle(records)
            stem = f'{split}_{kind}'
            datasets[stem] = records
            relevant = [p for r in records for p in r['positions'] if (p['group']=='在再')==(kind=='zai')]
            stats[stem] = dict(sentences=len(records),sources=Counter(r['source'] for r in records),
                  characters=Counter(p['char'] for p in relevant),categories=Counter(p['category'] for p in relevant),
                  strata=Counter(str(r['stratum']) for r in records))
    for split in ['dev_zai','test_zai']:
        counts = stats[split]['characters']
        assert stats[split]['sentences']==1000 and min(counts[c] for c in '在再') >= 400, stats[split]
    for stem, records in datasets.items():
        with (OUT/f'{stem}.txt').open('x') as text, (OUT/f'{stem}.jsonl').open('x') as meta:
            for row in records:
                text.write(row['context']+'\t'+row['text']+'\n')
                meta.write(json.dumps(row,ensure_ascii=False)+'\n')
    (OUT/'smoke.txt').write_text('\n'.join(SMOKE)+'\n')
    sample = rng.sample(datasets['dev_zai']+datasets['test_zai'],100)
    with (OUT/'review100.jsonl').open('x') as f:
        for i,row in enumerate(sample):
            f.write(json.dumps(dict(id=i,text=row['text'],context=row['context'],positions=row['positions']),ensure_ascii=False)+'\n')
    save(OUT/'splits.json',dict(seed=20261027,sets=stats,
         exclusion='No shared exact text or 8-character substring across splits, contexts included; smoke excluded.',
         exposure='Source-mined holdouts may have been seen by original checkpoints. Only continuation excludes them.'))
    print(json.dumps(stats,ensure_ascii=False,indent=2))


if __name__=='__main__':
    main()

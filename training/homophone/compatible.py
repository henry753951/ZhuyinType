"""Rebuild continuation indexes and real-source examples without Taiwan Tongues."""
import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import json
import multiprocessing as mp
from pathlib import Path
import re
import shutil
import sys
import time

import numpy as np

from common import BASE, ROOT, OUT as OLD, Exclusions, SMOKE, digest, jsonl, save, v3

OUT = ROOT/'data/build/homophone2'


def evaluation_files():
    paths = [ROOT/p for p in (
        'data/build/lm/test_sentences.txt', 'data/build/corpus/test_colloquial.txt',
        'data/build/corpus/dev_colloquial.txt', 'data/build/corpus/dev_wikipedia.txt',
        'data/build/lm_chat/dev_chat_tt.txt', 'data/build/lm_chat/test_chat_tt.txt',
        'data/build/lm_chat2/dev_dialogue.txt', 'data/build/lm_chat2/test_dialogue.txt')]
    paths += [p for prefix in ('dev_', 'test_') for p in OLD.glob(prefix+'*.txt')]
    paths += [OLD/'smoke.txt'] + list((ROOT/'training/eval').rglob('*.txt'))
    return sorted(set(paths))


def blocked_fields(line):
    if not line.strip() or line.startswith('#'):
        return []
    fields = line.split('|')[:2] if '|' in line else line.split('\t')
    # Short source contexts are not independent evaluation sentences.
    return [fields[-1]] + [f for f in fields[:-1] if len(f)>=8] + [''.join(fields)]


def exclusions():
    vocab = set(json.loads((BASE/'chars.json').read_text())[4:]) | {'\n'}
    held = Exclusions(SMOKE)
    for path in evaluation_files():
        for line in path.read_text().splitlines():
            for text in blocked_fields(line):
                for variant in (text, text.replace('臺','台')):
                    held.add(variant)
                    held.add(''.join(c if c in vocab else '\ufffd' for c in variant))
    return held.compile()


def init_filter():
    import filter as filtering
    filtering.OUT = OUT
    filtering.holdouts = lambda **_: exclusions()
    filtering.init()
    filtering.VOCAB = [c.replace('臺','台') for c in filtering.VOCAB]


def filter_shard(row):
    import filter as filtering
    result = filtering.filter_shard(row)
    stem = BASE/'encoded'/row['name']
    ids = np.memmap(str(stem)+'.ids',np.uint16,'r')
    examples = np.load(str(stem)+'.examples.npy',mmap_mode='r')
    vocab = json.loads((BASE/'chars.json').read_text())
    for char in '在再':
        group = 'initial-'+char
        choices = np.load(OUT/'filtered'/f'{row["name"]}.primary-{char}.npy')
        choices = choices[ids[examples[choices,0]]==vocab.index(char)]
        np.save(OUT/'filtered'/f'{row["name"]}.{group}.npy',choices)
        result['groups'][group] = len(choices)
    return result


def real_examples():
    v3()
    from dataset import Builder
    from readings import load_poly
    builder = Builder()
    poly, labels = load_poly()
    held = exclusions()
    reserved = json.loads((ROOT/'data/build/lm_chat2/splits.json').read_text())['reserved_documents']
    seen, counts = set(), Counter()
    def rows():
        for record in jsonl(ROOT/'data/build/lm_chat2/real_dialogues.jsonl'):
            assert record['source'] in ('gazette','wiki_talk')
            if record['source']+':'+record['document'] in reserved:
                continue
            previous = ''
            for turn in record['turns']:
                text = turn['text']
                # Retain complete clauses and their actual preceding text, never synthetic prefixes.
                for match in re.finditer(r'[\u4e00-\u9fff]{4,64}',text):
                    if match.start() and re.match(r'[A-Za-z0-9\u4e00-\u9fff]',text[match.start()-1]): continue
                    if match.end()<len(text) and re.match(r'[A-Za-z0-9\u4e00-\u9fff]',text[match.end()]): continue
                    yield dict(text=match[0],context=(previous+'\n'+text[:match.start()])[-255:],
                               source=record['source'],document=record['document'],url=record['url'])
                previous = text if text else ''
        for line in (ROOT/'data/build/corpus/train_commonvoice.txt').open():
            text = line.strip()
            if 4<=len(text)<=64:
                yield dict(text=text,context='',source='commonvoice')
    with (OUT/'real-train.jsonl').open('x') as f:
        for row in rows():
            counts[row['source']+'_candidates'] += 1
            text,ctx = row['text'],row['context']
            # Check the whole context-target join, plus each side independently.
            if any(s in held or s.replace('臺','台') in held for s in (text,ctx,ctx+text)):
                counts['excluded'] += 1
                continue
            if text in seen: continue
            readings,low = builder.lex.label(text,poly,builder.chars,labels)
            if low: continue
            row['readings'] = readings
            seen.add(text)
            f.write(json.dumps(row,ensure_ascii=False)+'\n')
            counts[row['source']+'_kept'] += 1
            for char in '在再':
                counts['contains_'+char] += char in text
                counts['initial_'+char] += text.startswith(char)
    save(OUT/'real-train.json',dict(counts))
    print(dict(counts),flush=True)


def audit():
    """Independent rescan of every selected document and supplemental context join."""
    held = exclusions()
    vocab = json.loads((BASE/'chars.json').read_text())
    count = documents = 0
    for row in json.loads((OUT/'filtered.json').read_text())['shards']:
        stem = BASE/'encoded'/row['name']
        ids = np.memmap(str(stem)+'.ids',np.uint16,'r')
        examples = np.load(str(stem)+'.examples.npy')
        allowed = np.load(OUT/'filtered'/f'{row["name"]}.normal.npy')
        starts = set(examples[allowed,2].tolist())
        for start,end in np.load(str(stem)+'.documents.npy'):
            if start not in starts: continue
            text = ''.join(vocab[n] if n>=4 else '\n' if n==3 else '\ufffd' if n==2 else '' for n in ids[start:end])
            assert text not in held and text.replace('臺','台') not in held, row['name']
            documents += 1
        count += len(allowed)
    real = 0
    for row in jsonl(OUT/'real-train.jsonl'):
        assert row['source'] in ('gazette','wiki_talk','commonvoice')
        for s in (row['text'],row['context'],row['context']+row['text']):
            assert s not in held and s.replace('臺','台') not in held
        real += 1
    save(OUT/'leakage-audit.json',dict(examples=count,documents=documents,real=real,violations=0))


def target_indexes():
    from train_v3 import target_text
    destination=ROOT/'data/build/homophone2b'
    destination.mkdir(exist_ok=False)
    for path in OUT.glob('*.txt'):
        shutil.copy2(path,destination/path.name)
    for name in ['inventory.json','dev_zai.jsonl','dev_pairs.jsonl','test_zai.jsonl','test_pairs.jsonl','inputs.json','leakage-audit.json','real-train.json']:
        shutil.copy2(OUT/name,destination/name)
    (destination/'filtered').symlink_to('../homophone2/filtered',target_is_directory=True)
    (destination/'real-train.jsonl').symlink_to('../homophone2/real-train.jsonl')
    vocab=json.loads((BASE/'chars.json').read_text())
    manifest=json.loads((OUT/'filtered.json').read_text())
    for row in manifest['shards']:
        stem=BASE/'encoded'/row['name']
        ids=np.memmap(str(stem)+'.ids',np.uint16,'r')
        examples=np.load(str(stem)+'.examples.npy',mmap_mode='r')
        for char in '在再':
            parent=np.load(OUT/'filtered'/f'{row["name"]}.primary-{char}.npy')
            kept=[]
            for index in parent:
                start,end,_=examples[index]
                text=''.join(vocab[n] if n>=4 else '\ufffd' for n in ids[start:end])
                if target_text(text,char): kept.append(index)
            group='target-'+char
            path=OUT/'filtered'/f'{row["name"]}.{group}.npy'
            assert not path.exists()
            np.save(path,np.asarray(kept,np.int64))
            row['groups'][group]=len(kept)
    save(destination/'filtered.json',manifest)
    counts={c:dict(shards=sum(r['groups']['target-'+c] for r in manifest['shards']),real=0) for c in '在再'}
    for row in jsonl(OUT/'real-train.jsonl'):
        for char in counts: counts[char]['real']+=target_text(row['text'],char)
    assert all(r['shards'] and r['real'] for r in counts.values())
    save(destination/'targeted.json',counts)
    print(counts,flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase',choices=['prepare','audit','test','target'])
    p.add_argument('--workers',type=int,default=4)
    a = p.parse_args()
    if a.phase=='test':
        assert blocked_fields('前文前文前文前文|目標|ㄇㄨˋ ㄅㄧㄠ')==['目標','前文前文前文前文','前文前文前文前文目標']
        held = Exclusions(['一二三四五六七八','我在家']).compile()
        assert '零一二三四五六七八九' in held and '他說我在家呢' in held and '再試一次' not in held
        assert all(p.exists() for p in evaluation_files())
        print('compatible self-check passed')
        return
    assert sys.platform=='linux', 'Preparation and audits run on GPU host.'
    if a.phase=='target':
        target_indexes()
        return
    if a.phase=='audit':
        audit()
        return
    OUT.mkdir(exist_ok=False)
    for name in ['inventory.json','dev_zai.txt','dev_zai.jsonl','dev_pairs.txt','dev_pairs.jsonl','test_zai.txt','test_zai.jsonl','test_pairs.txt','test_pairs.jsonl','smoke.txt']:
        shutil.copy2(OLD/name,OUT/name)
    source_paths = [BASE/n for n in ('encode.json','colloquial.json','provenance.json','syllable.pt','context.pt')]
    source_paths += [ROOT/'data/build/lm_chat2'/n for n in ('real_dialogues.jsonl','splits.json','source-manifest.json')]
    source_paths += [ROOT/'data/build/corpus/train_commonvoice.txt']
    save(OUT/'inputs.json',dict(sources={str(p.relative_to(ROOT)):digest(p) for p in source_paths},
          holdouts={str(p.relative_to(ROOT)):digest(p) for p in evaluation_files()},
          protocol='Source allowlist: Wikipedia, deduplicated Qwen/Common Voice, gazette, wiki talk, Common Voice. No TT training source.'))
    (OUT/'filtered').mkdir()
    rows = json.loads((BASE/'encode.json').read_text())['shards']
    assert all(r['name'].startswith(('wiki-','colloquial-')) for r in rows)
    started = time.monotonic()
    with ProcessPoolExecutor(a.workers,mp_context=mp.get_context('spawn'),initializer=init_filter) as pool:
        result = list(pool.map(filter_shard,rows,chunksize=4))
    save(OUT/'filtered.json',dict(shards=result,allowed=sum(r['allowed'] for r in result),
        rejected=sum(r['rejected'] for r in result),seconds=time.monotonic()-started))
    real_examples()
    audit()


if __name__=='__main__':
    main()

#!/usr/bin/env python3
"""Freeze CC0 Common Voice splits before generation; exclude held-out 8-grams."""
import argparse
import hashlib
import json
import random
import re
import unicodedata
import urllib.request
from pathlib import Path
from opencc import OpenCC
import ahocorasick

SEED = 20261026

def fetch(url, path):
    data = urllib.request.urlopen(url).read()
    path.write_bytes(data)
    return {'url': url, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}

def exclusion(sentences):
    machine = ahocorasick.Automaton()
    exact = set(sentences)
    for s in exact:
        for i in range(len(s)-7):
            machine.add_word(s[i:i+8], True)
    machine.make_automaton()
    return lambda s: s in exact or next(machine.iter(s), None) is not None

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--root', type=Path, default=Path('.'))
    args = p.parse_args()
    out = args.root / 'data/build/corpus'
    raw = out / 'raw'
    raw.mkdir(parents=True, exist_ok=True)
    if (out/'splits.json').exists():
        print((out/'splits.json').read_text()); return
    commit = json.load(urllib.request.urlopen('https://api.github.com/repos/common-voice/common-voice/commits/main'))['sha']
    receipt = fetch(f'https://raw.githubusercontent.com/common-voice/common-voice/{commit}/server/data/zh-TW/sentence-collector.txt', raw/'sentence-collector.txt')
    receipt['commit'] = commit
    receipt['license'] = 'CC0-1.0'
    receipt['license_evidence'] = 'https://github.com/common-voice/common-voice/blob/'+commit+'/docs/submitting-bulk-sentences.md'
    fetch(receipt['license_evidence'].replace('github.com','raw.githubusercontent.com').replace('/blob/','/'), raw/'common-voice-submission-policy.md')
    cc = OpenCC('s2t')
    lines = sorted({s for line in (raw/'sentence-collector.txt').read_text().splitlines()
                    if 4 <= len(s := unicodedata.normalize('NFC', line.strip())) <= 60
                    and len(re.findall(r'[\u3400-\u9fff]',s)) >= .6*len(s)
                    and cc.convert(s) == s})
    assert len(lines) > 5000, len(lines)
    random.Random(SEED).shuffle(lines)
    test, dev, train = lines[:3000], lines[3000:5000], lines[5000:]
    blocked = exclusion(test+dev)
    train = [s for s in train if not blocked(s)]
    for name, values in [('test_colloquial',test),('dev_colloquial',dev),('train_commonvoice',train)]:
        (out/(name+'.txt')).write_text('\n'.join(values)+'\n')
    # The old Wikipedia training shards have already excluded the fixed Wikipedia test.
    shards = sorted((args.root/'work/sentences').glob('*.txt'))
    reservoir=[]; rng=random.Random(SEED+1); seen=0
    for shard in shards:
        for line in shard.open():
            seen+=1
            if len(reservoir)<2000: reservoir.append(line.strip())
            else:
                j=rng.randrange(seen)
                if j<2000: reservoir[j]=line.strip()
    assert len(reservoir)==2000
    (out/'dev_wikipedia.txt').write_text('\n'.join(reservoir)+'\n')
    # Also protect Wikipedia dev/test against synthetic leakage.
    stats={'seed':SEED,'source':receipt,'clean_unique':len(lines),'test_colloquial':len(test),
           'dev_colloquial':len(dev),'train_commonvoice':len(train),'dev_wikipedia':len(reservoir),
           'wikipedia_candidates':seen,'policy':'strict OpenCC s2t identity; training excludes held-out equality and every 8-char overlap'}
    (out/'splits.json').write_text(json.dumps(stats,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(stats,ensure_ascii=False))

if __name__=='__main__': main()

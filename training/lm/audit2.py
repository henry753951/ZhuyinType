#!/usr/bin/env python3
"""Independent post-training corpus separation, lexicon and binary audit."""
import argparse
from collections import Counter,defaultdict
import json
import math
from pathlib import Path
import random
import sys
import time
import numpy as np
import lmbin
import pipeline
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'corpus'))
from prepare import exclusion
from clean import normalized
from opencc import OpenCC

def main(args):
    root=Path(args.root); corpus=root/'data/build/corpus'; out=root/'data/build/lm2'; started=time.time()
    stats=json.loads((out/'stats.json').read_text())
    synthetic=root/stats['config']['synthetic_input']
    assert pipeline.digest(synthetic)==stats['config']['inputs'][stats['config']['synthetic_input']]
    held=[]
    for path in [corpus/'test_colloquial.txt',corpus/'dev_colloquial.txt',corpus/'dev_wikipedia.txt',root/'data/build/lm/test_sentences.txt']:
        held+=path.read_text().splitlines()
    assert len(held)==12000
    blocked=exclusion(held); counts=Counter()
    for folder in ['wiki','colloq']:
        for path in sorted((root/'work2'/folder).glob('*.txt')):
            for line in path.open():
                assert not blocked(line.strip()),(path,line)
                counts[folder]+=1; counts[folder+'_characters']+=len(line.rstrip('\n'))
    cc=OpenCC('s2t'); tw=OpenCC('s2twp'); sample=[]; rng=random.Random(20261027); clean_count=0
    for clean_count,line in enumerate(synthetic.open(),1):
        s=line.rstrip('\n'); counts['synthetic_characters']+=len(s)
        for a,b in [(4,12),(13,24),(25,45),(46,80)]:
            if a<=len(s)<=b: counts[f'synthetic_length_{a}_{b}']+=1
        value,reason=normalized(s,cc,tw)
        assert value==s and reason is None,(s,reason)
        assert not blocked(s)
        if len(sample)<50: sample.append(s)
        else:
            i=rng.randrange(clean_count)
            if i<50: sample[i]=s
    assert clean_count>0
    assert clean_count==stats['corpus'][synthetic.name+'_input_sentences'],clean_count
    (out/'generated_samples_50.txt').write_text('\n'.join(sample)+'\n')
    _,seed,old=pipeline.read_lexicon(root/'data/build/lexicon.tsv')
    _,rows,new=pipeline.read_lexicon(out/'lexicon.tsv')
    assert [(r,w) for r,w,_ in rows[:len(seed)]]==[(r,w) for r,w,_ in seed]
    before=defaultdict(list); after=defaultdict(list)
    for r,w,p in seed: before[w].append((r,p))
    for r,w,p in rows: after[w].append((r,p))
    for w,readings in before.items():
        if len(readings)>1:
            shift=after[w][0][1]-readings[0][1]
            assert all(abs((q-p)-shift)<3e-10 for (_,p),(_,q) in zip(readings,after[w]))
    model=lmbin.Model(out/'lm.bin')
    preference={}
    for word in ['臺灣','臺北','臺中','臺南','臺東','臺北市','臺灣人','舞臺','櫃臺','月臺']:
        other=word.replace('臺','台')
        if word in model.ids and other in model.ids:
            preference[other]={'台_logp':float(model.uni[model.ids[other]]),'臺_logp':float(model.uni[model.ids[word]])}
    result={'status':'validated','heldout_sentences':len(held),'full_8char_overlap_scan':dict(counts),
            'synthetic_sentences_rechecked':clean_count,'sample_seed':20261027,'seed_lexicon_rows_preserved':len(seed),
            'new_lexicon_rows':len(rows)-len(seed),'polyphone_ratios_preserved':True,'tai_preference':preference,
            'binary':lmbin.verify(out/'lm.bin',512),'seconds':time.time()-started}
    pipeline.save_json(out/'validation.json',result)
    stats['validation']=result
    stats['corpus']['audited_character_counts']=dict(counts)
    stats['corpus']['synthetic_input']=stats['config']['synthetic_input']
    stats['corpus']['synthetic_sha256']=pipeline.digest(synthetic)
    stats['corpus']['commonvoice_split_receipt']=json.loads((corpus/'splits.json').read_text())
    stats['generation_config']=json.loads((corpus/'generation-config.json').read_text())
    stats['generation_progress_at_audit']=json.loads((corpus/'clean-progress.json').read_text())
    usage=Counter(); times=[]
    for line in (corpus/'raw/synthetic.jsonl').open():
        if not line.endswith('\n'): break
        record=json.loads(line); usage['requests']+=1; usage['output_tokens']+=record['usage'].get('completion_tokens',0)
        usage['finish_'+record['finish_reason']]+=1; times.append(record['created'])
    stats['generation_at_audit']={**usage,'first_completion':min(times),'last_completion':max(times),'elapsed_completion_seconds':max(times)-min(times)}
    for path in [out/'validation.json',out/'generated_samples_50.txt']:
        stats['files'][path.name]={'bytes':path.stat().st_size,'sha256':pipeline.digest(path)}
    pipeline.save_json(out/'stats.json',stats)
    print(json.dumps(result,ensure_ascii=False,indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--root',default='.'); main(p.parse_args())

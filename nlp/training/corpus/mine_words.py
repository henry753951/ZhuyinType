#!/usr/bin/env python3
"""Conservative phrase mining: frequency, cohesion, boundaries, certain readings."""
import argparse
import ast
from collections import Counter,defaultdict
import json
import math
from pathlib import Path
import re
import unicodedata
from prepare import exclusion

def reading_converter(path):
    tree=ast.parse(path.read_text())
    names={'INITIALS','FINALS','Y_W','TONE_MARKS','DIACRITICS'}
    nodes=[n for n in tree.body if (isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id in names for t in n.targets)) or (isinstance(n,ast.FunctionDef) and n.name in {'numbered_from_diacritic','to_zhuyin','cedict_reading'})]
    scope={'re':re,'unicodedata':unicodedata}; exec(compile(ast.Module(body=nodes,type_ignores=[]),str(path),'exec'),scope)
    return scope['cedict_reading']

def main(args):
    root=Path(args.root); out=root/'data/build/lm2'; out.mkdir(exist_ok=True)
    source=root/'data/build/lexicon.tsv'; rows=[]; readings=defaultdict(set)
    for line in source.read_text().splitlines():
        if line.startswith('#'): continue
        r,w,p=line.split('\t'); rows.append((r,w,float(p))); readings[w].add(r)
    convert=reading_converter(root/'data/tools/build_lexicon.py')
    cedict=defaultdict(set); taiwan={}
    for line in (root/'data/raw/cedict.txt').open():
        match=re.match(r'^(\S+) \S+ \[([^\]]+)\] /(.*)/',line)
        if not match: continue
        w,py,defs=match.groups(); r=convert(py)
        if r and len(r.split())==len(w): cedict[w].add(r)
        tw=re.search(r'Taiwan pr\. \[([^\]]+)\]',defs)
        if tw and (r:=convert(tw[1])) and len(r.split())==len(w): taiwan[w]=r
    held=[]
    for name in ['test_colloquial.txt','dev_colloquial.txt','dev_wikipedia.txt']:
        held+=(root/'data/build/corpus'/name).read_text().splitlines()
    held+=(root/'data/build/lm/test_sentences.txt').read_text().splitlines()
    blocked=exclusion(held)
    counts=Counter(); chars=Counter(); total=0
    synthetic=root/args.synthetic
    paths=[root/'data/build/corpus/train_commonvoice.txt',synthetic]
    for path in paths:
        for line in path.open():
            if blocked(line.strip()): continue
            for run in re.findall('[\u3400-\u9fff]+',line):
                chars.update(run); total+=len(run)
                counts.update(run[i:i+n] for n in range(2,7) for i in range(len(run)-n+1))
    candidates={w:n for w,n in counts.items() if n>=args.minimum and w not in readings}
    left=defaultdict(Counter); right=defaultdict(Counter)
    for path in paths:
        for line in path.open():
            s=line.strip()
            if blocked(s): continue
            for n in range(2,7):
                for i in range(len(s)-n+1):
                    w=s[i:i+n]
                    if w in candidates:
                        left[w][s[i-1] if i else '^']+=1; right[w][s[i+n] if i+n<len(s) else '$']+=1
    def frequency(w): return chars[w] if len(w)==1 else counts[w]
    def entropy(c):
        z=sum(c.values()); return -sum(n/z*math.log(n/z) for n in c.values())
    added=[]
    for w,n in candidates.items():
        cohesion=min(math.log(n*total/(frequency(w[:i])*frequency(w[i:]))) for i in range(1,len(w)))
        if cohesion<2 or min(entropy(left[w]),entropy(right[w]))<.6: continue
        if w in taiwan: r=taiwan[w]; evidence='CC-CEDICT Taiwan reading'
        elif len(cedict[w])==1: r=next(iter(cedict[w])); evidence='CC-CEDICT unique word reading'
        elif all(len(readings[ch])==1 for ch in w): r=' '.join(next(iter(readings[ch])) for ch in w); evidence='every character has exactly one seed reading'
        else: continue
        added.append((w,n,r,evidence,cohesion))
    added.sort(key=lambda x:(-x[1],x[0]))
    added=added[:args.cap]
    with (out/'new_words.tsv').open('w') as f:
        f.write('# word\tcount\treading\tevidence\tcohesion\n')
        for w,n,r,e,c in added: f.write(f'{w}\t{n}\t{r}\t{e}\t{c:.6f}\n')
    with (out/'lexicon.seed.tsv').open('w') as f:
        f.write(source.read_text())
        for w,n,r,_,_ in added: f.write(f'{r}\t{w}\t{math.log(n/total):.12f}\n')
    (out/'mining.json').write_text(json.dumps({'minimum_count':args.minimum,'cohesion_min':2,'boundary_entropy_min':.6,'added':len(added),'han_characters':total,'candidate_count':len(candidates),'synthetic_input':args.synthetic,'source_sentences':'synthetic input plus Common Voice training only'},indent=2)+'\n')
    print((out/'mining.json').read_text(),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(); p.add_argument('--root',default='.'); p.add_argument('--synthetic',default='data/build/corpus/synthetic_phase1.txt'); p.add_argument('--minimum',type=int,default=200); p.add_argument('--cap',type=int,default=2000); main(p.parse_args())

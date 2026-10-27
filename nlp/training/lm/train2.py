#!/usr/bin/env python3
"""Frozen-split mixed-corpus hard EM, MKN candidate selection and ZTLM export."""
import argparse
from collections import Counter
import datetime
import json
import math
import importlib.metadata
import platform
import multiprocessing as mp
import os
from pathlib import Path
import shutil
import subprocess
import time
import numpy as np
import pipeline
import lmbin
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'corpus'))
from prepare import exclusion

STATE={}

def token_words(sentence):
    tokens=pipeline.segment(sentence,STATE['trie'],STATE['ids'],STATE['scores'],STATE['unknown'])
    return [STATE['words'][i] for i in tokens]

def worker(task):
    source,dest,final=task; observed=Counter(); lines=0
    with open(source) as f:
        output=open(dest,'w') if final else None
        try:
            for line in f:
                s=line.rstrip('\n'); tokens=token_words(s)
                observed.update(STATE['ids'][w] for w in tokens)
                if output:
                    # Keep both spellings typeable, with a 9:1 token-level preference for 台.
                    tokens=[w.replace('臺','台') if lines%10 and w.replace('臺','台') in STATE['lexicon'] else w for w in tokens]
                    tokens=[w if w.encode()>b'<unk>' and not any(c.isspace() for c in w) else '�' for w in tokens]
                    output.write(' '.join(tokens)+'\n')
                lines+=1
        finally:
            if output: output.close()
    counts=np.zeros(len(STATE['words']),dtype=np.uint64)
    for i,n in observed.items(): counts[i]=n
    return counts,lines

def prepare(root,workers,synthetic=None):
    work=root/'work2'; marker=work/'prepared.json'
    if marker.exists(): return json.loads(marker.read_text())
    out=root/'data/build/corpus'; wiki=work/'wiki'; wiki.mkdir(exist_ok=True)
    held=[]
    for name in ['test_colloquial.txt','dev_colloquial.txt','dev_wikipedia.txt']:
        held+=(out/name).read_text().splitlines()
    held+=(root/'data/build/lm/test_sentences.txt').read_text().splitlines()
    blocked=exclusion(held); result=Counter(); chars=set()
    wiki_marker=work/'wiki_prepared.json'
    if wiki_marker.exists():
        saved=json.loads(wiki_marker.read_text()); result.update(saved['counts']); chars.update(saved['characters'])
    else:
        for i,src in enumerate(sorted((root/'work/sentences').glob('*.txt'))):
            with (wiki/src.name).open('w') as dest:
                for line in src.open():
                    s=line.strip()
                    if blocked(s): result['wikipedia_overlap_removed']+=1; continue
                    dest.write(s+'\n'); result['wikipedia_sentences']+=1; chars.update(s)
        pipeline.save_json(wiki_marker,{'counts':dict(result),'characters':sorted(chars)})
    if synthetic is None and not (out/'PHASE1_READY').exists():
        if getattr(prepare,'wiki_only',False): return dict(result)
        raise ValueError('phase one generation is not complete')
    colloq=work/'colloq'; colloq.mkdir(exist_ok=True)
    handles=[(colloq/f'{i:03d}.txt').open('w') for i in range(workers)]
    seen=set()
    try:
        for path in [out/'train_commonvoice.txt',synthetic or out/'synthetic_phase1.txt']:
            name=path.name
            for line in path.open():
                result[name+'_input_sentences']+=1
                s=line.strip()
                if blocked(s): result[name+'_removed']+=1; continue
                if s in seen: result['colloquial_exact_duplicates_removed']+=1; continue
                seen.add(s)
                handles[result['colloquial_sentences']%workers].write(s+'\n')
                result['colloquial_sentences']+=1; result[name]+=1; chars.update(s)
    finally:
        for h in handles: h.close()
    value={**result,'characters':sorted(chars)}; pipeline.save_json(marker,value)
    return value

def segment(root,workers,info):
    global STATE
    work=root/'work2'; lex=root/'data/build/lm2/lexicon.seed.tsv'
    header,rows,old=pipeline.read_lexicon(lex)
    words=sorted(set(old)|set(info['characters'])|{'<s>','</s>','<unk>'})
    STATE={'words':words,'ids':{w:i for i,w in enumerate(words)},'lexicon':old}
    STATE['trie']=pipeline.make_trie(words,old)
    normal=sum(old.values()); STATE['scores']=np.array([math.log(old.get(w,1e-10)/normal) for w in words]); STATE['unknown']=math.log(1e-10/normal)
    paths=sorted((work/'wiki').glob('*.txt'))+sorted((work/'colloq').glob('*.txt')); timings=[]
    for iteration in range(3):
        started=time.time(); directory=work/f'mix_em{iteration}'; directory.mkdir(exist_ok=True)
        marker=directory/'done.json'
        if marker.exists(): counts=np.load(directory/'counts.npy')
        else:
            tasks=[]
            for path in paths:
                (directory/path.parent.name).mkdir(exist_ok=True)
                tasks.append((str(path),str(directory/path.parent.name/path.name),iteration==2))
            with mp.get_context('fork').Pool(workers) as pool:
                results=list(pool.imap_unordered(worker,tasks))
            counts=sum((c for c,_ in results),np.zeros(len(words),dtype=np.uint64))
            np.save(directory/'counts.npy',counts); pipeline.save_json(marker,{'seconds':time.time()-started,'tokens':int(counts.sum())})
        timings.append(json.loads(marker.read_text()))
        if iteration<2:
            STATE['scores']=np.log((counts.astype(float)+.1)/(float(counts.sum())+.1*len(words)))
            STATE['unknown']=math.log(.1/(float(counts.sum())+.1*len(words)))
        pipeline.log(f'EM {iteration}: {timings[-1]}')
    np.save(work/'mix_segmentation_scores.npy',STATE['scores']); pipeline.save_json(work/'mix_vocabulary.json',words)
    return header,rows,old,counts,timings

def evaluations(root):
    out=root/'data/build/corpus'; files={'dev_wikipedia':out/'dev_wikipedia.txt','dev_colloquial':out/'dev_colloquial.txt',
        'test_wikipedia':root/'data/build/lm/test_sentences.txt','test_colloquial':out/'test_colloquial.txt'}
    data={}
    for name,path in files.items():
        tokens=[]; oov=0; total=0
        for line in path.open():
            row=token_words(line.strip()); oov+=sum(w not in STATE['lexicon'] for w in row); total+=len(row)
            # Identical spelling policy for all model orders; do not alter Taiwanese held-out spelling.
            row=[w if w=='<unk>' or (w.encode()>b'<unk>' and not any(c.isspace() for c in w)) else '�' for w in row]
            tokens.append(row)
        data[name]={'sentences':tokens,'lexicon_oov_tokens':oov,'tokens':total}
    return data

def evaluate(model,data):
    sums=[0.,0.,0.]; predictions=0; unknown=0
    for row in data['sentences']:
        history=['<s>']
        for word in [*row,'</s>']:
            sums[0]+=model.score(word,[]); sums[1]+=model.score(word,history[-1:]); sums[2]+=model.score(word,history[-2:])
            unknown+=word not in model.ids or word=='<unk>'; predictions+=1; history.append(word)
    return {'unigram_perplexity':math.exp(-sums[0]/predictions),'bigram_perplexity':math.exp(-sums[1]/predictions),
            'trigram_perplexity':math.exp(-sums[2]/predictions),'predictions':predictions,'log_likelihoods':sums,
            'model_oov_rate':unknown/predictions,'lexicon_oov_rate':data['lexicon_oov_tokens']/data['tokens'],
            'sentences':len(data['sentences']),'tokens':data['tokens']}

def main(args):
    start=time.time(); root=Path(args.root).resolve(); work=root/'work2'; out=root/'data/build/lm2'
    if args.prepare_wiki:
        prepare.wiki_only=True; print(prepare(root,args.workers)); return
    synthetic=(root/args.synthetic).resolve() if args.synthetic else root/'data/build/corpus/synthetic_phase1.txt'
    config={'workers':args.workers,'weights':args.weights,'synthetic_input':str(synthetic.relative_to(root)),'inputs':{str(p.relative_to(root)):pipeline.digest(p) for p in [out/'lexicon.seed.tsv',synthetic,root/'data/build/corpus/train_commonvoice.txt',root/'data/build/corpus/dev_wikipedia.txt',root/'data/build/corpus/dev_colloquial.txt',root/'data/build/corpus/test_colloquial.txt',root/'data/build/lm/test_sentences.txt']},'code':{p.name:pipeline.digest(p) for p in [Path(__file__),Path(lmbin.__file__),Path(pipeline.__file__)]}}
    checkpoint=work/'training2-config.json'
    if checkpoint.exists() and json.loads(checkpoint.read_text())!=config:
        raise ValueError('training checkpoint inputs/code/settings changed; use a fresh work2 training directory')
    pipeline.save_json(checkpoint,config)
    info=prepare(root,args.workers,synthetic if args.synthetic else None); header,rows,old,counts,timings=segment(root,args.workers,info); sets=evaluations(root)
    candidates=[]; kenlm=work/'kenlm/build/bin/lmplz'; environment=dict(os.environ,LD_LIBRARY_PATH=str(work/'sysroot/usr/lib/x86_64-linux-gnu'),OMP_NUM_THREADS='16')
    for weight in args.weights:
        arpa=work/f'mix_k{weight}.arpa'; binary=work/f'mix_k{weight}.bin'; marker=work/f'mix_k{weight}.json'
        started=time.time()
        if not arpa.exists():
            command=[str(kenlm),'-o','3','-S','40G','-T',str(work/f'kenlm-k{weight}'), '--prune','0','8','12','--discount_fallback']
            with (root/f'logs/kenlm-k{weight}.log').open('w') as log, arpa.with_suffix('.tmp').open('w') as output:
                proc=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=output,stderr=log,env=environment)
                try:
                    for directory,repeats in [('wiki',1),('colloq',weight)]:
                        for _ in range(repeats):
                            for path in sorted((work/'mix_em2'/directory).glob('*.txt')):
                                with path.open('rb') as source: shutil.copyfileobj(source,proc.stdin,8*1024*1024)
                    proc.stdin.close()
                    if proc.wait(): raise RuntimeError(f'lmplz failed: see {log.name}')
                except BaseException: proc.kill(); raise
            arpa.with_suffix('.tmp').replace(arpa)
        if not binary.exists():
            # Bigrams consume 16B each including trigram pointers; fill the remaining budget with trigrams.
            _,_,_,records=lmbin.read_arpa(arpa)
            b=len(records[2]); cap=max(0,(148_000_000-16*b-5_000_000)//8); del records
            lmbin.write(arpa,binary,max_trigrams=cap)
        m=lmbin.Model(binary)
        dev={name:evaluate(m,data) for name,data in sets.items() if name.startswith('dev_')}
        loss=-sum(x['log_likelihoods'][2] for x in dev.values())/sum(x['predictions'] for x in dev.values())
        result={'colloquial_weight':weight,'dev_union_perplexity':math.exp(loss),'dev':dev,'seconds':time.time()-started,'bytes':binary.stat().st_size}
        candidates.append(result); pipeline.save_json(marker,result); pipeline.log(str(result))
    best=min(candidates,key=lambda x:x['dev_union_perplexity']); weight=best['colloquial_weight']
    shutil.copyfile(work/f'mix_k{weight}.bin',out/'lm.bin'); m=lmbin.Model(out/'lm.bin')
    metrics={name:evaluate(m,data) for name,data in sets.items()}
    verification=lmbin.verify(out/'lm.bin',256)
    if verification['bytes']>150_000_000: raise ValueError('model exceeds 150 MB')
    common=[w for w in old if w in m.ids and counts[STATE['ids'][w]]>0]
    shift=float(np.median([float(m.uni[m.ids[w]])-math.log(old[w]) for w in common]))
    with (out/'lexicon.tsv').open('w') as f:
        f.write(header+' Mixed corpus recalibration; see NOTICE.md.\n')
        for r,w,p in rows:
            # Unseen entries retain seed scale; lm.json calibrates their engine fallback.
            value=float(m.uni[m.ids[w]])+p-math.log(old[w]) if w in m.ids and counts[STATE['ids'][w]]>0 else p
            f.write(f'{r}\t{w}\t{value:.12f}\n')
    sources=json.loads((root/'data/build/lm/stats.json').read_text())['sources']
    sources += [json.loads((root/'data/build/corpus/splits.json').read_text())['source'],
                {'name':'Qwen3-8B synthetic Taiwan Mandarin','checkpoint':'Qwen/Qwen3-8B','revision':'b968826d9c46dd6066d109eabc6255188de91218','weights_license':'Apache-2.0','license_url':'https://huggingface.co/Qwen/Qwen3-8B/blob/b968826d9c46dd6066d109eabc6255188de91218/LICENSE'}]
    metadata={'format':'ZTLM/1','order':3,'unknown_word_shift':shift,'sources':sources,'license':'CC BY-SA 4.0','created':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sha256':verification['sha256']}
    pipeline.save_json(out/'lm.json',metadata)
    shutil.copyfile(root/'data/build/lm/articles.tsv.gz',out/'articles.tsv.gz')
    notice=(root/'data/build/lm/NOTICE.md').read_text()+'''\n\n## Mixed-corpus trigram additions\n\nCommon Voice zh-TW sentence-collector texts, by Common Voice contributors: CC0-1.0.\nPinned source and policy URLs and hashes are in corpus/splits.json and lm.json.\nhttps://creativecommons.org/publicdomain/zero/1.0/\n\nSynthetic text: Qwen/Qwen3-8B, Alibaba Cloud/Qwen contributors, exact revision\nb968826d9c46dd6066d109eabc6255188de91218. Weights licensed Apache-2.0;\nthe actual LICENSE was verified and is retained in Qwen3-8B-LICENSE.\nNo output-use restriction is stated there. Generated text was filtered, deduplicated,\nand excluded against held-out substrings. No scraped social posts or subtitles.\n\nNew lexicon entries use CC-CEDICT Taiwan/unique readings or uniquely read seed\ncharacters. The mixed model and recalibrated lexicon are distributed CC BY-SA 4.0.\nKenLM (LGPL estimator tool, not linked into the app) supplies modified Kneser-Ney.\nChinese Wikipedia article attribution is retained in articles.tsv.gz.\n'''
    (out/'NOTICE.md').write_text(notice)
    shutil.copyfile(root/'data/build/corpus/raw/Qwen3-8B-LICENSE',out/'Qwen3-8B-LICENSE')
    stats={'status':'validated','corpus':info,'candidates':candidates,'selected_colloquial_weight':weight,'evaluation':metrics,
           'binary':verification,'unknown_word_shift':shift,'segmentation':timings,'timings':{'pipeline_seconds':time.time()-start},
           'mining':json.loads((out/'mining.json').read_text()),'sources':sources,'config':config,
           'runtime':{'python':platform.python_version(),'packages':{name:importlib.metadata.version(name) for name in ['numpy','opencc','mwparserfromhell','pyahocorasick']}},
           'decisions':['One shared frozen tokenizer: lexicon Viterbi plus two hard-EM updates on unweighted mixed training text',
           'Choose k by token-weighted perplexity on union of both dev sets; test sets never select hyperparameters',
           'KenLM MKN with count pruning 0/8/12, then global explicit trigram probability cap to fit 150 decimal MB; recompute backoffs',
           'Map 90 percent of training sentence 臺 tokens to corresponding lexicon 台 tokens before MKN; preserve original held-out spellings',
           'Whitespace and ASCII fallback tokens incompatible with reserved sorted ids map to Unicode replacement token',
           'Common Voice is read speech, not a verified natural-chat benchmark; conversion accuracy awaits Swift integration']}
    stats['files']={p.name:{'bytes':p.stat().st_size,'sha256':pipeline.digest(p)} for p in out.iterdir() if p.is_file() and p.name!='stats.json'}
    pipeline.save_json(out/'stats.json',stats); pipeline.log('validated mixed trigram model')

if __name__=='__main__':
    p=argparse.ArgumentParser(); p.add_argument('--root',default='.'); p.add_argument('--synthetic',help='frozen synthetic input path, relative to root; skips PHASE1_READY'); p.add_argument('--workers',type=int,default=48); p.add_argument('--weights',nargs='+',type=int,default=[1,4,12]); p.add_argument('--prepare-wiki',action='store_true'); main(p.parse_args())

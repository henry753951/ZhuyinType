"""A1: additive McBopomofo lexicon and posterior-count word bigrams (remote only)."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
import multiprocessing as mp
from pathlib import Path
import re
import time
import numpy as np
from opencc import OpenCC
from common import ROOT, OUT, lm, chat, save, digest, remote_only


def mcbopomofo(path):
    rows = {}
    rejected = Counter()
    for line in path.read_text().splitlines():
        if line.startswith('#'): continue
        if line.startswith('_'):
            rejected['control_or_punctuation'] += 1
            continue
        fields = line.split()
        if len(fields) != 3:
            raise ValueError(f'malformed McBopomofo row: {line}')
        reading, word, score = fields
        if not re.fullmatch(r'[\u3400-\u9fff\uf900-\ufaff]+', word):
            rejected['non_han'] += 1
            continue
        reading = reading.replace('-', ' ')
        if len(reading.split()) != len(word) or not re.fullmatch(r'[\u3105-\u3129ˊˇˋ˙ ]+',reading):
            rejected['invalid_reading'] += 1
            continue
        p = float(score)*math.log(10)
        if not math.isfinite(p): raise ValueError('nonfinite source score')
        rows[reading,word] = max(rows.get((reading,word),-math.inf),p)
    return rows, rejected


def merge(seed, extra):
    best = {(r,w):p for r,w,p in seed}
    tops = defaultdict(lambda:-math.inf)
    source_tops = defaultdict(lambda:-math.inf)
    for (_,w),p in best.items(): tops[w] = max(tops[w],p)
    for (_,w),p in extra.items(): source_tops[w] = max(source_tops[w],p)
    offsets = defaultdict(list)
    for w in tops.keys() & source_tops.keys():
        offsets[min(len(w),5)].append(tops[w]-source_tops[w])
    global_offset = float(np.median([x for v in offsets.values() for x in v]))
    calibration = {n:float(np.median(v)) for n,v in offsets.items()}
    added = Counter()
    for (r,w),p in sorted(extra.items()):
        if (r,w) in best: continue
        if w in tops:
            # New readings cannot change any existing reading's relative prior.
            best[r,w] = tops[w] + min(-1.,p-source_tops[w])
            added['new_readings_existing_words'] += 1
        else:
            best[r,w] = p + calibration.get(min(len(w),5),global_offset)
            added['new_word_rows'] += 1
    # The existing builder's phonological rule is reused only for newly added rows.
    spec = __import__('common').load('base_lexicon_habits','training/eval/error_analysis.py')
    seed_keys = {(r,w) for r,w,_ in seed}
    for (r,w),p in list(best.items()):
        if (r,w) in seed_keys: continue
        if len(w)>1 and ('一' in w or '不' in w):
            changed = spec.sandhi(w,r.split())
            for variant in [' '.join(changed)] if changed else []:
                if (variant,w) not in best:
                    best[variant,w]=p-.01
                    added['sandhi_rows']+=1
    assert all(best[r,w] == p for r,w,p in seed)
    return best, dict(added, offsets_by_length=calibration, shared_words=sum(map(len,offsets.values())))


def lexicon(a):
    _, seed, _ = lm.read_lexicon(OUT/'inputs/lexicon.tsv')
    extra, rejected = mcbopomofo(a.source)
    best, report = merge(seed,extra)
    target = OUT/'lexicon_mcbopomofo.tsv'
    with target.open('w') as f:
        f.write('# ZhuyinType + McBopomofo; CC BY-SA 4.0 and retained MIT/BSD notices. See base/NOTICE.md.\n')
        for (r,w),p in sorted(best.items(),key=lambda x:(x[0][0],-x[1],x[0][1])):
            f.write(f'{r}\t{w}\t{p:.12f}\n')
    save(OUT/'lexicon-merge.json',dict(report,source_rows=len(extra),rejected=dict(rejected),
         seed_rows=len(seed),merged_rows=len(best),source_sha256=digest(a.source),
         seed_sha256=digest(OUT/'inputs/lexicon.tsv'),output_sha256=digest(target)))
    print(json.dumps(report),flush=True)


def logadd(a,b):
    if a == -math.inf: return b
    if b == -math.inf: return a
    m=max(a,b)
    return m+math.log1p(math.exp(-abs(a-b)))


def lattice(text,trie,ids,scores):
    edges=[[] for _ in text]
    for i,ch in enumerate(text):
        token=ids.get(ch,2)
        edges[i].append((i+1,token,float(scores[token])))
        node=trie
        for j in range(i,len(text)):
            node=node.get(text[j])
            if node is None: break
            token=node.get(None)
            if token is not None and j>i:
                edges[i].append((j+1,token,float(scores[token])))
    return edges


def expected(text,trie,ids,scores):
    """Exact unigram-lattice posterior; include BOS/EOS and adjacent-edge joints."""
    edges=lattice(text,trie,ids,scores)
    n=len(text)
    forward=[-math.inf]*(n+1);forward[0]=0.
    for i,group in enumerate(edges):
        for j,token,p in group:
            forward[j]=logadd(forward[j],forward[i]+p)
    backward=[-math.inf]*(n+1);backward[n]=0.
    for i in range(n-1,-1,-1):
        for j,token,p in edges[i]:
            backward[i]=logadd(backward[i],p+backward[j])
    z=forward[n]
    uni=Counter();pairs=Counter()
    if not n:
        pairs[1]=1.
        return uni,pairs
    for i,group in enumerate(edges):
        for j,token,p in group:
            probability=math.exp(forward[i]+p+backward[j]-z)
            uni[token]+=probability
            if i==0: pairs[token]+=probability
            if j==n: pairs[(token<<32)|1]+=probability
            else:
                for k,next_token,q in edges[j]:
                    pairs[(token<<32)|next_token]+=math.exp(forward[i]+p+q+backward[k]-z)
    return uni,pairs


def heldouts(source):
    paths=[source/'data/build/lm/test_sentences.txt',ROOT/'training/eval/colloquial_smoke.txt']
    for directory,patterns in [('corpus',('dev*.txt','test*.txt')),('lm_chat',('*_chat_tt.txt',)),
                               ('lm_chat2',('*_dialogue.txt',)),('homophone',('dev_*','test_*','smoke.txt'))]:
        for pattern in patterns: paths.extend((source/'data/build'/directory).glob(pattern))
    paths.extend((ROOT/'training/eval/sets').rglob('*.txt'))
    paths.extend((ROOT/'training/eval/sets').rglob('*.jsonl'))
    paths=sorted(set(p for p in paths if p.is_file() and p.suffix in ('.txt','.jsonl')))
    texts=set()
    def strings(v):
        if isinstance(v,str): yield v
        elif isinstance(v,dict):
            for x in v.values(): yield from strings(x)
        elif isinstance(v,list):
            for x in v: yield from strings(x)
    for path in paths:
        for line in path.read_text().splitlines():
            texts.update(strings(json.loads(line)) if path.suffix=='.jsonl' else line.split('\t'))
    converter=OpenCC('s2twp')
    texts.update(converter.convert(s).replace('臺','台') for s in list(texts))
    texts.update(s.replace('臺','台') for s in list(texts))
    return paths,texts,lm.automaton(texts)


def verify_corpus(a):
    directory=OUT/'corpus'
    if (directory/'validation.json').exists(): return
    paths,held,machine=heldouts(a.source_root)
    assert json.loads((directory/'holdouts.json').read_text())=={str(p):digest(p) for p in paths}
    counts=Counter()
    for path in sorted(directory.glob('*.txt')):
        for line in path.open():
            text=line.rstrip('\n')
            for value in (text,text.replace('臺','台')):
                assert value not in held and next(machine.iter(value),None) is None, path
            counts[path.name.split('-')[0]]+=1
    prepared=json.loads((directory/'prepared.json').read_text())
    assert all(counts[name]==prepared[name+'_kept'] for name in ('wiki','real','generated'))
    assert prepared['files']=={p.name:digest(p) for p in directory.glob('*.txt')}
    save(directory/'validation.json',{'rescanned':dict(counts),'exact_or_eight_character_overlap':0,
                                     'prepared_sha256':digest(directory/'prepared.json')})
    print('CORPUS VALIDATED',dict(counts),flush=True)


def prepare(a):
    directory=OUT/'corpus'
    if directory.exists(): raise ValueError('corpus already exists; use frozen prepared data')
    directory.mkdir()
    paths,held,machine=heldouts(a.source_root)
    save(directory/'holdouts.json',{str(p):digest(p) for p in paths})
    # Source files remain read-only; freshly rescan all of them against every current holdout.
    source=a.source_root
    wiki=sorted((source/'work/sentences').glob('*.txt'))
    real=sorted((source/'data/build/lm_chat2/no_generated/work/sentences').glob('*.txt'))
    generated=[source/'data/build/v3/colloquial.txt']
    if not wiki or not real: raise ValueError('missing existing Wikipedia/chat2 training shards')
    counts=Counter();chars=set();seen=set()
    def blocked(s):
        return any(t in held or next(machine.iter(t),None) is not None for t in (s,s.replace('臺','台')))
    def write_source(name,files,extra=()):
        handles=[(directory/f'{name}-{i:02}.txt').open('w') for i in range(a.workers)]
        def add(s):
            s=chat.normalized(s)
            counts[name+'_input']+=1
            if not s or blocked(s):counts[name+'_excluded']+=1;return
            key=hashlib.blake2b(s.encode(),digest_size=16).digest()
            if name!='wiki' and key in seen:counts[name+'_duplicate']+=1;return
            if name!='wiki':seen.add(key)
            handles[counts[name+'_kept']%len(handles)].write(s+'\n')
            counts[name+'_kept']+=1
            chars.update(s)
        try:
            for path in files:
                with path.open() as f:
                    for line in f:add(line)
            for line in extra:add(line)
        finally:
            for f in handles:f.close()
        print(name,dict(counts),flush=True)
    # Prepared Wikipedia was globally deduplicated by its original pipeline.
    write_source('wiki',wiki)
    tt,receipt=chat.read_tt(source/'data/build/lm_chat/inputs/train.tsv')
    converter=OpenCC('s2twp')
    write_source('real',real,(converter.convert(s) for s in sorted(tt)))
    write_source('generated',generated)
    save(directory/'prepared.json',dict(counts,training_characters=sorted(chars),
         tt_train=receipt,inputs={str(p):digest(p) for p in wiki+real+generated+
                                [source/'data/build/lm_chat/inputs/train.tsv']},
         files={p.name:digest(p) for p in directory.glob('*.txt')}))


def shard(task):
    path,directory,method=task
    uni=np.zeros(len(IDS));pairs=Counter();sentences=0
    for line in path.open():
        text=line.rstrip('\n')
        if method=='expected':u,b=expected(text,TRIE,IDS,SCORES)
        else:
            tokens=lm.segment(text,TRIE,IDS,SCORES,float(SCORES[2]))
            u=Counter(tokens);seq=[0,*tokens,1]
            b=Counter((x<<32)|y for x,y in zip(seq,seq[1:]))
        for token,count in u.items():uni[token]+=count
        pairs.update(b);sentences+=1
    keys=np.array(sorted(pairs),dtype=np.uint64)
    target=directory/path.stem
    np.save(str(target)+'.uni.npy',uni)
    np.save(str(target)+'.keys.npy',keys)
    np.save(str(target)+'.counts.npy',np.array([pairs[int(k)] for k in keys]))
    return {'sentences':sentences,'pair_types':len(keys)}


def fit(a):
    global IDS,TRIE,SCORES
    while True:
        available=int(re.search(r'MemAvailable:\s+(\d+)',Path('/proc/meminfo').read_text())[1])*1024
        if available >= 32*1024**3: break
        print(f'Waiting for host RAM: {available/1024**3:.1f} GiB available; require 32 GiB',flush=True)
        time.sleep(10)
    started=time.monotonic()
    verify_corpus(a)
    output=OUT/'models'/a.name
    if output.exists(): raise ValueError('model output exists; refuse stale or partial retraining')
    output.mkdir(parents=True)
    work=output/'work';work.mkdir()
    prepared=json.loads((OUT/'corpus/prepared.json').read_text())
    _,_,old=lm.read_lexicon(a.lexicon)
    words=['<s>','</s>','<unk>']+sorted(set(old)|set(prepared['training_characters']))
    IDS={w:i for i,w in enumerate(words)};TRIE=lm.make_trie(words,old)
    # Freeze the incumbent final-E segmentation distribution to isolate posterior vs hard counts.
    tokenizer=a.source_root/('work' if a.domain=='wiki' else 'data/build/lm_chat/work')
    prior_words=json.loads((tokenizer/'vocabulary.json').read_text())
    prior_scores=np.load(tokenizer/'segmentation_scores.npy')
    prior=dict(zip(prior_words,map(float,prior_scores)))
    _,_,seed=lm.read_lexicon(OUT/'inputs/lexicon.tsv')
    shift=float(np.median([prior[w]-math.log(p) for w,p in seed.items() if w in prior]))
    SCORES=np.array([prior.get(w,math.log(old.get(w,1e-10))+shift) for w in words])
    tasks=[]
    sources=['wiki'] if a.domain=='wiki' else ['real']+(['generated'] if a.generated_weight else [])
    for source in sources:
        tasks.extend((p,work,a.method) for p in sorted((OUT/'corpus').glob(source+'-*.txt')))
    with mp.get_context('fork').Pool(a.workers) as pool:
        for result in pool.imap_unordered(shard,tasks): print(a.name,result,flush=True)
    keys=[];counts=[];uni=np.zeros(len(words))
    for path,_,_ in tasks:
        weight=a.generated_weight if path.name.startswith('generated') else 1.
        stem=work/path.stem
        uni+=weight*np.load(str(stem)+'.uni.npy')
        keys.append(np.load(str(stem)+'.keys.npy'))
        counts.append(weight*np.load(str(stem)+'.counts.npy'))
    keys=np.concatenate(keys);counts=np.concatenate(counts)
    order=np.argsort(keys);keys=keys[order];counts=counts[order]
    starts=np.r_[0,np.flatnonzero(keys[1:]!=keys[:-1])+1]
    keys=keys[starts];counts=np.add.reduceat(counts,starts)
    np.save(work/'expected_keys.npy',keys);np.save(work/'expected_counts.npy',counts)
    # Existing MKN uses integer count-of-count bins; round once AFTER all posterior/weight sums.
    rounded=np.floor(counts+.5).astype(np.uint64);keep=rounded>0
    model=lm.fit_mkn(keys[keep],rounded[keep],len(words),3_000_000 if a.domain=='wiki' else 1_600_000)
    chat.export(output,words,model)
    validation=chat.Model(output).verify()
    save(output/'validation.json',validation)
    save(output/'stats.json',{'name':a.name,'domain':a.domain,'method':a.method,
         'generated_weight':a.generated_weight,'seed_lexicon_sha256':digest(a.lexicon),
         'posterior_bigram_mass':float(counts.sum()),'posterior_word_mass':float(uni.sum()),
         'posterior_pair_types':len(keys),'rounded_pair_types':int(keep.sum()),'model':model[4],
         'tokenizer_scores_sha256':digest(tokenizer/'segmentation_scores.npy'),
         'tokenizer_vocabulary_sha256':digest(tokenizer/'vocabulary.json'),
         'code':{p.name:digest(p) for p in Path(__file__).parent.glob('*.py')},
         'seconds':time.monotonic()-started,
         'files':{p.name:digest(p) for p in output.glob('*.tsv')},'validation':validation})
    print('VALIDATED',a.name,time.monotonic()-started,flush=True)


if __name__=='__main__':
    remote_only()
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase',choices=['lexicon','prepare','fit'])
    p.add_argument('--source',type=Path,default=ROOT/'references/shanjie/data/lexicon/mcbpmf-data.txt')
    p.add_argument('--source-root',type=Path,required=True)
    p.add_argument('--workers',type=int,default=8)
    p.add_argument('--name',default='wiki_expected')
    p.add_argument('--domain',choices=['wiki','chat'],default='wiki')
    p.add_argument('--method',choices=['hard','expected'],default='expected')
    p.add_argument('--generated-weight',type=float,default=0.)
    p.add_argument('--lexicon',type=Path,default=OUT/'lexicon_mcbopomofo.tsv')
    a=p.parse_args()
    if a.workers<1 or not 0<=a.generated_weight<=1:p.error('invalid workers/weight')
    {'lexicon':lexicon,'prepare':prepare,'fit':fit}[a.phase](a)

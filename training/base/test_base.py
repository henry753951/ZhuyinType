"""Small independent checks; run on the remote host, never the developer Mac."""
from collections import Counter
import math
import tempfile
from pathlib import Path
import numpy as np
from common import lm, remote_only
from train import expected, lattice, merge, mcbopomofo
from evaluate import distance, paired
from prior import distribution
from fallback import correction
import common


def check_posterior():
    words=['<s>','</s>','<unk>','好','吧','好吧','啊']
    ids={w:i for i,w in enumerate(words)}
    scores=np.log([.01,.01,.01,.3,.2,.25,.22])
    trie=lm.make_trie(words,set(words[3:]))
    for text in ('好吧','好吧啊','好好吧','外好吧',''):
        edges=lattice(text,trie,ids,scores)
        paths=[]
        def walk(i,tokens,p):
            if i==len(text):paths.append((tokens,p));return
            for j,token,score in edges[i]:walk(j,tokens+[token],p*math.exp(score))
        walk(0,[],1.)
        z=sum(p for _,p in paths)
        uni=Counter();bi=Counter()
        for tokens,p in paths:
            for token in tokens:uni[token]+=p/z
            seq=[0,*tokens,1]
            for a,b in zip(seq,seq[1:]):bi[(a<<32)|b]+=p/z
        got_u,got_b=expected(text,trie,ids,scores)
        assert uni.keys()==got_u.keys() and bi.keys()==got_b.keys()
        assert all(abs(p-got_u[k])<1e-12 for k,p in uni.items())
        assert all(abs(p-got_b[k])<1e-12 for k,p in bi.items())
        assert abs(sum(got_b.values())-sum(got_u.values())-1)<1e-12
    u,b=expected('好吧',trie,ids,scores)
    assert abs(b[(ids['好']<<32)|ids['吧']]-(.3*.2)/(.25+.3*.2))<1e-12


def check_merge():
    seed=[('ㄧ','一',-2.),('ㄧˊ','一',-4.5),('ㄅㄨˋ','不',-2.),('ㄅㄨˊ','不',-4.5),
          ('ㄏㄠˇ','好',-3.),('ㄏㄠˋ','好',-8.),('ㄧ ㄉㄧㄥˋ','一定',-5.)]
    extra={('ㄏㄠˇ','好'):-1.,('ㄏㄠ','好'):-2.,('ㄅㄨˋ ㄧㄠˋ','不要'):-6.,
           ('ㄅㄨˊ ㄧㄠˋ','不要'):-6.1,('ㄉㄧˋ ㄧ ㄊㄞ','第一胎'):-9.}
    result,report=merge(seed,extra)
    assert all(result[r,w]==p for r,w,p in seed)
    assert result['ㄏㄠ','好']<=-4.
    assert ('ㄅㄨˊ ㄧㄠˋ','不要') in result
    assert ('ㄉㄧˋ ㄧˋ ㄊㄞ','第一胎') not in result
    with tempfile.TemporaryDirectory() as temp:
        p=Path(temp)/'source'
        p.write_text('# comment\nㄏㄠˇ-ㄅㄚ˙ 好吧 -2.0\n_punctuation_! ！ 0.0\n_punctuation_list 　 0.0\nㄐㄧˊ 𠮷 -5.0\n')
        rows,rejected=mcbopomofo(p)
        assert rows=={('ㄏㄠˇ ㄅㄚ˙','好吧'):-2*math.log(10),('ㄐㄧˊ','𠮷'):-5*math.log(10)}
        assert rejected['control_or_punctuation']==2


def check_frozen_guard():
    original=common.OUT
    try:
        with tempfile.TemporaryDirectory() as temp:
            common.OUT=Path(temp)
            data=common.OUT/'model.tsv';data.write_text('original')
            try:common.verify_frozen()
            except ValueError:pass
            else:raise AssertionError('missing frozen manifest accepted')
            common.save(common.OUT/'frozen-inputs.json',{str(data):common.digest(data)})
            common.verify_frozen()
            for missing in (False,True):
                if missing:data.unlink()
                else:data.write_text('changed')
                try:common.verify_frozen()
                except ValueError:pass
                else:raise AssertionError('changed/missing frozen model accepted')
    finally:common.OUT=original


if __name__=='__main__':
    remote_only()
    check_posterior();check_merge()
    check_frozen_guard()
    assert distance('台灣','臺灣')==1 and distance('好','好啊')==1 and distance('好啊','好')==1
    assert paired({'cases':[{'gold':'臺灣'}],'outputs':['台灣']})[0]['ok_variants']==1
    p=distribution({'好':-1000.,'吧':-1001.},.1)
    assert abs(sum(math.exp(v) for v in p.values())-1)<1e-12
    assert abs((p['好']-p['吧'])-1)<1e-12
    # Original conditional: stored A=.5, backed-off B=.2, deleted C=.3.
    mass=.2;alpha=1.5;scale=math.exp(correction(alpha,mass))
    restricted_alpha=alpha*(1-mass)*scale
    assert abs(.5*scale+restricted_alpha*(.2/alpha)/(1-mass)-1)<1e-12
    assert correction(1.,0.)==0.
    print('base posterior/enumeration, boundaries, unknowns, reading/sandhi preservation and parser checks passed')

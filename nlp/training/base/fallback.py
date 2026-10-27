"""Ablation: leave unused lexicon words to the existing decoder fallback."""
import argparse
import json
import math
from pathlib import Path
import shutil
from common import OUT, lm, chat, save, digest, remote_only


def correction(alpha, mass):
    if not 0 <= mass < 1 or alpha < 0 or alpha * mass >= 1:
        raise ValueError('invalid restricted probability mass')
    return -math.log1p(-alpha * mass)


def restrict(source, output, lexical):
    receipt=json.loads((source/'stats.json').read_text())
    assert all(digest(source/name)==sha for name,sha in receipt['files'].items())
    uni={w:float(p) for w,p in (s.rstrip('\n').split('\t') for s in (source/'unigram.tsv').open())}
    back={w:float(p) for w,p in (s.rstrip('\n').split('\t') for s in (source/'backoff.tsv').open())}
    used=set(back)
    with (source/'bigram.tsv').open() as f:
        for s in f:
            a,b,_=s.rstrip('\n').split('\t');used.update((a,b))
    floor=min(uni.values())
    removed={w for w,p in uni.items() if abs(p-floor)<1e-10 and w in lexical and w not in used}
    mass=math.fsum(math.exp(uni[w]) for w in removed)
    if not removed: raise ValueError('no unused lexical floor words')
    shift=correction(1.,mass)
    adjustments={w:correction(math.exp(p),mass) for w,p in back.items()}
    shutil.copytree(source,output,ignore=shutil.ignore_patterns('work','*.tsv','stats.json','validation.json'))
    with (output/'unigram.tsv').open('w') as f:
        for w,p in uni.items():
            if w not in removed:f.write(f'{w}\t{p+shift:.12f}\n')
    with (source/'bigram.tsv').open() as src,(output/'bigram.tsv').open('w') as dst:
        for s in src:
            a,b,p=s.rstrip('\n').split('\t')
            dst.write(f'{a}\t{b}\t{float(p)+adjustments.get(a,shift):.12f}\n')
    with (output/'backoff.tsv').open('w') as f:
        for w,p in back.items():f.write(f'{w}\t{p-shift+adjustments[w]:.12f}\n')
    validation=chat.Model(output).verify()
    save(output/'validation.json',validation)
    save(output/'stats.json',{'parent':source.name,'parent_stats_sha256':digest(source/'stats.json'),
         'removed_unused_lexical_words':len(removed),'removed_probability_mass':mass,
         'remaining_unigram_types':len(uni)-len(removed),
         'files':{p.name:digest(p) for p in output.glob('*.tsv')},'validation':validation})
    (output/'removed-words.txt').write_text('\n'.join(sorted(removed))+'\n')
    with (output/'NOTICE.md').open('a') as f:
        f.write('\nA1 unused-vocabulary ablation: remove lexical floor words absent from all stored '
                'bigram/backoff references, restrict and renormalize every conditional distribution. '
                'The engine retains its existing lexicon-frequency fallback for those words. '
                'No engine or individual-word exception is introduced.\n')
    print('VALIDATED',output.name,len(removed),mass,flush=True)


if __name__=='__main__':
    remote_only()
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    lexical=set(lm.read_lexicon(OUT/'lexicon_source.tsv')[2])
    restrict(a.source,a.output,lexical)

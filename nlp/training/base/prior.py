"""Data-only control: McBopomofo word frequencies as the chat-side unigram prior."""
from collections import defaultdict
import math
import numpy as np
from common import ROOT, OUT, lm, chat, save, digest, remote_only
from train import mcbopomofo


def distribution(scores,eos):
    unk=1e-9
    if not scores or not 0<eos<1-unk: raise ValueError('invalid frequency distribution')
    peak=max(scores.values())
    normalizer=peak+math.log(math.fsum(math.exp(p-peak) for p in scores.values()))
    shift=math.log1p(-eos-unk)-normalizer
    return {'</s>':math.log(eos),'<unk>':math.log(unk),**{w:p+shift for w,p in scores.items()}}


def main():
    remote_only()
    source=OUT/'inputs/mcbpmf-data.txt'
    extra,_=mcbopomofo(source)
    native=defaultdict(lambda:-math.inf)
    old=defaultdict(lambda:-math.inf)
    for (_,w),p in extra.items():native[w]=max(native[w],p)
    for r,w,p in lm.read_lexicon(OUT/'inputs/lexicon.tsv')[1]:old[w]=max(old[w],p)
    offsets=defaultdict(list)
    for w in old.keys() & native.keys():offsets[min(len(w),5)].append(old[w]-native[w])
    calibration={n:float(np.median(v)) for n,v in offsets.items()}
    fallback=float(np.median([x for v in offsets.values() for x in v]))
    scores={w:native[w] if w in native else p-calibration.get(min(len(w),5),fallback)
            for w,p in {**old,**native}.items()}
    wiki_uni={w:float(p) for w,p in (line.rstrip('\n').split('\t') for line in (OUT/'inputs/baseline/lm/unigram.tsv').open())}
    probabilities=distribution(scores,math.exp(wiki_uni['</s>']))
    output=OUT/'models/native_prior'
    output.mkdir(parents=True,exist_ok=False)
    with (output/'unigram.tsv').open('w') as f:
        for w,p in sorted(probabilities.items()):f.write(f'{w}\t{p:.12f}\n')
    for name in ('bigram','backoff'):(output/f'{name}.tsv').write_text('')
    validation=chat.Model(output).verify()
    save(output/'validation.json',validation)
    save(output/'stats.json',{'native_words':len(native),'fallback_words':len(scores)-len(native),
         'eos_probability':math.exp(wiki_uni['</s>']),'offsets_by_length':calibration,
         'source_sha256':digest(source),'seed_sha256':digest(OUT/'inputs/lexicon.tsv'),
         'files':{p.name:digest(p) for p in output.glob('*.tsv')},'validation':validation})
    (output/'NOTICE.md').write_text('McBopomofo frequency-prior control. McBopomofo MIT/libtabe BSD; '
        'CC-CEDICT/wordfreq CC BY-SA 4.0; Unicode Unihan Unicode License v3. '
        'EOS mass comes from the incumbent Wikipedia unigram (CC BY-SA 4.0). '
        'No chat training text is counted. Retain the base bundle NOTICE.md and licenses/. '
        'Changes: word-frequency calibration, vocabulary union and normalization; no stored bigrams.\n')
    print('VALIDATED native_prior',len(scores),flush=True)


if __name__=='__main__':main()

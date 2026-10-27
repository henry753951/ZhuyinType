"""Check one conservative half-update after the two known smoke regressions."""
import json
from pathlib import Path
import shutil
import sys
import torch

from common import BASE, ROOT, digest, save
from train_v3 import homophone_cases, score, v3


def main():
    assert sys.platform=='linux'
    torch.set_num_threads(4)
    source=ROOT/'data/build/v3_homophone2'
    target=ROOT/'data/build/v3_homophone2_stable'
    run=ROOT/'data/build/homophone2_stable'
    assert not target.exists() and not run.exists()
    base=torch.load(BASE/'syllable.pt',map_location='cpu',weights_only=False)
    tuned=torch.load(source/'syllable.pt',map_location='cpu',weights_only=False)
    selection=json.loads((ROOT/'data/build/homophone2/v3-training.json').read_text())
    selected=next(r for r in selection['history'] if r['step']==selection['selected_step'])
    assert tuned['step']==selected['step'] and tuned.get('metrics')==selected['metrics']
    assert base['vocab']==tuned['vocab'] and base['syllables']==tuned['syllables']
    for name in ['context','syllable']:
        assert base[name].keys()==tuned[name].keys()
        for key,value in base[name].items():
            assert value.shape==tuned[name][key].shape
            if name=='context': assert torch.equal(value,tuned[name][key]), key
    state={}
    for key,value in base['syllable'].items():
        if torch.is_floating_point(value): state[key]=torch.lerp(value,tuned['syllable'][key],.5)
        else:
            assert torch.equal(value,tuned['syllable'][key])
            state[key]=value
        assert torch.isfinite(state[key]).all(),key
    target.mkdir();run.mkdir()
    provenance=dict(original=digest(BASE/'syllable.pt'),continued=digest(source/'syllable.pt'),
                    continued_step=tuned['step'],continued_fraction=.5,
                    method='Parameter midpoint, frozen context identical. No app scoring-weight change.')
    torch.save(dict(context=base['context'],syllable=state,vocab=base['vocab'],syllables=base['syllables'],
                    step=tuned['step'],stabilization=provenance),target/'syllable.pt')
    for name in ['context.pt','chars.json','syllables.json','config.json']:
        shutil.copy2(BASE/name,target/name)
    save(target/'stabilization.json',provenance)
    for name in ['inventory.json','dev_zai.txt','dev_zai.jsonl','dev_pairs.txt','dev_pairs.jsonl','test_zai.txt','test_zai.jsonl','test_pairs.txt','test_pairs.jsonl','smoke.txt','inputs.json']:
        shutil.copy2(ROOT/'data/build/homophone2'/name,run/name)
    v3()
    from dataset import Builder
    from evaluate import cases
    from models import load_models
    builder=Builder()
    context,model=load_models(target/'syllable.pt','cuda')
    focused=homophone_cases(builder,run,guards=True)
    general={f'{domain}-{ctx}-{noise}':list(cases(builder,domain,ctx,noise))
             for domain in ['colloquial','wikipedia'] for ctx in [False,True] for noise in [0.,.05]}
    result=score(context,model,builder,focused,general)
    baseline=json.loads((ROOT/'data/build/homophone2/v3-baseline.json').read_text())
    admissible=all(result['general'][k]['accuracy']>=baseline['general'][k]['accuracy']-.002 for k in general)
    save(run/'model-eval.json',dict(metrics=result,admissible=admissible,stabilization=provenance,
                                  checkpoint=digest(target/'syllable.pt')))
    assert admissible, 'Half-update failed the original general-dev gate.'
    print(json.dumps(result,ensure_ascii=False),flush=True)


if __name__=='__main__':
    main()

"""Seal the dev winner only after every required dev row and quality gate passes."""
import json
import sys
from datetime import datetime, timezone

from common import OUT, ROOT, digest, save


def main():
    assert sys.platform=='linux'
    load=lambda path:json.loads(path.read_text())
    assert not (OUT/'choice.json').exists(), 'Choice is immutable.'
    assert len(list((OUT/'results').glob('*.json')))==22, 'Need 14 baseline and eight continuation rows.'
    training=load(OUT/'v3-training.json')
    baseline=load(OUT/'v3-baseline.json')
    selected=next(r for r in training['history'] if r['step']==training['selected_step'])
    assert training['plateau'] and selected['admissible']
    assert all(selected['metrics']['general'][k]['accuracy']>=v['accuracy']-.002 for k,v in baseline['general'].items())
    scores={label:sum(load(OUT/'results'/f'{label}-dev_zai-ctx{c}.json')['positions']['interesting']['accuracy']
                       for c in [0,1])/2 for label in ['v2b-v3','v2b-v3-new']}
    assert scores['v2b-v3-new']>scores['v2b-v3']
    counts={}
    for dataset in ['colloquial','wikipedia']:
        counts[dataset]=[]
        for folder,label in [('general-baseline','v2b-v3'),('general-new','v2b-v3-new')]:
            row=load(OUT/folder/f'{label}-{dataset}.json')['datasets'][0]
            assert len(row['outputs'])==400
            counts[dataset].append(round(row['whole_accuracy_percent']*4))
        assert counts[dataset][1]>=counts[dataset][0]
    diagnosis=load(OUT/'diagnosis-new.json')
    assert not any(r['mostly_reranker'] for r in diagnosis.values()), 'Continue v2b before choosing.'
    choice=dict(models=['reranker','v3_homophone'],v3_weight=1,rerank_count=16,
        selected_step=training['selected_step'],v3_checkpoint_sha256=digest(ROOT/'data/build/v3_homophone/syllable.pt'),
        dev_interesting_mean=scores,general_correct_counts=counts,reranker_continuation=False,
        diagnosis=diagnosis,chosen_utc=datetime.now(timezone.utc).isoformat(),policy=load(OUT/'selection-policy.json'))
    save(OUT/'choice.json',choice)
    print(json.dumps(choice,ensure_ascii=False,indent=2))


if __name__=='__main__':
    main()

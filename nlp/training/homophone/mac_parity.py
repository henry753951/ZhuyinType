"""Single-use Mac CLI parity on 400 saved Linux dev inputs, including critical guards."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import sys

from common import ROOT, digest, save
from ship_eval import OUT, model_args


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('role',choices=['baseline','chosen'])
    p.add_argument('--run-name',choices=['homophone2','homophone2_stable'],default='homophone2')
    a=p.parse_args()
    assert sys.platform=='darwin', 'This is the authorized one-off Core ML parity check.'
    run=ROOT/'data/build'/a.run_name
    reference=run/'results'/f'{a.role}-wikipedia.json'
    record=json.loads(reference.read_text())['datasets'][0]
    baseline=json.loads((run/'results/baseline-wikipedia.json').read_text())['datasets'][0]
    assert record['inputs']==baseline['inputs'] and len(record['inputs'])==len(record['outputs'])==400
    reference_hashes={str(reference.relative_to(ROOT)):digest(reference)}
    if a.role=='chosen':
        # Preserve 397 fixed Wiki runs and cover the three user-supplied failure cases.
        sys.path.insert(0,str(ROOT/'data/build/homophone2_snapshot/nlp/eval'))
        from conversion_eval import load_lexicon,readings_for,keys_for
        smoke_path=run/'results/chosen-smoke-ctx0.json'
        smoke=json.loads(smoke_path.read_text())
        texts=(run/'smoke.txt').read_text().splitlines()
        assert smoke['test_sha256']==digest(run/'smoke.txt') and len(texts)==len(smoke['predictions'])
        lexicon=load_lexicon(OUT/'mac-runtime/lexicon.tsv')
        record={**record,'inputs':record['inputs'][:397],'outputs':record['outputs'][:397]}
        for text in ['再試試看','我再吃一碗','我在吃一碗麵']:
            readings=readings_for(text,lexicon)
            assert readings and len(readings)==len(text)
            for i,c in enumerate(text):
                if c in '在再': readings[i]='ㄗㄞˋ'
            record['inputs'].append(keys_for(readings)+'<enter>')
            record['outputs'].append(smoke['predictions'][texts.index(text)])
        reference_hashes[str(smoke_path.relative_to(ROOT))]=digest(smoke_path)
    else:
        assert a.run_name=='homophone2', 'Reuse the existing original-model control.'
    model='v3' if a.role=='baseline' else 'v3_homophone2'
    if a.role=='chosen':
        expected=json.loads((run/'choice2.json').read_text())['checkpoint']
        assert digest(ROOT/'data/build'/model/'syllable.pt')==expected
        assert json.loads((run/'chosen-dev-complete.json').read_text())['checkpoint']==expected
    command=[str(ROOT/'.build-codex5/release/zhuyin-cli'),'--lexicon',str(OUT/'mac-runtime/lexicon.tsv'),
             '--typo','standard','--batch']+model_args(model)
    command[command.index('--chat-lm')+1]=str(OUT/'mac-runtime/lm_chat')
    seal=run/f'mac-{a.role}-started.json'
    assert not seal.exists(), 'One-off parity already started; inspect saved output instead of rerunning.'
    assert len(record['inputs'])==len(record['outputs'])==400
    provenance=dict(references=reference_hashes,cli=digest(command[0]),
                    checkpoint=digest(ROOT/'data/build'/model/'syllable.pt'),command=command)
    save(seal,provenance)
    env={k:v for k,v in os.environ.items() if k!='ZHUYIN_UNITS'}
    env.update(OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1')
    def part(start):
        scripts=record['inputs'][start:start+100]
        result=subprocess.run(command,input='\n'.join(scripts)+'\n',capture_output=True,text=True,check=True,env=env,cwd=ROOT)
        output=result.stdout.splitlines()
        assert len(output)==len(scripts), result.stderr[-2000:]
        return output
    with ThreadPoolExecutor(4) as pool:
        predictions=[s for batch in pool.map(part,range(0,400,100)) for s in batch]
    mismatches=[dict(row=i+1,keys=keys,linux=linux,mac=mac) for i,(keys,linux,mac) in
                enumerate(zip(record['inputs'],record['outputs'],predictions,strict=True)) if linux!=mac]
    identical=400-len(mismatches)
    save(run/f'mac-{a.role}-parity.json',dict(**provenance,inputs=record['inputs'],outputs=predictions,
         identical=identical,total=400,identical_share=identical/400,mismatches=mismatches))
    print(f'{a.role}: {identical}/400 ({identical/4:.2f}%) identical Core ML / ONNX outputs')


if __name__=='__main__':
    main()

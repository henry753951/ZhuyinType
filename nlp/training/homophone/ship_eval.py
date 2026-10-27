"""Current-app paired evaluation for the compatible continuation; test is single-use."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import os

from common import ROOT, digest, jsonl, save

OUT = ROOT/'data/build/homophone2'


def model_args(model):
    return ['--reranker','data/build/reranker','--rerank-count','16','--rerank-weight','0.5',
            '--v3','data/build/'+model,'--v3-weight','0.5',
            '--chat-lm','data/build/lm_chat','--chat-prior','0.3']


def wait_ram(gib=48):
    while True:
        available = next(int(s.split()[1])/2**20 for s in Path('/proc/meminfo').read_text().splitlines() if s.startswith('MemAvailable:'))
        if available>=gib: return
        print(f'Waiting: {available:.1f} GiB available, {gib} required',flush=True)
        time.sleep(30)


def run(command, output):
    assert not output.exists(), f'Inspect existing result: {output}'
    wait_ram()
    with output.with_suffix('.log').open('x') as log:
        subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
    print('completed',output.name,flush=True)


def result_rows(truths, predictions, inputs):
    sys.path.insert(0,str(ROOT/'nlp/eval'))
    from rows_eval import edit_distance
    from conversion_eval import VARIANTS
    rows=[]
    for i,(truth,pred,keys) in enumerate(zip(truths,predictions,inputs,strict=True),1):
        exact = truth==pred
        variant = truth.translate(VARIANTS)==pred.translate(VARIANTS)
        error = edit_distance(truth,pred)
        rows.append(dict(row=i,id=hashlib.sha256((truth+'\t'+keys).encode()).hexdigest(),gold_len=len(truth),
                         ok_raw=int(exact),errors_raw=0 if exact else error,
                         ok_variants=int(variant),errors_variants=0 if variant else error,
                         truth=truth,prediction=pred,keys=keys))
    return rows


def write_rows(path, rows):
    assert not path.exists()
    path.write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in rows))


def paired():
    sys.path.insert(0,str(ROOT/'nlp/eval'))
    from stats import read_results, compare
    for base in sorted((OUT/'results').glob('baseline-*.jsonl')):
        new = base.with_name(base.name.replace('baseline-','chosen-',1))
        destination = base.with_name(base.name.replace('baseline-','paired-',1)).with_suffix('.json')
        if not new.exists() or destination.exists(): continue
        norms=['raw','variants']
        if 'ok_shanjie' in json.loads(base.open().readline()): norms.append('shanjie')
        clusters=[r['cluster'] for r in jsonl(new)] if 'cluster' in json.loads(new.open().readline()) else None
        stats = {norm:compare(read_results(base,norm),read_results(new,norm),normalization=norm,jobs=4,clusters=clusters)
                 for norm in norms}
        save(destination,stats)


def homophone(cli,jobs,role,model,dataset,context,recover=False):
    target = OUT/'results'/f'{role}-{dataset}-ctx{int(context)}.json'
    # The old evaluator's defaults are overridden with the exact current app arguments.
    command = [sys.executable,str(Path(__file__).with_name('eval.py')),'--cli',str(cli),
               '--jobs',str(jobs),'--test',str(OUT/f'{dataset}.txt'),'--output',str(target)]
    command += ['--cli-arg='+a for a in model_args(model)]
    if context: command += ['--line-context']
    if recover and target.exists():
        cached=json.loads(target.read_text())
        expected=[str(cli),'--lexicon',str(ROOT/'data/build/lexicon.tsv'),'--batch','--typo','standard']+model_args(model)
        assert cached['command']==expected and cached['line_context']==context
        assert cached['test_sha256']==digest(OUT/f'{dataset}.txt') and cached['cli_sha256']==digest(cli)
        assert len(cached['predictions'])==cached['sentences']
    else:
        assert not (recover and target.with_suffix('.log').exists()), 'Unfinished row already started; do not replay sealed inputs.'
        run(command,target)
    record = json.loads(target.read_text())
    from eval import align
    rows = list(jsonl(OUT/f'{dataset}.jsonl')) if dataset!='smoke' else None
    if rows is None:
        # Smoke has no source annotation; whole sentences are sufficient for the paired check.
        rows = [dict(text=s,positions=[]) for s in (OUT/'smoke.txt').read_text().splitlines()]
    whole = result_rows([r['text'] for r in rows],record['predictions'],[r['text'] for r in rows])
    clusters=[f'{r.get("source","row")}:{r.get("line",i)}' for i,r in enumerate(rows)]
    for r,cluster in zip(whole,clusters,strict=True): r['cluster']=cluster
    row_path=target.with_suffix('.jsonl')
    if recover and row_path.exists(): assert list(jsonl(row_path))==whole
    else: write_rows(row_path,whole)
    # Position-level paired metrics, clustered by source clause for confidence intervals.
    sys.path.insert(0,str(ROOT/'nlp/eval'))
    from stats import compare
    selected={}
    for i,(row,pred) in enumerate(zip(rows,record['predictions'],strict=True)):
        mapping=align(row['text'],pred)
        positions=[p for p in row['positions'] if (p['group']=='在再')==(dataset.endswith('_zai'))]
        for p in positions:
            keys=['all','char:'+p['char'],'category:'+p['category'],'group:'+p['group'],
                  'char_category:'+p['char']+':'+p['category']]
            for key in keys:
                selected.setdefault(key,[]).append([f'{i}:{p["index"]}',int(mapping[p['index']]==p['char']),int(mapping[p['index']]!=p['char']),1])
    position_path=target.with_suffix('.positions.json')
    if recover and position_path.exists(): assert json.loads(position_path.read_text())==selected
    else: save(position_path,selected)
    if role=='chosen':
        base=json.loads(target.with_name(target.name.replace('chosen-','baseline-',1)).with_suffix('.positions.json').read_text())
        stats={key:compare(base[key],values,normalization='position',jobs=4,
                    clusters=[clusters[int(r[0].split(':')[0])] for r in values]) for key,values in selected.items()}
        save(target.with_name(target.name.replace('chosen-','paired-',1)).with_suffix('.positions.json'),stats)


def main():
    global OUT
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cli',type=Path,required=True)
    p.add_argument('--jobs',type=int,default=24)
    p.add_argument('--cli-arg',action='append',default=[])
    p.add_argument('--role',choices=['baseline','chosen'],required=True)
    p.add_argument('--phase',choices=['dev','test'],default='dev')
    p.add_argument('--run-name',choices=['homophone2','homophone2b','homophone2_stable'])
    p.add_argument('--recover-test',action='store_true',help='Audit/reuse complete test rows and run only rows with no start log.')
    a=p.parse_args()
    run_name=a.run_name or 'homophone2'
    if a.run_name: OUT=ROOT/'data/build'/run_name
    assert sys.platform=='linux' and 1<=a.jobs<=40
    assert not a.cli_arg, 'Use ONNX CPU for the real-sentence parity reference.'
    model='v3' if a.role=='baseline' else 'v3_'+run_name
    assert OUT.exists()
    (OUT/'results').mkdir(exist_ok=True)
    manifest=dict(model=model,checkpoint=digest(ROOT/'data/build'/model/'syllable.pt'),
                  cli=digest(a.cli),args=model_args(model),phase=a.phase,jobs=a.jobs,
                  inputs={p.name:digest(p) for p in OUT.glob('*.txt')})
    seal=OUT/f'{a.role}-{a.phase}-started.json'
    assert not a.recover_test or a.phase=='test'
    if a.recover_test:
        assert seal.exists() and not (OUT/f'{a.role}-test-complete.json').exists()
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit() or int(proc.name) in (os.getpid(),os.getppid()): continue
            try:
                if (proc/'cwd').resolve()!=ROOT: continue
                argv=(proc/'cmdline').read_bytes().decode(errors='replace').split('\0')
                active=Path(argv[0]).name=='zhuyin-cli' or any(Path(s).name in ('experiment.py','ship_eval.py','eval.py') for s in argv[1:] if s)
                assert not active, f'Owned workspace still has evaluation PID {proc.name}; wait, do not replay.'
            except (OSError,RuntimeError): pass
    else:
        assert not seal.exists(), 'Single-use phase; inspect any previous partial run before proceeding.'
    if a.phase=='test':
        assert all((OUT/name).exists() for name in ('baseline-dev-complete.json','chosen-dev-complete.json','dev-decision.json')), 'Complete and decide the current-app dev comparison before opening test.'
        choice=json.loads((OUT/'choice2.json').read_text())
        assert choice['checkpoint']==digest(ROOT/'data/build'/('v3_'+run_name)/'syllable.pt')
        manifest['choice']=choice
    if a.recover_test:
        assert json.loads(seal.read_text())==manifest, 'Recovery inputs/configuration differ from the sealed attempt.'
        save(OUT/f'{a.role}-test-recovery.json',dict(protocol='Reuse only complete raw predictions; start only rows with no .log marker. No sealed row replay.',manifest=manifest))
    else: save(seal,manifest)
    for dataset in (['dev_zai','dev_pairs','smoke'] if a.phase=='dev' else ['test_zai','test_pairs']):
        for context in ([False] if dataset=='smoke' else [False,True]):
            homophone(a.cli,a.jobs,a.role,model,dataset,context,a.recover_test)
    if a.phase=='dev':
        sets=[('colloquial','data/build/corpus/dev_colloquial.txt',False),
              ('wikipedia','data/build/corpus/dev_wikipedia.txt',True),
              ('chat_tt','data/build/lm_chat/dev_chat_tt.txt',False),
              ('colloquial_smoke','nlp/eval/colloquial_smoke.txt',False)]
        for name,path,context in sets:
            target=OUT/'results'/f'{a.role}-{name}.json'
            command=[sys.executable,str(ROOT/'nlp/remote/capture_eval.py'),'--output',str(target),
                     '--cli',str(a.cli),'--jobs',str(a.jobs),'-n','400','--dump',str(target.with_suffix(''))+'-',
                     '--test',path]+['--cli-arg='+x for x in model_args(model)]
            if context: command += ['--line-context']
            run(command,target)
            record=json.loads(target.read_text())['datasets'][0]
            dump=Path(str(target.with_suffix(''))+'-'+Path(path).name+'.tsv')
            truths=[s.split('\t')[0] for s in dump.read_text().splitlines()]
            write_rows(target.with_suffix('.jsonl'),result_rows(truths,record['outputs'],record['inputs']))
        for name in ('dev302','typing76'):
            target=OUT/'results'/f'{a.role}-{name}.jsonl'
            command=[sys.executable,str(ROOT/'nlp/eval/rows_eval.py'),'--cli',str(a.cli),
                     '--jobs',str(a.jobs),'--rows',f'nlp/eval/sets/external/shanjie/eval/bench/suite-v1/{name}.clean.rows.txt',
                     '--output',str(target)]+['--cli-arg='+x for x in model_args(model)]
            run(command,target)
    paired()
    save(OUT/f'{a.role}-{a.phase}-complete.json',manifest)


if __name__=='__main__':
    main()

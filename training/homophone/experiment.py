"""Remote-only dev comparisons; seal test/smoke until a choice is recorded."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from common import OUT, ROOT, save

BASELINES = {'engine': (None,None), 'v2b': ('reranker',None),
             'v2b-v3': ('reranker','v3'), 'v3': (None,'v3')}
NEW = {'v3-new': (None,'v3_homophone'), 'v2b-v3-new': ('reranker','v3_homophone')}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase',choices=['baseline','new','reranker','final'])
    p.add_argument('--cli', type=Path, required=True)
    p.add_argument('--jobs',type=int,default=60)
    p.add_argument('--cli-arg',action='append',default=[])
    args = p.parse_args()
    assert sys.platform == 'linux', 'All evaluations run remotely.'
    (OUT/'results').mkdir(exist_ok=True)
    if args.phase=='baseline':
        configurations = BASELINES
    elif args.phase=='new':
        configurations = NEW
    elif args.phase=='reranker':
        configurations = {'reranker-new':('reranker_homophone',None),
                          'both-new':('reranker_homophone','v3_homophone')}
    else:
        choice = json.loads((OUT/'choice.json').read_text())
        assert not (OUT/'final-started.json').exists(), 'Final matrix already started; inspect rather than rerun.'
        configurations = {'baseline':BASELINES['v2b-v3'], 'chosen':tuple(choice['models'])}
        save(OUT/'final-started.json',choice)
    datasets = ['test_zai','test_pairs','smoke'] if args.phase=='final' else ['dev_zai','dev_pairs']
    for label,models in configurations.items():
        for dataset in datasets:
            for context in [False,True]:
                # Smoke has no context, so type it once per model.
                if dataset == 'smoke' and context:
                    continue
                stem = f'{label}-{dataset}-ctx{int(context)}'
                destination = OUT/'results'/f'{stem}.json'
                assert not destination.exists(), f'Existing result: {destination}'
                command = [sys.executable,str(Path(__file__).with_name('eval.py')),
                           '--cli',str(args.cli.resolve()),'--jobs',str(args.jobs),
                           '--test',str(OUT/f'{dataset}.txt'),'--output',str(destination)]
                for flag,value in zip(['--reranker','--v3'],models):
                    if value:
                        command += [flag,str(ROOT/'data/build'/value)]
                command += ['--cli-arg='+arg for arg in args.cli_arg]
                if context:
                    command.append('--line-context')
                with (OUT/'results'/f'{stem}.log').open('x') as log:
                    subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
                print('completed',stem,flush=True)
    save(OUT/f'{args.phase}-complete.json',dict(labels=list(configurations),jobs=args.jobs))


if __name__=='__main__':
    main()

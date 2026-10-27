"""Snapshot the shared runner before invoking the isolated compatible-model matrix."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[3]


def main():
    role,phase=sys.argv[1:3]
    run=sys.argv[3] if len(sys.argv)>3 else 'homophone2'
    assert run in ('homophone2','homophone2b','homophone2_stable')
    assert role in ('baseline','chosen') and phase in ('dev','test')
    jobs=os.environ.get('JOBS','24')
    assert 1<=int(jobs)<=40
    env={**os.environ,'REMOTE_DIR':'zhuyintype/work/onnx-eval-homophone3','JOBS':jobs,'BACKEND':'cpu',
         'V3_MODEL':'v3' if role=='baseline' else 'v3_'+run,
         'ENGINE_SNAPSHOT':'data/build/homophone2_snapshot',
         'LOCAL_RESULTS':f'data/build/{run}/remote-{role}-{phase}',
         'EXTRA_SYNC':'nlp/training/homophone nlp/training/v3/common.py nlp/training/v3/readings.py nlp/training/v3/reference.py data/build/homophone/inventory.json '+
                      ' '.join(f'data/build/{run}/{name}' for name in ['dev_zai.txt','dev_zai.jsonl','dev_pairs.txt','dev_pairs.jsonl','test_zai.txt','test_zai.jsonl','test_pairs.txt','test_pairs.jsonl','smoke.txt'])}
    if phase=='test': env['EXTRA_SYNC']+=f' data/build/{run}/choice2.json data/build/{run}/dev-decision.json'
    recovery=['--recover-test'] if os.environ.get('RECOVER_TEST')=='1' else []
    assert not recovery or phase=='test'
    runner=ROOT/'nlp/remote/run.sh'
    with tempfile.TemporaryDirectory(prefix='homophone2-runner-') as directory:
        snapshot=Path(directory)/'run.sh'
        snapshot.write_bytes(runner.read_bytes())
        subprocess.run(['bash','-n',str(snapshot)],check=True)
        # Preserve $0 because run.sh resolves the repository relative to its original path.
        subprocess.run(['bash','-c',snapshot.read_text(),str(runner),'homophone','compatible',
                        '--role',role,'--phase',phase,'--run-name',run,*recovery],cwd=ROOT,env=env,check=True)


if __name__=='__main__':
    main()

"""Invoke the portable runner from an immutable in-memory script during concurrent edits."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[3]
os.chdir(ROOT)
pinned=ROOT/'data/build/homophone/runner-pinned.sh'
source=(pinned if pinned.exists() else ROOT/'nlp/remote/run.sh').read_text()
subprocess.run(['bash','-n'],input=source,text=True,check=True)
snapshot=ROOT/'data/build/homophone'/('runner-'+hashlib.sha256(source.encode()).hexdigest()[:16]+'.sh')
if snapshot.exists():
    assert snapshot.read_text()==source
else:
    snapshot.write_text(source)
raise SystemExit(subprocess.call(['bash','-c',source,'nlp/remote/run.sh',*sys.argv[1:]]))

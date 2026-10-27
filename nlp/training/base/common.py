"""Reuse the project's estimators without changing the shared pipelines."""
import importlib.util
from pathlib import Path
import platform
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'data/build/base'
sys.path.insert(0, str(ROOT / 'nlp/training/lm'))
import pipeline as lm


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


chat = load('base_chat', 'nlp/training/lm_chat/train.py')
ev = load('base_conversion', 'nlp/eval/conversion_eval.py')
stats = load('base_stats', 'nlp/eval/stats.py')
save, digest = lm.save_json, lm.digest


def remote_only():
    if platform.system() != 'Linux':
        raise SystemExit('Run training and evaluation on the GPU host only.')


def verify_frozen():
    import json
    path=OUT/'frozen-inputs.json'
    if not path.is_file(): raise ValueError('missing frozen-inputs.json; freeze inputs before evaluation')
    expected=json.loads(path.read_text())
    if not expected: raise ValueError('empty frozen input manifest')
    changed=[name for name,sha in expected.items() if not Path(name).is_file() or digest(Path(name))!=sha]
    if changed: raise ValueError(f'frozen experiment inputs changed: {changed}')

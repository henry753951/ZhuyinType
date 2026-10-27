"""Reuse the project's estimators without changing the shared pipelines."""
import importlib.util
from pathlib import Path
import platform
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'data/build/base'
sys.path.insert(0, str(ROOT / 'training/lm'))
import pipeline as lm


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


chat = load('base_chat', 'training/lm_chat/train.py')
ev = load('base_conversion', 'training/eval/conversion_eval.py')
stats = load('base_stats', 'training/eval/stats.py')
save, digest = lm.save_json, lm.digest


def remote_only():
    if platform.system() != 'Linux':
        raise SystemExit('Run training and evaluation on the GPU host only.')

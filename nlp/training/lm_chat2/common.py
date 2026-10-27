"""Shared paths and existing estimators; never modify their source directories."""
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'data/build/lm_chat2'
RAW = ROOT / 'data/raw/lm_chat2'
sys.path.insert(0, str(ROOT / 'nlp/training/lm'))
import pipeline as lm

spec = importlib.util.spec_from_file_location('old_chat', ROOT / 'nlp/training/lm_chat/train.py')
chat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(chat)
save = lm.save_json
digest = lm.digest


def evaluation_files():
    frozen = OUT / 'inputs/evaluation'
    if frozen.exists():
        return sorted(p for p in frozen.rglob('*') if p.is_file() and p.suffix in ('.txt', '.jsonl'))
    paths = [ROOT / 'data/build/lm/test_sentences.txt',
             ROOT / 'nlp/eval/colloquial_smoke.txt']
    paths += sorted((ROOT / 'data/build/corpus').glob('dev*.txt'))
    paths += sorted((ROOT / 'data/build/corpus').glob('test*.txt'))
    paths += sorted((ROOT / 'data/build/lm_chat').glob('*_chat_tt.txt'))
    for pattern in ('dev_*', 'test_*', 'smoke.txt'):
        paths += sorted((ROOT / 'data/build/homophone').glob(pattern))
    return sorted(set(p for p in paths if p.is_file() and p.suffix in ('.txt', '.jsonl')))


def blocked_text(paths):
    import json
    texts = []
    def strings(value):
        if isinstance(value, str):
            yield value
        elif isinstance(value, dict):
            for item in value.values():
                yield from strings(item)
        elif isinstance(value, list):
            for item in value:
                yield from strings(item)
    for path in paths:
        for line in path.read_text().splitlines():
            texts.extend(strings(json.loads(line)) if path.suffix == '.jsonl' else line.split('\t'))
    return set(texts)


def excluded(text, exact, machine):
    return text in exact or next(machine.iter(text), None) is not None

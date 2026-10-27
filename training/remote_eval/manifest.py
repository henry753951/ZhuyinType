#!/usr/bin/env python3
"""Hash the frozen portable sources and exact data consumed by the validation matrix."""
import hashlib
import json
from pathlib import Path

paths = [Path('nlp/engine/Package.swift'), Path('data/build/lexicon.tsv')]
for directory, patterns in [
    ('nlp/engine/Sources/ZhuyinCore', ['*.swift']), ('nlp/engine/Sources/ZhuyinCoreChecks', ['*.swift']),
    ('nlp/engine/Sources/ZhuyinML', ['*.swift']), ('nlp/engine/Sources/zhuyin-cli', ['*.swift']), ('nlp/engine/Sources/ONNXBridge', ['*.c', '*.h']),
    ('training/eval', ['*.py']), ('training/remote_eval', ['validation.json']),
    ('data/build/lm', ['unigram.tsv', 'bigram.tsv', 'backoff.tsv']),
    ('data/build/lm_chat', ['unigram.tsv', 'bigram.tsv', 'backoff.tsv', 'dev_chat_tt.txt']),
    ('data/build/corpus', ['dev_colloquial.txt', 'dev_wikipedia.txt']),
    ('data/build/reranker', ['vocab.json', 'config.json']),
    ('data/build/v3', ['chars.json', 'syllables.json', 'config.json', 'context.pt', 'syllable.pt', 'test_vectors.json'])]:
    for pattern in patterns:
        paths += [p for p in Path(directory).glob(pattern) if not p.name.startswith('._')] if directory.startswith('data/') else [p for p in Path(directory).rglob(pattern) if not p.name.startswith('._')]
print(json.dumps({str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}, indent=2))

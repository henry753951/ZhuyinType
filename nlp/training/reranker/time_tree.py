#!/usr/bin/env python3
"""Mac measurements only through zhuyin-cli --timing, using evaluator-derived keys."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'nlp/eval'))
from conversion_eval import load_lexicon, readings_for, keys_for


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cli', required=True)
    p.add_argument('--label', required=True)
    p.add_argument('--reranker', default='data/build/reranker')
    p.add_argument('--units', default='cpu')
    p.add_argument('--repeat', type=int, default=3)
    a = p.parse_args()
    out = ROOT / 'data/build/reranker/tree-validation' / a.label
    out.mkdir(parents=True, exist_ok=True)
    lex = load_lexicon(ROOT / 'data/build/lexicon.tsv')
    results = []
    for text in ['今天晚上要不要一起吃飯', '我們明天下午三點在公司門口集合然後一起去吃晚餐好嗎']:
        readings = readings_for(text, lex)
        keys = keys_for(readings)
        for down in [False, True]:
            for repeat in range(a.repeat):
                command = [a.cli, '--timing', '--reranker', a.reranker, '--rerank-count', '16',
                           '--rerank-weight', '1', '--v3', 'data/build/v3', '--v3-weight', '0.5',
                           '--chat-lm', 'data/build/lm_chat', '--chat-prior', '0.5', keys + ('<down>' if down else '')]
                result = subprocess.run(command, capture_output=True, text=True,
                                        env={**os.environ, 'ZHUYIN_UNITS': a.units}, check=True)
                raw = result.stdout + result.stderr
                name = f'{len(readings)}-{int(down)}-{repeat}'
                (out / (name + '.log')).write_text(raw)
                latency = re.search(r'\[keys (\d+): p50 ([\d.]+) ms, p95 ([\d.]+) ms, max ([\d.]+) ms\]', raw)
                cpu = re.search(r'\[cpu ([\d.]+) ms total, ([\d.]+) ms per key', raw)
                energy = re.search(r'\[energy ([\d.]+) mJ total, ([\d.]+) mJ per key', raw)
                memory = re.search(r'\[memory ([\d.]+) MB now, ([\d.]+) MB peak', raw)
                assert latency and cpu and energy and memory, f'missing Mac --timing metrics in {name}'
                row = dict(text=text, syllables=len(readings), keys=keys, down=down, repeat=repeat,
                           latency=[float(v) for v in latency.groups()], cpu=[float(v) for v in cpu.groups()],
                           energy=[float(v) for v in energy.groups()] if energy else None,
                           memory=[float(v) for v in memory.groups()],
                           candidate_ms=sum(map(float, re.findall(r'\[candidates \d+ ([\d.]+) ms\]', raw))))
                results.append(row)
                (out / 'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n')
                print(name, row['latency'], row['cpu'], row['energy'], row['memory'], row['candidate_ms'], flush=True)


if __name__ == '__main__':
    main()

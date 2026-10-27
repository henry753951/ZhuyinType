#!/usr/bin/env python3
"""Named CLI flag sets x evaluation sets; retain ordered predictions and print one table."""
import argparse
import json
from pathlib import Path
import subprocess
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--matrix', type=Path, required=True)
    p.add_argument('--cli', default='.build/release/zhuyin-cli')
    p.add_argument('--jobs', type=int, default=60)
    p.add_argument('--cli-arg', action='append', default=[], help='extra CLI flags for every configuration')
    p.add_argument('--output', type=Path, default=Path('data/build/remote_eval/results'))
    p.add_argument('-n', type=int, default=400)
    a = p.parse_args()
    matrix = json.loads(a.matrix.read_text())
    a.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for config in matrix['configurations']:
        for dataset in matrix['sets']:
            for name in [config['name'], dataset['name']]:
                if not name or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-' for c in name):
                    p.error('names must contain only letters, digits, _ and -')
            target = a.output / (config['name'] + '-' + dataset['name'] + '.json')
            command = [sys.executable, str(Path(__file__).with_name('capture_eval.py')), '--output', str(target),
                       '--cli', a.cli, '--jobs', str(a.jobs), '-n', str(a.n)]
            command += ['--cli-arg=' + arg for arg in config['args'] + a.cli_arg]
            command += dataset.get('args', []) + ['--test', dataset['path']]
            result = subprocess.run(command, text=True, capture_output=True)
            target.with_suffix('.log').write_text(result.stdout + result.stderr)
            if result.returncode:
                raise RuntimeError(f'{target}: evaluator failed: {result.stderr[-4000:]}')
            record = json.loads(target.read_text())['datasets'][0]
            rows.append((config['name'], dataset['name'], len(record['outputs']),
                         record['whole_accuracy_percent'], record['seconds']))
            print(f"Finished {config['name']} / {dataset['name']}: {record['seconds']:.1f}s", file=sys.stderr, flush=True)
    print('| Configuration | Set | Runs | Whole-run accuracy | Wall seconds |')
    print('|---|---|---:|---:|---:|')
    for config, dataset, count, accuracy, seconds in rows:
        print(f'| {config} | {dataset} | {count} | {accuracy:.1f}% | {seconds:.2f} |')


if __name__ == '__main__':
    main()

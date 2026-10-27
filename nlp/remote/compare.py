#!/usr/bin/env python3
"""Require identical evaluation inputs; compare predictions and whole-run accuracy by matrix row."""
import argparse
import json
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mac', type=Path, required=True)
    p.add_argument('--linux', type=Path, required=True)
    p.add_argument('--matrix', type=Path, default=Path(__file__).with_name('validation.json'))
    p.add_argument('--output', type=Path)
    a = p.parse_args()
    matrix = json.loads(a.matrix.read_text())
    table = ['| Config | Set | Mac accuracy | Linux accuracy | Different / runs | Mac seconds | Linux seconds |',
             '|---|---|---:|---:|---:|---:|---:|']
    passed = True
    for config in matrix['configurations']:
        for dataset in matrix['sets']:
            name = config['name'] + '-' + dataset['name'] + '.json'
            mac, linux = [json.loads((directory / name).read_text())['datasets'][0] for directory in [a.mac, a.linux]]
            if mac['inputs'] != linux['inputs'] or mac['test'] != linux['test']:
                raise ValueError(f'{name}: input alignment mismatch')
            if len(mac['outputs']) != len(linux['outputs']) or not mac['outputs']:
                raise ValueError(f'{name}: missing outputs')
            different = sum(x != y for x, y in zip(mac['outputs'], linux['outputs']))
            count = len(mac['outputs'])
            passed &= different / count <= .01
            table.append(f"| {config['name']} | {dataset['name']} | {mac['whole_accuracy_percent']:.1f}% | "
                         f"{linux['whole_accuracy_percent']:.1f}% | {different}/{count} | {mac['seconds']:.2f} | {linux['seconds']:.2f} |")
    output = '\n'.join(table) + '\n'
    if a.output:
        a.output.write_text(output)
    print(output, end='')
    raise SystemExit(0 if passed else 1)

if __name__ == '__main__':
    main()

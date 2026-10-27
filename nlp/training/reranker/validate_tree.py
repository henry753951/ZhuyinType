#!/usr/bin/env python3
"""Linux-only score/decision parity on real Composer calls from run.sh sweep captures."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--captures', type=Path, default=Path('data/build/remote_eval/tree'))
    p.add_argument('--out', type=Path, default=Path('data/build/reranker/tree-validation/parity'))
    p.add_argument('--jobs', type=int, default=60)
    p.add_argument('--lines-per-set', type=int, default=40)
    a = p.parse_args()
    if sys.platform != 'linux':
        p.error('evaluations run only on the remote Linux runner')
    a.out.mkdir(parents=True, exist_ok=True)
    products = next(x for x in Path('.build').glob('out/Products/*') if (x / 'ZhuyinCore.o').exists())
    modulemap = next(Path('.build').glob('out/Intermediates.noindex/GeneratedModuleMaps-*/ONNXBridge.modulemap'))
    executable = a.out / 'check-tree'
    command = ['swiftc', '-O', '-swift-version', '5', '-parse-as-library', '-I', str(products),
               '-Xcc', '-fmodule-map-file=' + str(modulemap)]
    command += [str(x) for x in Path('nlp/engine/Sources/ZhuyinML').glob('*.swift')]
    command += ['nlp/training/reranker/check_tree.swift', str(products / 'ZhuyinCore.o'), str(products / 'ONNXBridge.o'),
                '-L', os.path.expanduser('~/opt/onnxruntime/lib'), '-lonnxruntime', '-Xlinker', '-rpath',
                '-Xlinker', os.path.expanduser('~/opt/onnxruntime/lib'), '-o', str(executable)]
    subprocess.run(command, check=True)
    subprocess.run([str(executable)], check=True)
    empty = a.out / 'edges-input.json'
    empty.write_text('[]\n')
    subprocess.run([str(executable), str(empty), str(a.out / 'edges.json'), '--edges'], check=True)
    edges = json.loads((a.out / 'edges.json').read_text())
    edge_error = max(abs(x - y) for row in edges for x, y in zip(row['current'], row['tree']))
    assert edge_error < .01, edge_error
    lines = []
    line_rows = []
    for current_path in sorted(a.captures.glob('current-*.json')):
        current = json.loads(current_path.read_text())['datasets'][0]
        tree = json.loads(current_path.with_name(current_path.name.replace('current-', 'tree-')).read_text())['datasets'][0]
        assert current['inputs'] == tree['inputs'] and current['test'] == tree['test']
        assert len(current['outputs']) == len(tree['outputs']) == 400
        differences = [dict(index=i, input=key, current=x, tree=y)
                       for i, (key, x, y) in enumerate(zip(current['inputs'], current['outputs'], tree['outputs'])) if x != y]
        assert len(differences) / len(current['outputs']) <= 0.005, current_path
        line_rows.append(dict(name=current_path.stem[8:], lines=len(current['outputs']), differences=differences))
        lines += current['inputs'][:a.lines_per_set]
    assert len(line_rows) == 5
    shards = [lines[i::a.jobs] for i in range(a.jobs) if lines[i::a.jobs]]
    def run_shard(pair):
        index, lines = pair
        source, target = [a.out / f'{index:02}-{suffix}.json' for suffix in ['input', 'calls']]
        source.write_text(json.dumps(lines, ensure_ascii=False) + '\n')
        result = subprocess.run([str(executable), str(source), str(target)], capture_output=True, text=True)
        (a.out / f'{index:02}.log').write_text(result.stdout + result.stderr)
        result.check_returncode()
        return json.loads(target.read_text())
    with ThreadPoolExecutor(a.jobs) as pool:
        calls = [row for chunk in pool.map(run_shard, enumerate(shards)) for row in chunk]
    assert len(calls) >= 1000
    deltas = [abs(x - y) for row in calls for x, y in zip(row['current'], row['tree'])]
    differences = [row for row in calls if any(row['current' + key] != row['tree' + key] for key in ['Argmax', 'TotalArgmax', 'Choice'])]
    parity = {key: sum(row['current' + key] == row['tree' + key] for row in calls) / len(calls)
              for key in ['Argmax', 'TotalArgmax', 'Choice']}
    assert all(value >= .999 for value in parity.values()), parity
    assert statistics.median(deltas) < .01
    sorted_deltas = sorted(deltas)
    summary = dict(lines=line_rows, replayed_lines=len(lines), calls=len(calls), sentence_scores=len(deltas),
                   median_abs_error=statistics.median(deltas), max_abs_error=max(deltas),
                   edge_max_abs_error=edge_error,
                   p95_abs_error=sorted_deltas[int(.95 * (len(deltas) - 1))],
                   fraction_under_001=sum(value < .01 for value in deltas) / len(deltas),
                   decision_parity=parity, decision_differences=differences,
                   median_nodes=statistics.median(row['nodes'] for row in calls),
                   max_nodes=max(row['nodes'] for row in calls))
    (a.out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(summary, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    main()

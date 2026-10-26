#!/usr/bin/env python3
"""Run an existing evaluator, parallelizing its independent CLI batch lines without changing it."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import runpy
import subprocess
import sys
import threading

TOOLS = ['conversion_eval', 'error_analysis', 'typing_errors', 'typo_eval']


def parallel_run(original, command, jobs, slots, **kwargs):
    lines = kwargs['input'].splitlines(keepends=True)
    if not lines:
        return original(command, **kwargs)
    size = -(-len(lines) // min(jobs, len(lines)))

    def part(start):
        with slots:
            return original(command, **{**kwargs, 'input': ''.join(lines[start:start + size])})

    with ThreadPoolExecutor(min(jobs, len(lines))) as pool:
        parts = list(pool.map(part, range(0, len(lines), size)))
    return subprocess.CompletedProcess(command, next((r.returncode for r in parts if r.returncode), 0),
                                       ''.join(r.stdout for r in parts), ''.join(r.stderr for r in parts))


def main():
    p = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    p.add_argument('--jobs', type=int, default=60)
    p.add_argument('--cli-arg', action='append', default=[])
    p.add_argument('tool', choices=TOOLS)
    a, rest = p.parse_known_args()
    if a.jobs < 1:
        p.error('--jobs must be positive')
    original = subprocess.run
    slots = threading.Semaphore(a.jobs)
    if a.tool == 'conversion_eval':
        rest += ['--jobs', str(a.jobs)] + ['--cli-arg=' + x for x in a.cli_arg]
    else:
        cli = rest[rest.index('--cli') + 1] if '--cli' in rest else None
        lexicon = rest[rest.index('--lexicon') + 1] if '--lexicon' in rest else 'data/build/lexicon.tsv'

        def run(command, **kwargs):
            if isinstance(command, list) and '--batch' in command and (cli is None or command[0] == cli):
                command = command + a.cli_arg
                if '--lexicon' not in command:
                    command += ['--lexicon', lexicon]
                if '--keep-memory' in command:
                    raise ValueError('Cannot parallelize stateful --keep-memory batches')
                return parallel_run(original, command, a.jobs, slots, **kwargs)
            return original(command, **kwargs)

        subprocess.run = run
    path = Path(__file__).parents[1] / 'eval' / (a.tool + '.py')
    sys.path.insert(0, str(path.parent))
    sys.argv = [str(path)] + rest
    try:
        runpy.run_path(str(path), run_name='__main__')
    finally:
        subprocess.run = original


if __name__ == '__main__':
    main()

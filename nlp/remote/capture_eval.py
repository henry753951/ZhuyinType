#!/usr/bin/env python3
"""Run the unmodified evaluator, retaining ordered inputs/outputs and per-dataset wall time."""
import argparse
from contextlib import redirect_stdout
import io
import re
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args, rest = parser.parse_known_args()
    spec = importlib.util.spec_from_file_location('conversion_eval', Path(__file__).parents[1] / 'eval/conversion_eval.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    original_popen = subprocess.Popen
    captures, datasets = [], []

    class Captured(original_popen):
        def __init__(self, *a, **kw):
            super().__init__(*a, **kw)
            self.record = {}
            captures.append(self.record)

        def communicate(self, input=None, **kw):
            result = super().communicate(input, **kw)
            self.record.update(inputs=(input or '').splitlines(), outputs=result[0].splitlines())
            return result

    original_evaluate = module.evaluate

    def evaluate(test, *a):
        captures.clear()
        start = time.monotonic()
        stream = io.StringIO()
        with redirect_stdout(stream):
            original_evaluate(test, *a)
        output = stream.getvalue()
        print(output, end="", flush=True)
        accuracy = float(re.search(r"whole-run accuracy ([0-9.]+)%", output)[1])
        datasets.append({'test': test, 'seconds': time.monotonic() - start,
                         'whole_accuracy_percent': accuracy,
                         'inputs': [line for chunk in captures for line in chunk['inputs']],
                         'outputs': [line for chunk in captures for line in chunk['outputs']]})
        args.output.write_text(json.dumps({'argv': rest, 'datasets': datasets}, ensure_ascii=False, indent=2) + '\n')

    module.evaluate = evaluate
    subprocess.Popen = Captured
    sys.argv = [str(spec.origin)] + rest
    module.main()


if __name__ == '__main__':
    main()

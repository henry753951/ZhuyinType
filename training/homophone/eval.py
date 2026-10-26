"""Frozen CLI and reference-v3 confusion-position scoring, with explicit context."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import os
import subprocess
import sys
import time
import threading

from common import OUT, ROOT, digest, jsonl, positions, save, v3


def align(want, got):
    costs = [[0]*(len(got)+1) for _ in range(len(want)+1)]
    for i in range(len(want)+1):
        costs[i][0] = i
    for j in range(len(got)+1):
        costs[0][j] = j
    for i, a in enumerate(want, 1):
        for j, b in enumerate(got, 1):
            costs[i][j] = min(costs[i-1][j-1]+(a != b), costs[i-1][j]+1, costs[i][j-1]+1)
    mapping, i, j = {}, len(want), len(got)
    while i or j:
        if i and j and costs[i][j] == costs[i-1][j-1]+(want[i-1] != got[j-1]):
            mapping[i-1] = got[j-1]
            i -= 1; j -= 1
        elif i and costs[i][j] == costs[i-1][j]+1:
            mapping[i-1] = ''
            i -= 1
        else:
            j -= 1
    return mapping


def metrics(rows, predictions):
    groups = defaultdict(lambda: [0, 0])
    errors = []
    for row, got in zip(rows, predictions, strict=True):
        text = row['text']
        mapping = align(text, got)
        for p in row['positions']:
            hit = mapping[p['index']] == p['char']
            keys = ['all', 'char:'+p['char'], 'group:'+p['group'], 'category:'+p['category'],
                    'group_category:'+p['group']+':'+p['category']]
            if p['category'] != 'fixed':
                keys.append('interesting')
                keys.append('group_interesting:'+p['group'])
            for key in keys:
                groups[key][0] += hit
                groups[key][1] += 1
        if got != text:
            errors.append(dict(text=text, got=got, context=row['context']))
    return dict(sentences=len(rows), whole_correct=len(rows)-len(errors),
                whole_accuracy=(len(rows)-len(errors))/max(1, len(rows)),
                positions={k: dict(correct=c, total=n, accuracy=c/n) for k, (c,n) in sorted(groups.items())},
                errors=errors)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--test', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--cli', type=Path, default=OUT/'zhuyin-cli')
    p.add_argument('--lexicon', type=Path, default=ROOT/'data/build/lexicon.tsv')
    p.add_argument('--cli-arg', action='append', default=[])
    p.add_argument('--reranker', type=Path)
    p.add_argument('--v3', type=Path)
    p.add_argument('--line-context', action='store_true')
    p.add_argument('--model-only', action='store_true')
    p.add_argument('--jobs', type=int, default=60)
    p.add_argument('--backend', choices=['torch','coreml'], default='coreml')
    args = p.parse_args()
    assert sys.platform == 'linux', 'Evaluation runs remotely; Mac is export/parity only.'
    assert args.jobs > 0
    assert not args.output.exists(), 'Output exists: do not silently rerun evaluations.'
    sys.path.insert(0, str(ROOT/'training/eval'))
    import conversion_eval as conversion
    lexicon = conversion.load_lexicon(args.lexicon)
    v3()
    from readings import Lexicon, split_reading
    lex = Lexicon(args.lexicon)
    inventory = json.loads((OUT/'inventory.json').read_text())['kept']
    metadata = args.test.with_suffix('.jsonl')
    if metadata.exists():
        rows = list(jsonl(metadata))
    else:
        rows = []
        for line in args.test.read_text().splitlines():
            fields = line.split('\t')
            context, text = (fields if len(fields)==2 else ('', fields[0]))
            rows.append(dict(text=text, context=context, positions=positions(text, lex, inventory)))
    for row in rows:
        if args.test.stem.endswith('_zai') or args.test.stem=='smoke':
            row['positions'] = [p for p in row['positions'] if p['group']=='在再']
        elif args.test.stem.endswith('_pairs'):
            row['positions'] = [p for p in row['positions'] if p['group']!='在再']
    scripts, selected = [], []
    for row in rows:
        readings = conversion.readings_for(row['text'], lexicon)
        if readings is None or len(readings) != len(row['text']):
            raise ValueError(f'Unaligned or untypable evaluation text: {row["text"]}')
        # Use the shared toned reading for each annotated confusion (including neutral 的/得).
        for pos in row['positions']:
            base, tone = pos['reading']
            readings[pos['index']] = base + {1:'',2:'ˊ',3:'ˇ',4:'ˋ',5:'˙'}[tone]
        row['typed'] = readings
        prefix = row['context'].replace('\n',' ').replace('\t',' ') + '\t' if args.line_context else ''
        scripts.append(prefix+conversion.keys_for(readings)+'<enter>')
        selected.append(row)
    start = time.monotonic()
    if args.model_only:
        assert args.v3
        from reference import Runner, inputs
        runner = Runner(args.backend, args.v3)
        predictions = []
        elapsed = []
        for row in selected:
            typed = [list(split_reading(r))+[0,0] for r in row['typed']]
            candidates = []
            for char, r in zip(row['text'], row['typed']):
                pool = sorted(lex.candidates[split_reading(r)], key=lambda c: (-lex.candidates[split_reading(r)][c],c))
                candidates.append([char]+[c for c in pool if c != char][:63])
            case = dict(context=row['context'] if args.line_context else '', syllables=typed, candidates=candidates)
            ctx, values = inputs(case, args.v3)
            tick = time.perf_counter()
            _, score = runner.predict(ctx, values)
            elapsed.append(1000*(time.perf_counter()-tick))
            predictions.append(''.join(pool[int(i)] for pool,i in zip(candidates, score.argmax(-1)[0])))
        command = None
    else:
        command = [str(args.cli), '--lexicon',str(args.lexicon),'--batch','--typo','standard']
        if args.reranker:
            command += ['--reranker',str(args.reranker),'--rerank-weight','1','--rerank-count','16']
        if args.v3:
            command += ['--v3',str(args.v3),'--v3-weight','1']
        command += args.cli_arg
        env = {k:v for k,v in os.environ.items() if k != 'ZHUYIN_UNITS'}
        sys.path.insert(0, str(ROOT/'training/remote_eval'))
        from evaluate import parallel_run
        result = parallel_run(subprocess.run, command, args.jobs, threading.Semaphore(args.jobs),
                              input='\n'.join(scripts)+'\n', capture_output=True,text=True,check=True,env=env)
        predictions = result.stdout.splitlines()
        elapsed = None
    result = metrics(selected, predictions)
    result.update(seconds=time.monotonic()-start, command=command, line_context=args.line_context,
                  model_only=args.model_only, test_sha256=digest(args.test),
                  predictions=predictions, per_reference_call_ms=elapsed,
                  cli_sha256=digest(args.cli) if not args.model_only else None,
                  model_only_candidates='top64 with gold injected; conditional selection, not decoder recall' if args.model_only else None)
    save(args.output, result)
    print(json.dumps({k:v for k,v in result.items() if k not in ['errors','predictions','per_reference_call_ms']},ensure_ascii=False,indent=2))


if __name__ == '__main__':
    main()

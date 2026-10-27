"""Remote-only paired conversion matrix and common-token word perplexity."""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import math
from pathlib import Path
import platform
import random
import re
import subprocess
import sys
import time
import numpy as np
from common import ROOT, OUT, lm, chat, save, digest

evaluator = ROOT / 'nlp/eval/conversion_eval.py'
if not evaluator.exists(): evaluator = OUT / 'inputs/conversion_eval.py'
spec = importlib.util.spec_from_file_location('conversion', evaluator)
ev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ev)
SETS = [('dev_colloquial', ROOT / 'data/build/corpus/dev_colloquial.txt', False),
        ('dev_chat_tt', ROOT / 'data/build/lm_chat/dev_chat_tt.txt', False),
        ('dev_wikipedia', ROOT / 'data/build/corpus/dev_wikipedia.txt', True),
        ('dev_dialogue', OUT / 'dev_dialogue.txt', True)]


def candidates(path, context):
    found = []
    for line in path.read_text().splitlines():
        previous, target = line.split('\t', 1) if path.name == 'dev_dialogue.txt' else ('', line)
        for m in re.finditer(r'[\u4e00-\u9fff]{4,20}', target):
            before = (previous + (' ' if m.start() else '') + target[:m.start()])[-32:] if previous else target[:m.start()][-32:]
            found.append((m.group(), before if context else ''))
    random.Random(7).shuffle(found)
    if path.name != 'dev_dialogue.txt':
        # Match conversion_eval exactly, including its duplicate-run context lookup.
        before = dict(found)
        found = [(run, before[run]) for run, _ in found]
    return found


def selected_inputs(path, context, lexicon, n):
    selected, scripts = [], []
    for run, previous in candidates(path, context):
        readings = ev.readings_for(run, lexicon)
        if not readings: continue
        selected.append(run)
        prefix = previous.replace('\t', ' ').replace('\n', ' ') + '\t' if context else ''
        scripts.append(prefix + ev.keys_for(readings) + '<enter>')
        if len(selected) == n: break
    if not selected: raise ValueError(f'no eligible runs: {path}')
    return selected, scripts


def run_batch(command, scripts, failure):
    result = subprocess.run(command, input=''.join(s + '\n' for s in scripts), text=True, capture_output=True)
    if result.returncode:
        failure.parent.mkdir(parents=True, exist_ok=True)
        save(failure, {'command': command, 'inputs': scripts, 'returncode': result.returncode,
                       'stdout': result.stdout, 'stderr': result.stderr})
        raise RuntimeError(f'CLI returncode={result.returncode}; batch and diagnostics saved to {failure}')
    return result


def conversion(a):
    lexicon = ev.load_lexicon(ROOT / 'data/build/lexicon.tsv')
    cases = {name: selected_inputs(path, context, lexicon, a.n) for name, path, context in SETS}
    out = OUT / 'conversion'
    out.mkdir(exist_ok=True)
    rows = []
    base = [a.cli, '--lexicon', 'data/build/lexicon.tsv', '--typo', 'standard', '--batch',
            '--reranker', 'data/build/reranker', '--rerank-count', '16', '--v3', 'data/build/v3', '--v3-weight', '.5']
    base += a.cli_arg
    for model in ('lm_chat', 'lm_chat2'):
        for prior in (.3, .5, .7):
            for name, path, context in SETS:
                target = out / f'{model}-{prior}-{name}.json'
                wanted, scripts = cases[name]
                command = base + ['--chat-lm', 'data/build/' + model, '--chat-prior', str(prior)]
                config = {'command': command, 'input_sha256': __import__('hashlib').sha256(('\n'.join(scripts) + '\n').encode()).hexdigest(),
                          'cli_sha256': digest(Path(a.cli)), 'lexicon_sha256': digest(ROOT / 'data/build/lexicon.tsv')}
                if target.exists():
                    record = json.loads(target.read_text())
                    if record['config'] != config: raise ValueError(f'stale result: {target}')
                    rows.append(record)
                    continue
                started = time.monotonic()
                jobs = min(a.jobs, len(scripts))
                size = -(-len(scripts) // jobs)
                def run(i):
                    return run_batch(command, scripts[i:i + size],
                                     out / 'failures' / f'{model}-{prior}-{name}-{i}.json')
                with ThreadPoolExecutor(jobs) as pool:
                    results = list(pool.map(run, range(0, len(scripts), size)))
                outputs = [s for result in results for s in result.stdout.splitlines()]
                if len(outputs) != len(wanted): raise ValueError('CLI output count mismatch')
                exact = sum(w == g.strip() for w, g in zip(wanted, outputs))
                variant = sum(w.translate(ev.VARIANTS) == g.strip().translate(ev.VARIANTS) for w, g in zip(wanted, outputs))
                characters = sum(sum(x == y for x, y in zip(w, g.strip())) for w, g in zip(wanted, outputs))
                record = {'model': model, 'prior': prior, 'set': name, 'runs': len(wanted),
                          'whole_correct': exact, 'whole_percent': 100 * exact / len(wanted),
                          'variant_correct': variant, 'variant_percent': 100 * variant / len(wanted),
                          'character_percent': 100 * characters / sum(map(len, wanted)),
                          'seconds': time.monotonic() - started, 'config': config,
                          'targets': wanted, 'inputs': scripts, 'outputs': outputs,
                          'stderr': [r.stderr for r in results if r.stderr]}
                save(target, record)
                rows.append(record)
                print(f"{model} P={prior} {name}: {exact}/{len(wanted)} ({record['whole_percent']:.2f}%), variants {record['variant_percent']:.2f}%, {record['seconds']:.1f}s", flush=True)
    save(OUT / 'conversion-summary.json', [{k: v for k, v in row.items() if k not in ('targets', 'inputs', 'outputs', 'stderr')} for row in rows])


def perplexity():
    # Freeze the OLD chat final-E tokenizer for all models to preserve historical comparability.
    tok = OUT / 'inputs/tokenizer'
    words = json.loads((tok / 'vocabulary.json').read_text())
    scores = np.load(tok / 'segmentation_scores.npy')
    _, _, old = lm.read_lexicon(OUT / 'inputs/lexicon.tsv')
    ids = {w: i for i, w in enumerate(words)}
    trie = lm.make_trie(words, old)
    def tokens(text):
        return [words[i] for i in lm.segment(chat.normalized(text), trie, ids, scores, float(scores[ids['<unk>']]))]
    tokenized = {}
    for name, path, _ in SETS:
        tokenized[name] = []
        for line in path.read_text().splitlines():
            previous, target = line.split('\t', 1) if name == 'dev_dialogue' else ('', line)
            prior = tokens(previous) if previous and re.search(r'[\u4e00-\u9fff]$', previous) else []
            target_words = tokens(target)
            seq = [prior[-1] if prior else '<s>', *target_words, '</s>']
            tokenized[name].append((target, list(zip(seq, seq[1:]))))
    paths = {'wiki': ROOT / 'data/build/lm', 'lm_chat': ROOT / 'data/build/lm_chat',
             'lm_chat2': OUT, 'no_generated': OUT / 'no_generated'}
    metrics = {name: {'lines': len(rows), 'predictions_with_eos': sum(len(events) for _, events in rows), 'models': {}}
               for name, rows in tokenized.items()}
    with (OUT / 'perplexity-lines.tsv').open('w') as lines:
        lines.write('model\tset\tline\tpredictions_with_eos\tlog_likelihood\n')
        for model_name, directory in paths.items():
            model = chat.Model(directory)
            for name, rows in tokenized.items():
                total = 0.
                unknown = 0
                for i, (_, events) in enumerate(rows, 1):
                    ll = math.fsum(model.score(a, b) for a, b in events)
                    total += ll
                    unknown += sum(b not in model.uni or b == '<unk>' for _, b in events)
                    lines.write(f'{model_name}\t{name}\t{i}\t{len(events)}\t{ll:.12f}\n')
                metrics[name]['models'][model_name] = {'perplexity': math.exp(-total / metrics[name]['predictions_with_eos']),
                    'log_likelihood': total, 'unknown_targets_with_eos': unknown}
            del model
    save(OUT / 'perplexity.json', {'protocol': 'Same old chat final E-step tokens; natural logs, EOS included, BOS/context not scored; dialogue target only, previous final word seeds context only if no trailing boundary punctuation.',
         'tokenizer_sha256': {p.name: digest(p) for p in tok.iterdir()}, 'sets': metrics})
    print(json.dumps(metrics, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    if platform.system() != 'Linux': raise SystemExit('Evaluation must run on the remote Linux host, never on the Mac.')
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--phase', choices=['perplexity', 'conversion', 'all'], default='all')
    p.add_argument('--cli', default='.build/release/zhuyin-cli')
    p.add_argument('--jobs', type=int, default=60)
    p.add_argument('--cli-arg', action='append', default=[])
    p.add_argument('-n', type=int, default=400)
    a = p.parse_args()
    if a.phase in ('perplexity', 'all'): perplexity()
    if a.phase in ('conversion', 'all'): conversion(a)

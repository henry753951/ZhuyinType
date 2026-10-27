"""Frozen-key paired dev/test conversion, lexicon audit, and typing habits."""
import argparse
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import random
import re
import subprocess
import time
from common import ROOT, OUT, ev, stats, save, digest, load, remote_only


def cases(path, context, n):
    lexicon = ev.load_lexicon(OUT / 'inputs/lexicon.tsv')
    found = [(m.group(), line[:m.start()][-32:] if context else '')
             for line in path.read_text().splitlines()
             for m in re.finditer(r'[\u4e00-\u9fff]{4,20}', line)]
    random.Random(7).shuffle(found)
    before = dict(found)
    rows = []
    for word, _ in found:
        reading = ev.readings_for(word, lexicon)
        if reading:
            rows.append({'gold': word, 'reading': reading, 'context': before[word]})
        if len(rows) == n:
            break
    if len(rows) != n:
        raise ValueError(f'only {len(rows)} eligible runs: {path}')
    return rows


def batch(a, command, scripts, target):
    while True:
        available=int(re.search(r'MemAvailable:\s+(\d+)',Path('/proc/meminfo').read_text())[1])*1024
        if available >= 18*1024**3: break
        print(f'Waiting for host RAM: {available/1024**3:.1f} GiB available',flush=True)
        time.sleep(10)
    jobs=min(a.jobs,max(1,int((available-16*1024**3)/(1.1*1024**3))),len(scripts))
    size = -(-len(scripts) // jobs)
    def part(i):
        p = subprocess.run(command, input='\n'.join(scripts[i:i+size])+'\n',
                           text=True, capture_output=True)
        if p.returncode or len(p.stdout.splitlines()) != len(scripts[i:i+size]):
            save(target.with_name(target.stem + f'-failure-{i}.json'),
                 {'command': command, 'inputs': scripts[i:i+size], 'returncode': p.returncode,
                  'stdout': p.stdout, 'stderr': p.stderr})
            raise RuntimeError(f'CLI batch failed ({p.returncode}); diagnostics: {target}')
        return p.stdout.splitlines()
    with ThreadPoolExecutor(jobs) as pool:
        return [s.strip() for chunk in pool.map(part, range(0, len(scripts), size)) for s in chunk]


def distance(a, b):
    row = list(range(len(b)+1))
    for i, x in enumerate(a, 1):
        next_row = [i]
        for j, y in enumerate(b, 1):
            next_row.append(min(next_row[-1]+1, row[j]+1, row[j-1]+(x != y)))
        row = next_row
    return row[-1]


def paired(record):
    result = []
    for i, (case, output) in enumerate(zip(record['cases'], record['outputs']), 1):
        gold = case['gold']
        g, o = gold.translate(ev.VARIANTS), output.translate(ev.VARIANTS)
        result.append({'row': i, 'id': hashlib.sha256(json.dumps(case, sort_keys=True).encode()).hexdigest(),
                       'gold_len': len(gold), 'ok_raw': int(gold == output),
                       'ok_variants': int(g == o), 'errors_raw': distance(gold, output),
                       'errors_variants': distance(g, o)})
    return result


def command(a, config, app):
    cmd = [a.cli, '--batch', '--lexicon', config['lexicon'], '--typo', 'standard',
           '--chat-lm', config['chat'], '--chat-prior', '.5']
    if app:
        cmd += ['--reranker', 'data/build/reranker', '--rerank-count', '16', '--rerank-weight', '1',
                '--v3', 'data/build/v3', '--v3-weight', '.5']
    return cmd


def conversion(a):
    matrix = json.loads(a.matrix.read_text())
    phase = a.phase
    paths = [('colloquial', ROOT/f'data/build/corpus/{phase}_colloquial.txt', False),
             ('wikipedia', ROOT/('data/build/corpus/dev_wikipedia.txt' if phase == 'dev'
                                else 'data/build/lm/test_sentences.txt'), False),
             ('wikipedia_context', ROOT/('data/build/corpus/dev_wikipedia.txt' if phase == 'dev'
                                        else 'data/build/lm/test_sentences.txt'), True),
             ('taiwan_tongues', ROOT/f'data/build/lm_chat/{phase}_chat_tt.txt', False)]
    directory = OUT / 'evaluation' / phase
    directory.mkdir(parents=True, exist_ok=True)
    selected = {name: cases(path, context, a.n) for name, path, context in paths}
    for config in matrix:
        for mode in ('engine', 'app'):
            for name, path, context in paths:
                target = directory / f"{config['name']}-{mode}-{name}.json"
                rows = selected[name]
                scripts = [(r['context'].replace('\t', ' ')+'\t' if context else '')
                           + ev.keys_for(r['reading'])+'<enter>' for r in rows]
                cmd = command(a, config, mode == 'app')
                files = [Path(config['lexicon']), Path(a.cli)]
                for model in (Path(config['lexicon']).parent/'lm', Path(config['chat'])):
                    files += [model/f'{n}.tsv' for n in ('unigram', 'bigram', 'backoff')]
                if mode == 'app':
                    files += [p for d in ('reranker','v3') for p in (ROOT/'data/build'/d).glob('*')
                              if p.suffix in ('.onnx', '.json') and 'fp16' not in p.name]
                signature = {'command': cmd, 'cases': rows, 'files': {str(p): digest(p) for p in files}}
                if target.exists():
                    record = json.loads(target.read_text())
                    if record['signature'] != signature:
                        raise ValueError(f'stale result: {target}')
                else:
                    start = time.monotonic()
                    outputs = batch(a, cmd, scripts, target)
                    record = {'config': config['name'], 'mode': mode, 'set': name, 'cases': rows,
                              'outputs': outputs, 'seconds': time.monotonic()-start, 'signature': signature}
                    metrics = paired(record)
                    record.update(n=len(rows), exact=100*sum(r['ok_raw'] for r in metrics)/len(rows),
                                  variants=100*sum(r['ok_variants'] for r in metrics)/len(rows))
                    save(target, record)
                    target.with_suffix('.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in metrics))
                print(f"{config['name']} {mode} {name}: {record['exact']:.2f}% / {record['variants']:.2f}% ({record['seconds']:.1f}s)", flush=True)
                baseline = directory / f'baseline-{mode}-{name}.jsonl'
                if config['name'] != 'baseline' and baseline.exists():
                    for normalization in ('raw', 'variants'):
                        result = stats.compare(stats.read_results(baseline, normalization),
                                               stats.read_results(target.with_suffix('.jsonl'), normalization),
                                               normalization=normalization, jobs=1)
                        save(target.with_name(target.stem+f'-paired-{normalization}.json'), result)


def audit(a):
    config = json.loads(a.matrix.read_text())[0]
    _, rows, _ = __import__('common').lm.read_lexicon(Path(config['lexicon']))
    by_reading = defaultdict(list)
    for reading, word, p in rows:
        by_reading[reading].append((word, p))
    readings = sorted(by_reading)
    target = OUT / f"audit-{config['name']}.json"
    outputs = batch(a, command(a, config, False),
                    [ev.keys_for(r.split())+'<enter>' for r in readings], target)
    counts = defaultdict(Counter)
    old = set(ev.load_lexicon(OUT/'inputs/lexicon.tsv'))
    model_words = {line.split('\t',1)[0] for line in
                   (Path(config['lexicon']).parent/'lm/unigram.tsv').open()}
    wins = set()
    with target.with_suffix('.tsv').open('w') as f:
        f.write('reading\tword\twinner\tclass\tlogp\n')
        for reading, winner in zip(readings, outputs):
            for word, p in by_reading[reading]:
                source = 'existing' if word in old else 'mcbopomofo_new'
                collision = len(by_reading[reading]) > 1
                for group in ('all', source, f'length_{len(word)}', source+f'_length_{len(word)}',
                              'lm_known' if word in model_words else 'lm_oov',
                              'homophone' if collision else 'unique_reading'):
                    counts[group]['rows'] += 1
                    counts[group]['loses'] += winner != word
                if winner == word: wins.add(word)
                else: f.write(f'{reading}\t{word}\t{winner}\t{source}\t{p}\n')
    result = {k: dict(v, loss_percent=100*v['loses']/v['rows']) for k,v in counts.items()}
    words = {w for _, w, _ in rows}
    result['words_any_reading'] = {'words': len(words), 'loses': len(words-wins),
                                   'loss_percent': 100*len(words-wins)/len(words)}
    save(target, result)
    print(json.dumps(result, indent=2), flush=True)


def habits(a):
    error = load('base_habits', 'training/eval/error_analysis.py')
    selected = cases(ROOT/'data/build/corpus/dev_colloquial.txt', False, a.n)
    rng = random.Random(7)
    groups = defaultdict(list)
    for row in selected:
        word, syllables = row['gold'], row['reading']
        groups['clean'].append((word, error.keys_for(syllables)))
        for name, value in (('sandhi', error.sandhi(word, syllables)), ('neutral', error.neutral(syllables))):
            if value: groups[name].append((word, error.keys_for(value)))
        flags = error.nospace(syllables, rng)
        if flags: groups['nospace'].append((word, error.keys_for(syllables, flags)))
        groups['typo5'].append((word, error.noisy_script(syllables,.05,rng).removesuffix('<enter>')))
    for config in json.loads(a.matrix.read_text()):
        for mode in ('engine','app'):
            metrics = {}
            for name, items in groups.items():
                cmd = command(a, config, mode == 'app')
                target = OUT / f"habits-{config['name']}-{mode}-{name}.json"
                outputs = batch(a, cmd, [k+'<enter>' for _,k in items], target)
                # Engine oracle is measured without neural models, as in error_analysis.py.
                oracle = batch(a, command(a,config,False)+['--report-nbest','16'], [k for _,k in items], target)
                metrics[name] = {'n':len(items), 'exact':100*sum(w==o for (w,_),o in zip(items,outputs))/len(items),
                                 'oracle16':100*sum(w in o.split('\t') for (w,_),o in zip(items,oracle))/len(items)}
                save(target, {'gold':[w for w,_ in items],'outputs':outputs,'oracle':oracle})
            save(OUT/f"habits-{config['name']}-{mode}.json",metrics)
            print(config['name'],mode,json.dumps(metrics),flush=True)


if __name__ == '__main__':
    remote_only()
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--phase', choices=['dev','test','audit','habits'],default='dev')
    p.add_argument('--matrix',type=Path,required=True)
    p.add_argument('--cli',default='.build/release/zhuyin-cli')
    p.add_argument('--jobs',type=int,default=8)
    p.add_argument('-n',type=int,default=400)
    a=p.parse_args()
    if a.jobs < 1 or a.n < 1: p.error('positive jobs and run count required')
    {'dev':conversion,'test':conversion,'audit':audit,'habits':habits}[a.phase](a)

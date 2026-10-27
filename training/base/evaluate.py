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
from common import ROOT, OUT, ev, stats, save, digest, load, remote_only, verify_frozen


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
    runtime_files=[Path(a.cli)]+[p for d in ('reranker','v3') for p in (ROOT/'data/build'/d).glob('*')
                               if p.suffix in ('.onnx','.json') and 'fp16' not in p.name]
    runtime={str(p):digest(p) for p in runtime_files}
    frozen=OUT/'runtime.json'
    if frozen.exists():
        if json.loads(frozen.read_text())!=runtime:
            raise ValueError('frozen CLI/neural runtime changed; comparisons would not be paired')
    else:
        baseline=OUT/'evaluation/dev/baseline-app-colloquial.json'
        if baseline.exists():
            signatures=json.loads(baseline.read_text())['signature']['files']
            if any(signatures.get(k)!=v for k,v in runtime.items()):
                raise ValueError('runtime changed since the recorded baseline')
        save(frozen,runtime)
    phase = a.phase
    heldout=OUT/'inputs/evaluation'
    wiki=heldout/('dev_wikipedia.txt' if phase=='dev' else 'test_sentences.txt')
    paths = [('colloquial', heldout/f'{phase}_colloquial.txt', False),
             ('wikipedia', wiki, False),('wikipedia_context', wiki, True),
             ('taiwan_tongues', heldout/f'{phase}_chat_tt.txt', False)]
    if phase=='test':
        paths.append(('smoke',heldout/'colloquial_smoke.txt',False))
    directory = OUT / 'evaluation' / phase
    directory.mkdir(parents=True, exist_ok=True)
    selected = {name: cases(path, context, 68 if name=='smoke' else a.n) for name, path, context in paths}
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


def audit(a,config=None):
    config = config or json.loads(a.matrix.read_text())[0]
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
    preferred = ev.load_lexicon(config['lexicon'])
    model_words = {line.split('\t',1)[0] for line in
                   (Path(config['lexicon']).parent/'lm/unigram.tsv').open()}
    wins = set()
    preferred_wins = set()
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
                if winner == word:
                    wins.add(word)
                    if reading == preferred[word]: preferred_wins.add(word)
                else: f.write(f'{reading}\t{word}\t{winner}\t{source}\t{p}\n')
    result = {k: dict(v, loss_percent=100*v['loses']/v['rows']) for k,v in counts.items()}
    words = {w for _, w, _ in rows}
    result['words_any_reading'] = {'words': len(words), 'loses': len(words-wins),
                                   'loss_percent': 100*len(words-wins)/len(words)}
    result['words_preferred_reading'] = {'words': len(words), 'loses': len(words-preferred_wins),
                                        'loss_percent': 100*len(words-preferred_wins)/len(words)}
    save(target, result)
    print(json.dumps(result, indent=2), flush=True)


def habits_set(a,dataset):
    error = load('base_habits', 'training/eval/error_analysis.py')
    selected = cases(OUT/f'inputs/evaluation/dev_{dataset}.txt', False, a.n)
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
        oracles={}
        for mode in ('engine','app'):
            metrics = {}
            for name, items in groups.items():
                cmd = command(a, config, mode == 'app')
                target = OUT / f"habits-{config['name']}-{dataset}-{mode}-{name}.json"
                outputs = batch(a, cmd, [k+'<enter>' for _,k in items], target)
                reference=OUT/f"evaluation/dev/{config['name']}-{mode}-{dataset}.json"
                if name=='clean' and reference.exists():
                    recorded=json.loads(reference.read_text())
                    if [r['gold'] for r in recorded['cases']] != [w for w,_ in items] or recorded['outputs'] != outputs:
                        save(target.with_name(target.stem+'-mismatch.json'),{'outputs':outputs,'reference':recorded})
                        raise ValueError('clean habit differs from frozen dev capture; refuse mixed comparisons')
                # Engine oracle is measured without neural models, as in error_analysis.py.
                if name not in oracles:
                    oracles[name] = batch(a, command(a,config,False)+['--report-nbest','16'], [k for _,k in items], target)
                oracle=oracles[name]
                metrics[name] = {'n':len(items), 'exact':100*sum(w==o for (w,_),o in zip(items,outputs))/len(items),
                                 'oracle16':100*sum(w in o.split('\t') for (w,_),o in zip(items,oracle))/len(items)}
                save(target, {'gold':[w for w,_ in items],'outputs':outputs,'oracle':oracle})
            save(OUT/f"habits-{config['name']}-{dataset}-{mode}.json",metrics)
            print(config['name'],dataset,mode,json.dumps(metrics),flush=True)


def habits(a):
    for dataset in ('colloquial','wikipedia'):
        habits_set(a,dataset)


def habit_pairs(a):
    matrix=json.loads(a.matrix.read_text())
    for dataset in ('colloquial','wikipedia'):
        for mode in ('engine','app'):
            summary=json.loads((OUT/f"habits-{matrix[0]['name']}-{dataset}-{mode}.json").read_text())
            for rule in summary:
                paths=[];gold=None
                for config in matrix:
                    path=OUT/f"habits-{config['name']}-{dataset}-{mode}-{rule}.json"
                    record=json.loads(path.read_text())
                    if gold is not None and gold!=record['gold']:raise ValueError('habit rows do not pair')
                    gold=record['gold']
                    metrics=paired({'cases':[{'gold':w,'dataset':dataset,'habit':rule} for w in gold],
                                    'outputs':record['outputs']})
                    path=path.with_suffix('.jsonl')
                    path.write_text(''.join(json.dumps(r)+'\n' for r in metrics));paths.append(path)
                for path in paths[1:]:
                    for norm in ('raw','variants'):
                        result=stats.compare(stats.read_results(paths[0],norm),stats.read_results(path,norm),normalization=norm)
                        save(path.with_name(path.stem+f'-paired-{norm}.json'),result)


def checks(a):
    comparisons(a)
    available=int(re.search(r'MemAvailable:\s+(\d+)',Path('/proc/meminfo').read_text())[1])*1024
    budget=min(a.jobs,max(2,int((available-16*1024**3)/(1.1*1024**3))))
    audit_args=argparse.Namespace(**{**vars(a),'jobs':max(1,budget//2)})
    habit_args=argparse.Namespace(**{**vars(a),'jobs':max(1,budget-budget//2)})
    config=json.loads(a.matrix.read_text())[-1]
    def word_audits():
        audit(audit_args,config)
        previous=OUT/'preliminary-selection.json'
        if previous.exists():
            source=json.loads(previous.read_text())['config']
            # Audit the complete McBopomofo candidate even if its lexicon is rejected.
            if digest(Path(source['lexicon']))!=digest(Path(config['lexicon'])):
                audit(audit_args,source)
    with ThreadPoolExecutor(2) as pool:
        word_future=pool.submit(word_audits)
        habit_future=pool.submit(habits,habit_args)
        word_future.result()
        habit_args.jobs=a.jobs
        habit_future.result()
    habit_pairs(a)


def comparisons(a):
    directory=OUT/'evaluation/dev'
    controls=[('baseline','lexicon'),('lexicon','lexicon_source'),('lexicon','chat_hard'),('chat_hard','chat_expected'),
              ('chat_expected','chat_generated01'),('lexicon','wiki_hard'),('wiki_hard','wiki_expected'),
              ('wiki_expected','wiki_expected_wide'),('lexicon_source','native_prior'),
              ('wiki_expected_wide','combined_chat_hard'),
              ('combined_chat_hard','combined_chat_expected'),
              ('combined_chat_expected','combined_chat_generated01'),
              ('combined_chat_hard','combined_chat_fresh_hard'),
              ('combined_chat_fresh_hard','combined_chat_fresh_expected'),
              ('combined_chat_fresh_expected','combined_chat_fresh_generated01'),
              ('combined_chat_fresh_expected','unused_fallback'),
              ('combined_chat_fresh_expected','selected_no_mc')]
    controls.append(('baseline',json.loads(a.matrix.read_text())[-1]['name']))
    for old,new in controls:
        for mode in ('engine','app'):
            for dataset in ('colloquial','wikipedia','wikipedia_context','taiwan_tongues'):
                baseline=directory/f'{old}-{mode}-{dataset}.jsonl'
                target=directory/f'{new}-{mode}-{dataset}.jsonl'
                if not baseline.exists() or not target.exists():
                    raise ValueError(f'incomplete matched comparison: {baseline} / {target}')
                for normalization in ('raw','variants'):
                    result=stats.compare(stats.read_results(baseline,normalization),
                                         stats.read_results(target,normalization),normalization=normalization)
                    save(directory/f'{old}-to-{new}-{mode}-{dataset}-paired-{normalization}.json',result)
        print('Paired control',old,'->',new,flush=True)


if __name__ == '__main__':
    remote_only()
    verify_frozen()
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--phase', choices=['dev','test','audit','habits','checks','compare'],default='dev')
    p.add_argument('--matrix',type=Path,required=True)
    p.add_argument('--cli',default='.build/release/zhuyin-cli')
    p.add_argument('--jobs',type=int,default=8)
    p.add_argument('-n',type=int,default=400)
    a=p.parse_args()
    if a.jobs < 1 or a.n < 1: p.error('positive jobs and run count required')
    {'dev':conversion,'test':conversion,'audit':audit,'habits':habits,'checks':checks,'compare':comparisons}[a.phase](a)

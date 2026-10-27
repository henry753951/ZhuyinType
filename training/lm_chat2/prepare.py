"""Turn extraction, document-disjoint real holdouts, and all-source leakage filtering."""
import argparse
from collections import Counter
import hashlib
import json
import random
import re
import unicodedata
import zipfile
import xml.etree.ElementTree as ET
from opencc import OpenCC
from common import ROOT, OUT, RAW, lm, chat, save, digest, evaluation_files, blocked_text, excluded

TW = OpenCC('s2twp')
SPEAKER = re.compile(r'(?m)^([^\s：:，。！？、（）()\d]{2,22})[：:]')
STAMP = re.compile(r'\d{4}年\d{1,2}月\d{1,2}日[^\n]{0,35}\(UTC\)')
OFF_REGISTER = re.compile('貌似|掰扯|秀下限|丫的|閣下|鄙人|敝人|俺|咋|咱|麼|咩|嘅|唔|視頻|軟件|鼠標|出租車|地鐵|靠譜|性價比|點贊')


def normalize(text):
    return ' '.join(unicodedata.normalize('NFC', TW.convert(text)).split())


def valid(text):
    return (4 <= len(text) <= 1200 and len(lm.HAN.findall(text)) >= .65 * len(text)
            and not lm.BAD.search(text) and not re.search(r'\b\S+@\S+\b|\d{8,}', text))


def gazette_turns(text):
    matches = list(SPEAKER.finditer(text))
    turns = []
    for i, match in enumerate(matches):
        body = text[match.end():matches[i + 1].start() if i + 1 < len(matches) else len(text)]
        body = re.sub(r'^\s*（[^）]{0,30}）', '', body)
        body = normalize(body)
        speaker = match[1]
        # Tables, written attachments and procedural formulae are poor conversational evidence.
        if speaker == '主席' or any(s in speaker for s in ('說明', '提案', '決議', '案由', '時間', '地點', '議事', '秘書長', '出席', '列席')):
            turns.append({'speaker': speaker, 'text': '', 'position': i})
            continue
        if valid(body) and not re.search('書面質詢|附錄|附件|臨時提案|表決結果|有無異議|議事處|備質詢|進行專題報告|擬請院會|請審議案|請查照案|第[一二三四五六七八九十百]+條', body):
            turns.append({'speaker': speaker, 'text': body, 'position': i})
        else:
            turns.append({'speaker': speaker, 'text': '', 'position': i})
    return turns


def wiki_threads(raw):
    import mwparserfromhell
    threads, turns, buffer = [], [], []
    def flush():
        nonlocal turns
        if len(turns) >= 2:
            threads.append(turns)
        turns = []
    for line in raw.splitlines():
        if re.match(r'^==+', line):
            flush()
            buffer = []
            continue
        buffer.append(line)
        if not STAMP.search(line):
            continue
        content = '\n'.join(buffer)
        buffer = []
        signatures = list(re.finditer(r'\[\[(?:User|使用者|用户|用戶):([^\]|/]+)', line, re.I))
        if not signatures:
            turns.append({'speaker': '', 'text': '', 'position': len(turns)})
            continue
        signature = signatures[-1]
        # Remove the signed tail before markup stripping; do not train on usernames or timestamps.
        signature_start = content.rfind(line) + signature.start()
        content = content[:signature_start]
        content = STAMP.sub('', content)
        if re.search(r'<(?:blockquote|ref)|\{\{|https?://|^\s*[|!]', content, re.I | re.M):
            text = ''
        else:
            code = mwparserfromhell.parse(content)
            for link in list(code.filter_wikilinks()):
                if ':' in str(link.title):
                    code.remove(link)
            text = normalize(re.sub(r'(?m)^[:*#;]+', '', code.strip_code()))
            text = re.sub(r'--[^\s]{1,40}\s*$', '', text)
            text = text.strip(' -—：:')
            if not valid(text) or OFF_REGISTER.search(text) or re.search(r'User:|使用者:|「|」|“|”', text, re.I): text = ''
        turns.append({'speaker': signature[1], 'text': text, 'position': len(turns)})
    flush()
    return threads


def extract():
    records = []
    index = RAW / 'gazette-index.json'
    if index.exists():
        entries = json.loads(index.read_text())
    else:
        entries = {}
        for line in (RAW / 'gazette-agenda.jsonl').read_text().splitlines():
            metadata = json.loads(line)
            for doc in metadata.get('處理後公報網址', []):
                if doc['type'] == 'txt':
                    name = doc['url'].split('/')[-2]
                    if (RAW / 'gazette' / (name + '.txt.receipt.json')).exists():
                        entries[name] = {'id': name, 'url': doc['url'], 'metadata': metadata}
        entries = [entries[k] for k in sorted(entries)]
    for entry in entries:
        path = RAW / 'gazette' / (entry['id'] + '.txt')
        turns = gazette_turns(path.read_text())
        records.append({'source': 'gazette', 'document': entry['id'], 'url': entry['url'],
                        'metadata': entry['metadata'], 'turns': turns})
    seen = set()
    for path in sorted((RAW / 'wiki').glob('*.json')):
        if path.name.endswith('.receipt.json'): continue
        response = json.loads(path.read_text())
        if 'error' in response: raise ValueError(response['error'])
        for page in response.get('query', {}).get('pages', []):
            if page['pageid'] in seen or not page.get('revisions'): continue
            seen.add(page['pageid'])
            rev = page['revisions'][0]
            for i, turns in enumerate(wiki_threads(rev['slots']['main'].get('content', ''))):
                records.append({'source': 'wiki_talk', 'document': str(page['pageid']),
                                'thread': i, 'title': page['title'], 'revision': rev['revid'],
                                'timestamp': rev['timestamp'],
                                'url': f"https://zh.wikipedia.org/w/index.php?oldid={rev['revid']}", 'turns': turns})
    with (OUT / 'real_dialogues.jsonl').open('w') as f:
        for record in records: f.write(json.dumps(record, ensure_ascii=False) + '\n')
    counts = Counter()
    samples = {}
    rng = random.Random(20261027)
    for source in ('gazette', 'wiki_talk'):
        pool = [r for r in records if r['source'] == source]
        counts[source + '_documents'] = len({r['document'] for r in pool})
        candidates = []
        for record in pool:
            for a, b in zip(record['turns'], record['turns'][1:]):
                if a['text'] and b['text'] and a['speaker'] != b['speaker']:
                    candidates.append({'url': record['url'], 'previous': a['text'], 'target': b['text']})
        counts[source + '_pairs'] = len(candidates)
        samples[source] = rng.sample(candidates, min(20, len(candidates)))
    save(OUT / 'source_samples.json', {'counts': dict(counts), 'samples': samples})
    print(dict(counts), flush=True)


def pairs(record):
    for a, b in zip(record['turns'], record['turns'][1:]):
        if a['text'] and b['text'] and a['speaker'] != b['speaker']:
            yield a['text'], b['text']


def split():
    if (OUT / 'splits.json').exists():
        raise ValueError('holdouts already frozen; refusing to resample')
    records = [json.loads(s) for s in (OUT / 'real_dialogues.jsonl').read_text().splitlines()]
    paths = evaluation_files()
    held = blocked_text(paths)
    used = set().union(*(chat.grams(s) for s in held))
    by_source = {}
    for source in ('gazette', 'wiki_talk'):
        docs = {}
        for record in records:
            if record['source'] == source:
                docs.setdefault(record['document'], []).append(record)
        names = sorted(docs)
        random.Random(20261027).shuffle(names)
        by_source[source] = [(name, docs[name]) for name in names]
    rows = {'dev': [], 'test': []}
    reserved = {}
    provenance = []
    for source, docs in by_source.items():
        quota = 100 if source == 'wiki_talk' else 900
        used_docs = set()
        for kind in rows:
            selected = []
            for doc, group in docs:
                if doc in used_docs: continue
                used_docs.add(doc)
                reserved[source + ':' + doc] = kind
                for record in group:
                    for previous, target in pairs(record):
                        if not 4 <= len(target) <= 160 or not re.search(r'[\u4e00-\u9fff]{4,20}', target): continue
                        grams = chat.grams(previous) | chat.grams(target)
                        if grams & used or previous in held or target in held: continue
                        used.update(grams)
                        held.update((previous, target))
                        selected.append((previous, target))
                        provenance.append({'split': kind, 'source': source, 'document': doc,
                                           'url': record['url'], 'previous': previous, 'target': target})
                        if len(selected) == quota: break
                    if len(selected) == quota: break
                if len(selected) == quota: break
            if len(selected) != quota: raise ValueError(f'{source}/{kind}: only {len(selected)} eligible real turns; need {quota}')
            rows[kind].extend(selected)
    for kind, values in rows.items():
        random.Random(20261027).shuffle(values)
        (OUT / f'{kind}_dialogue.txt').write_text(''.join(a + '\t' + b + '\n' for a, b in values))
    save(OUT / 'splits.json', {'seed': 20261027, 'each': 1000, 'source_each': {'gazette': 900, 'wiki_talk': 100},
         'reserved_documents': reserved, 'provenance': provenance,
         'real_sha256': digest(OUT / 'real_dialogues.jsonl'),
         'files': {p.name: digest(p) for p in OUT.glob('*_dialogue.txt')},
         'existing_evaluation': {str(p.relative_to(ROOT)): digest(p) for p in paths}})


def prepare(variant, workers):
    out = OUT if variant == 'mixed' else OUT / 'no_generated'
    work = out / 'work'
    work.mkdir(parents=True, exist_ok=True)
    marker = work / 'prepared.json'
    if marker.exists(): raise ValueError('training corpus already prepared')
    held = blocked_text(evaluation_files() + [OUT / 'dev_dialogue.txt', OUT / 'test_dialogue.txt'])
    # Canonical variants are also blocked so 台/臺 conversion cannot bypass the guard.
    held.update(normalize(s).replace('臺', '台') for s in list(held))
    machine = lm.automaton(held)
    splits = json.loads((OUT / 'splits.json').read_text())
    reserved = splits['reserved_documents']
    directory = work / 'sentences'
    directory.mkdir(exist_ok=True)
    handles = [(directory / f'{i:03d}.txt').open('w') for i in range(workers)]
    counts, chars, seen = Counter(), set(), set()
    contexts = (work / 'context_pairs.tsv').open('w')
    context_seen = set()
    def add_context(previous, target):
        if any(excluded(s, held, machine) or excluded(normalize(s).replace('臺', '台'), held, machine)
               for s in (previous, target)): return
        key = hashlib.blake2b((previous + '\t' + target).encode(), digest_size=16).digest()
        if key in context_seen: return
        context_seen.add(key)
        contexts.write(previous + '\t' + target + '\n')
        counts['context_pairs'] += 1
    def add(text, source):
        text = chat.normalized(text)
        counts[source + '_input'] += 1
        if not text or excluded(text, held, machine) or excluded(normalize(text).replace('臺', '台'), held, machine):
            counts[source + '_heldout'] += 1
            return
        key = hashlib.blake2b(text.encode(), digest_size=16).digest()
        if key in seen:
            counts[source + '_duplicates'] += 1
            return
        seen.add(key)
        assert '\t' not in text and '\n' not in text
        handles[counts['training_sentences'] % workers].write(text + '\n')
        counts['training_sentences'] += 1
        counts[source + '_kept'] += 1
        chars.update(text)
    try:
        for line in (OUT / 'real_dialogues.jsonl').open():
            record = json.loads(line)
            if record['source'] + ':' + record['document'] in reserved: continue
            for previous, target in pairs(record): add_context(previous, target)
            for turn in record['turns']:
                for sentence in lm.SPLIT.split(turn['text']):
                    sentence = sentence.strip()
                    if 4 <= len(sentence) <= 200: add(sentence, record['source'])
        for line in (ROOT / 'data/build/corpus/train_commonvoice.txt').open():
            add(line.strip(), 'commonvoice')
        if variant == 'mixed':
            # Provenance of v3/colloquial excludes TT: it was synthetic + Common Voice only.
            for line in (ROOT / 'data/build/v3/colloquial.txt').open():
                add(line.strip(), 'old_colloquial')
            synthetic = OUT / 'synthetic_dialogues.jsonl'
            if synthetic.stat().st_size:
                review = json.loads((OUT / 'generation-review.json').read_text())
                if review.get('approved_sha256') != digest(synthetic):
                    raise ValueError('generated corpus lacks a matching sample-quality review')
            for line in synthetic.open():
                turns = json.loads(line)['turns']
                for previous, target in zip(turns, turns[1:]): add_context(previous, target)
                for text in turns: add(text, 'new_generated')
    finally:
        contexts.close()
        for handle in handles: handle.close()
    save(marker, {**counts, 'training_characters': sorted(chars)})
    print(dict(counts), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase', choices=['extract', 'split', 'prepare'])
    p.add_argument('--variant', choices=['mixed', 'real'], default='mixed')
    p.add_argument('--workers', type=int, default=24)
    a = p.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    if a.phase == 'extract': extract()
    elif a.phase == 'split': split()
    else: prepare(a.variant, a.workers)

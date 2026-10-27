"""Cache licensed real sources, raw responses and SHA256 receipts. Bounded retries."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import datetime
import hashlib
import json
import random
import re
import time
from urllib.parse import urlencode, quote
from urllib.request import Request, urlopen
from urllib.error import HTTPError
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'data/build/lm_chat2'
RAW = ROOT / 'data/raw/lm_chat2'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

REV = '16860178b7fbf234201db7d5c030f3fbab869a7a'
WIKI = 'https://zh.wikipedia.org/w/api.php'


def fetch(url, path, license):
    path.parent.mkdir(parents=True, exist_ok=True)
    receipt = path.with_suffix(path.suffix + '.receipt.json')
    if receipt.exists():
        old = json.loads(receipt.read_text())
        if old['url'] != url or digest(path) != old['sha256']:
            raise ValueError(f'cache mismatch: {path}')
        return path.read_bytes()
    for attempt in range(3):
        try:
            req = Request(url, headers={'User-Agent': 'ZhuyinType-corpus/1.0 (open-source Taiwan IME; research)'})
            with urlopen(req, timeout=90) as response:
                body = response.read()
                headers = dict(response.headers)
            break
        except Exception as error:
            if attempt == 2:
                raise
            delay = max(60, int(error.headers.get('Retry-After', '60'))) if isinstance(error, HTTPError) and error.code == 429 else 2 ** attempt
            print(f'{url}: retry after {delay}s: {error}', flush=True)
            time.sleep(delay)
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_bytes(body)
    tmp.replace(path)
    save(receipt, {'url': url, 'license': license, 'sha256': digest(path),
                   'bytes': len(body), 'retrieved_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   'headers': {k: v for k, v in headers.items() if k.lower() in ('etag', 'last-modified', 'content-type')}})
    return body


def government(limit):
    fetch('https://v2.ly.govapi.tw/', RAW / 'lyapi-license.html', 'CC BY 4.0 data / BSD code')
    fetch('https://data.gov.tw/license', RAW / 'ogdl.html', 'Taiwan OGDL v1')
    url = f'https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda/resolve/{REV}/'
    fetch(url + 'README.md', RAW / 'gazette-README.md', 'CC BY 4.0')
    body = fetch(url + 'gazette_agenda-11.jsonl', RAW / 'gazette-agenda.jsonl', 'CC BY 4.0')
    docs = {}
    for line in body.decode().splitlines():
        row = json.loads(line)
        for doc in row.get('處理後公報網址', []):
            if doc['type'] == 'txt':
                docs[doc['url']] = row
    items = sorted(docs.items())
    random.Random(20261027).shuffle(items)
    items = items[:limit]
    def download(item):
        url, row = item
        name = url.split('/')[-2]
        fetch(url, RAW / 'gazette' / (name + '.txt'), 'CC BY 4.0; Legislative Yuan original OGDL v1')
        return {'id': name, 'url': url, 'metadata': row}
    with ThreadPoolExecutor(1) as pool:
        records = list(pool.map(download, items))
    save(RAW / 'gazette-index.json', records)


def wikipedia(batches):
    fetch('https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use', RAW / 'wikimedia-terms.html', 'CC BY-SA 4.0')
    # Require substantial talk text instead of downloading thousands of assessment-only stubs.
    continuation = {}
    for i in range(batches):
        params = dict(action='query', generator='allpages', gapnamespace=1, gaplimit=50,
                      gapminsize=8000, gapfilterredir='nonredirects',
                      prop='revisions', rvprop='ids|timestamp|content', rvslots='main', format='json', formatversion=2)
        params.update(continuation)
        body = fetch(WIKI + '?' + urlencode(params), RAW / 'wiki' / f'large-{i:04d}.json', 'CC BY-SA 4.0; Chinese Wikipedia contributors')
        response = json.loads(body)
        if 'error' in response: raise ValueError(response['error'])
        continuation = response.get('continue')
        if not continuation: break
        if i % 10 == 0:
            print('wiki batches', i + 1, flush=True)
        time.sleep(10)


def similar():
    url = 'https://file.data.gov.tw/' + quote('行政院資料開放諮詢小組會議紀錄.json')
    rows = json.loads(fetch(url, RAW / 'data-gov-16827.json', 'Taiwan OGDL v1'))
    # Inspect an original ODT, not the accompanying slide decks or a third-party transcript.
    row = next(r for r in rows if r['會議紀錄連結'].strip().endswith('.odt'))
    url = row['會議紀錄連結'].strip().replace('http:', 'https:')
    url = quote(url, safe=':/%')
    fetch(url, RAW / 'data-gov-16827-sample.odt', 'Taiwan OGDL v1')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('source', choices=['government', 'wikipedia', 'similar'])
    p.add_argument('--documents', type=int, default=800)
    p.add_argument('--batches', type=int, default=100)
    a = p.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    if a.source == 'government': government(a.documents)
    elif a.source == 'wikipedia': wikipedia(a.batches)
    else: similar()

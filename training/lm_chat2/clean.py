"""Validate whole generated dialogues, then deduplicate/template-cap their turns."""
from collections import Counter
import json
import re
from common import ROOT, RAW, OUT, save, digest
from prepare import normalize, valid


def main():
    from datasketch import MinHash, MinHashLSH
    # Same name-slot/number normalization and family definition as training/v3/prepare.py.
    name = re.compile(r'(?:(?<=我叫)|(?<=我是)|(?<=名字是)|(?<=找)|(?<=聯絡))[陳林黃張李王吳劉蔡楊許鄭謝洪郭邱曾廖賴徐周葉蘇莊呂江何蕭羅高潘簡朱鍾游彭][\u4e00-\u9fff]{1,2}(?=[，。！？、：\s]|$)|[小阿][明華美玲芳強豪偉傑婷君志翔]|\b[A-Z][a-z]{1,15}\b')
    mainland = re.compile('視頻|軟件|網絡|出租車|地鐵|自行車|打印|鼠標|充電寶|靠譜|性價比|小區|點贊|\ufffd')
    exact, signatures = set(), {}
    families, counts = Counter(), Counter()
    lsh = MinHashLSH(threshold=.8, num_perm=64)
    samples = []
    with (OUT / 'synthetic_dialogues.jsonl').open('w') as out:
        for line in (RAW / 'generated.jsonl').open():
            row = json.loads(line)
            counts['responses'] += 1
            choice = row['response']['choices'][0]
            if choice['finish_reason'] != 'stop':
                counts['truncated'] += 1
                continue
            try:
                body = choice['message']['content'].strip()
                body = re.sub(r'^```(?:json)?\s*|\s*```$', '', body)
                dialogs = json.loads(body)['dialogues']
                if not isinstance(dialogs, list): raise ValueError('dialogues is not a list')
            except (ValueError, KeyError, TypeError):
                counts['invalid_json'] += 1
                continue
            for number, turns in enumerate(dialogs):
                if (not isinstance(turns, list) or not 4 <= len(turns) <= 12
                        or not all(isinstance(t, str) for t in turns)):
                    counts['invalid_dialogue'] += 1
                    continue
                counts['raw_turns'] += len(turns)
                if any(mainland.search(t) or not valid(t) or len(t) > 80 or
                       re.match(r'^[AB甲乙][：:]', t) for t in turns):
                    counts['quality_rejected_dialogues'] += 1
                    continue
                turns = [normalize(t).replace('臺', '台') for t in turns]
                kept = []
                for text in turns:
                    templ = re.sub(r'[0-9０-９一二三四五六七八九十百千萬億]+', '#', text)
                    templ = re.sub(r'[\W_]+', '', name.sub('@', templ))
                    family = templ[:6] + ':' + templ[-4:]
                    if templ in exact or families[family] >= 100:
                        counts['duplicate_or_template_turns'] += 1
                        kept.append(None)
                        continue
                    grams = {templ[i:i + 5] for i in range(max(1, len(templ) - 4))}
                    mh = MinHash(num_perm=64)
                    mh.update_batch([g.encode() for g in sorted(grams)])
                    if any(len(grams & signatures[n]) / len(grams | signatures[n]) >= .8 for n in lsh.query(mh)):
                        counts['near_duplicate_turns'] += 1
                        kept.append(None)
                        continue
                    index = len(signatures)
                    signatures[index] = grams
                    lsh.insert(index, mh)
                    exact.add(templ)
                    families[family] += 1
                    kept.append(text)
                # Preserve original adjacency: never join across a rejected turn.
                run = []
                for text in kept + [None]:
                    if text is not None:
                        run.append(text)
                    else:
                        if 4 <= len(run) <= 12:
                            record = {'request': row['id'], 'dialogue': number, 'turns': run}
                            out.write(json.dumps(record, ensure_ascii=False) + '\n')
                            counts['kept_dialogues'] += 1
                            counts['kept_turns'] += len(run)
                            if len(samples) < 20: samples.append(record)
                        run = []
    save(OUT / 'generation-quality.json', {**counts, 'samples': samples,
         'raw_sha256': digest(RAW / 'generated.jsonl'), 'clean_sha256': digest(OUT / 'synthetic_dialogues.jsonl'),
         'family_cap': 100, 'max_family': max(families.values(), default=0),
         'limit': 'MinHash retrieval is probabilistic; exact Jaccard confirms candidates. Mechanical filtering does not certify every spelling.'})
    print(dict(counts), flush=True)


if __name__ == '__main__': main()

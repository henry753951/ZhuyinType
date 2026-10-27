"""Freeze source/version/license/hash rows and the reused checkpoints without changing them."""
import json
from pathlib import Path
import shutil
from common import ROOT, RAW, OUT, save, digest, lm, chat


def main():
    inputs = OUT / 'inputs'
    inputs.mkdir(exist_ok=True)
    paths = [ROOT / 'data/build/v3/colloquial.txt', ROOT / 'data/build/v3/colloquial.json',
             ROOT / 'data/build/corpus/train_commonvoice.txt', ROOT / 'data/build/corpus/splits.json',
             ROOT / 'data/build/lm_chat/inputs/lexicon.tsv', Path(lm.__file__), Path(chat.__file__)]
    weights = sorted((ROOT / 'work2/Qwen3-8B').glob('*.safetensors'))
    files = {str(p.relative_to(ROOT)): {'sha256': digest(p), 'bytes': p.stat().st_size} for p in paths + weights}
    qwen_license = ROOT / 'work2/Qwen3-8B/LICENSE'
    if not qwen_license.exists(): qwen_license = ROOT / 'data/build/lm_chat/Qwen3-8B-LICENSE'
    shutil.copyfile(qwen_license, inputs / 'Qwen3-8B-LICENSE')
    receipts = {str(p.relative_to(ROOT)): json.loads(p.read_text()) for p in sorted(RAW.rglob('*.receipt.json'))}
    documents = {}
    for line in (OUT / 'real_dialogues.jsonl').open():
        row = json.loads(line)
        key = row['source'], row['document']
        record = {k: row[k] for k in ('source', 'document', 'url')}
        if row['source'] == 'wiki_talk':
            record.update({k: row[k] for k in ('revision', 'timestamp', 'title')})
            record['history_url'] = f"https://zh.wikipedia.org/w/index.php?curid={row['document']}&action=history"
            record['license'] = 'CC BY-SA 4.0'
        else:
            record['original_pdf'] = row['metadata']['公報完整PDF網址']
            record['date'] = row['metadata']['會議日期']
            record['license'] = 'OpenFun CC BY 4.0; Legislative Yuan OGDL v1'
        documents[key] = record
    with (OUT / 'attribution.jsonl').open('w') as f:
        for key in sorted(documents): f.write(json.dumps(documents[key], ensure_ascii=False) + '\n')
    save(OUT / 'source-manifest.json', {'files': files, 'raw_receipts': receipts,
         'commonvoice': json.loads((ROOT / 'data/build/corpus/splits.json').read_text()),
         'generator_revision': 'b968826d9c46dd6066d109eabc6255188de91218',
         'generator_license_sha256': digest(inputs / 'Qwen3-8B-LICENSE'),
         'gazette_metadata_revision': '16860178b7fbf234201db7d5c030f3fbab869a7a',
         'licenses': {'gazette': 'CC BY 4.0 (OpenFun conversion); OGDL v1 (Legislative Yuan originals)',
                      'wiki_talk': 'CC BY-SA 4.0; Chinese Wikipedia contributors', 'commonvoice': 'CC0 1.0',
                      'old_synthetic': 'Qwen3-8B Apache-2.0 weights; no output-use restrictions',
                      'Taiwan_Tongues': 'EXCLUDED from both new training variants; existing baseline/evaluation only'},
         'real_dialogues_sha256': digest(OUT / 'real_dialogues.jsonl')})
    (OUT / 'NOTICE.md').write_text(
        '# ZhuyinType chat2 language models\n\n'
        'Model TSVs and the combined derived corpus: CC BY-SA 4.0, '
        'https://creativecommons.org/licenses/by-sa/4.0/ . Code: AGPL-3.0-only.\n\n'
        'Attribution: Legislative Yuan, Taiwan (2024–2025 gazettes), and OpenFun Ltd., '
        'LYAPI https://v2.ly.govapi.tw/ (CC BY 4.0). Original government data: '
        'https://data.gov.tw/license (OGDL v1). Gazette agenda metadata: '
        'https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda . '
        'attribution.jsonl retains original gazette links, dates and document IDs.\n\n'
        'Chinese Wikipedia contributors, talk-page revisions fetched 2026-10-27 (CC BY-SA 4.0). '
        'attribution.jsonl retains titles, page IDs, revision IDs and exact revision links. '
        'For attribution/history use https://zh.wikipedia.org/w/index.php?curid=PAGE_ID&action=history . '
        'Terms: https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content .\n\n'
        'Common Voice sentence-collector contributors, CC0 1.0; exact commit and URL in source-manifest.json '
        'and inputs/local-input-manifest.json. Legacy Qwen synthetic text uses the existing project corpus; '
        'Qwen3-8B weight license is retained in inputs/Qwen3-8B-LICENSE. New generator pilots failed sample '
        'review and are not training data. Taiwan Tongues is excluded from both new model variants.\n\n'
        'Seed lexicon: ZhuyinType, CC BY-SA 4.0; CC-CEDICT (CC BY-SA 4.0), Unihan '
        '(Unicode License v3), wordfreq data (CC BY-SA 4.0); preserve the existing component notices.\n\n'
        'Changes: speaker/thread extraction, removal of signatures/markup/quoted or procedural material, '
        'OpenCC s2twp, document holdouts and 8-character exclusions, exact deduplication, three-pass '
        'Viterbi segmentation, supplemental turn-boundary word counts, MKN smoothing and pruning. '
        'Retain this notice, source-manifest.json, attribution.jsonl and the component notices when '
        'redistributing. Neither the Legislative Yuan nor OpenFun nor Wikimedia endorses this model.\n')
    print('manifest frozen:', len(files), 'reused files,', len(receipts), 'raw receipts', flush=True)


if __name__ == '__main__': main()

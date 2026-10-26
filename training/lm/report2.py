#!/usr/bin/env python3
"""Summarize validated artifacts with the reproducible first 15 of 50 sampled lines."""
import json
import pipeline
from pathlib import Path
root=Path(__file__).resolve().parents[2]; out=root/'data/build/lm2'
s=json.loads((out/'stats.json').read_text()); m=json.loads((out/'lm.json').read_text())
synthetic=Path(s['config']['synthetic_input'])
lines=['# ZhuyinType mixed-corpus trigram report','',
       'Model and lexicon: CC BY-SA 4.0. Code: AGPL-3.0-only. Python training/audit report; independent Swift measurements, when available, are in EXPERIMENT.md.', '',
       '## Sources and corpus sizes','',
       '- Chinese Wikipedia contributors, 2026-10-01 snapshot: CC BY-SA 4.0; article/revision attribution retained in `articles.tsv.gz`.',
       f"- Wikipedia training: {s['corpus']['wikipedia_sentences']:,} sentences, after removal of all four held-out sets and their 8-character overlaps.",
       '- Common Voice zh-TW `sentence-collector.txt`: CC0-1.0; exact source/policy URLs and commit are in `lm.json` and `stats.json`.',
       f"- Common Voice training: {s['corpus']['train_commonvoice.txt']:,} sentences; colloquial dev/test: 2,000/3,000. Existing Wikipedia test: 5,000; new Wikipedia dev: 2,000.",
       f"- Synthetic input `{synthetic}`: {s['corpus'][synthetic.name]:,} clean sentences. Qwen/Qwen3-8B, revision `b968826d9c46dd6066d109eabc6255188de91218`; exact checkpoint LICENSE verified Apache-2.0, without output-use restriction.",
       '- Grid: 200 topics × 8 registers × 10 personas × 4 lengths; temperature .95. Exact and 64-permutation MinHash dedup; held-out substring exclusion.',
       '- Strict OpenCC s2t identity can discard valid Taiwan variants such as 吃 and 台. Common Voice read prompts are not exclusively natural chat.', '',
       '## Dev selection','', '| Colloquial repeat k | Wikipedia dev PPL | Colloquial dev PPL | Token-weighted union PPL |', '|---:|---:|---:|---:|']
for c in s['candidates']:
    lines.append(f"| {c['colloquial_weight']} | {c['dev']['dev_wikipedia']['trigram_perplexity']:.3f} | {c['dev']['dev_colloquial']['trigram_perplexity']:.3f} | {c['dev_union_perplexity']:.3f} |")
lines += ['',f"Selected k={s['selected_colloquial_weight']}; no test tuning. One frozen tokenizer for every model order/candidate.", '',
          '## Held-out evaluation','', '| Test | Unigram PPL | Bigram PPL | Trigram PPL | LM OOV | Lexicon OOV |', '|---|---:|---:|---:|---:|---:|']
for name in ['test_wikipedia','test_colloquial']:
    e=s['evaluation'][name]
    lines.append(f"| {name} | {e['unigram_perplexity']:.3f} | {e['bigram_perplexity']:.3f} | {e['trigram_perplexity']:.3f} | {e['model_oov_rate']:.5%} | {e['lexicon_oov_rate']:.3%} |")
lines += ['', 'These are the selected trigram model’s lower-order components. EOS is scored; BOS is context-only. Punctuation/fallback characters count toward lexicon OOV.', '',
          '## Model, lexicon and verification','',
          f"- lm.bin: {s['binary']['bytes']:,} bytes ({s['binary']['bytes']/1e6:.2f} decimal MB), {s['binary']['vocabulary']:,} words, {s['binary']['bigrams']:,} bigrams, {s['binary']['trigrams']:,} trigrams.",
          f"- SHA256: `{m['sha256']}`.",
          f"- unknown_word_shift: {m['unknown_word_shift']:.12f}; seen lexicon words recalibrated, unseen rows keep seed scale and use this shift once.",
          f"- {s['mining']['added']:,} new lexicon rows; frequency >=200, split PMI >=2, boundary entropy >=.6; certain per-character readings or unique/Taiwan CC-CEDICT readings. See `new_words.tsv`.",
          '- Initial lexicon Viterbi plus two hard-EM updates; MKN via KenLM. Pruning count thresholds 0/8/12; cap explicit trigram probabilities if needed, then recompute backoffs.',
          '- 台 preference applied before MKN for 90% of training sentences, retaining original 臺 spellings for 10%; measured unigram comparisons are in `validation.json`.',
          '- Full corpus equality/8-character exclusion rescan; original reading rows and polyphone odds preserved; binary structural/sorted-id/backoff/normalization checks passed.',
          f"- Training pipeline wall time: {s['timings']['pipeline_seconds']:.1f} s; final audit: {s['validation']['seconds']:.1f} s.",
          '', '## 15 of the 50 sampled synthetic sentences','',
          'Uniform reservoir sample over the selected frozen synthetic input, seed 20261027. Full sample: `generated_samples_50.txt`.', '']
lines += [f'{i}. {line}' for i,line in enumerate((out/'generated_samples_50.txt').read_text().splitlines()[:15],1)]
lines += ['', '## Continuing generation','',
          'Host: `ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`; directory: `~/zhuyintype/`.',
          'Clean phase two appends to `data/build/corpus/synthetic_background.txt`, targeting 6M total (phase one included). Raw requests append to `data/build/corpus/raw/synthetic.jsonl`.',
          'Progress: `cat data/build/corpus/clean-progress.json`; logs: `logs/generation.log`, `logs/clean.log`, `logs/vllm.log`.',
          'Stop: create `data/build/corpus/STOP_GENERATION`, wait for recorded generation PID to exit, then create `STOP_CLEANER`. After the cleaner exits, optionally `kill "$(cat logs/vllm.pid)"`.',
          'Resume: remove both stop files, ensure server is running, then restart the nohup generator/cleaner commands in `training/corpus/README.md`. Do not start duplicates. Recorded PIDs: `logs/{generation,clean,vllm}.pid`.',
          '', 'This Python stage does not check the macOS Swift reader, conversion accuracy, or latency; see EXPERIMENT.md for any separate engine measurements. LSH can miss near-duplicates; synthetic fluency and strict orthographic filtering remain corpus limitations.', '']
(out/'REPORT.md').write_text('\n'.join(lines))
print(out/'REPORT.md')

s['files']['REPORT.md']={'bytes':(out/'REPORT.md').stat().st_size,'sha256':pipeline.digest(out/'REPORT.md')}
pipeline.save_json(out/'stats.json',s)

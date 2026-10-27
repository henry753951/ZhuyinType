# Compatible continuation — 2026-10-27

**Recommendation: do not adopt.** The licensed continuation improves aggregate
在／再 accuracy but still outputs 在試試看 for 再試試看 and introduces
我再吃一碗麵 for the previously correct 我在吃一碗麵. These are explicit development
acceptance failures, regardless of aggregate significance. The targeted retry and
one parameter-midpoint attempt do not preserve both eating examples either.
The no-adoption decision was sealed before this run's test scoring; test only tracks.

The delivered experimental bundle is `data/build/v3_homophone2/`, selected training
step **500**, with both fp16 packages/compiled models, original vocabularies,
configuration, vectors, PyTorch checkpoints, source notices and attribution.
It is license-clean and interface-validated, **not a completed production fix**.
The app, engine, release assets and original `data/build/v3` remain unchanged by this task.

Checkpoint SHA256: `67955335d9d4b16fbc8faa329d12d71f87917eadec59690e268b0775af8ba51e`. Original starting checkpoint: `03c27edd7632e6fa650b656691219dd4dc3a7cc7e52a3fc1d7efb38905ffd133`; never the TRAIL-trained checkpoint.

## Sources and exclusions

No Taiwan Tongues source text enters either fine-tune stream. TT remains evaluation
only. Reuse immutable Wikipedia/colloquial shards and cached chat2 real sources;
no new generated pilot text or downloaded resampling. All paths below are relative
to the repository; remote access uses `$ZHUYIN_REMOTE_HOST` / `$ZHUYIN_REMOTE_KEY`.

| Eligible source pool | Candidates / input | Retained | License |
| --- | --- | --- | --- |
| Wikipedia + deduplicated legacy colloquial shards | 16,950,352 indexed examples | 5,407,644 | Wikipedia CC BY-SA 4.0; legacy Qwen3-8B / Common Voice lineage |
| Legislative Yuan gazettes | 690,984 | 148,939 | OGDL v1; OpenFun LYAPI CC BY 4.0 |
| Wikipedia talk | 17,474 | 6,916 | CC BY-SA 4.0 |
| Common Voice sentence collector | 8,882 | 6,114 | CC0 1.0 |
| Taiwan Tongues | Evaluation only | 0 training | Not a model source |

Reject **11,542,708** indexed examples by whole-document exclusion. The
supplement has **161,969 internally deduplicated clauses** (not an additional
unique count across the legacy CV-containing shards). Independent rescan:
**3,777,130 retained documents**, 5,407,644 indexed examples and 161,969
supplemental clauses, **0 violations**. Filtering took 118.04 s.

All 38 frozen evaluation files block heldout eight-character substrings and short
actual sentences, including context/target joins, homophone dev/test, old TT
holdouts, chat2 dialogue dev/test, smoke and every imported shanjie text. Check
original, 台/臺-folded and encoded-UNK forms. Short context alone contributes
only eight-grams. This is continuation exclusion; original pretraining may already
contain later-created holdouts. Evaluation files remain byte-identical.

Legacy colloquial/Common Voice source hashes match the compatible chat2 manifest.
Agent reading of 24 deterministic initial 在/再 samples found no evident primary
label error; one awkward inherited Wiki phrase was noted. This is not independent
human annotation. Initial sampling also includes fixed words such as 再生能源.
See `inputs.json`, `source-integrity.json`, `source-review.json`, `filtered.json`
and `leakage-audit.json` under `data/build/homophone2/`.

Licenses/attribution: [model NOTICE](../../data/build/v3_homophone2/NOTICE.md),
`articles.tsv.gz`, `attribution.jsonl`, `source-manifest.json`, Qwen3-8B Apache-2.0
license and the existing lexicon/Unicode/wordfreq notices are retained in the bundle.

## Training and rejected follow-ups

Original recipe: frozen context, AdamW 2e-5 (100-step warmup), batch 64,
30% focused batches, half explicitly balanced primary 在/再, 3x confusion-position
loss, unchanged augmentation and 30% context dropout. Four data workers.
The compatible run replaces the old TT supplement with a 25% real-source draw in
normal and explicitly primary-focused batches. Half the latter focus on initial
characters. Check every 500 steps; select lowest primary dev NLL only if all eight
original general-dev conditions lose at most 0.2 percentage points; stop after
four non-improving checks. A 10,000-step cap was never reached.

| Step | Primary dev NLL | Conditional primary accuracy | General gate | Selected |
| --- | --- | --- | --- | --- |
| 500 | 0.185696 | 92.89% | True | yes |
| 1000 | 0.340019 | 87.37% | True |  |
| 1500 | 0.204008 | 92.51% | True |  |
| 2000 | 0.219059 | 91.45% | True |  |
| 2500 | 0.310261 | 88.86% | True |  |

Stop at 2,500; selected 500; training time 201.41 s. Baseline NLL 0.514428; conditional primary accuracy 82.56% → 92.89%. These model-only unions inject gold and are not decoder recall.

| General model-only condition | Positions | Original | Continued | Delta pp |
| --- | --- | --- | --- | --- |
| colloquial-False-0.0 | 15837 | 95.05% | 95.43% | +0.38 |
| colloquial-False-0.05 | 15837 | 94.85% | 95.20% | +0.35 |
| colloquial-True-0.0 | 15837 | 94.86% | 95.19% | +0.33 |
| colloquial-True-0.05 | 15837 | 94.65% | 94.98% | +0.33 |
| wikipedia-False-0.0 | 60462 | 94.95% | 95.01% | +0.06 |
| wikipedia-False-0.05 | 60462 | 94.76% | 94.82% | +0.06 |
| wikipedia-True-0.0 | 60462 | 95.02% | 95.10% | +0.09 |
| wikipedia-True-0.05 | 60462 | 94.84% | 94.92% | +0.09 |

The targeted retry reuses exactly these allowed data, focusing on initial 再
before verbs (109 shard + 187 real examples) and subject + 在 + verb
(750 + 76). A prefix-only preparation (49 + 28 for 在) was
archived before training; allowing preceding clause text corrected that coverage.
Three known requested examples are model-only development gates, excluded from
gradient training and not unseen confirmation evidence.
Retry stop: 3,000; selected 1,000; 249.25 s.

| Retry step | NLL | General + guard admissible | 再試 / 我再吃 / 我在吃麵 guards |
| --- | --- | --- | --- |
| 500 | 0.252763 | False | True / True / False |
| 1000 | 0.346597 | True | True / True / True |
| 1500 | 0.251791 | False | True / True / False |
| 2000 | 0.197358 | False | True / True / False |
| 2500 | 0.210847 | False | True / True / False |
| 3000 | 0.328445 | False | True / True / False |

The midpoint uses original/continued decoder parameters equally, identical frozen
context and vocabularies, no additional gradient training and no coefficient sweep.
All eight original general gates pass; conditional primary accuracy
89.29%, NLL 0.292398.
Its full dev primary accuracy is 88.38% / 92.80% without/with context, versus
85.69% / 91.07% baseline, but it retains the same protected eating regression.
Neither follow-up opens the homophone test sets.

A fast-probe mistake is retained explicitly: `conversion_eval --reranker` injects
reranker weight 1, whereas today's frozen CLI default is 0.5. Early fast probes
are **not app-configuration evidence**. Corrected probes pin 0.5 and reproduce the
failures below. All full matrices used 0.5 throughout; the adapter now pins it.

| Known dev target | Current v3 | Full continuation | Targeted retry (corrected) | Midpoint (corrected) |
| --- | --- | --- | --- | --- |
| 再試試看 | 在試試看 | 在試試看 | 在試試看 | 在試試看 |
| 我再吃一碗 | exact | exact | 我在吃一碗 | exact |
| 我在吃一碗麵 | exact | 我再吃一碗麵 | exact | 我再吃一碗麵 |

The two short three-character rows are absent from the 16-run conversion probes;
the formal homophone smoke includes all 18 rows / 19 primary positions.
Full-continuation smoke: 18/19 → 17/19 primary positions; 16/18 → 15/18 whole sentences.
在啦幹 → 在拉幹 remains a non-primary spelling error. A model-only correct 再試試看
is not end-to-end success: it survives as n-best #2, raw scores -21.401834 (在)
versus -22.013088 (再), neural scores -21.117 versus -20.917; the fixed app stack
still commits 在. No engine/reranker/chat-prior change is made to hide this failure.

## Current-app paired evaluation

Same frozen engine and data throughout: reranker count 16, **reranker weight 0.5**,
v3 weight 0.5, chat LM `data/build/lm_chat` at prior 0.3, default gate τ=8,
standard keyboard/typo handling. Linux ONNX CPU via a temporary copy of
`training/remote_eval/run.sh`, `REMOTE_DIR=zhuyintype/work/onnx-eval-homophone3`.
JOBS=24 for baseline dev, 40 for candidate/full final rows, never above 40.
No selected checkpoint changes after the dev decision. Do not compare these
numbers directly with historical experiment 1's different engine/configuration.

Paired statistics use `training/eval/stats.py`: exact McNemar, 10,000 paired
bootstrap samples, seed 7; homophone bootstrap clusters by source/line,
other sets by row. Category tests are exploratory; McNemar itself is unclustered.
Position scoring uses edit alignment (deletion wrong); whole-sentence accuracy is exact.
Category priority remains fixed word > subject > initial > verb > place/time > other.

### Primary dev

| Subset | N | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- |
| All | 1041 | 85.69% | 90.68% | 91.07% | 94.33% |
| Non-fixed | 842 | 82.90% | 89.07% | 89.55% | 93.47% |
| 在 | 536 | 100.00% | 100.00% | 100.00% | 100.00% |
| 再 | 505 | 70.50% | 80.79% | 81.58% | 88.32% |
| Fixed word | 199 | 97.49% | 97.49% | 97.49% | 97.99% |
| After subject | 158 | 94.30% | 94.94% | 94.94% | 94.94% |
| Initial | 163 | 62.58% | 79.14% | 85.28% | 93.87% |
| Before verb | 156 | 91.03% | 93.59% | 94.23% | 94.87% |
| Before place/time | 157 | 94.90% | 95.54% | 94.90% | 96.18% |
| Other | 208 | 75.00% | 84.13% | 81.25% | 88.94% |
| Whole sentence | 1000 | 67.10% | 71.30% | 74.20% | 76.70% |

| Context | Subset | N | Old → new | Fixed/broken | Delta pp | 95% CI pp | p |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | all | 1041 | 85.69% → 90.68% | 52/0 | +5.00 | [+3.71, +6.36] | 4.441e-16 |
| 0 | char_category:再:initial | 118 | 48.31% → 71.19% | 27/0 | +22.88 | [+15.38, +30.77] | 1.49e-08 |
| 0 | char_category:在:after_subject | 145 | 100.00% → 100.00% | 0/0 | +0.00 | [+0.00, +0.00] | 1 |
| 1 | all | 1041 | 91.07% → 94.33% | 34/0 | +3.27 | [+2.21, +4.41] | 1.164e-10 |
| 1 | char_category:再:initial | 118 | 79.66% → 91.53% | 14/0 | +11.86 | [+5.98, +17.95] | 0.0001221 |
| 1 | char_category:在:after_subject | 145 | 100.00% → 100.00% | 0/0 | +0.00 | [+0.00, +0.00] | 1 |

### Primary test

| Subset | N | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- |
| All | 1051 | 87.44% | 91.53% | 92.20% | 94.67% |
| Non-fixed | 851 | 85.08% | 90.01% | 90.83% | 93.89% |
| 在 | 543 | 99.82% | 99.82% | 99.82% | 99.82% |
| 再 | 508 | 74.21% | 82.68% | 84.06% | 89.17% |
| Fixed word | 200 | 97.50% | 98.00% | 98.00% | 98.00% |
| After subject | 157 | 94.90% | 94.90% | 95.54% | 95.54% |
| Initial | 165 | 63.03% | 78.79% | 83.03% | 90.30% |
| Before verb | 154 | 93.51% | 95.45% | 92.86% | 96.10% |
| Before place/time | 159 | 96.23% | 97.48% | 97.48% | 98.74% |
| Other | 216 | 80.56% | 85.65% | 87.04% | 90.28% |
| Whole sentence | 1000 | 68.30% | 71.50% | 74.80% | 77.00% |

| Context | Subset | N | Old → new | Fixed/broken | Delta pp | 95% CI pp | p |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | all | 1051 | 87.44% → 91.53% | 43/0 | +4.09 | [+2.92, +5.35] | 2.274e-13 |
| 0 | char_category:再:initial | 118 | 48.31% → 70.34% | 26/0 | +22.03 | [+14.41, +29.66] | 2.98e-08 |
| 0 | char_category:在:after_subject | 144 | 100.00% → 100.00% | 0/0 | +0.00 | [+0.00, +0.00] | 1 |
| 1 | all | 1051 | 92.20% → 94.67% | 26/0 | +2.47 | [+1.54, +3.45] | 2.98e-08 |
| 1 | char_category:再:initial | 118 | 76.27% → 86.44% | 12/0 | +10.17 | [+5.08, +16.10] | 0.0004883 |
| 1 | char_category:在:after_subject | 144 | 100.00% → 100.00% | 0/0 | +0.00 | [+0.00, +0.00] | 1 |

### General dev and external rows

| Set | N | Exact | Variants | Fixed/broken exact | Exact delta CI pp | p |
| --- | --- | --- | --- | --- | --- | --- |
| colloquial | 400 | 87.00% → 87.00% | 87.00% → 87.00% | 4/4 | [-1.25, +1.50] | 1 |
| wikipedia | 400 | 83.75% → 84.00% | 83.75% → 84.00% | 4/3 | [-1.00, +1.50] | 1 |
| chat_tt | 400 | 71.25% → 70.75% | 85.50% → 85.00% | 0/2 | [-1.25, +0.00] | 0.5 |
| colloquial_smoke | 68 | 100.00% → 100.00% | 100.00% → 100.00% | 0/0 | [+0.00, +0.00] | 1 |
| dev302 | 266 | 81.58% → 81.20% | 84.21% → 83.83% | 2/3 | [-2.26, +1.13] | 1 |
| typing76 | 49 | 83.67% → 85.71% | 83.67% → 85.71% | 1/0 | [+0.00, +6.12] | 1 |

Shanjie common-character normalization, dev302: 86.09% → 85.71%. Shanjie common-character normalization, typing76: 95.92% → 95.92%. No external full variants dictionary is supplied. Clean rows contain 266/49 examples, not the original 302/76. Wiki uses line context; each large general set uses 400 runs, seed 7; colloquial smoke has 68 runs. No significant general improvement or noninferiority claim follows from these small samples.

### Secondary groups

| Split | Group | Positions | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- | --- |
| dev | 做作 | 243 | 83.95% | 84.36% | 85.19% | 84.77% |
| dev | 已以 | 263 | 92.02% | 92.78% | 93.54% | 93.92% |
| dev | 他她它 | 307 | 65.80% | 65.15% | 72.64% | 71.34% |
| dev | 像向象項 | 176 | 89.20% | 88.64% | 90.91% | 90.34% |
| dev | 須需 | 130 | 74.62% | 73.85% | 74.62% | 72.31% |
| dev | 帳賬 | 115 | 48.70% | 49.57% | 51.30% | 48.70% |
| dev | 份分 | 128 | 67.19% | 67.97% | 67.97% | 67.19% |
| dev | 的得 | 1029 | 96.21% | 96.40% | 96.60% | 96.60% |
| dev | 吧巴 | 133 | 94.74% | 93.98% | 96.99% | 96.24% |
| dev | 嗎嘛 | 110 | 92.73% | 92.73% | 92.73% | 92.73% |
| dev | 那哪 | 105 | 99.05% | 99.05% | 99.05% | 99.05% |
| dev | 到道 | 231 | 97.84% | 97.40% | 97.84% | 97.40% |
| dev | 是事 | 386 | 94.30% | 95.08% | 94.30% | 95.34% |
| dev | 時實 | 296 | 90.88% | 90.54% | 90.88% | 91.55% |
| dev | 試式 | 176 | 75.00% | 75.00% | 77.27% | 77.84% |
| dev | 受授 | 127 | 96.06% | 96.85% | 95.28% | 96.85% |
| dev | 權全 | 169 | 95.27% | 95.27% | 95.27% | 95.27% |
| dev | 利力 | 218 | 91.74% | 91.28% | 88.99% | 88.99% |
| dev | 治制 | 156 | 86.54% | 86.54% | 87.82% | 88.46% |
| dev | 記紀 | 123 | 79.67% | 77.24% | 82.93% | 82.93% |
| dev | 竟競 | 51 | 92.16% | 92.16% | 92.16% | 92.16% |
| dev | 帶戴 | 122 | 89.34% | 90.16% | 90.16% | 89.34% |
| dev | 副付 | 105 | 88.57% | 88.57% | 91.43% | 90.48% |
| dev | 坐座 | 98 | 87.76% | 88.78% | 88.78% | 89.80% |
| dev | 候後 | 187 | 98.93% | 98.93% | 98.93% | 98.93% |
| dev | 里裡 | 157 | 85.35% | 87.90% | 86.62% | 89.81% |
| dev | ALL | 5341 | 88.50% | 88.62% | 89.48% | 89.48% |
| dev | NON-FIXED | 3475 | 85.27% | 85.32% | 86.59% | 86.71% |
| test | 做作 | 245 | 84.49% | 84.49% | 85.31% | 85.31% |
| test | 已以 | 253 | 93.28% | 92.89% | 93.28% | 93.68% |
| test | 他她它 | 308 | 65.58% | 66.23% | 72.08% | 72.08% |
| test | 像向象項 | 192 | 91.15% | 91.67% | 92.71% | 93.23% |
| test | 須需 | 140 | 75.71% | 78.57% | 77.14% | 77.14% |
| test | 帳賬 | 111 | 49.55% | 53.15% | 54.05% | 57.66% |
| test | 份分 | 123 | 69.11% | 66.67% | 67.48% | 66.67% |
| test | 的得 | 1026 | 95.61% | 95.61% | 95.81% | 95.91% |
| test | 吧巴 | 138 | 98.55% | 98.55% | 98.55% | 98.55% |
| test | 嗎嘛 | 110 | 96.36% | 95.45% | 96.36% | 96.36% |
| test | 那哪 | 108 | 98.15% | 98.15% | 98.15% | 98.15% |
| test | 到道 | 258 | 96.12% | 96.12% | 97.67% | 97.67% |
| test | 是事 | 395 | 92.41% | 92.91% | 93.16% | 93.42% |
| test | 時實 | 295 | 91.53% | 92.54% | 93.90% | 94.58% |
| test | 試式 | 176 | 79.55% | 80.11% | 82.39% | 82.95% |
| test | 受授 | 135 | 95.56% | 95.56% | 97.04% | 97.04% |
| test | 權全 | 181 | 92.27% | 92.27% | 94.48% | 95.03% |
| test | 利力 | 196 | 86.22% | 86.73% | 87.76% | 88.27% |
| test | 治制 | 151 | 85.43% | 87.42% | 88.74% | 88.74% |
| test | 記紀 | 131 | 81.68% | 81.68% | 83.21% | 83.21% |
| test | 竟競 | 57 | 91.23% | 91.23% | 89.47% | 89.47% |
| test | 帶戴 | 129 | 88.37% | 87.60% | 86.82% | 88.37% |
| test | 副付 | 110 | 91.82% | 90.91% | 93.64% | 92.73% |
| test | 坐座 | 100 | 93.00% | 94.00% | 95.00% | 95.00% |
| test | 候後 | 195 | 98.97% | 98.97% | 98.97% | 98.97% |
| test | 里裡 | 180 | 85.00% | 85.00% | 86.67% | 86.67% |
| test | ALL | 5443 | 88.65% | 88.92% | 89.95% | 90.19% |
| test | NON-FIXED | 3553 | 85.39% | 85.70% | 87.22% | 87.50% |


| Secondary split | Context | Fixed/broken | Delta pp | 95% CI pp | p |
| --- | --- | --- | --- | --- | --- |
| dev | 0 | 27/21 | +0.11 | [-0.13, +0.37] | 0.4709 |
| dev | 1 | 25/25 | +0.00 | [-0.26, +0.26] | 1 |
| test | 0 | 31/16 | +0.28 | [+0.04, +0.53] | 0.03999 |
| test | 1 | 23/10 | +0.24 | [+0.04, +0.44] | 0.03508 |

Full group/category counts and ordered predictions are retained in `results/*.json`; paired category/character intervals in `paired-*.positions.json`, whole-sentence/variant/CER statistics in `paired-*.json`. Secondary changes include both gains and regressions.

## Core ML and real-output parity

| Model | Max logprob error | Max memory error | Max probability error | Synthetic argmax flips |
| --- | --- | --- | --- | --- |
| Original | 0.072990417 | 0.089843750 | 0.013366759 | 2/504 |
| Delivered continuation | 0.082813263 | 0.093750000 | 0.010225266 | 5/504 |
| Rejected midpoint | 0.073307037 | 0.093750000 | 0.007131398 | 4/504 |

Same upstream fp16 exporter, macOS 14 target, Xcode coremlcompiler via
`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`. The original 20 vector
inputs are unchanged; expected outputs come from each Core ML package. Swift
`--check-v3`: **20/20**, largest difference **4.33680869e-19**. This is an interface
check, not an independent PyTorch numerical check. The saved-output audits above
are independent and use the original v3 as calibration; all delivered cases also
meet the historical 0.1 numerical bound. ONNX/PyTorch: 20 cases, max difference
**0.0000181198**, shape/padding/finite/normalization checks pass.

The single-use real-output check uses 397 frozen Wiki inputs plus the three known
critical smoke inputs (400 total), all with saved Linux ONNX outputs. The original
control uses the full 400 Wiki inputs: **396/400 (99.00%)** identical. Delivered
candidate: **395/400 (98.75%)**. On the 397 shared Wiki inputs, original is
**393/397 (98.99%)**, candidate **392/397 (98.74%)**. Only one divergence overlaps;
three old ones disappear and four new ones appear. Wiki exact-correct counts:
original **332 Linux / 332 Mac**, candidate **333 Linux / 329 Mac**. Thus four
Linux-correct Wiki decisions become Mac-wrong for this candidate. All **53**
primary target positions in shared Wiki runs and the **3** critical outputs agree
across platforms; the critical outputs still include both known errors. Parity
is not accuracy approval. Full-stack comparison includes the unchanged reranker's
fp16/fp32 differences too; do not attribute every difference to v3 alone.

Artifacts: `mac-*-parity.json`, `mac-parity-summary.json`, `coreml-*-audit.json`,
`onnx-chosen-audit.json`, `check-v3.log`, `export-manifest.json`. Binary lexicon and
both LMs were rebuilt in an experiment-only Mac directory; all three hashes match
Linux. No Mac corpus selection/training, app installation/UI or latency test.

## Execution and integrity

Baseline test SSH closed after its first completed row; the remote driver continued
through three complete raw prediction rows, then stopped while reporting progress.
No last-row start log existed. Audited recovery verified the sealed manifest,
CLI/model/input hashes and raw commands, retained the three predictions, rebuilt
missing sidecars and decoded only the never-started final row. No sealed decode
was replayed. The compatible dispatcher now uses nohup, a new session, PID receipt
and logs in `~/zhuyintype/logs/`; recovery refuses an already-started incomplete row.
Both final completion seals are retained. Final `/proc` inspection finds no owned
evaluator/model-server processes in the isolated workspace (`final-process-audit.json`).
No model/test retuning follows.

Sampled host MemAvailable stayed at least 63 GiB; this is not a continuous peak
measurement. No owned OOM or broad process-name kill. Initial slow sync was stopped
by verified local rsync PID before evaluation; hash-identical remote weights seeded
the workspace. Initial regression-test failures were missing old-layout remote
helper files; syncing the current evaluator dependencies fixed them.

Remote: Python 3.12.15, Torch 2.13.0+cu130, ONNX Runtime 1.24.4, Swift 6.4.1,
uv 0.13.5. Mac: Python 3.12.14, Torch 2.7.0, coremltools 9.0, NumPy 2.3.5,
Swift 6.4. The new isolated venv reuses existing site-packages read-only; no
package/tool installs or lockfile/global-default changes. Nine inherited and
four focused regression checks pass, including source routing, paired identity,
cluster bootstrap, test guards, nohup dispatch and incomplete-row replay refusal.

Code changes are confined to `training/homophone/`; no engine/client/docs/release
workflow/asset-manifest edits, app deployment, Git commit/push/stash/reset/checkout
by this task. Other workspace changes are preserved. Reproduction: [README.md](README.md).

The observed gains support further research, not promotion of this checkpoint:
keep the current v3 until the protected conversational regression and the remaining
initial 再 failure are resolved under the same exact app configuration.

---

# Historical experiment 1

# Homophone continuation report

Dev-selected candidate: **unchanged v2b + continued v3, weight 1, rerank count 16**.
**Core ML numerical gate failed:** one synthetic case exceeds the
inherited fp32 tolerance (0.118900 > 0.1), with four top-1 differences across 504
positions. The compiled artifacts are not approved for promotion. PyTorch/ONNX
quality results below remain valid; no checkpoint or test choice was changed.
Dev choice was sealed at 2026-10-27 02:19:52 UTC, before test/smoke evaluation.
All **32 end-to-end rows** are complete: 22 dev + ten sealed final rows. Both final
roles exit 0 and their completion seals are retrieved. Test/smoke ran once per
role/context; smoke has no duplicated context run. No model change follows test.

The selected model is `data/build/v3_homophone/`, step **2,500**; checkpoint SHA256:
`c9fce9e1d2f7a245b9aad96e57d7bd1f16d048dcda8d623e7d08630806072d4e`.
Both compiled Core ML models and the original PyTorch checkpoint layout are present.
No app deployment, engine edits or upstream trainer edits were made. The only
existing-file exception is the authorized portable-runner extension; concurrent
work by other engineers is preserved. No dependency installs or Git mutations.

## Data and label quality

The zero-candidate bug compared list readings with tuple inventory readings.
Canonical tuples fix the shared matcher; regression covers both native and JSON
inventory. The repaired final mine has **40,937 clauses / 27 groups** (120.9 s).
Of 11,172 primary-bearing candidates, Qwen3-8B independently accepted 9,319 and
rejected 1,853; 29,765 secondary-only rows need no primary judgment. Total accepted:
39,084. Identical completed judgments were cached. Both inherited and additional
judge servers were stopped by verified PID immediately after their passes.

Mining preserves complete 4–31-Han clauses, preceding Wikipedia document context,
source line/offset, toned readings and category flags. Counts include Wikipedia,
deduplicated colloquial and Taiwan Tongues zh-TW **training TSV**; evaluation clauses
come only from Wikipedia and TT. No TT test.tsv or lm_chat holdout is mined.

| Set | Clauses | Wikipedia / TT | 在 / 再 positions |
| --- | --- | --- | --- |
| dev_zai | 1000 | 959 / 41 | 536 / 505 |
| test_zai | 1000 | 962 / 38 | 543 / 508 |
| dev_pairs | 2482 | 2172 / 310 | secondary only |
| test_pairs | 2480 | 2155 / 325 | secondary only |

Twenty-two secondary groups reach 100 clauses per split; 帶戴 94/93, 副付 79/78,
坐座 68/68, 竟競 41/41 fall short after overlap/typing filters. All 18 requested smoke
sentences are retained. Each split is allocated jointly to avoid exhausting 再.

Manual reading of 100 final source clauses and contexts (105 primary positions)
found **0 evident 在/再 label errors**. This was agent reading, not an independent
human annotator. Five clear category issues were flagged, not an exhaustive category
error-rate estimate. Categories remain heuristic: fixed lexicon word > recognized
subject > initial > before verb > before place/time > other. Non-fixed is the
interesting subset. `manual-review.json` and `review100.jsonl` preserve the audit.

Continuation excludes every old/new heldout 8-gram, short actual sentence and smoke
substring, checking original and UNK-normalized text, context and context-target
joins. Short context alone contributes only 8-grams. Whole contaminated documents
are removed: **6,102,901 allowed / 10,847,451 rejected examples**, plus 352 confident
unique TT training clauses. Original pretraining may already contain the newly
mined holdouts: this is continuation exclusion, not retroactively clean pretraining.
Primary evaluation is predominantly Wikipedia, limiting conversational coverage.

## Frequency inventory

Counts are source-text occurrences and longest-match standalone/word-initial counts before continuation exclusions. Shared toned readings come from the frozen lexicon. Counts do not claim every occurrence uses the shared reading.

| Group | Shared readings | Text counts | Standalone / initial counts |
|---|---|---|---|
| 在／再 | ㄗㄞˋ | 在: 5,782,456; 再: 459,529 | 在: 5,305,658; 再: 418,517 |
| 做／作 | ㄗㄨㄛˋ | 做: 269,713; 作: 1,806,707 | 做: 253,980; 作: 902,413 |
| 已／以 | ㄧˇ | 已: 515,536; 以: 2,884,291 | 已: 491,416; 以: 1,919,251 |
| 他／她／它 | ㄊㄚ | 他: 2,697,768; 她: 562,963; 它: 367,010 | 他: 2,393,382; 她: 562,963; 它: 341,256 |
| 像／向／象／項 | ㄒㄧㄤˋ | 像: 884,654; 向: 577,770; 象: 188,497; 項: 185,261 | 像: 163,080; 向: 375,703; 象: 53,321; 項: 124,927 |
| 須／需 | ㄒㄩ | 須: 123,242; 需: 277,446 | 須: 39,626; 需: 245,906 |
| 帳／賬 | ㄓㄤˋ | 帳: 102,966; 賬: 8,535 | 帳: 83,779; 賬: 6,685 |
| 份／分 | ㄈㄣˋ | 份: 268,744; 分: 1,585,078 | 份: 124,443; 分: 947,770 |
| 的／得 | ㄉㄜ˙ | 的: 18,122,380; 得: 1,972,512 | 的: 17,213,505; 得: 656,125 |
| 吧／巴 | ㄅㄚ | 吧: 83,108; 巴: 556,854 | 吧: 75,990; 巴: 445,953 |
| 嗎／嘛 | ㄇㄚˊ, ㄇㄚ˙ | 嗎: 670,593; 嘛: 12,124 | 嗎: 669,882; 嘛: 3,486 |
| 那／哪 | ㄋㄚˇ, ㄋㄨㄛˊ | 那: 528,851; 哪: 83,846 | 那: 496,590; 哪: 83,753 |
| 到／道 | ㄉㄠˋ | 到: 2,362,568; 道: 1,157,795 | 到: 1,018,032; 道: 476,527 |
| 是／事 | ㄕˋ | 是: 5,635,283; 事: 1,221,959 | 是: 3,995,229; 事: 535,721 |
| 時／實 | ㄕˊ | 時: 2,996,826; 實: 629,691 | 時: 1,881,949; 實: 411,886 |
| 試／式 | ㄕˋ | 試: 417,437; 式: 863,702 | 試: 227,289; 式: 219,706 |
| 受／授 | ㄕㄡˋ | 受: 582,169; 授: 169,690 | 受: 406,406; 授: 83,100 |
| 權／全 | ㄑㄩㄢˊ | 權: 354,150; 全: 833,866 | 權: 152,966; 全: 613,603 |
| 利／力 | ㄌㄧˋ | 利: 1,029,994; 力: 891,784 | 利: 405,523; 力: 163,487 |
| 治／制 | ㄓˋ | 治: 612,762; 制: 521,437 | 治: 160,841; 制: 185,854 |
| 記／紀 | ㄐㄧˋ | 記: 580,621; 紀: 417,210 | 記: 365,554; 紀: 205,868 |
| 竟／競 | ㄐㄧㄥˋ | 竟: 27,128; 競: 104,034 | 竟: 19,617; 競: 86,770 |
| 帶／戴 | ㄉㄞˋ | 帶: 565,655; 戴: 101,832 | 帶: 379,557; 戴: 90,431 |
| 副／付 | ㄈㄨˋ | 副: 225,983; 付: 103,491 | 副: 224,121; 付: 51,754 |
| 坐／座 | ㄗㄨㄛˋ | 坐: 95,778; 座: 307,423 | 坐: 80,558; 座: 235,582 |
| 候／後 | ㄏㄡˋ | 候: 353,365; 後: 2,708,109 | 候: 55,908; 後: 1,767,906 |
| 里／裡 | ㄌㄧˇ | 里: 545,848; 裡: 677,075 | 里: 202,956; 裡: 539,540 |

## Training and dev selection

Continue from original v3 (`03c27edd...ffd133`), freeze context, retain original
augmentation/context dropout, batch 64, learning rate 2e-5, 30% focused batches,
3x confusion-position loss. Half of focused sampling targets balanced 在/再;
eligible TT supplies 25% of that stream. Four non-improving 500-step checks stop
training at 4,500; best step 2,500. Runtime: 515.53 s. Selection is primary dev NLL
subject to every original general condition losing <=0.2 percentage points.

Primary model-only dev improves **82.565% -> 92.747%** (both context conditions,
uncapped gold-containing candidate unions; conditional selection, not decoder recall).
All eight original general per-position conditions improve:

| Set / context / noise | Positions | Original | Continued | Delta pp |
| --- | --- | --- | --- | --- |
| colloquial-False-0.0 | 15837 | 95.05% | 95.16% | +0.1137 |
| colloquial-False-0.05 | 15837 | 94.85% | 94.93% | +0.0758 |
| colloquial-True-0.0 | 15837 | 94.86% | 94.96% | +0.1010 |
| colloquial-True-0.05 | 15837 | 94.65% | 94.70% | +0.0442 |
| wikipedia-False-0.0 | 60462 | 94.95% | 95.04% | +0.0926 |
| wikipedia-False-0.05 | 60462 | 94.76% | 94.85% | +0.0959 |
| wikipedia-True-0.0 | 60462 | 95.02% | 95.13% | +0.1125 |
| wikipedia-True-0.05 | 60462 | 94.84% | 94.95% | +0.1108 |

End-to-end dev confusion-position accuracy, frozen portable engine, JOBS=60.
Columns 0/1 mean without/with line context. Original v3 alone is diagnostic on
primary dev; the optional secondary-alone rows are omitted (14 baseline + eight
continued-model rows total).

| Stack | Primary 0 | Primary 1 | Secondary 0 | Secondary 1 |
| --- | --- | --- | --- | --- |
| engine | 78.58% | 78.58% | 83.62% | 83.64% |
| v2b | 85.78% | 91.35% | 89.03% | 89.78% |
| v3 | 80.98% | 81.84% | — | — |
| v2b-v3 | 84.63% | 89.91% | 89.14% | 90.49% |
| v3-new | 90.97% | 89.91% | 86.69% | 88.26% |
| v2b-v3-new | 90.97% | 94.72% | 89.42% | 90.96% |

Non-fixed primary dev mean across contexts improves **84.857% -> 91.449%**.
With continued v3, reranking introduces 24/94 residual errors without context and
5/55 with context; 70/50 are also wrong without reranking. Reranking rescues 24/55.
Neither condition has mostly reranker-caused residual errors, so conditional v2b
continuation is **not triggered**; reuse checkpoint `e7f5ebcf...081a53`.

General end-to-end dev, exact whole-sentence counts, n=400 per set:

| Stack | Colloquial | Wikipedia |
| --- | --- | --- |
| engine | 302/400 (75.50%) | 285/400 (71.25%) |
| v2b | 335/400 (83.75%) | 319/400 (79.75%) |
| v3 | 327/400 (81.75%) | 301/400 (75.25%) |
| v2b-v3 | 337/400 (84.25%) | 320/400 (80.00%) |
| v3-new | 321/400 (80.25%) | 298/400 (74.50%) |
| v2b-v3-new | 337/400 (84.25%) | 320/400 (80.00%) |

Complete-stack correct counts are unchanged, although 14/17 saved predictions
change. Continued v3 alone declines on general whole-sentence accuracy and is not
the selected configuration. No test/smoke result is used for model selection.

## Sealed primary test

1,000 clauses, 1,051 primary positions. Same fixed models/weights as dev; each
role/context runs once. Percentages below use edit-aligned confusion positions;
deletions are wrong. Whole-sentence accuracy requires exact text equality.

| Subset | N | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- |
| All positions | 1051 | 86.96% | 91.06% | 91.63% | 94.77% |
| Non-fixed | 851 | 84.49% | 89.31% | 90.13% | 93.77% |
| 在 | 543 | 99.82% | 99.63% | 99.82% | 99.82% |
| 再 | 508 | 73.23% | 81.89% | 82.87% | 89.37% |
| Fixed lexicon word | 200 | 97.50% | 98.50% | 98.00% | 99.00% |
| After subject | 157 | 94.27% | 95.54% | 94.90% | 96.82% |
| Sentence-initial | 165 | 57.58% | 68.48% | 78.79% | 86.67% |
| Before verb | 154 | 94.81% | 96.10% | 94.81% | 96.10% |
| Before place/time | 159 | 96.86% | 98.74% | 98.11% | 98.74% |
| Other | 216 | 81.48% | 88.89% | 86.11% | 91.67% |
| Whole sentence | 1000 | 68.10% | 72.80% | 76.30% | 78.90% |

Sentence-initial remains the weakest category. Without context, 在 loses one
correct position while 再 gains 44; with context, 在 is unchanged and 再 gains 33.

## Sealed secondary test and smoke

2,480 clauses / 5,443 secondary positions, with each role/context run once.

| Subset | N | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- |
| all | 5443 | 89.22% | 89.73% | 91.00% | 91.25% |
| interesting | 3553 | 86.24% | 86.94% | 88.77% | 88.91% |
| Whole sentence | 2480 | 68.10% | 68.79% | 72.18% | 72.38% |

| Group | Positions | Old 0 | New 0 | Old 1 | New 1 |
| --- | --- | --- | --- | --- | --- |
| 做／作 | 245 | 86.12% | 87.35% | 88.16% | 86.94% |
| 已／以 | 253 | 93.28% | 92.09% | 94.86% | 94.07% |
| 他／她／它 | 308 | 65.58% | 67.21% | 72.73% | 73.70% |
| 像／向／象／項 | 192 | 91.67% | 92.19% | 93.75% | 93.75% |
| 須／需 | 140 | 77.86% | 77.14% | 73.57% | 73.57% |
| 帳／賬 | 111 | 47.75% | 47.75% | 55.86% | 57.66% |
| 份／分 | 123 | 73.98% | 74.80% | 75.61% | 76.42% |
| 的／得 | 1026 | 96.10% | 96.30% | 96.49% | 96.59% |
| 吧／巴 | 138 | 97.83% | 98.55% | 98.55% | 98.55% |
| 嗎／嘛 | 110 | 94.55% | 94.55% | 96.36% | 96.36% |
| 那／哪 | 108 | 98.15% | 98.15% | 98.15% | 98.15% |
| 到／道 | 258 | 96.90% | 96.51% | 98.45% | 98.84% |
| 是／事 | 395 | 93.42% | 93.92% | 94.43% | 94.43% |
| 時／實 | 295 | 91.19% | 92.54% | 94.24% | 94.24% |
| 試／式 | 176 | 81.82% | 81.25% | 86.36% | 85.80% |
| 受／授 | 135 | 94.07% | 95.56% | 97.04% | 97.78% |
| 權／全 | 181 | 91.71% | 92.82% | 94.48% | 95.58% |
| 利／力 | 196 | 87.24% | 86.22% | 88.78% | 88.27% |
| 治／制 | 151 | 86.75% | 88.74% | 89.40% | 90.07% |
| 記／紀 | 131 | 83.21% | 86.26% | 87.02% | 87.79% |
| 竟／競 | 57 | 91.23% | 91.23% | 89.47% | 91.23% |
| 帶／戴 | 129 | 89.15% | 91.47% | 89.92% | 91.47% |
| 副／付 | 110 | 90.00% | 90.91% | 90.91% | 92.73% |
| 坐／座 | 100 | 97.00% | 97.00% | 97.00% | 97.00% |
| 候／後 | 195 | 98.46% | 98.46% | 98.97% | 98.97% |
| 里／裡 | 180 | 86.67% | 87.78% | 87.78% | 89.44% |

Smoke: primary positions **17/19 -> 17/19**; whole sentence **15/18 -> 15/18**. No net improvement. These results were not used to retune or reselect. All other smoke rows are exact in both stacks; the union of failures is:

| Input / expected | Original stack | Continued stack |
| --- | --- | --- |
| 再試試看 | 在試試看 | 在試試看 |
| 在啦幹 | 在拉幹 | 在拉幹 |
| 我再吃一碗 | 我在吃一碗 | 我再吃一碗 |
| 我在吃一碗麵 | 我在吃一碗麵 | 我再吃一碗麵 |

The candidate fixes 我再吃一碗 but regresses 我在吃一碗麵; 再試試看 remains wrong. 在啦幹 fails on 啦/拉 outside the primary confusion. Sentence-initial and aspect-versus-repetition disambiguation are therefore still unresolved. Preserve the dev-locked checkpoint as an experiment; **do not promote these Core ML artifacts as a completed fix**.

## Latency and export checks

Remote ONNX CPU, one thread, 10 warmups + 100 calls; v3 Lc=256, Ls=30, C=32;
reranker B=16, L=30. Includes the ONNX bridge, excludes model load. Both repetitions
ran together after host load fell; no claim about Mac IME latency or a speedup.

| Call | Old p50 ms | New p50 ms | Old p95 ms | New p95 ms |
| --- | --- | --- | --- | --- |
| context | 111.2 | 113.4 | 123.6 | 123.1 |
| syllable | 30.3 | 32.1 | 36.6 | 39.7 |
| reranker | 187.3 | 191.3 | 401.6 | 431.1 |

The unchanged reranker control differs by 2.2% in median. Earlier measurements
showed a 43% control difference under heavier load; retained as `latency-{baseline,new}.json`.
Repetition reason/time/PIDs are in `latency-repeat-provenance.json`.

* Remote ONNX vs selected PyTorch: 20 dynamic interface cases, maximum valid
  logprob error 0.000023842; shape/finite/padding/normalization checks pass.
* Remote CPU vs CUDA decoder probe: all 24 generic dev predictions identical.
* Mac: export the selected checkpoint only, compile both models with Xcode
  `coremlcompiler`, regenerate golden outputs for the original 20 synthetic
  interface inputs, then run the frozen CLI's **single** `--check-v3`: 20/20 pass,
  largest difference 1.301e-18. These golden vectors are Core ML outputs, not an
  independent PyTorch/Core ML numerical comparison. Only vector format/package
  hash metadata and example.json were added afterward; vector inputs/expected
  stayed byte-semantically identical, recorded in `export-manifest.json`.
  No Mac corpus evaluation,
  training, or latency benchmark. See `check-v3.log` and `export-manifest.json`.
* Nine remote regression checks passed; no upstream tests/trainers were modified.
  Runtime: Python 3.12.15, Torch 2.13.0+cu130, NumPy 2.3.5, coremltools 9.0;
  portable ONNX Runtime 1.24.4, Linux Swift 6.4.1, H100L-3-47C MIG.

Additional saved-output comparison runs entirely on Linux. The first strict
Core ML/PyTorch comparison stops at case 9 (logprob error 0.118900, memory 0.058594).
An explicit diagnostic scan then records all 20 cases before returning failure:
19/20 meet the original 0.1 bound; case 9 exceeds it, with no top-1 change in that
case. Four other positions (cases 11,14,15) change argmax: **500/504 agreement**.
Maximum memory error is 0.085938, maximum probability error 0.013088. Using the
identical saved Core ML memory still produces 0.126284 logprob error at case 9,
so context differences alone cannot explain it; this points to syllable FP16
execution, without identifying an individual operation. The 0.1 gate is retained.
See `coreml-pytorch-{first-failure,audit}.log` and `coreml-pytorch-audit.json`.
No further export/parity retries or model changes are made after this failure;
Mac end-to-end impact is unmeasured under the requested execution restriction.

## Execution audit and reproduction

All end-to-end runs use the portable runner, JOBS=60, frozen engine/evaluator at
`data/build/homophone/portable/`, named PyTorch checkpoint directories, dedicated
remote `work/onnx-eval-homophone-{baseline,new,general}` trees. Opt-in EXTRA_SYNC,
ENGINE_SNAPSHOT and named-model support preserve runner defaults. `remote.py`
executes a syntax-checked in-memory snapshot; `runner-pinned.sh` retains the known
good pre-refactor runner after concurrent live paths moved to nlp/engine.
The 19 engine source hashes plus Package.swift are verified; binary hashes differ
by build workspace and are recorded in each result. No Mac CLI rebuild occurs;
frozen CLI SHA256 `ba2e7030...9b1fb9`. Existing Mac `general-*.log` are untouched.

Rejected attempts and corrections are retained, not silently retried:

* Partial regex clauses, dev-first 再 exhaustion, whitespace context and dangling
  final 在 were found before final data lock. Prior splits/judgments remain in
  `audit-first-split/` and `audit-before-manual-qc/`.
* Short punctuation context wrongly excluded most training documents. Stop the
  preliminary 300+ step run by exact PID tree; archive its indexes/weights remotely
  under `data/build/homophone/audit-context-exclusion/`. It is never selected.
  UNK decoding now preserves one character per encoded unknown.
* Another experiment overwrote the default portable workspace. Stop our process
  tree, retain seven certified rows, discard two uncertifiable rows, and compute
  only missing dev rows in dedicated workspaces. Fresh sync initially missed
  Package.swift due to rsync /./ behavior; syncing from the snapshot directory fixes
  it. In-memory runner execution avoids concurrent Bash-source rereads.
* Kernel logs confirm shared-host **RAM OOM**, not GPU exhaustion, killed one dev
  scorer. Archive the failed row under `audit-host-oom/`; resume only unfinished dev
  rows after diagnosing the cause. RAM admission gates then guard worker startup.
  Baseline final uses 64 GiB, chosen 80 GiB; final roles overlap only after available
  RAM recovers to 103 GiB. Admission cannot prevent unrelated later memory pressure.
* First Core ML metadata save targeted the source package and coremltools deleted
  it before copying weights. Save to a separate package then rename; re-export the
  same selected checkpoint. No parity/test prediction was rerun for this repair.

Raw measurements are in `results/`, `general-{baseline,new}/`, `v3-training.json`,
`diagnosis-new.json`, `onnx-parity-new.json` and `remote-*.log` under
`data/build/homophone/`. `final-inputs.json`, `final-integrity.json`, source checks
and export-input hashes identify the frozen inputs/models. Model output retains
source notices/licenses and Wikipedia attribution. See README.md for commands.

`cleanup.json` records no owned active process and no GPU compute process at
2026-10-27 03:24:29 UTC. Both final runner invocations exit 0. The numerical audit
intentionally exits nonzero on its retained 0.1 gate. No further model work is running.

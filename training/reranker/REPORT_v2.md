# Reranker v2 experiment

Experiment started 2026-10-26; completed 2026-10-27 (Asia/Taipei). The Results section records the final status.

## Protocol

Train from seed 7 with the existing trainer and unchanged v1 defaults (batch 512, learning rate 0.001, bf16, context 64, vocabulary 12,000). Separate remote `reranker_v2a/`, `reranker_v2b/` and `reranker_v2c/` directories contain byte-identical vocabulary copies and symlinks to v1 token streams. This experiment did not change data preparation or application sources; the trainer and exporter are unchanged. The existing train.py, export_coreml.py and prepare.py matched SHA-256 on both hosts, so scp was unnecessary.

Use the existing remote `.venv-gen/bin/python` (Python 3.12.15, Torch 2.13.0+cu130, NumPy 2.3.5, coremltools 9.0). The specified `.venv` lacks Torch; no packages were installed. GPU: H100L-3-47C / MIG 3g.47gb (46,587 MiB).

Local evaluation uses Python 3.12.14 via uv 0.12.7; the CLI was built with Swift 6.4. All variants were exported to Core ML fp16.

Build with `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex --product zhuyin-cli` (passed). Freeze that executable and the existing evaluator under `data/build/reranker_v2_experiment/` to isolate comparisons from concurrent source edits. Hashes and selection rules: `manifest.json` there. The default developer directory lacks `coremlcompiler`; all compilation uses the full Xcode `DEVELOPER_DIR` above. CPU only (`ZHUYIN_UNITS` unset); MacBook Pro Apple M5, 32 GB, macOS 27.0.1.

Selection: maximize equal-weight mean of dev colloquial and Wikipedia whole-run accuracy among v2 models with slowest post-first-call CPU latency <25 ms. Ties: mean character accuracy, lower latency, then weight nearest 1.0. Evaluate weights {0.7, 1.0, 1.4} for each model and v1; reference v1 uses its shipped weight 1.0. Train v2c only if v2b final perplexity is at least 2% lower than v2a on **both** dev token streams (predeclared interpretation of “clearly beats”).

Evaluate held-out sets once per locked dev winner and shipped v1, never for tuning. Promote only if the winner strictly exceeds v1's equal-weight mean whole-run accuracy across the three held-out sets and passes latency.

Resource recovery: the first-pass integrity check found that the shared `data/build/lexicon.tsv` no longer matched its initial hash (observed modification time 19:13:51). The first pass stopped on a hash check before any held-out evaluation. `data/build/lexicon.before-priors.tsv` exactly matches the initial lexicon SHA-256 (`e7cc8a1edd019180f88ce2313715abfeb5fb5968444a5f1d4da5a756534325a1`). All first-pass dev/timing results were archived under `pre-resource-freeze/` and discarded for selection. Every dev combination and CPU timing was rerun using private frozen copies of that original lexicon, the unchanged LM/dev files, the CLI and evaluator. Test files were mechanically copied without using their outcomes for selection; held-out evaluation was run once per locked winner and v1. The shared lexicon and concurrent source edits were preserved. Frozen resource hashes are in `manifest.json` (`frozen_evaluation_sha256`).

## Commands

SSH: `ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`; remote root `~/zhuyintype`. Jobs use `nohup`; logs and exit status files are under `logs/reranker_v2*`.

```sh
# Remote; each directory has vocab.json and symlinked train.bin/val_*.bin.
.venv-gen/bin/python -u training/reranker/train.py --data reranker_v2a --steps 90000
.venv-gen/bin/python -u training/reranker/train.py --data reranker_v2b --steps 90000 --layers 8 --width 512 --heads 8 --ffn 2048
# Conditional v2c:
.venv-gen/bin/python -u training/reranker/train.py --data reranker_v2c --steps 90000 --layers 10 --width 640 --heads 10 --ffn 2560
# Per finished variant:
.venv-gen/bin/python -u training/reranker/export_coreml.py --data reranker_v2a --out reranker_v2a/Reranker.mlpackage
scp -r -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST:zhuyintype/reranker_v2a/Reranker.mlpackage data/build/reranker_v2a/
scp -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST:zhuyintype/reranker_v2a/vocab.json data/build/reranker_v2a/
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile data/build/reranker_v2a/Reranker.mlpackage data/build/reranker_v2a/
# Repeat for every finished variant, initial config {"weight": 1.0, "count": 16}.

# Local dev; DIR is v1 or each variant, W is 0.7 / 1.0 / 1.4.
env -u ZHUYIN_UNITS uv run --no-project --python 3.12 data/build/reranker_v2_experiment/conversion_eval.py --cli data/build/reranker_v2_experiment/zhuyin-cli --lexicon data/build/reranker_v2_experiment/lexicon.tsv -n 400 --reranker DIR --rerank-weight W --rerank-count 16 --test data/build/reranker_v2_experiment/dev/dev_colloquial.txt data/build/reranker_v2_experiment/dev/dev_wikipedia.txt
# CPU latency, omit first timed call:
env -u ZHUYIN_UNITS data/build/reranker_v2_experiment/zhuyin-cli --lexicon data/build/reranker_v2_experiment/lexicon.tsv --reranker DIR --rerank-count 16 --timing "ji3ap6vu35j/ru04xu4vu/5j4u4ek75k4cl3vu04xu4cl3wu4"
# Held-out, once per winner and v1 (same options and n=400):
# --test data/build/reranker_v2_experiment/test/test_sentences.txt data/build/reranker_v2_experiment/test/test_colloquial.txt data/build/reranker_v2_experiment/test/colloquial_smoke.txt
```

## Results

Status: complete.

### Training

| Model | Steps | Minutes | Wiki perplexity | Colloquial perplexity |
|---|---:|---:|---:|---:|
| v1 (historical) | 30000 | 16.71 | 29.5594 | 75.2983 |
| reranker_v2a | 90000 | 49.42 | 27.5580 | 72.2539 |
| reranker_v2b | 90000 | 79.04 | 23.3940 | 65.1481 |
| reranker_v2c | 90000 | 122.17 | 20.9905 | 60.7843 |

### Dev (400 runs per set)

| Model / weight | Colloquial char % | Colloquial whole % | Wiki char % | Wiki whole % | Whole mean % |
|---|---:|---:|---:|---:|---:|
| v1 / 0.7 | 95.9 | 80.50 | 96.3 | 76.25 | 78.375 |
| v1 / 1.0 | 95.8 | 79.00 | 96.3 | 76.50 | 77.750 |
| v1 / 1.4 | 95.9 | 79.00 | 96.1 | 75.00 | 77.000 |
| v2a / 0.7 | 96.2 | 81.75 | 96.3 | 76.00 | 78.875 |
| v2a / 1.0 | 96.2 | 81.75 | 96.2 | 74.75 | 78.250 |
| v2a / 1.4 | 96.0 | 80.50 | 96.0 | 74.00 | 77.250 |
| v2b / 0.7 | 96.3 | 82.25 | 96.4 | 76.50 | 79.375 |
| v2b / 1.0 | 96.4 | 82.25 | 96.7 | 78.25 | 80.250 |
| v2b / 1.4 | 96.3 | 81.25 | 96.7 | 78.00 | 79.625 |
| v2c / 0.7 | 96.3 | 83.00 | 96.7 | 76.75 | 79.875 |
| v2c / 1.0 | 96.3 | 82.50 | 96.8 | 77.75 | 80.125 |
| v2c / 1.4 | 96.3 | 82.25 | 96.7 | 76.75 | 79.500 |

### CPU latency

| Model | First ms (excluded) | Every subsequent call, ms | Maximum ms | Pass <25 ms |
|---|---:|---|---:|---|
| v1 | 3.7 | 2.1, 2.0, 3.1, 3.7, 4.6, 5.0, 6.0, 6.5, 7.3, 8.0, 8.8, 9.2, 10.1, 10.6 | 10.6 | yes |
| reranker_v2a | 3.8 | 2.1, 2.1, 3.2, 3.7, 4.5, 5.0, 6.0, 6.5, 7.3, 8.0, 8.7, 9.3, 10.2, 10.6 | 10.6 | yes |
| reranker_v2b | 7.0 | 4.0, 3.8, 5.6, 6.5, 8.0, 8.9, 10.3, 11.5, 12.8, 14.1, 15.6, 16.3, 17.9, 18.9 | 18.9 | yes |
| reranker_v2c | 11.5 | 6.0, 6.0, 9.0, 10.0, 12.6, 13.8, 17.0, 22.9, 21.3, 22.4, 25.7, 26.2, 28.5, 29.9 | 29.9 | no |

### Held-out test (once per model)

| Model / weight | Set | Runs | Character % | Whole-run % |
|---|---|---:|---:|---:|
| reranker_v2b-w1.0 | test_sentences.txt | 400 | 96.8 | 76.75 |
| reranker_v2b-w1.0 | test_colloquial.txt | 400 | 96.5 | 81.25 |
| reranker_v2b-w1.0 | colloquial_smoke.txt | 68 | 99.8 | 98.53 |
| reranker_v2b-w1.0 | **Equal-weight mean** | — | 97.70 | **85.510** |
| v1-w1.0 | test_sentences.txt | 400 | 96.6 | 76.50 |
| v1-w1.0 | test_colloquial.txt | 400 | 96.1 | 79.25 |
| v1-w1.0 | colloquial_smoke.txt | 68 | 99.8 | 98.53 |
| v1-w1.0 | **Equal-weight mean** | — | 97.50 | **84.760** |

### Decision

Adopt reranker_v2b, weight 1.0: held-out whole-run mean improves by 0.75 percentage points versus shipped v1; slowest post-first CPU call 18.9 ms. Winner artifacts are in data/build/reranker_v2/.

Raw evaluation output, timings, copy/compile logs, manifest and machine details: `data/build/reranker_v2_experiment/`. Training logs: remote `logs/reranker_v2*-train.log` and local variant `train-log.json`.

Character accuracy is printed by the existing evaluator to 0.1 percentage point. Whole-run counts are recovered unambiguously from the printed percentage and sample count (at most 400 runs), so whole-run means and the adoption gate use exact fractions. Latency uses the specified single typing sequence and excludes its first timed call. No broader workload, app UI or installation test was run.

The full chronological execution log, including the recovered first-pass interruption, is `data/build/reranker_v2_experiment/experiment.log`. Only the clean frozen-resource measurements above are used for selection and adoption.

Final artifact verification passed: the promoted mlpackage and mlmodelc match v2b file-for-file by SHA-256; vocabulary matches v1 (12,000 entries); config is {"weight": 1.0, "count": 16}. All frozen resources retained their hashes. Exactly two held-out commands ran, one for the locked winner and one for v1. Details: `data/build/reranker_v2_experiment/adoption-check.json`.

## Verification on the 0.2.3 engine

Re-run on 2026-10-27 with the current engine (reading priors, tone sandhi, exact-match margin 4.0, Taiwan sense
readings, <unk> share) and the shipped lexicon, weight 1.0, 16 candidates, n = 400; "context" is
`conversion_eval.py --line-context`. Whole-run accuracy:

| Model | Wikipedia | Colloquial | Everyday | Wikipedia + context | Colloquial + context |
|---|---:|---:|---:|---:|---:|
| v1 | 80.5 | 80.5 | 100.0 | 83.0 | 80.8 |
| v2a | 79.8 | 80.8 | 100.0 | 83.0 | 81.0 |
| v2b | 80.8 | 82.8 | 100.0 | 85.2 | 83.0 |

CPU time per reranking call while typing a 25-syllable sentence (`zhuyin-cli --timing`), first call excluded:
v1 2.1–11.0 ms, v2a 2.1–11.0 ms, v2b 3.8–20.9 ms.  v2b is the only clear gain (+2.2 points with context) and about
1.9× slower per call; v2a matches v1's speed without a clear gain.  Not adopted yet: the adoption rule for the app
was "better on the test sets and not slower".

# Chat2 corpus and word bigrams

Run CPU preparation/training and **all evaluations on the lab Linux host**. The
Mac can cache public downloads and transfer files. No global tools are installed.
Code is AGPL-3.0-only; model/corpus attribution is in `data/build/lm_chat2/NOTICE.md`.

**Stopped incomplete:** repeated global kernel OOM kills interrupted the 60-worker
conversion sweep (7/24 rows complete). No chat2 conversion/prior claim is established.
Wait for sufficient dedicated host capacity before resuming. A concurrent runner
layout change to `nlp/engine` also broke the retry's sync against the original
snapshot. Preserve the isolated runtime and verify its `evaluation-manifest.json`
hashes; do not mix new live sources/models into the partial comparison.

This experiment stopped new Qwen generation after two failed quality pilots.
Both pilots remain in raw storage and are excluded. `lm_chat2` means compatible
old Qwen/Common Voice text plus filtered new real transcripts; `no_generated/`
contains only the real transcripts and Common Voice. Neither contains Taiwan
Tongues training text. Do not treat the old model's TRAIL license as compatible.

## Frozen data

- Raw HTTP bodies and individual URL/date/license/SHA256 receipts:
  `data/raw/lm_chat2/`. Government API errors stop after three attempts.
  Wiki requests observe a ten-second interval and HTTP 429 Retry-After.
- `real_dialogues.jsonl`: original source document, speaker, turn order, wiki
  thread, revision and attribution links. Invalid turns remain empty barriers;
  no context is joined across them. Wiki context is the previous signed turn in
  source order, not a verified reply-tree edge.
  The inherited four-character minimum discards minimal replies; this remains
  a coverage limit for natural messaging.
- `source_samples.json`, `quality-review.json`: sample review before training.
  Summary-only data.gov.tw minutes and both generator pilots were rejected.
- `dev_dialogue.txt` / `test_dialogue.txt`: 1,000 **turn pairs** each, TSV
  `previous_turn<TAB>target_turn`; 900 gazette and 100 wiki pairs per split.
  Whole documents are reserved for one split or excluded from training. Seed,
  row-level source URLs, source hashes and existing evaluation hashes are in
  `splits.json`. Test text is never scored or used to select a prior.
- `inputs/evaluation/`: frozen existing dev/test text, including all present
  homophone `dev_*` / `test_*` TXT and JSONL files and the short smoke set.
  `inputs/local-input-manifest.json` maps back to original paths and hashes.

All training variants remove exact matches and any eight-character overlap
with **both columns** of both new holdouts and every frozen old evaluation
file. The filter also blocks OpenCC/common-台 normalized variants. A separate
post-export scan checks every retained training sentence and context pair.
`source-manifest.json` records the legacy corpus, metadata, estimator and
generator weight hashes. Preserve it and the full attribution manifest.
`attribution.jsonl` is the compact 0.49 MB redistribution manifest; the full
transcript text is not needed in the app bundle.

## Reproduce

Use the existing remote `.venv` (Python/NumPy/OpenCC/parser/Aho-Corasick) and
`.venv-gen` (vLLM/aiohttp/datasketch). Do not install into either environment.
The raw-source downloader is standard-library-only and can run on the Mac:

```sh
.venv/bin/python training/lm_chat2/sources.py government --documents 800
.venv/bin/python training/lm_chat2/sources.py wikipedia --batches 20
.venv/bin/python training/lm_chat2/sources.py similar
```

The 800-document government request stopped at an upstream HTTP 500; the
experiment freezes the 506 cached, receipt-verified documents. Do not silently
redownload/resample for a published comparison. `prepare.py extract` can build
an index from those cached receipts when a bulk request was interrupted.

After reviewing the samples on the remote host:

```sh
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
.venv/bin/python training/lm_chat2/test_pipeline.py
.venv/bin/python training/lm_chat2/prepare.py extract
# Review source_samples.json before freezing splits/counting training text.
.venv/bin/python training/lm_chat2/prepare.py split
nohup bash training/lm_chat2/run_train.sh \
  > logs/lm-chat2-train-final.log 2>&1 < /dev/null &
```

Preparation refuses to overwrite a completed corpus or resample frozen splits.
Training refuses changed inputs/code/config in an existing work tree. Preserve
old trees and choose a separate experiment before changing the data. The two
model outputs are the same natural-log `unigram.tsv`, `bigram.tsv`, `backoff.tsv`
format, with an independent 60,000,000-byte cap. Reuse `training/lm/pipeline.py`
three-pass Viterbi hard EM, 0.1 pseudocount, MKN and count-threshold pruning.
Each clean adjacent turn pair contributes one additional last-word/first-word
bigram; turn-internal sentences retain BOS/EOS events. This is a supplemental
boundary-count estimator, not a learned long-context neural model.

## Remote evaluation

Use the README setup for `training/remote_eval`, at
`~/zhuyintype/work/onnx-eval-chat2/.venv`. Its `.pth` files reuse Torch from
`.venv-gen` and the parser/OpenCC/Aho-Corasick packages from `.venv`, read-only.
Install only the runner's pinned requirements into that isolated environment.
The shared runner adds `chat2` for target-only TSV evaluation and `LOCAL_RESULTS`
to keep collected logs inside this experiment. `EXTRA_SYNC` handles new files.

`ENGINE_SNAPSHOT=data/build/lm_chat2/engine` freezes the existing engine and
conversion evaluator; their SHA256 inventory is `engine-manifest.json`.
Identical existing remote checkpoint copies can seed the isolated directory
after comparing hashes, avoiding a second network transfer.

Copy these completed directories/files to the same relative paths inside the
isolated workspace: model TSVs, `no_generated/` TSVs, both dialogue files,
`inputs/` (including the old chat tokenizer), `training/lm/pipeline.py`, and
`training/lm_chat/train.py`. For Mac-driven sync, list them in `EXTRA_SYNC`:

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-chat2 JOBS=60 BACKEND=cpu \
  ENGINE_SNAPSHOT=data/build/lm_chat2/engine \
  LOCAL_RESULTS=data/build/lm_chat2/remote-logs \
  EXTRA_SYNC="training/lm_chat2 training/lm/pipeline.py training/lm_chat/train.py data/build/lm_chat2/inputs data/build/lm_chat2/dev_dialogue.txt data/build/lm_chat2/test_dialogue.txt data/build/lm_chat2/unigram.tsv data/build/lm_chat2/bigram.tsv data/build/lm_chat2/backoff.tsv data/build/lm_chat2/no_generated/unigram.tsv data/build/lm_chat2/no_generated/bigram.tsv data/build/lm_chat2/no_generated/backoff.tsv" \
  bash -c "$(cat data/build/lm_chat2/frozen-runner.sh)" \
    training/remote_eval/run.sh chat2 --phase conversion -n 400
```

This invokes the preserved original launcher with its original script location
as `$0`, retaining the frozen snapshot's root layout despite the concurrent live
runner migration. Use only when current input files match the recorded hashes;
otherwise use a new experiment rather than reusing the seven cached rows.

The runner launches the evaluator with `nohup` and records its PID. Evaluation
uses ONNX CPU on the remote GPU host and refuses execution on macOS. The optional
`BACKEND=cuda` uses the runner's private 2 GiB batching server; it was slower under
  this run's shared load. The partial comparison uses CPU. The evaluator is configured for
the exact requested v2b count 16 / v3 weight 0.5 configuration with both chat
LMs at priors 0.3/0.5/0.7, 60 workers, 400 runs/set, seed 7. Standard sets use
the existing evaluator's selection rules; dialogue targets alone supply runs,
with preceding turn plus earlier target text as context (last 32 characters,
matching `conversion_eval.py`). Ordered keys, targets, outputs and variant-equal
accuracy are saved per row in `conversion/`. No test split is evaluated.

Perplexity uses **one frozen old-chat final-E tokenizer** for all four models,
including the ablation, on all dev lines. Include EOS, exclude BOS and context
from the denominator. Novel target/context tokens map to each model's `<unk>`;
unknown counts are reported. Dialogue scores predict only the target, seeded
by the previous final word when it ends in Han, otherwise BOS. These are not
the new model's independently retokenized training perplexities. Counts and
per-line log likelihoods allow reconstruction.

## Generator boundary

`serve.sh` uses private port 8112 and requires >=40,500 MiB GPU memory free;
inspect **host RAM as well** before starting it. Never restart an existing
server or stop another job. The stopped server PIDs and its logs are retained.
`generate.py` sends bounded requests and stops after three request errors.
`clean.py` checks 4–12 turns, finishes, Taiwan vocabulary and length, then uses
exact/template deduplication, 64-permutation MinHash + exact 5-gram Jaccard >=.8,
and v3's first-six/last-four family cap 100. Rejected turns break adjacency.
MinHash retrieval is probabilistic. These filters cannot certify semantics or
zero typos: **both actual pilots failed human sample review**. Nonempty generated
training text additionally needs `generation-review.json` with a reviewed
`approved_sha256`; no such approval was granted in this experiment.

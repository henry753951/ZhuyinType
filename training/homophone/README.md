# Homophone continuation

Dev-selected candidate: unchanged v2b + `data/build/v3_homophone`, v3 weight 1,
rerank count 16. `data/build/homophone/choice.json` is immutable. See REPORT.md for
measurements, rejected attempts and limitations. Do not repeat sealed test runs.
PyTorch/ONNX dev gates pass. Compiled Core ML passes Swift interface vectors but
fails the inherited fp32 numerical tolerance on one synthetic case; do not promote
the Core ML artifacts as fully validated. See `coreml-pytorch-audit.json`.
Final smoke is unchanged at 17/19 primary positions, with a new aspect/repetition
regression; this experiment is not a completed production fix.

All training, model-quality scoring and end-to-end evaluation run on Linux under
`~/zhuyintype`, accessed with
`ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`.
Reuse `.venv-gen`; portable workspaces reuse the validated ONNX environment.
No dependency installs. Prefix remote Python commands with
`PYTHONDONTWRITEBYTECODE=1`; long jobs use nohup and `logs/homophone-*.log`.
Inspect failures rather than automatically retrying; stop only verified owned PIDs.
The Mac only exports/compiles the final chosen Core ML model and runs the single
requested frozen-CLI `--check-v3` (already passed).

## Data and training

1. `mine.py --reuse-inventory`: preserve completed full-corpus counts, sample every
   twentieth Wikipedia line in document order and all eligible Taiwan Tongues
   zh-TW train.tsv rows. Keep complete 4–31-Han clauses; reject alphanumeric-adjacent
   fragments. Reservoir per character/category/source: 800 for 在再, 160 otherwise.
   Inventory counts include the deduplicated colloquial corpus; its encoded
   training examples also supply normal and focused continuation batches.
2. `judge.py --previous PATH`: independently mask/judge every 在/再 with Qwen3-8B,
   reject disagreement/uncertainty and inconsistent strong collocations. Cache only
   identical text/context judgments. `judge_job.sh VERIFIED_PID ...` stops that
   verified model/port process immediately after judging. Never read TT test.tsv.
3. `split.py`: alternate dev/test allocation, normalize empty contexts, exclude
   dangling final 在/再 and untypable clauses. Require 1,000 primary clauses per
   split and >=400 positions per character; target 100 per secondary group.
   Inspect `review100.jsonl`; record findings in `manual-review.json` before training.
4. `filter.py`: exclude whole encoded documents on heldout 8-grams or short actual
   sentences/smoke substrings. Include previous holdouts, new dev/test, context and
   context-target joins. Check original and UNK-normalized text. Short context alone
   contributes only 8-grams. Store indexes into immutable mmap shards and unique,
   confident TT training clauses. Rejected earlier splits/indexes stay in audit trees.
5. `train_v3.py`: continue original v3 at 2e-5, batch 64, 30% focused batches,
   3x confusion loss, unchanged augmentation/context dropout. Half of focused
   choices target balanced 在/再; eligible TT supplies 25% of that stream. Check
   dev every 500 steps, stop after four non-improving checks. Save improved primary
   dev NLL only when all eight original general-dev conditions lose <=0.2 points.

## Portable evaluation

Use **JOBS=60**, dedicated remote directories, and the frozen engine/evaluator
snapshot. Concurrent work moved the live engine paths; `remote.py` uses the known
good `data/build/homophone/runner-pinned.sh`, syntax-checks it and executes its
complete source in memory. The live runner remains available to other experiments.

Example dev invocation (existing rows require explicitly audited `--resume`):

```sh
ENGINE_SNAPSHOT=data/build/homophone/portable \
REMOTE_DIR=zhuyintype/work/onnx-eval-homophone-new \
EXTRA_SYNC='training/homophone training/v3/common.py training/v3/readings.py training/v3/reference.py data/build/homophone/inventory.json data/build/homophone/dev_zai.txt data/build/homophone/dev_zai.jsonl data/build/homophone/dev_pairs.txt data/build/homophone/dev_pairs.jsonl' \
V3_MODEL=v3_homophone BACKEND=cuda JOBS=60 \
data/build/v3/.venv/bin/python training/homophone/remote.py homophone new --min-free-gb 64
```

The runner's opt-in `EXTRA_SYNC`, `ENGINE_SNAPSHOT`, `V3_MODEL` and
`RERANKER_MODEL` preserve its default behavior. Named v3 models export directly from
`context.pt`, `syllable.pt`, `chars.json`, `syllables.json`, `config.json`.
CUDA serves one checkpoint pair for the entire invocation: match V3_MODEL to the
matrix. CPU supports mixed versions. Never overlap invocations in one remote tree.
Retrieve homophone results explicitly from that tree's
`data/build/homophone/results/`; the runner collects its normal logs separately.

`experiment.py baseline` runs the three requested stacks on both dev sets, both
context conditions, plus original v3 alone on primary dev for diagnosis (14 rows).
`new` runs continued v3 alone and with v2b (eight rows). `diagnose.py --new` tests
whether most residual primary errors were introduced by reranking. Only then would
v2b continuation be required. `choose.py` seals the winner after all dev gates.
No chat-prior/end-share tuning is performed.

`final --final-role baseline` and `final --final-role chosen` each require the same
choice file and matching V3_MODEL. Each role evaluates both test sets with/without
context and smoke once without context. Started/completed seals prevent replay;
full-final mode also rejects replay after either role starts. No final-phase resume.

GPU evaluation splits 60 workers between the runner's scorer and one identical
extra scorer, fitting each server's 32-context cache. Owned extra scorers are
terminated/waited by PID in finally. `--min-free-gb 64` waits for host RAM before
workers start. On a busy host, serialize jobs: admission control cannot prevent
unrelated later OOM.
Nine regression checks cover readings/exclusions, splitting, alignment/metrics,
dispatch/cleanup, RAM waiting and final replay guards.

`eval.py --model-only --backend torch --v3 DIR ...` imports the unchanged reference
runner remotely. Top-64 candidate pools inject gold: this measures conditional
selection, not decoder recall. CLI confusion metrics use edit alignment (deletion
is wrong); whole-sentence accuracy requires exact equality. Category priority is
fixed lexicon word, recognized subject, initial, before verb, before place/time,
other. Categories are heuristic; overlapping flags/manual findings are retained.

## Final artifacts

After dev selection, `export_v3.py` reuses the upstream exporter on Mac, compiles
with Xcode and emits golden outputs for the original 20 synthetic interface cases.
The frozen CLI parity check is already recorded in `homophone/check-v3.log`.
`check_onnx.py` independently compares ONNX with the selected PyTorch checkpoint
remotely; `--coreml-vectors PATH` instead compares saved Mac outputs with PyTorch,
records all 20 cases and exits nonzero on a reference-tolerance failure.
`latency.py` times remote one-thread ONNX calls on identical synthetic
shapes, 10 warmups and 100 calls; shared-host load limits interpretation.
Keep model notices, licenses and `articles.tsv.gz` attribution with artifacts.
The original pretraining may already contain newly mined holdouts; exclusion is
guaranteed for continuation only. No app deployment or UI validation is included.

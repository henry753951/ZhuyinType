# Homophone continuation

## Compatible rerun (v3_homophone2)

The delivered `v3_homophone2` is the original-recipe 500-step compatible candidate.
**Do not adopt it:** the current app stack still regresses 我在吃一碗麵 and does not
fix 再試試看. It starts from original `data/build/v3`, never the TRAIL-trained
checkpoint. Alternative targeted and midpoint attempts are rejected on dev.
Results and the shipping recommendation are in REPORT.md. Historical
instructions below describe experiment 1 only; do not reuse its TT sampler or
60-worker setting for the compatible rerun.

On the GPU host, using the existing environments (no dependency installation):

```sh
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python training/homophone/compatible.py test
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python training/homophone/compatible.py prepare --workers 4
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python training/homophone/test_compatible.py
nohup env PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
  .venv-gen/bin/python -u training/homophone/train_v3.py --compatible \
  --out data/build/homophone2 --model data/build/v3_homophone2 --steps 10000 \
  > logs/homophone2-train.log 2>&1 < /dev/null &
```

`compatible.py prepare` refuses an existing output directory. It filters immutable
Wikipedia/colloquial shards, reserves chat2 documents, labels complete real-source
clauses, and independently rescans retained documents and context-target joins.
It records input and holdout hashes. Evaluation-only TT and all imported shanjie
texts block training. Short context alone contributes eight-grams, not substrings.
Normal batches also draw 25% real-source examples. Half the primary-focused batches
target sentence-initial characters, balanced between 在 and 再; the other sampling,
loss, augmentation, frozen context and dev-plateau rules remain unchanged.

On the Mac, `run_compatible.py baseline dev` then `chosen dev` invoke a temporary
copy of the current shared runner with `REMOTE_DIR=zhuyintype/work/onnx-eval-homophone3`,
`V3_MODEL=v3|v3_homophone2`, ONNX CPU, JOBS=24 by default (environment override up to 40), and the frozen current engine snapshot
in `data/build/homophone2_snapshot`. The isolated environment reuses the existing
portable and Torch site-packages read-only. Each evaluation phase is single-use;
inspect any partial result before an explicitly audited recovery. Never overlap
runner invocations in this workspace. The compatible dispatcher uses nohup in a
new session with logs under ~/zhuyintype/logs and a PID receipt under
remote_eval/compatible-evaluation.pid. A dropped SSH client does not authorize
replaying sealed rows: inspect completion seals and the owned process first.
After certifying that no evaluator remains, `RECOVER_TEST=1` permits only exact
manifest/cache reuse and rows with no start log; a started row lacking complete
raw predictions is never replayed. Derived sidecars can be rebuilt without decoding. Match the current app: reranker count 16 and weight 0.5,
v3 weight 0.5, chat prior 0.3, default reranker gate 8. Each evaluation row waits
for at least 48 GiB available host RAM. Baseline and chosen ordered outputs feed
`training/eval/stats.py` (paired bootstrap and exact McNemar; source-line clusters
for homophone intervals). `choice2.json` locks the checkpoint; both dev completion
seals and `dev-decision.json` are required before
`baseline test` and `chosen test`, once per role/context. The old test sets remain
byte-identical; they never drive checkpoint selection.

After the dev choice, `export_v3.py --compatible` uses the unchanged upstream fp16
exporter and Xcode compiler, preserves attribution/component licenses, and emits
the original 20 interface cases. `check_onnx.py --coreml-vectors ... --diagnostic`
records the original and new models' saved Core ML versus PyTorch differences
without applying the historical fixed 0.1 bound. This diagnostic does not assert
numerical equivalence: compare its magnitudes and argmax flips against original
v3, then check the same saved 400 real dev inputs on the Mac CLI as the authorized
one-off parity check. Keep `--check-v3` and independent numerical/real-output
checks distinct. This task does not modify release assets or install the app.

## Controlled follow-ups

The first compatible checkpoint improves primary dev but repeats the known
aspect/repetition regression. `compatible.py target` creates `homophone2b` by
reusing the same allowed indexes/real clauses and adding focused subsets:
sentence-initial 再 before verbs, and subject + 在 + verb anywhere in a clause.
No new training text is generated. The midpoint also retains the same
我在吃一碗麵 regression at reranker weight 0.5 and is rejected. The initial prefix-only subset was archived
before training because it omitted clauses with preceding conjunctions.

```sh
# GPU host, after the compatible preparation above:
.venv-gen/bin/python training/homophone/compatible.py target
nohup env PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
  .venv-gen/bin/python -u training/homophone/train_v3.py --compatible --targeted \
  --out data/build/homophone2b --model data/build/v3_homophone2b --steps 10000 \
  > logs/homophone2b-train.log 2>&1 < /dev/null &
.venv-gen/bin/python training/homophone/stabilize.py
```

The targeted retry additionally gates selection on model-only primary decisions
for the three known requested examples: 再試試看, 我再吃一碗, 我在吃一碗麵. These are
explicit development guards, excluded from gradient training; they are not unseen
confirmation evidence. Model-only success does not establish app success: the
retry regresses 我再吃一碗 at the current app weight 0.5 and is rejected. Early
fast probes accidentally used conversion_eval's reranker default 1; they are
archived separately and excluded from the app-configuration comparison. All
formal matrices used the CLI default 0.5; the adapter now pins it explicitly.

`stabilize.py` checks exactly one parameter midpoint between original v3 and the
first licensed continuation. It asserts identical vocabularies, tensor shapes and
frozen context, finite output weights, parent-selection identity, then measures
all eight original general-dev conditions on the GPU host. It changes no app
scoring flag. `stabilization.json` identifies both parents and the 0.5 coefficient.
The app still uses v3 weight 0.5. This is an additional stabilization step, not the
unchanged historical recipe or a coefficient sweep.

For `homophone2_stable`, reuse certified baseline results only after matching the
CLI, model, lexicon/LM and evaluation hashes; keep them in that run's `results/`
with `baseline-dev-complete.json`. From the Mac:

```sh
JOBS=40 data/build/v3/.venv/bin/python training/homophone/run_compatible.py chosen dev homophone2_stable
# The midpoint is rejected on dev; neither alternative opens test.
# Freeze the default homophone2 dev decision, then score each role once:
JOBS=40 data/build/v3/.venv/bin/python training/homophone/run_compatible.py baseline test
JOBS=40 data/build/v3/.venv/bin/python training/homophone/run_compatible.py chosen test
# Mac export/check and single-use parity for the delivered default candidate:
data/build/v3/.venv/bin/python training/homophone/export_v3.py --compatible
.build-codex5/release/zhuyin-cli --check-v3 data/build/v3_homophone2
data/build/v3/.venv/bin/python training/homophone/mac_parity.py chosen
```

The parity command is single-use, runs 400 saved Linux inputs: the first 397 frozen Wikipedia runs
with line context plus the three known critical smoke inputs, and asserts that the delivered checkpoint matches the
selected run. Its temporary lexicon/LM binaries are rebuilt under `homophone2/`
and hash-match the Linux binaries. The original-v3 control retains all 400 Wiki inputs (397 shared with the final check).
Never run Mac corpus evaluation for checkpoint selection. 再試試看 remains a known
failure in the midpoint probe; see REPORT.md for the final measured recommendation.

## Historical experiment 1

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

# v2b prefix-tree scoring — 2026-10-27

The tree model and shared Swift path are implemented, but **acceptance is partial**.
On the Apple M5, the corrected model improves the 25-syllable composition from
**53.4 to 39.9 ms p95** and **162.46 to 102.74 mJ/key**. The requested **p95 < 30 ms
target is not met**. Remote fp32 ONNX parity passes: **2,000/2,000 identical lines**
and **5,209/5,209 identical argmax / exactMargin decisions**, without fallback.

Core ML fp16 spot checks still exceed the requested typical <0.01-nat score
tolerance (short-sentence median 0.026 nats, maximum 0.106). The sampled choices
agree after fixing an integer-index conversion bug, but this does not establish
99.9% decision parity for Core ML. Do not treat this as a fully accepted app change.
Per the stop-on-repeated-failure instruction, precision/performance tuning stopped
after the normalization experiment failed to resolve this limit. CPU remains the
default; no accelerator or context-cache change was adopted.

## Implementation

- `export_tree.py` imports `CharLM`, `Config`, and the original blocks from
  `train.py`. Training behavior and all checkpoint weights/token IDs are unchanged.
- A trie node is identified by its parent and encoded token. Equal characters
  after different prefixes are different nodes. OOV characters can share `<unk>`
  states, retaining the existing per-character unknown penalty.
- RoPE indexes the original cosine/sine tables with explicit **depth**, not the
  node's packed offset. Every attention row permits only itself and its ancestors.
  The mask uses 0 / -10000, whose blocked probabilities underflow at these precisions.
- Terminal leaves are targets only: their probabilities come from their parent's
  logits, so their hidden states need not be evaluated. Context nodes are evaluated
  once. Output scores are summed along each candidate's path; context scores are
  excluded. Empty candidates, duplicates, unequal lengths, context truncation and
  the 5,465-way unknown penalty retain their old behavior.
- Inputs: `ids`, `positions` `[1,N]`; `ancestors` `[N,N]` with integer 1 = allowed;
  `parents`, `targets` `[E]`. Output `logprob` `[E]` scores each target at its parent.
  Swift constructs the same arrays for Core ML and ONNX. `scores`,
  `candidateScores`, and `choose` retain their public signatures and mixing /
  exactMargin rules.
- Core ML uses fp16 and flexible bounds 1…2017, covering 32 candidate-list entries
  of 63 characters plus BOS (and therefore 16 sentence candidates). Depth remains
  bounded by the existing 64-token model context. ONNX uses fp32 with dynamic N/E.
- The Core ML export removes `common::add_int16_cast`: its symbolic-shape
  analysis incorrectly narrowed flattened parent × vocabulary indices to uint16,
  overflowing beyond 65,535. Transformer arithmetic stays fp16; gather indices
  remain int32. An export-time MIL assertion prevents this regression.
- `RerankerTree.mlmodelc` / `RerankerTree.onnx` are detected automatically.
  Missing, unloadable, throwing, malformed or non-finite tree results fall back
  to the existing batch model. More than 32 candidates retain the batch path;
  candidate-list scoring keeps its existing 32-entry chunks.
- `TREE_RERANKER=1` opts the remote runner into exporting the additional ONNX
  model. Default export behavior for other jobs remains unchanged. The optional
  CUDA socket backend still uses batched scoring.

No dependencies, package managers, global defaults, ONNX C bridge, training
behavior, engine, app, or scripts were changed by this task. No app installation,
git commit, stash, reset, or checkout was performed.

The current `scripts/build-app.sh` copies only the original compiled reranker.
That script belongs to another concurrent task and was left untouched: packaging
must also copy `data/build/reranker/RerankerTree.mlmodelc` before a newly bundled
app can use the tree. Runtime model-directory overrides can already load it.

## Artifacts and reproducibility

Models are next to the current reranker in `data/build/reranker/`:

- `charlm.pt`: retrieved from `~/zhuyintype/reranker_v2b/charlm.pt` for an exact
  same-weight export; current model files were preserved.
- `RerankerTree.mlpackage`, compiled `RerankerTree.mlmodelc`, `RerankerTree.onnx`.
- `tree-validation/`: frozen sources, binaries, raw timing logs, experiment
  artifacts, input keys, manifests and summary JSON.

Existing local tools: Python 3.12.14 in `data/build/v3/.venv`, PyTorch 2.7.0,
coremltools 9.0; Xcode Swift 6.4; Apple M5, 32 GiB RAM. No installation was needed.
The remote uses the pre-existing isolated ONNX environment and Swift 6.4.1.

```sh
PYTHONDONTWRITEBYTECODE=1 data/build/v3/.venv/bin/python \
  nlp/training/reranker/export_tree.py --data data/build/reranker --format coreml
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcrun coremlcompiler compile data/build/reranker/RerankerTree.mlpackage data/build/reranker
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  swift build -c release --scratch-path .build-codex4

# Linux, inside the isolated remote evaluation directory:
.venv/bin/python nlp/training/reranker/export_tree.py --data data/build/reranker --format onnx
# Or the existing combined exporter with the optional --tree flag.
```

`tree-validation/frozen` fixes the engine, CLI, package and evaluator inputs for
comparison. Only ZhuyinML was replaced between baseline and tree builds. The one
exception is the other engineer's existing Linux-only `getrusage` type fix, copied
into the CLI snapshot after its first Linux build exposed the incompatibility;
it does not change conversion or the Mac branch. The final timing control uses
**one identical final CLI binary** with a baseline-only model directory versus
the tree-enabled directory. `manifest-mac.json` records the frozen source hashes.

## Mac timing

All Mac model measurements use **only `zhuyin-cli --timing`**, with:

```text
--reranker DIR --rerank-count 16 --rerank-weight 1
--v3 data/build/v3 --v3-weight 0.5
--chat-lm data/build/lm_chat --chat-prior 0.5
```

No accuracy evaluator ran on the Mac. Each primary case starts a fresh CLI three
times, with no preceding text, no Enter, and an optional final `<down>`. The table
uses medians of the three reported p50/p95, CPU, energy and memory values; **max
is the worst key across all three runs**, including warm-up/scheduling outliers.
Energy is the CLI's per-process macOS estimate, not whole-machine energy. Shared
system work continued; timings are not isolated hardware measurements.

| Input / path | Model | Keys | p50 ms | p95 ms | Max ms | CPU ms/key | Energy mJ/key | Memory now / peak MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Short sentence, typing | Current | 33 | 16.2 | 31.8 | 35.3 | 16.7 | 81.84 | 328 / 332 |
| | Tree | 33 | 10.6 | 21.6 | 56.2 | 10.7 | 52.38 | 328 / 331 |
| Short sentence + down | Current | 34 | 15.1 | 29.6 | 34.9 | 16.3 | 81.04 | 328 / 332 |
| | Tree | 34 | 10.3 | 22.1 | 23.9 | 10.6 | 51.99 | 328 / 331 |
| Long sentence, typing | Current | 76 | 29.6 | 53.4 | 91.4 | 31.3 | 162.46 | 333 / 343 |
| | Tree | 76 | 19.0 | 39.9 | 44.4 | 20.1 | 102.74 | 335 / 342 |
| Long sentence + down | Current | 77 | 30.6 | 52.4 | 60.7 | 31.3 | 164.06 | 333 / 348 |
| | Tree | 77 | 19.2 | 37.4 | 46.0 | 19.5 | 102.66 | 335 / 342 |

The candidate-list model calls, summed from the CLI's `[candidates … ms]` lines
after typing, improve **14.9 → 2.8 ms** for the short sentence and **46.4 → 3.8 ms**
for the long sentence. These sums are model-call time, not a separately instrumented
whole-down-key latency. The aggregate timing rows include the down key.

The requested short sentence, 「今天晚上要不要一起吃飯」, actually has **11**
syllables. 「我們明天下午三點在公司門口集合然後一起去吃晚餐好嗎」 has 25.
Readings and keys come unchanged from `conversion_eval.py`'s `load_lexicon`,
`readings_for`, `keys_for`; no hand-chosen sandhi or alternate readings:

```text
11: rup<space>wu0<space>j03g;7ul41j4ul4u<space>fu3t<space>z04
25: ji3ap7au/6wu0<space>vu84j3n0<space>2u03y94ej/<space>n<space>ap6d.3ru6ck6b06c.4u<space>fu3fm4t<space>j03h0<space>cl3a87
```

Raw primary results: `tree-validation/{before-control,after-int32}/*.log`,
each with `results.json`; combined medians: `summary-timing.json`.
Initial trials remain under `before`, `tree-cpu`, `tree-pruned`, and `after-final`.
**All tree timing trials before `after-int32` used the overflowing uint16 model
and are invalid for acceptance**, including the earlier reported 38.6 ms figure.
They are retained as failure evidence, not substituted into the final comparison.

```sh
data/build/v3/.venv/bin/python nlp/training/reranker/time_tree.py \
  --cli data/build/reranker/tree-validation/zhuyin-cli-final \
  --label before-control --reranker data/build/reranker/tree-validation/current
data/build/v3/.venv/bin/python nlp/training/reranker/time_tree.py \
  --cli data/build/reranker/tree-validation/zhuyin-cli-final --label after-int32
```

The baseline directory contains only links to `../../Reranker.mlmodelc`,
`../../vocab.json`, `../../config.json`, without a tree model.

## Compute units and caching

- Corrected flexible tree, CPU+ANE one-repeat check: 25-syllable p95 **36.2 ms**,
  CPU **19.3 ms/key**, energy **101.55 mJ/key**, memory **338 / 346 MB**, versus
  the CPU three-run range **37.1–40.1 ms** (median 39.9). This single trial is
  faster, but does not establish a repeatable placement benefit under varying
  shared load. It still misses 30 ms and does not resolve fp16 parity, so no
  compute-unit default was changed. Raw logs: `after-int32-ane/`.
  Compute-unit permission is not proof of ANE execution.
- Earlier CPU+ANE and fixed-128 trials used the bad uint16 export. Their numbers
  are not acceptance evidence. The fixed trial also permitted fallback for
  larger tries, without instrumentation of placement/fallback counts. Its
  padding/shape-dispatch code was removed; experimental sources, binary and model
  remain under `tree-validation/` for inspection. No bucket variant was adopted.
- Context KV caching was not implemented. This exported graph has no state/KV
  interface, and the app supports macOS 14; clean persistent MLState would require
  a newer deployment/API path or explicit per-layer KV inputs/outputs. It also
  would not save the long benchmark's candidate work because that benchmark has
  empty preceding context. The existing v3 context cache remains unchanged.
- Core ML bounded flexible-shape behavior follows Apple's
  [flexible input guide](https://apple.github.io/coremltools/docs-guides/source/flexible-inputs.html);
  stateful model API is documented under
  [MLState](https://developer.apple.com/documentation/coreml/mlstate).

## Remote fp32 acceptance — passed

The runner is isolated at `$ZHUYIN_REMOTE_HOST:~/zhuyintype/work/onnx-eval-tree`.
Its `.venv` links the existing evaluation environment read-only; data/model inputs
were seeded by copies from the existing evaluation directory and then checksum-
synced by `run.sh`. No GPU training workspace or other evaluation was stopped.

```sh
TREE_RERANKER=1 REMOTE_DIR=zhuyintype/work/onnx-eval-tree \
ENGINE_SNAPSHOT=data/build/reranker/tree-validation/frozen JOBS=60 \
EXTRA_SYNC='nlp/training/reranker/check_tree.swift nlp/training/reranker/validate_tree.py' \
nlp/remote/run.sh sweep \
  --matrix nlp/remote/tree_validation.json -n 400 \
  --output data/build/remote_eval/tree

# After all ten matrix rows have finished, on the remote runner:
PATH="$HOME/opt/swift/usr/bin:$PATH" PYTHONDONTWRITEBYTECODE=1 \
OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
.venv/bin/python nlp/training/reranker/validate_tree.py --jobs 60
```

The baseline ONNX directory `data/build/reranker/tree-validation/current` contains
links to the current `Reranker.onnx`, vocabulary and config only. The five sets
are dev_colloquial, dev_wikipedia, dev_chat_tt, Wikipedia `--line-context`, and
colloquial `--split continue`, all 400 lines with the app flags above.
Disabling tree export does not disable a model already on disk; a baseline-only
directory is required for a controlled comparison.

`validate_tree.py` checks exact ordered input alignment and ≤0.5% differing lines
per row, listing every line difference. It replays 40 lines from each set through
the real Composer, scoring every N-best call and opening candidate lists before
commits. It compares sentence scores, neural argmax, combined-score argmax and
exactMargin decisions, requiring ≥99.9% identical decisions. A wrapped tree
predictor asserts successful execution, correct output shape and finite values;
fallback cannot silently pass this study.

Structural and real-model edge checks already passed: sibling isolation, depths,
shared prefixes, prefix candidates, duplicates, empty candidates, OOV, long
context, 63/64-character limits, the 32 × 63 worst-case tree, malformed-output
fallback and throwing-model fallback. Maximum edge-case sentence score error is
**0.0000815392 nats**, including the 1,985 hidden-node / 2,017-target worst case.

| Set | Current accuracy | Tree accuracy | Different lines / 400 | Current seconds | Tree seconds |
|---|---:|---:|---:|---:|---:|
| dev_colloquial | 87.0% | 87.0% | 0 / 400 | 175.41 | 97.66 |
| dev_wikipedia | 80.5% | 80.5% | 0 / 400 | 197.68 | 87.29 |
| dev_chat_tt | 70.5% | 70.5% | 0 / 400 | 94.66 | 77.39 |
| Wikipedia line-context | 85.8% | 85.8% | 0 / 400 | 277.91 | 67.60 |
| Colloquial split-continue | 81.2% | 81.2% | 0 / 400 | 91.04 | 44.89 |

**Every output difference: none.** All ordered CLI inputs match exactly.
Times include startup/preparation under concurrent lab work, not isolated CPU
benchmarks. Both configurations ran with JOBS=60 and -n 400.

The score replay covers **200 lines, 5,209 actual model calls, 82,831 candidate
sentence scores**, including candidate-list calls. Score absolute error:
median **0**, p95 **0.00000429153**, maximum **0.0000203997 nats**. Every score is
below 0.01 nats. Neural argmax, combined-score argmax, and exactMargin choice are
each **5,209 / 5,209 identical (100%)**. Decision differences: **none**. Wrapped
predictions confirm tree execution rather than silent batch fallback. Real-call
tries have median 16 and maximum 218 hidden nodes.

Raw retrieved evidence is `tree-validation/remote/matrix/*.json` and
`tree-validation/remote/parity/{summary.json,*-calls.json,edges.json}`. The replay
and edge checks were on Linux only. These results **do not establish full Core ML
fp16 parity**.

The existing Linux `BackendTests` also passed **2/2**, covering injected batch
encoding, padding, OOV penalties, candidate chunking, exactMargin boundaries and
v3 input/output mapping. Final root and frozen Mac release builds passed. Local
and remote checkpoint, vocabulary and tree ONNX SHA256 values agree:

```text
charlm.pt         e7f5ebcff4ae2b61d78647aa73ee7a8efe9abce86aa86626394325ae34081a53
RerankerTree.onnx dafc21e08790c087b6098dcd4b1258fb41bfb3aad7d257c15659884da0cae863
vocab.json        59fc563c0989ec4643652194e0695655d02ebff831b5fb635c7cc2618644c0f3
```

Final portable manifests agree on **all 51 consumed source/data/model files**, with
zero missing or differing files. Evidence:
`tree-validation/manifest-portable-mac.json` and
`tree-validation/remote/manifest-comparison.json`. Final Core ML artifact hashes
are recorded separately in `manifest-coreml-final.json`.

## Core ML fp16 spot checks — score tolerance not met

Only the two requested benchmark inputs were used, via `zhuyin-cli --timing
--nbest 16` with the same app flags. This prints real N-best lists and neural
scores to three decimal places; these extra runs are excluded from the latency
and energy table. Commands are preserved in
`tree-validation/check-coreml-nbest.py`; raw logs and JSON are under
`coreml-nbest-int32.*` and `coreml-corrected*`.

The int32 index fix removes the 85–286-nat errors of the original bad export.
Remaining sampled fp16 differences are small but not consistently below 0.01:
11-syllable median **0.026**, maximum **0.106 nats**; 25-syllable median **0**,
maximum **0.105 nats**, using the first corrected spot check. The sampled neural,
combined-score and exactMargin argmax decisions agree. A two-list spot check is
not enough to certify a 99.9% Mac decision rate.

Retaining fp32 output normalization was also tested: short/long median differences
were **0.0245 / 0.013 nats**, maxima **0.096 / 0.098**. Choices agreed, but the
requested tolerance still failed, so that variant was rejected. The final export
keeps the original fp16 arithmetic with the int32-index fix. Further precision
changes or broad Mac accuracy sweeps were not attempted.

The exporter regression checks the final gather index type directly. The cause
and exclusion are consistent with Apple's
[integer cast pass](https://apple.github.io/coremltools/_modules/coremltools/converters/mil/mil/passes/defs/quantization.html),
whose dynamic-shape check only guards known large dimensions. The compiled bad
program's `var_811_to_uint16` cast is retained in the rejected model artifact;
the corrected exporter asserts int32 after conversion.

## Failures and limits

- The first runner invocation had a wrongly nested evaluator snapshot; corrected
  the snapshot layout. A task-owned rsync was then stopped by exact PID 7291 while
  moving to the final model format. Other processes were not killed.
- Leaf pruning initially captured Swift `self` before initializing `ancestors`;
  initialized that field before closure use. Both final Mac builds succeeded.
- The first Linux CLI snapshot exposed the existing `getrusage` enum mismatch;
  used the other engineer's already-present Linux-only type fix in the snapshot.
- A score-validation invocation was started before matrix captures existed.
  Structural/model edge checks passed, then the script correctly refused an
  incomplete five-set comparison. Its log is preserved; acceptance must use the
  final successful run after all captures existed.
- Core ML uint16 index narrowing caused large score errors despite Linux parity.
  Corrected the export, added the MIL assertion, and discarded all earlier tree
  timing results. Remaining fp16 score-tolerance failures are documented above.
- After all ten complete matrix rows printed, the live-edited runner logged
  `syntax error near unexpected token elif`. The current file passes `bash -n`;
  it was edited concurrently. The failure is consistent with the long-lived
  shell resuming at a changed file offset (inference, not a reproduced shell bug).
  All complete captures were independently validated. A subsequent integration
  smoke uses the runner's text frozen in memory so concurrent edits cannot alter
  its remaining shell input. That smoke passed **4/4 lines, 100% accuracy**, with
  the app configuration and JOBS=60, including build/export/automatic retrieval.
  Log: `tree-validation/runner-immutable-smoke.log`.
- No corpus accuracy evaluator ran on the Mac. No claim is made that the
  25-syllable p95 target was achieved, that ANE executed the graph, or that context
  KV caching was measured. Further latency work is still required.

## Summary — partial acceptance

| Measure | Current | Tree |
|---|---:|---:|
| 11 syllables: p50 / p95 | 16.2 / 31.8 ms | 10.6 / 21.6 ms |
| 11 syllables: energy | 81.84 mJ/key | 52.38 mJ/key |
| 25 syllables: p50 / p95 | 29.6 / 53.4 ms | 19.0 / 39.9 ms |
| 25 syllables: energy | 162.46 mJ/key | 102.74 mJ/key |
| Long candidate-list model calls | 46.4 ms | 3.8 ms |
| Five-set fp32 output parity | Baseline | 2,000/2,000 identical |
| Real-call fp32 decision parity | Baseline | 5,209/5,209 identical |
| Typical fp32 score error < 0.01 nats | Reference | Pass (maximum 0.0000204) |
| Typical Core ML fp16 score error < 0.01 nats | Reference | **Not met in spot checks** |
| 25-syllable p95 < 30 ms | No | **No** |

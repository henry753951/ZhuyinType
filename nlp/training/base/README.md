# Statistical base experiment (A1)

Code is AGPL-3.0-only. Run every training, model check and evaluation on the Linux GPU host.
The developer Mac only edits/transfers files and retrieves artifacts. No engine,
app, shared evaluator, asset manifest or documentation integration is included.
The dev-selected bundle has TT/smoke test regressions; A1 acceptance remains
unmet. Read REPORT.md before integrating these research assets.

`data/build/base/` holds the frozen engine/evaluator snapshot, incumbent typing
lexicon, source hashes, additive McBopomofo lexicon, prepared corpora, candidate
models, ordered predictions and paired statistics. Final deliverables are
`lexicon.tsv`, `lm/`, `lm_chat/` and their notices. Unselected candidates remain
under `models/`. The incumbent assets are never overwritten.

## Method

The additive lexicon preserves every incumbent `(reading, word, score)` exactly.
Only Han entries with one valid Zhuyin syllable per character are imported.
McBopomofo's log10 scores become natural logs. New words use the median score
offset among shared words of the same length (lengths >=5 pooled); new readings
of existing words cannot exceed their current best reading minus 1 nat. Existing
Taiwan readings and polyphone odds therefore remain intact. The final merge retains source-supplied 一/不 variants; it does not infer new
ones for ordinals or numeral names. Extension-plane Han characters are accepted. Source
membership, rejected classes, calibration offsets and SHA256s are recorded.

Segmentation uses the incumbent final-E unigram distribution (wiki or chat by
domain), with calibrated lexicon probabilities for newly introduced words.
The tokenizer is held constant between matched hard and expected-count models.
Expected counts sum exact forward/backward edge marginals and adjacent-edge
joint posteriors over every segmentation, including BOS and EOS. There is no
per-edge posterior cutoff. Counts are summed across shards/sources, generated
text is weighted, then counts are rounded **once** to use the existing integer
MKN estimator and its pruning/backoff normalization. This isolates the final
counting change. A second, matched chat control uses `--fresh`: initialize from
normalized lexicon priors, perform three E steps with two intervening empirical
unigram M steps (0.1 pseudocount), and compare hard versus posterior counts.
The generated weight also applies during those M steps. This separates corpus
adaptation from the incumbent tokenizer's generated-text history.

Wikipedia reuses the incumbent cleaned, globally deduplicated training shards.
Real chat text reuses chat2's document-reserved no-generated shards (Legislative
Yuan, Wikipedia talk, Common Voice) and adds only Taiwan Tongues `train.tsv`.
Generated text reuses the existing v3 Qwen/Common Voice pool, subtracting exact
real-source duplicates. It is tested at weight 0.1, not generated afresh.
The candidate models retain Taiwan Tongues' TRAIL terms and are research assets
pending integration/license review; CC BY-SA alone does not describe them.

Before fitting, every source is rescanned against all current dev/test files,
both chat2 dialogue columns, homophone files, smoke and imported external row
sets. Exact matches and any contiguous eight-character match are excluded,
including 台/臺-normalized versions. Heldout conversion supplies only exclusion
forms, never training examples. Test predictions never select a model.

All configurations use the same 400 runs/set, seed 7, incumbent typing lexicon
and frozen keys. Dev sets: colloquial, Wikipedia without/with line context, and
Taiwan Tongues (report exact and variant-equal). Engine mode has the same chat
prior 0.5 but no neural model. App mode adds unchanged v2b top-16, weight 1, and
v3 weight 0.5. CLI batches save diagnostics and stop on any nonzero return or
wrong output count. Results include model/CLI SHA256s. `stats.py` supplies paired
McNemar, bootstrap intervals and break-rate bounds. Lexicon audits distinguish
per-reading rows, the preferred reading per word, and words that lose under every reading: homophones cannot
all be top-1. Habits reuse `error_analysis.py` rules on the frozen colloquial dev
runs and report shown accuracy and engine oracle@16.

Selection uses dev only: primary score is the equal-weight mean of colloquial
exact, Wikipedia with-context exact and Taiwan Tongues variant-equal. Prefer
engine gains while retaining app primary score; Wikipedia without context is
an additional guard. All stepwise numbers and any tradeoffs go in REPORT.md.
Freeze the final configuration before evaluating test sets once.

The selected bundle is the corrected McBopomofo lexicon, `wiki_expected_wide`
and `chat_fresh_expected`, without generated counts. Use the three together;
the new LMs perform poorly with the old decoding lexicon. Regenerate compiled
lexicon/LM assets together during integration: these experiments use TSVs with
no binary sidecar overriding them. Mac mmap memory and latency remain unmeasured.

Keep all incumbent TSVs in `inputs/baseline/` and all seven evaluation texts in
`inputs/evaluation/`. `frozen-inputs.json` pins those files, the typing lexicon,
CLI and neural assets. The evaluator refuses changed inputs and stale cached
results. A clean habit run must reproduce its main dev capture exactly. Shared
`data/build/lm_chat/` is not an immutable baseline: another job can replace it.

## Reproduce

Use the existing remote Python 3.12 environments; no packages are added. The
isolated `.venv` uses `.pth` references to the existing parser/NumPy/Torch/ONNX
environments without modifying them. The initial runner is copied to
`data/build/base/frozen-runner.sh`; its only local dispatch addition invokes
`nlp/training/base/evaluate.py` with `nohup` and logs under `~/zhuyintype/logs/`.
Invoke the copy with the original `$0` so the root lookup remains valid:

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-base JOBS=8 BACKEND=cpu \
  ENGINE_SNAPSHOT=data/build/base/snapshot LOCAL_RESULTS=data/build/base/runner-logs \
  EXTRA_SYNC='nlp/training/base nlp/training/lm/pipeline.py nlp/training/lm_chat/train.py data/build/base/inputs data/build/base/baseline.json nlp/eval/stats.py' \
  bash -c "$(cat data/build/base/frozen-runner.sh)" nlp/remote/run.sh \
  base --matrix data/build/base/baseline.json --phase dev -n 400
```

After transferring `nlp/training/base/` and the McBopomofo source/notice to that
workspace, execute remotely:

```sh
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
.venv/bin/python nlp/training/base/test_base.py
.venv/bin/python nlp/training/base/train.py lexicon --source-root "$HOME/zhuyintype"
nohup .venv/bin/python -u nlp/training/base/train.py prepare \
  --source-root "$HOME/zhuyintype" --workers 8 > "$HOME/zhuyintype/logs/base-prepare.log" 2>&1 < /dev/null &
echo $! > data/build/base/prepare.pid
nohup bash nlp/training/base/run_train.sh > "$HOME/zhuyintype/logs/base-train.log" 2>&1 < /dev/null &
echo $! > data/build/base/training.pid
```

Models refuse existing output directories; preserve any partial directory and
choose a new model name before retrying. Never kill another job or use broad
process-name matching. Stop and record repeated failures.

The recorded LM controls use the archived first lexicon (`lexicon_mcbopomofo.tsv`,
232,700 rows). The corrected source-only lexicon (`lexicon_source.tsv`) is a
separate dev ablation: it removes inferred sandhi rows and admits extension-plane
Han. Word-model fitting is independent of typed readings; the final combination
is rechecked on dev with the corrected lexicon. `train-v1.py` preserves the
initial merge implementation; `run_train.sh` names that frozen seed explicitly.
The current trainer defaults to the corrected lexicon for future experiments.
`train-fresh-v2.py` in the data archive matches the selected chat fit's exact
training-source fingerprint; the current trainer adds only the rounding-limit
comment after that fit.
`run_fresh.sh` runs the three corpus-adapted chat controls. `prior.py` is the
rejected context-free frequency control; `prune.py` performs the single wider
Wikipedia pruning ablation without recounting its corpus. Matrices, frozen
receipts and ordered result files accompany the local data bundle.

`fallback.py --source MODEL --output NEW_MODEL` is a data-only ablation. It
removes lexical words at the zero-continuation floor only when no stored
bigram/backoff references them. They then use the engine's existing lexicon
fallback. With removed unigram mass M and context backoff alpha, restrict stored
probabilities by Z = 1 - alpha*M, rescale unigrams by 1/(1-M), and set the new
backoff to alpha*(1-M)/Z. This preserves conditional normalization over the
retained vocabulary. Old models remain available unchanged.

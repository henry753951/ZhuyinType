# Timed typing-error evaluation

The harness evaluates committed text and pure-engine recall@16 on the development
sets. It writes no engine or model files. Run it from the repository root.

## Reproduce

Rebuild before **each full run**, because the engine is being changed concurrently:

```sh
mkdir -p .build-eval
uv run --no-project python nlp/eval/typing_errors.py --self-test
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-eval --product zhuyin-cli > .build-eval/typing-errors-build.log 2>&1 && \
env -u ZHUYIN_UNITS uv run --no-project python nlp/eval/typing_errors.py \
  -n 300 --seed 7 \
  --config current '' \
  --config current+reranker '--reranker data/build/reranker --rerank-weight 1 --rerank-count 16' \
  --examples 2 --json .build-eval/typing-errors.json \
  > .build-eval/typing-errors-table.md 2> .build-eval/typing-errors.log
```

`--test` takes corpus paths; `--self-test` runs the built-in assertions without
data or a CLI. `-n` defaults to 300 **per dataset before applicability filtering**.
Configurations accept shell-quoted CLI arguments, for example
`--config timed '--decoder timed'`. The current build advertises
`slots|boundaries|reorder|timed`; the two measured configurations use its default,
`slots`. No decoder names are hard-coded in the harness.

The full-run command above used the two default dev paths for `--test` and
redirected output to the scratch directory. Core ML used the CLI default, CPU only.
Pilot runs used `-n 3` and `-n 1`; a separate 30-run timing probe found CPU faster
than `ZHUYIN_UNITS=all` or `ane` on this Mac. These pilots are excluded below.

## Model and counting rules

- Select readable Han runs using the same `[\u4e00-\u9fff]{4,20}` matching,
  lexicon longest-match readings, and first-tone Space convention as
  `conversion_eval.py`; require one reading per reference character. Shuffle with
  seed 7. Each condition uses the same selected runs, filtering inapplicable ones.
  Readings are inferred, not manually annotated pronunciation truth.
- Use US Mac key centres, with row offsets 0, 0.25, 0.5, 1 key widths, touch-typing
  hands/fingers, and right-thumb Space. Every key gets a `<dt:MS>` prefix.
  Intervals are log-normal with log standard deviation 0.35 and base median
  120 ms (60 ms fast), multiplied by 0.75 for alternating hands or 1.4 for the
  same finger. Exact and exact-fast share random timing draws, scaled by two.
- Swaps exchange distinct adjacent keys, including tone keys and first-tone
  Spaces. `swap-in-syllable` stays inside a syllable; `swap-across` crosses its
  boundary. Rollover samples either kind and sets the exchanged pair's gap to
  uniform 0–15 ms. Ordinary swaps retain ordinary sampled timing.
- Hotspots sample a landing point from a Gaussian centred on the intended key
  (horizontal sigma 0.45, vertical sigma 0.30), snap to the nearest physical key
  centre, and reject unchanged/non-neighbour landings (distance over 1.6 widths).
  Physical punctuation keys remain possible slips. Wide Space is excluded as a
  hotspot/extra-key anchor; it can still be swapped or dropped. Extra-key inserts
  a sampled neighbour before or after its anchor, with uniform 0–20 ms pair gap.
- Conditions 3–9 inject exactly one edit per applicable run. `swap-across-2`,
  `rollover-2`, and `hotspot-2` inject two edits at disjoint positions. Missing-key
  drops one key; tone exchanges only 3↔4 or 6↔7.
- Nospace omits only canonical first-tone Spaces **before another syllable**;
  the final Space is retained. `nospace-all` removes every eligible Space;
  `nospace-30` samples each independently at 30%. Only runs with an actual omission
  count, so this row is conditioned on observing an error.
- Mixed chooses at most one slip per syllable: 3% hotspot, 2% rollover, 1% extra,
  1% missing, 1% tone, otherwise none. Inapplicable slips are skipped rather than
  replaced by another type. It independently omits 30% of eligible first-tone
  Spaces and chooses the fast profile for 30% of runs. Mixed includes unchanged
  realizations; JSON records each run's actual edit count and profile.
- Fixed per-run/per-condition RNGs keep scripts identical across configurations.
  Both datasets share one batch per condition/configuration. At most three CLI
  processes overlap; batch mode resets context and learning for each input line.
  Commit scripts end in Enter; recall scripts do not. Recall strips reranker
  options and reuses the same pure-engine result for equivalent configurations.
- Whole-run accuracy requires exact committed-text equality. Character accuracy
  is `max(0, 1 - total Levenshtein distance / reference characters)`, reusing
  `typo_eval.py`. Recall@16 requires the entire reference in the engine list.
  False correction means output differs from **that configuration's normal,
  canonical exact output**, irrespective of whether the baseline is correct;
  exact is therefore 0% by definition and exact-fast measures speed-induced
  changes. This is not the off-mode correctness-conditioned metric in typo_eval.

JSON contains every input script, output, N-best list, metrics, artifact hashes,
and wall time; it is checkpointed after each result row. Empty conditions report
N=0 and undefined metrics rather than entering the denominator.

## Results

Measured 2026-10-27 (Asia/Taipei), starting at 2026-10-26T19:34:17.982916+00:00.
Wall time: **24 min 33 s** (1472.6 s), including both configurations,
all conditions, corpus loading and hashing. No tracked artifact changed during
measurement (26 CLI/corpus/lexicon/model files checked before and after).

- CLI SHA-256: `170b82f6ad951185d6c494f83fe968eec92771c290460af82ca498420011aa2b`.
- Lexicon SHA-256: `9e651e711661ae942a3bc4c4cd7c9422ec3794e7b828a82095c566ecc2f3ae60`.
- Python 3.15.0 through uv 0.12.7; default CPU-only Core ML.
- Full records: `.build-eval/typing-errors.json` (scratch artifact).
- Self-checks, the small batch pilots, and actual `--decoder slots` / `timed`
  argument pass-through checks passed. All 60 metric rows were independently
  recomputed from recorded outputs. All 8,535 distinct condition/run inputs were
  regenerated from seeds; paired configuration scripts and engine N-best matched.

Whole and Char describe committed output; Recall@16 always excludes the
reranker. A dash means the metric is not requested for that condition.

| Dataset | Condition | Config | N | Whole | Char | Recall@16 | False corr. |
|---|---|---|---:|---:|---:|---:|---:|
| colloquial | exact | current | 300 | 75.7% | 95.3% | 95.7% | 0.0% |
| colloquial | exact | current+reranker | 300 | 84.7% | 97.4% | 95.7% | 0.0% |
| colloquial | exact-fast | current | 300 | 75.7% | 95.3% | 95.7% | 0.0% |
| colloquial | exact-fast | current+reranker | 300 | 84.7% | 97.4% | 95.7% | 0.0% |
| colloquial | swap-in-syllable | current | 300 | 47.0% | 87.4% | 62.7% | — |
| colloquial | swap-in-syllable | current+reranker | 300 | 54.0% | 89.6% | 62.7% | — |
| colloquial | swap-across | current | 300 | 0.3% | 70.3% | 1.7% | — |
| colloquial | swap-across | current+reranker | 300 | 1.3% | 72.1% | 1.7% | — |
| colloquial | swap-across-2 | current | 300 | 0.0% | 50.8% | 0.3% | — |
| colloquial | swap-across-2 | current+reranker | 300 | 0.3% | 52.2% | 0.3% | — |
| colloquial | rollover | current | 300 | 29.7% | 83.2% | 45.0% | — |
| colloquial | rollover | current+reranker | 300 | 35.3% | 85.5% | 45.0% | — |
| colloquial | rollover-2 | current | 300 | 14.3% | 71.0% | 20.0% | — |
| colloquial | rollover-2 | current+reranker | 300 | 18.0% | 73.6% | 20.0% | — |
| colloquial | hotspot | current | 300 | 40.3% | 86.2% | 60.3% | — |
| colloquial | hotspot | current+reranker | 300 | 48.7% | 88.7% | 60.3% | — |
| colloquial | hotspot-2 | current | 300 | 16.3% | 75.5% | 27.7% | — |
| colloquial | hotspot-2 | current+reranker | 300 | 22.0% | 78.4% | 27.7% | — |
| colloquial | extra-key | current | 300 | 59.3% | 89.7% | 78.7% | — |
| colloquial | extra-key | current+reranker | 300 | 68.0% | 92.2% | 78.7% | — |
| colloquial | missing-key | current | 300 | 6.3% | 78.3% | 9.3% | — |
| colloquial | missing-key | current+reranker | 300 | 8.0% | 80.2% | 9.3% | — |
| colloquial | tone | current | 300 | 66.7% | 93.4% | 89.0% | — |
| colloquial | tone | current+reranker | 300 | 75.0% | 95.7% | 89.0% | — |
| colloquial | nospace-all | current | 243 | 0.0% | 60.0% | 0.0% | — |
| colloquial | nospace-all | current+reranker | 243 | 0.0% | 61.8% | 0.0% | — |
| colloquial | nospace-30 | current | 108 | 0.0% | 71.0% | 0.0% | — |
| colloquial | nospace-30 | current+reranker | 108 | 0.0% | 74.6% | 0.0% | — |
| colloquial | mixed | current | 300 | 34.7% | 79.5% | 48.3% | — |
| colloquial | mixed | current+reranker | 300 | 42.7% | 82.1% | 48.3% | — |
| wikipedia | exact | current | 300 | 71.7% | 95.7% | 94.0% | 0.0% |
| wikipedia | exact | current+reranker | 300 | 81.0% | 97.2% | 94.0% | 0.0% |
| wikipedia | exact-fast | current | 300 | 71.7% | 95.7% | 94.0% | 0.0% |
| wikipedia | exact-fast | current+reranker | 300 | 81.0% | 97.2% | 94.0% | 0.0% |
| wikipedia | swap-in-syllable | current | 300 | 48.0% | 90.0% | 62.0% | — |
| wikipedia | swap-in-syllable | current+reranker | 300 | 55.0% | 91.6% | 62.0% | — |
| wikipedia | swap-across | current | 300 | 0.0% | 75.4% | 0.0% | — |
| wikipedia | swap-across | current+reranker | 300 | 0.0% | 77.4% | 0.0% | — |
| wikipedia | swap-across-2 | current | 300 | 0.0% | 60.0% | 0.0% | — |
| wikipedia | swap-across-2 | current+reranker | 300 | 0.0% | 61.0% | 0.0% | — |
| wikipedia | rollover | current | 300 | 35.7% | 86.3% | 47.7% | — |
| wikipedia | rollover | current+reranker | 300 | 40.3% | 87.7% | 47.7% | — |
| wikipedia | rollover-2 | current | 300 | 13.3% | 76.9% | 19.3% | — |
| wikipedia | rollover-2 | current+reranker | 300 | 15.7% | 78.2% | 19.3% | — |
| wikipedia | hotspot | current | 300 | 42.0% | 88.9% | 60.7% | — |
| wikipedia | hotspot | current+reranker | 300 | 50.7% | 90.7% | 60.7% | — |
| wikipedia | hotspot-2 | current | 300 | 19.0% | 80.6% | 31.3% | — |
| wikipedia | hotspot-2 | current+reranker | 300 | 28.7% | 83.2% | 31.3% | — |
| wikipedia | extra-key | current | 300 | 58.0% | 92.1% | 78.0% | — |
| wikipedia | extra-key | current+reranker | 300 | 65.7% | 93.6% | 78.0% | — |
| wikipedia | missing-key | current | 300 | 6.7% | 82.6% | 10.0% | — |
| wikipedia | missing-key | current+reranker | 300 | 8.3% | 84.5% | 10.0% | — |
| wikipedia | tone | current | 300 | 65.0% | 94.7% | 91.3% | — |
| wikipedia | tone | current+reranker | 300 | 75.7% | 96.4% | 91.3% | — |
| wikipedia | nospace-all | current | 255 | 0.0% | 62.5% | 0.0% | — |
| wikipedia | nospace-all | current+reranker | 255 | 0.0% | 64.3% | 0.0% | — |
| wikipedia | nospace-30 | current | 129 | 0.0% | 79.2% | 0.0% | — |
| wikipedia | nospace-30 | current+reranker | 129 | 0.0% | 80.4% | 0.0% | — |
| wikipedia | mixed | current | 300 | 26.3% | 79.5% | 34.7% | — |
| wikipedia | mixed | current+reranker | 300 | 29.7% | 81.9% | 34.7% | — |

The reranker improves clean whole-run accuracy from 75.7% to 84.7% on colloquial
and 71.7% to 81.0% on Wikipedia. Exact-fast reproduces exact outputs in both
configurations: speed-induced false correction is 0% on these samples. Exact's
0% is the same-configuration baseline by definition, not a claim of perfect
conversion.

Boundary failures dominate: one cross-syllable swap has only 1.7% / 0% recall;
one missing key has 9.3% / 10.0%. Both omission-of-Space conditions have 0% whole
accuracy and recall among runs with actual omissions. Raising reranker quality
alone cannot recover these absent candidates. Conversely, tone slips retain
89.0% / 91.3% recall, with reranked accuracy 75.0% / 75.7%.

Two hotspots reduce reranked whole accuracy from 48.7% / 50.7% (one) to
22.0% / 28.7% (two). Mixed achieves 42.7% / 29.7% with reranking versus
34.7% / 26.3% without; its realized fast counts are 94/300 and 75/300.
The 30% fast rate is sampled, not a quota. Rollover and ordinary-swap conditions
use independently drawn positions; their difference is not a paired timing-only
ablation. Compare configurations within a condition for that purpose.

These are synthetic-error dev-set results with lexicon-inferred readings, not
human typing calibration or held-out acceptance tests. The binary hash identifies
this snapshot even if concurrently edited Swift sources change afterward.


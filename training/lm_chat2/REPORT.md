# Chat2 experiment — 2026-10-27

**Stopped under the requested repeated-failure rule. Training/perplexity are validated; generation failed quality review and conversion is incomplete.**
The retrained models have substantially lower perplexity than the old chat LM,
especially on legislative dialogue, but the mixed model still loses to Wikipedia on the two
general speech-like dev sets. New multi-turn generation failed two sample
reviews and was stopped; **zero new generated turns were used**, so the one
million turn target was not achieved.

## Sources and quality

Counts below are retained training sentences after document reservation,
deduplication and all holdout exclusions. Raw sizes are downloaded HTTP bodies,
not model sizes. Full URLs, retrieval dates, individual SHA256s and generator
weight hashes are in [source-manifest.json](../../data/build/lm_chat2/source-manifest.json);
the two receipt-bundle hashes and raw sizes are in
[source-bundles.json](../../data/build/lm_chat2/source-bundles.json).

| Source | Version / license | Raw size | Retained sentences |
|---|---|---:|---:|
| Legislative Yuan Q&A, via [OpenFun LYAPI](https://v2.ly.govapi.tw/) and [gazette agenda metadata](https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda/tree/16860178b7fbf234201db7d5c030f3fbab869a7a) | 2024–2025 documents; metadata revision `16860178…`; OpenFun CC BY 4.0, originals [Taiwan OGDL v1](https://data.gov.tw/license) | 506 documents, 63.00 MB | 203,621 |
| [Chinese Wikipedia talk pages](https://zh.wikipedia.org/w/api.php) | 2026-10-27 API snapshot; exact revisions/history links retained; [CC BY-SA 4.0](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content) | 30 responses, 21.02 MB | 7,297 |
| [Common Voice sentence collector](https://github.com/common-voice/common-voice/blob/20fd76cd7d3b1e937fe99359b2ef3649b9c466ac/server/data/zh-TW/sentence-collector.txt) | Commit `20fd76cd…`; CC0 1.0 | Existing train file 0.22 MB | 8,309 |
| Compatible legacy v3 colloquial corpus | Qwen/Common Voice lineage, SHA256 `787ae7aa64dcf88a…`; [Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B/tree/b968826d9c46dd6066d109eabc6255188de91218) weights Apache-2.0, no output-use restriction | 320.39 MB | 5,894,993 |
| New Qwen multi-turn pilots | Same pinned checkpoint; 8 requests per pilot, both rejected | Raw responses retained | **0** |
| [data.gov.tw meeting minutes](https://data.gov.tw/dataset/16827) | OGDL v1; 2015-06-01 ODT sample, SHA256 `2906815305965d9d…` | 0.24 MB sample | **0: summary resolutions, no speaker turns** |
| Taiwan Tongues | TRAIL is outside the task's license allowlist | Existing baseline/eval only | **0 in both new variants** |

Initial 20-pair samples per real source exposed chair/agenda/attendance text and
wiki signature/quotation artifacts. Filtering was tightened before model fitting;
the preliminary splits/preparation were archived. Gazette samples include
“這樣不好啊！” and “是啦，這個目前都正在改善當中”, alongside long policy answers.
Wiki samples contain real back-and-forth editing discussions, but also regional
vocabulary, disputes and awkward original writing. OpenCC **s2twp**, as in the
existing wiki pipeline, does not make all of this natural Taiwan messaging.
Signed source order is retained; wiki adjacency is not a verified reply-tree edge.
The current four-character minimum also drops minimal replies such as “對” or
“好啊”; their conversational coverage is not established by this experiment.
See [source_samples.json](../../data/build/lm_chat2/source_samples.json) and
[quality-review.json](../../data/build/lm_chat2/quality-review.json).

The first generator pilot mechanically retained 225 turns but included invented
facilities and inconsistent exchanges. A one-dialogue-per-request revision
mechanically retained only six turns: malformed nesting, repeated JSON and
incoherent exchanges persisted. Both pilots were excluded after sample review;
no large generation run followed. Deduplication/template-cap code is present but
has **not** been validated at million-turn scale.

## Models and holdouts

| Variant | Unique training sentences | Word tokens | Stored bigrams | Three TSVs |
|---|---:|---:|---:|---:|
| `lm_chat2` | 6,114,220 | 80,276,183 | 1,147,406 | 36,403,019 bytes |
| `lm_chat2/no_generated` | 219,227 | 5,563,978 | 363,934 | 13,753,556 bytes |

Reuse three-pass lexicon Viterbi hard EM and pruned MKN from the existing
pipeline; count threshold 2, natural-log TSVs, each variant below 60 MB.
73,435 clean adjacent pairs contribute supplemental last-word/first-word
boundary counts. No new neural model was trained.

Dev/test contain **1,000 real turn pairs each**, 900 gazette + 100 wiki, with
previous turn in TSV column 1 and target in column 2. Seed 20261027; entire
documents reserved across splits. Both columns and all 16 frozen old evaluation
files, including homophone TXT/JSONL and smoke sets, block exact and eight-character
matches, including normalized 台/臺 forms, before counting. Both models passed
separate full-corpus/context rescans and serialized probability normalization.
Dev/test hashes, document separation and cross-split eight-gram separation were
also checked after retrieval. Tests remain unscored. These holdouts are primarily
**parliamentary speech**, not a private-chat benchmark; leakage guarantees cover
the new word LMs, not retroactive exclusion from inherited wiki/v3/reranker models.

## Word perplexity

Same frozen **old chat final-E tokenizer** for all models; predict target words
and EOS, exclude BOS/context from denominator. Existing three-set wiki/chat
numbers reproduce the previous report. Dialogue predicts only target text,
using the previous final word when it ends in Han, otherwise BOS.

| Dev set | Lines | Predictions + EOS | Wiki | Old chat | Chat2 | No generated text |
|---|---:|---:|---:|---:|---:|---:|
| Wikipedia | 2,000 | 52,831 | 455.540 | 29,584.005 | 7,975.715 | 4,063.979 |
| Colloquial | 2,000 | 12,113 | 1,349.236 | 4,863.629 | 2,451.426 | 1,444.970 |
| Taiwan Tongues | 1,500 | 8,894 | 1,242.673 | 3,084.437 | 2,276.882 | 1,718.976 |
| Dialogue | 1,000 | 25,324 | 1,596.623 | 1,932.647 | 393.939 | 254.579 |

Novel tokens map to each model's `<unk>`; unknown counts and all per-line log
likelihoods are retained. Wiki has 1,034 unknown dialogue targets versus 11 for
each chat model, so vocabulary/fallback differences remain part of this metric.
The no-generated ablation has lower word perplexity than the mixed model on every dev set,
but still does not beat wiki on general colloquial or Taiwan Tongues text.

## Conversion and recommendation

**Stopped incomplete: 7/24 rows.** At `P=0.5`, dialogue evaluation lost a CLI worker to SIGKILL. Kernel journal records repeated global OOM kills during the row, including another job's Python process. Memory pressure is the likely cause; the exact victim-to-batch attribution was not recorded by the initial subprocess wrapper. No new-model conversion result exists, so **no new chat prior or default change is recommended**.

400 paired 4–20 Han runs/set, seed 7, 60 workers, frozen CLI and models; reranker count 16, v3 weight 0.5, requested chat priors. Wiki uses line context; dialogue scores only the target with preceding-turn context. Cells are exact / variant-equal percent. `—` means not completed.

| Chat LM | Prior | Colloquial | Taiwan Tongues | Wiki + context | Dialogue + context |
|---|---:|---:|---:|---:|---:|
| lm_chat | 0.3 | 85.50 / 85.50 | 70.50 / 84.75 | 84.00 / 84.00 | 85.75 / 85.75 |
| lm_chat | 0.5 | 85.50 / 85.50 | 70.00 / 84.00 | 84.25 / 84.25 | — |
| lm_chat | 0.7 | — | — | — | — |
| lm_chat2 | 0.3 | — | — | — | — |
| lm_chat2 | 0.5 | — | — | — | — |
| lm_chat2 | 0.7 | — | — | — | — |

Baseline P=0.3 means are 81.44% exact and 85.00% variant-equal. This does not select a prior for chat2. The no-generated ablation has word-perplexity results only. Ordered inputs/targets/predictions, partial summary and failure logs are retained; no test split was evaluated.

Runtime fingerprints of all 26 recorded files remain unchanged. The isolated workspace has no owned evaluation/CLI/model-server process running. The retry with saved-batch diagnostics exited during sync: a concurrent shared-runner edit now expects `nlp/engine`, while this experiment's snapshot preserves the original root layout. The original launcher is retained as `data/build/lm_chat2/frozen-runner.sh`; see README before resuming after host capacity is available.

## Execution and failures

- All fitting/scoring/conversion ran on `gpu-lab`; seven completed conversion rows
  use ONNX CPU, `JOBS=60`, in `~/zhuyintype/work/onnx-eval-chat2`. The separate
  CUDA pilot is described below. Python 3.12.15,
  uv 0.13.5, NumPy 2.5.4, OpenCC 1.4.2, mwparserfromhell 0.7.2,
  pyahocorasick 2.3.1, ONNX Runtime 1.24.4, Swift 6.4.1.
- Remote HTTP 429s stopped the first fetchers. Mac caching with slower requests
  recovered the wiki snapshot; a repeated upstream HTTP 500 ended the gazette
  request at 506/800 documents. Cached bodies/receipts were frozen, not silently
  replaced. One invalid API enum was corrected before extraction.
- Host RAM pressure stopped the first owned Qwen startup. A later owned server
  used private port 8112 and 0.80 GPU utilization after checking capacity; it
  was stopped by its PID after failed pilots. Only owned process PIDs received stop signals. Later kernel OOM events killed other processes on the shared host.
- A legacy-tab assertion stopped mixed preparation once. Reusing the old
  trainer's NFC/whitespace normalization fixed it; the partial tree was archived
  and mixed preparation/fitting/audit completed successfully.
- Hash-identical existing remote checkpoints seeded the isolated workspace
  after a slow transfer. Export/build logs, runtime identities, ordered
  predictions, hashes, validation JSON and nohup logs are retained under
  `data/build/lm_chat2/`; raw downloads under `data/raw/lm_chat2/`.
- A private fp32 CUDA batching pilot matched all 800 completed CPU predictions,
  but its two rows took 121.8/125.9 seconds versus CPU 66.0/66.4 under shared
  load. It was stopped by owned evaluator/worker PIDs; its owned server was
  cleaned up by the runner. The partial comparison retained the unchanged CPU rows.
- Five focused parser/context/leakage/MKN/error-receipt tests pass, plus shell syntax checks
  and the actual isolated runner. No app/Swift/docs/data-SOURCES changes by this task,
  dependency changes to existing environments, Git commit/stash/reset/checkout by this task,
  test-split scoring or Mac model evaluation. The shared runner has only the
  chat2 entry point and configurable local log destination added by this task.

Reproduction and file layout: [README.md](README.md). Preserve
[NOTICE.md](../../data/build/lm_chat2/NOTICE.md) and the compact 0.49 MB `attribution.jsonl` manifest when
redistributing these models.

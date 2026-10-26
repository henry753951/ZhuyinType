# ZhuyinType v3 — M3/M4 execution report

Training, export, compilation, CPU latency and interface validation are complete. Detailed results and scope limits follow.

## Scope and provenance

- Only `training/v3/` and `data/build/v3/` are written locally. No Swift, docs, existing training code, Git history changes, or engine integration changes.
- Remote: `$ZHUYIN_REMOTE_HOST`, `~/zhuyintype`; NVIDIA H100L-3-47C MIG, 48,128 MiB, initially idle; 384 GiB disk available.
- Existing remote runtime: Python 3.12.15, torch 2.13.0+cu130, coremltools 9.0, numpy 2.3.5. Existing `.venv` contains mwparserfromhell 0.7.2; reuse its site-packages after `.venv-gen` rather than change either environment.
- Read MODEL_V3.md, MODEL.md, all reranker Python files, lm/pipeline.py, corpus/README.md before implementation.
- The original remote lexicon was stale (`e7cc8a1e...`); Mac lexicon SHA256 `9e651e711661ae942a3bc4c4cd7c9422ec3794e7b828a82095c566ecc2f3ae60` was snapshotted under v3; the final Mac lexicon still matches it.
- All four holdouts already match Mac SHA256. They are exclusion-only; only dev is scored. Short exact holdout sentences are excluded in addition to the required 8-character overlap.

## Decisions

- Preserve original article and paragraph order; reuse existing Wikipedia markup cleaning and OpenCC s2twp. Do not manufacture extra paragraphs to meet the 10M target.
- Remove contaminated paragraphs entirely and break context at removals. Context and targets must both be leakage-free.
- No optional dialogue generation: existing corpus is available and core data/training/export work has priority.
- Core ML contract follows the task: stateless macOS 14 fp16 packages, candidate-only float32 log probabilities, projected per-layer context memory.
- Use bounded RangeDim shapes as documented by Apple: https://apple.github.io/coremltools/docs-guides/source/flexible-inputs.html .

## Initial implementation checks

- Remote invariant self-check passed: 3,303 valid changed readings from 10,000 attempted corruptions; every resulting input remains in the lexicon syllable inventory.
- Checked causal prefix invariance, candidate padding, probability normalization, projected memory shape/dtype, Torch tracing and shorter dynamic dimensions.
- A temporary remote transfer file was initially placed one level above v3 and immediately moved into v3; no existing remote data was replaced. All persistent outputs remain inside v3 (apart from the requested remote logs).
- Local uv environment: Python 3.12.14, torch 2.7.0, coremltools 9.0, numpy 2.3.5, uv 0.12.7; lockfile under training/v3, environment under data/build/v3/.venv.

- Wikipedia completed: 10,101,669 leakage-filtered paragraphs from 1,130,481 articles, 643,655,802 characters, 12.11 minutes; capped in dump order.
- Latest Core ML smoke check passed min/default/max shapes (Lc/Ls/C = 1/1/1, 64/10/32, 256/64/64); fp32-vs-fp16 logprob max differences 0, 0.01266, 0.02508 on random weights. These are export checks, not model-quality results.
- Explicit self- and cross-attention padding masks preserve predictions under padded syllables or context; invariant self-check rerun passed.

- Candidate inventory: 1,569 toned readings, 419 syllable tokens including specials; mapped-union candidate median 32, p95 89.6, maximum 190; 199 reading unions exceed the export C=64 ceiling. Training keeps the complete union; export scores any caller-supplied subset up to 64.

- Audited all encoded colloquial streams, including previous-line/current-line concatenation boundaries: zero heldout 8-gram matches. No checkpoint invalidation needed.
- Syllable context and reference inference map paragraph newlines to reserved SEP id 3; paragraph boundaries are not stripped or concatenated.

- Polyphone training reached its 1.5M-example cap in the colloquial source (1,500,001 examples after completing a word). No Wikipedia examples were added to that classifier; its internal accuracy does not establish coverage of Wikipedia-specific senses. Low-confidence filtering and reading priors remain necessary.

- Every lexicon candidate is represented in training: characters absent from the corpus vocabulary share UNK id 2 as a negative bucket. Reference inference also maps unseen candidate characters to UNK; dev OOV positions count as errors.

- First syllable-stage launch failed before training while building dev readings: Torch checkpoint labels were tuples, but Lexicon.label compared lists, producing an empty allowed-reading set. Fixed tuple comparisons at the shared caller and added an actual checkpoint-backed ambiguous-character regression check. Encoded annotations already used tuple keys and are unaffected. Initial failure log retained at remote data/build/v3/first-syllable-failure.log. Resume uses --from-syllable and preserves the completed context checkpoint.

- Syllable training reached its initial 60,000-step ceiling without plateau (selected 57,000; NLL 0.1432105). Paused only the coordinator before evaluation/export, then continued from that selected checkpoint at 6e-5 initial LR with a fresh data seed and fresh optimizer. Cumulative steps/time and the original history are retained; completion requires the same four-evaluation plateau criterion.

- Wikipedia paragraph counts refer to nonempty cleaned source lines; list items and source soft-wrap lines remain separate blocks, preserving article order.
- On-the-fly augmentation: sigma_x=0.9 and sigma_y=0.45 key units; uniform 0-15% attempted syllable noise, invalid outcomes revert to clean. 一/不 sandhi 50%, neutral-as-first 30%, Taiwan sense alternatives 10%, implicit first-tone boundary 20%. Noise type weights: substitution/tone swap/transposition/extra/missing = 45/20/15/10/10.

## Live execution

Updated UTC: 2026-10-26T21:23:07.559490+00:00

Status: Complete: trained to plateau, exported on Mac, compiled, golden vectors checked and CPU latency measured.

Remote jobs use nohup, per-stage `logs/v3-*.log`, and stop at first failure. No automatic retries.

Wikipedia: **10,101,669 paragraphs**, 643,655,802 characters; 1,130,481 articles; 1,387,784 contaminated paragraphs removed; 12.11 minutes.

Colloquial: 7,208,847 inputs → **5,908,512 kept**; normalized duplicates 1,238,732, near duplicates 60,737, family cap removals 863, heldout/empty exclusions 3; 18.03 minutes. Family maximum 100 (cap 100).

64-permutation MinHash retrieval is probabilistic; exact 5-gram Jaccard confirms retrieved matches; family is normalized first 6 + last 4 characters.

Character vocabulary 22,791; single-character lexicon entries occurring in corpus 12,690. Vocabulary also retains observed punctuation, Latin and other corpus characters.

Polyphone classifier: 1,500,001 training-only word-supervised examples; 1033 classes, 760 character/reading pairs; 0.42 minutes. 9-character window, 48-dimensional embeddings, 192-hidden-unit MLP. Prediction is normalized over the central character's lexicon readings; p>=0.9 required.

Internal seeded 10% validation (not an independent annotated G2P benchmark):

```json
{
  "epoch": 8,
  "val_accuracy": 0.999019980430603,
  "confident_fraction": 0.9982133507728577,
  "confident_accuracy": 0.9995325207710266
}
```

Encoded corpus:

```json
{
  "characters": 748726011,
  "low_confidence_characters": 13920838,
  "examples": 26889527,
  "low_confidence_examples": 9939175,
  "confident_examples": 16950352,
  "tokens": 877258119
}
```

Encoding time: 3.31 minutes. Low-confidence examples are excluded from syllable training but retained for the context LM. Colloquial context is the preceding retained line; existing generated lines are independent messages, not genuine dialogue.

Full post-extraction leakage audit passed for wiki and colloquial; 20 encoded reading/context samples checked. Details: data/build/v3/data-audit.json.

| Model | Parameters | Steps | Selected step | Minutes | Selected dev NLL | Plateau reached |
|---|---:|---:|---:|---:|---:|---|
| context | 30,556,672 | 30000 | 26000 | 32.89 | 4.2591 | True |
| syllable | 37,097,735 | 64000 | 57000 | 40.00 | 0.1432 | True |

Both models: 6 layers, 512 width, 8 heads, 2048 FFN, pre-LN and RoPE. Context LM uses 256 positions and a tied character head; syllable self-attention/character embeddings initialize from it. The context encoder is frozen during syllable training; each syllable layer's cross K/V projections train and are copied into ContextEncoder export. Context drops for 30% of training batches. Wiki and colloquial shards are sampled by token/example count, with colloquial weighted twice. Validation every 1000 steps; improvement threshold 0.002 NLL; patience 4 after at least 5000 initial steps. Initial segment limits are 30000/60000 steps; syllable continuation uses 10000-step segments until plateau.

### Offline dev selection accuracy

Backend: PyTorch CUDA BF16. Core ML CPU parity is measured separately below.

All dev lines; longest-match + polyphone labels. Score nonliteral reading positions; OOV predictions count wrong. Candidate union includes gold, no candidate cap (selection quality conditional on recall). Literal-inclusive accuracy also given. Wikipedia context is preceding text on same line; colloquial additionally uses previous dev line. 5% requested noise can revert to clean if invalid.

| Dev | Context | Requested noise | Positions | Model top-1 % | Lexicon baseline % | Valid changed % | Candidates >64 |
|---|---|---:|---:|---:|---:|---:|---:|
| colloquial | False | 0% | 15837 | 95.05 | 65.79 | 0.00 | 5516 |
| colloquial | False | 5% | 15837 | 94.85 | 64.52 | 1.82 | 5513 |
| colloquial | True | 0% | 15837 | 94.86 | 65.79 | 0.00 | 5516 |
| colloquial | True | 5% | 15837 | 94.65 | 64.52 | 1.82 | 5513 |
| wikipedia | False | 0% | 60462 | 94.95 | 62.91 | 0.00 | 21490 |
| wikipedia | False | 5% | 60462 | 94.76 | 62.00 | 1.54 | 21455 |
| wikipedia | True | 0% | 60462 | 95.02 | 62.91 | 0.00 | 21490 |
| wikipedia | True | 5% | 60462 | 94.84 | 62.00 | 1.54 | 21455 |

Baseline uses the highest-prior **single-character** lexicon row for the typed reading. Training and offline selection evaluate the full candidate union, including gold; exported C<=64 requires caller pruning. This evaluates character selection conditional on gold recall, not the Swift decoder, whole-sentence correctness, or M4 engine adoption gate.

### Core ML CPU-only latency

Machine: Apple M5; macOS-27.0.1-arm64-arm-64bit. 10 warmups then 100 wall-clock predict calls including Python/Core ML bridge; model load and first compilation excluded; projected memory reused for syllable calls.

| Model | Length | p50 ms | p95 ms |
|---|---:|---:|---:|
| context | 64 | 8.553 | 10.667 |
| context | 128 | 13.590 | 18.521 |
| context | 256 | 22.665 | 29.515 |
| syllable | 10 | 2.766 | 3.315 |
| syllable | 30 | 3.962 | 5.039 |
| syllable | 60 | 9.369 | 12.542 |

Syllable Ls=30, C=32, Lc=256 target <=15 ms: p50 PASS, p95 PASS.

20 Core ML golden vectors passed shape/finite/normalization/parity checks. FP16 Core ML changed 2/504 argmax positions versus fp32 PyTorch on these synthetic interface cases. Maximum fp32 PyTorch logprob difference 0.072987; memory difference 0.089844. test_vectors.json contains exact CPU Core ML outputs, with float16 memories encoded as base64 little-endian bytes and SHA256. Core ML comparisons use atol/rtol 0.002; fp32 PyTorch uses atol 0.1. Both mlpackages compiled with the requested Xcode coremlcompiler.

### Limits

No Swift changes, installation, engine integration, decoder recall, end-to-end UI tests, optional dialogue generation, int8 export, or held-out test scoring. Existing independent colloquial messages provide sequential context without conversational guarantees. Polyphone confidence relies on word-derived labels and has not been human-calibrated. Core ML CPU target is reported as measured, not assumed. Models are not promoted to app resources.

## Artifacts and handoff

The two fp16 packages total 129.38 MiB (64.43 + 64.95 MiB); compiled models total 129.42 MiB.

Mac artifacts: `data/build/v3/{ContextEncoder,SyllableEncoder}.{mlpackage,mlmodelc}`, `chars.json`, `syllables.json`, `config.json`, `test_vectors.json`, `syllable.pt`, `polyphone.pt`, and `articles.tsv.gz`. `training/v3/reference.py` supports CPU Core ML and PyTorch; both backends were executed using `data/build/v3/example.json`.

Raw ordered corpus and encoded shards remain on the remote under `~/zhuyintype/data/build/v3/`: `wiki.txt`, `colloquial.txt`, `wiki_shards/`, `encoded/`. No app-resource promotion or Git commit was performed.

Paired checkpoint SHA256: `03c27edd7632e6fa650b656691219dd4dc3a7cc7e52a3fc1d7efb38905ffd133`. Exact CPU Core ML outputs are authoritative for Swift comparison; PyTorch is an approximate numerical reference.

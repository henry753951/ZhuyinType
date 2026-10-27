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

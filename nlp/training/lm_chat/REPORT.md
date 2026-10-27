# Chat word LM execution report

**Complete and validated; dev results do not support assuming that a chat-default mix improves real-speech prediction.**

## Scope and reproduction

- Writes only `nlp/training/lm_chat/` and `data/build/lm_chat/`, plus the explicitly requested remote `logs/lm-chat-*.log`. No Swift/docs/existing training edits, dependency changes, Git commits, stash, reset or checkout.
- The strict write boundary conflicts with the requested sibling corpus paths. Both holdouts are delivered as `data/build/lm_chat/{test,dev}_chat_tt.txt`; the integrating engineer must promote them if the corpus paths are required.
- Read all five requested files first. Reuse `pipeline.py` Viterbi, three segmentation passes, count merging and MKN directly. `train2.py` delegates estimation to KenLM; no new estimator or dependency is introduced.
- Remote: `$ZHUYIN_REMOTE_HOST`, `~/zhuyintype`; existing uv-managed Python 3.12.15, NumPy 2.5.4, OpenCC 1.4.2, mwparserfromhell 0.7.2, pyahocorasick 2.3.1. 48 CPU workers; no GPU.
- Run: `nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 .venv/bin/python -u nlp/training/lm_chat/train.py --workers 48 > logs/lm-chat-train-20261027.log 2>&1 < /dev/null &`. See README.md for tests/input layout.
- Finished 2026-10-26T21:47:46.575354+00:00; 94.30 seconds. Remote checkpoints and frozen inputs remain under `data/build/lm_chat/`; local `training.log` preserves the nohup log. Code/input hashes are in stats.json. Remote shared pipeline/train2 hashes matched the Mac before execution.
- Snapshot current Mac lexicon (SHA256 `9e651e711661ae942a3bc4c4cd7c9422ec3794e7b828a82095c566ecc2f3ae60`) and baseline TSVs into the allowed remote input tree. All 11 copied inputs matched Mac hashes; all 11 original Mac inputs still matched after completion. Existing untracked `nlp/training/v3/` was left alone.

## Corpus and holdouts

| Item | Count |
|---|---:|
| Taiwan Tongues source train/test rows | 118,695 / 1,317 |
| Accepted unique zh-TW transcripts, union | 17,137 |
| Already in train_commonvoice.txt (exact) | 7,677 |
| New test / dev holdouts | 1,500 / 1,500 |
| v3 input / retained | 5,908,512 / 5,906,465 |
| v3 holdout-overlap removals | 2,047 |
| Additional TT training / duplicates / heldout exclusions | 2,162 / 7,589 / 7,386 |
| Final unique training sentences / characters | 5,908,627 / 105,034,180 |
| Final training word tokens | 74,800,171 |

- Parse the untrusted sources only as TSV; ignore audio paths and participant metadata. Keep only locale `zh-TW`, >=2 Han characters and no Latin run longer than 3. NFC + whitespace normalization, exact deduplication; no Traditional conversion or additional domain filtering.
- Select before training with seed 20261027. Exclude train_commonvoice equality and old dev/test equality/8-grams; new holdouts also share no 8-grams with each other or internally. Selection provenance and hashes: splits.json. These are read-speech transcripts, not a validated spontaneous-chat benchmark.
- Filter the union corpus against equality and every contiguous 8-character substring from all four old and both new holdouts. Remove the whole matching sentence, then deduplicate across sources. Full independent post-export scan: 5,908,627 lines, zero duplicates, zero equality or 8-gram violations. New test is never scored or used for tuning.

## Model and validation

- Natural-log unigram.tsv / bigram.tsv / backoff.tsv: **28,219,837 bytes (28.22 MB)**. All root TSVs, including line_scores.tsv: 29,004,622 bytes; below 60 MB.
- 124,573 unigram targets, 1,536,963 observed pair types, 864,293 stored pairs, 24,013 stored contexts. Global count threshold 2; no dev-selected hyperparameters.
- Identical vocabulary/fallback handling, 0.1 hard-EM pseudocount, KN continuation floors, Chen–Goodman discounts and normalized post-pruning backoffs as the Wikipedia TSV pipeline. BOS context-only, EOS predicted, <unk> retained; no corpus repetition or spelling remapping.
- Discounts D1/D2/D3+: 0.612266260, 1.029344986, 1.437795207. Serialized unigram error 4.55e-14; maximum conditional normalization error across all contexts 4.89e-13; full-vocabulary enumeration error 1.15e-13.
- Three regression checks pass (TSV safety/filtering/deduplication; deterministic nonoverlapping holdouts; serialized pruned MKN and unknown lookup). Full training leakage audit passes. On Mac, all 15 original exported-file hashes match and all six perplexities reconstruct from line_scores.tsv within 1e-12 relative tolerance.

## Dev perplexity and domain scores

Both models score **identical frozen final chat E-step word tokens**. Include EOS, exclude BOS; natural logs. These numbers are tokenizer-dependent and are not directly comparable to old reports using a different tokenizer. Unknown targets and contexts map to <unk>; known absent-context backoff is zero.

| Dev set | Lines | Word predictions + EOS | Wikipedia PPL | Chat PPL | Chat / wiki |
|---|---:|---:|---:|---:|---:|
| dev_wikipedia | 2,000 | 52,831 | 455.540 | 29,584.005 | 64.94x |
| dev_colloquial | 2,000 | 12,113 | 1,349.236 | 4,863.629 | 3.60x |
| dev_chat_tt | 1,500 | 8,894 | 1,242.673 | 3,084.437 | 2.48x |

Per-line delta = logP(chat) − logP(wikipedia), nats. Full raw values and sentence text are in line_scores.tsv; all quantiles, standard deviations and per-prediction distributions are in stats.json.

| Dev set | Mean | p05 | p25 | Median | p75 | p95 | Delta > 0 |
|---|---:|---:|---:|---:|---:|---:|---:|
| dev_wikipedia | -110.245 | -262.045 | -144.820 | -92.559 | -53.375 | -22.334 | 0.10% |
| dev_colloquial | -7.766 | -28.090 | -15.463 | -7.819 | -0.855 | 12.245 | 23.15% |
| dev_chat_tt | -5.390 | -22.676 | -12.950 | -5.891 | 1.906 | 12.609 | 29.73% |

Chat is worse on all three heldout sets; the requested synthetic-heavy corpus does not establish a better real-speech LM. The dev delta distributions still separate encyclopedic from speech-like text, but raw zero-threshold/equal-prior classification favors chat on only 29.73% of new speech dev lines. Calibrate the domain prior/score scale before using these values as a posterior. No mixture weight or engine default was changed. Corpus-domain mismatch is a plausible explanation, not an established causal diagnosis.

## Sentence-start comparisons

Rank the 30 most frequent two-syllable readings by final segmented training word count, apportioned among polyphones by normalized seed reading mass. For each reading, maximize P(word|<s>) over exact-reading lexicon entries, without adding a reading prior. Top word happens to agree for all 30; their probabilities differ substantially. Full precision: sentence_starts.json.

| Rank | Reading | Wikipedia top | Chat top | Wiki BOS logp | Chat BOS logp |
|---:|---|---|---|---:|---:|
| 1 | ㄐㄧㄣ ㄊㄧㄢ | 今天 | 今天 | -8.643 | -2.540 |
| 2 | ㄏㄠˇ ㄒㄧㄤˋ | 好像 | 好像 | -11.805 | -6.558 |
| 3 | ㄐㄩㄝˊ ㄉㄜ˙ | 覺得 | 覺得 | -11.610 | -6.994 |
| 4 | ㄓㄣ ㄉㄜ˙ | 真的 | 真的 | -12.080 | -5.589 |
| 5 | ㄎㄢˋ ㄉㄠˋ | 看到 | 看到 | -10.260 | -5.674 |
| 6 | ㄎㄜˇ ㄧˇ | 可以 | 可以 | -7.873 | -6.409 |
| 7 | ㄕㄣˊ ㄇㄜ˙ | 什麼 | 什麼 | -11.373 | -9.745 |
| 8 | ㄧㄡˇ ㄖㄣˊ | 有人 | 有人 | -8.409 | -4.653 |
| 9 | ㄈㄚ ㄒㄧㄢˋ | 發現 | 發現 | -9.336 | -8.877 |
| 10 | ㄏㄞˊ ㄕ˙ | 還是 | 還是 | -10.620 | -8.768 |
| 11 | ㄨㄣˋ ㄊㄧˊ | 問題 | 問題 | -10.324 | -11.822 |
| 12 | ㄊㄚ ㄇㄣ˙ | 他們 | 他們 | -6.067 | -5.905 |
| 13 | ㄏㄡˋ ㄊㄧㄢ | 後天 | 後天 | -12.605 | -5.794 |
| 14 | ㄅㄨˋ ㄧㄠˋ | 不要 | 不要 | -10.281 | -9.439 |
| 15 | ㄅㄨˊ ㄧㄠˋ | 不要 | 不要 | -10.281 | -9.439 |
| 16 | ㄗㄨㄛˊ ㄊㄧㄢ | 昨天 | 昨天 | -12.589 | -4.192 |
| 17 | ㄕㄡˇ ㄐㄧ | 手機 | 手機 | -10.623 | -5.939 |
| 18 | ㄐㄧㄝ ㄍㄨㄛˇ | 結果 | 結果 | -7.840 | -8.669 |
| 19 | ㄐㄧㄝˊ ㄍㄨㄛˇ | 結果 | 結果 | -7.840 | -8.669 |
| 20 | ㄒㄧㄠˇ ㄏㄞˊ | 小孩 | 小孩 | -11.710 | -4.779 |
| 21 | ㄗˋ ㄐㄧˇ | 自己 | 自己 | -10.034 | -11.280 |
| 22 | ㄧㄡˇ ㄉㄧㄢˇ | 有點 | 有點 | -11.362 | -8.432 |
| 23 | ㄅㄧˇ ㄐㄧㄠˋ | 比較 | 比較 | -9.563 | -11.497 |
| 24 | ㄋㄚˋ ㄍㄜ˙ | 那個 | 那個 | -10.517 | -4.706 |
| 25 | ㄧ ㄒㄧㄚˋ | 一下 | 一下 | -13.045 | -12.831 |
| 26 | ㄧˊ ㄒㄧㄚˋ | 一下 | 一下 | -13.045 | -12.831 |
| 27 | ㄗㄨㄟˋ ㄐㄧㄣˋ | 最近 | 最近 | -8.476 | -5.434 |
| 28 | ㄓˇ ㄏㄠˇ | 只好 | 只好 | -13.330 | -10.198 |
| 29 | ㄅㄨˋ ㄋㄥˊ | 不能 | 不能 | -10.268 | -11.705 |
| 30 | ㄅㄧㄢˋ ㄉㄤ | 便當 | 便當 | -13.941 | -6.422 |

The requested ㄏㄨㄛˋ ㄕˋ example is supplementary (estimated 64 training occurrences). Both models choose 或是 by BOS alone. Including EOS in a complete one-word sentence produces the motivating difference: Wikipedia chooses 獲釋, chat chooses 或是. This is a TSV probability calculation, not a Swift decoder/UI test. Exact scores: example_scores.json.

| Model | Word | logP(word|BOS) | logP(EOS|word) | Sum |
|---|---|---:|---:|---:|
| wikipedia | 或是 | -10.156 | -5.866 | -16.022 |
| wikipedia | 獲釋 | -11.334 | -0.933 | -12.267 |
| chat | 或是 | -13.834 | -4.628 | -18.462 |
| chat | 獲釋 | -22.781 | -4.489 | -27.270 |

## Provenance and limits

- Taiwan Tongues publisher revision `ff0e8047bdce71c881c6d26eec7b2bbab6381ac1`: both pinned upstream train/test TSV hashes exactly match the supplied Mac files. source_receipts.json records URLs, sizes and hashes.
- [Publisher dataset card](https://huggingface.co/datasets/adi-gov-tw/Taiwan-Tongues-ASR-CE-dataset-zhtw/blob/ff0e8047bdce71c881c6d26eec7b2bbab6381ac1/README.md) labels the license `other`; [its LICENSE](https://huggingface.co/datasets/adi-gov-tw/Taiwan-Tongues-ASR-CE-dataset-zhtw/blob/ff0e8047bdce71c881c6d26eec7b2bbab6381ac1/LICENSE) supplies TRAIL v0.1. The exact license, use restrictions and attribution are retained. Do not label all artifacts CC0 or CC BY-SA-only. Distribution compatibility was not reviewed.
- Existing synthetic corpus: Alibaba/Qwen Qwen3-8B revision `b968826d9c46dd6066d109eabc6255188de91218`, Apache-2.0 weights, upstream license retained. Existing sentence-collector source has the project's pinned CC0 receipt; current lexicon has its original CC BY-SA/component attribution. See NOTICE.md.
- No training job failures or retries. One prelaunch hash check detected the still-transferring baseline bigram; the completed transfer then passed all hashes before launch. Regression checks initially emitted unclosed-file warnings; file handles were corrected before training and the checks passed cleanly.
- No Swift integration, conversion accuracy/UI check, heldout test scoring, mixture tuning, or redistribution-license compatibility determination. The new test set is held out from this chat LM; no broader claim about unrelated pretrained components is made.

# A1 statistical base — 2026-10-27

**In progress. No adoption or test-set result is claimed yet.**

## Protocol and controls

Frozen engine/evaluator snapshot and incumbent typing lexicon; 400 identical
4–20-Han runs per set, seed 7. Engine mode uses the chat/wiki mixture at prior
0.5 without neural models. App mode adds unchanged v2b top-16, rerank weight 1,
and v3 weight 0.5. Wikipedia is scored both without and with line context.
Taiwan Tongues reports exact and variant-equal scores. These are dev results,
not the test-derived head-to-head numbers in the task description.

Selection uses only dev: mean of colloquial exact, Wikipedia with-context exact,
and Taiwan Tongues variant-equal, preferring engine gains while retaining app
score. Wikipedia without context is a guard. Test sets are reserved for one
final baseline/selected comparison. All numbers below must come from saved
ordered predictions and paired `training/eval/stats.py` results.

## Lexicon and attribution

The McBopomofo reference copy contains 96,838 new-word rows and 7,741 additional
readings for incumbent words. Reusing the existing sandhi rule adds 504 rows.
All incumbent scores, Taiwan-reading preferences and 一/不 sandhi rows are
unchanged. Shared-word score calibration uses 70,013 words. Median natural-log
offsets for lengths 1, 2, 3, 4 and >=5 are respectively -1.8306133180,
-0.0611862028, -1.0844112260, -2.3281009149 and -5.3986204178.

McBopomofo's local license file and the upstream MIT notice agree: copyright
2011–2026 Mengjuei Hsieh et al. Upstream's data README confirms that phrase
mappings derive from BSD-licensed libtabe `tsi.src`. The merged lexicon retains
CC BY-SA 4.0 for existing CC-CEDICT/wordfreq data and the MIT/BSD notices.
Sources: [MIT notice](https://github.com/openvanilla/McBopomofo/blob/master/LICENSE.txt),
[data lineage](https://github.com/openvanilla/McBopomofo/blob/master/Source/Data/README.md).
Source/output SHA256s and all row counts are in `lexicon-merge.json`.

Ready-to-paste `nlp/data/SOURCES.md` table row (that file is deliberately untouched):

```markdown
| [McBopomofo 小麥注音](https://github.com/openvanilla/McBopomofo) 3.1.1 `data.txt`（本次取自 Shanjie 保留的原始副本） | 補充台灣詞彙、注音讀音與詞頻；既有讀音機率及一／不變調列保留 | MIT，Copyright (c) 2011–2026 Mengjuei Hsieh et al.；片語源自 libtabe `tsi.src`（BSD）；完整授權與來源 SHA256 隨 `base/NOTICE.md` 保存 |
```

## Counting and sources

Matched hard/expected models use one frozen incumbent final-E unigram tokenizer
per domain. New lexicon words receive calibrated seed priors. Exact
forward/backward edge and adjacent-edge joint posteriors replace the final hard
path; round once after all source-weighted expected counts, then reuse the
existing MKN/pruning/export/normalization verifier. This is a final E-step
replacement, not iterative soft EM. No implementation from reference engines
is copied.

Wikipedia uses incumbent cleaned/deduplicated training shards. Real chat uses
chat2's document-reserved Legislative Yuan/Wikipedia talk/Common Voice text,
plus Taiwan Tongues **train.tsv only**. The generated ablation reuses the old
Qwen pool at weight 0.1 after real-source deduplication. Every source is freshly
filtered against all current dev/test files and normalized eight-character
overlap. Inherited neural/wiki baseline training is not retroactively cleaned;
the leakage guarantee applies to the new word models.

Taiwan Tongues TRAIL terms remain with chat-derived research models; these are
not represented as CC BY-SA-only or release-ready assets.

## Execution and failures

All checks, fitting and conversion execute on the GPU host in the requested
isolated workspace. Python 3.12.15, NumPy 2.5.4, Swift 6.4.1; existing environments
are reused through a local uv-created virtual environment and read-only `.pth`
references. No dependency was installed into an existing environment.

The parser initially stopped on `_punctuation_list` containing a full-width
space. It now excludes control/punctuation rows before splitting fields; the
regression fixture covers that exact class. Remote checks compare posterior
counts against exhaustive path enumeration, BOS/EOS and OOV handling, and
assert incumbent reading/sandhi score preservation. These checks passed.

Other jobs share the host. Evaluation starts at 8 workers; fitting waits for at
least 32 GiB available host RAM. No unrelated process is stopped. Logs are under
`~/zhuyintype/logs/`; copied runner/build/evaluation logs remain under
`data/build/base/`. No test split has been scored at this stage.

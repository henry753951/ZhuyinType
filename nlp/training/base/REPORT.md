# A1 statistical base — 2026-10-27

**Selected on dev: McBopomofo lexicon + wider posterior-count Wikipedia LM +
fresh posterior-count real-text chat LM, generated weight 0.** The choice is
frozen in `selection.json`; word/typing checks pass and the single test pass is
complete. Balanced dev scores are 76.33 -> 78.75% engine and 85.83 -> 86.08% app.
Dev per-set gains are not significant at 0.05. Tests improve colloquial/Wikipedia
engine accuracy but regress on TT and smoke; **A1 acceptance is not achieved**.
The first captures with changing shared assets are archived, not acceptance evidence.

## Protocol

All fitting, probability checks, corpus audits and conversion run on the Linux
GPU host, in the requested isolated evaluation workspace. Mac activity is file
editing, transfer, snapshot preparation and artifact retrieval. No engine,
client, docs, CI, asset-manifest or shared lexicon-builder change is included.
No Git commit, push, stash, reset or checkout is performed by this task.

400 paired 4–20-Han runs per set, seed 7; the task-start typing lexicon fixes the
keys for every model. Wikipedia is measured without and with 32-character line
context. TT cells report exact / variant-equal. Engine mode uses the word-model
mixture with chat prior 0.5 and standard typo handling; app mode adds unchanged
v2b top-16, rerank weight 1 and v3 weight 0.5. The quoted 0c reference/test figures
in the task are not this dev cohort's baseline.

Dev selection uses the equal-weight mean of colloquial exact, Wikipedia with
context exact and TT variant-equal. Prefer engine gains while retaining app
primary score; Wikipedia without context is an additional guard. Paired results
use the repository's `stats.py`: fixed/broken, exact McNemar, 10,000 paired
bootstrap draws, 95% intervals and break-rate bounds. These exploratory trials
are not multiplicity-corrected evidence of statistical superiority. Test sets
are reserved for one final baseline/selected comparison after freezing assets.

## Lexicon

The corrected merge has **233,213 rows, 215,811 distinct words**, from 127,617
incumbent rows plus 97,852 new-word rows and 7,744 new readings for existing
words. It accepts extension-plane Han and valid one-syllable-per-character
readings. All incumbent scores and Taiwan-reading odds are unchanged. New
readings cannot exceed the incumbent best reading minus 1 nat. Source-supplied
and existing 一/不 variants remain; no new ordinal/numeral sandhi is inferred.

McBopomofo log10 scores become natural logs. Median offsets for shared words of
length 1, 2, 3, 4 and >=5 are -1.8306133180, -0.0611862028, -1.0844112260,
-2.3281009149 and -5.3986204178, using 70,021 shared words. Rejections: 2,319
control/punctuation rows, 1,534 non-Han rows and four invalid readings.

Source SHA256: `0deae7b7c1dcde1d7a30d139e7068543e0c0e7112e944b63eb52947bca1db7ac`.
Corrected output: `ac8c5e2275eed3da4637fe6407301b83f2be0310486a317ea632c7e21d1b08d6`.
See `lexicon-source-merge.json`. Earlier matched LM controls use the archived
232,700-row first merge (96,838 new-word rows, 7,741 new readings and 504 inferred
sandhi rows); that first merge is not the proposed source-only lexicon. The
corrected merge is separately measured; its 3,200 dev predictions match the
first merge under incumbent LMs.

The first full word audit, with incumbent LMs, covers 214,996 words in that first
merge: 68,638 (31.9252%) lose under every reading; 69,156 (32.1662%) lose under
their preferred reading. Per-row losses are 78,218 / 232,700 (33.6132%). The worst
classes are new single characters (8,917 / 8,918; 99.9888%), new two-character
rows (18,780 / 36,246; 51.8126%), and homophones (77.9241% versus 16.9839% for
unique readings). Homophones cannot all be top-1. A context-free McBopomofo
frequency-prior control is measured and rejected rather than boosting individual
words. The selected model receives a new complete audit below.

### Attribution ready for SOURCES.md

The reference's local MIT file agrees with [upstream McBopomofo's license](https://github.com/openvanilla/McBopomofo/blob/master/LICENSE.txt).
Its [data README](https://github.com/openvanilla/McBopomofo/blob/master/Source/Data/README.md)
confirms libtabe `tsi.src` BSD heritage. Full MIT and original libtabe 0.2.3 BSD
notices are retained in `licenses/`, including TaBE/Pai-Hsiang Hsiao, Academia
Sinica and Chih-Hao Tsai attribution. `libtabe-receipt.json` pins the archive URL,
member and SHA256. Existing CC-CEDICT/wordfreq CC BY-SA 4.0 and Unicode Unihan
notices remain applicable. No reference-engine implementation is copied.

```markdown
| [McBopomofo 小麥注音](https://github.com/openvanilla/McBopomofo) 3.1.1 `data.txt`（取自 Shanjie 保留的原始副本） | 補充台灣詞彙、注音讀音與詞頻；既有讀音機率及一／不變調列保留 | MIT，Copyright (c) 2011–2026 Mengjuei Hsieh et al.；片語源自 libtabe `tsi.src`（BSD）；完整授權與來源 SHA256 隨 `base/NOTICE.md` 保存 |
```

## Corpora and counting

| Corpus | Input sentences | Retained | Holdout-excluded | Duplicates removed |
|---|---:|---:|---:|---:|
| Wikipedia | 20,370,968 | 19,841,857 | 529,111 | Already deduplicated upstream |
| Real Taiwan text | 236,318 | 220,925 | 8,395 | 6,998 |
| Existing generated pool | 5,908,512 | 5,892,956 | 7,244 | 8,312 |

Wikipedia reuses the pinned cleaned article shards, CC BY-SA 4.0; exact revision
attribution is in `articles.tsv.gz`. Real text reuses Chat2's no-generated
Legislative Yuan, Wikipedia talk and Common Voice training shards, plus Taiwan
Tongues **train.tsv only**. Before the broader filter, Chat2 contributes 203,621
legislative, 7,297 talk and 8,309 Common Voice sentences; TT contributes 17,091
unique training sentences. Source manifests, attribution records and source
receipts accompany the word models. No new generated text is produced.

Every new corpus is rescanned against **42** dev/test files, including both
Chat2 dialogue columns, homophone TXT/JSONL, smoke and imported external sets.
Exact and contiguous eight-character overlaps are excluded, including OpenCC
and 台/臺 exclusion forms. A separate full rescan finds zero retained overlaps;
all 42 exclusion-file hashes remain unchanged. All seven scored text files
match their training-exclusion hashes. The provided Chat2 no-generated model is
not substituted as a clean candidate: a broader audit of its 366,097 training
sentences/context sides finds 355 overlaps under the current rule. Refiltering
its real-source sentences recovers usable training data. Inherited lexicon,
legacy tokenizers and neural models are not retroactively retrained/cleaned.

The first matched controls freeze the incumbent final-E tokenizer per domain.
Exact forward/backward edge marginals and adjacent-edge joint posteriors replace
a single hard path, including BOS/EOS; there is no per-edge cutoff. Counts are
summed across shards and weighted sources, then rounded **once** for the existing
integer MKN count-of-count estimator. Stored TSVs use the existing normalized
pruned/backoff format. This is posterior-count bigram re-estimation with a fixed
segmentation prior, not a claim of full soft-EM convergence.

Fresh chat controls initialize from normalized lexicon priors and perform three
E steps with two empirical unigram M steps (0.1 pseudocount). Hard and posterior
variants share source/vocabulary/initialization; generated weight 0.1 also applies
to the M steps. Generated text still contributes many tokens because its corpus
is much larger. No fresh Chat2 turn-boundary augmentation is included.
The real corpus is dominated by parliamentary speech; everyday messaging
coverage remains limited.

TT raw scores must be read alongside variant-equal scores. In the first combined
hard model, 52 engine predictions differ only by accepted variants; 48 contain
gold 台 versus predicted 臺, and four contain 妳 versus 你. The incumbent has 21
variant-only engine errors. This explains much of the apparent raw TT regression;
literal raw accuracy is still reported, and TT selection uses the requested
variant-equal metric. `variant-audit.json` retains the counts.

The wider Wikipedia control reuses identical posterior counts: cap 3 million
produces 2,947,897 stored bigrams at count threshold 14; cap 6 million produces
5,357,836 at threshold 7. Both normalize, with maximum serialized context error
below 5e-13. Counts include 467,369,692.5271 word-token mass, 73,871,325 posterior
pair types and 41,851,010 nonzero rounded pair types. The six-syllable decoder
cannot use a longer entry as one word: 961 vocabulary entries exceed six Han;
these represent 0.0277103% of Wiki and 0.00545466% of fresh-chat posterior token
mass. Long words can still win through shorter paths; this residual mismatch is
reported, not patched in the engine.

Taiwan Tongues retains TRAIL / TWM Responsible AI License v0.1; chat-derived
research assets are not described as CC BY-SA-only or release-ready. Legislative
Yuan originals retain OGDL v1/OpenFun CC BY 4.0 metadata, Wikipedia talk CC BY-SA
4.0 and Common Voice CC0. Generated candidates also retain Qwen's license and
legacy provenance. Each final component has its own NOTICE.md.

A final data-only control addresses uniform zero-continuation floors overriding
calibrated lexical frequencies. `fallback.py` removes lexical floor words only
if no stored bigram/backoff references them: 33,385 Wiki words (unigram mass
0.0000797647000363) and 166,977 chat words (mass 0.0135700757306). With removed
mass M and context backoff alpha, Z = 1-alpha*M; stored probabilities divide by
Z, retained unigrams divide by 1-M, and alpha becomes alpha*(1-M)/Z. The engine's
existing lexicon fallback handles removed words. Serialized models pass the
independent normalization verifier; a tiny conditional-distribution check covers
the formula. This generic rule is measured on dev alongside the selected plain
models with the incumbent lexicon (the McBopomofo component ablation).

### Fit inventory

Token mass includes source weights and fractional posteriors before final MKN
rounding. Full precision, discounts, thresholds and normalization checks are in
each model's `stats.json` / `validation.json`.

| Fit | Word-token mass | Stored bigrams |
|---|---:|---:|
| `chat_hard` | 5225352.0000 | 373,672 |
| `chat_expected` | 5264683.3208 | 376,116 |
| `chat_generated01` | 12527943.4472 | 584,948 |
| `chat_fresh_hard` | 5030319.0000 | 388,377 |
| `chat_fresh_expected` | 4986505.5919 | 393,828 |
| `chat_fresh_generated01` | 11650408.4531 | 635,974 |
| `wiki_hard` | 464498221.0000 | 2,938,277 |
| `wiki_expected` | 467369692.5271 | 2,947,897 |
| `wiki_expected_wide` | Reuses `wiki_expected` counts | 5,357,836 |

## Execution, recovery and limits

Python 3.12.15, uv 0.13.5, NumPy 2.5.4, Swift 6.4.1, ONNX Runtime 1.24.4,
ONNX 1.20.1, OpenCC 1.4.2, mwparserfromhell 0.7.2, pyahocorasick 2.3.1.
A project-local uv virtual environment reuses existing remote packages through
read-only `.pth` references; no package is installed into an existing environment.
Conversion uses ONNX CPU on the GPU host, not a Mac model run. Concurrent jobs
are respected: workers are RAM-budgeted, fitting waits for capacity, and no
unrelated process or broad process-name pattern is stopped.

The first baseline accidentally used an older remote lexicon. The mutable shared
chat model also changed during evaluation, to Chat2's no-generated TSVs. A clean
habit replay exposed different outputs. Those confounded comparisons, habits
and word audit are quarantined in `archive/pre-freeze/`. The corrected baseline
uses task-start lexicon `9e651e…` and recovered task-start chat unigram `be917a…`.
A 26-file hash guard now pins private incumbent TSVs, evaluation texts, typing
lexicon, CLI and neural assets. Thirty-two standalone chat/prior rows had
identical complete input fingerprints and were reused with receipts; all
asset-dependent rows were rerun. These archived figures do not select a model.

One parser stop on `_punctuation_list` with a full-width space was fixed by
excluding control rows before field splitting; a regression fixture covers it.
One guessed libtabe archive URL was 404; the original SourceForge archive was
retrieved and pinned. A length diagnostic initially assumed a newer vocabulary
receipt existed for an older fit; its vocabulary is instead reconstructed from
the verified serialized order. None of these failed probes supplies model data.

Remote focused checks pass: posterior counts versus exhaustive path enumeration,
BOS/EOS/OOV handling, old reading/sandhi score preservation, extension Han and
punctuation parsing, frozen-file rejection, edit distance/variant equality and
frequency-prior normalization. CLI batches preserve diagnostics and stop on
nonzero status or wrong output length. Clean habit replay must exactly reproduce
its frozen dev predictions. Mac UI, memory and per-key latency are not validated.
The larger Wikipedia model requires integration-side compilation/latency checks.

## Results

All cells below are percentages; 400 runs/view. `chat_*` changes only chat
counts with the first merged lexicon and incumbent Wikipedia model. `wiki_*`
changes only Wikipedia counts with the incumbent chat model. `combined_*`
uses the corrected lexicon and wider Wikipedia model. `fresh` refits the chat
tokenizer; `generated01` adds generated counts at weight 0.1. `selected_no_mc`
keeps the selected new LMs but restores the incumbent lexicon. `unused_fallback`
is the normalized vocabulary restriction described above.

The selected `combined_chat_fresh_expected` has the highest engine primary
score among configurations retaining the incumbent app primary score. The
generated ablations lose balanced engine accuracy. The unused-floor fix has
78.58% engine / 86.17% app versus the chosen 78.75 / 86.08, so it is retained
as a rejected control. McBopomofo alone has 75.92% engine / 85.75% app, but
removing it from the new LMs gives only 72.17 / 84.67. **Integrate the lexicon
and word models together**: their segmentation vocabulary is coupled.

The final files are byte-identical to the selected dev model. The frozen guard
extends from 26 to 34 files, adding the seven final TSVs and the selection
receipt. Tests run once after that receipt and track this choice without
further selection.
### engine

| Configuration | Colloquial | Wiki | Wiki + context | TT exact / variant | Primary |
|---|---:|---:|---:|---:|---:|
| `baseline` | 78.75 | 70.00 | 71.50 | 73.50 / 78.75 | 76.33 |
| `lexicon` | 78.75 | 69.75 | 70.25 | 73.75 / 78.75 | 75.92 |
| `lexicon_source` | 78.75 | 69.75 | 70.25 | 73.75 / 78.75 | 75.92 |
| `chat_hard` | 79.50 | 71.75 | 70.75 | 72.25 / 81.50 | 77.25 |
| `chat_expected` | 79.25 | 71.50 | 70.75 | 71.75 / 81.25 | 77.08 |
| `chat_generated01` | 81.50 | 71.00 | 70.50 | 72.00 / 79.00 | 77.00 |
| `wiki_hard` | 79.75 | 70.50 | 70.75 | 64.50 / 77.00 | 75.83 |
| `wiki_expected` | 80.25 | 69.75 | 71.25 | 64.75 / 77.25 | 76.25 |
| `native_prior` | 75.00 | 73.00 | 70.50 | 75.00 / 78.50 | 74.67 |
| `wiki_expected_wide` | 81.25 | 71.75 | 71.25 | 66.25 / 79.00 | 77.17 |
| `combined_chat_hard` | 82.50 | 72.50 | 71.75 | 67.50 / 80.50 | 78.25 |
| `combined_chat_expected` | 82.50 | 72.25 | 71.75 | 67.50 / 80.50 | 78.25 |
| `combined_chat_generated01` | 83.00 | 70.25 | 71.50 | 67.00 / 79.75 | 78.08 |
| `combined_chat_fresh_hard` | 81.75 | 72.75 | 73.00 | 67.25 / 80.00 | 78.25 |
| `combined_chat_fresh_expected` | 82.50 | 72.50 | 73.25 | 67.50 / 80.50 | 78.75 |
| `combined_chat_fresh_generated01` | 82.50 | 71.50 | 71.75 | 67.25 / 79.75 | 78.00 |
| `unused_fallback` | 82.50 | 72.00 | 72.50 | 68.00 / 80.75 | 78.58 |
| `selected_no_mc` | 72.50 | 69.25 | 70.00 | 62.75 / 74.00 | 72.17 |

### app

| Configuration | Colloquial | Wiki | Wiki + context | TT exact / variant | Primary |
|---|---:|---:|---:|---:|---:|
| `baseline` | 87.00 | 80.50 | 85.75 | 70.50 / 84.75 | 85.83 |
| `lexicon` | 86.75 | 80.50 | 85.75 | 70.50 / 84.75 | 85.75 |
| `lexicon_source` | 86.75 | 80.50 | 85.75 | 70.50 / 84.75 | 85.75 |
| `chat_hard` | 86.50 | 80.75 | 85.25 | 71.25 / 85.50 | 85.75 |
| `chat_expected` | 86.75 | 81.00 | 84.75 | 71.25 / 85.50 | 85.67 |
| `chat_generated01` | 87.00 | 81.00 | 85.50 | 71.25 / 85.50 | 86.00 |
| `wiki_hard` | 86.75 | 80.00 | 85.25 | 70.50 / 84.75 | 85.58 |
| `wiki_expected` | 86.50 | 80.25 | 85.75 | 70.75 / 85.00 | 85.75 |
| `native_prior` | 86.75 | 81.75 | 85.50 | 71.75 / 85.75 | 86.00 |
| `wiki_expected_wide` | 87.00 | 79.75 | 85.75 | 71.25 / 85.50 | 86.08 |
| `combined_chat_hard` | 86.75 | 80.50 | 85.25 | 71.75 / 86.00 | 86.00 |
| `combined_chat_expected` | 86.50 | 81.00 | 85.50 | 71.50 / 85.75 | 85.92 |
| `combined_chat_generated01` | 87.00 | 80.00 | 85.50 | 71.50 / 85.75 | 86.08 |
| `combined_chat_fresh_hard` | 86.75 | 80.75 | 85.75 | 71.75 / 86.00 | 86.17 |
| `combined_chat_fresh_expected` | 86.75 | 80.50 | 85.75 | 71.50 / 85.75 | 86.08 |
| `combined_chat_fresh_generated01` | 87.00 | 80.50 | 85.50 | 71.50 / 85.75 | 86.08 |
| `unused_fallback` | 87.00 | 80.50 | 85.50 | 71.75 / 86.00 | 86.17 |
| `selected_no_mc` | 84.75 | 79.50 | 84.75 | 70.50 / 84.50 | 84.67 |


### Selected vs incumbent, paired dev

| Mode | Dev view | Fixed / broken | Delta pp [95% CI] | McNemar p |
|---|---|---:|---:|---:|
| engine | colloquial | 37 / 22 | +3.75 [+0.00, +7.50] | 0.0674 |
| engine | wikipedia | 24 / 14 | +2.50 [-0.50, +5.50] | 0.1433 |
| engine | wikipedia_context | 21 / 14 | +1.75 [-1.00, +4.75] | 0.3105 |
| engine | taiwan_tongues | 23 / 16 | +1.75 [-1.25, +5.00] | 0.3368 |
| app | colloquial | 8 / 9 | -0.25 [-2.25, +1.75] | 1.0000 |
| app | wikipedia | 7 / 7 | +0.00 [-1.75, +1.75] | 1.0000 |
| app | wikipedia_context | 7 / 7 | +0.00 [-1.75, +1.75] | 1.0000 |
| app | taiwan_tongues | 6 / 2 | +1.00 [-0.25, +2.50] | 0.2891 |

TT uses variant-equal; other views use raw exact. Intervals are paired row bootstrap; p-values are uncorrected exact McNemar.

### Complete selected-word audit

All 233,213 reading rows / 215,811 words were queried through the actual engine,
with standard typo handling and the selected word models. A word with several
readings is counted as losing under every reading only if none wins.

| Class | Loses / total | Loss % |
|---|---:|---:|
| Words, every reading loses | 66,499 / 215,811 | 30.8135 |
| Words, preferred reading loses | 67,019 / 215,811 | 31.0545 |
| All reading rows | 76,039 / 233,213 | 32.6050 |
| New single-character rows | 9,921 / 9,931 | 99.8993 |
| New two-character rows | 19,205 / 36,234 | 53.0027 |
| Homophone reading rows | 50,451 / 64,487 | 78.2344 |
| Unique full-word reading rows | 25,588 / 168,726 | 15.1654 |
| Three-character rows | 5,255 / 51,400 | 10.2237 |
| Four-character rows | 1,639 / 43,596 | 3.7595 |

Rows at the uniform continuation floor in both LMs lose 17,490 / 35,574
(49.1651%); one LM at the floor loses 49,784 / 145,658 (34.1787%); neither at
the floor loses 8,765 / 51,981 (16.8619%). This association and the source-code
fallback override motivate the generic vocabulary control, but its lower engine
primary score prevents adoption. The first-merge/incumbent-LM audit above uses a
different dictionary; its loss percentage is not a paired lexical improvement.
Homophone collisions, rare/cold-word priors and long-word decomposition remain.
No individual word's score is hand-patched. Full failures, winners and source
classes are in `audit-combined_chat_fresh_expected.tsv`; aggregates and floor
classes are in the adjacent JSON files.

### Typing habits (dev)

| Dev set / habit | N | Engine before -> after | App before -> after | Engine oracle@16 before -> after |
|---|---:|---:|---:|---:|
| colloquial / clean | 400 | 78.75 -> 82.50 | 87.00 -> 86.75 | 96.75 -> 97.00 |
| colloquial / nospace | 320 | 61.25 -> 63.75 | 66.25 -> 65.94 | 75.31 -> 75.31 |
| colloquial / typo5 | 400 | 66.50 -> 70.00 | 75.25 -> 74.25 | 83.50 -> 83.50 |
| colloquial / neutral | 159 | 77.36 -> 81.13 | 87.42 -> 88.05 | 97.48 -> 97.48 |
| colloquial / sandhi | 53 | 81.13 -> 86.79 | 90.57 -> 88.68 | 98.11 -> 98.11 |
| wikipedia / clean | 400 | 70.00 -> 72.50 | 80.50 -> 80.50 | 93.50 -> 93.75 |
| wikipedia / nospace | 336 | 49.11 -> 48.81 | 55.65 -> 54.76 | 63.99 -> 63.39 |
| wikipedia / typo5 | 400 | 57.75 -> 58.25 | 66.25 -> 65.25 | 78.50 -> 77.25 |
| wikipedia / neutral | 129 | 67.44 -> 69.77 | 80.62 -> 80.62 | 93.02 -> 94.57 |
| wikipedia / sandhi | 48 | 60.42 -> 68.75 | 87.50 -> 81.25 | 87.50 -> 89.58 |

Shown accuracy is raw exact; oracle@16 is always measured with the non-neural engine. Clean replays exactly match the saved dev captures. All 40 raw/variant paired habit comparisons are saved next to the ordered predictions. App typo5 drops 1 point on both sets; app sandhi drops 1/53 colloquial and 3/48 Wikipedia runs. These tradeoffs prevent a blanket claim that the stronger engine lifts every app behavior.

### Frozen choice, test tracking only

The baseline and selected bundle were each run once in both modes: 20 completed views, 6,672 predictions. Selection and all 34 frozen inputs remain unchanged. TT cells are exact / variant-equal; other cells are exact. Smoke has 68 eligible runs (two three-Han lines are outside the shared 4–20-Han protocol).

| Test view | N | Engine before -> after | App before -> after |
|---|---:|---:|---:|
| colloquial | 400 | 71.00 -> 80.25 | 83.25 -> 84.00 |
| wikipedia | 400 | 66.50 -> 72.00 | 81.00 -> 82.50 |
| wikipedia_context | 400 | 67.00 -> 71.25 | 87.00 -> 86.50 |
| taiwan_tongues | 400 | 73.75 / 79.75 -> 65.75 / 76.50 | 74.25 / 85.50 -> 74.25 / 85.50 |
| smoke | 68 | 97.06 -> 92.65 | 100.00 -> 100.00 |

| Mode | Test view / metric | Fixed / broken | Delta pp [95% CI] | McNemar p |
|---|---|---:|---:|---:|
| engine | colloquial / raw | 47 / 10 | +9.25 [+5.75, +12.75] | 7.513e-07 |
| engine | wikipedia / raw | 36 / 14 | +5.50 [+2.00, +8.75] | 0.0026022 |
| engine | wikipedia_context / raw | 29 / 12 | +4.25 [+1.25, +7.50] | 0.011508 |
| engine | taiwan_tongues / raw | 22 / 54 | -8.00 [-12.25, -3.75] | 0.00031298 |
| engine | taiwan_tongues / variants | 22 / 35 | -3.25 [-7.00, +0.50] | 0.11116 |
| engine | smoke / raw | 2 / 5 | -4.41 [-11.76, +2.94] | 0.45312 |
| app | colloquial / raw | 9 / 6 | +0.75 [-1.25, +2.75] | 0.60724 |
| app | wikipedia / raw | 11 / 5 | +1.50 [-0.50, +3.50] | 0.21011 |
| app | wikipedia_context / raw | 7 / 9 | -0.50 [-2.50, +1.50] | 0.80362 |
| app | taiwan_tongues / raw | 7 / 7 | +0.00 [-1.75, +1.75] | 1 |
| app | taiwan_tongues / variants | 7 / 7 | +0.00 [-1.75, +1.75] | 1 |
| app | smoke / raw | 0 / 0 | +0.00 [+0.00, +0.00] | 1 |

Test primary means are 72.58 -> 76.00% engine and 85.25 -> 85.33% app. Colloquial and Wikipedia engine gains are supported by the paired results. Literal TT accuracy falls significantly; variant-equal TT and smoke have uncertain point regressions. App changes are small and individually inconclusive. These are tracking results: no other model is tested or reselected afterward.

A1 acceptance is **not achieved**. Colloquial remains below the historical 82.3% reference point, TT falls short, and the new base does not uniformly lift app behavior. The 0c reference run is not repeated in this frozen cohort, so these figures are not a direct comparison with 0c. Deliver this as a dev-selected research bundle, with the coupled lexicon/LM requirement, the TT/smoke regressions, app noise regressions and unmeasured Mac mmap/latency clearly carried into integration. Further decisions need fresh dev evidence.

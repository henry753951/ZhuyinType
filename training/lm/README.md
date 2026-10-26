# Reproduce the Taiwan Traditional word language model

This directory contains AGPL-3.0-only training code. All generated models, the
recalibrated lexicon, and the test sentences are **CC BY-SA 4.0** because they
derive from Chinese Wikipedia and the project's CC BY-SA seed lexicon. Keep the
generated `NOTICE.md` and `articles.tsv.gz` with redistributed model artifacts.

Only this directory and the ignored `data/build/lm/` directory are local outputs.
The original lexicon, Swift sources, data tools, and project documentation are
not modified. This produces the requested TSV training artifacts; app integration
and the quantized mmap binary described in `docs/MODEL.md` are separate work.

## Linux setup and run

Requirements: Linux, `curl`, GNU `sort`, approximately 50 GB available disk and
at least 64 GB RAM for 64 workers. The production run uses the CPU, not the GPU.
Python 3.12 and all dependencies are isolated in a uv-managed virtual environment.
No system packages or services need changing.

```sh
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST 'mkdir -p ~/zhuyintype/training/lm ~/zhuyintype/data/build ~/zhuyintype/logs'
rsync -a -e "ssh -i $ZHUYIN_REMOTE_KEY" training/lm/ \
  $ZHUYIN_REMOTE_HOST:zhuyintype/training/lm/
scp -i $ZHUYIN_REMOTE_KEY data/build/lexicon.tsv \
  $ZHUYIN_REMOTE_HOST:zhuyintype/data/build/lexicon.tsv
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST

cd ~/zhuyintype
# Only if uv is absent:
curl -LsSf https://astral.sh/uv/install.sh | sh
~/.local/bin/uv python install 3.12.15
~/.local/bin/uv venv --python 3.12.15 .venv
~/.local/bin/uv pip install --python .venv/bin/python -r training/lm/requirements.txt
nohup bash training/lm/run.sh > logs/pipeline.log 2>&1 < /dev/null &
echo $!
```

Poll with short SSH commands; no persistent SSH connection is needed:

```sh
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST \
  'tail -8 ~/zhuyintype/logs/pipeline.log; ps -u <remote-user> -o pid,etime,pcpu,rss,args --sort=-pcpu | head -8'
```

`WORKERS=64` is the default. `LM_PYTHON=/path/to/venv/bin/python` overrides the
interpreter. BLAS/OpenMP are limited to one thread per worker. Extraction, sentence
shards, all three segmentation passes, and the full separation audit use up to
64 processes. GNU sort also uses up to 64 threads; merging and final smoothing
are vectorized NumPy operations in one coordinator.

After `stats.json` reports `"status": "validated"`, copy outputs back:

```sh
mkdir -p data/build/lm
rsync -av -e "ssh -i $ZHUYIN_REMOTE_KEY" \
  $ZHUYIN_REMOTE_HOST:zhuyintype/data/build/lm/ data/build/lm/
```

`run.sh` first runs regression tests, verifies/downloads the dump, trains, and
independently verifies the serialized files. It exits on any failed step. Completed
stages have markers in remote `work/`; rerunning resumes them. Input hashes and
arguments must match the saved config. For changed cleaning/tokenization code,
different inputs, or different settings, use a **fresh work directory**. Never
reuse old checkpoints after changing the algorithm. Outputs are overwritten on
successful export; original inputs are never rewritten.

Individual commands, useful for a pinned historical reproduction:

```sh
.venv/bin/python training/lm/pipeline.py \
  --dump raw/zhwiki-latest-pages-articles.xml.bz2 \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm \
  --workers 64 --holdout 5000 --seed 20261026 --max-bigrams 3000000
.venv/bin/python training/lm/verify.py \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm --workers 64
```

For an exact historical run, fetch the **dated URL** and verify the hashes in
the delivered `stats.json` instead of downloading the moving `latest` alias.
Store that source's receipt as `raw/download.json`; `pipeline.py` requires it.
Dependency versions are pinned in `requirements.txt`; runtime and package
versions, input hashes, settings, and source receipt are also recorded in stats.

## Corpus, licensing, and attribution

Required source:

- Chinese Wikipedia `pages-articles.xml.bz2`, namespace 0, nonredirect articles.
  Latest URL: <https://dumps.wikimedia.org/zhwiki/latest/zhwiki-latest-pages-articles.xml.bz2>.
  [Wikimedia's reuse terms](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content)
  specify CC BY-SA 4.0 for text. Output license:
  <https://creativecommons.org/licenses/by-sa/4.0/>.
- `download.py` records the RSS-derived dated snapshot URL, dump date, download
  date, HTTP Last-Modified, byte size, Wikimedia-published SHA1, actual SHA1, and
  SHA256. It rejects a checksum mismatch, including a moving-alias race.
- `articles.tsv.gz` lists page ID, revision ID, and title for every article that
  supplied clean sentences before deduplication/holdout. It deliberately includes
  some articles whose sentences were subsequently removed. `NOTICE.md` explains
  exact-revision and author-history URLs for attribution.
- Seed lexicon: `data/build/lexicon.tsv`, CC BY-SA 4.0, from
  [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict),
  [Unicode Unihan](https://www.unicode.org/charts/unihan.html), and
  [wordfreq](https://github.com/rspeer/wordfreq); see the unchanged `data/SOURCES.md`.

The optional web corpus is omitted to keep this run bounded and its provenance
unambiguous. No subtitles, forums, synthetic chats, restricted research corpora,
or code from `references/` are used.

## Extraction, cleaning, and leakage prevention

1. Stream the bzip2 XML. Parse wikitext with `mwparserfromhell` in bounded batches.
   Omit templates, references, tables, code/math/gallery tags, media/interwiki
   links, external links, headings, and trailing reference/external-link sections.
   Templates are not expanded; this is a plain-prose corpus, not a MediaWiki render.
2. Resolve `-{zh-tw:...;zh-cn:...}-` variants toward `zh-tw`/`zh-hant`, convert with
   OpenCC **s2twp**, and normalize Unicode to NFC. Conversion outputs are retained
   as produced by OpenCC; examples are saved in `conversion_spotchecks.json`.
   In particular, it normally writes **臺灣**. The lexicon contains both 台灣 and
   臺灣, and their existing spellings/rows remain separate.
3. Split on `。！？；\n`, collapse internal whitespace, and keep sentences of
   **4–200 characters** with at least **60% Han**. Reject residual URLs, markup,
   table separators, and obvious code. The explicit task's length and exact-dedup
   rules override MODEL.md's shorter lengths and MinHash suggestion.
4. GNU sort with byte ordering (`LC_ALL=C`) performs global exact sentence dedup.
5. Uniform reservoir-sample **5,000 unique sentences**, seed **20261026**, before
   any segmentation or corpus frequency counting. Save `test_sentences.txt`.
6. Remove those sentences and every training sentence sharing **any contiguous
   8-character substring** with any test sentence. Aho-Corasick performs the full
   scan. Short test sentences are still removed by exact equality.

The test set is not used to choose discounts, pruning, or segmentation parameters.
Document counts refer to namespace-0 input documents and documents supplying
clean sentences before deduplication; sentence and character training totals are
after holdout and overlap removal. A character is a Unicode code point.

## Segmentation and modified Kneser–Ney

Read all `(reading, word, logp)` rows and sum `exp(logp)` over readings per word.
Normalize these masses and run exact unigram Viterbi against a word trie. Every
out-of-lexicon character is a one-character token, including punctuation, Latin
characters, digits, and spaces that survived filtering.

Run the initial lexicon-weighted segmentation, then **two M/E updates**, for three
total segmentation passes. Each update uses empirical token counts with a 0.1
pseudocount per vocabulary item to keep unseen words reachable. The final E-step
tokenizer is frozen for test evaluation. Only the final pass counts word bigrams,
with one `<s>` and `</s>` per sentence; no bigram crosses sentence boundaries.

For all observed pair types, compute count-of-counts `n1..n4` and Chen–Goodman
discounts:

```text
Y = n1 / (n1 + 2*n2)
Dr = r - (r+1)*Y*n[r+1]/n[r]      for r=1,2,3; D3 applies to counts >=3
Pcont(w) ∝ number of distinct preceding tokens of w
lambda(h) = sum_w D(count(h,w)) / count(h)
PMKN(w|h) = (count(h,w)-D(count(h,w)))/count(h) + lambda(h)*Pcont(w)
```

Missing count-of-count bins use discount 0.75; numerical bounds keep discounts
strictly between 0 and their count class. The unigram is the KN continuation
distribution. Every zero-continuation word receives 0.1 pseudo-continuations,
and `<unk>` receives at least 1, followed by renormalization. `<s>` is context-only
and is never a unigram prediction; `</s>` is included as a target. Additional
observed fallback characters are exported alongside every lexicon word.

Use the smallest global count threshold **>=2** that leaves at most **3,000,000**
pairs. Stored pairs retain their interpolated MKN probabilities. For retained
successors `K(h)`, recompute the exported backoff after pruning:

```text
alpha(h) = (1 - sum_{w in K(h)} PMKN(w|h)) / (1 - sum_{w in K(h)} Pcont(w))
```

Lookup is exactly:

```text
logP(w|h) = bigram[h,w]                       if stored
            backoff[h] + unigram[w]          otherwise
```

For a context absent from `backoff.tsv`, use log-backoff **0**. For a target absent
from `unigram.tsv`, substitute `<unk>`. This is a pruned interpolated MKN model;
the redistributed mass is normalized explicitly rather than keeping the original
interpolation coefficient after deleting pairs.

`lexicon_tw.tsv` preserves the exact seed `(reading,word)` rows and row order.
For corpus-seen words, the word's exported KN unigram mass is divided in proportion
to the seed reading probabilities. Thus all within-word polyphone odds are
unchanged. Unseen words retain their old row logp plus a shared scale shift: the
median of `new_word_logp - old_word_logp` over corpus-seen lexicon words. This
intentional unseen-word exception means the lexicon's total row mass is not
required to equal one; the LM's unigram and conditional distributions do normalize.

## Outputs and validation

| File | Contents |
| --- | --- |
| `unigram.tsv` | `word<TAB>natural-log probability`; every lexicon word, fallback characters, `</s>`, `<unk>` |
| `bigram.tsv` | `w1<TAB>w2<TAB>natural-log probability`; pruned interpolated MKN |
| `backoff.tsv` | `w1<TAB>natural-log backoff`; exactly the stored bigram contexts |
| `lexicon_tw.tsv` | Original header style and exact reading/word rows, recalibrated logp |
| `test_sentences.txt` | Fixed 5,000-sentence test set |
| `stats.json` | Counts, coverage/OOV, perplexity, source receipt/licenses, sizes/hashes, timing, runtime, decisions |
| `validation.json` | Verification of the exported text files and full leakage scan |
| `examples.json` | Exact top-five conditional predictions for 我、今天、台灣、的, plus 臺灣; flags stored/backoff results |
| `conversion_spotchecks.json` | Actual `s2twp` conversion examples |
| `articles.tsv.gz`, `NOTICE.md` | Source attribution and redistribution notice |

All TSV probabilities use natural logs with 12 decimal places. `stats.json.files`
records bytes and SHA256 for deliverables other than stats itself. Evaluation
uses identical frozen word tokens for unigram and bigram, counts EOS in the
perplexity denominator, and excludes BOS. Lexicon OOV includes fallback punctuation
and other non-Han tokens; `test_model_unk_rate` measures characters missing from
the trained model vocabulary. `test_unseen_training_token_rate` additionally
counts lexicon words that were never selected in training.

Regression checks cover Viterbi against exhaustive enumeration, extraction and
Taiwan conversions, hand-computed MKN interpolation, pruning normalization,
holdout exclusion, reading ratios, unseen-word calibration, and JSON serialization.
An end-to-end smoke run can use a smaller dump, `--holdout 100 --workers 4`, and
the same verifier. The final verifier reloads the TSVs, checks every conditional
distribution, explicitly enumerates the target vocabulary for 24 contexts,
verifies all lexicon rows/reading ratios/calibration, checks artifact hashes, and
rescans **all** training sentences for holdout equality and 8-character overlap.

Wikipedia converted to Taiwan orthography remains encyclopedic, mixed-origin
language: these counts do not establish representative Taiwanese conversational
usage. No typing accuracy, macOS integration, or mmap-size claim follows from
held-out Wikipedia perplexity.

## Mixed-corpus trigram (`lm2`)

The successor pipeline is `run2.sh` / `train2.py`; it leaves the old pipeline and
its fixed Wikipedia test untouched. See [../corpus/README.md](../corpus/README.md)
for the pinned CC0 source, exact Apache-2.0 generator checkpoint, cleaning rules,
and stop/resume commands. Run it on the existing Linux host from `~/zhuyintype`:

```sh
bash training/lm/build_kenlm.sh  # only if work2/kenlm/build/bin/lmplz is absent
nohup bash training/lm/run2.sh > logs/pipeline2.log 2>&1 < /dev/null &
```

The script waits for the frozen 1.5M synthetic phase, mines only reading-certain
lexicon additions, excludes every dev/test 8-gram from both sources, and reuses
`pipeline.py`'s exact lexicon Viterbi segmenter. An initial segmentation plus two
hard-EM updates uses Wikipedia + colloquial **once each**. This tokenizer then
stays frozen across all candidate LMs and all evaluation sets.

To train before phase one finishes, copy the current synthetic file to a temporary
file, truncate any incomplete final line, then atomically rename it to a frozen
snapshot. Leave generation and cleaning running. Pass that snapshot explicitly:

```sh
bash training/lm/run2.sh data/build/corpus/synthetic_snapshot1.txt
```

This skips the `PHASE1_READY` wait. Both mining and training accept the same
`--synthetic PATH` (relative to the project root); defaults are unchanged. The
audit and report read the selected input from `stats.json` and verify its hash
and complete sentence count instead of requiring 1.5M lines. Training config
includes the snapshot and Common Voice input hashes; changing inputs still
requires fresh training checkpoints. The prepared Wikipedia shards may be reused
when the frozen holdouts and preparation code are unchanged.

KenLM (pinned commit `4cb443e60b7bf2c0ddf3c745378f76cb59e254e5`) estimates modified
Kneser–Ney order 3. Candidates repeat the colloquial corpus **k=1,4,12** times,
keeping Wikipedia once. `--discount_fallback` is explicit because corpus
repetition can empty count-of-count bins. Select the minimum **token-weighted
union dev perplexity**, including EOS and excluding BOS; retain source-specific
dev scores. Neither test set selects model parameters. Unigram, bigram, and
trigram test perplexities use the selected model's identical frozen word tokens;
they are not directly comparable to the predecessor's different-tokenizer score.

Count pruning is `--prune 0 8 12` (KenLM removes counts <= thresholds), followed
if necessary by a global explicit-trigram log-probability cap. The cap accounts
for every bigram's trigram pointer and targets <148 decimal MB with headroom.
`lmbin.py` recomputes lower-order and trigram backoffs after pruning so deleted
probabilities return to the fallback distribution. Float32 scores are natural
logs, as specified in **unchanged** [FORMAT.md](FORMAT.md).

The old `prefer_tai.py` increases probability mass after estimation. The new
pipeline instead maps 臺→台 for **90% of training sentences**, only when the 台
word exists in the augmented lexicon, and leaves 10% as originally written.
This preserves both spellings without breaking normalization. Held-out spellings
are unchanged. Check the actual trained preference in the model report.

FORMAT.md requires both UTF-8 byte ordering and reserved IDs 0–2. ASCII
punctuation/whitespace from fallback segmentation would precede those reserved
spellings. Such tokens map to **U+FFFD** during training and evaluation; Chinese
vocabulary is unchanged, and the original corpus text is retained. The three
special spellings then really do sort first; the writer rejects an incompatible
ARPA rather than quietly violating the format.

The delivered `data/build/lm2/lexicon.tsv` preserves seed `(reading,word)` order
and polyphone ratios, then appends mined entries. Seen entries use the model's
unigram mass; unseen entries keep their seed scale. `unknown_word_shift` is the
median model-minus-seed word logp over seen shared words, so the engine can
calibrate the fallback for unseen lexicon entries. It is added **once**, at the
engine's unknown-word path. Do not also shift the unseen lexicon rows.

`lm2` contains `lm.bin`, `lm.json`, the recalibrated `lexicon.tsv`,
`new_words.tsv`, `stats.json`, `NOTICE.md`, article attribution, generator LICENSE,
and validation/sampling receipts. `stats.json` reports candidate dev perplexities,
selected test perplexities/OOV, pruning sizes, corpus counts, timings and hashes.
No Swift conversion-accuracy claim is made by this Python pipeline.

Writer/verifier and small regression tests:

```sh
.venv/bin/python training/lm/lmbin.py write model.arpa lm.bin
.venv/bin/python training/lm/lmbin.py verify lm.bin --samples 256
.venv/bin/python training/lm/test_lmbin.py
```

The verifier checks header/counts/length, section alignment and zero padding,
UTF-8 vocabulary ordering/reserved IDs, every pointer and sorted successor group,
finite log probabilities, empty-context backoffs, unigram normalization, sampled
bigram/trigram conditional normalization, and scalar/vector backoff agreement.
Writing uses a flushed temporary file and atomic rename after verification.
It does not run the Swift reader or measure IME conversion accuracy.

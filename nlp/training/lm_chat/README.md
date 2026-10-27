# Chat word bigram

Run on the existing Linux host, from `~/zhuyintype`, using the existing uv
environment. No installation or changes to the original pipelines are needed.

```sh
PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \
  .venv/bin/python nlp/training/lm_chat/test_train.py
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \
  .venv/bin/python -u nlp/training/lm_chat/train.py --workers 48 \
  > logs/lm-chat-train-20261027.log 2>&1 < /dev/null &
```

Frozen inputs live under `data/build/lm_chat/inputs/`: current Mac `lexicon.tsv`,
Taiwan Tongues `train.tsv`/`test.tsv`, `train_commonvoice.txt`, the four existing
dev/test files listed in `train.py`, Wikipedia TSVs under `wiki/`, both publisher
notices, Qwen's LICENSE, and source/transfer receipts. `NOTICE.md` records source
terms. `transfer_receipt.json` records the original Mac input paths and hashes.
The existing remote `data/build/v3/colloquial.txt` is read directly and hashed.

The holdouts live under `data/build/lm_chat/`, respecting the task's strict write
boundary; no files are installed into `data/build/corpus/`. `splits.json` records
seed, filtering, selection policy and hashes. They are excluded before training;
only the dev split is scored.

`work/` contains resumable sentence shards, segmentation scores/vocabulary,
counts, and a complete input/code configuration. A config mismatch aborts;
preserve the old run and use a fresh output/work tree before changing inputs or
algorithms. A completed unchanged run can be invoked again for verification.

Deliverables: the three natural-log TSVs (combined <=60,000,000 bytes), notices,
both new holdouts, `stats.json`, `splits.json`, `validation.json`, per-line
`line_scores.tsv`, and `sentence_starts.json`. `REPORT.md` has the human-readable
results. Perplexity uses both serialized models on identical final chat E-step
word tokens, includes EOS, and excludes BOS. Raw line delta is a sequence
log-likelihood ratio; its posterior interpretation still requires a domain prior
and calibration on representative preceding text.

Source TSVs are read only with `csv.DictReader`; audio paths and participant
metadata are ignored. Existing source, test and Swift files are never rewritten.

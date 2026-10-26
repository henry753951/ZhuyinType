# Homophone continuation

All code and outputs are isolated from the concurrently edited engine. Existing trainers
are imported read-only with bytecode writes disabled. Reuse Mac
`data/build/v3/.venv/bin/python` and remote `.venv-gen/bin/python`; no installs.

Run from the project root, prefixing commands with `PYTHONDONTWRITEBYTECODE=1`:

1. Freeze the release CLI once as `data/build/homophone/zhuyin-cli`; freeze lexicon,
   LM, reranker and six exclusion files under that tree. Copy only train.tsv from
   Taiwan Tongues into `inputs/`. `manifest.json` records hashes.
2. Remote: `python training/homophone/mine.py`. Counts use every eligible source once;
   standalone/word-initial counts use longest lexicon segmentation. The inventory is
   a curated Taiwan-confusion list extended from the requested seeds, then filtered
   for common toned readings and >=2,000 occurrences on each side.
3. Run the existing `training/corpus/serve.sh`, then
   `python training/homophone/judge.py`. All 在/再 positions are masked together;
   Qwen3-8B chooses without seeing their gold labels. Reject uncertainty/disagreement.
4. `python training/homophone/split.py` locks source-attributed text and metadata.
   Manually inspect `review100.jsonl` and record findings before training.
5. `python training/homophone/filter.py` builds clean indexes into the existing mmap
   shards, removing whole documents on any forbidden substring in target or context.
   It also keeps confident, unique Taiwan Tongues training examples.
6. `python training/homophone/train_v3.py` continues at 2e-5 with 30% focused batches,
   3x confusion loss, original augmentation/context dropout, and four-check dev
   plateau. Half the focused choices target 在/再; other groups share the remainder.
   The primary focused stream balances 在/再; its eligible Taiwan Tongues examples
   supply 25% of that stream. All eight original general-dev conditions must stay
   within 0.2 percentage points.
7. Copy the selected checkpoint to Mac `data/build/v3_homophone/`; run
   `python training/homophone/export_v3.py`, compile both packages with
   `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile`,
   then `python training/homophone/validate_v3.py` and
   `data/build/homophone/zhuyin-cli --check-v3 data/build/v3_homophone`.

Remote long jobs use nohup and `logs/homophone-*.log`; inspect each exit before the
next stage. There are no retries. Do not run the upstream prepare/train/finish entry
points: they would write into the baseline directories.

Example evaluation (omit `--line-context` for no context):

```sh
data/build/v3/.venv/bin/python training/homophone/eval.py \
  --test data/build/homophone/dev_zai.txt --output data/build/homophone/dev-example.json \
  --reranker data/build/homophone/reranker --v3 data/build/v3_homophone --line-context
```

`--model-only --backend coreml --v3 DIR` uses the existing reference runner. Its
top-64 pools include gold explicitly and measure conditional selection, not recall.
CLI evaluation types complete Han clauses, using the existing evaluator's key
mapping, without silently dropping untypable sentences. Position scoring uses edit
alignment; deletions count wrong. Sentence accuracy requires an exact full match.
Metadata retains group, toned reading, character, exclusive category and overlapping
category flags. Priority: fixed lexicon word, after recognized subject, initial,
before verb, before place/time, other. Subject/verb/place detection is heuristic;
the manual sample also audits it. “Sentence” here is a complete 4–31-Han-character clause;
preceding document text is kept as up to 32 characters of context.

Keep the test/smoke outcomes sealed until a dev winner is locked. Re-running eval
against an existing result path is refused. See REPORT.md for measured results and
pre-existing checkpoint exposure limits. Preserve source notices with artifacts.

`experiment.py baseline` runs engine, v2b, v3 and v2b+v3 on both dev sets with
and without context; `experiment.py new` evaluates isolated continuation artifacts.
Runs use four independent CLI processes; their total runtime is throughput under
contention, not a model-call latency measurement. The chat LM is frozen for optional
explicit experiments but is not loaded in this baseline; keep end-share at its
frozen CLI default 1 and reranker/v3 weights at 1. No domain parameter sweep.

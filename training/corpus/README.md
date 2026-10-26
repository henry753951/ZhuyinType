# Colloquial Taiwan Mandarin corpus

Code: AGPL-3.0-only. Common Voice input text: CC0-1.0. Mixed Wikipedia model/lexicon:
CC BY-SA 4.0. Raw generator weights are not redistributed.

The exact source URL, repository revision, SHA256, submission-license policy,
and split sizes are recorded in `data/build/corpus/splits.json`. Only Common
Voice's **sentence-collector.txt** is used, excluding the separately sourced
chatlogs/government files. These are donated **read-speech prompts**, which can
contain formal or unusual sentences; they are not a verified natural-chat test.
The old 5,000-line Wikipedia test is unchanged. A deterministic reservoir of
2,000 lines from the predecessor's Wikipedia training shards supplies dev.

`prepare.py` runs **before generation**. It NFC-normalizes, retains 4–60-character
Traditional lines, deduplicates, shuffles with seed 20261026, and freezes 3,000
test + 2,000 dev lines. Training removes equality and any 8-character overlap
with those holdouts. The later training preparation excludes all four holdouts
from *both* training sources; mining applies the same exclusion.

Generator: **Qwen/Qwen3-8B**, revision
`b968826d9c46dd6066d109eabc6255188de91218`. The exact checkpoint's LICENSE is
Apache-2.0, SHA256
`832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e`.
Its text is saved in `raw/Qwen3-8B-LICENSE`; it imposes no output-use restriction.
No Llama/Gemma model, forum scrape, subtitles, or non-commercial source is used.
The generator creates original messages rather than being asked to quote sources.

`generate.py` uses the Cartesian grid of **200 topics × 8 registers × 10 personas
× 4 lengths**, with additional seeded situations. It asks for 32 independent
messages per request, temperature .95, top-p .96, presence penalty .45, and
`enable_thinking=false`. Checkpoint weights are BF16; vLLM binds to loopback port
8102. `VLLM_USE_FLASHINFER_SAMPLER=0` avoids requiring a global CUDA toolkit for
FlashInfer JIT on this host. The installed environment is pinned in
`requirements.lock` (uv, Python 3.12.15); it is independent of the predecessor's
`.venv` training environment.

`clean.py` accepts 4–80-character lines with >=60% Han, **strict `s2t` identity**,
Taiwan-vocabulary filtering, and equality/8-character exclusion against **all
four** dev/test sets. This stricter identity rule exceeds the requested 90%
Traditional threshold. OpenCC also changes some valid Taiwan variants, including
吃→喫 and 台→臺: they are rejected, not silently normalized. This costs coverage
and is an explicit limit of the requested filter. `s2twp` output is accepted only
if its Han sequence is unchanged; Mainland-only terms otherwise cause rejection.
Exact dedup uses BLAKE2b-128. Near-dedup uses 64-permutation MinHash LSH with
character trigrams, threshold .85, and exact Jaccard confirmation of candidates.
LSH is probabilistic and does not guarantee finding every near-duplicate.

At exactly **1,500,000 clean sentences**, `synthetic_phase1.txt` is frozen and
`PHASE1_READY` is written. Further accepted lines append only to
`synthetic_background.txt`, toward **6,000,000 total clean sentences**. A response
may cross the final target by a few lines. Raw responses, request IDs, completion
reasons, token usage, and timestamps append to `raw/synthetic.jsonl`.
Checkpoint offsets refer only to complete, flushed responses. On resume the
cleaner rebuilds its MinHash index from both clean files, then continues at its
raw offset; the generator skips completed request IDs. A partial final raw record
is truncated on generator restart.

## Remote operation

All commands below run from `~/zhuyintype/` on
`ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`.

```sh
# Existing pinned checkpoint is in work2/Qwen3-8B/.
nohup bash training/corpus/serve.sh > logs/vllm.log 2>&1 < /dev/null &
echo $! > logs/vllm.pid
nohup .venv-gen/bin/python training/corpus/generate.py > logs/generation.log 2>&1 < /dev/null &
echo $! > logs/generation.pid
nohup env OPENBLAS_NUM_THREADS=1 .venv-gen/bin/python training/corpus/clean.py > logs/clean.log 2>&1 < /dev/null &
echo $! > logs/clean.pid
# Waits for phase one, mines additions, then trains/validates all three candidates.
nohup bash training/lm/run2.sh > logs/pipeline2.log 2>&1 < /dev/null &
echo $! > logs/pipeline2.pid
```

Inspect short-lived commands:

```sh
cat data/build/corpus/clean-progress.json
tail -5 logs/generation.log
tail -5 logs/pipeline2.log
```

Graceful stop (workers finish their in-flight requests; cleaner drains complete
responses before observing its stop file):

```sh
touch data/build/corpus/STOP_GENERATION
# Wait until the generator PID exits, then:
touch data/build/corpus/STOP_CLEANER
# After the cleaner exits, optionally stop the model server:
kill "$(cat logs/vllm.pid)"
```

Resume: remove both stop files, start the server if absent, and rerun the
`generate.py`/`clean.py` nohup commands above. Do not start duplicate processes;
check the recorded PIDs first. For a hard stop, send SIGTERM to the recorded
job PIDs only; append-only files and exact dedup make replay safe. Avoid killing
unrelated host processes. Generation/cleaning automatically stop at 6M total.

Validation: `.venv-gen/bin/python training/corpus/test_corpus.py`.
`mine_words.py` uses frequency >=200, minimum split PMI 2 and both boundary
entropies >=.6, at most 2,000 additions. Readings come from a unique seed reading
for **every** character, or an unambiguous CC-CEDICT word reading, with Taiwan
pronunciation preferred. It loads the existing pure pinyin conversion functions
from `data/tools/build_lexicon.py` without changing that file or adding wordfreq.
`new_words.tsv` retains counts, readings, evidence and cohesion for review.

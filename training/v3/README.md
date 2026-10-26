# ZhuyinType v3 neural training

All new code lives here; all artifacts live in `data/build/v3/`. Existing training files are imported read-only.
See `REPORT.md` for actual results and limits. Nothing is installed into the app.

## Existing environments

Remote: `ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`, project `~/zhuyintype`, Python `.venv-gen/bin/python` (Torch 2.13/CUDA, coremltools 9, NumPy, OpenCC, datasketch, pyahocorasick already installed). Wikipedia extraction appends the existing `.venv/lib/python3.12/site-packages` to Python's module search path to reuse mwparserfromhell 0.7.2. Neither environment is changed.

Mac: restore the isolated, locked environment with:

```sh
UV_PROJECT_ENVIRONMENT="$PWD/data/build/v3/.venv" uv sync --project training/v3 --python 3.12.14
```

This pins Mac Torch 2.7/coremltools 9. The existing project `.venv` is untouched.

## Data and jobs

Snapshot the current `data/build/lexicon.tsv` into remote `data/build/v3/lexicon.tsv`. All four holdout files named in `common.py` must exist remotely and match the Mac hashes. Do not alter them or train on them.

From the remote root, launch the two independent preparation jobs once:

```sh
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 .venv-gen/bin/python -u training/v3/prepare.py wiki > logs/v3-wiki.log 2>&1 < /dev/null &
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 .venv-gen/bin/python -u training/v3/prepare.py colloquial > logs/v3-colloquial.log 2>&1 < /dev/null &
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 .venv-gen/bin/python -u training/v3/pipeline.py > logs/v3-pipeline.log 2>&1 < /dev/null &
```

Inspect `data/build/v3/status.json`, stage logs, and processes using `pgrep -f '[p]ython -u .*training/v3/|[p]ython -u training/v3/'`. Do not start duplicate jobs. The pipeline waits for both data jobs, then runs vocab → polyphone → encoding → leakage audit → self-check → context LM → syllable training → dev quality → export. A failure stops the queue; there are no automatic retries. Check the cause before restarting an individual stage. The default pipeline is a fresh run: rerunning it trains anew. After a completed context stage, `pipeline.py --from-syllable` resumes the remaining stages without retraining context; it starts syllable training anew. If a syllable segment reaches its step limit before plateau, the pipeline automatically continues in 10,000-step segments using `train.py syllable --resume --steps 10000 --batch 64`: a fresh optimizer, a new sampling seed and initial LR 6e-5. Cumulative history/time are retained, and four non-improving validations are required before downstream export. This is continuation, not an automatic retry of a failed stage. Preparation is also a fresh run; do not reuse stale shard directories when changing extraction limits or inputs.

The XML dump is read in document order. Existing `lm/pipeline.py` handles markup removal and s2twp conversion; its 4–200-character/60%-Han sentence filters are applied inside each original paragraph. Accepted sentence punctuation remains. Articles and excluded paragraphs end context. `articles.tsv.gz` retains original page/revision/title attribution. Extraction stops at the first batch reaching ten million retained paragraphs.

Template normalization removes punctuation, replaces digits (including Chinese numerals) and explicit name slots, then drops identical normalized lines. A 64-permutation MinHash LSH retrieves potential 5-gram Jaccard >=0.8 duplicates for exact confirmation. Family = normalized first six + last four characters; cap 100. This is an approximate template detector, not a semantic duplicate guarantee.

All corpus characters remain distinct, so vocabulary can exceed the expected Han count. IDs 0–3 are reserved as specified. The lexicon supplies the toneless syllable inventory. Longest matching words provide labels; ambiguous single characters use the context-window classifier only at p>=0.9. Low-confidence examples are kept for the LM but excluded from syllable training. Context LM is frozen during the syllable stage. Cross K/V projection weights are trained by the syllable loss and exported with the context model.

Characters absent from the corpus vocabulary share UNK id 2; training retains one UNK bucket when a reading has unseen lexicon candidates. Reference inference maps unseen characters to UNK as well. Training keeps the entire mapped candidate union, including all candidates from three nearby readings and the true character. This can exceed 64. Export accepts caller-selected subsets of at most 64, as required. Dev numbers measure full-union selection with gold guaranteed, not retrieval or decoder accuracy. Padding memories are zero and masked; empty context is BOS alone.

## Mac finalization and inference

`data/build/v3/.venv/bin/python training/v3/finish.py` monitors the remote, updates REPORT.md, retrieves checkpoint/vocab/packages, exports again with the Mac's pinned Torch, compiles with Xcode, writes 20 vectors, checks parity, and measures CPU-only latency. It stops and records an unreachable remote or failed stage. Avoid launching it twice. Raw ordered corpora and mmap training shards stay on the remote; handoff models, vocabularies, checkpoints and statistics are copied to the Mac.

Manual finalization after copying final checkpoints and vocabularies:

```sh
data/build/v3/.venv/bin/python training/v3/export.py
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile data/build/v3/ContextEncoder.mlpackage data/build/v3/
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile data/build/v3/SyllableEncoder.mlpackage data/build/v3/
data/build/v3/.venv/bin/python training/v3/validate.py
```

Reference input JSON:

```json
{"context":"我們住在", "syllables":[["ㄊㄞ",2,0,0],["ㄨㄢ",1,0,0]], "candidates":[["臺","台"],["灣","彎"]]}
```

```sh
data/build/v3/.venv/bin/python training/v3/reference.py --input data/build/v3/example.json
# Or use the matching fp32 checkpoint:
data/build/v3/.venv/bin/python training/v3/reference.py --backend torch --input data/build/v3/example.json
```

`config.json` describes dimensions, names, IDs and interface constraints. Context text is limited to its last 255 characters and prepended with BOS. Map paragraph newlines to SEP id 3; preserve those boundaries. Memory outputs are `[6,Lc,512]` float16. They already contain each final syllable layer's K/V projection: do not re-project them. Use matching context/syllable packages from the same checkpoint.

`test_vectors.json` contains integer model inputs and exact CPU Core ML logprobs. Expected memory arrays are base64 little-endian contiguous float16 bytes with shape and SHA256; decode directly into the memory tensors. Each case's full source text, typed readings and candidate strings are included. Never interpret the memory strings as decimal arrays. A caller can either compare context outputs against those bytes or feed those bytes directly to isolate syllable parity. fp16 execution is not bitwise equivalent to fp32 PyTorch; tolerances and measured differences are recorded.

Model/data license: CC BY-SA 4.0 as in the existing corpus; scripts are AGPL-3.0-only. Preserve existing source notices and the Wikipedia page/revision attribution manifest when redistributing trained artifacts.

# Mapped lexicon and language models — 2026-10-27

Implementation and layout: [../lm/LEXICON_FORMAT.md](../lm/LEXICON_FORMAT.md).
Compiler: `scripts/compile-data.sh OUT_DIR` (existing `.venv`, standard library,
no installs or network). App/build-script integration is owned by the other
engineer; this change does not edit `Sources/ZhuyinTypeApp/`, `scripts/build-app.sh`,
or `docs/`. Concurrent edits in those locations and other Core files are preserved.

## Mac memory and startup

Release build command:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex3
```

Both measurements use the same full app configuration:
`--reranker data/build/reranker --rerank-count 16 --v3 data/build/v3
--v3-weight 0.5 --chat-prior 0.5 --timing`.
Before: `--lexicon data/build/lexicon.tsv --chat-lm data/build/lm_chat`.
After: `--lexicon data/build/memory/binary/lexicon.tsv
--chat-lm data/build/memory/binary/lm_chat`. The after directory contains binaries
and metadata only; successful loading proves no TSV is needed at those URLs.

These are one short process per sentence/configuration, not repeated benchmark
averages. Core ML models and CPU backend are the same on both paths. No evaluation
or training ran on the Mac; only builds, these short measurements, and the
explicitly requested Core checks.

| Input | TSV footprint now / peak | Binary footprint now / peak | TSV engine load | Binary engine load |
|---|---:|---:|---:|---:|
| 10 syllables | 333 / 337 MB | 16 / 20 MB | 1253.931 ms | 111.032 ms |
| 25 syllables | 339 / 349 MB | 22 / 32 MB | 1248.231 ms | 66.467 ms |

The CLI's MB unit is 1,048,576 bytes. Its Darwin memory metric is
`rusage_info_v6.ri_phys_footprint` / `ri_lifetime_max_phys_footprint`, as requested.
Clean, file-backed mappings are accounted differently from heap allocations;
16–22 MB is **not total resident pages or the installed app's GUI footprint**.
The three mapped binaries total 67,940,238 bytes (64.79 MiB), and model weights,
frameworks, UI and existing bounded lookup caches can add pages. The installed
input method has not been rebuilt/installed/measured by this task.

Exact CLI summary lines:

```text
Before, 10 syllables:
engine load: 1253.931 ms
  [keys 30: p50 15.3 ms, p95 33.6 ms, max 120.4 ms]
  [memory 333 MB now, 337 MB peak]
After, 10 syllables:
engine load: 111.032 ms
  [keys 30: p50 17.6 ms, p95 36.0 ms, max 111.9 ms]
  [memory 16 MB now, 20 MB peak]
Before, 25 syllables:
engine load: 1248.231 ms
  [keys 79: p50 34.5 ms, p95 57.1 ms, max 60.2 ms]
  [memory 339 MB now, 349 MB peak]
After, 25 syllables:
engine load: 66.467 ms
  [keys 79: p50 34.9 ms, p95 57.5 ms, max 64.3 ms]
  [memory 22 MB now, 32 MB peak]
```

Inputs (no Enter, composition retained):

- `今天晚上一起去吃飯吧` (10): `rup<space>wu0<space>j03g;7u<space>fu3fm4t<space>z04187`
- `今天晚上我們一起去吃飯然後再去公園散步順便買一些水` (25): `rup<space>wu0<space>j03g;7ji3ap7u<space>fu3fm4t<space>z04b06c.4y94fm4ej/<space>m06n041j4gjp41u04a93u<space>vu,<space>gjo3`

Raw logs, command arrays and model metadata are under `data/build/memory/`.

## Behavior checks

Mac: **3568/3570 checks passed**, including every new binary fixture, personal
overlay, malformed/truncated lexicon, mixed binary/TSV LM and ASCII/OOV backoff
check. The only two failures are existing latency assertions (`cold channel at
40 syllables under 3 ms`, `discovered LM aggressive neutral-tolerant handle under
3 ms`). Full output: `data/build/memory/mac-checks.log`. The suite ran once.

Linux behavior checks and the complete remote parity results are recorded below.

## Build outputs

| Binary | Bytes | SHA256 |
|---|---:|---|
| lexicon.bin | 17,012,566 | `17fa3b1af7739710eeb5cc6aa0ecbe6f5bcf5b5cdc07cf579e7019b7a53cbe16` |
| lm/lm.bin | 37,648,528 | `9d94cf72618525f54e482eb46a287cc6e1aee2ff807e350a3d2303dffa02d28e` |
| lm_chat/lm.bin | 13,279,144 | `2d41df7a0a9f8663f80febbf31d1e6ff4f719d5b7ff8d0094acd5c66d13f0d9e` |

The lexicon has 124,227 words, 105,061 readings and 127,617 entries. The LM
vocabularies contain 134,698 and 124,573 spellings; the mixture retains only
538,792 bytes of cross-vocabulary UInt32 IDs. All three binary hashes match
between independent Mac and Linux compilations. The Linux stdlib compiler test
passes (fixture structure, float scores, omitted unigrams, metadata SHA256 and
byte-for-byte deterministic recompilation). Mac Swift: Apple Swift 6.4
(`swiftlang-6.4.0.34.1`); compiler Python: existing 3.15.0 `.venv`. No dependency
or toolchain configuration changed.

## Remote validation setup

Isolated host workspace: `$ZHUYIN_REMOTE_HOST:~/zhuyintype/work/onnx-eval-memory`;
SSH key `$ZHUYIN_REMOTE_KEY`. Reuses the existing evaluation virtualenv by
symlink; existing ONNX exports were copied into this isolated workspace. No GPU
backend or training-directory writes. Existing `nlp/remote/run.sh`
changes belong to another job and were left intact; its sync list needed no edit.
The compiler and matrix were synced separately, then binaries compiled there.

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-memory JOBS=60 \
  nlp/remote/run.sh sweep \
  --matrix data/build/memory/matrix.json \
  --output data/build/remote_eval/memory -n 400
```

Matrix uses TSV vs binary configs with exactly the app's requested model flags:
`--reranker data/build/reranker --rerank-count 16 --v3 data/build/v3
--v3-weight 0.5 --chat-prior 0.5`, and the corresponding `--chat-lm` directory.
The binary config overrides only `--lexicon` and `--chat-lm`; both evaluator
configurations derive identical input keys from the original TSV lexicon.
Sets: dev_colloquial, dev_wikipedia, dev_chat_tt, Wikipedia `--line-context`,
colloquial `--split continue`, each `-n 400`, ONNX CPU, `JOBS=60`.

Source freeze: the remote runner copied the portable sources at invocation time.
Subsequent concurrent work adds a third CompositionToken syllable field and CLI
forms/boundary flags. Those changes were not overwritten or resynced into an
in-progress validation. The mmap reader, vocabulary/mixture implementation,
compiler and binary fixtures are byte-identical to the local sources; the Engine
snapshot differs only in the two concurrent syllable pattern arity changes.
The CLI's engine-load instrumentation is identical. Source SHA256 manifests and
the two remote source snapshots are saved in `data/build/memory/`.

Runner preparation: the initial checkpoint transfer was stopped by its own rsync
PID after verifying that the server already held all three checkpoint SHA256s.
Those checkpoints were copied locally on the server, and run.sh restarted with
`ENGINE_SNAPSHOT=.build-memory-snapshot`, a copy of the original remote portable
source snapshot. The first Linux build exposed an existing CLI Usage.swift
`getrusage(RUSAGE_SELF, ...)` enum/type mismatch. The concurrent engineer's existing
local fix (`__rusage_who_t(RUSAGE_SELF.rawValue)`) was copied into that snapshot;
no separate change to Usage.swift was authored here. Build/check logs retain the
successful retry. This did not require touching homophone or GPU processes.

Linux Core checks: **3561/3568 passed, zero behavior failures**, using the real
mapped lexicon and binary Wikipedia LM. The suite ran from `core-checks/`, whose
`data/build/{lexicon.tsv,lexicon.bin,lm}` link only to the frozen TSV input and
compiled binaries; the TSV parity directory was not changed. The optional lm2
benchmark is absent there, accounting for two fewer checks than the Mac. Seven
existing latency thresholds failed under shared host load: 20-syllable 8-best,
cold channel, discovered LM neutral-tolerant (normal/aggressive), synthetic
150 MB LM neutral-tolerant (normal/aggressive), and synthetic LM aggressive
first-tone. All fixture and real-data behavior checks passed. Real mapped
lexicon + LM init: 215.035 ms; lexicon alone: 120.927 ms. Logs:
`data/build/memory/memory-linux-{build,checks}.log`. Linux Swift 6.4.1.

One runner start also encountered a shell syntax error while run.sh was being
edited concurrently. A syntax-checked script snapshot was executed from one
in-memory read, preserving its original `$0` and argument behavior. The saved
script is `data/build/memory/runner-snapshot.sh`; no changes to run.sh were made.

A final Mac rebuild after the successful measured/check build encountered new
concurrent work outside this task: `Sources/ZhuyinML/RerankerTree.swift:41–42`
captures `self.ancestors` through closures before initialization. This file was
not changed here. The earlier Mac release build, all short memory measurements,
and the once-only Mac Core checks succeeded against the pre-tree source state;
the frozen Linux build also succeeds. See `data/build/memory/mac-final-build.log`.
The current shared working tree needs that separate build error resolved by its
owner before a fresh whole-app build can pass. No attempt was made to rewrite or
revert the concurrent reranker implementation.

The frozen CLI's default `rerankWeight` is 0.5. The requested command omits
`--rerank-weight`, so both memory measurements and both parity configurations
use that default, alongside explicit `--rerank-count 16`, `--v3-weight 0.5` and
`--chat-prior 0.5`. Accuracy percentages here should therefore be compared within
these frozen runs, not against documentation rows using reranker weight 1.0.

## Final parity result

**Passed: 2,000/2,000 aligned output lines identical (100%). No differences.**
Each of the five cases independently exceeds the required 99.5% threshold.
The comparison asserts identical ordered inputs and exactly 400 inputs/outputs
per case before comparing text. Runner and comparison both exited zero.

| Set | Identical lines | Parity | TSV / binary accuracy | TSV / binary seconds |
|---|---:|---:|---:|---:|
| colloquial | 400/400 | 100.00% | 82.0% / 82.0% | 118.52 / 98.29 |
| wikipedia | 400/400 | 100.00% | 79.0% / 79.0% | 134.57 / 150.65 |
| chat | 400/400 | 100.00% | 82.8% / 82.8% | 90.82 / 73.32 |
| context | 400/400 | 100.00% | 83.0% / 83.0% | 367.88 / 251.56 |
| continue | 400/400 | 100.00% | 78.5% / 78.5% | 81.22 / 73.94 |

Wall times reflect changing shared host load and are not a controlled decoding
speed comparison. Accuracy is unchanged in every row. No differences, including
float32 tie flips, were observed.

Artifacts: ordered inputs and outputs are in
`data/build/remote_eval/remote/memory/{tsv,binary}-*.json` (all ten rows);
`data/build/memory/parity.json` records the per-case counts and empty difference
lists, and `parity-summary.md` is the comparison table. The comparison script ran
on Linux; no Mac evaluation was used. The full runner log is
`data/build/memory/remote-run.log`. Neural checkpoint hashes match the existing
server copies; actual ONNX export hashes and Linux Python 3.12.15 are recorded in
`data/build/memory/linux-model-sha256.txt`. The final mmap source/compiler hashes
match between Mac and Linux in `final-{local,linux}-mmap-sha256.txt`.

## Handoff summary

| Metric | TSV | Mapped binaries |
|---|---:|---:|
| Mac footprint, 10 syllables | 333 MB (337 peak) | 16 MB (20 peak) |
| Mac footprint, 25 syllables | 339 MB (349 peak) | 22 MB (32 peak) |
| Mac engine load, 10 / 25 syllables | 1253.931 / 1248.231 ms | 111.032 / 66.467 ms |
| Remote output parity, 5 × 400 | Reference | 2,000/2,000 identical |

`phys_footprint` is the requested metric; clean mapped pages are accounted
differently from heap allocations. Total RSS and the installed input method UI
were not measured. Mac and Linux
behavior checks pass; timing-only failures remain as listed above. App packaging
and the concurrent RerankerTree build error remain outside this change.

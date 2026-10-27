# Mapped shipping data: ZTLX/1 and TSV-compatible ZTLM/1

Run from the repository root:

```sh
scripts/compile-data.sh OUT_DIR
```

The command uses the project's existing `.venv/bin/python` and the Python standard
library. It reads `data/build/lexicon.tsv`, `data/build/lm/{unigram,bigram,backoff}.tsv`
and the same three files in `lm_chat/`. There are no downloads, installs, training,
or timestamps. Identical inputs produce identical binary and JSON bytes on Mac
and Linux. OUT_DIR can be the app's staged Resources directory or `data/build`
itself (the source TSVs remain intact). It receives `lexicon.bin`, `lexicon.json`, `lm/lm.bin`,
`lm/lm.json`, `lm_chat/lm.bin`, and `lm_chat/lm.json`. Each JSON includes the
binary SHA256, size, format and individual source SHA256 hashes. Files are replaced
atomically; run the command successfully before packaging the output directory.
The compiler intentionally matches `scanTSV`'s skipped comments, malformed rows,
nonfinite scores, ASCII-space separation for LMs, and last-wins LM duplicates.
Lexicon duplicate rows and their source order are retained.

Engine accepts either `lexicon.bin` or the old `lexicon.tsv` URL. If the adjacent
same-stem `.bin` exists, the TSV need not exist. Otherwise it uses the TSV. Invalid
lexicon binaries throw rather than silently loading an unrequested data set.
Each LM directory independently prefers the mmap binary and falls back to TSV
if its binary cannot be loaded. Personal data formats are unchanged. An app
package should retain source license notices alongside these derived data.

## ZTLX/1

All integers and IEEE-754 scores are little endian. Sections start at 8-byte
boundaries, padded with zero bytes. String tables sort by raw UTF-8 bytes, with
strictly increasing keys. Strings have no terminators. Every table stays in
`Data(contentsOf:options:.alwaysMapped)`, on Darwin and Linux.

128-byte header (`<4s7I12Q`):

| Offset | Type | Meaning |
|---:|---|---|
| 0 | 4 bytes | `ZTLX` |
| 4 | u32 | version 1 |
| 8, 12, 16 | u32 each | W words, R readings, E entries |
| 20, 24, 28 | u32 each | B toneless bases, P prefixes, S syllables |
| 32..96 | 9 u64 | offsets of the sections below, in order |
| 104 | u64 | complete file size |
| 112, 120 | u64 each | reserved, zero |

A string reference is `(u32 blobOffset, u32 byteLength)`.

| Section | Record/count | Contents |
|---|---|---|
| Words | 24 bytes × W | string ref, reverse start, reverse count, best entry index, reserved zero |
| Readings | 16 bytes × R | string ref, first entry, entry count |
| Entries | 24 bytes × E | u32 word index, u32 reading index, f64 best word logp, f64 reading prior |
| Reverse refs | u32 × E | entry indices, grouped by word, original TSV order within word |
| Toneless bases | 16 bytes × B | string ref, first base ref, base ref count |
| Base refs | u32 × R | reading indices, grouped by toneless base |
| Prefixes | 8 bytes × P | string refs for every complete-syllable toneless prefix |
| Syllables | 8 bytes × S | string refs for all individual toned syllables |
| String blob | bytes | concatenated, interned UTF-8 strings |

Entries group by reading and retain TSV order within each reading. For each word,
the compiler computes `top = max(original logp)` and stores `top` and
`original logp - top`. Float64 preserves the engine's reading-prior arithmetic;
reverse lookup recovers the original score from their sum. The word table's best
entry selects the first maximum in source order, matching Swift's `max(by:)`.
The reader checks file/section bounds, string references/UTF-8/sort order, ranges,
entry/reference indices and finite scores before accessing the mapped tables.

Reading lookup, word lookup, toneless lookup and prefix membership use binary
search. Reverse lookup and the best-reading fallback use the precomputed refs.
Only the small syllable inventory and personal overlays are indexed in memory.
Word records refer to the lexicon's sorted word index; Engine resolves the LM ID
by spelling in the active model's sorted vocabulary when materializing entries,
so replacing a model never uses stale precompiled LM IDs. Lexicon words missing
from the LM receive `0x40000000 + wordIndex`; other personal IDs follow that range.
No full lexicon word-ID dictionary is built. Existing bounded lookup caches
retain the entries actually used while typing.

## TSV-compatible ZTLM/1

Array layout is exactly [FORMAT.md](FORMAT.md). The TSV importer must **not** use
`lmbin.write`'s ARPA renormalization or pruning: doing so would change conversions.
It writes the existing natural-log unigram, explicit bigram and backoff scores as
float32, with zero two-word backoffs and no trigrams.

The shipping TSVs contain ASCII tokens such as `!` that sort before `</s>`.
IDs 0, 1 and 2 remain `</s>`, `<s>` and `<unk>`; the remaining spellings sort by
UTF-8. This clarifies the original format's assumption that the reserved spellings
sort first. NGramLanguageModel checks the reserved spellings separately, then
binary-searches IDs 3 onward. Existing ARPA binaries remain readable.

`lm.json` adds `vocabulary_order: "reserved-then-utf8"`,
`tsv_compatibility: true`, and `missing_unigrams: [ids...]` for vocabulary entries
without TSV unigram rows (including `<s>` and the skipped `#` comment token).
The compatibility mode preserves BigramLanguageModel's exact fallback semantics:
unknown targets still receive the previous word's backoff, invalid history has
zero backoff, and words without unigrams use the supplied lexicon score. No
context gives zero backoff. This differs from the existing ARPA reader's unknown
word calibration; ARPA binaries without that metadata retain their old behavior.
The metadata is part of the model and must ship beside the binary.

MixtureLanguageModel uses encyclopedia IDs directly. Chat-only words encode as
`encyclopediaVocabularyCount + chatID`. A compact `u32[V_encyclopedia]` array maps
encyclopedia IDs to chat IDs (or UInt32.max). It builds once using transient
spellings, without a union dictionary or retained vocabulary strings. Binary,
TSV, and mixed-format model pairs all use the same mixture calculation.

## Checks

```sh
# Run compiler tests on the remote Linux workspace, not the Mac.
.venv/bin/python training/lm/test_compile_data.py
# Swift checks include independent binary fixtures, every truncated lexicon
# prefix, malformed offsets, personal overlays, ASCII IDs, and mixed-format LMs.
.build/release/ZhuyinCoreChecks
```

See [../memory/REPORT.md](../memory/REPORT.md) for measurements, exact evaluation
configuration and parity differences.

The existing ARPA-specific `lmbin.py verify` also checks normalized probability
mass and assumes all vocabulary spellings sort together. Use the TSV compiler
checks for this profile: the TSV reader historically omits comment/space tokens
and retains its original rounded scores, so renormalizing to satisfy the ARPA
verifier would change behavior.

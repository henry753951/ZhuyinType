# `lm.bin` — ZhuyinType n-gram language model

A backoff n-gram model (n ≤ 3) laid out so the engine can `mmap` the file and use it without parsing. All integers
are little-endian; every section starts on an 8-byte boundary (pad with zero bytes). Scores are natural logarithms.

## Header (64 bytes)

| Offset | Type | Field |
|---:|---|---|
| 0 | `[u8; 4]` | magic `ZTLM` |
| 4 | `u32` | version = 1 |
| 8 | `u32` | order (2 or 3) |
| 12 | `u32` | V — vocabulary size |
| 16 | `u32` | B — number of bigrams |
| 20 | `u32` | T — number of trigrams (0 when order = 2) |
| 24 | `u64` | offset of the vocabulary section |
| 32 | `u64` | offset of the unigram section |
| 40 | `u64` | offset of the bigram section |
| 48 | `u64` | offset of the trigram section (0 when order = 2) |
| 56 | `u64` | total file size |

## Vocabulary

Word ids are `0 ..< V`, assigned in **byte-wise ascending order of the UTF-8 spelling**. Ids 0, 1, 2 are reserved and
spelled `</s>`, `<s>`, `<unk>` (these happen to sort first byte-wise; the writer must check that they do).

- `u32 offsets[V + 1]` — byte offsets into the string blob; word `i` is `blob[offsets[i] ..< offsets[i+1]]`.
- `u8 blob[offsets[V]]` — the UTF-8 spellings, concatenated, no terminators.

## Unigrams

- `f32 logp[V]` — log P(w).
- `f32 backoff[V]` — log α(w) for w as a one-word context (0 when the word starts no bigram).

## Bigrams (grouped by the first word)

- `u32 start[V + 1]` — bigrams with first word `w1` are entries `start[w1] ..< start[w1+1]`.
- `u32 next[B]` — second word id; ascending within each group (binary search).
- `f32 logp[B]` — log P(w2 | w1).
- `f32 backoff[B]` — log α(w1 w2) as a two-word context (0 when it starts no trigram).

## Trigrams (grouped by the bigram that is their context)

Present only when order = 3.

- `u32 start[B + 1]` — trigrams whose context is bigram entry `b` are `start[b] ..< start[b+1]`.
- `u32 next[T]` — third word id; ascending within each group.
- `f32 logp[T]` — log P(w3 | w1 w2).

## Scoring

```
P(w3 | w1 w2) = trigram(w1 w2 w3)                     if present
              = α(w1 w2) + P(w3 | w2)                 if bigram (w1 w2) exists
              = P(w3 | w2)                            otherwise
P(w2 | w1)    = bigram(w1 w2)                         if present
              = α(w1) + P(w2)                         otherwise
```

Sentence start is `<s>`, end is `</s>`. A word missing from the vocabulary scores as `<unk>` — the engine instead uses
the lexicon's own log probability for words the model does not know, shifted by the calibration constant in
`lm.json`.

## `lm.json` (next to `lm.bin`)

```json
{ "format": "ZTLM/1", "order": 3, "unknown_word_shift": -2.3, "sources": [ ... ], "license": "CC BY-SA 4.0",
  "created": "2026-10-26T00:00:00Z", "sha256": "<of lm.bin>" }
```

`unknown_word_shift` is added to a lexicon log probability to put it on the model's scale.

## Tools

- `training/lm/lmbin.py` — writer and verifier (`python3 training/lm/lmbin.py verify lm.bin`).

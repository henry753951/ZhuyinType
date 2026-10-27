# `lm.bin`：ZhuyinType n-gram 語言模型

回退式（backoff）n-gram 模型（n ≤ 3），排列方式讓引擎可以直接 `mmap` 檔案使用，不需要解析。所有整數皆為
little-endian；每個區段都從 8 位元組邊界開始（以零位元組補齊）。分數為自然對數。

## 檔頭（64 位元組）

| Offset | Type | Field |
|---:|---|---|
| 0 | `[u8; 4]` | magic `ZTLM` |
| 4 | `u32` | version = 1 |
| 8 | `u32` | order（2 或 3） |
| 12 | `u32` | V：詞彙表大小 |
| 16 | `u32` | B：bigram 數量 |
| 20 | `u32` | T：trigram 數量（order = 2 時為 0） |
| 24 | `u64` | 詞彙表區段的 offset |
| 32 | `u64` | unigram 區段的 offset |
| 40 | `u64` | bigram 區段的 offset |
| 48 | `u64` | trigram 區段的 offset（order = 2 時為 0） |
| 56 | `u64` | 檔案總大小 |

## 詞彙表

詞 ID 為 `0 ..< V`。ID 0、1、2 保留，拼寫為 `</s>`、`<s>`、`<unk>`。
其餘的詞依 **UTF-8 拼寫的位元組遞增順序**排列，不含這三個拼寫。
ARPA 匯出會過濾 ASCII token，因此保留詞在全域排序中也排在最前面；出貨用的 TSV 模型保留 ASCII
token，並另外查找保留 ID。TSV 相容中繼資料、保留的 fallback 語意、編譯器與映射詞庫格式見
[LEXICON_FORMAT.md](LEXICON_FORMAT.md)。

- `u32 offsets[V + 1]`：字串 blob 內的位元組 offset；詞 `i` 為 `blob[offsets[i] ..< offsets[i+1]]`。
- `u8 blob[offsets[V]]`：串接在一起的 UTF-8 拼寫，沒有結尾字元。

## Unigram

- `f32 logp[V]`：log P(w)。
- `f32 backoff[V]`：w 作為單詞上下文時的 log α(w)（該詞不是任何 bigram 的開頭時為 0）。

## Bigram（依第一個詞分組）

- `u32 start[V + 1]`：第一個詞為 `w1` 的 bigram 是第 `start[w1] ..< start[w1+1]` 筆。
- `u32 next[B]`：第二個詞的 ID；在每組內遞增（供二分搜尋）。
- `f32 logp[B]`：log P(w2 | w1)。
- `f32 backoff[B]`：log α(w1 w2)，作為雙詞上下文（不是任何 trigram 的開頭時為 0）。

## Trigram（依作為上下文的 bigram 分組）

只在 order = 3 時存在。

- `u32 start[B + 1]`：以 bigram 第 `b` 筆為上下文的 trigram 是 `start[b] ..< start[b+1]`。
- `u32 next[T]`：第三個詞的 ID；在每組內遞增。
- `f32 logp[T]`：log P(w3 | w1 w2)。

## 計分

```
P(w3 | w1 w2) = trigram(w1 w2 w3)                     if present
              = α(w1 w2) + P(w3 | w2)                 if bigram (w1 w2) exists
              = P(w3 | w2)                            otherwise
P(w2 | w1)    = bigram(w1 w2)                         if present
              = α(w1) + P(w2)                         otherwise
```

句首為 `<s>`，句尾為 `</s>`。不在詞彙表中的詞以 `<unk>` 計分；對於模型不認識的詞，引擎改用詞庫本身的
對數機率，再加上 `lm.json` 中的校準常數。

## `lm.json`（與 `lm.bin` 放在同一處）

```json
{ "format": "ZTLM/1", "order": 3, "unknown_word_shift": -2.3, "sources": [ ... ], "license": "CC BY-SA 4.0",
  "created": "2026-10-26T00:00:00Z", "sha256": "<of lm.bin>" }
```

`unknown_word_shift` 加在詞庫的對數機率上，使其與模型的尺度一致。

## 工具

- `nlp/training/lm/lmbin.py`：寫入與驗證工具（`python3 nlp/training/lm/lmbin.py verify lm.bin`）。
- `scripts/compile-data.sh OUT_DIR`：確定性的出貨 TSV 編譯器；只使用專案既有的 `.venv` 與
  標準函式庫。其 TSV 相容設定由 `nlp/training/lm/test_compile_data.py` 與 Swift 二進位 fixture 檢查；
  上述 ARPA 驗證工具另外要求機率質量正規化與全域詞序。

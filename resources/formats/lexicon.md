# 映射式出貨資料：ZTLX/1 與 TSV 相容的 ZTLM/1

在 repository 根目錄執行：

```sh
zt data compile OUT_DIR      # 呼叫 ml/src/zhuyintype_ml/training/lm/compile_data.py；不給 OUT_DIR 時寫在資料目錄的 TSV 旁
```

這個指令只使用 Python 標準函式庫。它讀取 `data/build/lexicon.tsv`、
`data/build/lm/{unigram,bigram,backoff}.tsv`，以及 `lm_chat/` 中同樣的三個檔案。過程中沒有下載、
安裝、訓練或時間戳記。相同的輸入在 Mac 與 Linux 上產生位元組完全相同的二進位檔與 JSON。
OUT_DIR 可以是 App 暫存的 Resources 目錄，也可以是 `data/build` 本身（來源 TSV 保持不變）。
輸出為 `lexicon.bin`、`lexicon.json`、`lm/lm.bin`、`lm/lm.json`、`lm_chat/lm.bin` 與
`lm_chat/lm.json`。每個 JSON 都記錄二進位檔的 SHA256、大小、格式，以及各來源檔的 SHA256。
檔案以原子方式替換；打包輸出目錄前，先確認指令執行成功。
編譯器刻意與 `scanTSV` 的行為一致：略過註解、格式錯誤的列與非有限分數，LM 以 ASCII 空白分隔，
LM 重複列以最後一筆為準。詞庫的重複列與其來源順序則保留。

Engine 接受 `lexicon.bin` 或舊的 `lexicon.tsv` URL。若旁邊有同主檔名的 `.bin`，TSV 可以不存在；
否則使用 TSV。無效的詞庫二進位檔會拋出錯誤，不會默默載入未指定的資料集。
每個 LM 目錄各自優先使用 mmap 二進位檔，二進位檔無法載入時改用 TSV。個人資料格式不變。
App 套件應在這些衍生資料旁保留來源的授權聲明。

## ZTLX/1

所有整數與 IEEE-754 分數皆為 little endian。區段從 8 位元組邊界開始，以零位元組補齊。
字串表依原始 UTF-8 位元組排序，鍵嚴格遞增。字串沒有結尾字元。在 Darwin 與 Linux 上，
每個表都留在 `Data(contentsOf:options:.alwaysMapped)` 中。

128 位元組檔頭（`<4s7I12Q`）：

| Offset | Type | Meaning |
|---:|---|---|
| 0 | 4 bytes | `ZTLX` |
| 4 | u32 | version 1 |
| 8, 12, 16 | 各 u32 | W 個詞、R 個讀音、E 筆條目 |
| 20, 24, 28 | 各 u32 | B 個無聲調基底、P 個前綴、S 個音節 |
| 32..96 | 9 個 u64 | 下列各區段的 offset，依序排列 |
| 104 | u64 | 完整檔案大小 |
| 112, 120 | 各 u64 | 保留，為零 |

字串參照為 `(u32 blobOffset, u32 byteLength)`。

| Section | Record/count | Contents |
|---|---|---|
| Words | 24 bytes × W | 字串參照、反向起點、反向數量、最佳條目索引、保留零 |
| Readings | 16 bytes × R | 字串參照、第一筆條目、條目數量 |
| Entries | 24 bytes × E | u32 詞索引、u32 讀音索引、f64 最佳詞 logp、f64 讀音 prior |
| Reverse refs | u32 × E | 條目索引，依詞分組，詞內保留原始 TSV 順序 |
| Toneless bases | 16 bytes × B | 字串參照、第一個基底參照、基底參照數量 |
| Base refs | u32 × R | 讀音索引，依無聲調基底分組 |
| Prefixes | 8 bytes × P | 每個由完整音節組成的無聲調前綴的字串參照 |
| Syllables | 8 bytes × S | 所有個別帶聲調音節的字串參照 |
| String blob | bytes | 串接、去重後的 UTF-8 字串 |

條目依讀音分組，每個讀音內保留 TSV 順序。編譯器對每個詞計算 `top = max(original logp)`，
並儲存 `top` 與 `original logp - top`。Float64 保留引擎計算讀音 prior 時的算術結果；
反向查詢以兩者相加還原原始分數。詞表的最佳條目選擇來源順序中第一個最大值，與 Swift 的
`max(by:)` 一致。讀取端在存取映射表之前，先檢查檔案與區段邊界、字串參照、UTF-8 與排序、
範圍、條目與參照索引，以及分數是否有限。

讀音查詢、詞查詢、無聲調查詢與前綴成員判斷使用二分搜尋。反向查詢與最佳讀音 fallback
使用預先計算的參照。只有小型的音節清單與個人覆蓋層在記憶體中建立索引。
詞紀錄指向詞庫排序後的詞索引；Engine 在建立條目時，依拼寫在目前模型排序後的詞彙表中
解析 LM ID，因此替換模型時不會用到過時的預先編譯 LM ID。不在 LM 中的詞庫詞得到
`0x40000000 + wordIndex`；其他個人 ID 接在這個範圍之後。
不會建立完整的詞庫詞 ID 字典。既有的有上限查詢快取保留打字時實際用到的條目。

## TSV 相容的 ZTLM/1

陣列排列與 [FORMAT.md](lm.md) 完全相同。TSV 匯入器**不可**使用 `lmbin.write` 的 ARPA
重新正規化或剪枝，否則會改變轉換結果。它以 float32 寫入既有的自然對數 unigram、明確的 bigram
與 backoff 分數，雙詞 backoff 為零，且沒有 trigram。

出貨用的 TSV 含有 `!` 這類排在 `</s>` 之前的 ASCII token。
ID 0、1、2 仍為 `</s>`、`<s>`、`<unk>`；其餘拼寫依 UTF-8 排序。這釐清了原格式中「保留拼寫排在
最前面」的假設。NGramLanguageModel 另外檢查保留拼寫，再對 ID 3 以後做二分搜尋。
既有的 ARPA 二進位檔仍可讀取。

`lm.json` 增加 `vocabulary_order: "reserved-then-utf8"`、`tsv_compatibility: true`，
以及 `missing_unigrams: [ids...]`，列出沒有 TSV unigram 列的詞彙表項目（包括 `<s>` 與被略過的
`#` 註解 token）。相容模式保留 BigramLanguageModel 精確的 fallback 語意：
未知的目標詞仍會得到前一個詞的 backoff，無效的歷史 backoff 為零，沒有 unigram 的詞使用傳入的
詞庫分數。沒有上下文時 backoff 為零。這與既有 ARPA 讀取端對未知詞的校準不同；沒有這份中繼資料的
ARPA 二進位檔維持原本的行為。這份中繼資料是模型的一部分，必須與二進位檔一起出貨。

MixtureLanguageModel 直接使用百科 ID。只出現在聊天模型的詞編碼為
`encyclopediaVocabularyCount + chatID`。一個精簡的 `u32[V_encyclopedia]` 陣列將百科 ID
對應到聊天 ID（或 UInt32.max）。它只建立一次，使用暫時的拼寫，不建立聯集字典，也不保留詞彙字串。
二進位、TSV 與混合格式的模型組合都使用相同的混合計算。

## 檢查

```sh
python -m unittest zhuyintype_ml.training.lm.test_compile_data
# Swift 檢查包括獨立的二進位 fixture、每一種截斷的詞庫前綴、
# 格式錯誤的 offset、個人覆蓋層、ASCII ID 與混合格式 LM。
.build/release/ZhuyinCoreChecks
```

量測結果、確切的評測設定與一致性差異見 [../memory/REPORT.md](../../ml/src/zhuyintype_ml/training/memory/REPORT.md)。

既有、專用於 ARPA 的 `lmbin.py verify` 也會檢查機率質量是否正規化，並假設所有詞彙拼寫一起排序。
這個設定請改用 TSV 編譯器的檢查：TSV 讀取端歷來會略過註解與空白 token，並保留原本四捨五入後的
分數，為了通過 ARPA 驗證而重新正規化會改變行為。

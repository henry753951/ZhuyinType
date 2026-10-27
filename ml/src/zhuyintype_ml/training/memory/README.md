# 記憶體對應的詞庫與語言模型

把詞庫與兩個語言模型編譯成 mmap 二進位，並記錄改用二進位後的記憶體、載入時間與輸出一致性。

## 用途

- 0.4.0 起引擎以 mmap 讀取 `lexicon.bin` 與 `lm/lm.bin`、`lm_chat/lm.bin`，不再把 TSV 解析進記憶體。這個流程產生量測時用的二進位（`memory/binary/`）。
- app 建置與評測用的二進位由 `zt data compile` 產生，使用同一個編譯器。
- 量測結果見 [REPORT.md](REPORT.md)：打字時的記憶體占用從 333 MB 降到 16–22 MB，引擎載入從 1.25 秒降到 0.07–0.11 秒，遠端 2,000 行輸出與 TSV 版本完全相同。

## 指令

```sh
zt train memory                     # memory/binary/：lexicon.bin、lm/lm.bin、lm_chat/lm.bin 與各自的 JSON
zt data compile OUT                 # 同一個編譯器，輸出到任意目錄
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `compile` | 編譯 `lexicon.tsv`、`lm/` 與 `lm_chat/` 的 TSV；相同輸入產生逐位元組相同的檔案 | 任一（只用標準函式庫） |

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

- 讀取：`lexicon.tsv`、`lm/{unigram,bigram,backoff}.tsv`、`lm_chat/{unigram,bigram,backoff}.tsv`。
- 寫入：`memory/binary/lexicon.{bin,json}`、`memory/binary/lm/lm.{bin,json}`、`memory/binary/lm_chat/lm.{bin,json}`。JSON 記錄格式、大小、SHA-256 與來源檔的雜湊。
- `memory/` 底下的其他檔案（量測紀錄、矩陣、一致性結果）是 REPORT 的原始資料。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train memory` 的步驟；編譯器是 `ml/src/zhuyintype_ml/training/lm/compile_data.py` |

## 相關文件

- [REPORT.md](REPORT.md)：Mac 記憶體與載入時間、Linux 檢查、輸出一致性
- [../lm/FORMAT.md](../../../../../resources/formats/lm.md)、[../lm/LEXICON_FORMAT.md](../../../../../resources/formats/lexicon.md)：二進位格式
- [docs/engine/MODEL.md](../../../../../docs/engine/MODEL.md) §10

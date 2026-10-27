# v3：前文編碼器與音節對字編碼器

從維基百科與台灣口語語料訓練 0.3.0 的 v3：一個讀前文的字級語言模型，和一個依讀音與前文為每個位置的候選字評分的編碼器。

## 用途

- 產生 0.3.0 的 v3 模型與它的訓練資料：有序的維基百科段落、去重後的口語句、多音字讀音、編碼後的分片、兩個編碼器的檢查點、Core ML 套件與介面測試向量。
- 產物在資料目錄的 `v3_030/`。0.4.0 起 app 使用的 v3 是同音字流程從這裡繼續訓練的版本，見 [../homophone/README.md](../homophone/README.md)；它的起點、詞彙表與分片都取自 `v3_030/`。
- `models.py` 也由 `nlp/remote`（ONNX 匯出、模型伺服器）與 ZhuyinBench 匯入。

## 指令

```sh
zt train list                                # 列出所有流程與步驟
zt --remote train v3                         # GPU 主機：test 到 export；到 compile 會停止（compile 只能在 macOS 執行）
zt train v3 export compile validate          # Mac：取回 v3_030/ 的檢查點、詞彙表與 lexicon.tsv 之後
zt train v3 syllable --force                 # 重跑單一步驟
zt --remote train v3 --option workers=48     # 維基百科擷取的程序數（預設 24）
python -m nlp.training.v3.reference --input data/build/v3_030/example.json
python -m nlp.training.v3.reference --backend torch --data data/build/v3 --input case.json
```

步驟依 0.3.0 實際執行的順序排列：

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：讀音、打字變體、匯出介面 | 任一 |
| `snapshot` | 把 `lexicon.tsv` 與兩個開發集複製到 `v3_030/`（已存在就保留） | 任一 |
| `wiki` | 依傾印檔順序擷取維基百科段落，排除與保留集有 8 字重疊的段落 | GPU 主機 |
| `colloquial` | 口語句範本正規化、MinHash 近似去重、每個家族最多 100 句 | GPU 主機 |
| `vocab` | 字元與音節詞彙表、輸入雜湊 | GPU 主機 |
| `polyphone` | 多音單字的讀音分類器（只用單一讀音的詞當監督） | GPU 主機（CUDA） |
| `encode` | 編碼成 mmap 分片：字元、讀音、聲調、文件與訓練樣本 | GPU 主機 |
| `audit` | 洩漏稽核與分片檢查 | GPU 主機 |
| `selfcheck` | 以實際資料檢查讀音、變體、UNK 候選與匯出介面 | GPU 主機 |
| `context` | 前文語言模型訓練到開發集不再進步（30,000 步上限） | GPU 主機（CUDA） |
| `syllable` | 音節對字編碼器訓練到開發集不再進步；60,000 步後每次接續 10,000 步 | GPU 主機（CUDA） |
| `quality` | 開發集首選正確率：有無前文、有無 5% 打字雜訊 | GPU 主機（CUDA） |
| `export` | 兩個 Core ML 套件（fp16、macOS 14）與 `config.json` | 任一（需要 coremltools） |
| `compile` | 以 `xcrun coremlcompiler` 編譯 | 只限 macOS |
| `validate` | Core ML 與 PyTorch 一致性、20 組測試向量、CPU 延遲 | 只限 macOS |

訓練步驟需要 `train` 相依群組，匯出與驗證需要 `export` 相依群組；`zt train` 會自動帶入。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `lexicon.tsv`、`corpus/dev_wikipedia.txt`、`corpus/dev_colloquial.txt`（由 `snapshot` 凍結到 `v3_030/`）；
- `corpus/synthetic_*.txt`、`corpus/train_commonvoice.txt`（口語來源）；
- 保留集 `lm/test_sentences.txt`、`corpus/test_colloquial.txt` 與兩個開發集（只用來排除，不訓練）；
- 維基百科傾印檔 `../raw/lm/zhwiki-latest-pages-articles.xml.bz2`（`zt train lm download` 下載；清理方式沿用 `nlp/training/lm`）。

寫入 `v3_030/`：

- 語料：`wiki.txt`、`articles.tsv.gz`（每篇保留文章的頁面、修訂版本與標題，作為出處）、`colloquial.txt`、`wiki_shards/`、`colloquial_shards/`、`encoded/`；
- 詞彙與紀錄：`chars.json`、`syllables.json`、`provenance.json`、`wiki.json`、`colloquial.json`、`polyphone.json`、`encode.json`、`data-audit.json`；
- 模型：`polyphone.pt`、`context.pt`、`syllable.pt`、`*-history.json`、`*-done.json`、`quality.json`；
- 交付：`ContextEncoder`／`SyllableEncoder` 的 `.mlpackage` 與 `.mlmodelc`、`config.json`、`example.json`、`test_vectors.json`、`parity.json`、`latency.json`。

介面重點（完整的約定寫在 `config.json` 的 `contract`）：

- 前文只取最後 255 個字，前面加 BOS；段落換行對應到 SEP（id 3），段落不可直接相連。
- 前文編碼器輸出 `memory_k`、`memory_v`，形狀 `[6, Lc, 512]`、float16，已經用音節編碼器每一層的交叉注意力 K/V 投影過，不要再投影。兩個套件必須來自同一個檢查點。
- 每個位置最多 64 個候選字，由呼叫端挑選；未知字共用 UNK（id 2），填充為 0。
- `test_vectors.json` 是 CPU Core ML 的實際輸出；float16 記憶以 base64 little-endian 位元組加 SHA256 儲存，請直接解碼成張量。`zhuyin-cli --check-v3` 以它比對 Swift 的結果。

授權：模型與衍生語料為 CC BY-SA 4.0，程式碼為 AGPL-3.0-only。再散布時保留來源聲明與 `articles.tsv.gz`。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train v3` 的步驟 |
| `common.py` | 輸出目錄 `v3_030/`、保留集、JSON 與雜湊工具 |
| `prepare.py` | 維基百科段落、口語去重、詞彙表 |
| `readings.py` | 詞庫最長匹配與逐字讀音（不需要 PyTorch） |
| `polyphone.py` | 多音單字讀音分類器的訓練與載入 |
| `encode.py` | 編碼成 mmap 分片 |
| `verify_data.py` | 洩漏稽核與分片檢查 |
| `selfcheck.py` | 以實際資料做的訓練前檢查 |
| `noise.py` | 打字變體：鄰鍵、聲調、對調、多打、漏打、變調、台灣讀音 |
| `dataset.py` | 訓練批次與候選字集合 |
| `models.py` | 前文編碼器、音節對字編碼器與匯出介面 |
| `train.py` | 兩個編碼器的訓練與接續訓練 |
| `evaluate.py` | 開發集首選正確率 |
| `export.py` | Core ML 匯出與 `config.json` |
| `coreml.py` | 以 `coremlcompiler` 編譯（macOS） |
| `validate.py` | Core ML 一致性、測試向量、延遲（macOS） |
| `reference.py` | 以 Core ML 或 PyTorch 執行單一案例 |
| `test_v3.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：0.3.0 訓練的過程與數字
- [../homophone/README.md](../homophone/README.md)：0.4.0 起 app 使用的繼續訓練版本
- [docs/MODEL.md](../../../docs/MODEL.md) §6、[docs/MODEL_V3.md](../../../docs/MODEL_V3.md)：方法與接進引擎的方式
- [../README.md](../README.md)：所有訓練流程

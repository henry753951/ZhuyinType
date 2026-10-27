# nlp/data

不放進 git 的資料與模型：固定版本的下載、詞庫建置，以及引擎直接對應的二進位檔。

## 用途

詞庫、語言模型、神經模型、評測集與訓練檢查點都在 GitHub Release 上，以元件為單位打包，版本與 SHA-256 固定在 `assets.json`。建置 App、CI 與 GPU 主機都用 `zt data fetch` 取得同一份檔案，所以每個地方算出的結果都對應到同一組資料。

基本詞庫（`lexicon.tsv`）由公開資料建置，來源與授權見 [SOURCES.md](SOURCES.md)。App 出貨與評測時，詞庫與語言模型會編成 mmap 的二進位檔：引擎直接對應檔案，約 0.1 秒啟動，平行的多個行程共用同一份記憶體。

## 指令

```sh
zt data fetch                     # App 的元件：lexicon、lm、lm_chat、reranker、v3
zt data fetch eval                # 評測需要的全部元件（另含 eval-sets、checkpoints）
zt data fetch engine              # 只有詞庫與語言模型
zt data status                    # 每個元件是否存在、是否為固定的版本
zt data lexicon                   # 沿用 lexicon.tsv，沒有時下載來源並建置；--rebuild 重新建置
zt data compile                   # lexicon.bin、lm/lm.bin、lm_chat/lm.bin 放在 TSV 旁邊
zt data compile dist/Resources    # 輸出到指定目錄（App 建置時用）
```

發布新版本的資料：

```sh
zt data pack data-4 v3            # 只打包內容有變的元件，更新 assets.json
zt data publish data-4            # 上傳成 GitHub release（需要 gh）
```

之後提交 `assets.json`。打包是確定性的（檔案排序、時間設為 0），內容沒變的元件保持原本的雜湊值與版本。

元件與群組：

| 名稱 | 內容 |
|---|---|
| `lexicon` | `lexicon.tsv` |
| `lm`、`lm_chat` | 一般與聊天語言模型的 unigram／bigram／backoff TSV 與 NOTICE |
| `reranker`、`v3` | Core ML 模型、詞彙與設定 JSON、來源紀錄 |
| `eval-sets` | 開發集與測試集（口語、維基百科、Taiwan Tongues 對話） |
| `checkpoints` | reranker 與 v3 的 PyTorch 檢查點、v3 的測試向量（Linux 由此匯出 ONNX） |
| 群組 `app`、`engine`、`eval`、`all` | 見上方 `zt data fetch` 的說明 |

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build`，可用 `ZT_DATA` 改變）。

- `fetch`：讀 `assets.json`，壓縮檔下載到 `$ZHUYIN_ASSET_CACHE`（預設 `.cache/assets`），驗證雜湊後解壓縮到資料目錄，並在 `.assets/<元件>` 記錄來源的雜湊值。
- `pack`：讀資料目錄裡各元件的檔案，寫出 `dist/assets/zhuyintype-<元件>-<雜湊>.tar.gz`，更新 `assets.json`。
- `lexicon`：來源下載到 `data/raw/`（CC-CEDICT、Unihan），詞頻用 wordfreq（`data` 相依群組），寫出 `lexicon.tsv`。
- `compile`：讀 `lexicon.tsv`、`lm/`、`lm_chat/` 的 TSV，寫出 `lexicon.bin`、`lm/lm.bin`、`lm_chat/lm.bin` 與各自的 `.json` 說明（格式、筆數、來源 TSV 的雜湊值）。沒有 `bigram.tsv` 的語言模型會略過。

## 檔案

| 檔案 | 說明 |
|---|---|
| `assets.json` | 每個元件固定的 release、檔名、SHA-256 與大小 |
| `assets.py` | 元件定義、下載、狀態、打包與發布 |
| `lexicon.py` | 從 CC-CEDICT、Unihan 與 wordfreq 建置基本詞庫；下載來源 |
| `compiled.py` | 編譯詞庫與語言模型；編譯器是 `nlp/training/lm/compile_data.py`，和格式定義放在一起 |
| `SOURCES.md` | 資料來源與授權 |

## 相關文件

- [SOURCES.md](SOURCES.md)：資料來源與授權
- [nlp/training/lm/FORMAT.md](../training/lm/FORMAT.md)：二進位格式
- [tools/README.md](../../tools/README.md)：所有 zt 指令

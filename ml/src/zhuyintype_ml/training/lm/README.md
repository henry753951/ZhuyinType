# 維基百科詞 bigram

從中文維基百科訓練台灣正體中文的詞 bigram（modified Kneser–Ney），也提供其他語言模型共用的斷詞與平滑程式，以及 mmap 二進位格式的編譯器。

## 用途

- 產生 app 的百科語言模型 `lm/`：`unigram.tsv`、`bigram.tsv`、`backoff.tsv`。建置 app 時由 `compile_data.py` 編成 mmap 的 `lm.bin`（格式見 [FORMAT.md](../../../../../resources/formats/lm.md)），與詞庫的 `lexicon.bin`（[LEXICON_FORMAT.md](../../../../../resources/formats/lexicon.md)）一起放進 Resources。
- 產生維基百科測試集 `lm/test_sentences.txt`（評測集 `wiki-test`），以及其他流程使用的訓練分片 `lm/work/sentences/`（`corpus` 的維基開發集、`base` 的語料）。
- `mkn.py` 是 `lm_chat`、`lm_chat2`、`base` 共用的斷詞、hard EM 與 MKN 估計；`files.py` 是共用的檔案工具。

## 指令

```sh
zt --remote train lm                           # GPU 主機：test 到 prefer-tai
zt --remote train lm --option workers=48       # 擷取、斷詞與驗證的程序數（預設 64）
zt train lm prefer-tai --force                 # 重跑單一步驟
python -m zhuyintype_ml.training.lm.compile_data data/build/compiled          # 編譯詞庫與兩個語言模型（Mac 或主機皆可）
python -m zhuyintype_ml.training.lm.lmbin verify data/build/compiled/lm/lm.bin  # 檢查任一 lm.bin
python -m unittest discover -s ml/src/zhuyintype_ml/training/lm -t .
```

步驟依實際執行的順序排列：

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：Viterbi、MKN、holdout、讀音比例、二進位格式 | 任一 |
| `download` | 下載最新的 `pages-articles` 傾印檔，核對 Wikimedia 公布的 SHA-1，寫下收據 | 任一（約 3 GB） |
| `train` | 擷取、清理、去重、保留 5,000 句測試集、三輪斷詞、MKN、剪枝到 300 萬個 bigram | GPU 主機（CPU，64 程序，約 50 GB 磁碟，64 GB 以上 RAM） |
| `verify` | 重新讀取匯出的檔案，檢查正規化、詞庫列與讀音比例，並重新掃描全部訓練句 | GPU 主機 |
| `prefer-tai` | 把「臺」寫法的機率移給「台」寫法（只跑一次） | 任一 |

`train` 與 `verify` 需要 `train` 相依群組；`zt train` 會自動帶入。中斷後重跑會從 `lm/work/` 的檢查點繼續；輸入雜湊或設定改變時會拒絕，請換一個工作目錄。要重現某個固定版本，請下載帶日期的傾印檔網址，以交付的 `stats.json` 中的雜湊核對，並在傾印檔旁放對應的 `download.json`。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `lexicon.tsv`：種子詞庫；
- `../raw/lm/zhwiki-latest-pages-articles.xml.bz2` 與同目錄的 `download.json`（`download` 產生）。

寫入 `lm/`：

- 模型：`unigram.tsv`、`bigram.tsv`、`backoff.tsv`（自然對數），重新校準的 `lexicon_tw.tsv`；
- 測試集：`test_sentences.txt`；
- 紀錄：`stats.json`（計數、困惑度、來源收據、雜湊、執行環境）、`validation.json`、`examples.json`、`conversion_spotchecks.json`；
- 出處：`NOTICE.md`、`articles.tsv.gz`（每篇保留文章的頁面、修訂版本與標題）；
- 工作目錄 `work/`：擷取分片、去重後的句子、訓練分片 `sentences/`、三輪斷詞的計數、最後的斷詞器（`vocabulary.json`、`segmentation_scores.npy`）。

授權：程式碼為 AGPL-3.0-only；模型、重新校準的詞庫與測試句為 CC BY-SA 4.0。再散布時保留 `NOTICE.md` 與 `articles.tsv.gz`。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train lm` 的步驟；也提供其他流程共用的步驟工具（`run`、`tests`） |
| `download.py` | 下載傾印檔並寫下來源收據 |
| `train.py` | 擷取、清理、holdout、斷詞、MKN、匯出與評估 |
| `mkn.py` | 共用的 Viterbi 斷詞、hard EM、MKN 估計、TSV 讀寫與檢查 |
| `verify.py` | 匯出後的獨立驗證 |
| `prefer_tai.py` | 「台」寫法優先 |
| `files.py` | 原子寫入 JSON、SHA-256、記錄訊息、原始下載目錄 |
| `compile_data.py` | TSV 編成 mmap 二進位（ZTLX/1、ZTLM/1），只用標準函式庫 |
| `lmbin.py` | ARPA 轉 ZTLM/1（可含三元）、mmap 讀取與檢查 |
| `test_*.py` | 單元測試 |
| `FORMAT.md`、`LEXICON_FORMAT.md` | 二進位格式規格 |

## 相關文件

- [REPORT.md](REPORT.md)：來源、清理、斷詞與平滑的做法，沒有採用的做法
- [docs/engine/MODEL.md](../../../../../docs/engine/MODEL.md) §5：語言模型
- [ml/src/zhuyintype_ml/training/README.md](../README.md)：所有訓練流程

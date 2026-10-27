# chat2 聊天詞 bigram

以真實的台灣對話文字訓練的聊天詞 bigram：立法院公報逐字稿、中文維基百科討論頁與 Common Voice。0.4.0 起出貨為 `lm_chat/`。

## 用途

- 產生 app 的聊天語言模型。出貨的是只含真實文字的版本（`lm_chat2/no_generated/`，`ship` 步驟複製到 `lm_chat/`），連同 `NOTICE.md`、`attribution.jsonl`、`source-manifest.json`。
- 產生對話開發集與測試集 `lm_chat2/dev_dialogue.txt`、`test_dialogue.txt`：各 1,000 個「前一則<TAB>下一則」發言對（900 個公報、100 個討論頁），整份文件只屬於一個分割。
- 在開發集上比較四個模型的困惑度，以及不同聊天比例下的轉換正確率。結果與限制見 [REPORT.md](REPORT.md)。

## 指令

```sh
zt train lm_chat2 sources                     # 可在 Mac 執行：下載來源並寫收據（已快取的會沿用）
zt --remote train lm_chat2                    # GPU 主機：test 到 ship
zt --remote train lm_chat2 perplexity --force
python -m nlp.training.lm_chat2.prepare prepare --variant real --workers 24
python -m unittest discover -s nlp/training/lm_chat2 -t .
```

步驟依實際執行的順序排列：

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：發言解析、對話上下文、失敗診斷、洩漏比對、估計 | 任一 |
| `sources` | 公報 800 份（上游在 506 份時持續回傳 HTTP 500）、討論頁 20 批、data.gov.tw 樣本 | 任一（只用標準函式庫） |
| `extract` | 發言與出處 `real_dialogues.jsonl`，每個來源 20 對審查樣本 `source_samples.json` | 任一 |
| `split` | 對話開發集與測試集；已凍結就保留 | 任一 |
| `freeze` | 把種子詞庫與 0.3.0 聊天模型的斷詞器複製到 `inputs/`（已存在就保留） | 任一 |
| `manifest` | `source-manifest.json`、`attribution.jsonl`、`NOTICE.md` | 任一 |
| `real` | 只含真實文字與 Common Voice 的模型（`no_generated/`，出貨版本） | GPU 主機（24 程序） |
| `mixed` | 再加上 0.3.0 的口語語料（`lm_chat2/`） | GPU 主機 |
| `perplexity` | 四個模型在同一份斷詞上的開發集困惑度 | GPU 主機 |
| `conversion` | 0.3.0 聊天模型與 chat2 在聊天比例 0.3／0.5／0.7 下的開發集轉換；完成 24 列中的 7 列 | GPU 主機 |
| `ship` | 把 `no_generated/` 的三個 TSV 與出處檔案複製到 `lm_chat/`，0.3.0 的模型先保存到 `lm_chat_v1/` | 任一 |

`extract` 之後、`split` 之前，先審查 `source_samples.json`。`split` 不會重新抽樣，`real`、`mixed` 不會重新準備語料；要改資料請另開一個實驗目錄。訓練時工作目錄中的輸入、程式碼或設定改變會拒絕繼續。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `../raw/lm_chat2/`：原始 HTTP 回應與每個檔案的收據（網址、授權、SHA-256、日期）；
- `corpus/train_commonvoice.txt`、`v3_030/colloquial.txt`（`mixed` 才用；訓練時位於 `v3/`）；
- `lm_chat/inputs/lexicon.tsv` 與 `lm_chat/work/` 的斷詞器（由 `freeze` 凍結到 `lm_chat2/inputs/`）；
- `lm_chat2/inputs/evaluation/`：凍結的既有開發與測試文字；不存在時改讀目前所有評測檔，只用來排除。

寫入 `lm_chat2/`：

- 語料與評測集：`real_dialogues.jsonl`、`source_samples.json`、`dev_dialogue.txt`、`test_dialogue.txt`、`splits.json`；
- 模型：`no_generated/` 與 `lm_chat2/` 各自的 `unigram.tsv`、`bigram.tsv`、`backoff.tsv`、`validation.json`、`stats.json` 與 `work/`；
- 出處：`source-manifest.json`、`attribution.jsonl`（每份來源文件的連結、日期、版本）、`NOTICE.md`、`inputs/Qwen3-8B-LICENSE`；
- 評估：`perplexity.json`、`perplexity-lines.tsv`、`conversion/`、`conversion-summary.json`。

授權：模型與合併後的語料為 CC BY-SA 4.0；公報經 OpenFun LYAPI 為 CC BY 4.0（原始資料為政府資料開放授權第 1 版），討論頁為 CC BY-SA 4.0，Common Voice 為 CC0。兩個版本都不含 Taiwan Tongues。再散布時保留 `NOTICE.md`、`source-manifest.json` 與 `attribution.jsonl`。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train lm_chat2` 的步驟 |
| `sources.py` | 下載與快取來源，寫收據 |
| `prepare.py` | 發言擷取、對話分割、訓練語料與洩漏過濾 |
| `manifest.py` | 來源清單、出處清單與聲明 |
| `train.py` | 斷詞、MKN 與發言邊界 bigram，匯出與驗證 |
| `evaluate.py` | 困惑度與轉換比較（只在 Linux 主機執行） |
| `test_chat2.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：來源與品質、模型、困惑度、轉換評估與當時的執行方式
- [docs/MODEL.md](../../../docs/MODEL.md) §5：語言模型
- [../lm/README.md](../lm/README.md)：共用的斷詞與平滑

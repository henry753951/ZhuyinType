# 0.3.0 聊天詞 bigram

0.3.0 的聊天語言模型，以及從 Taiwan Tongues 選出的聊天開發集與測試集。模型已停用，評測集仍在使用。

## 用途

- 產生 Taiwan Tongues 的開發集與測試集 `lm_chat/dev_chat_tt.txt`、`lm_chat/test_chat_tt.txt`（評測集 `chat-dev`、`chat-test`），各 1,500 句，彼此及與舊評測集都沒有 8 字重疊。
- 產生 0.3.0 出貨的聊天詞 bigram：以 0.3.0 v3 的口語語料（Qwen3-8B 產生的句子與 Common Voice）和其餘的 Taiwan Tongues 句子訓練，斷詞與平滑方式和維基百科模型相同。0.4.0 起改出貨 chat2（[../lm_chat2](../lm_chat2)），這個模型保留為 `lm_chat_v1/`。
- 在開發集上比較聊天模型與維基百科模型的困惑度與逐句對數概似差，並比較句首選字。結果見 [REPORT.md](REPORT.md)。

## 指令

```sh
zt --remote train lm_chat                                 # GPU 主機：test、train
zt --remote train lm_chat train --option output=lm_chat_rerun --option workers=48
python -m zhuyintype_ml.training.lm_chat.train --output data/build/lm_chat_rerun --inputs data/build/lm_chat/inputs
python -m unittest discover -s ml/src/zhuyintype_ml/training/lm_chat -t .
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：Taiwan Tongues TSV 過濾、holdout 選取、序列化模型 | 任一 |
| `train` | holdout、三輪斷詞、MKN、困惑度與句首比較、洩漏稽核 | GPU 主機（CPU，48 程序） |

`train` 寫入 `data/build/<output>/`，預設是當初訓練時的 `lm_chat/`，會蓋過 0.4.0 出貨的 chat2 模型；要重新訓練請指定另一個 `output`。工作目錄中的輸入、程式碼或設定改變時，訓練會拒絕繼續，請換一個輸出目錄。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `lm_chat/inputs/`：凍結的輸入。`lexicon.tsv`、Taiwan Tongues 的 `train.tsv`／`test.tsv`、`train_commonvoice.txt`、四個舊評測集（`test_sentences.txt`、`test_colloquial.txt`、`dev_colloquial.txt`、`dev_wikipedia.txt`）、`wiki/` 底下的維基百科模型 TSV、`NOTICE.md`、兩份 Taiwan Tongues 發布者聲明、Qwen3-8B 的 LICENSE、`source_receipts.json`、`transfer_receipt.json`；
- `v3_030/colloquial.txt`：0.3.0 v3 的口語語料（訓練時位於 `v3/`）。

寫入輸出目錄：

- 評測集：`dev_chat_tt.txt`、`test_chat_tt.txt`、`splits.json`（seed、過濾條件、雜湊）；
- 模型：`unigram.tsv`、`bigram.tsv`、`backoff.tsv`（自然對數，合計不超過 60 MB）；
- 紀錄：`stats.json`、`validation.json`、`line_scores.tsv`（開發集逐句分數）、`sentence_starts.json`；
- 出處：從 `inputs/` 複製的聲明、授權與收據；
- 工作目錄 `work/`：訓練分片、三輪斷詞計數、斷詞器（`chat2` 的困惑度沿用它）。

Taiwan Tongues 採 TRAIL v0.1 授權，評測集與模型都要保留它的授權與出處；不要把產物標成 CC0 或只標 CC BY-SA。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train lm_chat` 的步驟 |
| `train.py` | holdout、訓練、評估與稽核；`normalized`、`grams`、`read_tt` 也供 `lm_chat2`、`base` 使用 |
| `test_train.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：語料、holdout、困惑度、句首比較與限制
- [docs/engine/MODEL.md](../../../../../docs/engine/MODEL.md) §5：語言模型
- [../lm/README.md](../lm/README.md)：共用的斷詞與平滑

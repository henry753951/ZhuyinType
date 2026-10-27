# 統計基底 A1

研究用：以小麥注音資料擴充詞庫，並以斷詞 lattice 上的期望次數（forward-backward）訓練詞 bigram。0.4.0 沒有採用。

## 用途

- 產生擴充詞庫 `base/lexicon_source.tsv`：保留種子詞庫的每一列，加入小麥注音中每個字都有有效音節的漢字條目，分數依同長度共有詞校準。
- 產生候選語言模型 `base/models/<名稱>/`：維基百科與聊天兩個領域，硬計數或期望次數，沿用現行斷詞器或以語料重新估計斷詞（`--fresh`），以及剪枝、頻率先驗與未用詞的消融實驗。
- 依開發集選出的組合是 `lexicon_source.tsv`、`wiki_expected_wide` 與 `chat_fresh_expected`，三者要一起使用；只用引擎時口語與維基進步，但整套 app 沒有可測的提升，Taiwan Tongues 與日常短句退步。數字與決定見 [REPORT.md](REPORT.md)。
- 聊天候選模型含 Taiwan Tongues 的訓練文字（TRAIL 授權），只能作為研究資產。

## 指令

```sh
zt --remote train base                     # GPU 主機：test 到 fallback
zt --remote train base fresh               # 單一步驟
python -m zhuyintype_ml.training.base.train fit --name chat_fresh_expected --domain chat --method expected --fresh \
    --lexicon data/build/base/lexicon_source.tsv --workers 8
python -m unittest discover -s ml/src/zhuyintype_ml/training/base -t .
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：lattice 後驗與窮舉一致、詞庫合併、小麥注音解析、頻率先驗、消融的重新正規化 | 任一 |
| `lexicon` | `lexicon_source.tsv` 與合併紀錄 | GPU 主機 |
| `prepare` | 維基百科、真實聊天、產生語料三種分片，與所有現行評測檔比對排除（只做一次） | GPU 主機（8 程序） |
| `fit` | `chat_hard`、`chat_expected`、`chat_generated01`、`wiki_hard`、`wiki_expected`，用封存的第一版詞庫 `lexicon_mcbopomofo.tsv` | GPU 主機 |
| `wide` | `wiki_expected_wide`：同一份期望次數，最多保留 600 萬個 bigram | GPU 主機 |
| `prior` | `native_prior`：小麥注音詞頻作為不看前文的聊天先驗（對照組） | GPU 主機 |
| `fresh` | `chat_fresh_hard`、`chat_fresh_expected`、`chat_fresh_generated01`：三輪語料 E/M 斷詞，用 `lexicon_source.tsv` | GPU 主機 |
| `fallback` | `chat_unused_fallback`、`wiki_unused_fallback`：把沒用到的詞庫詞交給引擎的詞庫退回 | GPU 主機 |

模組在 Linux 以外的系統拒絕執行；擬合前會等到主機有 32 GiB 可用記憶體。每個模型目錄只寫一次，已存在的模型由步驟略過。`lexicon_mcbopomofo.tsv` 是最早的合併結果，只存在資料中。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `base/inputs/`：凍結的種子詞庫 `lexicon.tsv`、小麥注音 `mcbpmf-data.txt`（MIT／BSD 資料）、現行模型 `baseline/`；
- `lm/work/sentences/` 與 `lm/work/` 的斷詞器（維基百科）；`lm_chat2/no_generated/work/sentences/`（真實聊天）；`lm_chat/inputs/train.tsv`（Taiwan Tongues）；`lm_chat/work/` 的斷詞器；`v3_030/colloquial.txt`（產生語料）；
- 評測檔：`lm/test_sentences.txt`、`corpus/`、`lm_chat/`、`lm_chat2/`、`homophone/` 的開發與測試檔、日常短句與 `ml/src/zhuyintype_ml/eval/sets/`，只用來排除；
- 出處：`lm/`、`lm_chat/`、`lm_chat2/` 的聲明與授權檔，複製到各模型目錄。

寫入 `base/`：

- `lexicon_source.tsv`、`lexicon-source-merge.json`；
- `corpus/`：分片、`holdouts.json`、`prepared.json`、`validation.json`；
- `models/<名稱>/`：`unigram.tsv`、`bigram.tsv`、`backoff.tsv`、`stats.json`、`validation.json`、聲明與 `work/`（斷詞器與期望次數）。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train base` 的步驟 |
| `train.py` | 詞庫合併、語料準備、lattice 期望次數與模型擬合 |
| `prune.py` | 較寬剪枝的維基百科模型 |
| `prior.py` | 小麥注音頻率先驗對照組 |
| `fallback.py` | 未用詞交給詞庫退回的消融實驗 |
| `test_base.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：做法、開發集與測試集結果、評估程式的紀錄
- [docs/research/log.md](../../../../../docs/research/log.md)：A1 的決定
- [../lm/README.md](../lm/README.md)：共用的斷詞與平滑

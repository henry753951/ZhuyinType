# 資料來源

| 資料 | 用途 | 授權 |
|---|---|---|
| [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict) | 詞彙、正體字形與讀音（有標註時優先採用台灣讀音） | CC BY-SA 4.0 |
| [Unicode Unihan](https://www.unicode.org/charts/unihan.html) | Big5 字元與讀音 | Unicode License v3 |
| [wordfreq](https://github.com/rspeer/wordfreq) | 詞頻 | 資料 CC BY-SA 4.0 |

模型另外使用以下文字資料（只用文字，不含語音或錄音者資料）：

| 資料 | 用途 | 授權 |
|---|---|---|
| 中文維基百科 | 百科語言模型（`lm/`）、重排模型、v3 字模型 | CC BY-SA 4.0 |
| [Common Voice](https://commonvoice.mozilla.org) zh-TW 句子 | 口語語料 | CC0 1.0 |
| [Taiwan Tongues ASR CE](https://huggingface.co/datasets/adi-gov-tw/Taiwan-Tongues-ASR-CE-dataset-zhtw) zh-TW 轉寫 | 聊天語言模型（`lm_chat/`）與聊天測試集 | TRAIL（TWM Responsible AI License v0.1），條款隨附於 `lm_chat/` |
| [Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) 產生的台灣口語句子 | 口語語料、聊天語言模型 | 模型權重 Apache-2.0 |

產生的詞庫（`data/build/lexicon.tsv`）採 CC BY-SA 4.0 授權，程式碼採 AGPL-3.0-only 授權。轉換流程包含讀音轉注音、篩選詞條與計算 unigram log 機率；來源署名與完整授權見 [Resources/Licenses](../Resources/Licenses)。

在專案根目錄執行以下指令，可沿用現有詞庫，或下載來源並產生詞庫：

```sh
make lexicon
```

首次準備詞庫需要網路與 uv，腳本會在專案的 `.venv` 安裝 wordfreq。轉換工具位於 `data/tools/`。需要重新產生時，將現有的 `data/build/lexicon.tsv` 移至備份位置，再執行 `make lexicon`。

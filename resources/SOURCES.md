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
| [Taiwan Tongues ASR CE](https://huggingface.co/datasets/adi-gov-tw/Taiwan-Tongues-ASR-CE-dataset-zhtw) zh-TW 轉寫 | 只用於評測集（`dev_chat_tt`、`test_chat_tt`）；app 與 0.4.0 起的模型訓練都不含 | TRAIL（TWM Responsible AI License v0.1），條款隨評測集打包 |
| [立法院公報](https://data.gov.tw/license)（經 [LYAPI](https://v2.ly.govapi.tw/)） | 聊天語言模型（`lm_chat/`）、v3 的在／再微調（0.4.0 起） | 政府資料開放授權條款第 1 版；LYAPI CC BY 4.0；來源連結在 `lm_chat/attribution.jsonl`、`v3/attribution.jsonl` |
| 中文維基百科討論頁 | 聊天語言模型、v3 的在／再微調 | CC BY-SA 4.0；頁面與版本在 `lm_chat/attribution.jsonl`、`v3/attribution.jsonl` |
| [小麥注音](https://github.com/openvanilla/McBopomofo) `data.txt` | 只用於研究（`ml/src/zhuyintype_ml/training/base/`，0.4.0 未採用）；app 不含 | MIT；片語源自 libtabe `tsi.src`（BSD） |
| [Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) 產生的台灣口語句子 | 口語語料（整句重排、v3 的訓練）；0.4.0 的聊天語言模型不含 | 模型權重 Apache-2.0 |

產生的詞庫（`data/build/lexicon.tsv`）採 CC BY-SA 4.0 授權，程式碼採 AGPL-3.0-only 授權。轉換流程包含讀音轉注音、篩選詞條與計算 unigram log 機率；來源署名與完整授權見 [Resources/Licenses](../platforms/macos/Resources/Licenses)。

在專案根目錄執行以下指令，可沿用現有詞庫，或下載來源並產生詞庫：

```sh
tools/zt data lexicon             # 或 make lexicon；--rebuild 重新產生
```

首次準備詞庫需要網路與 uv：來源下載到 `data/raw/`，詞頻用 `data` 相依群組的 wordfreq。轉換程式是 `ml/src/zhuyintype_ml/data/lexicon.py`。

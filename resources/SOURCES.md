# 資料來源

| 資料 | 用途 | 授權 |
|---|---|---|
| [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict) | 詞彙、正體字形與讀音（有標註時優先採用台灣讀音） | CC BY-SA 4.0 |
| [Unicode Unihan](https://www.unicode.org/charts/unihan.html) | Big5 字元與讀音 | Unicode License v3 |
| [wordfreq](https://github.com/rspeer/wordfreq) | 詞頻 | 資料 CC BY-SA 4.0 |

詞庫涵蓋流程（`ml/src/zhuyintype_ml/training/lexicon/`，研究中，App 尚未使用）另外使用以下資料，版本與 SHA-256 固定在 `sources.py`：

| 資料 | 用途 | 授權 |
|---|---|---|
| [Unicode Unihan](https://www.unicode.org/Public/18.0.0/ucd/Unihan.zip) 18.0.0 | 讀音通道：kHanyuPinyin、kXHC1983、kTGHZ2013、kMandarin 的全部讀音；kHanyuPinlu 估計比例 | Unicode License v3 |
| 英文 Wiktionary 的中文詞條，經 [Wiktextract](https://kaikki.org/dictionary/Chinese/)（2026-10-25 的擷取） | 讀音通道：字與詞的國語讀音（含台灣、異讀、常見誤讀的標記）、台語台羅讀音、形聲字的聲符、詞類（語氣詞）與「台灣華語」分類；只取這些事實，不含釋義 | CC BY-SA 4.0（與 GFDL）；署名 Wiktionary 貢獻者、Tatu Ylonen（Wiktextract） |
| [BabelStone IDS](https://www.babelstone.co.uk/CJK/IDS.TXT)（2025-06-27） | 讀音通道：字的第一層部件，找出形聲字的聲旁（有邊讀邊） | 維護者 Andrew West 聲明 IDS 是事實、不受著作權保護，並放棄對格式的權利；資料部分源自 Kawabata Taichi 的 IDS 與 IRG 的提交資料 |
| [小麥注音](https://github.com/openvanilla/McBopomofo) 3.1.1 的 `data.txt`（取自善解 `bd3b609` 未修改的複本） | 讀音通道：只取我們詞庫已有的詞的其他讀音（危險 ㄨㄟˊ ㄒㄧㄢˇ），不取它的詞與詞頻 | MIT；片語源自 libtabe `tsi.src`（BSD） |
| [全國營業(稅籍)登記資料集](https://data.gov.tw/dataset/9400)（2026-10-26） | 自動找詞的語料：營業人名稱（店名、品牌）與營業地址（地名、路名）；只留下出現 10 次以上、像詞的 n-gram，不保存個別登記 | 政府資料開放授權條款第 1 版：「本資料由財政部財政資訊中心提供，依政府資料開放授權條款進行公眾釋出」 |
| 中文維基百科（`zt train lm` 的訓練句）、立法院公報與維基討論頁、Common Voice（`lm_chat2` 的語料） | 自動找詞的語料與詞頻，重新計數兩個語言模型；和評測集有 8 字重疊的行先去掉 | 見下表 |
| 中文維基百科 2026-10-02 至 10-26 的新建頁面紀錄（MediaWiki API） | 只用於評測集 `wiki-new-dev`（條目名稱，時間切分） | CC BY-SA 4.0 |

善解（shanjie）的詞表檔依各檔授權可以再利用（`names-stores.tsv`、`names-creators.tsv` 為 CC0，`slang-add.tsv`、`names-add.tsv`、`it-add.tsv`、`acg-add.tsv` 為 CC BY-SA 4.0），但它的店名、創作者、IT、網路用語評測集就是從這些詞表抽出來的，用了就無法量測，而且屬於人工整理的清單。詞庫涵蓋流程只取用善解版本庫中小麥注音的 `data.txt` 原檔（MIT，見上），不使用善解自己的資料或程式碼；靈犀注音（AGPL-3.0）也不使用。教育部的辭典（CC BY-ND 3.0 TW）、新酷音（LGPL）、RIME 地球拼音與八股文（LGPL）、雾凇拼音（GPL）、唯音的詞庫（上游含 ND 與非商業資料）都不使用。

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

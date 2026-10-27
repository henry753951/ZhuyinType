# 詞庫涵蓋：讀音通道與自動找詞

在基本詞庫（`zt data lexicon`）之上加兩種一般的機制：讓每個字與詞可以從實際會打的讀音打到，並帶有從資料估計的成本（讀音通道）；以及從有授權的語料自動找出詞庫沒有的詞，讓它們進入語言模型的計數（詞包）。

## 用途

- 讀音通道：P(打的讀音 | 想打的字)，和 KeyChannel 估計 P(按的鍵 | 想打的音節) 是同一種想法。字的讀音來源與機率見 `channel.py`；詞的變體類別見 `merge.py`：
  - `char`：字的其他讀音，包括台灣讀音（熟 ㄕㄡˊ）、詞典詞裡的讀音（蚵 ㄎㄜ）、Wiktionary 與 Unihan 的讀音、不熟悉的字的聲旁讀音（有邊讀邊，BabelStone IDS），以及語氣詞的各種聲調（呃 ㄜ）；
  - `taiwan`：字的台灣讀音帶進含這個字的每個詞（期中 ㄑㄧˊ、危險 ㄨㄟˊ），強度由 Wiktionary 逐詞比較估計；
  - `mcbopomofo`：小麥注音給我們的詞的其他讀音（熟悉 ㄕㄡˊ ㄒㄧ）；
  - `neutral`：輕聲字打成本字的其他讀音（什麼 ㄕㄣˊ ㄇㄛˊ）；
  - `sandhi`：三聲前的二聲、三聲互換（你好 ㄋㄧˊ ㄏㄠˇ）；
  - `variants`：Wiktionary 標為異讀、口語或常見誤讀的詞讀音；
  - `loan`：台灣華語從台語借的詞，一個字打台語音（仔 ㄚ），聲調不限；
  - `misread`：不熟悉的字在詞裡打成它的其他讀音（蚵仔煎 ㄎㄜ ㄗˇ ㄐㄧㄢ）。
- 每一列都比該詞最常見的讀音低，所以照詞典讀音打時分數和原本相同。每一類可以用 `--offset` 加上成本，在開發集上調整：照標準讀音打的結果不能變差。`misread-dev` 和字音表同源，只用來確認解碼。
- 分工：台灣讀音與詞專屬的讀音是詞庫的列；系統性、能從單一字推出的變體（輕聲打本調、三聲變調、聲旁誤讀、台語音、語氣詞的聲調）交給引擎的讀音通道，引擎讀 `channel.tsv` 與 `taiwan.tsv`，格式見 [reading-channel.md](../../../../../resources/formats/reading-channel.md)。`merge` 步驟的預設是資料端的合併候選（`--data-only`，各類加 2 nats），強的台灣讀音直接成為詞的讀音（期中 ㄑㄧˊ），原讀音降為變體；單字的列只收台灣讀音與詞典詞裡的讀音（`--option char_classes=taiwan,word`），帶進詞的台灣讀音強度至少 0.5（`--option min_taiwan=0.5`），其餘留給引擎，因為單字的列在每個音節都會比對、拖慢每一鍵；`--option standard=0` 保留 CC-CEDICT 的讀音，`--option expanded=1` 產生每一類都展開的對照版。`misread` 步驟也照台灣標準讀音打，另有 `mainland` 子集照被取代的讀音打。
- 自動找詞：在維基百科、聊天語料（立法院公報、維基討論頁、Common Voice）、營業稅籍登記的名稱與地址上，挑出凝固度、自由度至少是同長度詞典詞中位數、邊界字不常單獨成詞的 2–6 字 n-gram；用目前的引擎單獨打一次，打錯的才收。讀音用 v3 的讀音標注，詞頻由次數換算，依語料分成詞包。
- 新詞要有語言模型的計數才選得上：`lm` 步驟把基本詞庫加詞包當成斷詞詞彙，重新斷詞並重算維基百科與聊天兩個 bigram（文字、保留集、平滑與剪枝都和出貨的模型相同）。
- 評測集：`misread-dev`（開發集文字照實際讀音打，依機制分子集，`misread.json` 記錄各子集的讀音狀態）與 `wiki-new-dev`（傾印檔之後新建的維基條目名稱）。
- `channel/v3_noise.tsv`：同一份變體表，給 v3 訓練當讀音雜訊，每列帶讀音狀態與 `cost_bucket`（格式見 reading-channel.md）。

## 指令

```sh
zt train lexicon sources base wiktionary channel misread newtitles   # 任一台機器
zt --remote --push data/build/lexicon_coverage --push data/raw/lexicon \
    --push data/build/v3_030/polyphone.pt --push data/build/v3_030/chars.json \
    --push data/build/lm_chat2/real_dialogues.jsonl --push data/build/lm_chat2/splits.json \
    --push data/build/corpus/train_commonvoice.txt \
    --detach train lexicon prepare mine vocabulary lm --option wiki=WIKI_SENTENCES_DIR --option workers=16
zt train lexicon merge --force    # data-4 的詞庫；加 --option packs=wiki,chat,registry,places 會收詞包
zt --remote eval rows --config app --data data/build/lexcov-X --set misread wiki-new-dev
python -m unittest zhuyintype_ml.training.lexicon.test_lexicon
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：台羅對應、Wiktionary 解析、IDS、讀音通道、合併、詞包、找詞 | 任一 |
| `sources` | 下載固定版本的來源到 `data/raw/lexicon/`，核對 SHA-256 | 任一 |
| `base` | 基本詞庫（`data/lexicon.py`）寫成 `base.tsv` | 任一 |
| `wiktionary` | Wiktionary 的讀音、台語讀音、聲符、詞類 | 任一 |
| `channel` | 讀音通道 `channel/` | 任一 |
| `misread` | `misread-dev` 評測集 | 任一 |
| `newtitles` | `wiki-new-dev` 評測集（需要網路） | 任一 |
| `prepare` | 語料轉成碼位陣列，先去掉和評測集有 8 字重疊的行 | GPU 主機（CPU） |
| `mine` | n-gram 次數、分支熵、選詞 | GPU 主機（16 程序，numpy） |
| `vocabulary` | 讀音、詞頻、引擎檢查，依語料分成詞包 | GPU 主機（`train` 相依群組） |
| `lm` | 以詞包重新斷詞，重算維基百科與聊天兩個 bigram | GPU 主機（16 程序） |
| `merge` | `lexicon.tsv`：資料端的合併候選（基本詞庫、台灣標準讀音、單字的台灣讀音與詞典詞讀音、詞層級的台灣讀音與異讀；詞包要另外指定） | 任一 |

## 輸入與輸出

路徑相對於資料目錄。

讀取：`../raw/lexicon/`（Unihan 18.0.0、Wiktextract 的 Wiktionary 中文資料、CNS11643 全字庫、BabelStone IDS、小麥注音 data.txt、營業稅籍登記，雜湊固定在 `sources.py`）、`../raw/cedict.txt`、`lm/`（維基模型與 `lm/work` 的訓練句）、`lm_chat/`、`lm_chat2/`（聊天語料與準備好的句子）、`corpus/`、`v3_030/polyphone.pt` 與 `chars.json`，以及所有評測集（只用來排除）。

寫入 `lexicon_coverage/`：

- `base.tsv`、`wiktionary.tsv`；
- `channel/`：`channel.tsv`（字、讀音、類別、log P、來源組；ZTRC/1）、`taiwan.tsv`（ZTRT/1）、`familiar.txt`、`v3_noise.tsv`、`channel.json`（估計出的比例與次數）；`channel.validate` 檢查格式；
- `misread/`、`wiki-new/`：評測集；
- `mined/`：`<語料>.ngrams.tsv`、`<語料>.mined.tsv`、`<語料>.json`（門檻與數量）；
- `packs/<語料>.tsv` 與 `vocabulary.json`；
- `lm/`、`lm_chat/`、`seed.tsv`：重新計數的語言模型；
- `lexicon.tsv` 與 `lexicon.json`（各類的列數）。

授權：程式碼 AGPL-3.0-only；產生的詞庫、詞包與語言模型 CC BY-SA 4.0，來源與署名見 [resources/SOURCES.md](../../../../../resources/SOURCES.md)。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train lexicon` 的步驟 |
| `sources.py` | 固定版本的下載與雜湊 |
| `phonology.py` | Wiktionary 注音轉成詞庫的寫法；台羅音節轉注音 |
| `wiktionary.py` | 從 Wiktextract 的資料取出讀音、台語讀音、聲符與詞類 |
| `channel.py` | 讀音通道的估計，以及給 v3 的讀音雜訊表 |
| `corpora.py` | 讀語料（文字檔、聊天語料、營業稅籍） |
| `mine.py` | 語料準備、n-gram 計數、分支熵、選詞 |
| `vocabulary.py` | 找到的詞的讀音、詞頻與引擎檢查，詞包 |
| `lm.py` | 以詞包重新斷詞，重算兩個語言模型 |
| `merge.py` | 合併成 `lexicon.tsv` |
| `misread.py`、`newtitles.py` | `misread-dev`、`wiki-new-dev` 評測集 |
| `test_lexicon.py` | 單元測試 |

## 相關文件

- [docs/research/log.md](../../../../../docs/research/log.md)：實驗與數字
- [docs/research/lexicon-survey.md](../../../../../docs/research/lexicon-survey.md)：其他輸入法的詞庫與公開資料
- [docs/engine/MODEL.md](../../../../../docs/engine/MODEL.md) §2：詞庫與讀音
- [resources/formats/lexicon.md](../../../../../resources/formats/lexicon.md)：詞庫的二進位格式
- [resources/formats/reading-channel.md](../../../../../resources/formats/reading-channel.md)：字音表的格式與引擎該用的成本

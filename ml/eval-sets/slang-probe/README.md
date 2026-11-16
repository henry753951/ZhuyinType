# 現代用語探針

量 ZhuyinType 打現代日常與網路用語的情形：一個問題類別一個檔，共 347 列，每列是含一個目標詞的短句，目標詞照一般人實際的打法標音。分析與修正建議見 [slang-inventory.md](../../../docs/research/slang-inventory.md)。

| 檔案（集合名稱） | 列數 | 內容 |
|---|---:|---|
| `slang.rows.txt`（`slang-slang`） | 46 | 網路用語與迷因：母湯、傻眼貓咪、醬、惹、很頂、破防、社死、耍廢 |
| `particle.rows.txt`（`slang-particle`） | 42 | 語氣詞與句尾助詞，同一句分別照教育部讀音與常見打法打：欸 ㄝˋ／ㄟˋ、囉 ㄌㄨㄛ˙／ㄌㄡ、啦 ㄌㄚ˙／ㄌㄚ |
| `loan.rows.txt`（`slang-loan`） | 36 | 用華語漢字寫的台語、客語借詞：蚵仔煎、古錐、揪團、凍未條、粄條 |
| `japanese.rows.txt`（`slang-japanese`） | 19 | 日語來源的詞：卡哇伊、歐巴桑、運將、中二 |
| `entity.rows.txt`（`slang-entity`） | 51 | 品牌、App、人名、地名、遊戲與動畫：酷澎、全聯、黃仁勳、南崁、原神 |
| `newword.rows.txt`（`slang-newword`） | 30 | 近年的新詞：快篩、載具、限動、提示詞、版控 |
| `reading.rows.txt`（`slang-reading`） | 35 | 實際打的讀音和詞庫不同：什麼 ㄕㄜˊ ㄇㄜ˙、這個 ㄓㄟˋ、垃圾 ㄌㄜˋ ㄙㄜˋ、輕聲打本調 |
| `variant.rows.txt`（`slang-variant`） | 19 | 寫法：台／臺、什麼／甚麼、計畫／計劃、身分／身份 |
| `zhuyinwen.rows.txt`（`slang-zhuyinwen`） | 8 | 注音文：好ㄉ、ㄏㄏ、ㄘ飯了沒 |
| `tech.rows.txt`（`slang-tech`） | 61 | 技術、軟體與學術詞：資料集、組態、首幀、版控、捷徑鍵、文獻回顧（2026-11-17 補） |

格式和善解的評測集相同：`前文|答案|注音`，注音以空白分隔，一聲不標。`labels.tsv` 每列對應同一類別檔案的同一列，欄位是：

| 欄位 | 意思 |
|---|---|
| `class` | 類別 |
| `freq` | 依日常聊天估計的常見程度：H 每天會用、M 特定族群或場合常用、L 少見 |
| `truth`、`target`、`typed` | 答案、目標詞、目標詞打的讀音 |
| `in_lexicon`、`typed_in_lexicon` | 目標詞是不是詞庫的詞（data-7），以及打的讀音是不是詞庫裡它的讀音 |
| `unigram_wiki`、`unigram_chat` | 目標詞在百科與聊天語言模型的 unigram 自然對數機率；`-` 是不在詞彙裡 |
| `note` | 說明 |

## 怎麼建的

- 句子與標記是 2026-11-17 為這份分析寫的，不取自任何人的輸入，也不取自其他專案的評測集。
- 目標詞的讀音照實際打法手寫，其他字的讀音用 data-7 詞庫最長比對產生（和 `zt eval conversion` 打字集時相同），再逐列人工核對。
- 一句只測一個目標詞；同一句換讀音的列（欸 ㄝˋ／ㄟˋ）是成對設計，用來分開「讀音」與「詞彙」兩種原因。
- 中英文與數字混打（LINE、IG、87、3C）沒有放進來：列的格式只能寫注音。這一類在文件裡另外說明。

## 用法

```sh
tools/zt --remote eval losses --set slang -k 64 --json data/build/remote_eval/slang/losses.json
tools/zt --remote eval rows --set slang --json data/build/remote_eval/slang/rows.json
```

決定任何改變時，探針和開發集一起做成對比較（`tools/zt eval stats`）。探針只有 347 列，單一類別的差異通常不顯著，只用來看方向與改壞；目標詞的選擇本身有偏差（見文件的「限制」）。

授權：CC0 1.0。

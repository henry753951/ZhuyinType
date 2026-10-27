# 字音表：ZTRC/1 與 ZTRT/1

讀音通道的資料：每個字可能被打成哪些讀音、各自的機率與來源。由 `zt train lexicon channel`（`ml/src/zhuyintype_ml/training/lexicon/channel.py`）產生，寫在資料目錄的 `lexicon_coverage/channel/`。Python（詞庫建置、`misread-dev`、v3 的讀音雜訊）與引擎讀同一份檔案；兩邊的讀法以本文件為準，`test_lexicon.py` 的格式檢查與引擎的單元測試都對照這裡。

所有檔案是 UTF-8 文字，以 Tab 分欄，`#` 開頭的列是註解。讀音的寫法和詞庫相同：注音符號，一聲不標，二、三、四聲與輕聲標在音節後（ˊ ˇ ˋ ˙）。

## channel.tsv（ZTRC/1）

第一列是 `# ZTRC/1 char<TAB>reading<TAB>class<TAB>log P(reading | char)<TAB>source groups`。之後每列一個（字，讀音）：

| 欄 | 內容 |
|---|---|
| char | 一個字（一個 Unicode 碼位） |
| reading | 一個音節 |
| class | 這個讀音從哪一類來，見下表 |
| log P | 自然對數，P(打這個讀音 \| 想打這個字)，同一個字的各列加起來是 1 |
| source groups | 證明這個讀音的來源組，以逗號分隔，可以是空的 |

同一個字的列依機率由大到小排列，同機率依讀音的 UTF-8 位元組排序。每個（字，讀音）只出現一次。

類別：

| class | 意思 | 引擎的處理 |
|---|---|---|
| `lexicon` | 基本詞庫原有的單字讀音 | 已是詞庫的列 |
| `taiwan` | Wiktionary 標為台灣讀音的單字讀音 | 接受的讀音 |
| `word` | 字在詞典的詞裡的讀音 | 接受的讀音 |
| `attested` | Wiktionary 給這個字的其他讀音 | 接受的讀音 |
| `cns` | 只有全字庫列出的讀音 | 接受的讀音 |
| `unihan` | 只有 Unihan 列出的讀音（多為古音） | 接受的讀音，成本高 |
| `series` | 聲旁讀音（有邊讀邊），只給不熟悉的字 | 修正（誤讀） |
| `interjection` | 語氣詞的其他聲調 | 接受的讀音 |

來源組：互相參照的來源算一組，同一個說法不重複計票。

| 組 | 來源 |
|---|---|
| `cedict-unihan` | CC-CEDICT、Unihan |
| `wiktionary` | Wiktionary（多參照教育部辭典） |
| `cns` | CNS11643 全字庫 |
| `ids` | BabelStone IDS 推出的聲旁讀音 |
| `rule` | 規則推出（語氣詞的聲調） |

## taiwan.tsv（ZTRT/1）

第一列是 `# ZTRT/1 char<TAB>word reading<TAB>Taiwan reading<TAB>log strength<TAB>standard`。每列是一個字在詞裡的讀音改成台灣讀音：

| 欄 | 內容 |
|---|---|
| char | 一個字 |
| word reading | 基本詞庫的詞裡這個字的讀音（多為 CC-CEDICT 的大陸讀音） |
| Taiwan reading | 台灣讀音 |
| log strength | 自然對數，Wiktionary 有讀音的含這個字的詞中，台灣讀音是後者的比例；只有 CC-CEDICT 單字 Taiwan pr. 的，用兩者都有的組合的中位數 |
| standard | `1`：全字庫列出台灣讀音，且至少兩組獨立來源同意（Wiktionary 的詞強度 ≥ 0.9、CC-CEDICT 單字的 Taiwan pr.、全字庫沒有列原讀音），台灣讀音就是標準讀音；`0`：其他 |

每個（字，詞裡的讀音）只出現一次，依字與讀音的 UTF-8 位元組排序。

## channel.json

估計出的比例與次數，包括：

| 鍵 | 意思 |
|---|---|
| `misread` | m：Wiktionary 多字詞中記錄了異讀、口語或常見誤讀的字位比例 |
| `neutral_full_tone` | Wiktionary 的輕聲音節有本調異讀的比例 |
| `interjection_tone_rate` | 語氣詞的音節在辭典裡有其他聲調的比例 |
| `alpha_attested`、`alpha_unihan` | 沒有 kHanyuPinlu 次數的字，只有 Wiktionary 或只有 Unihan 列出的讀音分到的比例 |
| `interjections` | 判定為語氣詞的字 |

另有 `familiar.txt`（涵蓋 99% 字次的常用字，聲旁讀音不給這些字）與 `v3_noise.tsv`。

## v3_noise.tsv

同一份資料改寫成 v3 訓練用的讀音雜訊。第一列是 `# char<TAB>reading in the text<TAB>reading typed<TAB>class<TAB>probability<TAB>state<TAB>cost bucket`，之後每列一種打法：

| 欄 | 內容 |
|---|---|
| char | 一個字 |
| reading in the text | 文中這個字的讀音；`*` 表示不論文中讀音 |
| reading typed | 打的讀音 |
| class | `channel.tsv` 的類別；`taiwan` 也用於 `taiwan.tsv` 的詞層級改寫；`misread` 是不熟悉的字打成它的其他讀音 |
| probability | 把文中讀音換成打的讀音的機率 |
| state | `accepted`（接受的讀音）或 `correction`（修正），見下方「三種狀態」。`series` 與台語音是修正，其他是接受的讀音；`misread` 列依打的讀音的類別 |
| cost bucket | 給 v3 的 `cost_bucket`：接受的讀音是 0；修正依 −log 機率分桶，和按錯鍵同一套界線（≤ 6 為 1，≤ 11 為 2，其他為 3，`composer.rs`） |

三聲變調與輕聲打本調是規則層級的變體，不逐字列出；它們是接受的讀音（`cost_bucket` 0），比例在 `channel.json`（`misread`、`neutral_full_tone`）。`channel.validate` 檢查狀態與分桶一致。

## 引擎該用的成本

引擎的讀音通道（`build_matches` 的限定字讀音選項）對一個詞裡的一個音節，用 `channel.tsv` 的字層級機率，`taiwan.tsv` 的詞層級改寫優先：

| 類別 | 狀態 | 成本（自然對數，負號省略） |
|---|---|---|
| `taiwan.tsv` 的 standard = 1 | 標準讀音 | 0；原讀音改為 −log max(1 − strength, m) |
| `taiwan.tsv` 的其他列 | 接受的讀音 | −log strength |
| `taiwan`、`word`、`attested`、`cns`、`unihan` | 接受的讀音 | −log m − log P(讀音 \| 字，不是詞裡的讀音)，只給不熟悉的字；熟悉的字只在單字時用 −log P 的比值 |
| `series` | 修正 | 同上，另外和按錯鍵修正一樣要贏過完全打對的整句一個差距 |
| 輕聲打本調 | 接受的讀音 | −log `neutral_full_tone` − log P(讀音 \| 字，非輕聲) |
| 三聲前二、三聲互換 | 接受的讀音 | −log m |
| 台語音（只對 Wiktionary「台灣華語」分類的詞） | 修正 | −log m + log 5（聲調不限） |

詞庫裡的列（詞專屬的讀音、兩個以上音節同時改變、需要詞層級成本的）覆寫這些預設值，同一組（詞，讀音）不重複計分。

這些起始成本在資料端用展開列量過（研究紀錄 2026-10-28「詞庫涵蓋」）：每一類各加 2 nats 時，四個開發集照標準讀音打沒有改壞的句子。改字記錄累積後應以實際的（打的讀音，選的字）重新估計。

## 引擎的三種狀態與模型看到什麼（步驟 2a 的決定，2026-10-28）

這一節是引擎、詞庫建置、`misread-dev` 與 v3 讀音雜訊共用的規格。各類別的狀態與起始成本見上方「引擎該用的成本」。

### 三種狀態

| 狀態 | 候選的標註 | 計入 `visit` 的編輯上限 | 重排的 `corrected` | 要贏過完全打對的整句 | v3 的 `cost_bucket` |
|---|---|---|---|---|---|
| 接受的讀音 | 不標 | 不計（和模糊音一樣） | 否 | 不用 | 0 |
| 修正（`series`、台語音） | 標出原本的讀音 | 計 | 是 | `reading_margin`，引擎參數，起始值 4，在開發集上調 | 依成本分桶，和按錯鍵同一套分桶 |
| 按錯鍵（KeyChannel） | 不變 | 不變 | 不變 | `exact_margin` 8，不變 | 不變 |

### 規則

1. 三聲變調與輕聲先歸為「接受的讀音」。另一種歸法（修正）在開發集上量一次，結果記在研究紀錄；不採用也要記。
2. 一、不的變調扣分規則不改。新的三聲變調選項不碰一、不。
3. 每個詞最多一個音節走讀音通道。兩個以上音節同時改變的，只靠詞庫的列。
4. 每個音節的變體選項依成本最多取 4 個。依（詞庫讀音，該位置的字）在載入時建索引，不查完再過濾。字和音節對不齊的詞（含非漢字、兒化）跳過變體。
5. 詞庫的列覆寫引擎的字層級預設：同一組（詞，讀音）兩邊都有時以列為準，不重複計分。
6. 成本都是引擎參數，調整不必重建詞庫。起始值照上表；調整的條件是照台灣標準讀音打的開發集不變差（成對檢定）。
7. v3 在重訓之前的過渡做法：(a) 只因變體進來的字，該位置的 v3 分數改用標準讀音時同一個字的分數；(b) 這類字的 v3 權重為 0。兩種都做成參數，預設 (a)，在開發集上比較後決定，記在研究紀錄。
8. 各位置的候選字截斷時，變體字排序不扣變體成本，避免 v3 看不到它。
9. v3 重訓的讀音雜訊用同一張表與同一套狀態：接受的讀音 `cost_bucket` 0，修正依成本分桶。重訓的 v3 採用後，拿掉第 7 點的過渡做法。
10. `zhuyin-cli` 加除錯輸出，印出每個候選走了哪一類變體與成本；`zt eval losses --channel` 依類別分組。
11. 驗收照提案 §2.3。Mac 上每鍵 p95 不增加（`tools/zt eval latency`）。

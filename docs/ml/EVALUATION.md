# 評測

所有評測都用 `zhuyin-cli` 逐鍵打字，經過和 app 相同的 Composer、引擎與模型，比對送出的結果和原文。評測的指令是 `zt eval`，每版固定的整套評測是 `zt bench`。數字的意義與目前的成績見 [MODEL.md](../engine/MODEL.md)。

## 1. 原則

- 開發集用來調參數，測試集只在決定採用時跑一次。兩邊都不能出現在訓練資料裡，所有訓練語料都排除和評測集有連續 8 字相同的句子。
- 採用一項改變之前，要在開發集上做成對比較（`zt eval stats`），並記在 [research/log.md](../research/log.md)。
- 主要指標是整句全對率，也就是一段組字（4–20 個中文字）的結果和原文完全相同的比例。另外報字準確率。
- 「台／臺」「妳／你」「裏／裡」這類兩種寫法都對的字，另外報一個「寫法視為相同」的整句全對率（`zt eval conversion` 的 `forms equal`）。
- 打字的讀音由詞庫最長比對推得，每個詞用最常見的讀音，一聲按空白。讀音不是人工標注。data-4 起詞庫以台灣標準讀音為最常見讀音（期中 ㄑㄧˊ），文字集因此改用新讀音打（三個開發集各有 3–7% 的段落讀音改變）；2026-10-28 起的數字和之前的不能直接比較（[research/log.md](../research/log.md)「台灣讀音成為標準讀音」）。
- 每個 `zhuyin-cli` 程序只用一個 CPU 核心，批次輸入的每一行互相獨立，所以一組評測可以分給多個程序平行跑（`--jobs`，預設為 CPU 核心數），結果和單一程序相同。
- 引擎有些分數相同的候選依雜湊順序決定先後，而 Swift 的雜湊種子每個程序都不同，所以同一段按鍵在不同程序裡可能轉出不同的結果（在按錯的輸入上約 1% 的段落）。評測工具執行 `zhuyin-cli` 時一律設定 `SWIFT_DETERMINISTIC_HASHING=1`，讓結果只取決於輸入。這之前的評測數字（包括 v0.4.0 以前的 ZhuyinBench 紀錄）含有這部分的隨機差異。
- 評測與訓練都在 GPU 主機上跑，不佔用開發用的 Mac。引擎在 Linux 上編譯。開發用的 `zt --remote eval` 預設由 GPU 上的 PyTorch 模型伺服器替所有引擎行程計算模型；ZhuyinBench、與其他輸入法的比較等發布的數字以 ONNX Runtime 在每個引擎行程裡計算（`--backend cpu`）。兩者在開發集上逐行相同（`ml/src/zhuyintype_ml/remote/`）。Mac 只負責建置 app、Core ML 匯出與編譯、量測延遲與能耗（`zt eval latency`），以及確認遠端結果和 Mac 一致，也就是輸出不同的行數不超過 1%。
- 引擎正在修改時，整組評測用同一份建好的 `zhuyin-cli`（環境變數 `ZT_CLI` 指定路徑），不要在評測途中重新建置。

## 2. 評測集

評測集以名稱指定，`zt eval sets` 列出全部，並標出哪些已經在資料目錄裡。`--set` 也接受群組名稱或任意檔案路徑。

| 名稱 | 檔案 | 內容 | 用途 |
|---|---|---|---|
| `colloquial-dev` | `corpus/dev_colloquial.txt` | 台灣口語句子 2,000 句 | 開發 |
| `wiki-dev` | `corpus/dev_wikipedia.txt` | 維基百科 2,000 句，同一行前面的文字可作為前文 | 開發 |
| `chat-dev` | `lm_chat/dev_chat_tt.txt` | Taiwan Tongues zh-TW 轉寫 1,500 句（朗讀句子，偏日常） | 開發 |
| `wiki-test` | `lm/test_sentences.txt` | 維基百科測試句 | 測試 |
| `colloquial-test` | `corpus/test_colloquial.txt` | 台灣口語測試句 | 測試 |
| `chat-test` | `lm_chat/test_chat_tt.txt` | Taiwan Tongues 測試 1,500 句 | 測試 |
| `smoke` | `ml/eval-sets/colloquial_smoke.txt` | 日常短句 68 句 | 測試 |
| `latency` | `ml/eval-sets/latency_sentences.txt` | 量測延遲的固定 10 句 | 延遲 |
| `zai-dev`、`zai-test` | `homophone2/{dev,test}_zai.txt` | 「在／再」各約 1,000 句，依位置分類；工具在 `ml/src/zhuyintype_ml/training/homophone/` | 開發／測試 |
| `shanjie-*` | `ml/eval-sets/external/shanjie/` | 善解的評測集（CC0）：dev302、typing76、reported、fw、trap、daily | 外部 |

群組：`dev` 是三個開發集，`test` 是三個測試集加 `smoke`，`shanjie` 是六個善解評測集。檔案路徑除了 `ml/` 開頭的以外，都相對於資料目錄（預設 `data/build`，可用 `ZT_DATA` 或 `--data` 改變）。資料目錄不在 git 裡，評測集用 `zt data fetch eval` 下載；產生方式見 `ml/src/zhuyintype_ml/training/corpus/README.md`、`ml/src/zhuyintype_ml/training/lm_chat/README.md`。`data/build/homophone/{dev,test}_pairs.txt` 另有 25 組常見同音字，由 `ml/src/zhuyintype_ml/training/homophone/` 的工具使用。

文字集用來抽樣：每一行切出 4–20 個中文字的連續段，以固定的 seed（`--seed`，預設 7）打亂後取前 `-n` 段。每個評測工具都用同一套抽樣，所以同一個 seed 下看到的是同一批句子；需要把游標移到每個字上的工具（候選字、改選、打字習慣、打字錯誤）只取每個字都有讀音的段。善解格式的評測集每行是 `前文|答案|注音`，照原樣輸入。

## 3. 設定

每個評測都在一個具名的設定下執行，報告會記錄設定的名稱與選項：

| `--config` | 內容 |
|---|---|
| `app`（預設） | 資料目錄裡的所有模型，設定方式和 app 相同（`zhuyin-cli --models`）：詞庫與語言模型、聊天模型、整句重排（權重與句數讀 `reranker/config.json`，目前是權重 1.0、16 句）、v3（app 的權重 0.5） |
| `engine` | 只用詞庫與語言模型，不用神經模型與聊天模型 |

`--cli-args "…"` 在設定之上加 `zhuyin-cli` 選項，用來改變單一項設定，例如 `--cli-args "--rerank-weight 0.5"`。明確給的選項優先於 `--models` 讀到的設定。

v0.4.0 以前的 ZhuyinBench 紀錄，app 設定是直接列出模型選項，整句重排用的是 CLI 預設的權重 0.5，不是 app 實際用的 1.0。之後的紀錄用 `--config app`，和 app 一致；比較新舊紀錄的 app 欄位時要注意這一點。以相同的引擎選項執行時，新舊工具的每一筆輸出都相同（見 §8）。

## 4. 評測工具

| 指令 | 量什麼 |
|---|---|
| `zt eval conversion` | 整句全對率、字準確率、寫法視為相同的整句全對率；`--context line` 以同一行前面的文字為前文；`--split continue` 從中間的詞界切成兩次送出 |
| `zt eval candidates` | 候選字清單：整句轉錯時，到每個錯字打開清單，正確答案在其他候選裡排第幾（略過目前顯示的那一項；第一個、第一頁、清單內、平均名次） |
| `zt eval repick` | 免選字：模擬使用者錯了就到第一個錯字選正確的候選，量「不用選字就對」「最多選 1／2／3 次」、平均選字次數、選字改壞其他字的比例；`--relearn` 另外量選過的句子再打一次時是否記住 |
| `zt eval errors` | 依打字習慣（標準、一不變調、輕聲打一聲、一聲不按空白、5% 按錯）分組，報「顯示正確」與「前 K 句命中」（引擎的 K 個最佳句子，預設 16） |
| `zt eval typo` | 依比例隨機按錯（預設 0、10%、20%），三種容錯模式各自的整句全對率與字準確率；比例 0 時另報誤修正率 |
| `zt eval typing-errors` | 有按鍵時間的打字錯誤：對調、幾乎同時按下、熱區誤觸、多按、漏按、聲調、一聲省略空白、混合；`--variant` 比較多組選項；方法見 `ml/src/zhuyintype_ml/eval/TYPING_ERRORS.md` |
| `zt eval rows` | 善解格式（`前文|答案|注音`）的評測集：整句全對率、CER、前 16 句命中，各以三種正規化計算（原文、善解的正規化、我們的寫法對照） |
| `zt eval corrections FILE…` | App 的改字記錄（`corrections.jsonl`）重播：每一種改字，引擎現在不靠改字就打對的有幾筆（見下方） |
| `zt eval latency` | 固定 10 句在這台機器上的每鍵延遲、CPU、能耗（macOS）、記憶體；只在 Mac 上執行 |
| `zt eval losses` | 落後原因分析：收集 App 與只用引擎的前 64 句，把每一句錯誤分到出錯的階段（候選生成、引擎排序、重排、模型推翻）與根本原因（詞彙、讀音、候選、排序、異形詞），見 [research/loss-analysis.md](../research/loss-analysis.md)；`--reference FILE` 用另一份詞表判斷缺詞 |
| `zt eval stats A B` | 兩份結果的成對比較：McNemar 精確檢定、成對 bootstrap 信賴區間、改對／改壞、改壞率上界、最小可偵測效果 |
| `zt eval sweep MATRIX` | 多組設定 × 多個評測集，每個組合一份報告，最後列一張表；矩陣範例在 `ml/src/zhuyintype_ml/eval/matrices/` |
| `zt bench run`、`zt bench history` | ZhuyinBench：每版固定的整套評測與歷代表，結果在 [benchmarks/](../benchmarks/) |
| `zhuyin-cli --timing` | 每鍵延遲（p50、p95、最大）、CPU 時間、能耗（macOS）、每鍵指令數、記憶體占用；批次模式把摘要印在標準錯誤 |
| `zhuyin-cli --check-v3 DIR` | v3 Core ML 模型和訓練端輸出是否一致 |
| `make checks` | 核心行為與延遲檢查（3,000 多項） |

每個工具的說明與完整選項見 `zt eval NAME --help`。共用的選項有 `--config`、`--data`、`--cli-args`、`--set`、`-n`、`--seed`、`--jobs`、`--json`。

### 改字記錄

使用者在 App 開啟「記錄改字」後，每段有改字的組字記成 `~/Library/Application Support/ZhuyinType/corrections.jsonl` 的一行（格式見 [DESIGN.md §2.7](../product/DESIGN.md#27-改字記錄)）。這份檔案只在使用者自己的 Mac 上，評測前由使用者匯出提供；它含有使用者打的字，不放進版本庫，也不放進資料目錄的共用檔案。讀取記錄的程式在 `ml/src/zhuyintype_ml/core/corrections.py`。

`zt eval corrections` 把每筆記錄當時的按鍵再打一次，問引擎現在能不能不靠這次改字就打出使用者最後送出的字。檔名直接接在指令後面（也可以用 `--set`），`-n` 是每個檔案從頭取幾筆（預設 0，全部）。每一種改字各重播一次：

| 改字 | 重播的按鍵 | 對照 |
|---|---|---|
| `pick`、`literal_zhuyin`、`undo_literal`、`select`、`add_phrase`、`cursor_edit` | 最後那段組字的按鍵（`typing`），不含清單與選字 | 送出的字 |
| `backspace`、`escape` | 第一次打的按鍵（`original_typing`），也就是刪掉重打之前的鍵 | 送出的字 |
| `delete_after_commit` | 上一段組字的按鍵 | 上一段送出的字，刪掉的部分換成重打的這一段開頭同樣字數的字 |

- 重播用記錄的設定：按錯鍵修正、標點按法、台／臺、模糊音（`--typo`、`--punctuation`、`--forms`、`--fuzzy`）；`--cli-args` 明確給的選項優先。有記錄游標前的文字時，以它為前文。按鍵間隔照 `<dt:毫秒>` 重現，所以按鍵解碼看到的時間和當時相同。
- 結果分三種：`right`（打出送出的字）、`unchanged`（還是第一次改字之前畫面上的字，`shown`）、`other`。`backspace`、`escape` 在第一次改字之後還有打字時，`shown` 只是當時畫面的一部分，`unchanged` 不會成立，算在 `other`。
- 無法重播的記錄（在句中修改過，或有 `zhuyin-cli` 寫不出的鍵）只計入筆數。
- 重播不帶個人記憶：當時由記住的字詞決定的結果，重播時可能不同。
- 報告的每一項有 `id`（記錄編號加改字種類）、`group`（改字種類）、`reference`、`output`、`shown`、`keys` 與 `outcome`，可以用 `zt eval stats` 比較兩個版本的引擎。

`zt data corrections FILE… --summary` 列出改字的種類與次數、最常見的「刪掉的鍵 → 重打的鍵」（例如 `1j6 → 1j4`），以及最常見的「畫面上的字 → 改成的字」（選字、刪掉重打、送出後刪掉重打）。`--rows OUT` 把可重播的項目寫成 `前文|答案|注音` 的列，給 `zt eval rows` 使用：注音由按鍵推出，只收每個音節都以聲調鍵或空白結束、沒有刪除、按鍵順序和注音順序相同、每個字一個音節的項目；刪掉重打的項目用第一次打的鍵，所以按錯的鍵保留在注音中。

## 5. 常用指令

```sh
zt eval sets                                              # 評測集與是否已下載

# app 的設定，兩個開發集各 400 段
zt eval conversion --set colloquial-dev wiki-dev

# 只用引擎；改一項設定；存成報告
zt eval conversion --config engine --set dev
zt eval conversion --cli-args "--rerank-weight 0.5" --set chat-dev --json data/build/eval/weight-0.5.json

# 有前文、分兩次送出
zt eval conversion --context line --set wiki-dev
zt eval conversion --split continue --set colloquial-dev

# 打字習慣、候選字、免選字
zt eval errors --set colloquial-dev wiki-dev --examples 3
zt eval candidates --set dev
zt eval repick --relearn --set colloquial-dev

# 有按鍵時間的打字錯誤，比較兩組選項
zt eval typing-errors --config engine --variant current "" --variant timed "--decoder timed"

# 善解的評測集
zt eval rows --set shanjie

# App 的改字記錄：重播、統計、轉成列
zt eval corrections corrections.jsonl --json data/build/eval/corrections.json
zt data corrections corrections.jsonl --summary --rows data/build/eval/correction-rows.txt

# 在 GPU 主機上執行同一個指令
zt --remote eval conversion --set dev
```

## 6. 報告與成對比較

每個評測的結果是一份報告：工具名稱、設定、選項、執行環境（日期、平台、commit），以及每個評測集的一節。每一節有數字（`metrics`）、每個項目的結果（`items`）與幾個轉錯的例子。`--json FILE` 把報告存成 JSON，其他工具讀 JSON，不解析文字輸出。

`items` 的每一項有 `id`、`reference`（原文）與 `output`（送出的結果），有些工具另外記錄按鍵、候選名次或 N-best。一個工具量多種條件時（例如按錯比例與容錯模式、打字錯誤的條件與選項組），項目另有 `group`。

`zt eval stats` 讀兩份報告，在每一節（與每個 `group`）內依 `id` 配對項目，逐節比較：

```sh
zt eval conversion --set dev --json data/build/eval/base.json
zt eval conversion --set dev --cli-args "--rerank-weight 0.8" --json data/build/eval/new.json
zt eval stats data/build/eval/base.json data/build/eval/new.json --jobs 4
```

一個項目在選定的正規化下（`--normalization raw|variants|shanjie`；善解格式的結果預設 `shanjie`，其他預設 `raw`）輸出等於原文就算對；錯誤數是對的時候為 0，否則為 Levenshtein 距離。改對與改壞都以新結果減舊結果計算。bootstrap 以項目為單位重抽，項目彼此相關時用 `--clusters` 指定群組。最小可偵測效果以觀察到的不一致數為條件，評測之後算出來的值只作為參考，不等於事前的檢定力分析。`zt eval stats --discordant 61` 只計算給定不一致數下的最小可偵測效果。`zt eval stats` 也讀舊工具的逐列 JSONL 與「答案、輸出」TSV。

## 7. 設定掃描

`zt eval sweep` 依一個 JSON 矩陣，對每組設定與每個評測集各跑一次評測（預設 `conversion`），每個組合存一份報告，最後列出整句全對率與耗時：

```json
{"evaluator": "conversion",
 "configurations": [{"name": "plain", "config": "engine"},
                    {"name": "reranker", "config": "engine",
                     "cli_args": ["--reranker", "{data}/reranker", "--rerank-count", "16", "--rerank-weight", "1"]}],
 "sets": [{"name": "colloquial", "set": "colloquial-dev"},
          {"name": "context", "set": "wiki-dev", "options": ["--context", "line"]}]}
```

`config` 是 `app` 或 `engine`（預設 `engine`），`cli_args` 是 `zhuyin-cli` 選項，其中的 `{data}` 代表資料目錄。`sets` 的 `options` 是評測工具的選項。名稱只能使用英文字母、數字、`_` 與 `-`。報告預設存在 `data/build/eval/sweeps/<矩陣名稱>/`。

## 8. 每版評測（ZhuyinBench）

每次發布（`v*` tag）後，GitHub Actions 在 GPU 主機的自建 runner 上執行 `zt bench run <版本>`。它先準備主機：下載固定版本的資料與模型、編譯 Linux 版引擎、編譯詞庫與語言模型、匯出 ONNX；再在同一個程序裡透過評測登錄執行整套評測（和 `zt eval` 用同一份程式），寫出 `docs/benchmarks/<版本>.json`，最後重建每版頁面與歷代表，提交到 main。每一份報告（含每個項目）也存在 `data/build/bench/<版本>/`，可以用 `zt eval stats` 和其他版本做成對比較。Mac 上量到的延遲與能耗由發布流程以 `zt eval latency --data dist/ZhuyinType.app/Contents/Resources --json …` 附在 release 的 `metrics-mac.json`，也一起列出。也可以手動執行 Benchmark workflow 評任何版本；`zt bench run check-N --quick` 每組只跑 50 段，用來檢查流程。

套件內容固定（`SUITE = zhuyinbench-1`）：

| 項目 | 工具 | 設定 | 評測集 |
|---|---|---|---|
| 整句轉換 | conversion | app、engine | `wiki-test`、`colloquial-test`、`smoke`、`chat-test` |
| 前文 | conversion `--context line` | app | `wiki-test` |
| 分兩次送出 | conversion `--split continue` | app | `colloquial-test`、`wiki-test` |
| 打字習慣 | errors | app | `colloquial-test`、`wiki-test` |
| 候選字清單 | candidates | app | 四個測試集 |
| 善解評測集 | rows | app、engine | `shanjie` 群組 |
| 免選字 | repick | app | 四個測試集 |

改變內容時換新的套件名稱，新舊數字不混在一起。紀錄的欄位與 v0.4.0 相同：評測集以檔名為鍵，百分比照輸出的位數四捨五入。`zt bench history` 只用紀錄重建頁面，不跑評測。

改寫成 `zt eval`／`zt bench` 時，在 GPU 主機上以相同的引擎選項與固定的雜湊種子比對了新舊工具：整套 ZhuyinBench、打字錯誤、按錯比例、設定掃描與 `repick --relearn` 的每一筆 `zhuyin-cli` 輸入與輸出都相同，每個數字也相同。以固定的雜湊種子重跑 v0.4.0 的套件，只有打字習慣的 typo5 與免選字幾格和原紀錄差 0.2–0.6 個百分點，來自 §1 所說的雜湊順序。

測試集在這裡只用來追蹤；採用一項改變要看開發集上的成對比較，並記在 [research/log.md](../research/log.md)。

## 9. `zhuyin-cli` 選項

調參數用的選項（給 `--cli-args`）：

| 選項 | 意思 |
|---|---|
| `--models DIR` | 用 DIR 裡的所有模型，設定方式和 app 相同（`--config app` 用的就是它） |
| `--reranker DIR`、`--rerank-weight W`、`--rerank-count N`、`--exact-margin M` | 整句重排模型、權重、句數、打對優先的差距 |
| `--v3 DIR`、`--v3-weight W` | 音節對字模型與它在詞圖的權重 |
| `--chat-lm DIR`、`--chat-prior P`、`--domain-scale K`、`--domain-centre D` | 聊天模型、沒有前文時的聊天比例、前文影響的強度與中心 |
| `--end-share S` | 句尾比例 |
| `--decoder slots\|boundaries\|reorder\|timed`、`--decoder-costs …`、`--hotspot` | 按鍵解碼方法、成本、熱點成本 |
| `--typo off\|standard\|aggressive`、`--fuzzy zh,ch,…` | 容錯、模糊音 |
| `--context TEXT` | 游標前的文字 |
| `--forms standard\|common` | 「臺」或「台」。CLI 預設 `standard`，即引擎原本的字，可直接和評測文字、舊的數字比較；app 預設 `common` |
| `--timing` | 印出每次模型呼叫的時間，最後加上延遲、CPU、能耗、記憶體摘要 |
| `--rerank-gate τ`、`--rerank-switch δ` | 整句重排只在引擎前兩名差距小於 τ 時使用；換句要贏過 δ |
| `--freeze-on-pick on\|off` | 在清單選字後，其他已顯示的字是否固定 |
| `--v3-list on\|off`、`--v3-boundaries on\|off` | 清單排序是否參考 v3；是否告訴 v3 哪些音節是一聲不按空白 |
| `--report-list`、`--report-nbest K` | 批次模式印出打開的候選字清單，或引擎的 K 個最佳句子（tab 分隔），評測工具自己會加 |

批次輸入每行一段按鍵；`<dt:MS>` 指定和前一鍵的間隔，`<enter>`、`<left>`、`<sdown>`（⇧↓）、`<sup>`（⇧↑）等表示特殊鍵；「文字<tab>按鍵」把前面的文字當作前文。

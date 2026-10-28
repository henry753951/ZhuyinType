# reading：v3 的讀音雜訊

從 App 使用的 v3 繼續訓練，訓練時把一部分字的讀音換成使用者實際會打的讀音（台灣讀音、口語讀音、輕聲打本調、三聲變調、不熟悉的字打成其他讀音），讓 v3 依前文判斷打的讀音代表哪個字。

## 用途

- v3 看的是打的讀音。打 熟 ㄕㄡˊ、蚵 ㄎㄜ 這類詞庫以外的讀音時，原本的 v3 會壓低正確的字。這個流程用引擎讀音通道的同一張字音表（`lexicon_coverage/channel/`，格式見 [reading-channel.md](../../../../../resources/formats/reading-channel.md)）產生讀音雜訊，狀態與 `cost_bucket` 照表上的值：接受的讀音是 0，修正依成本分桶。
- 套用的位置和引擎的讀音通道相同：單字詞用字層級的列（`*`），任何詞用該字在文中讀音的列（詞層級的台灣讀音、不熟悉的字的其他讀音），輕聲字依 `neutral_full_tone` 打成本字的其他讀音，三聲前的二、三聲依 m 互換。
- 每個位置換讀音的機率是 min(0.5, Σ w·p)。p 是表上的機率；w 在 `s<倍數>` 的訓練裡對所有類別都是倍數，在 `r<倍數>` 裡只對機率是 m 或輕聲比例的類別（`noise.RARE`）是倍數，其他是 1。倍數在開發集上選。
- 其他部分和 0.4.0 的在／再微調（[../homophone/README.md](../homophone/README.md)）相同：同一批訓練資料與混淆字加權、學習率、批次大小與前文丟棄，前文編碼器不動。起點是發布的 v3（`v3_homophone2/`）。
- 產物是候選模型 `v3_reading_<名稱>/`，用 `zt data pack` 打包；採用前不發布。

## 指令

```sh
zt --remote --nice 10 --detach train reading test gap train evaluate        # GPU 主機
zt --remote train reading train --option runs=s10,r10 --option tolerance=0.005
zt train reading export --option candidate=r10                              # Mac：Core ML、編譯、介面向量
python -m zhuyintype_ml.training.reading.gap --lexicon L.tsv --misread DIR --out DIR
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：各位置套用哪些列、狀態與分桶、上限、成對的列 | 任一 |
| `gap` | `misread-dev` 的句子分別照標準讀音與變體讀音打，逐行對應，給 `zt eval stats` 比較兩者的差距 | 任一 |
| `train` | 每個名稱訓練一次，每 500 步檢查一般開發集、在／再開發集與讀音開發集，選讀音開發集 NLL 最低且一般與在／再都在容許範圍內的檢查點 | GPU 主機（CUDA） |
| `evaluate` | 每個候選（含起點）用 GPU 模型伺服器跑 `matrices/` 的兩個矩陣：App 設定（data-4 詞庫）的開發集，以及 `misread-dev`、善解的列與 `gap` | GPU 主機 |
| `export` | 選定的候選匯出 Core ML（沿用 homophone 的匯出：授權、來源聲明、20 組介面向量） | 只限 macOS |

讀音開發集是 `v3_030` 的兩個開發集，每段只換一個位置的讀音（`ReadingNoise.one`），只在那個位置計分。它和字音表同源，只用來選檢查點與倍數；採用與否看引擎層級的成對比較。

## 輸入與輸出

路徑相對於資料目錄。

讀取：

- `v3_030/`（0.3.0 的 v3、詞庫、分片、開發集）、`homophone2/`（在／再微調的訓練資料與開發集）、`v3_homophone2/`（發布的 v3，訓練的起點）；
- `lexicon_coverage/channel/`（`channel.tsv`、`channel.json`、`v3_noise.tsv`）、`lexicon_coverage/misread/`、`lexicon_coverage/eval/`（善解的列）；
- `lexicon.tsv`（data-4 起，台灣讀音是標準讀音）：`gap` 的標準讀音與矩陣的打字讀音。

寫入：

- `v3_reading/`：`start.json`（起點的三項檢查）、`<名稱>.json`（每次檢查的數字、選定的步數、設定）、`gap/`、`eval/<候選>/<矩陣>/`（每組設定與評測集一份報告）、`export/<候選>/choice2.json`；
- `v3_reading_<名稱>/`：`syllable.pt`、起點的 `context.pt`、詞彙表與 `config.json`；匯出後另有 Core ML 套件、`test_vectors.json`、來源聲明與 `training.json`。

授權：程式碼 AGPL-3.0-only；模型沿用 v3 的 CC BY-SA 4.0 與 [../homophone/README.md](../homophone/README.md) 列出的來源；字音表的來源見 [resources/SOURCES.md](../../../../../resources/SOURCES.md)。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train reading` 的步驟 |
| `noise.py` | 讀 `v3_noise.tsv` 與 `channel.json`，決定每個位置可以換成哪些讀音 |
| `train.py` | 從發布的 v3 繼續訓練與檢查點選擇 |
| `gap.py` | 同一批句子照標準讀音與變體讀音打的成對列 |
| `matrices/` | `zt eval sweep` 的矩陣 |
| `test_reading.py` | 單元測試 |

## 相關文件

- [docs/engine/MODEL_V3.md](../../../../../docs/engine/MODEL_V3.md)、[docs/research/log.md](../../../../../docs/research/log.md)：做法與數字
- [resources/formats/reading-channel.md](../../../../../resources/formats/reading-channel.md)：字音表、三種狀態與 `v3_noise.tsv`
- [../lexicon/README.md](../lexicon/README.md)：字音表與 `misread-dev` 的產生方式
- [../homophone/README.md](../homophone/README.md)：起點的訓練流程

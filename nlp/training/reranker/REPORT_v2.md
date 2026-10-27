# Reranker v2 實驗

實驗於 2026-10-26 開始，2026-10-27 完成（Asia/Taipei）。最終狀態記錄在「結果」一節。

## 實驗流程

以 seed 7、既有的訓練程式與未變更的 v1 預設值訓練（batch 512、學習率 0.001、bf16、上下文 64、詞彙 12,000）。遠端各自獨立的 `reranker_v2a/`、`reranker_v2b/` 與 `reranker_v2c/` 目錄包含逐位元組相同的詞彙複本，以及指向 v1 token 串流的符號連結。這個實驗沒有變更資料準備或 App 原始碼；訓練程式與匯出程式都沒有變更。既有的 train.py、export_coreml.py 與 prepare.py 在兩台主機上的 SHA-256 都一致，因此不需要 scp。

使用遠端既有的 `.venv-gen/bin/python`（Python 3.12.15、Torch 2.13.0+cu130、NumPy 2.3.5、coremltools 9.0）。指定的 `.venv` 沒有 Torch；沒有安裝任何套件。GPU：H100L-3-47C / MIG 3g.47gb（46,587 MiB）。

本機評估透過 uv 0.12.7 使用 Python 3.12.14；CLI 以 Swift 6.4 建置。所有變體都匯出為 Core ML fp16。

以 `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex --product zhuyin-cli` 建置（通過）。把這個執行檔與既有的評估器凍結在 `data/build/reranker_v2_experiment/` 下，讓比較不受同時進行的原始碼修改影響。雜湊值與選擇規則：該目錄下的 `manifest.json`。預設的開發者目錄沒有 `coremlcompiler`；所有編譯都使用上面完整 Xcode 的 `DEVELOPER_DIR`。只用 CPU（不設定 `ZHUYIN_UNITS`）；MacBook Pro Apple M5，32 GB，macOS 27.0.1。

選擇方式：在首次呼叫之後最慢的 CPU 延遲 <25 ms 的 v2 模型中，取 dev 口語與維基百科整段準確率等權平均最高者。平手時依序比較平均字元準確率、較低的延遲，以及最接近 1.0 的權重。每個模型與 v1 都評估權重 {0.7, 1.0, 1.4}；參考用的 v1 使用其出貨時的權重 1.0。只有在 v2b 的最終困惑度在**兩個** dev token 串流上都比 v2a 低至少 2% 時，才訓練 v2c（這是事先宣告的「明顯較佳」的解讀）。

保留測試集只對鎖定的 dev 勝出者與出貨的 v1 各評估一次，從不用於調整。只有當勝出者在三個保留測試集上的整段準確率等權平均嚴格高於 v1，且延遲通過時，才升級採用。

資源復原：第一輪的完整性檢查發現共用的 `data/build/lexicon.tsv` 已與初始雜湊值不符（觀察到的修改時間為 19:13:51）。第一輪在任何保留測試集評估之前，就因雜湊檢查而停止。`data/build/lexicon.before-priors.tsv` 與初始詞庫的 SHA-256（`e7cc8a1edd019180f88ce2313715abfeb5fb5968444a5f1d4da5a756534325a1`）完全一致。第一輪所有的 dev 與計時結果都封存在 `pre-resource-freeze/` 下，不用於選擇。每一個 dev 組合與 CPU 計時都使用該原始詞庫、未變更的語言模型與 dev 檔案、CLI 及評估器的私有凍結複本重新執行。測試檔只是機械式地複製，沒有使用其結果來選擇；保留測試集評估對每個鎖定的勝出者與 v1 各執行一次。共用詞庫與同時進行的原始碼修改都保留不動。凍結資源的雜湊值在 `manifest.json`（`frozen_evaluation_sha256`）。

## 指令

當時以 `experiment_v2.py`（已移除）在 GPU 主機上排程訓練、取回模型、在 Mac 上編譯並執行 dev／保留測試集評估與計時，再把結果寫進本報告的「結果」一節。評估使用凍結在 `data/build/reranker_v2_experiment/` 的 `conversion_eval.py` 與 `zhuyin-cli`；整段準確率由評估器輸出的百分比與樣本數還原。原始輸出、清單與執行紀錄都在該目錄。依現行規則，評估改在 GPU 主機上執行。

目前的對應指令（變體目錄共用 `reranker/` 的 `vocab.json` 與 token 串流，以符號連結指向 `train.bin`、`val_*.bin`）：

```sh
# GPU 主機
python -m nlp.training.reranker.train --data data/build/reranker_v2a --steps 90000
python -m nlp.training.reranker.train --data data/build/reranker_v2b --steps 90000 --layers 8 --width 512 --heads 8 --ffn 2048
# 有條件執行的 v2c：
python -m nlp.training.reranker.train --data data/build/reranker_v2c --steps 90000 --layers 10 --width 640 --heads 10 --ffn 2560
# 每個完成的變體：
python -m nlp.training.reranker.export --data data/build/reranker_v2a
# Mac，取回 Reranker.mlpackage 與 vocab.json 之後；初始設定為 {"weight": 1.0, "count": 16}
xcrun coremlcompiler compile data/build/reranker_v2a/Reranker.mlpackage data/build/reranker_v2a/

# dev；W 為 0.7 / 1.0 / 1.4（現行工具，結果格式與當時不同）
zt eval conversion --config engine --cli-args "--reranker data/build/reranker_v2a --rerank-weight W --rerank-count 16" \
  --set colloquial-dev wiki-dev -n 400
# CPU 延遲，略過第一次計時的呼叫：
zhuyin-cli --lexicon data/build/lexicon.tsv --reranker DIR --rerank-count 16 --timing "ji3ap6vu35j/ru04xu4vu/5j4u4ek75k4cl3vu04xu4cl3wu4"
# 保留測試集，每個勝出者與 v1 各一次（相同選項，n=400）：
#   --set wiki-test colloquial-test smoke
```

## 結果

狀態：完成。

### 訓練

| 模型 | 步數 | 分鐘 | Wiki 困惑度 | 口語困惑度 |
|---|---:|---:|---:|---:|
| v1（歷史紀錄） | 30000 | 16.71 | 29.5594 | 75.2983 |
| reranker_v2a | 90000 | 49.42 | 27.5580 | 72.2539 |
| reranker_v2b | 90000 | 79.04 | 23.3940 | 65.1481 |
| reranker_v2c | 90000 | 122.17 | 20.9905 | 60.7843 |

### Dev（每組 400 次執行）

| 模型／權重 | 口語字元 % | 口語整段 % | Wiki 字元 % | Wiki 整段 % | 整段平均 % |
|---|---:|---:|---:|---:|---:|
| v1 / 0.7 | 95.9 | 80.50 | 96.3 | 76.25 | 78.375 |
| v1 / 1.0 | 95.8 | 79.00 | 96.3 | 76.50 | 77.750 |
| v1 / 1.4 | 95.9 | 79.00 | 96.1 | 75.00 | 77.000 |
| v2a / 0.7 | 96.2 | 81.75 | 96.3 | 76.00 | 78.875 |
| v2a / 1.0 | 96.2 | 81.75 | 96.2 | 74.75 | 78.250 |
| v2a / 1.4 | 96.0 | 80.50 | 96.0 | 74.00 | 77.250 |
| v2b / 0.7 | 96.3 | 82.25 | 96.4 | 76.50 | 79.375 |
| v2b / 1.0 | 96.4 | 82.25 | 96.7 | 78.25 | 80.250 |
| v2b / 1.4 | 96.3 | 81.25 | 96.7 | 78.00 | 79.625 |
| v2c / 0.7 | 96.3 | 83.00 | 96.7 | 76.75 | 79.875 |
| v2c / 1.0 | 96.3 | 82.50 | 96.8 | 77.75 | 80.125 |
| v2c / 1.4 | 96.3 | 82.25 | 96.7 | 76.75 | 79.500 |

### CPU 延遲

| 模型 | 首次 ms（不計入） | 之後每次呼叫，ms | 最大 ms | 通過 <25 ms |
|---|---:|---|---:|---|
| v1 | 3.7 | 2.1, 2.0, 3.1, 3.7, 4.6, 5.0, 6.0, 6.5, 7.3, 8.0, 8.8, 9.2, 10.1, 10.6 | 10.6 | 是 |
| reranker_v2a | 3.8 | 2.1, 2.1, 3.2, 3.7, 4.5, 5.0, 6.0, 6.5, 7.3, 8.0, 8.7, 9.3, 10.2, 10.6 | 10.6 | 是 |
| reranker_v2b | 7.0 | 4.0, 3.8, 5.6, 6.5, 8.0, 8.9, 10.3, 11.5, 12.8, 14.1, 15.6, 16.3, 17.9, 18.9 | 18.9 | 是 |
| reranker_v2c | 11.5 | 6.0, 6.0, 9.0, 10.0, 12.6, 13.8, 17.0, 22.9, 21.3, 22.4, 25.7, 26.2, 28.5, 29.9 | 29.9 | 否 |

### 保留測試集（每個模型一次）

| 模型／權重 | 測試集 | 執行次數 | 字元 % | 整段 % |
|---|---|---:|---:|---:|
| reranker_v2b-w1.0 | test_sentences.txt | 400 | 96.8 | 76.75 |
| reranker_v2b-w1.0 | test_colloquial.txt | 400 | 96.5 | 81.25 |
| reranker_v2b-w1.0 | colloquial_smoke.txt | 68 | 99.8 | 98.53 |
| reranker_v2b-w1.0 | **等權平均** | — | 97.70 | **85.510** |
| v1-w1.0 | test_sentences.txt | 400 | 96.6 | 76.50 |
| v1-w1.0 | test_colloquial.txt | 400 | 96.1 | 79.25 |
| v1-w1.0 | colloquial_smoke.txt | 68 | 99.8 | 98.53 |
| v1-w1.0 | **等權平均** | — | 97.50 | **84.760** |

### 決定

採用 reranker_v2b，權重 1.0：保留測試集的整段平均比出貨的 v1 高 0.75 個百分點；首次之後最慢的 CPU 呼叫為 18.9 ms。勝出者的產出物在 data/build/reranker_v2/。

原始評估輸出、計時、複製與編譯紀錄、清單與機器細節：`data/build/reranker_v2_experiment/`。訓練紀錄：遠端的 `logs/reranker_v2*-train.log` 與本機各變體的 `train-log.json`。

既有的評估器以 0.1 個百分點的精度輸出字元準確率。整段的計數可以從輸出的百分比與樣本數（最多 400 次執行）明確還原，因此整段平均與採用門檻都使用精確的分數。延遲使用指定的單一打字序列，並略過第一次計時的呼叫。沒有執行更廣的工作負載、App UI 或安裝測試。

完整的依時間順序執行紀錄，包括已復原的第一輪中斷，在 `data/build/reranker_v2_experiment/experiment.log`。選擇與採用只使用上面以乾淨凍結資源量測的結果。

最終產出物驗證通過：升級的 mlpackage 與 mlmodelc 在每個檔案的 SHA-256 上都與 v2b 一致；詞彙與 v1 一致（12,000 個條目）；設定為 {"weight": 1.0, "count": 16}。所有凍結資源的雜湊值都維持不變。保留測試集指令恰好執行兩次，一次給鎖定的勝出者，一次給 v1。詳情：`data/build/reranker_v2_experiment/adoption-check.json`。

## 在 0.2.3 引擎上的驗證

2026-10-27 以目前的引擎（讀音先驗、變調、完全相符邊際 4.0、台灣讀音、<unk> 分攤）與出貨的詞庫重新執行，權重 1.0，16 個候選字，n = 400；「上下文」指
`conversion_eval.py --line-context`（現行的 `zt eval conversion --context line`）。整段準確率：

| 模型 | 維基百科 | 口語 | 日常 | 維基百科 + 上下文 | 口語 + 上下文 |
|---|---:|---:|---:|---:|---:|
| v1 | 80.5 | 80.5 | 100.0 | 83.0 | 80.8 |
| v2a | 79.8 | 80.8 | 100.0 | 83.0 | 81.0 |
| v2b | 80.8 | 82.8 | 100.0 | 85.2 | 83.0 |

輸入一個 25 音節句子時，每次重新排序呼叫的 CPU 時間（`zhuyin-cli --timing`），不計第一次呼叫：
v1 2.1–11.0 ms，v2a 2.1–11.0 ms，v2b 3.8–20.9 ms。v2b 是唯一明顯的進步（含上下文 +2.2 個百分點），每次呼叫約慢
1.9 倍；v2a 與 v1 速度相同，但沒有明顯進步。目前尚未採用：App 的採用規則是「在測試集上較好，而且不更慢」。

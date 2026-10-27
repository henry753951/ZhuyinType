# v2b 前綴樹評分：2026-10-27

樹狀模型與共用的 Swift 路徑都已實作，但**只部分通過驗收**。
在 Apple M5 上，修正後的模型把 25 音節組字的 p95 從 **53.4 降到 39.9 ms**，能耗從
**162.46 降到 102.74 mJ/key**。要求的 **p95 < 30 ms 目標未達成**。遠端 fp32 ONNX 一致性
通過：**2,000/2,000 行完全相同**，**5,209/5,209 個 argmax／exactMargin 決策完全相同**，
沒有退回批次模型。

Core ML fp16 的抽樣檢查仍超過要求的一般 <0.01-nat 分數容許誤差（短句中位數 0.026 nats，
最大 0.106）。修正一個整數索引轉換錯誤後，抽樣的選擇結果一致，但這不足以證明 Core ML 的
決策一致性達到 99.9%。不要把它當成完全通過驗收的 App 變更。依照「重複失敗即停止」的指示，
正規化實驗未能解決這個限制後，精度與效能調校就停止了。預設仍為 CPU；沒有採用任何加速器
或上下文快取的變更。

## 實作

- `export_tree.py` 從 `train.py` 匯入 `CharLM`、`Config` 與原本的區塊。訓練行為與所有
  檢查點權重、token ID 都不變。
- trie 節點由它的父節點與編碼後的 token 識別。接在不同前綴後的相同字元是不同的節點。
  OOV 字元可以共用 `<unk>` 狀態，保留既有的逐字未知懲罰。
- RoPE 以明確的**深度**索引原本的 cosine/sine 表，而不是節點在打包後的位移。每一列注意力
  只允許自己與其祖先節點。遮罩使用 0 / -10000，被遮蔽的機率在這些精度下會下溢為零。
- 終端葉節點只作為目標：它們的機率來自父節點的 logits，因此不需要計算它們的隱藏狀態。
  上下文節點只計算一次。輸出分數沿著每個候選的路徑加總；不計入上下文分數。空候選、重複、
  長度不等、上下文截斷，以及 5,465 類的未知懲罰都維持原本的行為。
- 輸入：`ids`、`positions` `[1,N]`；`ancestors` `[N,N]`，整數 1 表示允許；
  `parents`、`targets` `[E]`。輸出 `logprob` `[E]` 為每個目標在其父節點上的分數。
  Swift 為 Core ML 與 ONNX 建構相同的陣列。`scores`、`candidateScores` 與 `choose` 保留
  原本的公開簽章，以及混合與 exactMargin 規則。
- Core ML 使用 fp16 與彈性範圍 1…2017，涵蓋 32 個 63 字的候選字清單項目加 BOS（因此也
  涵蓋 16 個句子候選）。深度仍受既有 64 token 的模型上下文限制。ONNX 使用 fp32，N/E 為
  動態。
- Core ML 匯出移除了 `common::add_int16_cast`：它的符號形狀分析錯誤地把攤平的父節點 ×
  詞彙索引縮窄為 uint16，超過 65,535 就溢位。Transformer 的運算維持 fp16；gather 索引維持
  int32。匯出時的 MIL 斷言防止這個問題再次發生。
- `RerankerTree.mlmodelc`／`RerankerTree.onnx` 會自動偵測。樹狀結果缺少、無法載入、拋出
  錯誤、格式錯誤或含非有限值時，會退回既有的批次模型。超過 32 個候選時沿用批次路徑；候選字
  清單評分維持既有的每批 32 個項目。
- `TREE_RERANKER=1` 讓遠端執行腳本另外匯出 ONNX 模型。其他工作的預設匯出行為不變。選用的
  CUDA socket 後端仍使用批次評分。

本任務沒有變更任何相依套件、套件管理工具、全域預設值、ONNX C 橋接、訓練行為、引擎、App
或腳本。沒有安裝 App，也沒有執行 git commit、stash、reset 或 checkout。

目前的 `scripts/build-app.sh` 只複製原本編譯好的 reranker。該腳本屬於另一個同時進行的
任務，保留不動：打包時必須同時複製 `data/build/reranker/RerankerTree.mlmodelc`，新打包的
App 才能使用樹狀模型。執行期的模型目錄覆寫已經可以載入它。

## 產出物與可重現性

模型與目前的 reranker 一起放在 `data/build/reranker/`：

- `charlm.pt`：從 `~/zhuyintype/reranker_v2b/charlm.pt` 取回，以便用完全相同的權重匯出；
  目前的模型檔案都保留。
- `RerankerTree.mlpackage`、編譯後的 `RerankerTree.mlmodelc`、`RerankerTree.onnx`。
- `tree-validation/`：凍結的原始碼、二進位檔、原始計時紀錄、實驗產出物、輸入按鍵、清單與
  摘要 JSON。

本機既有工具：`data/build/v3/.venv` 中的 Python 3.12.14、PyTorch 2.7.0、coremltools 9.0；
Xcode Swift 6.4；Apple M5，32 GiB RAM。不需要安裝任何東西。遠端使用原本就有的獨立 ONNX
環境與 Swift 6.4.1。

```sh
zt train reranker tree compile        # Mac：python -m zhuyintype_ml.training.reranker.export_tree --format coreml，再編譯
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  swift build -c release --scratch-path .build-codex4

# Linux：
python -m zhuyintype_ml.training.reranker.export_tree --format onnx
# 或使用 ml/src/zhuyintype_ml/remote 的合併匯出程式，加上選用的 --tree 旗標。
```

`tree-validation/frozen` 固定了比較用的引擎、CLI、套件與評估器輸入。基準與樹狀版本的建置
之間只替換了 ZhuyinML。唯一的例外是另一位工程師既有、只在 Linux 上的 `getrusage` 型別
修正：第一次 Linux 建置暴露出不相容後，把它複製進 CLI 快照；它不改變轉換結果，也不影響
Mac 分支。最終的計時對照使用**同一個最終 CLI 二進位檔**，比較只含基準模型的目錄與啟用樹狀
模型的目錄。`manifest-mac.json` 記錄凍結原始碼的雜湊值。

## Mac 計時

所有 Mac 上的模型量測都**只使用 `zhuyin-cli --timing`**，參數為：

```text
--reranker DIR --rerank-count 16 --rerank-weight 1
--v3 data/build/v3 --v3-weight 0.5
--chat-lm data/build/lm_chat --chat-prior 0.5
```

Mac 上沒有執行任何準確率評估器。每個主要案例重新啟動 CLI 三次，沒有前文、不按 Enter，
最後可選擇按一次 `<down>`。表格使用三次回報的 p50/p95、CPU、能耗與記憶體數值的中位數；
**最大值是三次執行中最差的一個按鍵**，包括暖機與排程造成的離群值。能耗是 CLI 依 macOS
估計的單一程序數值，不是整台機器的能耗。共用系統上的其他工作持續進行；這些計時不是隔離的
硬體量測。

| 輸入／路徑 | 模型 | 按鍵數 | p50 ms | p95 ms | 最大 ms | CPU ms/key | 能耗 mJ/key | 記憶體 目前／峰值 MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 短句，輸入中 | 目前 | 33 | 16.2 | 31.8 | 35.3 | 16.7 | 81.84 | 328 / 332 |
| | 樹狀 | 33 | 10.6 | 21.6 | 56.2 | 10.7 | 52.38 | 328 / 331 |
| 短句 + down | 目前 | 34 | 15.1 | 29.6 | 34.9 | 16.3 | 81.04 | 328 / 332 |
| | 樹狀 | 34 | 10.3 | 22.1 | 23.9 | 10.6 | 51.99 | 328 / 331 |
| 長句，輸入中 | 目前 | 76 | 29.6 | 53.4 | 91.4 | 31.3 | 162.46 | 333 / 343 |
| | 樹狀 | 76 | 19.0 | 39.9 | 44.4 | 20.1 | 102.74 | 335 / 342 |
| 長句 + down | 目前 | 77 | 30.6 | 52.4 | 60.7 | 31.3 | 164.06 | 333 / 348 |
| | 樹狀 | 77 | 19.2 | 37.4 | 46.0 | 19.5 | 102.66 | 335 / 342 |

候選字清單的模型呼叫時間，由輸入後 CLI 的 `[candidates … ms]` 行加總，短句從 **14.9 → 2.8 ms**，
長句從 **46.4 → 3.8 ms**。這些加總是模型呼叫時間，不是另外量測的整個 down 鍵延遲。彙總的
計時列已包含 down 鍵。

要求的短句「今天晚上要不要一起吃飯」實際上有 **11** 個音節。「我們明天下午三點在公司門口
集合然後一起去吃晚餐好嗎」有 25 個。讀音與按鍵直接取自 `conversion_eval.py` 的
`load_lexicon`、`readings_for`、`keys_for`，未做更動；沒有人工挑選的變調或替代讀音：

```text
11: rup<space>wu0<space>j03g;7ul41j4ul4u<space>fu3t<space>z04
25: ji3ap7au/6wu0<space>vu84j3n0<space>2u03y94ej/<space>n<space>ap6d.3ru6ck6b06c.4u<space>fu3fm4t<space>j03h0<space>cl3a87
```

主要的原始結果：`tree-validation/{before-control,after-int32}/*.log`，各附 `results.json`；
合併的中位數：`summary-timing.json`。最初的試驗保留在 `before`、`tree-cpu`、`tree-pruned`
與 `after-final` 下。**`after-int32` 之前的所有樹狀計時試驗都使用了會溢位的 uint16 模型，
不能作為驗收依據**，包括先前回報的 38.6 ms。它們保留作為失敗的證據，不替換進最終比較。

這些計時由已移除的一次性腳本 `time_tree.py` 產生：對上面兩句各以 `zhuyin-cli --timing` 執行三次（有無最後的
`<down>`），旗標如上，從輸出解析 p50／p95、CPU、能耗與記憶體，寫到 `tree-validation/<label>/`。現行的延遲與能耗
量測工具是 `ml/src/zhuyintype_ml/eval/mac_metrics.py`。

基準目錄只含指向 `../../Reranker.mlmodelc`、`../../vocab.json`、`../../config.json` 的連結，
沒有樹狀模型。

## 運算單元與快取

- 修正後的彈性樹狀模型，CPU+ANE 單次檢查：25 音節 p95 **36.2 ms**，CPU **19.3 ms/key**，
  能耗 **101.55 mJ/key**，記憶體 **338 / 346 MB**；CPU 三次執行的範圍則為 **37.1–40.1 ms**
  （中位數 39.9）。這次單一試驗較快，但在共用負載變動的情況下，無法證明運算單元的配置能
  穩定帶來好處。它仍未達到 30 ms，也沒有解決 fp16 一致性，因此沒有變更運算單元的預設值。
  原始紀錄：`after-int32-ane/`。允許使用某個運算單元，不代表實際在 ANE 上執行。
- 較早的 CPU+ANE 與固定 128 試驗使用了有問題的 uint16 匯出。它們的數字不能作為驗收依據。
  固定形狀試驗也允許較大的 trie 退回，且沒有量測配置與退回次數。它的填充與形狀分派程式碼
  已移除；實驗原始碼、二進位檔與模型保留在 `tree-validation/` 下供檢視。沒有採用任何分桶
  變體。
- 沒有實作上下文 KV 快取。這個匯出的計算圖沒有 state／KV 介面，而 App 支援 macOS 14；
  乾淨的持久 MLState 需要較新的部署目標或 API 路徑，或明確的逐層 KV 輸入與輸出。由於長句
  基準測試沒有前文，它也省不到該測試中的候選計算。既有的 v3 上下文快取維持不變。
- Core ML 有界彈性形狀的行為依照 Apple 的
  [彈性輸入指南](https://apple.github.io/coremltools/docs-guides/source/flexible-inputs.html)；
  有狀態模型的 API 記載於
  [MLState](https://developer.apple.com/documentation/coreml/mlstate)。

## 遠端 fp32 驗收：通過

執行腳本獨立放在 `$ZHUYIN_REMOTE_HOST:~/zhuyintype/work/onnx-eval-tree`。它的 `.venv`
以唯讀方式連結既有的評估環境；資料與模型輸入先從既有的評估目錄複製，再由 `run.sh` 依
checksum 同步。沒有停止任何 GPU 訓練工作區或其他評估。

當時的做法：以 `ml/src/zhuyintype_ml/remote/run.sh sweep --matrix ml/src/zhuyintype_ml/remote/tree_validation.json -n 400`（`TREE_RERANKER=1`、
`JOBS=60`、凍結的引擎快照）擷取十個矩陣列，再在遠端執行已移除的一次性腳本 `validate_tree.py`：它把
`check_tree.swift`（也已移除）和 `engine/swift/Sources/ZhuyinML` 一起編譯，連結建置好的 ZhuyinCore 與 ONNXBridge，
以實際的 Composer 重播擷取到的輸入並比較每一次模型呼叫。原始證據在 `data/build/reranker/tree-validation/`；
兩個腳本可從 git 歷史取回（0.4.0 的提交）。

基準 ONNX 目錄 `data/build/reranker/tree-validation/current` 只含指向目前 `Reranker.onnx`、
詞彙表與設定的連結。五個測試集是 dev_colloquial、dev_wikipedia、dev_chat_tt、維基百科
`--line-context`，以及口語 `--split continue`，每個都是 400 行，使用上述 App 旗標。
停用樹狀匯出並不會停用已在磁碟上的模型；受控比較需要一個只含基準模型的目錄。

`validate_tree.py` 檢查有序輸入完全對齊，以及每列不同的行數 ≤0.5%，並列出每一行差異。它
從每個測試集取 40 行，透過實際的 Composer 重播，為每一次 N-best 呼叫評分，並在確認送出前
開啟候選字清單。它比較句子分數、神經網路 argmax、合併分數 argmax 與 exactMargin 決策，
要求 ≥99.9% 的決策完全相同。包裝後的樹狀預測器會斷言執行成功、輸出形狀正確且數值有限；
退回批次模型無法在這項研究中默默通過。

結構與實際模型的邊界檢查都已通過：兄弟節點隔離、深度、共用前綴、前綴候選、重複、空候選、
OOV、長上下文、63/64 字上限、32 × 63 的最差情況樹、格式錯誤輸出時的退回，以及模型拋出錯誤
時的退回。邊界案例的句子分數最大誤差為 **0.0000815392 nats**，包括 1,985 個隱藏節點／
2,017 個目標的最差情況。

| 測試集 | 目前準確率 | 樹狀準確率 | 不同行數 / 400 | 目前秒數 | 樹狀秒數 |
|---|---:|---:|---:|---:|---:|
| dev_colloquial | 87.0% | 87.0% | 0 / 400 | 175.41 | 97.66 |
| dev_wikipedia | 80.5% | 80.5% | 0 / 400 | 197.68 | 87.29 |
| dev_chat_tt | 70.5% | 70.5% | 0 / 400 | 94.66 | 77.39 |
| 維基百科 line-context | 85.8% | 85.8% | 0 / 400 | 277.91 | 67.60 |
| 口語 split-continue | 81.2% | 81.2% | 0 / 400 | 91.04 | 44.89 |

**輸出差異：無。** 所有有序的 CLI 輸入都完全一致。時間包含在實驗室其他工作同時進行下的
啟動與準備，不是隔離的 CPU 基準測試。兩種設定都以 JOBS=60 與 -n 400 執行。

分數重播涵蓋 **200 行、5,209 次實際模型呼叫、82,831 個候選句子分數**，包括候選字清單的
呼叫。分數絕對誤差：中位數 **0**，p95 **0.00000429153**，最大 **0.0000203997 nats**。每個
分數都低於 0.01 nats。神經網路 argmax、合併分數 argmax 與 exactMargin 選擇各自都是
**5,209 / 5,209 完全相同（100%）**。決策差異：**無**。包裝後的預測確認執行的是樹狀模型，
而不是默默退回批次模型。實際呼叫中的 trie 隱藏節點數中位數為 16，最大為 218。

取回的原始證據在 `tree-validation/remote/matrix/*.json` 與
`tree-validation/remote/parity/{summary.json,*-calls.json,edges.json}`。重播與邊界檢查只在
Linux 上執行。這些結果**不能證明 Core ML fp16 完全一致**。

既有的 Linux `BackendTests` 也通過 **2/2**，涵蓋注入的批次編碼、填充、OOV 懲罰、候選分批、
exactMargin 邊界，以及 v3 輸入輸出對應。最終的根目錄與凍結 Mac release 建置都通過。本機與
遠端的檢查點、詞彙表與樹狀 ONNX 的 SHA256 一致：

```text
charlm.pt         e7f5ebcff4ae2b61d78647aa73ee7a8efe9abce86aa86626394325ae34081a53
RerankerTree.onnx dafc21e08790c087b6098dcd4b1258fb41bfb3aad7d257c15659884da0cae863
vocab.json        59fc563c0989ec4643652194e0695655d02ebff831b5fb635c7cc2618644c0f3
```

最終的可攜清單在**全部 51 個使用到的原始碼、資料與模型檔案**上一致，沒有缺少或不同的檔案。
證據：`tree-validation/manifest-portable-mac.json` 與
`tree-validation/remote/manifest-comparison.json`。最終 Core ML 產出物的雜湊值另外記錄在
`manifest-coreml-final.json`。

## Core ML fp16 抽樣檢查：分數容許誤差未達成

只使用兩個要求的基準輸入，透過 `zhuyin-cli --timing --nbest 16` 並搭配相同的 App 旗標。這會
輸出實際的 N-best 清單，以及到小數點後三位的神經網路分數；這些額外的執行不計入延遲與能耗
表格。指令保留在 `tree-validation/check-coreml-nbest.py`；原始紀錄與 JSON 在
`coreml-nbest-int32.*` 與 `coreml-corrected*` 下。

int32 索引修正消除了原本有問題的匯出中 85–286 nat 的誤差。抽樣中剩下的 fp16 差異很小，但
並非一律低於 0.01：以第一次修正後的抽樣檢查為準，11 音節中位數 **0.026**，最大 **0.106
nats**；25 音節中位數 **0**，最大 **0.105 nats**。抽樣的神經網路、合併分數與 exactMargin
argmax 決策都一致。只檢查兩份清單，不足以證明 Mac 上的決策一致率達到 99.9%。

也測試了保留 fp32 輸出正規化的做法：短句／長句的差異中位數為 **0.0245 / 0.013 nats**，最大
**0.096 / 0.098**。選擇結果一致，但仍未達到要求的容許誤差，因此不採用該變體。最終匯出維持
原本的 fp16 運算，加上 int32 索引修正。沒有再嘗試其他精度變更或大範圍的 Mac 準確率掃描。

匯出程式的回歸檢查直接確認最終 gather 索引的型別。原因與排除做法與 Apple 的
[整數轉型 pass](https://apple.github.io/coremltools/_modules/coremltools/converters/mil/mil/passes/defs/quantization.html)
一致；該 pass 的動態形狀檢查只防範已知的大維度。有問題的編譯程式中的 `var_811_to_uint16`
轉型保留在被否決的模型產出物中；修正後的匯出程式在轉換後斷言為 int32。

## 失敗與限制

- 第一次呼叫執行腳本時，評估器快照的巢狀結構有誤；已修正快照的配置。之後在改用最終模型
  格式時，以確切的 PID 7291 停止一個本任務自己的 rsync。沒有終止其他程序。
- 葉節點剪枝最初在初始化 `ancestors` 之前就擷取了 Swift 的 `self`；已改為在 closure 使用前
  先初始化該欄位。兩次最終 Mac 建置都成功。
- 第一份 Linux CLI 快照暴露了既有的 `getrusage` 列舉不符；在快照中使用另一位工程師原本就有、
  只在 Linux 上的型別修正。
- 有一次分數驗證在矩陣擷取結果存在之前就啟動了。結構與模型的邊界檢查通過，之後腳本正確地
  拒絕不完整的五集比較。它的紀錄有保留；驗收必須使用所有擷取結果都存在後的最終成功執行。
- Core ML 的 uint16 索引縮窄導致很大的分數誤差，儘管 Linux 上一致。已修正匯出、加入 MIL
  斷言，並捨棄所有較早的樹狀計時結果。剩下的 fp16 分數容許誤差失敗記錄在上方。
- 十個完整的矩陣列都輸出後，被即時修改的執行腳本記錄了
  `syntax error near unexpected token elif`。目前的檔案通過 `bash -n`；它當時正被同時修改。
  這個失敗與長時間執行的 shell 在已變更的檔案位移處繼續讀取的情況相符（這是推論，並未重現
  shell 的錯誤）。所有完整的擷取結果都經過獨立驗證。之後的整合冒煙測試使用凍結在記憶體中
  的執行腳本文字，因此同時進行的修改無法改變它剩下的 shell 輸入。該冒煙測試以 App 設定與
  JOBS=60 執行，包括建置、匯出與自動取回，結果 **4/4 行通過，準確率 100%**。
  紀錄：`tree-validation/runner-immutable-smoke.log`。
- Mac 上沒有執行語料準確率評估器。本報告不主張已達成 25 音節 p95 目標、ANE 實際執行了計算圖，
  或已量測上下文 KV 快取。延遲仍需要後續改善。

## 摘要：部分通過驗收

| 量測 | 目前 | 樹狀 |
|---|---:|---:|
| 11 音節：p50 / p95 | 16.2 / 31.8 ms | 10.6 / 21.6 ms |
| 11 音節：能耗 | 81.84 mJ/key | 52.38 mJ/key |
| 25 音節：p50 / p95 | 29.6 / 53.4 ms | 19.0 / 39.9 ms |
| 25 音節：能耗 | 162.46 mJ/key | 102.74 mJ/key |
| 長句候選字清單的模型呼叫 | 46.4 ms | 3.8 ms |
| 五集 fp32 輸出一致性 | 基準 | 2,000/2,000 相同 |
| 實際呼叫的 fp32 決策一致性 | 基準 | 5,209/5,209 相同 |
| 一般 fp32 分數誤差 < 0.01 nats | 參考 | 通過（最大 0.0000204） |
| 一般 Core ML fp16 分數誤差 < 0.01 nats | 參考 | **抽樣檢查未達成** |
| 25 音節 p95 < 30 ms | 否 | **否** |

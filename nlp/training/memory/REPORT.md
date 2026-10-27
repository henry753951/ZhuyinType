# 記憶體對應的詞庫與語言模型：2026-10-27

實作與檔案配置：[../lm/LEXICON_FORMAT.md](../lm/LEXICON_FORMAT.md)。
編譯器：`scripts/compile-data.sh OUT_DIR`（使用既有的 `.venv` 與標準函式庫，不安裝套件，
不連網）。App 與建置腳本的整合由另一位工程師負責；這次變更不修改 `Sources/ZhuyinTypeApp/`、
`scripts/build-app.sh` 或 `docs/`。這些位置與其他 Core 檔案中同時進行的修改都保留不動。

## Mac 記憶體與啟動

Release 建置指令：

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex3
```

兩次量測使用相同的完整 App 設定：
`--reranker data/build/reranker --rerank-count 16 --v3 data/build/v3
--v3-weight 0.5 --chat-prior 0.5 --timing`。
變更前：`--lexicon data/build/lexicon.tsv --chat-lm data/build/lm_chat`。
變更後：`--lexicon data/build/memory/binary/lexicon.tsv
--chat-lm data/build/memory/binary/lm_chat`。變更後的目錄只含二進位檔與詮釋資料；
能成功載入，就證明這些 URL 不需要 TSV。

這些是每個句子與設定各執行一次的短程序，不是重複量測的基準平均值。兩條路徑使用相同的
Core ML 模型與 CPU 後端。Mac 上沒有執行任何評估或訓練；只做了建置、這些短量測，以及明確
要求的 Core 檢查。

| 輸入 | TSV 目前／峰值用量 | 二進位目前／峰值用量 | TSV 引擎載入 | 二進位引擎載入 |
|---|---:|---:|---:|---:|
| 10 個音節 | 333 / 337 MB | 16 / 20 MB | 1253.931 ms | 111.032 ms |
| 25 個音節 | 339 / 349 MB | 22 / 32 MB | 1248.231 ms | 66.467 ms |

CLI 的 MB 單位為 1,048,576 位元組。依照要求，它在 Darwin 上的記憶體指標是
`rusage_info_v6.ri_phys_footprint`／`ri_lifetime_max_phys_footprint`。
乾淨的、以檔案為後盾的對應與堆積配置的計算方式不同；16–22 MB **不是常駐分頁總量，也不是
安裝後 App 圖形介面的用量**。三個對應的二進位檔合計 67,940,238 位元組（64.79 MiB），
模型權重、框架、UI 與既有的有界查詢快取都可能再增加分頁。本任務沒有重新建置、安裝或量測
已安裝的輸入法。

CLI 摘要輸出原文：

```text
Before, 10 syllables:
engine load: 1253.931 ms
  [keys 30: p50 15.3 ms, p95 33.6 ms, max 120.4 ms]
  [memory 333 MB now, 337 MB peak]
After, 10 syllables:
engine load: 111.032 ms
  [keys 30: p50 17.6 ms, p95 36.0 ms, max 111.9 ms]
  [memory 16 MB now, 20 MB peak]
Before, 25 syllables:
engine load: 1248.231 ms
  [keys 79: p50 34.5 ms, p95 57.1 ms, max 60.2 ms]
  [memory 339 MB now, 349 MB peak]
After, 25 syllables:
engine load: 66.467 ms
  [keys 79: p50 34.9 ms, p95 57.5 ms, max 64.3 ms]
  [memory 22 MB now, 32 MB peak]
```

輸入（不按 Enter，保留組字）：

- `今天晚上一起去吃飯吧`（10）：`rup<space>wu0<space>j03g;7u<space>fu3fm4t<space>z04187`
- `今天晚上我們一起去吃飯然後再去公園散步順便買一些水`（25）：`rup<space>wu0<space>j03g;7ji3ap7u<space>fu3fm4t<space>z04b06c.4y94fm4ej/<space>m06n041j4gjp41u04a93u<space>vu,<space>gjo3`

原始紀錄、指令陣列與模型詮釋資料都在 `data/build/memory/` 下。

## 行為檢查

Mac：**3568/3570 項檢查通過**，包括每一個新的二進位測試資料、個人覆蓋層、格式錯誤或被截斷
的詞庫、二進位與 TSV 混用的語言模型，以及 ASCII／OOV backoff 檢查。僅有的兩個失敗都是既有的
延遲斷言（`cold channel at 40 syllables under 3 ms`、`discovered LM aggressive
neutral-tolerant handle under 3 ms`）。完整輸出：`data/build/memory/mac-checks.log`。這套
檢查只執行一次。

Linux 的行為檢查與完整的遠端一致性結果記錄在下方。

## 建置產出

| 二進位檔 | 位元組 | SHA256 |
|---|---:|---|
| lexicon.bin | 17,012,566 | `17fa3b1af7739710eeb5cc6aa0ecbe6f5bcf5b5cdc07cf579e7019b7a53cbe16` |
| lm/lm.bin | 37,648,528 | `9d94cf72618525f54e482eb46a287cc6e1aee2ff807e350a3d2303dffa02d28e` |
| lm_chat/lm.bin | 13,279,144 | `2d41df7a0a9f8663f80febbf31d1e6ff4f719d5b7ff8d0094acd5c66d13f0d9e` |

詞庫有 124,227 個詞、105,061 個讀音與 127,617 個條目。兩個語言模型的詞彙分別包含 134,698
與 124,573 個寫法；混合模型只保留 538,792 位元組的跨詞彙 UInt32 ID。三個二進位檔的雜湊值
在 Mac 與 Linux 各自獨立編譯的結果之間都一致。Linux 上只用標準函式庫的編譯器測試通過
（測試資料結構、浮點分數、省略的 unigram、詮釋資料 SHA256，以及逐位元組相同的確定性重新
編譯）。Mac Swift：Apple Swift 6.4（`swiftlang-6.4.0.34.1`）；編譯器的 Python：既有的
3.15.0 `.venv`。沒有變更任何相依套件或工具鏈設定。

## 遠端驗證環境

獨立的主機工作區：`$ZHUYIN_REMOTE_HOST:~/zhuyintype/work/onnx-eval-memory`；
SSH 金鑰 `$ZHUYIN_REMOTE_KEY`。以符號連結沿用既有的評估 virtualenv；既有的 ONNX 匯出檔複製到
這個獨立工作區。不使用 GPU 後端，也不寫入訓練目錄。既有 `nlp/remote/run.sh` 的變更屬於另一個
工作，保留不動；它的同步清單不需要修改。編譯器與矩陣另外同步，再在遠端編譯二進位檔。

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-memory JOBS=60 \
  nlp/remote/run.sh sweep \
  --matrix data/build/memory/matrix.json \
  --output data/build/remote_eval/memory -n 400
```

矩陣比較 TSV 與二進位兩種設定，模型旗標與 App 要求的完全相同：
`--reranker data/build/reranker --rerank-count 16 --v3 data/build/v3
--v3-weight 0.5 --chat-prior 0.5`，以及對應的 `--chat-lm` 目錄。
二進位設定只覆蓋 `--lexicon` 與 `--chat-lm`；兩種評估設定都從原始的 TSV 詞庫推得相同的輸入按鍵。
測試集：dev_colloquial、dev_wikipedia、dev_chat_tt、維基百科 `--line-context`、口語
`--split continue`，各 `-n 400`，ONNX CPU，`JOBS=60`。

原始碼凍結：遠端執行腳本在呼叫當下複製可攜原始碼。之後同時進行的工作為 CompositionToken
加入第三個音節欄位，以及 CLI 的形式與邊界旗標。這些變更沒有被覆寫，也沒有重新同步到進行中
的驗證。mmap 讀取器、詞彙與混合模型的實作、編譯器與二進位測試資料都與本機原始碼逐位元組
相同；Engine 快照只差在兩處同時進行的音節模式元數變更。CLI 的引擎載入量測程式碼完全相同。
原始碼 SHA256 清單與兩份遠端原始碼快照都存放在 `data/build/memory/`。

執行腳本的準備：確認伺服器上已有全部三個檢查點的 SHA256 後，以最初檢查點傳輸自己的 rsync
PID 停止該傳輸。這些檢查點在伺服器上就地複製，run.sh 以
`ENGINE_SNAPSHOT=.build-memory-snapshot` 重新啟動，這是原始遠端可攜原始碼快照的複本。
第一次 Linux 建置暴露了既有 CLI Usage.swift 中 `getrusage(RUSAGE_SELF, ...)` 的列舉／型別不符。
同時作業的工程師在本機已有的修正（`__rusage_who_t(RUSAGE_SELF.rawValue)`）被複製到該快照中；
這裡沒有另外撰寫對 Usage.swift 的變更。建置與檢查紀錄保留了成功的重試。過程中不需要動到
同音字或 GPU 程序。

Linux Core 檢查：**3561/3568 通過，零個行為失敗**，使用實際記憶體對應的詞庫與二進位維基百科
語言模型。這套檢查在 `core-checks/` 中執行，其中的 `data/build/{lexicon.tsv,lexicon.bin,lm}`
只連結到凍結的 TSV 輸入與編譯後的二進位檔；TSV 一致性比對目錄沒有變更。那裡沒有選用的 lm2
基準測試，所以檢查項目比 Mac 少兩項。在共用主機的負載下，有七個既有的延遲門檻失敗：20 音節
8-best、cold channel、discovered LM neutral-tolerant（normal/aggressive）、合成的 150 MB 語言
模型 neutral-tolerant（normal/aggressive），以及合成語言模型 aggressive first-tone。所有
測試資料與真實資料的行為檢查都通過。實際記憶體對應的詞庫加語言模型初始化：215.035 ms；只有
詞庫：120.927 ms。紀錄：`data/build/memory/memory-linux-{build,checks}.log`。Linux Swift 6.4.1。

有一次啟動執行腳本時，run.sh 正好被同時修改，遇到 shell 語法錯誤。改為執行一份經過語法檢查、
從記憶體中一次讀入的腳本快照，保留原本的 `$0` 與引數行為。儲存的腳本是
`data/build/memory/runner-snapshot.sh`；沒有對 run.sh 做任何變更。

在成功完成量測與檢查的建置之後，最後一次的 Mac 重新建置遇到本任務範圍外、同時進行的新工作：
`Sources/ZhuyinML/RerankerTree.swift:41–42` 在初始化之前就透過 closure 擷取 `self.ancestors`。
這裡沒有變更該檔案。較早的 Mac release 建置、所有短記憶體量測，以及只執行一次的 Mac Core 檢查，
都是在加入 tree 之前的原始碼狀態下成功的；凍結的 Linux 建置也成功。見
`data/build/memory/mac-final-build.log`。目前共用的工作目錄需要由該程式碼的負責人先解決這個
建置錯誤，重新建置整個 App 才能通過。沒有嘗試改寫或還原同時進行的 reranker 實作。

凍結的 CLI 預設 `rerankWeight` 為 0.5。要求的指令沒有指定 `--rerank-weight`，所以兩次記憶體量測
與兩種一致性設定都使用這個預設值，並明確指定 `--rerank-count 16`、`--v3-weight 0.5` 與
`--chat-prior 0.5`。因此這裡的準確率百分比應在這些凍結的執行之間互相比較，不應與文件中使用
reranker 權重 1.0 的資料列比較。

## 最終一致性結果

**通過：2,000/2,000 行對齊的輸出完全相同（100%）。沒有任何差異。**
五個案例各自都超過要求的 99.5% 門檻。比對文字之前，先確認有序輸入完全相同，且每個案例恰好有
400 筆輸入與輸出。執行腳本與比對腳本都以零結束。

| 測試集 | 相同行數 | 一致性 | TSV／二進位準確率 | TSV／二進位秒數 |
|---|---:|---:|---:|---:|
| colloquial | 400/400 | 100.00% | 82.0% / 82.0% | 118.52 / 98.29 |
| wikipedia | 400/400 | 100.00% | 79.0% / 79.0% | 134.57 / 150.65 |
| chat | 400/400 | 100.00% | 82.8% / 82.8% | 90.82 / 73.32 |
| context | 400/400 | 100.00% | 83.0% / 83.0% | 367.88 / 251.56 |
| continue | 400/400 | 100.00% | 78.5% / 78.5% | 81.22 / 73.94 |

實際經過時間反映的是共用主機不斷變動的負載，不是受控的解碼速度比較。每一列的準確率都沒有
變化。沒有觀察到任何差異，包括 float32 平手時的順序翻轉。

產出物：有序的輸入與輸出在 `data/build/remote_eval/remote/memory/{tsv,binary}-*.json`（全部十列）；
`data/build/memory/parity.json` 記錄每個案例的計數與空的差異清單，`parity-summary.md` 是比對表。
比對腳本在 Linux 上執行；沒有使用 Mac 上的評估。完整的執行紀錄是
`data/build/memory/remote-run.log`。神經網路檢查點的雜湊值與伺服器上既有的複本一致；實際的 ONNX
匯出雜湊值與 Linux Python 3.12.15 記錄在 `data/build/memory/linux-model-sha256.txt`。最終的 mmap
原始碼與編譯器雜湊值在 Mac 與 Linux 之間一致，見 `final-{local,linux}-mmap-sha256.txt`。

## 交接摘要

| 指標 | TSV | 記憶體對應的二進位檔 |
|---|---:|---:|
| Mac 記憶體用量，10 個音節 | 333 MB（峰值 337） | 16 MB（峰值 20） |
| Mac 記憶體用量，25 個音節 | 339 MB（峰值 349） | 22 MB（峰值 32） |
| Mac 引擎載入，10 / 25 個音節 | 1253.931 / 1248.231 ms | 111.032 / 66.467 ms |
| 遠端輸出一致性，5 × 400 | 參考 | 2,000/2,000 相同 |

`phys_footprint` 是要求的指標；乾淨的對應分頁與堆積配置的計算方式不同。沒有量測總 RSS 與已安裝
輸入法的 UI。Mac 與 Linux 的行為檢查都通過；只與時間有關的失敗仍如上所列。App 打包與同時進行的
RerankerTree 建置錯誤不在這次變更的範圍內。

# 可攜引擎與遠端評測報告

2026-10-27（Asia/Taipei）。CPU／GPU 評測路徑、模型一致性、兩組完整的 15 列平台比較，以及遠端 runner 的兩種模式都已驗證。已知的延遲失敗明確保留在報告中。

這份紀錄寫於 2026-10-27，當時的工具是 `run.sh`、`evaluate.py`、`sweep.py`、`compare.py`、`manifest.py` 與 `gpu_benchmark.sh`。這些工具後來由 `zt --remote`、`zt eval`、`zt models` 取代（對應見 [README.md](README.md)）；以下保留當時的指令與數字作為紀錄。

## 起始狀態與範圍

工作目錄中原本已有同時進行中的引擎與 App 變更、條件式的 Package.swift／Core ML import、預測注入用的初始化函式、一個 Python GPU socket 後端，以及一個執行中的 Mac 矩陣。這些都保留原狀。這項工作沒有 commit、reset、stash，也沒有編輯 App 原始碼、文件、建置 App 的腳本或其他訓練目錄。隔離的遠端目的地是 `~/zhuyintype/work/onnx-eval`；同音字工作區與其 GPU 行程都未被觸碰。

全新基準：`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex2` 通過，接著檢查回報 **3072/3083，11 項僅延遲失敗**，而不是預期的六項。完整記錄：`data/build/remote_eval/baseline/checks.log`。這項工作沒有改動核心原始碼。延遲失敗的數量會隨同時進行的評測負載變動；行為上的退化必須與這些時間門檻區分開來。

遠端：Ubuntu 26.04.1 x86_64；Swift 6.4.1；PyTorch 2.13.0+cu130；64 個 CPU／約 125 GB RAM。起始時 GPU 使用量為 46587 MiB 中的 43549 MiB，由既有的 VLLM 行程占用。CPU 推論是基準，不使用 GPU 記憶體。

## 安裝

沒有修改 Mac 環境、全域執行環境預設值或訓練環境。

* Swift **6.4.1**，先前的嘗試已安裝到 `~/opt/swift-6.4.1-RELEASE-ubuntu26.04`，並有 `~/opt/swift` 符號連結。官方來源：`https://download.swift.org/swift-6.4.1-release/ubuntu2604/swift-6.4.1-RELEASE/swift-6.4.1-RELEASE-ubuntu26.04.tar.gz`。當時的安裝輸出在 `nlp/remote/results/swift-install.log`；這次執行驗證了 `~/opt/swift/usr/bin/swift --version`。沒有重新安裝。
* ONNX Runtime C SDK **1.24.4**，Microsoft 官方發行版：`curl -fL https://github.com/microsoft/onnxruntime/releases/download/v1.24.4/onnxruntime-linux-x64-1.24.4.tgz -o ~/opt/onnxruntime-linux-x64-1.24.4.tgz`；`tar -xzf ~/opt/onnxruntime-linux-x64-1.24.4.tgz -C ~/opt`；`ln -sfn onnxruntime-linux-x64-1.24.4 ~/opt/onnxruntime`。
* 隔離的 Python **3.12.15** 環境，使用既有的 uv **0.13.5**：`~/.local/bin/uv venv ~/zhuyintype/work/onnx-eval/.venv --python ~/zhuyintype/.venv-gen/bin/python`。`training-torch.pth` 指向既有的 `.venv-gen/lib/python3.12/site-packages`，以重用 Torch 而不修改它。
* PyPI 套件：`~/.local/bin/uv pip install --python ~/zhuyintype/work/onnx-eval/.venv/bin/python onnx==1.20.1 onnxruntime==1.24.4 onnxconverter-common==1.16.0`。解析出的相依套件：flatbuffers 25.12.19、ml-dtypes 0.6.0、mpmath 1.3.0、numpy 2.5.4、packaging 26.3、protobuf 7.36.2、sympy 1.14.0、typing-extensions 4.16.0。來源：PyPI。不需要 sudo 或系統層級安裝。

## 實作與驗證

以下為實測結果與限制；原始產出物在 data/build/remote_eval/ 下。

### 第一批已驗證的結果

* Mac 全產品 release 建置通過。Linux `swift build -c release` 通過，ZhuyinCore 不需要任何相容性修改。兩個平台都通過兩項注入後端的 XCTest 檢查（reranker 的編碼／上下文／padding／OOV／批次／exactMargin；音節的上下文／換行／候選截斷／對應／輸出驗證）。三項 Python 轉接層回歸檢查通過。
* `manifest-mac.json` 與 `manifest-linux.json` 對全部 **43** 個凍結的原始碼／資料檔案都相符。Mac 快照：`data/build/remote_eval/frozen`；遠端快照：`~/zhuyintype/work/onnx-eval`。manifest 排除 AppleDouble 封存中繼資料與無關的 v3 冒煙測試模型。完整的測試向量輸入、詞彙 JSON、檢查點、評測集與 LM 檔案都包含在內。
* fp32 ONNX 對 PyTorch：在全部 20 個既有黃金向量上，v3 有效 logprob 的最大絕對誤差為 **0.0000181198**。對 CPU Core ML 的最大誤差為 **0.0729847**，在既有的 PyTorch 對 fp16 容許誤差 0.1 之內；向量中有個別接近平手的 top-1 差異。reranker 在真實句子／長上下文／單字／OOV 輸入上與隨附的 Mac 模型相符：ONNX 對 Core ML 的最大 logprob 誤差為 **0.0675192**（以輸出驗證是 v2b，而非依目錄名稱）。測試過的 reranker 動態形狀：(B,L)=(1,2),(3,7),(16,48),(32,64)，對 Torch 的最大誤差為 **0.0000448227**。
* 也匯出並測試了 fp16：v3 logprob 對 fp32 Torch 的最大誤差為 **0.00740290**，對 Core ML 的最大誤差為 **0.0734596**。基準仍是 fp32。第一次以 onnxconverter-common 轉換時，它拒絕原本就有的 fp16 記憶邊界；強制轉換則產生型別混用的 Mul 圖。後來改為直接以 PyTorch 半精度權重匯出，保留 int32 輸入、fp16 K/V 記憶與 fp32 logprob。最終的匯出檔可以載入，並通過所有一致性檢查；轉換器相依套件已不再需要（其安裝紀錄仍保留在上方）。
* 先前嘗試留下、已放棄的 Mac 矩陣仍在 `nlp/remote/results/frozen` 下執行。確認其 PID／cwd 與子行程後，只停止該矩陣（PID 72887）及其 worker。它已完成的產出物都保留。其他工程師的評測與 GPU 訓練服務持續執行。實際耗時是在共用機器負載下的量測，不是隔離硬體的效能測試。
* 在較重負載下最後一次執行核心檢查，回報 **3070/3083，13 項失敗**，全部僅與延遲有關；最初為 **3072/3083，11 項失敗**。檢查總數與行為上的通過數不變；延遲失敗數在變動的負載下並不相同。完整記錄照實保留，沒有把時間失敗重新標為通過。
* 一次轉接層冒煙測試發現 argparse 的縮寫機制把 `--cli` 當成 `--cli-arg`；以 `allow_abbrev=False` 修正這個根本原因，沒有改動任何既有評測程式。修正後重新執行了遠端冒煙測試。

### 中間的整體引擎檢查與傳輸

五列無模型的結果在 Mac／Linux 上完全相同（每列 0/400 筆輸出差異）。第一批 reranker 列在口語上有 1/400、Wikipedia 上有 2/400 筆預測不同；chat 為 0/400。在評測程式回報的精度下，準確率一致。這些列都低於 1% 的目標；驗收標準仍以最終矩陣為準。

有兩次長時間的 rsync 傳輸被 SSH 對端重設（一次是初次的檢查點同步，一次是 ONNX 取回）。評測前另外驗證了遠端完整的檢查點副本與原始碼／資料 manifest。之後的小型傳輸都成功。runner 加入了 keepalive 選項；ONNX 取回改用可續傳的部分傳輸。這沒有中斷遠端評測行程，也沒有影響訓練工作區。

最終的轉接層測試組現在有 **四** 項測試，包含針對 argparse 縮寫問題的端到端假 CLI 回歸測試。原本的評測腳本都未修改。`error_analysis`、`typing_errors` 與 `typo_eval` 已在遠端以獨立批次和四個 worker 做冒煙測試；400 次執行的矩陣使用 60 個 worker。

### 已完成的 Mac／Linux CPU 驗證

完整的笛卡兒積矩陣是 **15 列 x 400 次 = 每個平台 6,000 筆預測**，涵蓋三個資料集在無模型、reranker（count 16、weight 1）、reranker + v3（weight 1）下的結果，以及每種設定的 Wikipedia `--line-context`／口語 `--split continue` 變體。`compare.py` 先確認依序的 CLI 輸入完全相同，再比較輸出。每一列都達到 <=1% 的目標；最差的一列為 **3/400 = 0.75%**。原始預測、評測程式輸出與實際耗時在 `data/build/remote_eval/{mac,remote/linux}/` 下；合併表格為 `data/build/remote_eval/comparison.md`。

實際耗時包含評測程式準備、CLI／模型啟動與打字，Mac 使用 **8** 個 worker，Linux 使用 **60** 個。共用機器負載與啟動成本都有影響：Linux 的 v3 接續組字列略慢於 Mac。這裡不宣稱任何隔離硬體下的加速。

最初的 GPU 效能測試碰到既有 socket 實作在 Linux Foundation 上 `read(upToCount:)` 的行為：串流仍開著時，短回覆會一直阻塞到逾時。現在有一個只在 Linux 上使用的 `recv` 分支，會讀取已到達的片段，macOS 原本的讀取路徑保持不變。回歸測試伺服器刻意讓 socket 保持開啟；**八個短回覆通過**，4 次執行的真實 GPU 冒煙測試以 100% 整段準確率通過。修正後，完整 GPU 實驗以相同的凍結引擎／資料／模型權重重新執行一次。其 2 GiB 配置上限有實際生效；當時觀察到的 GPU 行程記憶體約 1.1 GiB。

CPU 驗證時原本完整的 43 檔 manifest 都保留。凍結之後唯一的原始碼變更，是 `ModelServer.swift` 中只在 Linux 上的選用 socket 讀取；ONNX CPU 路徑從不呼叫它，macOS 上也不會編譯進去。最終 manifest 另外記錄；矩陣凍結後沒有更動任何引擎或計分器邏輯。加入該條件判斷後，Mac 全產品 release 建置再次通過。

完整的 `run.sh` 整合測試（12 次執行、12 個 worker、reranker）通過：同步、Linux release 建置、依內容快取的 ONNX 匯出、評測與自動取回。結果：整段準確率 **91.7%**，12/12 行輸出，評測程式輸出格式與原本相同。

| 設定 | 資料集 | Mac 準確率 | Linux 準確率 | 不同／執行次數 | Mac 秒數 | Linux 秒數 |
|---|---|---:|---:|---:|---:|---:|
| plain | colloquial | 75.5% | 75.5% | 0/400 | 27.28 | 5.08 |
| plain | wikipedia | 71.2% | 71.2% | 0/400 | 21.87 | 4.98 |
| plain | chat | 73.0% | 73.0% | 0/400 | 26.74 | 4.11 |
| plain | context | 71.5% | 71.5% | 0/400 | 35.45 | 4.40 |
| plain | continue | 70.5% | 70.5% | 0/400 | 18.46 | 3.84 |
| reranker | colloquial | 85.0% | 85.0% | 1/400 | 197.37 | 30.55 |
| reranker | wikipedia | 81.5% | 81.5% | 2/400 | 90.63 | 42.31 |
| reranker | chat | 67.8% | 67.8% | 0/400 | 49.76 | 29.45 |
| reranker | context | 84.0% | 84.2% | 3/400 | 181.39 | 115.54 |
| reranker | continue | 79.2% | 79.2% | 1/400 | 46.72 | 25.79 |
| v3 | colloquial | 85.2% | 85.2% | 0/400 | 64.69 | 49.36 |
| v3 | wikipedia | 81.0% | 81.2% | 1/400 | 100.91 | 66.58 |
| v3 | chat | 71.8% | 71.8% | 1/400 | 60.22 | 46.81 |
| v3 | context | 85.2% | 85.5% | 1/400 | 214.08 | 139.71 |
| v3 | continue | 81.0% | 81.0% | 0/400 | 38.76 | 41.62 |

### 最終原始碼檢查與產出物

最終的原始碼／資料 manifest 在兩個平台間仍全部 43 個檔案相符（`manifest-final-{mac,linux}.json`）。它們與原本矩陣 manifest 的唯一差異，是選用的 Linux socket 傳輸條件判斷。取回到本機的基準 ONNX 模型與 v3 fp16 實驗模型，其 SHA256 雜湊值與遠端匯出檔相同；雜湊值保存在 `data/build/remote_eval/model-sha256.txt`。

Mac 矩陣結束後，檢查回報 **3076/3083，七項僅延遲失敗**，全部屬於全新基準那 11 項失敗的子集。**沒有新的行為失敗**。中間那次負載較重的檢查有 13 項時間失敗。在變動的共用負載下，無法重現要求的完全相同時間失敗數；沒有放寬任何效能門檻或測試。最終記錄：`data/build/remote_eval/checks-after-matrix.log`。

完整實測 CPU 矩陣總計：**Mac 1174.31 s；Linux 610.12 s**（在觀察到的共用負載下，這個完整矩陣快 1.92x）。6,000 筆輸入全部完全對齊；**10/6,000 筆輸出預測不同**，最差的單一列為 3/400。這是平台之間的比較，不代表 macOS 後端變慢：Mac 保留 Core ML 與既有的推論路徑。

Linux 核心檢查也執行了：**3075/3081，六項僅延遲失敗**，沒有回報行為失敗。少了兩項針對選用 Mac `data/build/lm2` fixture 的檢查，因為這個未隨附的模型不在遠端同步範圍內。既有的時間目標都沒有更動。

已驗證連結：Mac CLI 連結 CoreML.framework，沒有 ONNX 相依；Linux CLI 從 `~/opt/onnxruntime/lib/` 解析 `libonnxruntime.so.1`。全部六個匯出的 ONNX 檔案（三個 fp32、三個 fp16）現在都已取回，放在 Mac 模型 JSON 旁邊，並與遠端 SHA256 雜湊值相符。

執行環境參考資料：[ONNX Runtime C API](https://onnxruntime.ai/docs/get-started/with-c.html)、[執行緒管理](https://onnxruntime.ai/docs/performance/tune-performance/threading.html)、[官方 1.24.4 SDK 發行版](https://github.com/microsoft/onnxruntime/releases/tag/v1.24.4)。以上的實測結果來自這個工作區的擷取，不是來自這些參考頁面。

### 已完成的 GPU 比較與採用的選項

socket 修正後，完整的 15 列 GPU 矩陣耗時 **393.13 s**，ONNX CPU 為 **610.12 s**：**快 1.55x／各列實際耗時少 35.6%**。全部 **6,000 筆 GPU 預測與 CPU 預測相同**，因此與 Mac 的差異和準確率都與上方 CPU 表格一致。原始 GPU 預測與計時在 `data/build/remote_eval/remote/gpu/`；`comparison-gpu.md` 保留 Mac／GPU 表格。GPU 使用 fp32 並停用 TF32，不是 bf16。選用的伺服器使用 PyTorch 檢查點推論；原生的每行程 Linux 模型推論則是透過 C API 使用 ONNX Runtime。

這樣的改善足以在 run.sh 中為大型遠端 sweep 提供 **`BACKEND=cuda`**。**`BACKEND=cpu` 仍是預設值**，不需要 GPU。兩種模式建置同一套 Swift 引擎／CLI。CUDA 模式擁有私有的 socket／伺服器，Torch 配置上限 2 GiB，會傳遞模型與伺服器錯誤，並在結束時只終止自己的伺服器。觀察到的 GPU 行程使用量約 **0.8–1.2 GiB**，與未變動的既有 VLLM 行程（約 43.5 GiB）並存。不需要安裝 ONNX CUDA SDK 或驅動程式。

矩陣總計是各列評測程式實際耗時的加總；共用 GPU 伺服器一次性的啟動與清理不計入（其 Python 初始化／import／載入只需數秒，不分攤到每一列）。CPU 計時包含每個 CLI worker 的模型啟動。原始碼同步、建置與 ONNX 匯出在兩者的穩定評測總計中都不計入。所有計時都在實驗室有其他工作同時進行時量測。

| 設定 | 資料集 | CPU 秒數 | GPU 秒數 |
|---|---|---:|---:|
| plain | colloquial | 5.08 | 4.05 |
| plain | wikipedia | 4.98 | 4.46 |
| plain | chat | 4.11 | 3.93 |
| plain | context | 4.40 | 4.52 |
| plain | continue | 3.84 | 3.77 |
| reranker | colloquial | 30.55 | 21.59 |
| reranker | wikipedia | 42.31 | 26.37 |
| reranker | chat | 29.45 | 21.02 |
| reranker | context | 115.54 | 38.25 |
| reranker | continue | 25.79 | 18.41 |
| v3 | colloquial | 49.36 | 35.27 |
| v3 | wikipedia | 66.58 | 51.25 |
| v3 | chat | 46.81 | 35.18 |
| v3 | context | 139.71 | 95.22 |
| v3 | continue | 41.62 | 29.83 |

### runner 最終驗證與交接

`BACKEND=cuda JOBS=4 nlp/remote/run.sh sweep --matrix nlp/remote/validation.json -n 4 --output data/build/remote_eval/runner-gpu` 通過全部 **15** 列小型整合測試。這驗證了新的全域 socket 旗標經過 sweep.py 的傳遞、遠端伺服器啟動、相同的 line-context／split 變體、包含測試 target 的原始碼同步、建置與匯出快取的重用、JSON／表格取回，以及結束時的清理。先前的 ONNX CPU run.sh 整合測試也已通過。最終的 Python 回歸測試組為 **4/4**，計分器測試在每個作業系統上為 **2/2**，socket 回歸測試為 **8 個請求**。Shell 語法檢查通過。

這項工作啟動的評測伺服器在結束時都已停止。最後的 GPU 行程清單只剩原本的 VLLM PID 262190，使用 43547 MiB。沒有嘗試原生 Linux／Windows 輸入法整合、C ABI 實作、DirectML／OpenVINO provider、模型量化或安裝 App；這些項目的範圍與封裝計畫見 PORTING.md。這項工作沒有更動既有的 macOS Core ML 執行環境與 App／前端檔案。工作目錄中其他無關的未提交變更都保持原狀。沒有執行 git commit／stash／reset／checkout。

驗收摘要：可攜 release 建置通過；fp32／fp16 模型檢查通過；全部 15 列 CPU 與 15 列 GPU 對 Mac 的比較都達到 1% 輸出差異目標；遠端 runner 兩種模式都通過整合測試；剩下的核心檢查失敗都是共用負載下原本就有的延遲門檻（Mac 最終 7 項，全新基準 11 項；Linux 6 項）。失敗數照實測回報，不宣稱在負載下完全穩定。

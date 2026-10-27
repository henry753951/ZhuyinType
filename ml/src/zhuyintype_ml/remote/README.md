# ml/src/zhuyintype_ml/remote

在 GPU 主機上執行任何 zt 指令，以及 Linux 上的模型執行環境：ONNX 匯出、一致性檢查與 CUDA 模型伺服器。

## 用途

評測與訓練在 GPU 主機上執行，不佔用開發用的 Mac。`zt --remote <指令>` 把目前的工作目錄同步到主機上專屬的目錄，在那裡準備環境與資料，執行同一個 zt 指令，把輸出即時傳回，最後取回結果檔。程式裡不寫主機、使用者目錄或 SSH 金鑰；主機設定只放在不進 git 的 `.remote.env`，也可以放在環境變數。

Mac 用 Core ML 執行模型。Linux 用同一套 Rust 引擎與判斷邏輯，模型有兩種執行方式：

- `cuda`：GPU 上的一個 PyTorch 模型伺服器替所有引擎行程計算（`zhuyin-cli --model-server`），是 `zt --remote eval` 的預設，用在開發時的評測與設定掃描；
- `cpu`：每個引擎行程透過 ONNX Runtime 執行從檢查點匯出的 ONNX 模型（`zhuyin-cli` 的 `onnx` feature），用在發布的數字（ZhuyinBench、與其他輸入法的比較），以 `--backend cpu` 指定。

兩種方式的輸出在開發集上逐行相同；和 Mac 的一致性與耗時見下方「實測結果」與 [REPORT.md](REPORT.md)。

## 指令

```sh
cp .remote.env.example .remote.env                 # 填入 ZHUYIN_REMOTE_HOST（與 ZHUYIN_REMOTE_KEY）
tools/zt --remote setup                            # 檢查主機的工具、建立 .venv
tools/zt --remote eval conversion --config engine -n 3 --set smoke
tools/zt --remote eval conversion --set colloquial-dev wiki-dev --json data/build/eval/app.json
tools/zt --remote eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json
tools/zt --remote --backend cpu --export eval conversion --set dev    # ONNX Runtime，發布數字用的方式
tools/zt --remote --push data/build/v3_new --detach train v3
tools/zt remote status                             # 主機負載、執行中的工作、記錄的最後幾行
tools/zt remote logs -f                            # 持續顯示記錄
tools/zt remote pull                               # 取回 data/build/remote_eval/
tools/zt remote stop                               # 依 PID 停止背景工作
```

`zt --remote` 的選項寫在指令之前：

| 選項 | 說明 |
|---|---|
| `--workdir NAME` | 主機上 `<base>/work/NAME`；預設每個 checkout 一個（目錄名稱加路徑雜湊值） |
| `--push PATH` | 另外傳送 checkout 內的檔案或目錄，例如本機訓練的模型（可重複） |
| `--pull PATH` | 執行後另外取回的路徑，相對於 checkout（可重複） |
| `--fetch COMPONENTS` | 先下載的資料元件，以逗號分隔；`eval`、`bench`、`models`、`train` 預設 `eval` 群組，其他指令預設不下載；`none` 不下載 |
| `--export` | 先執行 `zt models export --tree`（依內容快取）；`--backend cpu` 的 `--config app` 評測需要 ONNX 模型 |
| `--backend cpu\|cuda` | `cuda`：所有模型呼叫交給 GPU 上的 PyTorch 伺服器，`zt eval` 的預設（`stats`、`sets`、`latency` 除外）；`cpu`：每個引擎行程用 ONNX Runtime，其他指令的預設，ZhuyinBench 也是 |
| `--group GROUP` | 在主機上另外安裝相依群組 |
| `--env KEY=VALUE` | 執行時的環境變數，例如 `ZT_BUILD_JOBS=16` 限制建置使用的核心數 |
| `--detach` | 以 `nohup` 在背景執行，指令立刻返回 |
| `--nice N` | 以較低的優先權執行（0–19）；沒有人在等的長時間工作用 10 |

模型相關的指令也可以直接在 Linux 主機上執行：

```sh
zt models export [--tree] [--fp16] [--force]       # 依檢查點、匯出程式與選項的雜湊值快取
zt models export --reranker reranker_v2c --v3 v3_new
zt models parity --coreml                          # Mac：Core ML 的參考輸出
zt models parity                                   # Linux：ONNX 對 PyTorch 與 Core ML
zt models serve --device cuda -- eval conversion   # 指令裡所有 zhuyin-cli 行程經由 GPU 伺服器計算
zt data compile --quiet                            # 詞庫與語言模型的 mmap 檔，只重編過期的
```

## 一次遠端執行的流程

1. 確認沒有其他執行占用這個工作目錄（主機上的 `flock`）。被占用時直接結束，避免同步覆蓋執行中的檔案。
2. 同步原始碼：`git ls-files` 列出的追蹤檔案與未被忽略的新檔案，加上 `--push` 的路徑。上一次同步過、這次已不存在的檔案會被刪除；資料、`.venv` 與其他檔案不受影響。
3. 在主機上依序執行 `tools/zt setup --quiet`、`tools/zt data fetch`（從固定版本的 release 下載，驗證雜湊值；壓縮檔快取在 `<base>/asset-cache`，各工作目錄共用）、`eval` 與 `bench` 再執行 `tools/zt data compile --quiet`（詞庫與語言模型編成 App 載入的 mmap 檔，來源沒變時略過）、需要時 `tools/zt models export --tree`，最後執行指令本身；`cuda` 時由 `tools/zt models serve` 啟動模型伺服器再執行指令。zhuyin-cli 在第一次用到時建置。輸出即時傳回，同時寫入 `<base>/logs/<工作目錄>-<時間>.log`。
4. 取回：指令的 `--json`、`--out`、`--output` 檔案（checkout 外的路徑先寫到主機的 `data/build/remote_eval/out/`）、主機上的 `data/build/remote_eval/`（放到本機 `data/build/remote_eval/<工作目錄>/`）與 `--pull` 的路徑。指令失敗時也會取回。

`--detach` 的工作以 `setsid` 執行並記錄 PID；`zt remote stop` 只停止這個 PID 的行程群組。`ZHUYIN_REMOTE_HOST=local` 時，同樣的步驟在本機執行（例如在 GPU 主機上直接使用）。

## 引擎行程池與模型伺服器

評測把每一段輸入交給 `zhuyin-cli --batch` 行程（`core/engine.py` 的 `EnginePool`）。行程數預設等於核心數（`--jobs` 可改），每個行程做完一段就拿下一段，所以少數長句不會讓其他核心在最後閒著；`zt eval typo` 把所有錯誤比例與容錯模式一次交給同一組行程。同一次 `zt eval` 的各個評測集共用這些行程，每個行程只載入一次模型。這需要 zhuyin-cli 在 `--batch` 下每寫完一行就送出（Rust 的 stdout 本來就逐行送出；替代用的假 CLI 要 `flush`）。

`cuda` 時，`zt models serve` 在主機上啟動 `zhuyintype_ml.remote.model_server`，再以一個加上 `--model-server SOCKET` 的 zhuyin-cli 包裝程式（`ZT_CLI`）執行指令，結束時只停止自己的伺服器，並在輸出最後印出伺服器的計數。引擎行程只做解碼，模型都在 GPU 上算：

- 一個執行緒服務所有連線。它讀進所有已到達的請求，把全部 reranker 請求合成一批、全部 v3 請求合成另一批，算完一起回覆；GPU 在算的時候，下一批請求在 socket 裡累積，所以批次隨引擎行程數變大，不需要等待計時。
- reranker 的每個請求是同一段前文後面的 N 個候選句，彼此共用大部分的字。伺服器把它們整理成前綴樹（`RerankerTree` 的做法），同一輪的樹排成一條序列，每個節點只看自己的祖先，實際要算的字約是原本的四分之一。補齊用的位置回覆 0。
- v3 的請求依形狀補齊到固定的大小（補上的音節、記憶與候選字都被遮蔽），前文的記憶依前文快取。
- 每種形狀第一次出現時把整個前向計算錄成 CUDA graph，之後一次啟動就重播，不必由 Python 逐一送出幾百個小 kernel。
- fp32，停用 TF32，和 ONNX 模型相同；GPU 記憶體上限預設每個 worker 12 GiB（`--memory-gb`）。`--workers N` 可以開多個伺服器行程共用同一個 socket，但它們分時使用 GPU，實測幾乎沒有更快。
- 伺服器每分鐘與結束時在 `remote_eval/model-server.log` 寫一行計數：各模型的請求數、平均批次大小、忙碌的時間比例。

伺服器的協定有兩種格式，看每個請求的第一個位元組分辨：JSON lines（舊格式，`{` 開頭）與二進位框架（`0x00` 開頭，zhuyin-cli 使用）。框架是 8 位元組的標頭（標記 0、種類、版本 1、內容長度）加上 little-endian 的 int32 內容，回覆是 float32；格式寫在 `model_server.py` 的開頭。

原生的 Linux 推論仍是 ONNX CPU；GPU 伺服器只是評測用的加速。

## 主機環境

`zt setup` 檢查並說明缺少的工具，不安裝系統套件，也不需要 sudo：

- uv：`curl -LsSf https://astral.sh/uv/install.sh | sh`（安裝到 `~/.local/bin`，`tools/zt` 會在那裡找到它）。Python 3.12 由 uv 提供。
- Rust：rustup 或系統的 cargo（PATH 上，或 `~/.cargo/bin/cargo`，或以 `ZT_CARGO` 指定）。
- ONNX Runtime 1.24.4 C SDK：`zt setup --install-onnxruntime` 下載官方發行版到 `~/opt` 並建立 `~/opt/onnxruntime` 連結；也可以用 `ONNXRUNTIME_ROOT` 指定其他位置（其下要有 `include/` 與 `lib/`）。`zt` 在 Linux 上以 `onnx` feature 建置 `zhuyin-cli`，連結 SDK 的 `lib/`（`ORT_LIB_PATH`）並嵌入它的路徑。
- PyTorch、onnx、onnxruntime（Python）：`export` 相依群組，第一次執行 `zt models export` 時由 uv 安裝；Linux 上是 CUDA 13.0 的 PyTorch。

主機上的目錄（`<base>` 預設是家目錄下的 `zhuyintype`，可用 `ZHUYIN_REMOTE_BASE` 改變）：

| 路徑 | 內容 |
|---|---|
| `<base>/work/<名稱>/` | 一個 checkout 的原始碼、`.venv`、`.build`、`data/build`；`.zt/` 放同步清單、工作腳本、PID、最後的結束狀態與記錄連結 |
| `<base>/logs/` | 每次執行的記錄 |
| `<base>/asset-cache/` | 資料壓縮檔，所有工作目錄共用 |

在共用的主機上：長時間的工作用 `--detach`；需要時用 `--jobs`、`--env ZT_BUILD_JOBS=N` 限制核心數；不同的實驗用不同的 `--workdir`；只依記錄的 PID 停止行程。

## 實測結果

2026-10-27 在 GPU 主機（Ubuntu 26.04.1 x86_64，64 核心、約 125 GB 記憶體，Swift 6.4.1，PyTorch 2.13.0+cu130）與開發用的 Mac 上量測，當時有其他工作同時執行。完整過程見 [REPORT.md](REPORT.md)。

- 模型一致性：fp32 ONNX 對 PyTorch，v3 有效 logprob 的最大絕對誤差為 0.0000181（20 個測試向量），對 CPU Core ML 為 0.0730，在 fp16 容許誤差 0.1 之內。reranker 在真實句子、長上下文、單字與未知字上對 Core ML 的最大誤差為 0.0675；動態形狀 (1,2)、(3,7)、(16,48)、(32,64) 對 PyTorch 的最大誤差為 0.0000448。fp16 實驗模型對 fp32 PyTorch 為 0.0074，基準仍是 fp32。
- Mac 與 Linux 的轉換結果：15 列 × 400 次（三個開發集，無模型、reranker、reranker + v3，另加 line-context 與 continue 變體），6,000 筆輸入完全對齊，10 筆輸出不同，最多的一列為 3/400（0.75%），每一列都在 1% 的目標內。無模型的五列完全相同。
- 耗時（各列實際耗時加總）：Mac 8 個 worker 1174.31 s；Linux ONNX CPU 60 個 worker 610.12 s；Linux CUDA 伺服器 393.13 s。CUDA 的 6,000 筆預測與 CPU 完全相同；伺服器的 GPU 記憶體約 0.8–1.2 GiB。
- 引擎檢查：Linux 3075/3081 通過，失敗的 6 項都是共用負載下的延遲門檻，沒有行為上的失敗。
- 連結：Mac 的 CLI 連結 CoreML.framework、不依賴 ONNX；Linux 的 CLI 從 `~/opt/onnxruntime/lib/` 載入 `libonnxruntime.so.1`。
- 2026-10-28 的評測加速（Rust 引擎）：App 設定的開發集轉換從 60 s 到 13 s（`--backend cpu` 46 s），打字錯誤從 239 s 到 32 s，驗證矩陣從 178 s 到 29 s；GPU 與 CPU 的輸出逐段相同。量測與改動見 [REPORT.md](REPORT.md) 的「評測加速」。
- Linux Foundation 的 `read(upToCount:)` 在 socket 保持開啟時會等到逾時，`ModelServer.swift` 因此在 Linux 上改用 `recv` 讀取已到達的資料；`test_socket.py` 是這個問題的回歸測試。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build`）。

- `zt models export`：讀 `reranker/charlm.pt`、`v3/context.pt`、`v3/syllable.pt`，寫出 `reranker/Reranker.onnx`（`--tree` 另有 `RerankerTree.onnx`）、`v3/ContextEncoder.onnx`、`v3/SyllableEncoder.onnx`、`v3/onnx-export.json`（版本、雜湊值、動態形狀的誤差），以及快取用的 `reranker/onnx-export.sha256`。
- `zt models parity`：讀 `v3/test_vectors.json` 與 `remote_eval/reranker-coreml.json`（Mac 上以 `--coreml` 產生，再用 `--push` 傳到主機），寫出 `remote_eval/parity.json`。
- `zt models serve`：記錄寫在 `remote_eval/model-server.log`。
- `zt --remote`：見上方「一次遠端執行的流程」。

## 檔案

| 檔案 | 說明 |
|---|---|
| `runner.py` | `Runner` 介面與 `LocalRunner`、`RemoteRunner`：同步、執行、取回、狀態、記錄、停止 |
| `host.py` | 主機設定（`.remote.env` 與環境變數）、ssh 與 rsync 指令、主機上的路徑 |
| `models.py` | ONNX 匯出的快取、評測前的準備（`prepare`）、啟動模型伺服器 |
| `export_onnx.py` | 從檢查點匯出 ONNX（`export` 相依群組） |
| `parity.py` | ONNX 對 PyTorch 與 Core ML 的一致性 |
| `model_server.py` | GPU 上的 PyTorch 批次推論伺服器（Unix socket；JSON lines 與二進位框架） |
| `test_runner.py`、`test_socket.py`、`test_model_server.py` | 執行步驟、輸出路徑、主機設定的單元測試；socket 短回覆的回歸測試；伺服器的兩種格式、前綴樹與補齊大小（需要 `export` 群組） |
| `REPORT.md` | 可攜引擎與遠端評測的實測紀錄 |
| `PORTING.md` | 平台邊界、未來的 C ABI 與各平台的封裝計畫 |

舊工具與現在的對應：`run.sh` → `zt --remote`；`evaluate.py`、`capture_eval.py`、`sweep.py` → `zt eval`（`ml/src/zhuyintype_ml/eval`，引擎行程池已內建平行處理與逐筆輸出）；`matrix.sh`、`gpu_benchmark.sh` → `zt eval sweep`（預設就在 GPU 上）；`compare.py` → `zt eval stats`；`manifest.py` → 以 `uv.lock`、`assets.json` 與 git commit 固定版本；`BACKEND=cuda` → `--backend cuda`；`TREE_RERANKER=1` → `zt models export --tree`。

## 相關文件

- [REPORT.md](REPORT.md)：實測紀錄
- [PORTING.md](PORTING.md)：平台邊界與未來的輸入法
- [tools/README.md](../../../../tools/README.md)：所有 zt 指令
- [docs/ml/EVALUATION.md](../../../../docs/ml/EVALUATION.md)：評測

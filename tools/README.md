# tools

`tools/zt` 是 ZhuyinType 所有開發工具的進入點：資料、建置、檢查、評測、訓練與 GPU 主機。

## 用途

同一個指令在 Mac、Linux 主機與 CI 上的寫法完全相同。`zt` 用 uv 在版本庫根目錄的 `.venv` 執行 `python -m zhuyintype_ml.cli`，可以從任何目錄呼叫，參數原樣傳下去。任何指令前加 `--remote` 就改在 GPU 主機上執行，主機設定只放在不進 git 的 `.remote.env`。

第一次使用：

```sh
curl -LsSf https://astral.sh/uv/install.sh | sh    # 或 brew install uv；uv 會自動準備 Python 3.12
tools/zt setup                                      # 檢查 Swift、Xcode（macOS）或 ONNX Runtime（Linux），建立 .venv
tools/zt data fetch                                 # 下載資料與模型到 data/build
```

`zt` 本身只用標準函式庫。需要第三方套件的步驟（PyTorch、ONNX、wordfreq、ruff、mypy）以 `pyproject.toml` 的相依群組執行，第一次用到時自動安裝，版本由 `uv.lock` 固定：

| 群組 | 內容 | 用在 |
|---|---|---|
| `dev` | ruff、mypy | `zt check lint`、`zt check types` |
| `data` | wordfreq | `zt data lexicon` |
| `export` | PyTorch、onnx、onnxruntime；macOS 另含 coremltools | `zt models export`、`zt models parity`、`zt models serve` |
| `train` | PyTorch、OpenCC、pyahocorasick、mwparserfromhell、datasketch、aiohttp | `zt train` |

PyTorch 在 Linux 上使用 CUDA 13.0 的版本（`download.pytorch.org/whl/cu130`），在 macOS 上使用 PyPI 的版本（CPU 與 MPS）。

## 指令

每個指令都有 `--help`；`zt help` 列出全部。

### 環境

| 指令 | 說明 |
|---|---|
| `zt setup` | 檢查 uv、Swift、git，macOS 另查 Xcode、Linux 另查 ONNX Runtime，再 `uv sync`；缺少的工具會附上安裝方式 |
| `zt setup --group export --group dev` | 一併安裝相依群組 |
| `zt setup --install-onnxruntime` | Linux：把 ONNX Runtime 1.24.4 C SDK 下載到 `~/opt`（或 `ONNXRUNTIME_ROOT` 所在的目錄） |
| `zt setup --quiet` | 只印出缺少的項目（`zt --remote` 在主機上每次都先執行） |

### 資料（`ml/src/zhuyintype_ml/data/`）

| 指令 | 說明 |
|---|---|
| `zt data fetch` | 下載 App 需要的元件（lexicon、lm、lm_chat、reranker、v3），依 `resources/assets.json` 驗證雜湊 |
| `zt data fetch eval` | 評測需要的全部元件，含評測集與檢查點；`engine` 只含詞庫與語言模型，`all` 是全部 |
| `zt data status` | 每個元件是否存在、是否為固定的版本 |
| `zt data pack data-4 v3` | 把變更過的元件打包到 `dist/assets/`，並更新 `assets.json` |
| `zt data publish data-4` | 用 gh 把打包好的檔案上傳成 GitHub release |
| `zt data lexicon` | 沿用 `lexicon.tsv`；沒有時下載 CC-CEDICT 與 Unihan 再建置。`--rebuild` 重新建置 |
| `zt data compile` | 把詞庫與語言模型編成 mmap 二進位，放在 TSV 旁邊；`zt data compile OUT` 輸出到 OUT |

### 建置

| 指令 | 說明 |
|---|---|
| `zt build cli` | `engine/swift` 的 zhuyin-cli 與檢查程式（release，`.build/release/`）；`--debug`、`--product zhuyin-cli` |
| `zt build app` | `dist/ZhuyinType.app`（macOS，需要 Xcode）；`--debug` |
| `zt build install` | 建置並安裝到 `~/Library/Input Methods` |
| `zt build pkg` | `dist/ZhuyinType-<版本>.pkg` |

### 檢查

| 指令 | 說明 |
|---|---|
| `zt check engine` | 引擎行為與延遲檢查（ZhuyinCoreChecks）；較慢或忙碌的機器設 `ZHUYIN_CHECKS_TIMING=warn` |
| `zt check app` | 已建置 App 的自我測試與封裝檢查 |
| `zt check python` | `ml/src` 底下 `test_*.py` 的 unittest；`--group train` 帶 PyTorch 執行，`--start src/zhuyintype_ml/remote` 只跑一個目錄 |
| `zt check lint` | ruff；`--fix` 自動修正，`--format` 一併檢查排版 |
| `zt check types` | mypy，範圍在 `pyproject.toml` 的 `[tool.mypy] files` |
| `zt check public` | 確認要發布的檔案裡沒有私人主機、金鑰或內部網路位址（`tools/check-public.sh`） |
| `zt check all` | python、lint、types、public 全部執行 |

### 評測與訓練

| 指令 | 說明 |
|---|---|
| `zt eval sets` | 評測集與是否存在 |
| `zt eval conversion --set colloquial-dev wiki-dev` | 整句轉換；其他評測見 `zt eval --help` 與 `docs/ml/EVALUATION.md` |
| `zt bench run v0.5.0` | ZhuyinBench（在 GPU 主機上；發布後由 CI 執行） |
| `zt train list` | 訓練流程與步驟；`zt train v3` 執行未完成的步驟 |

### 模型（`ml/src/zhuyintype_ml/remote/`）

| 指令 | 說明 |
|---|---|
| `zt models export` | 從檢查點匯出 ONNX；檢查點、匯出程式與選項的雜湊值沒變時略過。`--tree` 另外匯出 `RerankerTree.onnx` |
| `zt models export --reranker reranker_v2c --v3 v3_new` | 匯出資料目錄裡其他的模型目錄 |
| `zt models parity --coreml` | Mac：產生 Core ML 的參考輸出 |
| `zt models parity` | Linux：ONNX 對 PyTorch 與 Core ML 的一致性 |
| `zt models serve --device cuda -- eval conversion` | 啟動 PyTorch 模型伺服器，讓指令裡所有 zhuyin-cli 行程經由它在 GPU 上計算 |

### GPU 主機

| 指令 | 說明 |
|---|---|
| `zt --remote eval conversion --config engine -n 3 --set smoke` | 在主機上執行任何 zt 指令 |
| `zt --remote --export eval conversion --json out.json` | 先匯出 ONNX（`--config app` 需要），`out.json` 自動取回 |
| `zt --remote --backend cuda eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json` | 模型呼叫改由 GPU 上的 PyTorch 伺服器計算 |
| `zt --remote --push data/build/v3_new --detach train v3` | 先傳送本機檔案，並在背景執行 |
| `zt --remote --workdir onnx-eval-x --env ZT_BUILD_JOBS=16 ...` | 指定主機上的工作目錄；限制建置使用的核心數 |
| `zt remote status` | 主機負載、執行中的工作與記錄的最後幾行 |
| `zt remote logs -f` | 持續顯示最近一次執行的記錄 |
| `zt remote pull [PATH ...]` | 取回 `data/build/remote_eval/` 與指定的路徑 |
| `zt remote stop` | 依記錄的 PID 停止背景執行的工作 |
| `zt remote shell` | 在主機的工作目錄開一個 shell |
| `zt remote sync` | 只同步原始碼 |
| `zt remote list` | 主機上的工作目錄與最後的結束狀態 |

`zt --remote` 的流程、選項與主機上的目錄見 [ml/src/zhuyintype_ml/remote/README.md](../ml/src/zhuyintype_ml/remote/README.md)。

## 環境變數

| 變數 | 用途 | 預設 |
|---|---|---|
| `ZT_DATA` | 資料目錄 | `data/build` |
| `ZT_CLI` | zhuyin-cli 執行檔 | `.build/release/zhuyin-cli`，第一次使用時建置 |
| `SWIFT_SCRATCH_PATH` | SwiftPM 建置目錄 | `.build` |
| `ZT_BUILD_JOBS` | swift build 的平行數 | 全部核心 |
| `ZT_SWIFT` | swift 執行檔 | PATH，其次 `~/opt/swift/usr/bin/swift` |
| `ONNXRUNTIME_ROOT` | Linux 的 ONNX Runtime C SDK | `~/opt/onnxruntime` |
| `ZT_UV` | uv 執行檔 | PATH，其次 `~/.local/bin/uv`、`~/.cargo/bin/uv` |
| `ZHUYIN_ASSET_CACHE` | 下載的資料壓縮檔快取 | `.cache/assets` |
| `ZHUYIN_REMOTE_HOST`、`ZHUYIN_REMOTE_KEY`、`ZHUYIN_REMOTE_BASE` | GPU 主機（見 `.remote.env.example`） | |

`~/opt` 底下的位置只是手動解壓縮工具鏈時的後備位置；PATH 上的工具一律優先。

## 檔案

| 檔案 | 說明 |
|---|---|
| `zt` | 啟動程式（POSIX sh）：找到版本庫與 uv，執行 `uv run --project ml python -m zhuyintype_ml.cli` |
| `check-public.sh` | 發布前的隱私檢查 |
| `../ml/pyproject.toml`、`../ml/uv.lock`、`../ml/.python-version` | uv 專案、相依群組、ruff 與 mypy 設定、固定的版本 |
| `../ml/src/zhuyintype_ml/cli/` | 指令列：`main.py` 載入 `commands/` 底下每個模組 |

## 相關文件

- [ml/CONVENTIONS.md](../ml/CONVENTIONS.md)：Python 工具的結構與寫法
- [ml/src/zhuyintype_ml/remote/README.md](../ml/src/zhuyintype_ml/remote/README.md)：GPU 主機與 Linux 的模型執行環境
- [ml/src/zhuyintype_ml/data/README.md](../ml/src/zhuyintype_ml/data/README.md)：資料與模型的版本、詞庫
- [docs/ml/EVALUATION.md](../docs/ml/EVALUATION.md)：評測

# 可攜式 CLI 與遠端評測

Mac 維持使用 Core ML。Linux 使用同一套 Swift 引擎與計分程式，透過 ONNX Runtime 的 C API 執行模型。實測的一致性與耗時見 [REPORT.md](REPORT.md)，各平台調校與未來的 C ABI、前端見 [PORTING.md](PORTING.md)。

## 執行

在 Mac 的版本庫根目錄執行。主機與 SSH 金鑰從 `.remote.env` 的 `ZHUYIN_REMOTE_HOST`、`ZHUYIN_REMOTE_KEY` 讀取（範例見 `.remote.env.example`）：

```sh
JOBS=60 nlp/remote/run.sh conversion_eval -n 400 \
  --reranker data/build/reranker --rerank-count 16 \
  --cli-arg=--v3 --cli-arg=data/build/v3 --cli-arg=--v3-weight --cli-arg=1 \
  --test data/build/corpus/dev_colloquial.txt data/build/corpus/dev_wikipedia.txt data/build/lm_chat/dev_chat_tt.txt

JOBS=60 nlp/remote/run.sh sweep --matrix nlp/remote/validation.json -n 400
# 選用、較快的 GPU 主機評測（已實測；需要有空閒的 GPU 記憶體）：
BACKEND=cuda JOBS=60 nlp/remote/run.sh sweep --matrix nlp/remote/validation.json -n 400

JOBS=60 nlp/remote/run.sh error_analysis -n 400 --reranker data/build/reranker \
  --test data/build/corpus/dev_colloquial.txt
JOBS=60 nlp/remote/run.sh typing_errors -n 400 --json data/build/remote_eval/typing-errors.json
JOBS=60 nlp/remote/run.sh typo_eval -n 400 --reranker data/build/reranker \
  --json data/build/remote_eval/typos.json
```

`run.sh` 只把可攜原始碼、評測程式原始碼、詞庫／LM／評測檔案、JSON 與模型檢查點同步到隔離的 `~/zhuyintype/work/onnx-eval`。它不同步 Core ML 產出物，也不寫入訓練工作區。在 reranker 檢查點取回到本機 `data/build/reranker/charlm.pt` 之前，它會把已驗證的遠端 `reranker_v2b/charlm.pt` 複製到隔離目錄。建置與匯出都在該目錄進行（匯出結果依輸入內容的雜湊值快取）。遠端 `data/build/remote_eval/` 下的記錄檔、sweep 預測與 JSON 輸出會取回到本機 `data/build/remote_eval/remote/`，失敗時也會取回。建置、匯出或評測任一步回傳非零值時立即結束。

`HOST`、`KEY`、`REMOTE_DIR`（限定在隔離的前綴路徑下）、`JOBS` 與 `BACKEND=cpu|cuda` 都可以覆寫。`cpu` 是預設的 ONNX C API 路徑。`cuda` 會啟動已實測的 fp32 PyTorch 批次伺服器，記憶體配置上限 2 GiB，把它的私有 socket 傳給每個 worker，並在結束時清理。CUDA 適合在 GPU 主機上跑大型 sweep；CPU 不需要 GPU 容量。不要對同一個 `REMOTE_DIR` 同時執行多個 runner。其他 CLI worker 可以與訓練工作同時執行。每個 ONNX session 使用一個 CPU 執行緒。三個沒有 `--jobs` 的工具由 `evaluate.py` 把彼此獨立的 CLI 批次行分割平行處理，並保留順序、錯誤與原本的輸出。conversion_eval 使用它既有的 `--jobs`。平行轉接層會拒絕有狀態的 `--keep-memory` 批次。

`TREE_RERANKER=1` 會另外匯出 `RerankerTree.onnx`，v2b 權重不變。原生 CPU 計分器在此檔存在時自動使用它，並以批次計分作為後備。做基準比較時，另外準備一個只含 `Reranker.onnx`、`vocab.json` 與 `config.json` 的目錄。樹狀匯出須明確啟用，讓其他同時進行的 sweep 維持原本的匯出行為。App 設定矩陣、分數一致性與 Mac 計時指令見 [REPORT_tree.md](../training/reranker/REPORT_tree.md)。選用的 CUDA socket 伺服器仍使用批次計分。
`TREE_RERANKER=0` 不會移除或停用已經存在的樹狀模型；選用哪個模型只取決於模型檔是否存在。

## 具名 sweep

矩陣是一個 JSON，內容為 `configurations: [{"name": "…", "args": ["--flag", "value", …]}]` 與 `sets: [{"name": "…", "path": "…", "args": ["--line-context"]}]`。set 的 args 是 conversion_eval 的選項；configuration 的 args 是 CLI 選項。接續組字請用 `"args": ["--split", "continue"]`。範例見 `validation.json`。名稱只能使用英文字母、數字、`_` 與 `-`。runner 執行所有組合（笛卡兒積）並輸出一張表。`--jobs` 控制每一列內的 worker 數。除此之外，模型與引擎旗標不受限制。輸出包含每一筆依序的 CLI 輸入與預測、各 set 的準確率，以及整次評測的實際耗時（含 Python 準備與 CLI 啟動）。

## 遠端環境設定（已在 GPU 主機完成）

使用 `~/opt/swift/usr/bin` 既有的 Swift 6.4.1 工具鏈；官方的 Ubuntu 26.04 版本可在這台主機上運作。把官方 ONNX Runtime **1.24.4** Linux x64 SDK 安裝到 `~/opt/onnxruntime-linux-x64-1.24.4`，並建立 `~/opt/onnxruntime` 符號連結指向它。建置時可用 `ONNXRUNTIME_ROOT` 覆寫 SDK 路徑；其下必須有 `include/` 與 `lib/`。Package.swift 只在 Linux 上連結 bridge，並為評測嵌入 SDK 的 lib 路徑。

```sh
cd ~/zhuyintype/work/onnx-eval
~/.local/bin/uv venv .venv --python ~/zhuyintype/.venv-gen/bin/python
printf '%s\n' "$HOME/zhuyintype/.venv-gen/lib/python3.12/site-packages" > .venv/lib/python3.12/site-packages/training-torch.pth
~/.local/bin/uv pip install --python .venv/bin/python -r nlp/remote/requirements.txt
PATH="$HOME/opt/swift/usr/bin:$PATH" swift build -c release --package-path nlp/engine --scratch-path .build
```

所有安裝指令、來源與解析後的版本都記錄在 REPORT.md。這個獨立環境重用 Torch，不更動 `.venv-gen` 或其他訓練工作。Mac 上不需要任何 Python 或全域工具鏈設定。

## 匯出與檢查

```sh
.venv/bin/python nlp/remote/export_onnx.py \
  --reranker-checkpoint data/build/reranker/charlm.pt --fp16
# 在模型 JSON 旁寫出 fp32 基準模型與選用的 *.fp16.onnx 實驗模型。
.venv/bin/python nlp/remote/parity.py --output data/build/remote_eval/parity.json
PATH="$HOME/opt/swift/usr/bin:$PATH" swift test -c release --package-path nlp/engine --scratch-path .build --filter BackendTests
python3 nlp/remote/test_tools.py
# Linux socket 實驗的回歸測試（使用已建置的 CLI 與既有詞庫）：
python3 nlp/remote/test_socket.py
```

一致性檢查需要既有的 `data/build/v3/test_vectors.json` 與 `data/build/remote_eval/reranker-coreml.json`。後者在 Mac 上以 `data/build/v3/.venv/bin/python nlp/remote/parity.py --coreml --output data/build/remote_eval/reranker-coreml.json` 產生，再傳到遠端。匯出與一致性工具不會修改任何訓練原始碼檔案。PyTorch 的 import 為唯讀；若訓練目錄完全不能寫入快取，請設定 `PYTHONDONTWRITEBYTECODE=1`。

重新做一致性研究時，先凍結原始碼與資料，用 Xcode 建置 Mac CLI，在兩個平台上執行同一個矩陣，先比對依序的輸入再計算輸出差異，並保存 manifest。`compare.py` 輸出最後的合併表格；任一列的預測差異超過 1% 時回傳非零值。既有的暫存快照是證據，不是之後執行的輸入。

選用的 `gpu_benchmark.sh` 使用保留下來的 fp32 PyTorch socket 伺服器，CUDA 記憶體配置上限 2 GiB。它執行同一個完整矩陣，只清理自己的伺服器。完整實測 sweep 使用此伺服器時為 393.13 s，使用 ONNX CPU 時為 610.12 s；6,000 筆 GPU／CPU 預測全部一致。須以 `BACKEND=cuda` 明確啟用；原生可攜執行環境預設仍為 ONNX CPU。

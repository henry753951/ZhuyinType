# ml 的程式與文件慣例

這份文件規定 `ml/` 底下 Python 工具、訓練流程與文件的寫法。新程式照這裡寫，舊程式在修改時順便改成這樣。整個版本庫的目錄與依賴方向見 [../AGENTS.md](../AGENTS.md)。

## 結構

套件在 `ml/src/zhuyintype_ml/`（以下路徑都相對於它）：

```
cli/                     指令列：main.py 依序載入 commands/ 底下每個模組（tools/zt 呼叫 python -m zhuyintype_ml.cli）
cli/commands/<群組>.py   一個指令群組一個模組，提供 register(subparsers)
core/                    共用核心：paths、zhuyin（讀音與按鍵）、engine（設定與 zhuyin-cli 行程池）、
                         sets（評測集登錄）、results（Report／Section）
eval/                    評測：每種評測一個模組，實作 eval/base.py 的 Evaluator，登記在 registry.py
training/<模型>/         訓練：pipeline.py 定義 PIPELINE（training/pipeline.py 的 Pipeline／Step）
data/                    資料與模型的版本（resources/assets.json）、詞庫建置
remote/                  GPU 主機：同步、執行、取回；ONNX 匯出與一致性檢查
```

依賴方向只能由上往下：`cli` → `eval`／`training`／`data`／`remote` → `core`。`core` 不依賴其他模組。`ml` 只透過 `zhuyin-cli` 這個行程使用引擎（`engine/`），不 import 引擎的程式。

## 設計原則

- 一個模組只做一件事。指令模組只處理參數與輸出，實際工作在 `core`、`eval`、`training`。
- 新增功能用新增模組的方式完成，不改既有的分派程式：新指令加一個 `commands/*.py`，新評測加一個 Evaluator 並登記，新模型加一個 `pipeline.py`。
- 同一件事只寫一次：模型設定用 `EngineConfig`（`--config app` 會呼叫 `zhuyin-cli --models`，和 App 用同一份程式載入），評測集用名稱（`core/sets.py`），路徑用 `core/paths.py`，按鍵與讀音用 `core/zhuyin.py`。
- 在本機或 GPU 主機執行的是同一個指令；任何指令前加 `zt --remote` 就在主機上跑。程式裡不寫死主機、使用者目錄或 SSH。
- 評測結果是 `Report` 物件，存成 JSON；其他工具讀 JSON，不解析文字輸出。

## Python

- Python 3.12 以上，用 uv 管理：`ml/pyproject.toml` 定義套件（src 結構，以可編輯模式安裝）與相依群組，`ml/uv.lock` 固定版本。
  - 沒有相依群組：`zt` 本身與 `core`、`eval`、`cli` 只用標準函式庫。
  - `train`：PyTorch 等訓練用套件。`export`：ONNX、coremltools 等匯出用套件。`dev`：ruff、mypy。
  - 需要相依群組的步驟，用 `Context.python(module, *args, group=...)` 以 `uv run --group` 執行，不在 `zt` 的行程裡 import。
- 排版與檢查：`uv run --group dev ruff format`、`uv run --group dev ruff check`。設定在 `ml/pyproject.toml`（行寬 120）。
- 型別：所有函式標註參數與回傳型別，`from __future__ import annotations`；`uv run --group dev mypy` 必須通過。
- 每個模組開頭的 docstring 說明用途與用法（指令範例），寫給第一次讀的人。
- 執行的指令一律是 `zt ...` 或 `python -m zhuyintype_ml....`；不寫 `python3 某個檔案.py`，不用 `sys.path.insert`。
- 測試：模組旁的 `test_*.py` 用標準函式庫的 unittest，`zt check python` 執行全部。

## 文件

- 所有文件依 `docs/product/WRITING.md`：台灣正體中文、平實、不另寫英文版。
- 每個模組（`ml/src/zhuyintype_ml/<模組>/`、`training/<模型>/`）有一份 README.md，段落固定：
  1. 標題與一句話說明
  2. 用途：解決什麼問題、產物用在哪裡
  3. 指令：`zt ...` 的常用寫法
  4. 輸入與輸出：讀哪些檔、寫到哪裡（相對於資料目錄）
  5. 檔案：每個程式檔一行說明
  6. 相關文件
- 實驗過程與數字寫在 REPORT.md（或 `docs/research/log.md`），不寫在 README。

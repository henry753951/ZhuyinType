# nlp 的程式與文件慣例

這份文件規定 `nlp/` 底下 Python 工具、訓練流程與文件的寫法。新程式照這裡寫，舊程式在修改時順便改成這樣。

## 結構

```
tools/zt                 唯一的進入點（呼叫 uv run python -m nlp.cli）
nlp/cli/                 指令列：main.py 依序載入 commands/ 底下每個模組
nlp/cli/commands/<群組>.py  一個指令群組一個模組，提供 register(subparsers)
nlp/common/              共用核心：paths、zhuyin（讀音與按鍵）、engine（設定與 zhuyin-cli 行程池）、
                         sets（評測集登錄）、results（Report／Section）
nlp/eval/                評測：每種評測一個模組，實作 nlp/eval/base.py 的 Evaluator，登記在 registry.py
nlp/training/<模型>/     訓練：pipeline.py 定義 PIPELINE（nlp/training/pipeline.py 的 Pipeline／Step）
nlp/data/                資料與模型的版本（assets）、詞庫建置
nlp/remote/              GPU 主機：同步、執行、取回；ONNX 匯出與一致性檢查
nlp/engine/              引擎（Swift，之後換成 Rust）
```

依賴方向只能由上往下：`cli` → `eval`／`training`／`data`／`remote` → `common`。`common` 不依賴其他 nlp 模組。

## 設計原則

- 一個模組只做一件事。指令模組只處理參數與輸出，實際工作在 `common`、`eval`、`training`。
- 新增功能用新增模組的方式完成，不改既有的分派程式：新指令加一個 `commands/*.py`，新評測加一個 Evaluator 並登記，新模型加一個 `pipeline.py`。
- 同一件事只寫一次：模型設定用 `EngineConfig`（`--config app` 會呼叫 `zhuyin-cli --models`，和 App 用同一份程式載入），評測集用名稱（`nlp/common/sets.py`），路徑用 `nlp/common/paths.py`，按鍵與讀音用 `nlp/common/zhuyin.py`。
- 在本機或 GPU 主機執行的是同一個指令；任何指令前加 `zt --remote` 就在主機上跑。程式裡不寫死主機、使用者目錄或 SSH。
- 評測結果是 `Report` 物件，存成 JSON；其他工具讀 JSON，不解析文字輸出。

## Python

- Python 3.12 以上，用 uv 管理：根目錄的 `pyproject.toml` 定義專案與相依群組，`uv.lock` 固定版本。
  - 沒有相依群組：`zt` 本身、`nlp/common`、`nlp/eval`、`nlp/cli` 只用標準函式庫。
  - `train`：PyTorch 等訓練用套件。`export`：ONNX、coremltools 等匯出用套件。`dev`：ruff、mypy。
  - 需要相依群組的步驟，用 `Context.python(module, *args, group=...)` 以 `uv run --group` 執行，不在 `zt` 的行程裡 import。
- 排版與檢查：`uv run --group dev ruff format`、`uv run --group dev ruff check`。設定在 `pyproject.toml`（行寬 120）。
- 型別：所有函式標註參數與回傳型別，`from __future__ import annotations`；`uv run --group dev mypy` 必須通過。
- 每個模組開頭的 docstring 說明用途與用法（指令範例），寫給第一次讀的人。
- 執行的指令一律是 `zt ...` 或 `python -m nlp....`；不寫 `python3 nlp/xxx/yyy.py`，不用 `sys.path.insert`。
- 測試：`nlp/**/test_*.py` 用標準函式庫的 unittest，`zt check python` 執行全部。

## 文件

- 所有文件依 `docs/WRITING.md`：台灣正體中文、平實、不另寫英文版。
- 每個模組（`nlp/<模組>/`、`nlp/training/<模型>/`）有一份 README.md，段落固定：
  1. 標題與一句話說明
  2. 用途：解決什麼問題、產物用在哪裡
  3. 指令：`zt ...` 的常用寫法
  4. 輸入與輸出：讀哪些檔、寫到哪裡（相對於資料目錄）
  5. 檔案：每個程式檔一行說明
  6. 相關文件
- 實驗過程與數字寫在 REPORT.md（或 `docs/research/log.md`），不寫在 README。

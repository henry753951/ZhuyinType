# ml：資料、訓練與評測

ZhuyinType 的研發部分：建置詞庫與資料、訓練模型、評測引擎，以及在 GPU 主機上執行這些工作。這裡的程式不隨 App 出貨。

## 用途

- 產生 App 使用的詞庫、語言模型與神經模型，以 `resources/assets.json` 固定版本後發布。
- 用同一套評測量每個版本（ZhuyinBench）與每個實驗。
- 引擎只透過命令列 `zhuyin-cli` 使用，兩邊各自改版。

## 指令

所有工作都由根目錄的 `tools/zt` 執行（說明見 [../tools/README.md](../tools/README.md)）：

```sh
tools/zt setup
tools/zt data fetch
tools/zt --remote eval conversion --set dev
tools/zt train list
tools/zt check all
```

## 結構

| 路徑 | 內容 |
|---|---|
| `pyproject.toml`、`uv.lock` | uv 專案、相依群組、ruff 與 mypy 設定 |
| `src/zhuyintype_ml/core/` | 共用核心：路徑、注音讀音與按鍵、評測集、結果、引擎行程池、App 的改字記錄 |
| `src/zhuyintype_ml/cli/` | `zt` 指令 |
| `src/zhuyintype_ml/eval/` | 評測與 ZhuyinBench（[../docs/ml/EVALUATION.md](../docs/ml/EVALUATION.md)） |
| `src/zhuyintype_ml/training/` | 訓練流程（[README](src/zhuyintype_ml/training/README.md)） |
| `src/zhuyintype_ml/data/` | 資料與模型的版本、詞庫建置 |
| `src/zhuyintype_ml/remote/` | 在 GPU 主機上執行 |
| `eval-sets/` | 納入版本庫的評測集 |
| `CONVENTIONS.md` | 程式與文件慣例 |

# 參與 ZhuyinType 開發（給 AI 代理與貢獻者）

ZhuyinType 是開源的注音輸入法。

- 共用的引擎，以及所有和選字有關的程式，都在 `nlp/`。
- 各平台的輸入法在 `clients/`，目前只有 macOS。

請先讀這份文件，再讀你要修改的部分對應的文件。

## 目錄

| 路徑 | 內容 | 文件 |
|---|---|---|
| `nlp/engine/` | 跨平台的 Swift package（見下方說明） | `docs/ARCHITECTURE.md`、`docs/MODEL.md` |
| `nlp/data/` | 詞庫建置、資料編譯（mmap 二進位）、資料與模型的版本清單（`assets.json`、`assets.py`）、資料來源 | `nlp/data/SOURCES.md` |
| `nlp/training/` | 每個模型或資料流程一個目錄（`lm`、`lm_chat`、`lm_chat2`、`reranker`、`v3`、`homophone`、`base`、`corpus`、`memory`），各附 README／REPORT | `nlp/training/README.md` |
| `nlp/eval/` | 評測工具（轉換、候選字、改選、打字習慣、外部評測集、成對統計、Mac 效能量測）與 ZhuyinBench（`bench/`） | `docs/EVALUATION.md` |
| `nlp/remote/` | GPU 主機的執行工具：同步引擎、在 Linux 上建置、以 ONNX Runtime 載入模型，並在主機上執行任何 `nlp/eval` 工具 | `nlp/remote/README.md` |
| `clients/macos/` | InputMethodKit 輸入法的 Swift package（依賴 `../../nlp/engine` 與 Sparkle）、資源檔、建置與發布腳本 | `docs/DESIGN.md` |
| `docs/` | 設計、架構、模型、評測、規劃、研究（`docs/research/`）、每版成績（`docs/benchmarks/`）、發布說明（`docs/releases/`） | `docs/ROADMAP.md` |
| `data/` | 只在本機（gitignore）：`data/build/` 放 `nlp/data/assets.py` 下載或訓練產生的資料與模型 | |
| `references/` | 只在本機：供研究的其他輸入法。不得複製其中的程式碼（0c 為 AGPL-3.0） | `docs/research/README.md` |

`nlp/engine/` 包含：

- `ZhuyinCore`：組字、按鍵解碼、容錯通道、詞庫、語言模型、個人模型；
- `ZhuyinML`：整句重排 v2b 與音節對字模型 v3。macOS 用 Core ML，Linux 透過 `ONNXBridge` 用 ONNX Runtime；
- `zhuyin-cli`：所有評測都經由它執行；
- `ZhuyinCoreChecks`：引擎檢查。

## 指令（在版本庫根目錄執行）

所有工作都由 `tools/zt` 執行（`tools/zt help` 列出全部指令，說明見 [tools/README.md](tools/README.md)）。任何指令前加 `--remote`，就在 `.remote.env` 指定的 GPU 主機上執行同一個指令。

```sh
tools/zt setup                              # 檢查並準備這台機器（uv、Swift、Xcode 或 ONNX Runtime）
tools/zt data fetch                         # 下載資料與模型到 data/build（版本由 nlp/data/assets.json 固定）
tools/zt build cli | app | install | pkg    # zhuyin-cli、dist/ZhuyinType.app、安裝到 ~/Library/Input Methods、pkg
tools/zt check engine | app | python | lint | types | public | all
tools/zt --remote eval conversion --set dev           # 在 GPU 主機上評測（--config app 和 App 的設定相同）
tools/zt eval stats BEFORE.json AFTER.json            # 成對比較
tools/zt --remote bench run vX.Y.Z          # ZhuyinBench（CI 會在每次發布後執行）
tools/zt --remote train v3                  # 訓練流程（tools/zt train list）
clients/macos/scripts/release.sh            # 發布流程執行的內容（pkg、Sparkle zip、差分更新、appcast）
```

`make cli`、`make checks`、`make app`、`make install` 也可以用，它們會呼叫 `tools/zt`。程式與文件的寫法見 [nlp/CONVENTIONS.md](nlp/CONVENTIONS.md)。

建置需要完整的 Xcode（SwiftUI macro）；`build-app.sh` 會自動把 `DEVELOPER_DIR` 切到 Xcode。

## 規則

- **評測與訓練在 GPU 主機上執行，不在開發用的 Mac 上執行**。Mac 只做以下幾件事：
  - 建置；
  - 匯出、編譯 Core ML；
  - 量測延遲與能耗（`tools/zt eval latency`）；
  - 確認遠端結果和 Mac 一致的一次性檢查。
- **用開發集做決定，測試集只用來追蹤**。採用任何改變前要有成對比較（`tools/zt eval stats`），並寫進 `docs/research/log.md`。試過但沒採用的想法也記在那裡，並寫進 `docs/MODEL.md` §11。
- **文件與程式保持一致**。行為、預設值、檔案格式或數字改變時，在同一次修改中更新 `docs/`：
  - ARCHITECTURE 寫流程；
  - MODEL 寫方法與數字；
  - DESIGN 寫使用者看到的部分；
  - ROADMAP 寫進度。
- **發布**：
  1. 更新 `clients/macos/Resources/Info.plist` 的 `CFBundleShortVersionString` 與 `CFBundleVersion`。
  2. 撰寫 `docs/releases/vX.Y.Z.md`，依 `docs/WRITING.md` 的「發布說明與對外文字」撰寫。
  3. 提交後推送標籤 `vX.Y.Z`。CI 會建置、簽章、發布、更新 `appcast.xml`，並啟動 ZhuyinBench。
- **資料與模型不放進 git**。新版本用 `tools/zt data pack data-N` 與 `tools/zt data publish data-N` 打包、發布，再提交 `nlp/data/assets.json`。
- **隱私**：
  - 追蹤的檔案中不放私人主機或金鑰（`.remote.env` 只在本機）；
  - 記錄中不出現使用者輸入的文字，App 的 bundle ID 只以 `.private` 記錄；
  - 安全輸入時暫停讀取前文與學習。
- **文字**：
  - 所有文件與介面文字都依照 `docs/WRITING.md`：台灣正體中文、語氣平實，不另外寫英文版本；
  - 發布說明與對外文字比照正式產品；
  - 提到其他輸入法時視為相關工作：說明對方的貢獻、註明出處、寫出比較的條件與限制，不寫成要打敗的對象。
- GPU 主機上的長時間工作用 `nohup`，記錄放在 `~/zhuyintype/logs/`。不要用模糊的名稱比對來終止程序。
- Python 用 uv、ruff 與 mypy，Rust 用 cargo fmt 與 clippy；`tools/zt check all` 必須通過。
- 提交訊息結尾附上工作階段要求的署名行；不要對 `main` 強制推送。

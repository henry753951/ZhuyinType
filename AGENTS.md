# 參與 ZhuyinType 開發（給 AI 代理與貢獻者）

ZhuyinType 是開源的注音輸入法。

目錄依「會不會出貨」與語言分開，依賴只能單向：

```text
platforms/*  ──C 介面──▶  engine/  ◀──命令列行程──  ml/
platforms/*  ──讀取──▶  resources/assets.json  ◀──發布──  ml/
```

- `engine/` 是所有平台共用的輸入法引擎：Rust workspace（`engine/crates/`）。App 目前仍使用 Swift 版（`engine/swift/`），Rust 版完成模型與 C 介面（第 4 階段）後換上，再刪除 Swift 版。兩者的輸出逐位元相同。
- `platforms/` 是各作業系統的輸入法前端，目前只有 macOS。前端只透過引擎的公開介面使用它。
- `ml/` 是研發：資料、訓練、評測，以及在 GPU 主機上執行這些工作。它不隨 App 出貨，只透過 `zhuyin-cli` 使用引擎。
- `resources/` 是兩邊之間的契約：資料與模型的版本、來源與授權、二進位格式。

請先讀這份文件，再讀你要修改的部分對應的文件。

## 目錄

| 路徑 | 內容 | 文件 |
|---|---|---|
| `engine/` | Rust workspace：`zhuyin-core`（引擎核心）、`zhuyin-models`（模型介面）、`zhuyin-cli`；`parity/` 是和 Swift 版的一致性比對 | `engine/README.md`、`engine/DESIGN.md` |
| `engine/swift/` | 目前 App 使用的 Swift 引擎（參考實作，第 5 階段移除） | `docs/engine/ARCHITECTURE.md`、`docs/engine/MODEL.md` |
| `platforms/macos/` | InputMethodKit 輸入法、設定介面、Sparkle 更新、建置與發布腳本 | `docs/product/DESIGN.md` |
| `ml/` | Python 專案（uv）：`src/zhuyintype_ml/` 底下是 `core`（共用核心）、`cli`（`zt` 指令）、`eval`（評測、ZhuyinBench）、`training`（每個模型一個流程）、`data`（資料版本、詞庫建置）、`remote`（GPU 主機） | `ml/README.md`、`ml/CONVENTIONS.md`、`docs/ml/EVALUATION.md` |
| `ml/eval-sets/` | 納入版本庫的評測集（善解 CC0、日常短句、延遲用句） | `ml/eval-sets/external/shanjie/README.md` |
| `resources/` | `assets.json`（資料與模型的固定版本）、`SOURCES.md`（來源與授權）、`formats/`（詞庫與語言模型的二進位格式） | `resources/SOURCES.md` |
| `tools/` | `zt` 啟動器、隱私檢查 | `tools/README.md` |
| `docs/` | `product/`（設計、文案）、`engine/`（架構、模型）、`ml/`（評測）、`research/`、`releases/`、`benchmarks/`、`ROADMAP.md` | `docs/README.md` |
| `data/` | 只在本機（gitignore）：`data/build/` 放下載或訓練產生的資料與模型 | |
| `references/` | 只在本機：供研究的其他輸入法。不得複製其中的程式碼（0c 為 AGPL-3.0） | `docs/research/README.md` |

## 指令（在版本庫根目錄執行）

所有工作都由 `tools/zt` 執行（`tools/zt help` 列出全部指令，說明見 [tools/README.md](tools/README.md)）。任何指令前加 `--remote`，就在 `.remote.env` 指定的 GPU 主機上執行同一個指令。

```sh
tools/zt setup                              # 檢查並準備這台機器（uv、Swift、Xcode 或 ONNX Runtime）
tools/zt data fetch                         # 下載資料與模型到 data/build（版本由 resources/assets.json 固定）
tools/zt build cli | app | install | pkg    # zhuyin-cli、dist/ZhuyinType.app、安裝到 ~/Library/Input Methods、pkg
tools/zt check engine | app | python | lint | types | public | all
tools/zt --remote eval conversion --set dev           # 在 GPU 主機上評測（--config app 和 App 的設定相同）
tools/zt eval stats BEFORE.json AFTER.json            # 成對比較
tools/zt --remote bench run vX.Y.Z          # ZhuyinBench（CI 會在每次發布後執行）
tools/zt --remote train v3                  # 訓練流程（tools/zt train list）
tools/zt remote wait --timeout 540          # 等背景工作結束（不要自己寫 ssh／sleep 迴圈）；list 看主機上誰在跑
platforms/macos/scripts/release.sh            # 發布流程執行的內容（pkg、Sparkle zip、差分更新、appcast）
```

`make cli`、`make checks`、`make app`、`make install` 也可以用，它們會呼叫 `tools/zt`。程式與文件的寫法見 [ml/CONVENTIONS.md](ml/CONVENTIONS.md)。

建置需要完整的 Xcode（SwiftUI macro）；`build-app.sh` 會自動把 `DEVELOPER_DIR` 切到 Xcode。

## 規則

- **評測與訓練在 GPU 主機上執行，不在開發用的 Mac 上執行**。Mac 只做以下幾件事：
  - 建置；
  - 匯出、編譯 Core ML；
  - 量測延遲與能耗（`tools/zt eval latency`）；
  - 確認遠端結果和 Mac 一致的一次性檢查。
- **GPU 主機是共用的**，同時常有好幾件工作在跑：
  - 多個變體合成一次執行（一個 `eval sweep` 矩陣或一個腳本），不要一個變體各自執行一次，每次都要重新同步、準備環境；
  - 互不相干的部分同時跑（例如比較中的各個輸入法），總處理程序數控制在核心數以內；
  - 沒有人在等結果的長時間工作加 `--nice 10`，讓別人在等的檢查先跑；
  - 用 `tools/zt remote wait` 等工作結束，不要自己寫 ssh／sleep 迴圈；等待時先做別的事；
  - 系統本來就不支援的功能，只跑少量樣本並標明「不支援」，不要整組跑完得到 0；
  - `tools/zt remote list` 看主機負載與誰在跑。
- **用開發集做決定，測試集只用來追蹤**。採用任何改變前要有成對比較（`tools/zt eval stats`），並寫進 `docs/research/log.md`。試過但沒採用的想法也記在那裡，並寫進 `docs/engine/MODEL.md` §11。
- **文件與程式保持一致**。行為、預設值、檔案格式或數字改變時，在同一次修改中更新 `docs/`：
  - ARCHITECTURE 寫流程；
  - MODEL 寫方法與數字；
  - DESIGN 寫使用者看到的部分；
  - ROADMAP 寫進度。
- **發布**：
  1. 更新 `platforms/macos/Resources/Info.plist` 的 `CFBundleShortVersionString` 與 `CFBundleVersion`。
  2. 撰寫 `docs/releases/vX.Y.Z.md`，依 `docs/product/WRITING.md` 的「發布說明與對外文字」撰寫。
  3. 提交後推送標籤 `vX.Y.Z`。CI 會建置、簽章、發布、更新 `appcast.xml`，並啟動 ZhuyinBench。
- **資料與模型不放進 git**。新版本用 `tools/zt data pack data-N` 與 `tools/zt data publish data-N` 打包、發布，再提交 `resources/assets.json`。
- **隱私**：
  - 追蹤的檔案中不放私人主機或金鑰（`.remote.env` 只在本機）；
  - 記錄中不出現使用者輸入的文字，App 的 bundle ID 只以 `.private` 記錄；
  - 安全輸入時暫停讀取前文與學習。
- **文字**：
  - 所有文件與介面文字都依照 `docs/product/WRITING.md`：台灣正體中文、語氣平實，不另外寫英文版本；
  - 發布說明與對外文字比照正式產品；
  - 提到其他輸入法時視為相關工作：說明對方的貢獻、註明出處、寫出比較的條件與限制，不寫成要打敗的對象。
- GPU 主機上的長時間工作用 `nohup`，記錄放在 `~/zhuyintype/logs/`。不要用模糊的名稱比對來終止程序。
- Python 用 uv、ruff 與 mypy，`tools/zt check all` 必須通過。Rust 在 `engine/` 執行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`cargo deny check`，必須零警告。
- 引擎的行為改變要同時改 Swift 與 Rust 兩版，並用 `engine/parity/` 確認輸出仍然相同，直到 Swift 版移除。
- 提交訊息結尾附上工作階段要求的署名行；不要對 `main` 強制推送。

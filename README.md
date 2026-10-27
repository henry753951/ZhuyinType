# ZhuyinType 注音

開源、在本機運作的 macOS 注音輸入法。

- 整句連續輸入，依前後文選字。
- 依游標前的文字，自動調整日常口語與正式用語的比重。
- 按到相鄰鍵、多按、漏按、兩鍵順序打反、一聲沒按空白時，仍能選出想打的字。
- 記住你選過的字詞、自訂詞，以及你常按錯的鍵。
- 有新版本時自動更新。
- 所有輸入都在本機處理。

目前版本為 0.4.0，需要 macOS 14 或更新版本。各版本的變更見 [發布說明](docs/releases/)。

## 安裝

從 [GitHub Releases](https://github.com/henry753951/ZhuyinType/releases/latest) 下載 `ZhuyinType-<版本>.pkg`，開啟後依指示安裝。之後的版本會自動更新（設定 → 關於 → 更新），更新檔都經過簽章驗證，在組字結束後換上。
套件安裝到 `/Library/Input Methods/ZhuyinType.app`，需要管理員權限。
目前套件使用建置機的架構；app 採 ad-hoc 簽章，pkg 的簽署與公證尚待完成。
若 macOS 顯示安全性提示，請至「系統設定 → 隱私權與安全性」查看開啟選項。
安裝新版後，正在使用的 ZhuyinType 會在目前的組字結束後自動換成新版，不需要登出或重新選擇輸入法。

第一次使用時：

1. 開啟「系統設定 → 鍵盤 → 輸入方式 → 編輯」（部分版本顯示為「文字輸入」）。
2. 按「＋」，選擇「繁體中文」，加入「ZhuyinType 注音」。已加入時可直接進行下一步。
3. 從選單列的輸入法選單選擇「注音 ZhuyinType」，或按 Control-Space 切換（依系統快捷鍵設定）。

清單尚未出現時，請登出再登入。每個帳號各自加入輸入方式。
曾從原始碼安裝者，請先移除 `~/Library/Input Methods/ZhuyinType.app`，再使用 pkg 版本。

## 基本按鍵

以下為預設設定；完整的按鍵表見 [產品設計 §2.1](docs/DESIGN.md#21-組字與按鍵)，「設定 → 快捷鍵」可重新指定。

| 按鍵 | 操作 |
|---|---|
| 注音鍵、聲調鍵（6 3 4 7） | 輸入注音與聲調，連續輸入整句；空白是一聲 |
| ↓ 或空白 | 開啟候選字清單；清單開著時，空白翻下一頁 |
| 1–9 | 選取候選字 |
| ↑ ↓ | 在清單裡移動 |
| ← → | 在組字中移動游標；直式清單開著時改前一個或後一個字 |
| ⇧↓／⇧↑ | 清單只列單字／列出按錯修正與模糊音 |
| Return | 送出整段；清單開著時選取目前的候選字 |
| ⌫ | 刪掉一個注音符號或游標前的字；清單開著時先關閉清單 |
| Esc | 關閉清單；再按清除注音，再按清空組字 |
| 單按 Shift | 中英切換 |

## 從原始碼建置

需要 Xcode（設定視窗使用 SwiftUI，需要 Xcode 內附的 macro 外掛；本專案使用 Swift 6.4）與 make。`xcode-select -p` 指向 Command Line Tools 時，建置腳本會自動改用 `/Applications/Xcode.app`。

```sh
python3 nlp/data/assets.py fetch   # 下載詞庫、語言模型與神經模型（依 nlp/data/assets.json 驗證雜湊）
make app           # dist/ZhuyinType.app
make install       # 安裝到目前帳號；正在使用的輸入法會在組字結束後換成新版
make checks        # 引擎檢查
make pkg           # dist/ZhuyinType-<版本>.pkg
```

| 目錄 | 內容 |
|---|---|
| `nlp/engine/` | 跨平台的引擎（Swift package）：組字、按鍵解碼、容錯、語言模型、神經模型、`zhuyin-cli` |
| `nlp/data/` | 詞庫工具、資料編譯、資料與模型的版本清單 |
| `nlp/training/` | 每個模型與資料的訓練流程，`nlp/eval/` 是評測工具與每版評測 |
| `clients/macos/` | macOS 輸入法（InputMethodKit）與建置、發布腳本 |
| `docs/` | 設計、架構、模型、評測、規劃、研究、每版成績、發布說明 |

給 AI 代理與貢獻者的說明在 [AGENTS.md](AGENTS.md)。

## 移除

先切換至英文輸入方式，並在「系統設定 → 鍵盤 → 輸入方式」移除 ZhuyinType，再刪除安裝的 app。
原始碼提供 `clients/macos/scripts/uninstall.sh`，可移除系統與目前帳號的 app，預設保留自訂詞、記住的字詞與設定。
執行 `clients/macos/scripts/uninstall.sh --purge` 會一併刪除這些個人資料。

## 文件

| 文件 | 內容 |
|---|---|
| [產品設計](docs/DESIGN.md) | 按鍵、狀態、預設值、介面 |
| [架構與流程](docs/ARCHITECTURE.md) | 從按鍵到畫面的完整流程與分數公式 |
| [模型與演算法](docs/MODEL.md) | 詞庫、按鍵解碼、容錯、語言模型、神經模型、個人模型與評測數字 |
| [v3 模型設計](docs/MODEL_V3.md) | 按鍵層級解碼器與音節對字模型的設計與實驗紀錄 |
| [評測](docs/EVALUATION.md) | 資料集、評測工具與指令 |
| [規劃](docs/ROADMAP.md) | 開發階段、下一步與待決事項 |
| [研究](docs/research/) | 研究紀錄、相關工作與比較、研究計畫 |
| [每版成績](docs/benchmarks/) | 每個版本在同一套評測（ZhuyinBench）上的結果 |
| [訓練](nlp/training/README.md) | 各模型的訓練流程與產物 |
| [文案準則](docs/WRITING.md) | 介面與文件用語 |

## 資料與授權

程式碼採 [GNU AGPL 3.0 授權](LICENSE)（`AGPL-3.0-only`）；產生的詞庫採 CC BY-SA 4.0。
詞庫使用 CC-CEDICT、Unicode Unihan 與 wordfreq；語言模型與神經模型另外使用維基百科、立法院公報、維基百科討論頁、Common Voice 與 Qwen3-8B 產生的句子。來源、處理方式與署名見 [資料來源](nlp/data/SOURCES.md) 及 [授權文字](clients/macos/Resources/Licenses)。
發布修改過的詞庫或模型時，請保留來源、修改說明與各自的授權。

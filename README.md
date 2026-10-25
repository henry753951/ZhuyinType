# ZhuyinType 注音

開源、在本機運作的 macOS 注音輸入法。

- 標準注音輸入與整句轉換。
- 候選字視窗提供直式、橫式排列與字級設定。
- 加入自訂詞，記住你選過的字詞。
- 設定中英切換鍵、選字鍵與空白鍵行為。

目前為 0.1.0 開發版本，需要 macOS 14 或更新版本。

## 安裝

下載對應處理器架構的 `ZhuyinType-0.1.0.pkg`，開啟後依指示安裝。
套件安裝到 `/Library/Input Methods/ZhuyinType.app`，需要管理員權限。
目前套件使用建置機的架構；app 採 ad-hoc 簽章，pkg 的簽署與公證尚待完成。
若 macOS 顯示安全性提示，請至「系統設定 → 隱私權與安全性」查看開啟選項。
更新前請完成組字並切換至英文輸入方式；安裝程式會停止正在執行的 ZhuyinType。

第一次使用時：

1. 開啟「系統設定 → 鍵盤 → 輸入方式 → 編輯」（部分版本顯示為「文字輸入」）。
2. 按「＋」，選擇「繁體中文」，加入「ZhuyinType 注音」。已加入時可直接進行下一步。
3. 從選單列的輸入法選單選擇「注音 ZhuyinType」，或按 Control-Space 切換（依系統快捷鍵設定）。

清單尚未出現時，請登出再登入。每個帳號各自加入輸入方式。
曾從原始碼安裝者，請先移除 `~/Library/Input Methods/ZhuyinType.app`，再使用 pkg 版本。

開發者可從專案目錄安裝到目前帳號：

```sh
make install
```

## 基本按鍵

以下為預設設定。

| 按鍵 | 操作 |
|---|---|
| 注音鍵、聲調鍵 | 輸入注音與聲調，連續輸入整句 |
| 空白 | 完成目前音節；音節完成後開啟候選字視窗 |
| Backspace | 刪除游標前的一個字或注音符號；候選字視窗開啟時先關閉視窗 |
| ←／→ | 在組字中移動游標；候選字視窗開啟時換頁 |
| ↓ | 開啟候選字視窗；視窗開啟時移至下一個候選字 |
| 1–9 | 選取目前頁面的候選字 |
| Enter | 送出整段；候選字視窗開啟時選取目前候選字 |
| Esc | 關閉候選字視窗、清除目前注音，再清空組字，依目前狀態逐次操作 |
| 單按 Shift | 中英切換 |

## 從原始碼建置

需要 Apple Command Line Tools（本專案使用 Swift 6.4）與 make。
首次產生詞庫時需要網路與 uv；腳本會準備專案的 `.venv` 與 wordfreq。

```sh
make lexicon       # 沿用現有詞庫，或下載來源並產生詞庫
make app           # release app：dist/ZhuyinType.app
make debug-app     # debug app
make checks        # 核心檢查
make pkg           # dist/ZhuyinType-0.1.0.pkg
```

平行建置可指定獨立目錄，例如 `SWIFT_SCRATCH_PATH=.build-pkg make app`。

## 移除

先切換至英文輸入方式，並在「系統設定 → 鍵盤 → 輸入方式」移除 ZhuyinType，再刪除安裝的 app。
原始碼提供 `scripts/uninstall.sh`，可移除系統與目前帳號的 app，預設保留自訂詞、記住的字詞與設定。
執行 `scripts/uninstall.sh --purge` 會一併刪除這些個人資料。

## 開發中

容錯輸入、模糊音、進階個人記憶與神經選字屬於後續開發階段，規格見 [產品與架構設計](docs/DESIGN.md#7-開發階段)。

## 資料與授權

程式碼採 [GNU AGPL 3.0 授權](LICENSE)（`AGPL-3.0-only`）；產生的詞庫採 CC BY-SA 4.0。
詞庫使用 CC-CEDICT、Unicode Unihan 與 wordfreq，來源、處理方式與署名見 [資料來源](data/SOURCES.md) 及 [授權文字](Resources/Licenses)。
發布修改過的詞庫時，請保留來源、修改說明與 CC BY-SA 4.0 授權。

貢獻者文件：[產品與架構設計](docs/DESIGN.md) · [模型與訓練設計](docs/MODEL.md) · [文案準則](docs/WRITING.md)。

## English

ZhuyinType is an open-source macOS Zhuyin input method with sentence conversion, candidates, custom phrases, and remembered word choices. Input is processed locally.
Version 0.1.0 is a development build for macOS 14 or later. Install the `.pkg`, then add **ZhuyinType 注音** in System Settings → Keyboard → Input Sources → Traditional Chinese. Switch using the input menu or Control-Space.
Build with `make app`, install for your account with `make install`, and run core checks with `make checks`.
Typo tolerance and neural ranking are planned. Code is AGPL-3.0-only; the generated lexicon is CC BY-SA 4.0.

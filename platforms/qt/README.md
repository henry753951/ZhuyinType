# 共用 Qt 客戶端

Linux 與 Windows 共用的候選字視窗、設定頁面與客戶端狀態。資訊架構沿用 macOS 版，視覺是自己的一套設計系統，不模仿任何平台；元件、token 與規則見 [設計系統](DESIGN_SYSTEM.md)。macOS 也能執行此預覽，方便核對畫面。

## 目前範圍

已移植九個主頁與十個子頁，包含 43 項設定、字級拉桿、分段選項、側欄與五種並列候選字情境。候選字支援直／橫式、每頁數量、選字鍵、選取、翻頁及清單切換；設定會驗證範圍並儲存，淺色／深色／跟隨系統即時更新。

介紹頁的試打經公開 C ABI 呼叫真實 Rust 引擎；字詞管理、匯入匯出、自訂快捷鍵、經同意記錄的彙總統計、記錄檔操作與手動更新查詢已串接。外觀範例仍使用明確標示的固定資料。原生 Fcitx5／TSF 透過本機通訊接入同一個引擎、設定與 Qt 候選字視窗，協定見 [原生輸入法與共用介面](IPC.md)。神經模型回呼、正式發行安裝程式與自動更新尚未完成；原生平台的驗證範圍分別記錄於各平台目錄。

本階段以 `origin/main` 的 `d27cf74d`（0.4.7）為基準，沿用 AGPL-3.0-only。版本、授權全文、NOTICE 與資料來源直接使用專案檔案。逐頁狀態見 [Qt 介面移植](../../docs/product/QT_DESKTOP.md)。

## 建置與檢查

需要專案固定的 Rust、C++20 編譯器、CMake 3.24 以上、Qt 6.8 以上的 Core、Quick、QuickControls2、Test、QuickTest、Network，以及 SVG／ICNS 圖片外掛。已在 macOS Qt 6.12.0、Ubuntu 26.04 Qt 6.10.2 與 Windows Server 2025 Qt 6.10.2／MSVC 2022 建置；各快照的測試結果與介面待修項目見 [驗證狀態](../../docs/product/QT_DESKTOP.md#驗證與下一階段)。Qt SDK 不在預設位置時設定 `CMAKE_PREFIX_PATH`，不修改全域工具設定。

在版本庫根目錄執行：

```sh
tools/zt data fetch engine
tools/zt data compile
tools/zt build desktop
tools/zt check desktop
.build-qt/zhuyintype-desktop
.build-qt/zhuyintype-desktop --floating
.build-qt/zhuyintype-desktop --render-dir .build-qt/renders
```

macOS 預覽產生獨立的 `.build-qt/zhuyintype-desktop.app`，以 `open .build-qt/zhuyintype-desktop.app` 開啟；上面的執行檔路徑改成 `.build-qt/zhuyintype-desktop.app/Contents/MacOS/zhuyintype-desktop`。它不會安裝或取代正式 macOS 輸入法。

Windows PowerShell 使用同一個 Python 入口與 CMake 專案，Qt SDK 和編譯器架構須相同：

```powershell
.\tools\zt.ps1 data fetch engine
.\tools\zt.ps1 data compile
.\tools\zt.ps1 build desktop
.\tools\zt.ps1 check desktop
.build-qt/Release/zhuyintype-desktop.exe
```

多組態產物在 `Release/`，單組態產物在建置目錄根目錄。`check desktop` 會建置、執行 C++ 狀態測試，並以 offscreen／software 後端產生介面截圖；測試與截圖使用暫存設定，不碰個人設定。這不取代 Windows／Linux 的桌面測試。

一般執行用 `QSettings` 的 `ZhuyinType`／`Desktop` 命名空間，與正式 macOS 輸入法隔離。`build desktop` 先建置 Rust C ABI，再將動態函式庫放在執行檔旁。可用 `--lexicon` 與 `--engine-library` 指定路徑。Qt 圖片外掛須隨將來的發行包部署；這個階段沒有產生可分發的套件。

## 共用範圍與 SOLID

| 部分 | 責任 | Linux／Windows |
|---|---|---|
| `settings.json` | 名稱、預設值、選項、範圍、分組與控制項種類 | 同一份 |
| `Settings` | 驗證、讀寫、變更通知及儲存錯誤 | 同一份；建構時注入儲存物件 |
| `Candidates` | 顯示用資料、頁碼、選取與操作意圖 | 同一份，不計算轉換結果 |
| `EngineSession`／`InputController` | C ABI 生命週期、真實組字、按鍵派送與安全輸入狀態 | 同一份 |
| `Shortcuts`／`DesktopServices`／`PlatformSettings` | 快捷鍵、彙總統計與更新查詢、相容性清單與數值記錄 | 同一份；系統事件由 adapter 提供 |
| `qml/` | 設定與候選字元件、版面、主題 | 同一份 |
| `ImeHost`／`ImePopup` | 本機通訊、欄位隔離、待送文字與不搶焦點的候選視窗 | 同一份 |
| `main.cpp`／`PreviewCapture` | 服務與設定組裝／獨立截圖驗證 | 同一份；範例資料只供預覽 |
| Fcitx5／TSF adapter | 按鍵、焦點、文字送出、游標位置與敏感欄位 | 各平台獨立，不載入 Qt 或 Rust |
| `engine/` | 轉換、模型判斷、組字與學習 | 沿用既有 Rust 與公開 C ABI |

單一職責：畫面、狀態、儲存和系統 adapter 分開。開放封閉與介面隔離：系統整合透過顯示資料及操作訊號接入，不在 QML 放平台分支。里氏替換：內嵌預覽和浮動視窗使用同一個候選字元件，正式 host 也必須保持不搶焦點與正確的選取索引。依賴反轉：畫面接收設定與候選狀態，儲存由入口提供；無全域 singleton 或服務定位器。

後續優先共用設定遷移、操作協調、引擎包裝與資源載入；只把系統必要差異留給 adapter。當需要正式引擎與示範引擎互換時，才在已有的訊號邊界補小型介面，不預先建立未使用的抽象層。

## 視覺基準與資產

- 對照 `docs/product/DESIGN.md` §4、`SETTINGS_IA.md` 和 macOS 的實際渲染，不以 Qt 預設樣式取代原設計。
- App 圖示直接把 `platforms/macos/Resources/AppIcon.icns` 打包為 Qt resource，不維護第二份品牌圖案。
- 側欄的 macOS 來源是 SF Symbols，不是 repository 內的圖檔。`icons/*.svg` 是專案自行繪製的替代圖示，沿用相同圖意與色彩，適用本專案 AGPL-3.0-only 授權；並非從 SF Symbols 匯出的圖案。
- 字體使用系統字型，不捆綁 Apple 字型。不同系統的文字光柵化要分別確認。
- 橫式清單依文字與注音實際寬度分配空間，空間不足時壓縮列寬；長注音可能省略，完整候選字仍有無障礙名稱。翻頁已有 140 ms 淡入位移，尚未重現 macOS 的雙頁交接與邊界回彈。
- `--floating` 是無啟用權的普通 Qt 測試視窗，不能代表 Wayland 的 input-method popup surface 已整合。

## 程式慣例

C++20、RAII、`QObject` 明確所有權，C／COM 邊界不傳遞例外。設定在 C++ 驗證，QML 只處理呈現與互動；不記錄輸入文字。新狀態邏輯有小型 QtTest，QML 用 `qmlformat` 排版。C++ 以 `/W4 /WX` 或 `-Wall -Wextra -Wpedantic -Werror` 建置。

維持 `platforms/* → engine/` 的依賴方向；本次只讀 macOS 的圖示資源，不依賴 Swift 程式。引擎更動另依 `engine/README.md` 做遠端 parity，本預覽不更動引擎。

Linux 的 [Fcitx5 前端](../linux/README.md) 已串接共用 Qt 候選介面、設定與快捷鍵，在 Xfce／X11 驗證實際文字欄位、滑鼠選字、安全輸入及設定介面內輸入，詳見 [Linux 驗證紀錄](../linux/VALIDATION.md)。Windows 的 [TSF 前端](../windows/README.md) 經 named pipe 使用相同服務，開發安裝與互動測試由該目錄維護。兩平台可用 `tools/zt build input-method`／`check input-method` 建置或檢查共用服務與原生前端；Windows 使用 `tools/zt.ps1`，互動桌面測試須另外執行。介面外觀修正目前由另一項工作接續，原生整合測試不代表逐頁視覺已驗收。

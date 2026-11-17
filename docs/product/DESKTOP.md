# Linux 與 Windows 版

2026-11-16：Linux 與 Windows 改用 `platforms/desktop/` 的 GPUI 程式 `zhuyintype`，取代先前的 Qt 客戶端。架構、建置與介面規則見 [platforms/desktop/ARCHITECTURE.md](../../platforms/desktop/ARCHITECTURE.md)，通訊協定見 [IPC.md](../../platforms/desktop/IPC.md)。

## 和 macOS 版的關係

- **功能與版面相同**：九個主頁、子頁面、設定列的順序與文字、ⓘ 說明與示範、頁面之間的連結、候選字視窗的各種樣式、設定嚮導，都依照 macOS 版（本文件 [DESIGN.md](DESIGN.md) 與 `platforms/macos/Sources/ZhuyinTypeApp/UI/`）。
- **視覺不同**：Linux／Windows 版是 ZhuyinType 自己的一套現代設計語言，不模仿 macOS，也不模仿 Windows 原生外觀。暖色中性的表面、單色線條圖示、以細分隔線區隔設定列、強調色只用在少數地方；視窗不用系統邊框，標題列與視窗按鈕由 App 自己繪製。macOS 版維持原生外觀。
- **用語**：⌥ 寫成 Alt、⌃ 寫成 Ctrl、「這台 Mac」寫成「這台電腦」、Finder 寫成「檔案管理員」（Windows：「檔案總管」）。

## 主題與透明效果

「視窗與提示 › 外觀」有三列，macOS 版只有其中的「外觀」（在這裡叫「候選字視窗」）：

| 設定 | 選項 | 預設 | 內容 |
|---|---|---|---|
| 主題（`theme`） | 跟隨系統、淺色、深色 | 跟隨系統 | App 畫的每個視窗：設定視窗、設定嚮導、候選字視窗與中英提示。Windows 的標題列與視窗邊框（`DWMWA_USE_IMMERSIVE_DARK_MODE`）也跟著主題 |
| 透明效果（`transparency`） | 跟隨系統、開啟、關閉 | 跟隨系統 | 視窗背景透出桌面的顏色。「跟隨系統」在 Windows 依照「設定 › 個人化 › 色彩 › 透明效果」（`EnableTransparency`），在 KDE Plasma 為開啟 |
| 候選字視窗（`panelAppearance`） | 跟隨主題、淺色、深色 | 跟隨主題 | 只改候選字視窗與中英提示，和 macOS 的「外觀」是同一個設定與值（`system` 在這裡表示跟隨主題） |

改設定立即套用，不需要重新開啟。「跟隨系統」時也即時跟著系統改變：淺色或深色由 GPUI 跟著 Windows（`AppsUseLightTheme`，`WM_SETTINGCHANGE` 的 `ImmersiveColorSet`）與 XDG desktop portal 的 `org.freedesktop.appearance` `color-scheme`；沒有 portal 的 Linux 桌面改讀 GNOME 的 `color-scheme`（或 GTK 主題名稱）與 KDE 的 `kdeglobals`，每 3 秒讀一次。Windows 的透明效果開關由 `RegNotifyChangeKeyValue` 監看（`zt_core::appearance`）。

透明效果的材質：

- **Windows 11 22H2（組建 22621）以上**：設定視窗與設定嚮導用 Mica（`DWMSBT_MAINWINDOW`）。GPUI 的 DirectComposition 畫面以透明清除（視窗沒有重新導向的介面，所以背景材質會出現在整個工作區，不必把框架延伸到工作區；延伸時 DWM 會在 App 自己的視窗按鈕上再畫一組系統按鈕）。標題列與側欄在 Mica 上只加一層淡淡的視窗色，頁面再加一層較濃的頁面色，文字以灰階抗鋸齒畫，所以兩種主題的文字對比都和不透明時接近。Mica 只在視窗是使用中的視窗時顯示，其他時候、以及沒有硬體加速的顯示卡（虛擬機的基本顯示卡）上，DWM 改畫材質的不透明底色（淺色約 #f3f3f3、深色約 #202020）。候選字視窗與中英提示由 TSF DLL 畫，系統替視窗畫圓角時（不是虛擬機的基本顯示卡）用 Acrylic（`DWMSBT_TRANSIENTWINDOW`），透明效果關閉時改畫不透明的背景（`zt_panel::style::Panel` 的 `translucent`）。
- **較舊的 Windows**：不透明，「透明效果」變暗並寫「需要 Windows 11 22H2 或更新的版本」。
- **KDE Plasma**：設定視窗與設定嚮導用 KWin 的模糊：Wayland 由 GPUI 使用 `ext-background-effect` 或 KDE 的 blur 協定，範圍是圓角的視窗本身；X11 在有合成器時設定 `_KDE_NET_WM_BLUR_BEHIND_REGION`，範圍同樣不含陰影。模糊會透出後面移動的內容，所以框架與頁面的顏色保留得較多。Linux 的候選字視窗維持不透明。
- **其他 Linux 桌面**：不透明，「透明效果」變暗並寫「只在 KDE Plasma 上提供」。

## 安裝

| 平台 | 安裝檔 | 內容 |
|---|---|---|
| Windows 10／11 x64 | `ZhuyinType-<版本>-windows-x64-setup.exe`（NSIS） | 一次 UAC；登記 64 與 32 位元的 TSF 文字服務並加入輸入法清單；登入時啟動；開始功能表「ZhuyinType 設定」；從「設定 › 應用程式」解除安裝，個人資料預設保留 |
| Debian／Ubuntu | `zhuyintype_<版本>_amd64.deb` | `/usr/bin/zhuyintype`、Fcitx5 外掛、引擎資料、登入時啟動；第一次執行時把 ZhuyinType 加入 Fcitx5 的輸入法 |
| Fedora／openSUSE | `zhuyintype-<版本>.x86_64.rpm` | 同上 |

建置都在 GPU 主機上進行（`tools/zt --remote build desktop --package`），Windows 版以 cargo-xwin 交叉編譯、`makensis` 打包。細節見 [Windows 打包](../../platforms/desktop/packaging/windows/README.md) 與 [Linux 打包](../../platforms/desktop/packaging/linux/README.md)。

## 神經模型

和 macOS 版一樣載入整句重排（v2b）與逐字模型（v3）：

- 模型是 macOS 版出貨權重（`data-5` 的 int8 檢查點）的 ONNX Runtime 動態 int8，存成 ONNX Runtime 自己的格式（資料元件 `onnx`，`data-10`；產生方式見 `ml/src/zhuyintype_ml/training/compress/desktop_models.py`）。三個檔 107 MB，和 macOS 一樣只帶前綴樹的重排模型。
- App 以 memory-map 載入模型檔並直接使用其中的權重（`session.use_ort_model_bytes_for_initializers`，不重新排列權重），所以權重留在硬碟上，系統需要時可以丟掉再讀回，和 macOS 的 Core ML 相同。App 自己的記憶體只多約 21 MB。
- 以 ONNX Runtime 1.24.4 在 CPU 上執行。App 在執行時載入它（`ort` 的 `load-dynamic`）：Windows 為安裝目錄的 `onnxruntime.dll`，Linux 為 `/usr/lib/zhuyintype/libonnxruntime.so.1`。找不到時不用模型，其餘照常。
- 判斷邏輯（重排的門檻、逐字模型的權重 0.5、候選字排序）是 `engine/crates/zhuyin-models`，和 macOS 共用；接法在 `crates/zt-core/src/models.rs`。
- 和 macOS 的差別：動態 int8 的矩陣運算以 int8 進行，開發集兩組各 400 段中有 18 段和 macOS 權重的結果不同，整句全對率口語 86.2%（macOS 權重 87.0%）、百科 81.0%（81.2%）。改用和 macOS 逐句相同的作法（權重展開成 fp32）時，App 要多用約 490 MB 記憶體，見 [研究紀錄](../research/log.md) 2026-11-17。
- GPU 主機上模型載入約 30 ms 與 55 ms，每鍵延遲中位數 4.3 ms、最慢 21 ms（沒有模型時 3.9 ms）。

## 更新

和 macOS 版的 Sparkle 相同：每天檢查一次，更新檔都經過簽章驗證，在組字結束、沒有開著設定或嚮導視窗時換成新版本。「關於」頁有「自動檢查更新」「自動安裝更新」「立即檢查」，找到新版本時列出版本與「立即安裝」。

- **清單**：發布時 `release.yml` 寫出 main 上的 `desktop-update.json`：新版本的版本號、說明與各平台安裝檔的網址、大小與 Ed25519 簽章，整份清單本身也簽章。金鑰和 macOS 的 Sparkle 相同，App 內建公鑰（`zt_core::update::PUBLIC_KEY`）。
- **安裝要系統權限**，所以使用者的 App 只檢查、顯示，閒置時請系統的更新程式執行：
  - Windows：安裝程式建立的「ZhuyinType 更新」服務（`ZhuyinTypeUpdate`，手動啟動，LocalSystem）。安全性描述元讓登入的使用者可以查詢、啟動它，此外不能做任何事。服務自己下載清單與安裝程式、驗證簽章，放在只有系統管理員能寫入的安裝資料夾 `updates\`，再以 `/S /UPDATE` 安靜安裝；安裝程式結束 App，裝好後在每個登入中的工作階段重新啟動它（`--relaunch-sessions`）。不會出現 UAC。
  - Linux：套件附的 systemd 服務 `zhuyintype-update.service`（root）與 polkit 規則，讓目前在電腦前的使用者不輸入密碼就能啟動它。它驗證後用 apt、dnf 或 zypper 安裝套件；執行中的 App 發現自己的執行檔被換掉，閒置時換成新版本。
- 更新程式不接受使用者的任何輸入，只安裝用專案金鑰簽章、版本比目前新的發布。

## 和 macOS 版不同的地方

- 私密視窗（`IS_PRIVATE`、Fcitx 的 `Sensitive`）照常打中文、不學習；密碼與 PIN 欄位不經過輸入法（macOS 的安全輸入仍可打中文）。
- 「效能」頁沒有耗電：Windows 與 Linux 沒有單一程式的耗電數字。
- 加入「在候選字視窗顯示組字的 App」時輸入執行檔名稱或選取檔案（macOS 從執行中的 App 選）。

## 尚未完成

- Wayland 的候選字視窗位置（需要合成器的輸入法 popup 介面）；目前由 Fcitx 5 顯示自己的候選字視窗。
- 程式碼簽章。
- 引擎載入期間（登入後的頭一兩秒）打的鍵直接交給程式；macOS 會先保留最多 64 個鍵。
- 替換用的資料夾（macOS 的 `Models/`）與資料更新時自動重新載入。
- 透明效果只在虛擬機（基本顯示卡，DWM 畫材質的不透明底色）與沒有合成器的 Linux 上驗證過：真正的 Mica、候選字視窗的 Acrylic 與 KDE Plasma 的模糊還要在有 GPU 的 Windows 11 與 KDE 上確認文字對比。

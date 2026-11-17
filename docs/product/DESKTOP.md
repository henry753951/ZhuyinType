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
| 主題（`theme`） | 跟隨系統、淺色、深色 | 跟隨系統 | App 畫的每個視窗：設定視窗、設定嚮導、候選字視窗與中英提示。Linux 的 Wayland 工作階段中候選字視窗與中英提示由 Fcitx 5 顯示，樣式是 Fcitx 的，不跟著主題（見「尚未完成」）。Windows 的標題列與視窗邊框（`DWMWA_USE_IMMERSIVE_DARK_MODE`）也跟著主題 |
| 透明效果（`transparency`） | 跟隨系統、開啟、關閉 | 跟隨系統 | 視窗背景透出桌面的顏色。「跟隨系統」在 Windows 依照「設定 › 個人化 › 色彩 › 透明效果」（`EnableTransparency`），在 KDE Plasma 為開啟 |
| 候選字視窗（`panelAppearance`） | 跟隨主題、淺色、深色 | 跟隨主題 | 只改候選字視窗與中英提示（Wayland 工作階段中由 Fcitx 5 顯示，不受這個設定影響），和 macOS 的「外觀」是同一個設定與值（`system` 在這裡表示跟隨主題） |

改設定立即套用，不需要重新開啟。「跟隨系統」時也即時跟著系統改變：淺色或深色由 GPUI 跟著 Windows（`AppsUseLightTheme`，`WM_SETTINGCHANGE` 的 `ImmersiveColorSet`）與 XDG desktop portal 的 `org.freedesktop.appearance` `color-scheme`；沒有 portal 的 Linux 桌面改讀 GNOME 的 `color-scheme`（或 GTK 主題名稱）與 KDE 的 `kdeglobals`，每 3 秒讀一次。Windows 的透明效果開關由 `RegNotifyChangeKeyValue` 監看（`zt_core::appearance`）。

透明效果的材質：

- **Windows 11 22H2（組建 22621）以上**：設定視窗與設定嚮導用 Mica（`DWMSBT_MAINWINDOW`）。GPUI 的 DirectComposition 畫面以透明清除（視窗沒有重新導向的介面，所以背景材質會出現在整個工作區，不必把框架延伸到工作區；延伸時 DWM 會在 App 自己的視窗按鈕上再畫一組系統按鈕）。標題列與側欄在 Mica 上只加一層淡淡的視窗色，頁面再加一層較濃的頁面色，文字以灰階抗鋸齒畫，所以兩種主題的文字對比都和不透明時接近。Mica 只在視窗是使用中的視窗時顯示，其他時候、以及沒有硬體加速的顯示卡（虛擬機的基本顯示卡）上，DWM 改畫材質的不透明底色（淺色約 #f3f3f3、深色約 #202020）。候選字視窗與中英提示由 TSF DLL 畫，系統替視窗畫圓角時（不是虛擬機的基本顯示卡）用 Acrylic（`DWMSBT_TRANSIENTWINDOW`），透明效果關閉時改畫不透明的背景（`zt_panel::style::Panel` 的 `translucent`）。
- **較舊的 Windows**：不透明，「透明效果」變暗並寫「需要 Windows 11 22H2 或更新的版本」。
- **KDE Plasma**：設定視窗與設定嚮導用 KWin 的模糊：Wayland 由 GPUI 使用 `ext-background-effect` 或 KDE 的 blur 協定，範圍是圓角的視窗本身；X11 在有合成器時設定 `_KDE_NET_WM_BLUR_BEHIND_REGION`，範圍同樣不含陰影。模糊會透出後面移動的內容，所以框架與頁面的顏色保留得較多。Linux 的候選字視窗維持不透明。
- **其他 Linux 桌面**：不透明，「透明效果」變暗並寫「只在 KDE Plasma 上提供」。

## 無障礙

候選字視窗的顏色由 `zt_panel::a11y::resolve` 決定，Linux 的 GPUI 視窗、設定的預覽與 Windows 的 TSF DLL 共用。順序是：高對比，其次強調色，最後 ZhuyinType 自己的配色。

| 系統設定 | Linux | Windows | 候選字視窗 |
|---|---|---|---|
| 高對比 | XDG desktop portal `org.freedesktop.appearance` 的 `contrast`，或 GNOME 的 `org.gnome.desktop.a11y.interface high-contrast` | `SPI_GETHIGHCONTRAST`（`HCF_HIGHCONTRASTON`），顏色取自 `GetSysColor`（視窗、文字、醒目提示、醒目提示文字、灰色文字） | 系統的顏色（GNOME：白底黑字或黑底白字、藍色反白），不透明，不論透明效果與強調色；邊框與反白外框 2 px，所有字用文字色 |
| 強調色 | portal 的 `accent-color`，或 GNOME 47 起的 `accent-color`（名稱對應 libadwaita 的顏色），或 KDE `kdeglobals` 的 `[General] AccentColor`（沒有時用配色的 `[Colors:Selection] BackgroundNormal`） | `HKCU\Software\Microsoft\Windows\DWM` 的 `AccentColor`，沒有時用 `DwmGetColorizationColor` | 「強調色」為「跟隨系統」時，反白是強調色的實心圓角矩形，上面是白字 |

- **強調色的對比**：反白上的字一律是白色，和反白至少 4.5:1（WCAG 2 的相對亮度）。強調色不夠深時，淺色與深色視窗都逐步加深（每步 8%，最多 6 步），仍不夠就改用 ZhuyinType 藍（#4257D8）。反白靠底色和白字一起和其他列區分，所以只在高對比時要求反白和視窗背景至少 3:1（高對比另有外框）。下方開著的開關底色是淡淡的強調色；設定視窗的連結與焦點外框用調整到和背景 4.5:1 的強調色。
- **設定視窗**：「跟隨系統」時開關、焦點外框、連結與到達提示也用系統強調色（同樣調整到看得清楚）。
- **「ZhuyinType 藍」**或系統沒有強調色時，候選字視窗維持原本的中性灰反白，設定視窗的強調色是 ZhuyinType 藍。
- **跟著系統改變**：TSF DLL 在候選字視窗收到 `WM_SETTINGCHANGE`、`WM_SYSCOLORCHANGE`、`WM_THEMECHANGED` 或 `WM_DWMCOLORIZATIONCOLORCHANGED` 時重新讀取並重畫，所以改 Windows 的強調色或高對比時，開著的清單立即換色。Linux 每 3 秒讀一次（`zt_core::system_look`），改變時重畫所有視窗。設定視窗在 Windows 上讀登錄檔的 `AccentColor` 與 `Control Panel\Accessibility\HighContrast`（設定預覽用）。
- **螢幕報讀**：
  - Windows：候選字視窗與中英提示各有一個 UI Automation provider（`WM_GETOBJECT`），清單的類型是 List、名稱是反白的候選字，提示是 Text。有用戶端在聽時（`UiaClientsAreListening`）發出通知事件：開啟清單「ㄋㄧˇ，候選字清單，共 21 個，3 頁，候選字 1，你」、翻頁「第 2 頁，共 3 頁，候選字 1，……」、移動反白「候選字 2，妳」、新的提示，以及中英切換「中文」「英文」與「全形標點」「半形標點」。UI Automation 的程式庫只在有用戶端要求或螢幕報讀程式執行時才載入，沒有螢幕報讀時每次更新只多一次檢查。
  - Linux：GPUI 沒有 AT-SPI 介面，所以 ZhuyinType 自己的候選字視窗不能被報讀。偵測到螢幕報讀程式（AT-SPI 的 `org.a11y.Status` `ScreenReaderEnabled`，或 GNOME 的 `screen-reader-enabled`）時，改由 Fcitx 5 顯示清單（和 Wayland 工作階段相同），它的清單可以被 Orca 讀出。
- **字級**：13–36 pt（三個平台相同），沒有另外的「大字」設定。

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
- 模型在背景執行緒載入，和詞庫、語言模型同時進行，載入後各執行一次，第一個鍵不必等 ONNX Runtime 第一次執行與讀入權重。四核心以上的電腦每個模型用兩個執行緒（執行緒在運算之間休眠，不空轉），結果和一個執行緒相同。
- GPU 主機上服務啟動到可以接鍵約 135 ms（模型載入約 50 ms 與 60 ms）。桌面版的引擎路徑打開發集四組各 400 段：每鍵中位數 0.45–0.6 ms，95% 的鍵在 20–28 ms 內，最慢 45–68 ms；完成一個音節的鍵才會執行模型。數字與量法見[研究紀錄](../research/log.md) 2026-11-18。

## 更新

和 macOS 版的 Sparkle 相同：每天檢查一次（檢查失敗時一小時後再試；電腦時鐘曾經被調快時不會因此停止檢查），更新檔都經過簽章驗證，在組字結束、沒有開著設定或嚮導視窗時換成新版本。系統的更新程式啟動一小時後 App 仍是舊版（套件管理員正忙、下載失敗），下一次檢查後會再試一次。「關於」頁有「自動檢查更新」「自動安裝更新」「立即檢查」，找到新版本時列出版本與「立即安裝」。

- **清單**：發布時 `release.yml` 寫出 main 上的 `desktop-update.json`：新版本的版本號、說明與各平台安裝檔的網址、大小與 Ed25519 簽章，整份清單本身也簽章。金鑰和 macOS 的 Sparkle 相同，App 內建公鑰（`zt_core::update::PUBLIC_KEY`）。
- **安裝要系統權限**，所以使用者的 App 只檢查、顯示，閒置時請系統的更新程式執行：
  - Windows：安裝程式建立的「ZhuyinType 更新」服務（`ZhuyinTypeUpdate`，手動啟動，LocalSystem）。安全性描述元讓登入的使用者可以查詢、啟動它，此外不能做任何事。服務先確認安裝資料夾只有系統管理員能修改，自己下載清單與安裝程式、驗證簽章，放在安裝資料夾的 `updates\`，執行前鎖住檔案再驗證一次，以 `/S /UPDATE` 安靜安裝；安裝程式結束 App，裝好後（失敗也一樣）在原本執行它的每個工作階段、以該使用者身分重新啟動它（`--relaunch-sessions`）。不會出現 UAC。
  - Linux：套件附的 systemd 服務 `zhuyintype-update.service`（root）與 polkit 規則，讓目前在電腦前的使用者不輸入密碼就能啟動它（遠端桌面等規則不適用的情況，自動安裝不會跳出密碼視窗，按「立即安裝」才可能要求密碼，等待輸入密碼時輸入法照常使用）。它驗證後用 apt、dnf 或 zypper 安裝套件；執行中的 App 發現自己的執行檔被換掉，閒置時換成新版本。
- 更新程式不接受使用者的任何輸入，只安裝用專案金鑰簽章、版本比目前新的發布。

## 輸入法服務不中斷

提供輸入法的是常駐的 `zhuyintype --background`（登入時啟動），關閉設定視窗或嚮導都不會結束它。已經有一份在執行時，再啟動的 `--background`（例如登入時的自動啟動和 Fcitx 5 的 addon 同時啟動）直接結束，不會開啟設定視窗。設定視窗打不開（例如沒有可用的顯示卡）時只記錄錯誤，服務照常執行。介面執行緒發生無法處理的錯誤時程式會結束，這時先啟動一份新的 `--background --after-crash`，等舊的結束後接手（啟動後一分鐘內的錯誤不重啟，避免一直重來）。

## 候選字的字詞動作

候選字視窗的每一列可以按右鍵，選單有「不要再推薦」「忘記這個選字」「加入自訂詞」，這一列用不到的項目變灰。鍵盤也可以：Ctrl+Delete 不要再推薦、Ctrl+Shift+Delete 忘記這個選字、Ctrl+Return 加入自訂詞（反白的字詞；沒開清單時是整段組字），可在「快捷鍵」改，只在組字時有作用，沒在組字時按鍵照常交給程式。做完後視窗的提示列顯示結果（「不再推薦「八」」）與「復原」，5 秒內點「復原」或按 Ctrl+Z 就還原。前 5 次開清單時，清單下方有一行「Ctrl+Del 不要再推薦・右鍵看更多」（次數記在 `wordActionHintCount`）。設為不要再推薦的字詞列在「字詞」→「記住的字詞」，標示「不推薦」，移除就恢復推薦。

「選取的字加入自訂詞」（快捷鍵 › 編輯，預設沒有按鍵）：在 App 裡選取 1 到 6 個字再按它的鍵，設定視窗開在「字詞」→「自訂詞」，「加入自訂詞」已填好選取的字與第一個讀音，可以修改；字數或讀音不對時，表單上方直接說明（例如「讀音的音節數要和字數相同（2 個字，1 個音節）」），沒選取時表單是空的並顯示「沒有選取文字」。組字時按這個鍵沒有作用；密碼與私人欄位不讀取選取的文字。

Linux（X11）的選單畫在候選字視窗裡，視窗不取得焦點；Windows 的 TSF DLL 用系統選單，同樣不讓應用程式失去焦點。Wayland 工作階段的清單由 Fcitx 5 顯示，沒有右鍵選單與「復原」，但按鍵照常可用。

## 和 macOS 版不同的地方

- 私密視窗（`IS_PRIVATE`、Fcitx 的 `Sensitive`）照常打中文、不學習；密碼與 PIN 欄位不經過輸入法（macOS 的安全輸入仍可打中文）。
- 「效能」頁沒有耗電：Windows 與 Linux 沒有單一程式的耗電數字。
- 加入「在候選字視窗顯示組字的 App」時輸入執行檔名稱或選取檔案（macOS 從執行中的 App 選）。
- Linux（X11）的中英提示出現時以 0.09 秒淡入，並向游標方向移動 3 px（「減少動態效果」時只淡入）；macOS 版直接出現。
- 設定視窗可以只用鍵盤操作：Tab 在側邊欄、可點的列、分段選項、連結與按鍵欄位之間移動，取得焦點的項目以強調色外框標出，空白鍵或 Enter 執行；在按鍵上按 Enter 改按鍵、⌫ 刪除，錄製時 Esc 取消後焦點回到原來的按鍵。

## 快速設定

「開啟快速設定」快捷鍵（「快捷鍵」頁「其他」，預設沒有按鍵）與輸入法選單都有「英文」「半形標點」「簡體」「暫停學習」四個勾選項目：

- Linux（X11）：快捷鍵在游標旁開一個小視窗，標題「快速設定」，四個勾選項目之後是「設定…」與「自訂詞與學到的詞…」。視窗不取得焦點，按鍵仍交給原本的程式；點一個項目、按下一個鍵或換欄位就關閉。Fcitx 5 的輸入法選單（系統匣與狀態列）在原有的項目之前列出同樣四個勾選項目；簡體輸出時狀態標示是「简」，暫停學習時狀態說明加上「（暫停學習）」。
- Windows：見 [ARCHITECTURE.md](../../platforms/desktop/ARCHITECTURE.md) 的 TSF 說明（中／英按鈕的選單與游標旁的選單）。
- 暫停學習期間每個欄位都當成私人欄位（照常打字，不學習、不讀取前文、不記錄改字），服務重新啟動後恢復學習。

## 符號與 Emoji

按 Ctrl+Alt+,（「快捷鍵」可改；「打字 › 標點與符號 › 符號與 Emoji 面板」可關閉）開啟符號與 Emoji 面板，正在打字的 App 保持焦點。方向鍵移動、PgUp／PgDn 翻一頁、Tab／Shift+Tab 換類別、1–9 選反白那一列的第幾個、Return（或沒有搜尋時的空白）送出、Esc 先清除搜尋再關閉，再按一次快捷鍵也會關閉；其他鍵打進搜尋，可打注音（標準鍵盤）或英文。組字中開啟時，目前的字成為搜尋內容，組字取消不送出。資料與搜尋在引擎的 `zhuyin-symbols`，和 macOS 相同。

Linux（X11）在候選字視窗畫完整的格子：搜尋列、類別、每列 9 個、反白符號的名稱，可以點選。Windows 與 Linux 的 Wayland 工作階段由輸入法自己畫候選字清單（Windows 能出現在開始功能表的搜尋框上），面板以反白的那一列（1–9）呈現，類別與列數在提示裡。Windows 上 Ctrl+Alt 是 AltGr，沒有組字時文字服務原本不把它交給程式；回覆中的 `claims` 列出面板開著時要取得的按鍵（見 `platforms/desktop/IPC.md`）。

最近使用的符號（最多 32 個）存在使用者資料夾的 `recent-symbols.txt`；「回復預設設定」會刪除它，Windows 解除安裝時勾選刪除個人資料也會刪除。

## 尚未完成

- Windows 的符號與 Emoji 面板由文字服務畫完整的格子（目前只畫反白的那一列）。
- Wayland 工作階段裡「開啟快速設定」快捷鍵還沒有游標旁的清單（App 不能在 Wayland 上擺放自己的視窗）；請用 Fcitx 5 輸入法選單裡的四個勾選項目。X11 上點小視窗以外的地方不會關閉它，要按下一個鍵或換欄位。
- Windows 的 TSF 候選字清單介面（`ITfCandidateListUIElement`）：自己畫候選字清單的 App（UI-less 模式）拿不到清單。UI Automation 也還沒有每個候選字一個項目的樹狀結構，只有清單本身與通知事件。
- 高對比時只有候選字視窗與中英提示改用系統顏色，設定視窗與設定嚮導維持原本的配色。
- Wayland 上 ZhuyinType 自己的候選字視窗（需要合成器的輸入法 popup 介面）。Wayland 工作階段中候選字清單、注音、提示與中／英提示改由 Fcitx 5 顯示（它的 classicui，或 KDE、GNOME 的輸入法面板），樣式是 Fcitx 的。
- 程式碼簽章。
- 引擎載入期間（登入後的頭一兩秒）打的鍵直接交給程式；macOS 會先保留最多 64 個鍵。
- 替換用的資料夾（macOS 的 `Models/`）與資料更新時自動重新載入。
- 透明效果只在虛擬機（基本顯示卡，DWM 畫材質的不透明底色）與沒有合成器的 Linux 上驗證過：真正的 Mica、候選字視窗的 Acrylic 與 KDE Plasma 的模糊還要在有 GPU 的 Windows 11 與 KDE 上確認文字對比。

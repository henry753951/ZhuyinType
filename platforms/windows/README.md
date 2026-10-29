# Windows TSF 輸入法

`zhuyintype-tsf.dll` 是 Windows x64 的 COM 文字服務。Windows 將 DLL 載入文字應用程式；DLL 負責按鍵、焦點、TSF 組字與文字送出，共用 Qt 服務負責引擎、設定、快捷鍵與候選字視窗。通訊契約見 [../qt/IPC.md](../qt/IPC.md)，實測範圍見 [VALIDATION.md](VALIDATION.md)。

## 建置與開發安裝

需要 Windows x64、Visual Studio 2022 C++ Build Tools、Windows 10 SDK 與 CMake 3.24 以上。先依 [Qt README](../qt/README.md) 建置並部署共用桌面服務與引擎，準備 `data\build\lexicon.tsv`。

從版本庫根目錄執行：

```powershell
# 原生 DLL 與測試，使用獨立建置目錄。
.\platforms\windows\build.ps1

# 註冊文字服務、加入目前使用者的輸入法清單及登入啟動。
.\platforms\windows\install-dev.ps1
```

腳本預設使用 `C:\BuildTools`；不同位置可設定 `ZT_BUILD_TOOLS`，或先進入 x64 Developer PowerShell。服務預設位置為 `.build-qt\Release\zhuyintype-desktop.exe`，詞庫為 `data\build\lexicon.tsv`。開發時可明確指定：

```powershell
.\platforms\windows\install-dev.ps1 `
  -HostExe C:\src\ZhuyinType\.build-qt\Release\zhuyintype-desktop.exe `
  -Lexicon C:\src\ZhuyinType\data\build\lexicon.tsv
```

VM／遠端桌面若硬體繪圖造成首次候選字視窗逾時，可加 `-SoftwareRendering`。這只在該登入啟動腳本設定 `QT_QUICK_BACKEND=software`。測試 VM 使用此選項。

COM 預設登記於 `HKCU\Software\Classes`。TSF profile 與類別透過 Windows API 登記；實測 Windows Server 2025 會寫入 HKLM，因此該系統安裝與移除時須以目前使用者身分提升權限。此版本不能宣稱所有系統均可完全免管理員安裝。`build.ps1 -Install -Machine` 可明確登記系統層級 COM，移除時須用 `zt-tsf-register uninstall --machine`；一般開發安裝使用預設 HKCU COM。

安裝會保留既有語言、輸入法與預設選取，只把自己的 TIP 加入繁體中文清單。使用 Win+空白鍵選取 ZhuyinType 注音。新增的登入啟動項目是 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\ZhuyinTypeInputHost`；啟動腳本位於 `%LOCALAPPDATA%\ZhuyinType\Start-InputHost.ps1`，以絕對路徑傳入服務與詞庫，工作目錄為服務目錄。登記後可直接執行該腳本啟動，或下次登入時啟動。服務的單一實例鎖會處理重複啟動。

原生 DLL 安裝到 `%LOCALAPPDATA%\ZhuyinType\Tsf\<DLL SHA256 前 16 碼>`。更新時，已開啟的應用程式會繼續使用原來的 DLL，結束並重新開啟後才載入新版。相同版本重裝會保留已載入的檔案。TSF profile 使用服務 EXE 的第 0 個圖示。

```powershell
.\platforms\windows\install-dev.ps1 -Uninstall
```

移除只處理本服務的使用者清單、COM／TSF 登記與登入啟動項目。服務行程、已載入 DLL 的應用程式與使用者引擎資料由使用者自行結束或保留；版本目錄保留，方便復原。

## 原生端行為

每個 TSF context 使用獨立識別碼。切換文件時先對舊文件送出 `blur` 並在舊文件完成組字，再建立新 context。按鍵先取得同步 TSF 寫入 edit session，鎖定失敗時交回應用程式。焦點、輪詢與結束組字使用 TSF 可排程的 edit session，保留 COM 生命週期與 context 世代檢查。`OnTestKeyDown`／`OnTestKeyUp` 只做本機預測；實際 broker 與文字變更發生在正式按鍵處理。

預設 pipe 是 `\\.\pipe\ZhuyinType-ime-v1-<使用者 SID>`。`ZT_IME_SOCKET` 可指定完整本機 pipe，遠端 pipe 會被拒絕。JSON 使用 Windows 內建 `Windows.Data.Json`；原生 DLL 使用靜態 C++ runtime，沒有 Qt 或引擎 DLL 依賴。

每次交易最多等候 200 ms；忙碌 pipe 的連線等待上限為 10 ms，失敗後至少 250 ms 才再嘗試。逾時會取消 I/O、關閉連線並交回原鍵；已送出的按鍵不重送。模糊失敗時保留已顯示的正常組字為文字，安全欄位則清除組字。核心 I/O 取消完成與被呼叫的系統／GUI callback 受 Windows 排程影響，200 ms 是交易的名義期限，不是硬即時保證。

只有目前聚焦且已連線的 context 以約 20 ms 輪詢候選字滑鼠操作結果。所有回應核對版本、識別碼、context、UTF-8、欄位型別、256 KiB 上限與 UTF-16 游標邊界；文字只套用到原 context。

若 pipe server PID 是目前行程，DLL 才查找 EXE 的 `ZtProcessImeEvents`，在等候期間呼叫它處理 Qt 的公開事件迴圈。該 callback 使用 `ExcludeUserInputEvents`；原生端另有交易重入與焦點世代保護。這讓服務自己的 Qt 輸入欄位可透過相同 pipe 使用引擎。DLL 仍以 Win32 pipe 通訊，不連結 Qt，也不依賴 Qt 私有訊息常數。

安全判定涵蓋 TSF secure activation、唯讀／停用 context、標準 Edit／RichEdit 的 `ES_PASSWORD`、InputScope 的密碼、PIN 與私人欄位。安全欄位只送安全狀態，不序列化實際按鍵或文字。離開安全狀態時同步實際 Caps Lock；首鍵是 Caps Lock 時避免同步後再切換一次。一般焦點取得也會同步 Caps Lock。前文讀取目前未實作，原生端不記錄使用者輸入。

游標使用 `GetTextExt`，無法取得時嘗試 Windows caret。先依來源視窗 DPI context 正規化為實體座標，再依螢幕原點換算成 Qt 邏輯座標：`origin + (physical - origin) / scale`，寬高除以 scale，送出 `scale: 1`。

工作列右下角的輸入指示器有兩格 Windows 風格的單色圖示：

- 左格「中」／「英」：TSF 的輸入模式按鈕（`GUID_LBI_INPUTMODE`，`src/InputModeButton.cpp`），顏色跟著工作列深淺色。按一下切換中英（IPC 的 `mode`），按右鍵開啟設定；組字中與安全欄位不切換。
- 右格 ZhuyinType 標誌：輸入法本身的圖示（`src/logo.ico`），灰色方形微圓角、挖空「ㄓ」，深淺色工作列都看得清楚。

開啟設定：在左格按右鍵選「ZhuyinType 設定…」，或從開始功能表搜尋「ZhuyinType 設定」（`install-dev.ps1` 建立的捷徑，開啟執行檔時執行中的服務會顯示設定視窗）。

## 驗證

`build.ps1` 會執行 `pipe-contract`。互動測試需要已登入且可接收鍵盤滑鼠的桌面，不能以 Session 0 的 SSH 程序當成 GUI 驗證：

```powershell
# 在互動桌面啟動已部署服務後，依序執行。
.\platforms\windows\test-session.ps1 -Mode popup
.\platforms\windows\test-session.ps1 -Mode stock
.\platforms\windows\test-session.ps1 -Mode unavailable
```

同執行緒測試額外連結共用 `desktop_state.lib`，只在測試 EXE 使用 Qt。先在 x64 Developer PowerShell 設定原生建置，再執行：

```powershell
cmake -S platforms/windows -B .build-windows-tsf -A x64 `
  -DCMAKE_PREFIX_PATH=C:/Qt/6.10.2/msvc2022_64 `
  -DZT_QT_STATE_LIBRARY=C:/src/ZhuyinType/.build-qt/Release/desktop_state.lib
.\platforms\windows\build.ps1
.\platforms\windows\test-selfhost.ps1
```

測試腳本預設主建置目錄 `.build-qt`，VM 的暫存建置須明確傳 `test-host.ps1 -BuildDir .build-qt-next` 或 `test-selfhost.ps1 -HostBuildDir .build-qt-next`。輸出在 `.build-windows-tsf\evidence`，只含判定、計數與延遲，沒有輸入文字。

安裝保留性測試會移除並重裝本服務，應在開發 VM 執行：

```powershell
.\platforms\windows\tests\InstallDev.ps1 -HostExe C:\src\ZhuyinType\.build-qt\Release\zhuyintype-desktop.exe
```

Windows COM／TSF API 依 Microsoft 文件實作：[文字服務登記](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile)、[類別登記](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcategorymgr-registercategory)、[edit session](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)、[鍵盤事件](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfkeyeventsink)。

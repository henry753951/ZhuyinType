# Windows TSF 驗證紀錄

日期：2026-10-29。範圍為 `platforms/windows` 原生介面與共用 Qt broker 的實際整合；未改動介面外觀。

## 環境與產物

- Windows Server 2025 Standard Evaluation，10.0.26100，x64，已登入的遠端桌面，縮放 100%。
- Visual Studio 2022 Build Tools 17.14.43，MSVC 19.44.35229，Windows SDK 10.0.26100，Qt 6.10.2。
- 原生獨立建置目錄 `.build-windows-tsf`；共用服務使用明確指定的 `.build-qt-next\Release`。正式腳本預設仍為 `.build-qt\Release`。
- 實測使用 `QT_QUICK_BACKEND=software`。硬體後端首次顯示候選字視窗曾使回應超時，故本紀錄不將該後端列為通過。
- 共用 host 已重新建置並部署，`dumpbin /exports` 確認實際 `zhuyintype-desktop.exe` 含 `ZtProcessImeEvents`。
- 原生 DLL 的 `dumpbin /exports` 只有 `DllGetClassObject`、`DllCanUnloadNow`、`DllRegisterServer`、`DllUnregisterServer`；暫時診斷匯出已移除。`dumpbin /dependents` 只有 Windows 系統相依，沒有 Qt、引擎或動態 MSVC runtime。

## 實際輸入結果

| 測試 | 結果與觀察方式 |
|---|---|
| 真正 TSF 登記與啟用 | PASS。使用 Windows profile API 登記，`ActivateProfile` 回傳 S_OK，前景 key service CLSID 相符，Windows 載入已登記的 DLL。COM 預設 HKCU；此 VM 的 TSF profile／categories 位於 HKLM。 |
| Windows 標準 EDIT | PASS。實際 `SendInput`，標準 `TranslateMessage`／`DispatchMessage` 迴圈。觀察到 IMM 組字與 result 通知；Enter 後文字與實際 CJK 組字完全相等且只送出一次。 |
| Windows RichEdit | PASS。Windows `msftedit.dll` 控制項與實際 `SendInput`。原生 TSF 組字可由控制項觀察，Enter 後精確比對送出文字。沒有呼叫服務的 `OnKeyDown` 或測試用 `ITfKeystrokeMgr` 分派。 |
| 真正 Qt 候選字視窗 | PASS。自訂 TSF text store 呼叫 TSF keystroke manager；UI Automation 讀取真正 Qt 視窗與按鈕，`SendInput` 滑鼠操作上下頁及選字。游標錨定誤差在 2 px 內，點擊後保留原生焦點，由 poll 送入原文件且沒有重複送出。 |
| 文件／edit session | PASS。組字、Enter、`GetTextExt`、重複 TestKey 無文字變更、拒絕寫入鎖時交回按鍵、舊文件 blur 完成組字、新文件沒有遲到文字。 |
| 安全欄位 | PASS。自訂 text store 的 password InputScope 交回按鍵並保持無組字；標準 EDIT 的 `ES_PASSWORD` 收到原始合成測試按鍵。離開安全狀態後恢復正常輸入。 |
| 同行程 Qt 輸入 | PASS。隔離的真實 Qt `TextInput` 與共用 `EngineSession`／`InputController`／`ImeHost` 在同一個 GUI 執行緒。Windows `SendInput`、實際已登記 DLL；先觀察組字，Enter 後文字精確等於測試預期、preedit 為空且 commit 計數為 1。測試強制要求 callback 匯出存在。 |
| 服務不存在 | PASS。已登記文字服務接入不存在的隔離 pipe，原鍵交回應用程式，原控制項無變更，該測試耗時小於 150 ms。此項透過 TSF keystroke manager 驅動。 |
| pipe-contract | PASS。分段 UTF-8、識別碼／context／版本／型別錯誤、UTF-16 surrogate 邊界、損壞 JSON／UTF-8、額外 frame、超長 frame、逾時取消、立即正常重連、忙碌時重入拒絕與保留鍵盤滑鼠佇列。 |

標準 EDIT 與 RichEdit 測試在專用 Win32 測試應用程式內建立 Windows 控制項，屬於 Windows 實際分派的按鍵路徑。候選字／文件／鎖測試則使用自訂 `ITextStoreACP`，透過 `ITfKeystrokeMgr` 驅動；兩種證據分開記錄。

## 同行程問題的修正證據

原本僅過濾原生訊息的等候方式，在 Qt 同 GUI 執行緒時失敗：引擎已處理按鍵，pipe 回覆仍等候 Qt posted events，直到交易逾時而斷線。加入公開 Qt 事件 callback 後，同一測試觀察到組字與單次 commit，所有診斷 key／poll failure 計數為 0。移除診斷後以更嚴格的精確文字與空 preedit 條件再次通過。

DLL 只在 `GetNamedPipeServerProcessId` 相符時解析主 EXE 的 callback；其他應用程式仍走普通 bounded pipe 等候。callback 使用公開 `QCoreApplication::processEvents(ExcludeUserInputEvents)`。來源參考：[Qt 6.10.2 Windows dispatcher](https://github.com/qt/qtbase/blob/v6.10.2/src/corelib/kernel/qeventdispatcher_win.cpp)、[Qt 事件迴圈](https://doc.qt.io/qt-6/qcoreapplication.html#processEvents)。

這證明共享 broker 的同執行緒路徑；完整 Main 設定頁、自訂詞表單與搜尋欄位沒有在本次測試逐一操作。

## 期限與延遲

正式原生端交易期限為 200 ms，pipe busy 等候 10 ms，失敗重試間隔 250 ms。逾時測試伺服器刻意延後 600 ms，最後一輪測到 client 在 204 ms 返回，測試另限制 client 呼叫須介於 150–450 ms；取消 I/O 與 GUI callback 仍受系統排程影響。

最後一輪移除診斷後的候選字整合測試，40 個按鍵的 pipe 往返加 TSF edit session：p50 1.006 ms、p95 1.570 ms、p99 1.642 ms。這是暖機後的 VM 小樣本，不代表首次啟動、所有應用程式或硬體的延遲。

## 安裝與登入啟動

`tests/InstallDev.ps1` 在實際 VM 執行開發安裝、重複加入 profile、移除 profile、完整卸載、重裝，以及不存在 host／故意 CMake 失敗兩條路徑。比對既有語言與其他輸入法順序、預設輸入法 override、其他 Run 值，均通過。登入啟動腳本含指定的服務、絕對詞庫路徑與 VM 專用 software backend；建置失敗時沒有新增 Run 或改動 profile。

保留既有輸入法與預設選取，使用者以 Win+空白鍵選取本服務。正式登入啟動項目為 `ZhuyinTypeInputHost`；測試任務只用於在互動桌面執行，交付時移除。完整登出／重新登入未執行；以同一個已登記 launcher 在互動桌面啟動，PID 4424、session 2，並重新通過全部原生整合測試。

## 重現與證據

原始輸出在開發 VM 的 `.build-windows-tsf\evidence`：`native-stock.txt`、`native-popup.txt`、`native-unavailable.txt`、`selfhost.txt` 及各自 `.exit`／`.err`；另有 `pipe-contract.txt`、`installation.txt`、`dll-exports.txt`、`dll-dependencies.txt`。輸出只記錄判定、長度、計數與時間，沒有實際輸入文字。

```powershell
.\platforms\windows\build.ps1 -Install -HostExe "$pwd\.build-qt-next\Release\zhuyintype-desktop.exe"
# 下列指令在同一個已登入的互動桌面依序執行。
.\platforms\windows\test-session.ps1 -Mode popup
.\platforms\windows\test-session.ps1 -Mode stock
.\platforms\windows\test-session.ps1 -Mode unavailable
.\platforms\windows\test-selfhost.ps1 -HostBuildDir .build-qt-next
.\platforms\windows\tests\InstallDev.ps1 -HostExe "$pwd\.build-qt-next\Release\zhuyintype-desktop.exe"
```

## 尚未驗證的範圍

- Notepad、其他第三方應用程式與 32 位元程式；本產物只有 x64。
- 實體多螢幕、混合 DPI 與 DPI-unaware 應用程式；螢幕原點保留／負座標只有純計算測試，實際 VM 為單螢幕 100%。
- 完整 Main 設定頁與外部 UI 測試的 LibraryPages timeout；後者屬於共用 UI 的另外一項結果，不由本次原生整合通過推論。
- 非管理員帳號安裝、Windows 10／11、AppContainer／UI-less 模式與系統安全桌面；限制啟用模式會交回按鍵。
- 實際 Caps Lock 在安全欄位內切換後離開的完整合成鍵盤序列；程式有同步路徑，一般安全欄位離開已驗證。
- 服務執行途中 crash／重啟的完整 GUI 測試；本次故障證據為不存在的 endpoint、慢回覆、斷線與重連測試。
- 游標前文讀取尚未實作；原生端目前也未送出 `app` 分組欄位。共用服務的相關能力不等於此 adapter 已提供這些資料。

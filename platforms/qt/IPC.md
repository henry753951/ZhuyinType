# 原生輸入法與共用介面

Fcitx5／TSF 負責系統按鍵、焦點、安全輸入、組字範圍、游標矩形與文字送出。`zhuyintype-desktop --ime-host` 持有共用 `EngineSession`、`InputController`、`Settings` 與 `Shortcuts`，並顯示 `ImePopup.qml` 內的 `CandidatePanel`。設定介面與真正輸入法使用同一個行程和同一份狀態；再次開啟執行檔會要求既有服務顯示設定。

## 通訊

使用僅目前使用者可存取的本機通訊端。Linux 為 `$XDG_RUNTIME_DIR/zhuyintype/ime-v1`，未設定或不是絕對路徑時為 `/tmp/zhuyintype-<uid>/ime-v1`；目錄須由目前使用者擁有，權限為 0700。Windows 為 `\\.\pipe\ZhuyinType-ime-v1-<使用者 SID>`。服務以鎖定檔避免同時啟動兩個實例；引擎與 QML 載入後才開始接受連線，避免首鍵送進尚未就緒的服務。測試可用 `ZT_IME_SOCKET` 或服務的 `--ime-socket` 指定隔離通訊端。

每個封包是一行 UTF-8 JSON，上限 256 KiB。請求包含 `v: 1`、連線內單調遞增的正整數 `id`、最長 128 字元的 `context`、`op`。回應包含相同版本、識別碼與欄位識別。

| op | 內容 |
|---|---|
| `focus` | `secure` 布林，可選的 `rect: [x,y,width,height]`、`scale`、`capsLock`、`inlinePreedit`、`app` 與 `before`；取得焦點 |
| `key` | `key`／`modifiers` 為 Qt 數值、`release`／`secure` 布林，可選 `repeat`、`shiftSide`（0 未知、1 左、2 右）、`app`、`before` 及矩形 |
| `context` | 只更新 `app` 與 `before`（見〈前文〉），不處理按鍵 |
| `poll` | 取得滑鼠選字後待送出的文字與最新組字，可更新矩形 |
| `blur` | 結束目前組字並回傳待送出文字，解除焦點 |
| `reset` | 丟棄目前組字與尚未送出的文字 |
| `secure` | `secure` 布林；進入安全輸入時立即清除組字與待送出文字，暫停學習 |
| `show-settings` | 顯示既有設定視窗 |

成功的輸入回應含 `ok`、`handled`、`commit`、`preedit`、UTF-16 `cursor`、`candidates`、`english`。失敗回應含 `ok: false` 與錯誤說明。每份 `commit` 只在一個回應中取出；沒有未經請求的封包。原生介面必須核對識別碼與欄位，不能把舊欄位回應送入新欄位，也不能重送超時的按鍵。安全欄位不傳輸實際按鍵或文字。

同一時間只有一個取得焦點的欄位可操作組字。轉移焦點前，原生介面先處理舊欄位的 `blur` 回應，再送出新欄位的 `focus`。舊欄位的遲到請求會被拒絕。斷線清除服務端狀態；原生端僅能保留同一個仍聚焦且非安全欄位的已確認組字，不得重送未確認的按鍵。沒有輸入文字的持久化通訊記錄。

## 前文

和 macOS `ControllerCore` 相同，組字開始時引擎讀取游標前的文字（「參考游標前的文字」關閉時不讀）：

- `app`（最長 256 字元）：欄位所屬的程式，例如 Fcitx 的 `program()`、Windows 的執行檔名稱。只用來分開各程式送出過的文字，不寫入任何檔案或記錄。
- `before`（最長 1024 個 UTF-16 單位）：平台回報的游標前文字；服務只取最後 128 個字元。不知道時省略欄位，不要送空字串（空字串代表在文件開頭）。

沒有 `before` 時，服務改用同一個 `app` 在 10 分鐘內送出的文字（最多保留 255 個字元）；送出的字、交給程式的空白、換行與刪除都會更新這段文字。換程式、安全輸入或超過 10 分鐘就清空。安全欄位不傳 `before`，服務也不保留任何文字。Fcitx adapter 在欄位支援 SurroundingText 時傳送游標前文字；TSF adapter 目前只傳 `app`（尚未讀取 `ITfContext` 的文字）。

## 使用統計

`InputController::usageEvent(kind, data)` 對應 macOS `UsageStatsObserver` 的每一次計數，只在「記錄使用統計」開啟時送出，內容只有數字；`DesktopServices::recordUsageEvent` 收下並寫入 stats.json：

| kind | data | macOS 計數 |
|---|---|---|
| `key` | `latencyMs`、`composing` | 輸入法看到的每個鍵；`composing` 為真時（組字中由輸入法處理）計入 `keys` 與延遲直方圖 |
| `interval` | `ms` | 一段連續打字中兩鍵的間隔：`typingMilliseconds`、按鍵間隔 |
| `pause` | `ms` | 超過 `pauseThreshold`（預設 2 秒）、不到 60 秒的停頓：`thinkingMilliseconds` |
| `commit` | `characters`、`compositionEnded`、`clean`、`correctedShown`、`correctedCommitted` | `characters`；組字結束時 `compositions`、`cleanCompositions`；這個平台沒有重排序模型，後兩者恆為 false |
| `pick` | `list`：`normal`、`corrections`、`single`、`punctuation` | `picks*` |
| `backspace`、`escape`、`deleteAfterCommit`、`literalZhuyin`、`undoLiteral`、`phraseAdded` | — | 同名計數 |
| `correctionShown`、`correctionCommitted` | — | 保留給有重排序模型的平台；目前不送出 |
| `reset` | — | 進入安全輸入：忘記目前的連打 |

## 改字記錄

「記錄改字」開啟時，`CorrectionLog` 把有改字的組字寫成使用者資料夾 `corrections.jsonl` 的一行，格式與 macOS `CorrectionRecord` 相同（schema 1、snake_case 鍵依字母排序），`zt eval corrections` 可直接讀取。QML 以 `correctionLog` 取得 `count`、`location`、`recordsContext`（一併記錄游標前的文字），以及 `showLog()`、`exportLog(url)`、`clearLog()`。

## 視窗與設定

候選視窗不取得鍵盤焦點。游標矩形經 `scale` 換成 Qt 邏輯座標，視窗依「位置」放在游標下方或上方（間隔 4），候選字與組字對齊；空間不足改放另一邊並限制在螢幕可用範圍內。清單、引擎提示（例如 ↩ 加入自訂詞）或無法內嵌的組字出現時才顯示。切換中英時（單按 Shift 或 Caps Lock），「中英提示」開啟就在游標旁顯示「中」或「英」0.8 秒，再淡出 0.2 秒。Windows adapter 先依所在螢幕的原點換算座標，再以 `scale: 1` 傳入，避免混合 DPI 桌面把螢幕原點也縮放。無法在應用程式內呈現組字時，`inlinePreedit: false` 會把組字放在同一個 Qt 視窗，不另顯示 Fcitx 原生面板。滑鼠操作透過同一個 `InputController` 選字，再由原生介面在原欄位取得並送出結果。

一般 Qt 視窗的位置控制適用 X11 與 Windows。Wayland 的輸入法 popup surface 需要合成器與 Fcitx 的協定支援，不能以這個通用視窗的位置計算宣稱已完成。

Windows 的設定欄位可能在服務自己的 GUI 執行緒呼叫 TSF。Qt 執行檔提供 `ZtProcessImeEvents`，讓原生端只在 pipe server 與目前行程相同時，透過公開 Qt 事件迴圈處理等候中的通訊，並排除使用者輸入事件；原生 DLL 不因此依賴 Qt，也不查找 Qt 私有視窗訊息。原生端仍須防止重入、保持回應期限與原欄位識別。

`ImeHostTest` 使用真實公開引擎與本機通訊端，驗證送字、選字、安全輸入、失焦隔離、非法封包與重複識別碼。平台端仍須各自驗證系統註冊與應用程式送達。

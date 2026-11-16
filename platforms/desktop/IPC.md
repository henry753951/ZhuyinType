# 原生輸入法與共用介面

Fcitx5／TSF 負責系統按鍵、焦點、安全輸入、組字範圍、游標矩形與文字送出。`zhuyintype` 執行檔的 `zt_core::service::Service` 持有共用的 `EngineSession`、`InputController`、`Settings` 與 `Shortcuts`，GPUI 介面依 `UiEvent` 顯示候選字視窗。設定介面與真正輸入法使用同一個行程和同一份狀態；再次開啟執行檔會要求既有服務顯示設定，然後結束。

## 通訊

使用僅目前使用者可存取的本機通訊端：

- Linux 為 `$XDG_RUNTIME_DIR/zhuyintype/ime-v1`，未設定或不是絕對路徑時為 `/tmp/zhuyintype-<uid>/ime-v1`。目錄須由目前使用者擁有，權限為 0700；通訊端本身為 0600。每個連線以 `SO_PEERCRED` 取得對方的 uid，和服務的 uid 不同就關閉。路徑上已有其他檔案或其他使用者的通訊端時，服務不啟動，也不刪除那個檔案。
- Windows 為 `\\.\pipe\ZhuyinType-ime-v1-<使用者 SID>`。管道的存取權限只包含目前使用者，拒絕遠端連線；第一個管道以 `FILE_FLAG_FIRST_PIPE_INSTANCE` 建立，其他行程無法先占用這個名稱。

服務以鎖定檔避免同時啟動兩個實例（Linux 在通訊端旁的 `ime-v1.lock`，Windows 在 `%LOCALAPPDATA%\ZhuyinType`）。啟動時先取得鎖定，引擎載入後才開始接受連線，避免首鍵送進尚未就緒的服務。測試可用 `ZT_IME_SOCKET` 或 `ServiceConfig::endpoint` 指定隔離通訊端。

每個連線由一個執行緒處理，請求依序回應。處理請求時只取得服務的鎖，不等待介面的執行緒；同一個行程內的 TSF（App 自己的文字欄位）因此也能使用輸入法。

每個封包是一行 UTF-8 JSON，上限 256 KiB（含換行）。請求包含 `v: 1`、連線內單調遞增的正整數 `id`、最長 128 字元的 `context`、`op`。回應包含相同版本、識別碼與欄位識別。封包不是 JSON 物件、超過上限或 `id` 沒有遞增時，服務關閉連線。

| op | 內容 |
|---|---|
| `focus` | `secure` 布林，可選的 `rect: [x,y,width,height]`、`scale`、`capsLock`、`inlinePreedit`、`panel`、`private`、`activated`、`app` 與 `before`；取得焦點。`private`：私人欄位（見〈私人欄位〉）。`activated`：使用者剛從其他輸入法切換回 ZhuyinType，中／英依「開始輸入時使用英文」重設 |
| `key` | `key`／`modifiers` 為 Qt 數值、`release`／`secure` 布林，可選 `repeat`、`shiftSide`（0 未知、1 左、2 右）、`private`、`app`、`before` 及矩形。原生端也可以只「告知」應用程式自己會收到的按鍵（Windows 沒有組字時的 ⌫、空白、Enter），讓服務更新前文與「送出後刪掉重打」；這時不看回應的 `handled` |
| `context` | 只更新 `app` 與 `before`（見〈前文〉），不處理按鍵 |
| `poll` | 取得滑鼠選字後待送出的文字與最新組字，可更新矩形 |
| `blur` | 結束目前組字並回傳待送出文字，解除焦點 |
| `reset` | 丟棄目前組字與尚未送出的文字 |
| `secure` | `secure` 布林；進入安全輸入時立即清除組字與待送出文字，暫停學習 |
| `action` | `action`（`pick`、`page`、`single`、`corrections`、`zhuyin`）與 `value`（`pick` 的列、`page` 的 ±1）；原生端自己畫的候選字視窗上的點選（見〈候選字視窗〉） |
| `mode` | `english` 布林；設定中／英（Windows 工作列的輸入指示器）。和單按 Shift 相同，組字中與安全欄位不切換 |
| `show-settings` | 顯示既有設定視窗；可選 `place`：`words`（自訂詞與學到的詞）、`about`、`wizard`（設定嚮導）、`tour`（新功能導覽）。輸入法選單用 |

按鍵與修飾鍵沿用 Qt 的數值（`zt_core::keys::code`、`zt_core::keys::modifier`），Linux 與 Windows 的 Control 為 `0x04000000`，Windows 鍵或 Super 為 `0x10000000`。

成功的輸入回應含 `ok`、`handled`、`commit`、`preedit`、UTF-16 `cursor`、`candidates`、`english`、`composing`（有組字或候選字視窗；原生端據此預先判斷要不要攔下按鍵）。`preedit` 為空字串時 `cursor` 為 0。有組字時另含 `clauses`（組字中每個詞的 UTF-16 範圍 `[起, 迄]`，依序、不重疊）、`focus`（候選字清單正在替換的詞的索引，沒有時為 `null`）與 `target`（清單沒開、游標在組字中間時 ↓ 會改的詞，沒有時為 `null`），原生端據此在應用程式裡標示組字：每個詞底線，`focus` 反白，`target` 加粗。沒有組字或安全欄位時省略；舊版服務不送這三個欄位，原生端把整段組字當成一個詞。失敗回應含 `ok: false` 與錯誤說明。每份 `commit` 只在一個回應中取出；沒有未經請求的封包。原生介面必須核對識別碼與欄位，不能把舊欄位回應送入新欄位，也不能重送超時的按鍵。安全欄位不傳輸實際按鍵或文字。

同一時間只有一個取得焦點的欄位可操作組字。轉移焦點前，原生介面先處理舊欄位的 `blur` 回應，再送出新欄位的 `focus`。舊欄位的遲到請求會被拒絕。斷線清除服務端狀態；原生端僅能保留同一個仍聚焦且非安全欄位的已確認組字，不得重送未確認的按鍵。沒有輸入文字的持久化通訊記錄。

## 候選字視窗

候選字視窗有兩種畫法，由原生介面在 `focus` 時以 `panel` 決定：

- **App 畫（預設，Fcitx5 外掛）**：服務送 `UiEvent::Popup` 給 GPUI，App 開自己的浮動視窗（見〈視窗與設定〉）。
- **原生端畫（`panel: true`，Windows TSF DLL）**：App 不開視窗；每個回應多一個 `panel` 欄位，原生端在打字的應用程式行程裡自己畫。視窗由輸入法在應用程式內建立、以該應用程式的視窗為擁有者，所以會出現在開始功能表的搜尋框與沉浸式 App 之上（任何其他行程的視窗都做不到）。

`panel` 是 `zt_panel::style::Panel` 的 JSON：`model`（目前這一頁的候選字、選取、頁碼、注音、提示、單字／修正狀態、聚焦的詞，以及應用程式無法內嵌時的組字 `preview`）、`style`（排列、字級、密度、要顯示的列與按鍵標籤）、`appearance`（`system`、`light`、`dark`）、`animates` 與 `above`。值為 `null` 表示隱藏；和上一個回應相同時省略。版面、配色與鍵帽寫法都來自 `zt-panel`，GPUI 與 TSF DLL 共用，所以兩邊畫出來一樣。同一個回應也可能有 `toast`（`中` 或 `英`，「中英提示」開啟時）與 `boundary`（翻頁到清單盡頭，1 或 -1），原生端分別顯示中英提示與彈一下；這時服務不送對應的 `UiEvent`。

原生端把點選送成 `action` 請求，回應和按鍵的一樣套用（選字後組字更新、清單關閉）。

## 前文

和 macOS `ControllerCore` 相同，組字開始時引擎讀取游標前的文字（「參考游標前的文字」關閉時不讀）：

- `app`（最長 256 字元）：欄位所屬的程式，例如 Fcitx 的 `program()`、Windows 的執行檔名稱。只用來分開各程式送出過的文字，不寫入任何檔案或記錄。
- `before`（最長 1024 個 UTF-16 單位）：平台回報的游標前文字；服務只取最後 128 個字元。不知道時省略欄位，不要送空字串（空字串代表在文件開頭）。

服務把同一個 `app` 在 1 分鐘內送出的文字（最多保留 255 個字元）和 `before` 接在一起，重疊的部分只算一次；沒有 `before` 時只用送出過的文字。送出的字、交給程式的空白、換行與刪除都會更新這段文字。換程式、安全輸入或超過 1 分鐘就清空。安全欄位與私人欄位不傳 `before`，服務也不保留任何文字。Fcitx adapter 在欄位支援 SurroundingText 時傳送游標前文字；TSF adapter 傳送執行檔名稱與 `ITfContext` 中選取範圍前最多 255 個 UTF-16 單位的文字。

## 私人欄位

瀏覽器的私密視窗（Windows 的 `IS_PRIVATE`，Fcitx 的 `Sensitive`）要求輸入法不要學習。和 macOS 的安全輸入相同，這種欄位照常打中文，但不學習、不讀取也不保留前文、不記錄改字（`private: true`）。密碼、PIN 與停用的欄位才是安全欄位（`secure: true`）：按鍵直接交給程式，不經過服務。

## 提示

回應的 `toast` 是游標旁的提示：單按 Shift 切換中英時是「中」或「英」，切換中文標點的全形／半形時是「全」或「半」。「中英提示」關閉時不送。

## 使用統計

「記錄使用統計」開啟時，`InputController` 把 macOS `UsageStatsObserver` 的每一次計數送成一個 `zt_core::stats::UsageEvent`，內容只有數字；`UsageStats::record` 收下並寫入使用者資料夾的 `stats.json`（格式與 macOS 相同，最多每 10 分鐘寫入一次，設定視窗開啟與 App 結束時也會寫入）：

| UsageEvent | 內容 | macOS 計數 |
|---|---|---|
| `Key` | `latency_ms`、`composing` | 輸入法看到的每個鍵；`composing` 為真時（組字中由輸入法處理）計入 `keys` 與延遲直方圖 |
| `Interval` | `ms` | 一段連續打字中兩鍵的間隔：`typingMilliseconds`、按鍵間隔 |
| `Pause` | `ms` | 超過停頓門檻（預設 2 秒）、不到 60 秒的停頓：`thinkingMilliseconds` |
| `Commit` | `characters`、`composition_ended`、`clean`、`corrected_shown`、`corrected_committed` | `characters`；組字結束時 `compositions`、`cleanCompositions`；這個平台沒有重排序模型，後兩者恆為 false |
| `Pick` | `Normal`、`Corrections`、`Single`、`Punctuation` | `picks*` |
| `Backspace`、`Escape`、`DeleteAfterCommit`、`LiteralZhuyin`、`UndoLiteral`、`PhraseAdded` | — | 同名計數 |
| `CorrectionShown`、`CorrectionCommitted` | — | 保留給有重排序模型的平台；目前不送出 |
| `Reset` | — | 進入安全輸入：忘記目前的連打 |

## 改字記錄

「記錄改字」開啟時，`zt_core::corrections::CorrectionLog` 把有改字的組字寫成使用者資料夾 `corrections.jsonl` 的一行，格式與 macOS `CorrectionRecord` 相同（schema 1、snake_case 鍵依字母排序，含 `punctuation_width` 設定與選字事件的 `candidate_text`、`candidate_reading`、`candidate_start`、`candidate_end`），`zt eval corrections` 可直接讀取。介面以 `CorrectionLog` 的 `count`、`location`、`export`、`clear` 顯示與管理記錄；「一併記錄游標前的文字」是設定 `recordsCorrectionContext`。

## 視窗與設定

候選字視窗不取得鍵盤焦點。游標矩形經 `scale` 換成邏輯座標，服務以 `UiEvent::Popup(Some(PopupState))` 送出組字、目前這一頁的候選字與游標矩形；介面以 `zt_core::presentation::popup_position` 依「位置」把視窗放在游標下方或上方（間隔 4），候選字與組字對齊；空間不足改放另一邊並限制在螢幕可用範圍內。清單、引擎提示（例如 ↩ 加入自訂詞）或無法內嵌的組字出現時才顯示，否則送出 `UiEvent::Popup(None)`。切換中英時（單按 Shift 或 Caps Lock），「中英提示」開啟就送出 `UiEvent::ModeToast`，介面在游標旁顯示「中」或「英」0.8 秒，再淡出 0.2 秒。Windows adapter 先依所在螢幕的原點換算座標，再以 `scale: 1` 傳入，避免混合 DPI 桌面把螢幕原點也縮放。無法在應用程式內呈現組字時，`inlinePreedit: false` 會把組字放在同一個候選字視窗，不另顯示 Fcitx 原生面板；「在候選字視窗顯示組字的 App」清單中的程式也是如此。滑鼠操作以 `Service::popup_action` 透過同一個 `InputController` 選字，再由原生介面以 `poll` 取得並送出結果。

X11 與 Windows 可以由 App 指定視窗位置。Wayland 的輸入法 popup surface 需要合成器與 Fcitx 的協定支援，不能以這個通用視窗的位置計算宣稱已完成。

Windows 的設定欄位可能在 App 自己的介面執行緒呼叫 TSF。服務的連線執行緒不等待介面執行緒，所以同一個行程內的 TSF 照一般方式等候回應即可，不需要在等候時處理介面的事件。原生端仍須防止重入、保持回應期限與原欄位識別。

`crates/zt-core/tests/ime_host.rs` 使用真實引擎與本機通訊端，驗證送字、選字、安全輸入、失焦隔離、非法封包、重複識別碼、單一實例與顯示設定。平台端仍須各自驗證系統註冊與應用程式送達。

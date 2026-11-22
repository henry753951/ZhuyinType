# 原生輸入法與共用介面

Fcitx5／TSF 負責系統按鍵、焦點、安全輸入、組字範圍、游標矩形與文字送出。`zhuyintype` 執行檔的 `zt_core::service::Service` 持有共用的 `EngineSession`、`InputController`、`Settings` 與 `Shortcuts`，GPUI 介面依 `UiEvent` 顯示候選字視窗。設定介面與真正輸入法使用同一個行程和同一份狀態；再次開啟執行檔會要求既有服務顯示設定，然後結束。

## 通訊

使用僅目前使用者可存取的本機通訊端：

- Linux 為 `$XDG_RUNTIME_DIR/zhuyintype/ime-v1`，未設定或不是絕對路徑時為 `/tmp/zhuyintype-<uid>/ime-v1`。目錄須由目前使用者擁有，權限為 0700；通訊端本身為 0600。每個連線以 `SO_PEERCRED` 取得對方的 uid，和服務的 uid 不同就關閉。路徑上已有其他檔案或其他使用者的通訊端時，服務不啟動，也不刪除那個檔案。
- Windows 為 `\\.\pipe\ZhuyinType-ime-v1-<使用者 SID>`。管道的存取權限只包含目前使用者，拒絕遠端連線；第一個管道以 `FILE_FLAG_FIRST_PIPE_INSTANCE` 建立，其他行程無法先占用這個名稱。服務同時有三個管道等待連線，每接受一個連線就先建立下一個再交出去，登入時許多程式同時連線也不會遇到管道忙碌；建立管道失敗時稍後再試，不會停止接受連線。

服務以鎖定檔避免同時啟動兩個實例（Linux 在通訊端旁的 `ime-v1.lock`，Windows 在 `%LOCALAPPDATA%\ZhuyinType`）。啟動時先取得鎖定，引擎載入後才開始接受連線，避免首鍵送進尚未就緒的服務。測試可用 `ZT_IME_SOCKET` 或 `ServiceConfig::endpoint` 指定隔離通訊端。

每個連線由一個執行緒處理，請求依序回應。處理請求時只取得服務的鎖，不等待介面的執行緒；同一個行程內的 TSF（App 自己的文字欄位）因此也能使用輸入法。

每個封包是一行 UTF-8 JSON，上限 256 KiB（含換行）。請求包含 `v: 1`、連線內單調遞增的正整數 `id`、最長 128 字元的 `context`、`op`。回應包含相同版本、識別碼與欄位識別。封包不是 JSON 物件、超過上限或 `id` 沒有遞增時，服務關閉連線。

| op | 內容 |
|---|---|
| `focus` | `secure` 布林，可選的 `rect: [x,y,width,height]`、`scale`、`capsLock`、`inlinePreedit`、`panel`、`private`、`activated`、`app` 與 `before`；取得焦點。`private`：私人欄位（見〈私人欄位〉）。`activated`：使用者剛從其他輸入法切換回 ZhuyinType，中／英依「開始輸入時使用英文」重設（Windows：停用 ZhuyinType 時作用中的已是其他鍵盤輸入法或鍵盤配置才算；程式的執行緒結束或開新執行緒不算）。`fullscreen`：與 `activated` 一起送，欄位的視窗蓋滿整個螢幕（全螢幕遊戲），Windows 的輸入法切換提示被它擋住，服務改以「中英提示」顯示「中」或「英」。`inlinePreedit: false`：App 不能顯示組字（Linux：Fcitx 沒有 Preedit 能力；Windows：遊戲，見〈懸浮打字框〉），「看不到注音時改用懸浮打字框」開啟時組字改在候選字視窗顯示。回應另含 `inlinePreedit`：最後的決定（也考慮設定與「在候選字視窗顯示組字的 App」），為 false 時回應的 `preedit` 為空，原生端不在 App 裡組字 |
| `key` | `key`／`modifiers` 為 Qt 數值、`release`／`secure` 布林，可選 `repeat`、`shiftSide`（0 未知、1 左、2 右）、`private`、`app`、`before` 及矩形。原生端也可以只「告知」應用程式自己會收到的按鍵（Windows 沒有組字時的 ⌫、空白、Enter），讓服務更新前文與「送出後刪掉重打」；這時不看回應的 `handled` |
| `context` | 只更新 `app` 與 `before`（見〈前文〉），不處理按鍵 |
| `poll` | 取得滑鼠選字後待送出的文字與最新組字，可更新矩形。只在候選字視窗開著時需要（兩鍵之間只有視窗上的點選或剛改的設定會改變組字），其他時候原生端不定時喚醒 |
| `blur` | 結束目前組字並回傳待送出文字，解除焦點。個人資料在回應之後、不持有服務的鎖時寫入磁碟，失焦不必等磁碟 |
| `reset` | 丟棄目前組字與尚未送出的文字 |
| `secure` | `secure` 布林；進入安全輸入時立即清除組字與待送出文字，暫停學習 |
| `action` | `action`（`pick`、`page`、`single`、`corrections`、`zhuyin`、`block`、`forget`、`addPhrase`、`undo`、`symbol`、`symbolTab`）與 `value`（`pick`、`block`、`forget`、`addPhrase` 的列，`page` 的 ±1，`symbol` 是符號面板整份清單裡的第幾個、`symbolTab` 是第幾個類別）；原生端自己畫的候選字視窗上的點選：`block`「不要再推薦」、`forget`「忘記這個選字」、`addPhrase`「加入自訂詞」是某一列的右鍵選單，`undo` 是提示列的「復原」（見〈候選字視窗〉） |
| `selection` | `text` 字串（最多 32 個 UTF-16 單位，沒選取時是空字串）；回應裡有 `selection: true` 時才送：欄位中選取的文字，服務以它開啟填好的「加入自訂詞」（見〈選取的字加入自訂詞〉）。安全或私人欄位不送，服務也不接受 |
| `mode` | `english` 布林；設定中／英（Windows 工作列的輸入指示器）。和單按 Shift 相同，組字中與安全欄位不切換 |
| `quick` | 快速設定：可選的 `english`、`fullWidth`、`simplified`、`learningPaused` 布林，只套用有送的欄位，都沒有時只讀取（見〈快速設定〉） |
| `show-settings` | 顯示既有設定視窗；可選 `place`：`words`（自訂詞與學到的詞）、`about`、`wizard`（設定嚮導）、`tour`（新功能導覽）。輸入法選單用 |
| `diagnostics` | 「回報問題」與 `zhuyintype --health` 用：回應 `version`、`started`（Unix 秒）、`uptime`（秒）、`lastKey`（距離上次按鍵的秒數，還沒有按鍵時為 `null`）、`connections`（目前連線的原生端數量）與 `recent`（服務最近的警告與錯誤，`zt_core::health::recent`）。只有次數與時間，不含按鍵、文字或 App 名稱；不需要 `v` 以外的欄位，也不改變任何狀態。舊版服務回應失敗 |

按鍵與修飾鍵沿用 Qt 的數值（`zt_core::keys::code`、`zt_core::keys::modifier`），Linux 與 Windows 的 Control 為 `0x04000000`，Windows 鍵或 Super 為 `0x10000000`。

成功的輸入回應含 `ok`、`handled`、`commit`、`preedit`、UTF-16 `cursor`、`candidates`、`english`、`simplified`（「繁／簡」為簡體）、`learningPaused`（暫停學習）、`fullWidth`（中文標點全形）、`composing`（有組字或候選字視窗；原生端據此預先判斷要不要攔下按鍵；剛出現、還沒按 ↓ 進入的聯想詞清單不算，見下）。`preedit` 為空字串時 `cursor` 為 0。有組字時另含 `clauses`（組字中每個詞的 UTF-16 範圍 `[起, 迄]`，依序、不重疊）、`focus`（候選字清單正在替換的詞的索引，沒有時為 `null`）與 `target`（清單沒開、游標在組字中間時 ↓ 會改的詞，沒有時為 `null`），原生端據此在應用程式裡標示組字：每個詞底線，`focus` 反白，`target` 加粗。有按錯鍵修正或模糊音轉出的詞時另含 `corrections`（這些字在 `preedit` 中的 UTF-16 範圍 `[起, 迄]`，與 `clauses` 同單位，依序、不重疊），原生端在這些字下方加點線（Fcitx 沒有點線，改用斜體）；「標示自動修正」關閉或沒有修正時省略。沒有組字或安全欄位時省略；舊版服務不送這幾個欄位，原生端把整段組字當成一個詞、不標示修正。沒有組字時也要攔的按鍵見〈攔截的按鍵〉（`claims`）。失敗回應含 `ok: false` 與錯誤說明。每份 `commit` 只在一個回應中取出；沒有未經請求的封包。原生介面必須核對識別碼與欄位，不能把舊欄位回應送入新欄位，也不能重送超時的按鍵。安全欄位不傳輸實際按鍵或文字。

同一時間只有一個取得焦點的欄位可操作組字。轉移焦點前，原生介面先處理舊欄位的 `blur` 回應，再送出新欄位的 `focus`。舊欄位的遲到請求會被拒絕。斷線清除服務端狀態；原生端僅能保留同一個仍聚焦且非安全欄位的已確認組字，不得重送未確認的按鍵。沒有輸入文字的持久化通訊記錄。

## 候選字視窗

候選字視窗有兩種畫法，由原生介面在 `focus` 時以 `panel` 決定：

- **App 畫（預設，X11 上的 Fcitx5 外掛）**：服務送 `UiEvent::Popup` 給 GPUI，App 開自己的浮動視窗（見〈視窗與設定〉）。
- **原生端畫（`panel: true`，Windows TSF DLL；Wayland 工作階段的 Fcitx5 外掛）**：App 不開視窗；每個回應多一個 `panel` 欄位，原生端自己顯示。TSF DLL 在打字的應用程式行程裡畫：視窗由輸入法在應用程式內建立、以該應用程式的視窗為擁有者，所以會出現在開始功能表的搜尋框與沉浸式 App 之上（任何其他行程的視窗都做不到）。Fcitx5 外掛把它換成 Fcitx 自己的候選字清單，由 Fcitx 畫在游標旁（Wayland 上一般程式不能擺放自己的視窗）。

`panel` 是 `zt_panel::style::Panel` 的 JSON：`model`（目前這一頁的候選字、選取、頁碼、注音、提示、單字／修正狀態、聚焦的詞，以及應用程式無法內嵌時的組字 `preview` 與其中自動修正的字 `preview_corrected`、視窗開在修正過的詞上時的頁尾 `typed`：「你打的是 ㄊㄜˇ · ⌫ 還原」）、`style`（排列、字級、密度、要顯示的列與按鍵標籤）、`appearance`（`system`、`light`、`dark`；`system` 表示跟隨系統，「候選字視窗」與「主題」都跟隨時才是 `system`）、`translucent`（「透明效果」：有系統的背景材質時讓它透出來；沒有這個欄位時當成 `true`）、`animates`、`above` 與 `accent`（「強調色」：`system` 或 `zhuyintype`；沒有這個欄位時當成 `system`）。值為 `null` 表示隱藏；和上一個回應相同時省略。版面、配色與鍵帽寫法都來自 `zt-panel`，GPUI 與 TSF DLL 共用，所以兩邊畫出來一樣。同一個回應也可能有 `toast`（`中`、簡體輸出時的 `简` 或 `英`，以及 `全`／`半`，「中英提示」開啟時）與 `boundary`（翻頁到清單盡頭，1 或 -1），原生端分別顯示中英提示與彈一下；這時服務不送對應的 `UiEvent`。

原生端把點選送成 `action` 請求，回應和按鍵的一樣套用（選字後組字更新、清單關閉）。所以原生端只在顯示候選字清單時 `poll`（讓剛改的設定立刻反映到開著的清單），閒置的欄位不會定時喚醒應用程式與服務。服務從不主動送封包，原生端在送出按鍵前檢查閒置的管道（`PeekNamedPipe`）：App 結束或重新啟動後，第一個按鍵就重新連線，不會送進已斷的管道而被當成英文字母交給程式。

中英提示與翻頁到盡頭的彈一下只由原生端畫：最後取得焦點的欄位屬於自己畫視窗的原生端時，即使不在請求中（例如失焦之後），服務也不送 `UiEvent::Toast`、`UiEvent::Boundary`，App 不會再畫一個。

選取的字加入自訂詞：沒在組字時按下 `addSelectedText` 的鍵，`key` 的回應多一個 `selection: true`，原生端接著送 `selection`（TSF 以 `ITfContext::GetSelection` 與 `ITfRange::GetText` 讀，Fcitx 5 以 surrounding text 的 `selectedText()`）。這個指令預設沒有按鍵；它的鍵列在 `claims`（〈攔截的按鍵〉），只限中文模式、非私人欄位。

字詞動作（`zt_panel::model`）：`model.rows[].actions` 是這一列的右鍵選單裡能用的動作，第 n 位是 `WORD_ACTIONS[n]`（0 `block`、1 `forget`、2 `addPhrase`），沒有時省略；`model.undo` 為 `true` 時提示（例如「不再推薦「八」」）後面有可點的「Ctrl+Z 復原」（按鍵是 `style.keys.undo`，舊版 App 沒送時是 Ctrl+Z），原生端 5 秒後自己把它拿掉（服務那時也不再接受 Ctrl+Z）；`model.tip` 是清單下方的一行說明（「Ctrl+Del 不要再推薦・右鍵看更多」），只在前 5 次開清單時有。三個欄位在沒有內容時都省略，所以舊的原生端照樣能讀。

「符號與 Emoji」面板開著時，回應的 `composing` 與 `candidates` 為真、`preedit` 為空，原生端把每個鍵都送給服務；App 畫的視窗顯示面板的格子（`PopupState::picker`），原生端畫的視窗收到反白的那一列（最多 9 個）當成候選字清單：面板開啟時按鍵在搜尋列，數字也打進搜尋，這時 `rows[].key` 為 `null`；按 ↓ 進入格子後才是 1–9、類別與列數在 `notice`，點選照常送 `pick`；`panel.picker` 另帶整個格子（`zt_panel::picker::PickerView`，和 App 畫的同一份：類別、搜尋、顯示的列、反白，`numbered`：按鍵在格子、反白那一列標 1–9，`hint`：下方寫的目前狀態的按鍵；0.7.2 以前的服務不送這兩個欄位），認得它的原生端（Windows 文字服務）改畫格子，點格子送 `symbol`、點類別送 `symbolTab`。舊的原生端忽略 `picker`，照舊畫那一列。畫格子的原生端只要格子還在畫面上，就把每個鍵（按下與放開，中文與英文模式）都交給服務，不只看最近一份回應的 `composing`；不在編輯工作階段中收到的回應（`reset`、只告知服務的鍵）也要套用 `panel` 與 `composing`：`reset` 會關閉面板，回應的 `panel` 為 `null`，原生端沒套用就會留下一個沒有面板在背後、按鍵打不進搜尋的格子。請求失敗（逾時、連線中斷）時原生端收起視窗，下次聚焦時服務重新開始。

聯想詞（「送出後列出聯想詞」）剛出現、還沒按 ↓ 進入時，回應的 `candidates` 為真、`composing` 為假，另含 `associated: true`（其他時候省略）；`panel.model.rows[].key` 這時為 `null`，原生端不標選字鍵。這時清單只要 ↓：攔按鍵的原生端（TSF）照沒有組字時的規則攔鍵，另外攔下沒有修飾鍵的 ↓；其他不攔的鍵在交給應用程式的同時，以 `key` 告訴服務（和 ⌫、空白鍵、Return 相同，回應不套用到文件），服務關閉清單，原生端照回應的 `panel` 把視窗收起。這是因為 Win32 的欄位中，測試時被攔下的鍵即使服務沒處理也不會回到應用程式。按 ↓ 之後清單和一般的候選字清單相同，`composing` 為真。舊的服務不送 `associated`，那時 `composing` 為真、每個鍵都攔。

## 攔截的按鍵

原生端只把確定服務要的按鍵交給服務時（TSF：沒有組字、英文模式時不攔按鍵；Ctrl+Alt 組合鍵平常是 AltGr 的），需要知道哪些指令的鍵在沒有組字時也要送給服務。回應的 `claims` 是這些按鍵 `[[key, modifiers], …]`（Qt 鍵碼與修飾鍵；字母不分大小寫比對，有 Shift 時符號以它所在的鍵比對：Windows 報的 Ctrl+Shift+? 就是 Ctrl+Shift+/）：服務依目前的欄位與中英模式算好，原生端只要比對按鍵，不必再看模式。

| 指令 | 何時列出 |
| --- | --- |
| `quickSettings`（開啟快速設定） | 中文與英文模式、私人欄位也列 |
| `symbolPicker`（符號與 Emoji） | 功能開啟時；中文與英文模式、私人欄位也列 |
| `addSelectedText`（選取的字加入自訂詞） | 只在中文模式、非私人欄位 |

Windows 上不含 Ctrl 的 Alt 組合鍵是系統鍵（`WM_SYSKEYDOWN`），TSF 不交給文字服務的按鍵處理，有沒有組字都一樣：0.7.1 的 Alt+← 在記事本中沒有作用、Alt+空白 打開視窗選單。所以 Linux／Windows 的預設不用 Alt（[DESIGN.md](../../docs/product/DESIGN.md) §2.1「按鍵的原則」），Windows 版錄製按鍵時也不接受這種組合（「Windows 的 Alt 組合鍵交給 App，輸入法收不到」）；Ctrl+Alt 組合鍵照常送到。

安全欄位不列任何按鍵（服務在那裡不收按鍵）。`claims` 在 `focus` 的回應一定送，之後只在清單改變時送（改了快捷鍵、切換中英、私人或安全欄位改變、功能開關），沒變時省略，原生端沿用上一次的清單；`blur` 的回應不送。其他指令只作用於組字或候選字清單，那時原生端本來就攔下每個按鍵，不必列出。

## 前文

和 macOS `ControllerCore` 相同，組字開始時引擎讀取游標前的文字（「參考游標前的文字」關閉時不讀）：

- `app`（最長 256 字元）：欄位所屬的程式，例如 Fcitx 的 `program()`、Windows 的執行檔名稱。只用來分開各程式送出過的文字，不寫入任何檔案或記錄。
- `before`（最長 1024 個 UTF-16 單位）：平台回報的游標前文字；服務只取最後 128 個字元。不知道時省略欄位，不要送空字串（空字串代表在文件開頭）。

服務把同一個 `app` 在 1 分鐘內送出的文字（最多保留 255 個字元）和 `before` 接在一起，重疊的部分只算一次；沒有 `before` 時只用送出過的文字。送出的字、交給程式的空白、換行與刪除都會更新這段文字。換程式、安全輸入或超過 1 分鐘就清空。安全欄位與私人欄位不傳 `before`，服務也不保留任何文字。Fcitx adapter 在欄位支援 SurroundingText 時傳送游標前文字；TSF adapter 傳送執行檔名稱與 `ITfContext` 中選取範圍前最多 255 個 UTF-16 單位的文字。

## 私人欄位

瀏覽器的私密視窗（Windows 的 `IS_PRIVATE`，Fcitx 的 `Sensitive`）要求輸入法不要學習。和 macOS 的安全輸入相同，這種欄位照常打中文，但不學習、不讀取也不保留前文、不記錄改字（`private: true`）。密碼、PIN 與停用的欄位才是安全欄位（`secure: true`）：按鍵直接交給程式，不經過服務。

## 快速設定

輸入法選單與「開啟快速設定」快捷鍵（預設沒有按鍵）開啟的小清單：「英文」「半形標點」「簡體」「暫停學習」四個勾選項目，接著「設定…」「自訂詞與學到的詞…」（`show-settings` 的 `words`），以及有符號表時的「符號…」。原生端用 `quick` 讀取與切換：

- `english`：中／英，規則和 `mode` 相同（組字中與安全欄位不切換）。
- `fullWidth`：中文標點全形（`false` 為半形），和「切換標點全半形」快捷鍵相同，也顯示「全」或「半」的提示。
- `simplified`：設定 `outputScript`（`traditional`／`simplified`），會寫入設定檔。簡體時組字、候選字與送出的字在離開服務時換成簡體（`zhuyin_core::simplified::to_simplified`，字數與 UTF-16 長度不變，`cursor` 與 `clauses` 照用）；引擎、詞庫、學習與改字記錄仍用繁體。中文模式的提示與標示從「中」換成「简」。
- `learningPaused`：暫停學習，只存在服務的記憶體裡，不寫入任何檔案；服務每次啟動都恢復學習。暫停時每個欄位都當成私人欄位：照常打字，但不學習、不讀取也不保留前文、不記錄改字。

回應是一般的輸入回應，再加上 `quick: {"english", "fullWidth", "simplified", "learningPaused", "symbols"}`，`symbols` 表示有沒有符號表（目前都是 `false`，「符號…」不顯示）。`quick` 也可以從沒有焦點的連線或欄位送（例如欄位外的選單）：四個開關是整個服務的，照樣套用，但回應不帶任何欄位的文字（`commit`、`preedit` 為空、沒有 `panel`），取得焦點的欄位在下一個回應看到新的狀態。`simplified` 寫入設定檔的時機在服務處理完請求、放開鎖之後，同一個回應裡的組字與 `panel` 已經換好。

「開啟快速設定」的鍵列在 `claims`（〈攔截的按鍵〉），任何模式都攔。

按下「開啟快速設定」快捷鍵時（英文模式也有效），回應含 `handled: true` 與 `quickPanel: true`。自己畫視窗的原生端（`panel: true`）在游標旁顯示自己的快速設定選單；App 畫視窗時，服務另送 `UiEvent::QuickPanel { anchor }`，GPUI 在游標旁開一個不取得焦點的小視窗，點視窗外、按下一個鍵或 Esc 就關閉。

## 提示

回應的 `toast` 是游標旁的提示：單按 Shift 切換中英時是「中」或「英」，切換中文標點的全形／半形時是「全」或「半」。「中英提示」關閉時不送。

## 使用統計

「記錄使用統計」開啟時，`InputController` 把 macOS `UsageStatsObserver` 的每一次計數送成一個 `zt_core::stats::UsageEvent`，內容只有數字；`UsageStats::record` 收下並寫入使用者資料夾的 `stats.json`（格式與 macOS 相同；打字時最多每 10 分鐘寫入一次，停止打字一分鐘後也會寫入，因為登出或更新時程式會直接被結束，沒有機會再寫）：

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

候選字視窗不取得鍵盤焦點。游標矩形經 `scale` 換成邏輯座標，服務以 `UiEvent::Popup(Some(PopupState))` 送出組字、目前這一頁的候選字與游標矩形；介面以 `zt_core::presentation::popup_position` 依「位置」把視窗放在游標下方或上方（間隔 4），候選字與組字對齊；空間不足改放另一邊並限制在螢幕可用範圍內。清單、引擎提示（例如 ↩ 加入自訂詞）或無法內嵌的組字出現時才顯示，否則送出 `UiEvent::Popup(None)`。切換中英時（單按 Shift 或 Caps Lock），「中英提示」開啟就送出 `UiEvent::ModeToast`，介面在游標旁顯示「中」或「英」0.8 秒，再淡出 0.2 秒。Windows adapter 先依所在螢幕的原點換算座標，再以 `scale: 1` 傳入，避免混合 DPI 桌面把螢幕原點也縮放。Fcitx5 adapter 傳 X 伺服器的像素（`scale: 1`，Fcitx 只知道這個）；App 在 X11 上放視窗前再除以 GPUI 的縮放（Xft.dpi），所以 HiDPI 的 X11 桌面上視窗仍在游標旁（`candidate::popup::in_logical_pixels`）。無法在應用程式內呈現組字時，`inlinePreedit: false` 會把組字放在同一個候選字視窗，不另顯示 Fcitx 原生面板；「在候選字視窗顯示組字的 App」清單中的程式也是如此。滑鼠操作以 `Service::popup_action` 透過同一個 `InputController` 選字，再由原生介面以 `poll` 取得並送出結果。

螢幕報讀程式執行時（Linux，`zt_core::system_look::screen_reader`），沒有要求 `panel` 的 Fcitx5 外掛在 `focus` 的回應中收到 `fcitxPanel: true`，之後就像 `panel: true` 一樣由 Fcitx 顯示清單：GPUI 的視窗沒有 AT-SPI 介面，Fcitx 的清單可以被讀出。

X11 與 Windows 可以由 App 指定視窗位置。Wayland 的輸入法 popup surface 需要合成器與 Fcitx 的協定支援，所以 Wayland 工作階段由 Fcitx 顯示清單（`panel: true`）。

Windows 的設定欄位可能在 App 自己的介面執行緒呼叫 TSF。服務的連線執行緒不等待介面執行緒，所以同一個行程內的 TSF 照一般方式等候回應即可，不需要在等候時處理介面的事件。原生端仍須防止重入、保持回應期限與原欄位識別。

`crates/zt-core/tests/ime_host.rs` 使用真實引擎與本機通訊端，驗證送字、選字、安全輸入、失焦隔離、非法封包、重複識別碼、單一實例與顯示設定。平台端仍須各自驗證系統註冊與應用程式送達。

## 懸浮打字框（Windows）

多數遊戲（Minecraft: Java Edition 與其他 GLFW、SDL 或遊戲引擎的視窗）用 WM_CHAR 收字，沒有自己的 TSF 文件（Windows 經 IMM32 給它一個 transitory context），沒有系統游標，也不設定輸入法的組字或候選字視窗位置。Windows 因此把組字畫在螢幕左上角的小框裡（全螢幕時幾乎看不到），遊戲自己處理輸入法訊息時則完全不顯示；微軟注音也一樣。

文字服務在每次準備送鍵時判斷（`composition::floating`），以下全部成立才算：

- context 為 `TS_SS_TRANSITORY`；
- 文件回報的游標不是視窗內有高度的一行（不知道位置時，Windows 回報螢幕右下角一個高度為 0 的點，全螢幕遊戲裡這個點也在視窗內）；
- 執行緒沒有系統游標；
- IMM 的組字視窗與候選字視窗位置都是 `CFS_DEFAULT`；
- 這個欄位已經在 App 裡組過一次字。有些 IMM32 App 在組字開始時才設定位置。判斷為遊戲的視窗會記住，之後在同一個視窗打字直接用懸浮打字框。

Chromium 與 Electron App 的 TSF 文件也標為 transitory，但會回報游標，所以不算。符合時以 `inlinePreedit: false` 告訴服務，候選字視窗的位置改為遊戲視窗左下角往內 24 像素（遊戲的聊天列通常在那裡），視窗開在它上方。服務回應 `inlinePreedit: false` 時，組字只在候選字視窗顯示，App 只收到送出的字。會自己顯示組字的 IMM32 App（Scintilla、Qt 5、Edit 控制項）有系統游標或會設定這些位置，照舊在 App 裡組字。判斷結果改變時，文字服務重送 `focus`。

在測試機上可以用 `crates/zt-tsf/examples/game_window.rs` 重現：它像 GLFW 一樣收字，`--mode eat` 自己處理輸入法訊息，`--mode noime` 關掉輸入法，`--fullscreen` 蓋滿螢幕。

遊戲把視窗的輸入法關掉時（`ImmAssociateContextEx` 不給 input context，有些遊戲在遊玩時這樣做），Windows 不把按鍵交給任何輸入法，打出的是英文字母；輸入法無法處理，等遊戲打開聊天、重新開啟輸入法即可。


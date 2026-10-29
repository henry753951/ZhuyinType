# Qt 介面設計系統

Linux 與 Windows 共用的設定視窗、候選字視窗都由這裡的元件組成。目的是讓之後新增或修改頁面的人（或模型）不需要自己判斷美感：選對元件、用對 token，畫面就和其他頁一致。`design-rules` 檢查（`tests/DesignLint.cmake`，屬於 `tools/zt check desktop`）會擋下違反下列規則的修改。

## 方向

- **自己的樣子，不模仿任何平台。** 中性灰帶一點藍，加上一個取自 App 圖示的靛藍強調色。不用 macOS 系統藍、不用彩色圖示方塊，也不用 Qt Basic 的預設方框控制項。
- **安靜。** 一個畫面最多一個強調色主按鈕；次要動作用灰底，連結與低重要性的動作用純文字，破壞性動作用紅字。狀態用小標籤（`Chip`），不用整片色塊。
- **說明集中。** 「尚未接上」「示範文字」這類開發中說明，一頁最多一行，用 `Caption`。
- **淺色、深色都要好看。** 所有顏色都來自 `Theme.qml`，兩種外觀都在那裡定義。

## Token（`Theme.qml`）

| 類別 | Token | 用途 |
|---|---|---|
| 底色 | `canvas` `sidebar` `card` `surface` `field` | 視窗、側欄、卡片、浮動面板（候選字、選單）、輸入欄 |
| 文字 | `text` `secondary` `tertiary` | 主要文字、說明、更次要的註記與停用 |
| 線與狀態 | `line` `fieldBorder` `hover` `control` `controlHover` `controlPressed` `track` | 分隔線、輸入欄框、列的滑過、灰底按鈕的三種狀態、開關與拉桿的底 |
| 強調 | `accent` `accentHover` `accentText` `accentSoft` `selection` `focusRing` | 主按鈕、選取中的文字、強調色上的白字、淡強調底（目前頁、已選列）、候選字反白、鍵盤焦點框 |
| 狀態色 | `success` `warning` `danger` | 已連接、注意、破壞性動作與錯誤 |
| 形狀 | `cardRadius` 12、`controlRadius` 8、`controlHeight` 30 | 卡片、控制項、按鈕高度 |
| 字級 | `titleSize` 20、`displaySize` 18、`headingSize` 15、`bodySize` 13、`detailSize` 12、`captionSize` 11 | 頁面標題、試打與大字、卡片內小標題、內文、說明、註記 |

間距以 4／8 px 為單位：卡片內距 14–16，列高至少 44，區塊間距 `sectionSpacing`（24）。數字用等寬數字：`font.features: ({ "tnum": 1 })`。動畫 110–220 ms、`Easing.OutCubic`，一律檢查 `theme.motionEnabled`，`theme.reduceMotion` 時不做位移。

## 元件

每個元件第一行註解寫著什麼時候用它；都要傳入 `theme`。

### 版面

| 元件 | 用途 |
|---|---|
| `SectionCard` | 一組相關內容的圓角卡片，上方可有灰色小標題（`title`） |
| `SettingsPage` | 依 `settings.json` 的 `page`／`group` 自動排出設定卡片 |
| `Divider` | 卡片內兩列之間的細線 |
| `Caption` | 卡片下方或控制項旁一到兩行的說明 |
| `EmptyState` | 清單或統計沒有內容時：圖示、可選的標題（`title`）與一句話，置中 |

### 列

| 元件 | 用途 |
|---|---|
| `SettingRow` | 一項設定：標題、說明，右側依類型自動放開關、分段、數字、下拉或拉桿 |
| `NavigationRow` | 會開啟另一頁的列，右側有值與箭頭 |
| `ListRow` | 卡片內的一般列：左側標題與說明，右側放任意內容（值、鍵帽、控制項），`navigates` 時加箭頭 |
| `ValueRow` | 唯讀的「名稱：數值」列（指標、版本），沒有值時顯示淡色「—」 |
| `SelectableRow` | 清單頁（記住的字詞、自訂詞、App）的一列：字與同一行的讀音、右側的常用程度或註記；滑過時「編輯」「刪除」淡入在同一位置，不移動版面 |
| `DisclosureRow` | 可展開的列（授權與資料來源）：左側轉動的箭頭、標題與說明，展開後的內容對齊標題 |
| `SidebarItem` | 視窗側欄的一個頁面 |

### 控制項

| 元件 | 用途 |
|---|---|
| `AppButton` | 唯一的按鈕。`kind`：`secondary`（預設）、`primary`、`plain`、`danger`；`iconName` 加圖示；`compact` 用在工具列與列內 |
| `IconButton` | 只有圖示的圓形按鈕（說明、清除、關閉），一定要設定 `Accessible.name` |
| `AppTextField` | 單行輸入；`search: true` 加放大鏡與清除鈕 |
| `AppTextArea` | 多行輸入（試打） |
| `AppComboBox` | 下拉選單 |
| `SegmentedControl` | 切換同一塊內容的兩三種檢視（期間、清單模式）；儲存的設定改用 `SettingRow` 的分段 |
| `PillTabs` | 可換行的圓角分頁（示範情境、篩選） |
| `AppMenu` | 右鍵選單；項目用 `Action { text; onTriggered }`，不要用 `MenuItem` |
| `AppDialog` ＋ `AppDialogButtons` | 對話框與右對齊的按鈕列；確認類的按鈕用 `primary`，刪除、清除、回復用 `danger` |

### 內容

| 元件 | 用途 |
|---|---|
| `Icon` | 線條圖示，名稱見 `Icons.js`。新增圖示時照同樣畫法：24 格、只描線、圓頭，自己畫，不複製第三方圖示集 |
| `KeyCap`、`KeyCapFlow` | 按鍵：單一鍵帽與會換行的一組鍵帽；`active` 表示開啟中，`small` 用在候選字視窗 |
| `Chip` | 小標籤：狀態或數量；`tone` 為 `neutral`、`accent`、`success`、`warning`、`danger` |
| `StatTile` | 統計格：小標題、等寬大數字、說明 |
| `BarChart` | 單一系列的長條圖（每天送出的字、按鍵反應分布）：左側三條格線、部分長條下的標籤、最大值標數字、滑過顯示該條 |
| `MeterBar` | 放在列右側的細長條：一部分佔整體多少，或 `second` 時兩部分（打字／思考） |
| `WeekHeatMap` | 一週 × 24 小時的色階格（什麼時候打字） |
| `IntroStage` | 介紹頁的打字故事：用真正的 `CandidatePanel` 依目前設定播放 |
| `OptionTile` | 設定嚮導的選項：有幾行說明與鍵帽的卡片（右側勾選），或單行選項（前面圓鈕；`multi` 為核取方塊） |
| `DemoControls` | 示範動畫的播放／暫停與步驟點，所有示範共用 |
| `MiniDemo`、`DemoLine`、`MiniList` | 短示範：一行文字欄位（已送出的字、組字底線、游標）、按下的鍵與一句說明；需要時下方列出幾個候選字。ⓘ 卡片與模糊音頁使用 |
| `ChoiceDemo` | ⓘ 卡片中的一個選項：名稱、「目前的設定」標籤與它的 `MiniDemo` |
| `InfoPopover` | ⓘ 開啟的說明卡，出現在 ⓘ 旁，點別處或 Esc 關閉；不改設定 |
| `DocumentText` | 可選取的長文字（授權、NOTICE）；只有授權全文用 `monospace` |
| `DocumentViewer` | 在 App 內閱讀授權或署名的對話框：檔名、所屬項目、可選取的全文（授權用等寬字、Markdown 排版），「拷貝全文」「在檔案管理員中顯示」「完成」 |
| `FactRow` | ⓘ 卡片中的一項：強調色的線條圖示加一句話 |
| `UsageMeter` | 記住的字詞的常用程度：五根由低到高的小長條 |
| `MarkdownText` | 以格式顯示 Markdown（資料來源），連結用強調色並在瀏覽器開啟 |
| `CandidatePanel` | 候選字視窗本體，浮動視窗與設定頁預覽共用 |

## 寫一頁的方式

頁面檔（`Main`、`*Pages`、`AppearancePage`、`SettingsPage`、`ShortcutOverview`、`LiveInput`）只負責組合元件，不自己設計控制項：

```qml
SectionCard {
    theme: page.theme
    title: "改字記錄檔案"
    ValueRow {
        theme: page.theme
        divider: false
        title: "記錄改字"
        value: "已開啟"
    }
    Caption {
        theme: page.theme
        text: "只記錄彙總事件次數與時間。"
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6
        AppButton { theme: page.theme; compact: true; iconName: "upload"; text: "匯出…" }
        AppButton { theme: page.theme; compact: true; kind: "danger"; iconName: "trash"; text: "清除…" }
    }
}
```

`design-rules` 對頁面檔檢查：

- 不直接宣告 Qt 控制項（`Button`、`TextField`、`ComboBox`、`Dialog`、`Menu`、`Popup`、`CheckDelegate`……），包括寫在 `delegate:` 或屬性值的；
- 字級不寫數字，用 `theme.bodySize` 等 token；
- 不設定 `palette.*`。

對所有 QML 檔檢查：不寫死顏色（`"#rrggbb"` 只能出現在 `Theme.qml`）、不用文字符號當圖示（ⓘ › ‹ × ✕ ✓）、每個檔案都列在 `CMakeLists.txt` 的 `QML_FILES`。

需要新的樣子時，先找現有元件能不能透過屬性做到；不能，就新增一個元件檔（開頭註解寫用途、`required property Theme theme`、只用 token），加進 `QML_FILES` 與上面的表格，再在頁面使用。新的頁面檔要加進 `tests/DesignLint.cmake` 的 `pages`。

## 檢查畫面

```sh
tools/zt check desktop
```

會在離屏（offscreen）產生 80 張截圖到 `.build-qt/renders`，不開啟視窗、不搶焦點。修改後把相關頁面的淺色、深色與窄版拼成一張總覽圖一起看，不要逐張開。

# 設定的寫法（開發者文件）

每個設定只寫一次。設定頁、「快捷鍵」總覽、設定嚮導、習慣組合與「原本 → 改成」的清單，都從同一份描述產生。這份文件說明這套寫法，以及加一個設定時要改哪裡。

程式在 `platforms/macos/Sources/ZhuyinTypeApp/`：

| 檔案 | 內容 |
|---|---|
| `Shared/Settings/SettingsStorage.swift` | 設定存放的地方：`UserDefaults`（App）或 `MemorySettingsStorage`（嚮導的暫存設定、UI 預覽、自我測試） |
| `Shared/Preferences.swift` | 每個設定的型別與預設值（`register(defaults:)`），以及轉成引擎的 `ComposerOptions` |
| `UI/Settings/Registry/SettingsRegistry.swift` | 設定的描述（`SettingDescriptor`）、所有設定的清單、快照與變更（`SettingsSnapshot`、`SettingChange`） |
| `UI/Settings/Registry/Habits.swift` | 方式（`HabitStyle`、`HabitGroup`）、習慣組合（`SettingsPreset`）、嚮導的課程（`Lesson`） |
| `UI/Settings/Registry/RegistryViews.swift` | 依設定的種類畫出控制項與設定列（`SettingControl`、`RegistryRow`） |

## 1. 設定的描述

`SettingDescriptor` 描述一個設定：

- `id`：唯一的名稱，例如 `spaceAction`、`feature.deleteWord`、`keys.nextPage`；
- `title`：設定頁與清單上的名稱；`detail`：依目前的值寫出的一句說明；
- `kind`：種類：開關（`toggle`）、選項（`choice`，每個選項有標籤）、數字（`number`，範圍、間隔與單位）、多選（`flags`，例如模糊音）、按鍵（`keys`）；
- `place`：在設定頁的位置（頁、下一層、列），連結與到達提示用；
- `preview`：「視窗與提示」的預覽中顯示這個設定的狀態，改變時預覽切到這個狀態；
- `demo`：ⓘ 示範或嚮導動畫的名稱；`keywords`：搜尋用的其他詞；
- `read` 與 `write`：透過 `Preferences` 讀寫，所以同一個描述可以用在使用者的設定或嚮導的暫存設定。

預設值不另外寫：`SettingsRegistry.defaults` 是一份新的 `Preferences`（暫存）的快照，所以預設值只在 `Preferences` 的 `register(defaults:)` 中出現一次。

值的標籤（`label(_:)`）由種類產生：選項的標題、「開啟」「關閉」、數字加單位、按鍵（「⇧Tab、fn↓」）或「沒有按鍵」。

## 2. 頁面

設定頁的一般列用 `RegistryRow("id", preferences)` 畫：名稱、說明、數字的值與控制項都來自描述。需要時加上 `segmented: true`（兩三個短選項用分段控制）、`accessory:`（ⓘ 示範）或 `dimmed:`。有開關又有按鍵的功能仍用 `FeatureRow`，按鍵欄位與「快捷鍵」總覽共用 `ShortcutField`。

「快捷鍵」的「全部還原…」還原 `SettingsRegistry.keyAndFeatureIDs` 中的設定；進階的「回復預設設定」清除所有儲存的設定。

## 3. 快照與變更

```swift
let before = SettingsSnapshot(preferences)
// …改設定…
let changes = before.changes(to: SettingsSnapshot(preferences))   // [SettingChange]，依設定頁的順序
changes.first?.oldLabel   // 「標準」
changes.first?.newLabel   // 「積極」
changes.apply(to: other)  // 套用到另一份設定
changes.revert(in: other) // 改回原本的值
```

設定嚮導的確認頁、習慣組合的預覽都用這份清單；不另外撰寫「改了什麼」的文字。

`preferences.sandbox()` 是一份放在記憶體中的複本：嚮導在上面試，按「套用」時才把變更寫回。`preferences.restoreDefaults(ids)` 把指定的設定（或全部）還原成預設值。

## 4. 方式、習慣組合與課程

**方式（`HabitStyle`）** 是使用一個功能的一種做法；**方式組（`HabitGroup`）** 是同一個功能的幾種做法：

- 互斥（`exclusive: true`）：一次只能選一種，例如標點按法、排列、空白鍵；畫成分段控制或選項卡；
- 可以同時有效（`exclusive: false`）：例如翻頁的 Tab、Page Down、⌥↓ 與在頁底按 ↓；畫成勾選。按鍵的方式加入時經過按鍵衝突的檢查，已用於其他動作的按鍵不會加入。

方式用三種寫法產生：`.value(id, 值)`（一個設定的一個值）、`.key(動作, 按鍵)`（動作的其中一個按鍵）、`.feature(功能)`（功能的開關）與 `.flag(id, 位元)`（多選中的一項）。

**習慣組合（`SettingsPreset`）** 是一組設定值的資料。`changes(from:)` 產生從目前設定到這個組合的變更，套用前顯示「原本 → 改成」。組合只放各輸入法的官方說明寫到的行為（`sources` 列出出處），說明沒有寫的設定保留目前的值。

**課程（`Lesson`）** 是設定嚮導的一步：名稱、一句說明、等級（基本、完整、可略過、進階）、預覽的狀態、方式組與練習任務（`PracticeTask`：題目與完成的判斷）。`since` 記錄加入的版本，更新後的「新功能導覽」只列出新的課程。

## 5. 加一個設定

1. 在 `Preferences` 加上屬性與預設值（`register(defaults:)`），需要時接到 `composerOptions`；
2. 在 `SettingsRegistry` 對應的段落加一個描述：名稱、種類、位置、說明、預覽的狀態；
3. 在設定頁加一列 `RegistryRow("新的 id", preferences)`；
4. 想讓嚮導介紹它：在 `Habits` 加一個方式組，並加到某個 `Lesson`（或新增一課）；
5. 執行 `tools/zt check app`：自我測試檢查每個設定都能讀寫每個值、都有標籤、變更能套用與改回，每個方式都能開啟。

## 6. 方式一覽

常見注音輸入法使用各個功能的方式，以及 ZhuyinType 如何提供。「同時」表示幾種方式可以一起有效（預設都開啟，由按鍵衝突決定），「擇一」表示在設定中選一種。

| 功能 | 方式 | 類型 | ZhuyinType |
|---|---|---|---|
| 空白鍵 | 送出並輸入空白；輸入空白，不送出；開啟候選字視窗 | 擇一 | 「打字 › 空白鍵」，預設送出並輸入空白 |
| Esc | 清除整段組字；只清除注音 | 擇一 | 「打字 › Esc」 |
| 標點 | ⌃ 加符號鍵打中文標點、⇧ 打鍵上的符號（Windows）；⇧ 加符號鍵打中文標點（macOS） | 擇一 | 「打字 › 標點按法」；⌃ 與 ⌥ 的組合可以個別關閉 |
| 中英切換 | 單按 Shift（左右、左、右）；Caps Lock 打英文 | 同時 | 「打字 › 中英切換」 |
| 選字鍵 | 1–9；A S D F G H J K L | 擇一 | 「視窗與提示 › 選字鍵」 |
| 候選字清單的排列 | 直式；橫式 | 擇一 | 「視窗與提示 › 排列」 |
| 翻頁 | Tab；Page Down（fn ↓）；⌥↓；在頁底按 ↓；清單中的空白 | 同時 | 「視窗與提示 › 翻頁」與「到頁底時 ↓ 翻到下一頁」 |
| 改前後的字 | 直式的 ← →；⌥← ⌥→ | 同時 | 「視窗與提示 › 排列」下的按鍵 |
| 刪掉一個詞 | ⌥⌫ | 可以關閉 | 「打字 › 刪掉一個詞」 |
| 要改的字 | 游標右邊的字；游標左邊的字 | 擇一 | 「選字 › 游標在句中時改」 |
| 按錯鍵 | 關閉；標準；積極 | 擇一 | 「選字 › 按錯鍵修正」 |
| 常見的其他讀法（有邊讀邊、輕聲照本調打、台灣讀音） | 接受；不接受 | 可以關閉 | 「選字 › 按錯鍵 › 接受常見的其他讀法」，預設接受 |
| 模糊音 | ㄗ／ㄓ、ㄘ／ㄔ、ㄙ／ㄕ、ㄣ／ㄥ、ㄢ／ㄤ | 同時 | 「選字 › 模糊音」 |

## 7. 習慣組合的出處

兩個組合的內容與每一項的出處整理在 [PRESETS.md](PRESETS.md)。組合只放有出處的行為；標為「未確認」的項目（例如翻頁的按鍵、微軟注音沒有未完成的注音時的空白）不放進組合。macOS 組合的「標點按法」只有 2017 年的第二手資料，暫時不放。

| 組合 | 包含的設定 |
|---|---|
| Windows 注音的習慣 | 單按 Shift 切換中英（左右 Shift）、開始輸入時使用英文：關、⌃ 加符號鍵打中文標點：開、選字鍵 1–9（都和預設相同） |
| macOS 注音的習慣 | 空白鍵：開啟候選字視窗、單按 Shift：關、Caps Lock 打英文：開、⌥ 加數字或符號鍵打鍵上的符號：開、選字鍵 1–9 |
| ZhuyinType 預設 | 所有設定的預設值（按鍵與功能的開關除外） |
| 自訂 | 不改變設定 |

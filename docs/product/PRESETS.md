# 習慣組合的依據

設定嚮導的「Windows 注音的習慣」與「macOS 注音的習慣」（[SETUP_WIZARD.md](SETUP_WIZARD.md) §6）各是一組設定值。本文件整理兩種系統內建注音輸入法實際的行為與出處，再對應到 ZhuyinType 的設定。組合只放有出處的行為；找不到出處的列在 §5，不放進組合。

微軟注音與 Apple 的注音是很多人每天使用的注音輸入法，很多打字習慣由它們建立。ZhuyinType 的幾個固定按鍵沿用了這些習慣，例如 ↓ 開啟候選字清單、清單開著時空白翻頁、Return 或數字鍵選字、⌃ 加符號鍵打全形標點，以及選取幾個字後按 Return 加入自訂詞（微軟新注音的舊版說明中已有這個做法）。

## 1. 比較的條件

- **查閱日期**：2026-10-28。只根據文件，沒有在 Windows 或 macOS 上實際操作驗證。
- **微軟注音**：Windows 11 內建的版本，也就是微軟支援文件「Microsoft 繁體中文輸入法」所寫的版本。該文件標示適用於 Windows 11 與 Windows 10。另參考 Microsoft Learn 的「Traditional Chinese IME」，最後更新於 2024-06-20。
- **微軟注音的「舊版」**：設定 › 一般 › 相容性 ›「使用舊版的微軟注音」。微軟的文件只說明這個開關的用途，是在新版有問題時改用舊版，沒有逐項寫出新舊版的差異。
- **舊版說明**：「Help of Microsoft IMEs」是 Office IME 時代 Microsoft New Phonetic IME（微軟新注音）的說明，原為微軟的文件，現在只在第三方網站保存。它比 Windows 11 的「舊版」更早，所以只當作背景參考。它和現行文件說法一致的地方才算有依據，不能單獨作為組合的依據。
- **Apple 注音**：macOS 內建的「繁體注音」（Zhuyin – Traditional）。依據是 Apple 的「中文輸入法使用手冊」與「Mac 使用手冊」，網址預設開啟目前的版本（macOS 27）。macOS 15 與 26 版的同一組頁面，除了排版與幾處和注音無關的內容，其他相同。
- **標記**：
  - 「廠商」：微軟或 Apple 目前的文件；
  - 「第二手」：其他可信的來源，例如微軟社群支援人員的回答、專門介紹 Apple 產品的媒體、其他注音輸入法專案的說明；
  - 「舊版」：只有上述舊版說明；
  - 「未確認」：找不到出處。
- **引用**：只摘要，不抄錄原文。來源列在 §6，表格中以代號標示。

## 2. 微軟注音

| 項目 | 行為 | 依據 | 標記 |
|---|---|---|---|
| 空白，有未完成的注音 | 一聲，完成這個音節 | M2（「夏天」的例子） | 廠商 |
| 空白，沒有未完成的注音 | 現行版本找不到說明。舊版說明寫：在組字中兩個字之間按空白會開啟候選字清單 | M4 第 74 頁 | 未確認（舊版有說明） |
| 空白，清單開著 | 下一頁 | M1、M2 | 廠商 |
| Enter，清單開著 | 選取反白的候選字 | M1、M2 | 廠商 |
| Enter，組字中 | 送出組字 | M4 第 26 頁 | 舊版 |
| 單按 Shift | 中英切換 | M1、M2 | 廠商 |
| 左右 Shift | 舊版設定可以分別指定左、右 Shift 的作用：切換中英、送出游標前的字、不使用。兩邊預設都是切換中英 | M4 第 111–113 頁 | 舊版 |
| Ctrl + 空白 | 開啟或關閉中文輸入法；這是系統的快速鍵，不是輸入法內的中英切換 | M3 | 廠商 |
| Caps Lock | 現行版本找不到說明。舊版說明只提到：Caps Lock 關閉時，Shift 加字母打大寫；開啟時打小寫 | M4 第 28 頁 | 未確認 |
| 預設輸入模式 | 設定中有「預設輸入模式」 | M2 | 廠商 |
| 預設輸入模式的預設值 | 中文 | S3；M4 第 9 頁 | 第二手 |
| 開啟候選字清單 | ↓ 開啟。↑ 不開啟清單，直接換成另一個候選字 | M1、M2 | 廠商 |
| 清單中移動 | ↑ ↓ 移動反白 | M1、M2 | 廠商 |
| Tab、→ | 把清單展開成表格，用方向鍵瀏覽 | M1、M2 | 廠商 |
| Page Down、Page Up | 下一頁、上一頁 | M4 第 30 頁 | 舊版 |
| Shift 加方向鍵 | 現行版本找不到。舊版說明中，組字中按 Shift 加 ← → 選取文字，再按 Enter 加入使用者造詞 | M4 第 145 頁 | 未確認（舊版有說明） |
| 選字 | 數字鍵；Enter 選取反白的候選字 | M1、M2 | 廠商 |
| 關聯字視窗 | 在「一般」設定中開啟關聯字詞的提示後才出現；用 Shift 加數字選字，空白翻頁 | M1、M2 | 廠商 |
| Ctrl 加符號鍵 | 打出對應的全形符號。需要 Shift 的符號要加按 Shift，例如 Ctrl + Shift + ? 是「？」 | M1 | 廠商 |
| 反引號 ` 加符號鍵 | 打出對應的全形符號；只在中文模式有效 | M1 | 廠商 |
| Shift 加符號鍵（不加 Ctrl） | 找不到說明 | | 未確認 |
| 全形與半形 | Shift + 空白切換字元寬度。預設的寬度找不到說明 | M1、M2 | 廠商（預設值未確認） |
| 「、」 | 現行版本找不到說明。舊版說明中，反引號加 ' 打出「、」 | M4 第 81 頁 | 未確認 |
| 符號鍵盤 | Ctrl + Alt + 逗號 | M1 | 廠商 |
| Shift 加字母，中文模式 | 現行版本找不到說明。舊版說明列為打英數字元的方法之一 | M4 第 28 頁 | 未確認 |
| Esc，清單開著 | 關閉清單，不選字 | M2 | 廠商 |
| Esc，組字中 | 現行版本找不到說明。舊版：第一次取消剛打的注音，再按取消組字；設定「Esc 鍵用來」可選送出或清除，預設清除 | M4 第 76、115 頁 | 未確認（舊版有說明） |
| Backspace，沒有未完成的注音 | 現行版本找不到說明。舊版：刪掉游標前的字 | M4 第 75 頁 | 未確認 |
| 組字區滿了 | 舊版說明：組字區最多 31 個字，超過時自動送出前面的字。現行版本與 Windows 11 的「舊版」都找不到說明 | M4 第 9、27 頁 | 舊版（31 字） |
| 學習 | 設定中有輸入法學習的選項；內容與預設值找不到說明。舊版說明：預設開啟，可以匯出、匯入 | M2；M4 第 95–100 頁 | 廠商（有選項）；內容未確認 |
| 使用者造詞 | 上限 2,500 個，目前無法提高 | S1（微軟社群支援人員，2024-12） | 第二手 |
| 鍵盤配置 | 標準、倚天、漢語拼音、通用拼音 | M1 | 廠商 |

## 3. Apple 注音

| 項目 | 行為 | 依據 | 標記 |
|---|---|---|---|
| 空白，有未完成的注音 | 一聲 | A1 | 廠商 |
| 空白，字已轉換 | 開啟候選字視窗，換掉自動選出的字。所以打完最後一個一聲的字要按兩下空白：一下是一聲，一下開啟清單 | A1、A2 | 廠商 |
| 空白，清單開著 | 直式時下一頁，橫式時下一列 | A1 | 廠商 |
| 空白選字 | 注音不用空白選字；空白選字是其他中文輸入方式的做法 | A2 | 廠商 |
| Return | 確認並送出文字；清單開著時選取反白的候選字 | A1、A2 | 廠商 |
| 單按 Shift | 注音的設定中沒有以 Shift 切換中英的選項；切換輸入方式的說明也沒有提到 Shift | A4、A5 | 廠商（沒有這個選項） |
| Caps Lock | 系統設定 › 鍵盤 › 文字輸入 › 編輯，開啟「使用大寫鎖定鍵來切換」後，Caps Lock 或鍵盤上的「中／英」鍵切換中文與上次使用的拉丁輸入方式。這個選項的預設值找不到說明 | A5、A9 | 廠商（預設值未確認） |
| Control + 空白 | 選取上一個輸入方式；Control + Option + 空白選取下一個 | A5 | 廠商 |
| fn 或地球鍵 | 可以設定為切換輸入方式 | A5 | 廠商 |
| Control + Shift + Z | 切換到繁體注音 | A3 | 廠商 |
| 開啟候選字視窗 | 空白；↓ 沒有說明 | A1、A2 | 廠商（空白） |
| 清單中移動 | 方向鍵移到上一個或下一個候選字；= 與 -、[ 與 ] 移到上一列或下一列 | A2 | 廠商 |
| Tab | 切換候選字的排序方式（有排序選項時） | A2 | 廠商 |
| Page Down、Shift 加方向鍵 | 找不到說明 | | 未確認 |
| 選字 | 數字鍵、Return、按兩下候選字；清單開著時要打數字，按住 Option | A2 | 廠商 |
| 排列 | 可以選水平或垂直，也可以設定字級；預設值找不到說明 | A2、A4 | 廠商（預設值未確認） |
| Shift 加符號鍵 | 全形中文標點，例如 Shift 加逗號鍵是「，」、句點鍵是「。」、斜線鍵是「？」 | S2（2017） | 第二手 |
| 標點視窗 | Option + Shift + B | A3、A8 | 廠商 |
| 半形標點 | 快速鍵表列出 Option + Shift + H 使用半形標點，但設定頁的「使用半形標點符號」沒有列出注音 | A3、A4 | 廠商（兩頁不一致） |
| Control 加符號鍵 | 找不到說明 | | 未確認 |
| 「、」 | 一篇介紹說是直接按鍵盤上的一個鍵，但只以圖片標示，沒有寫出是哪一鍵 | S2 | 未確認 |
| Shift 加字母，中文模式 | 按住 Shift 打大寫拉丁字母，再從候選字視窗選取；拉丁字母預設是半形。這是中文輸入法共通的說明，不只針對注音。S2 說是全形大寫，和 Apple 的說明不同 | A6；S2 | 廠商 |
| Shift + 空白 | 顯示結構候選字 | A3 | 廠商 |
| Shift + Delete | 刪除最後的輸入，例如刪掉剛打的聲調 | A1、A3 | 廠商 |
| Esc | 找不到說明 | | 未確認 |
| Delete，沒有未完成的注音 | 找不到說明 | | 未確認 |
| 組字區滿了 | 找不到說明 | | 未確認 |
| 學習 | 「調整用字頻率」，注音適用：常用的候選字排在前面。預設值找不到說明 | A4 | 廠商（預設值未確認） |
| 使用者辭典 | 從輸入法選單選「編輯使用者辭典」，填「輸入碼」與「字詞」；以同一個 Apple 帳號同步到其他裝置 | A7 | 廠商 |

## 4. 對應到 ZhuyinType 的設定

設定的名稱與值見 [DESIGN.md](DESIGN.md) §2.1、§2.2 與 [SETTINGS_IA.md](SETTINGS_IA.md)；括號內是 `Preferences` 的識別字。組合沒有列出的設定，套用時維持目前的值。

### 4.1 Windows 注音的習慣

| 設定（頁面） | 值 | 依據 |
|---|---|---|
| 單按 Shift 切換中英（打字）（`shiftToggle`） | 開，左右 Shift（`both`） | 單按 Shift 切換中英：M1、M2，廠商。左右兩邊都切換是舊版的預設（M4）；現行文件沒有分左右 |
| 開始輸入時使用英文（打字）（`startInEnglish`） | 關 | 預設輸入模式是中文：S3，第二手；M4 第 9 頁，舊版 |
| ⌃ 加符號鍵打中文標點（打字 › 標點與符號）（`KeyFeature.controlPunctuation`） | 開 | Ctrl 加符號鍵打全形符號：M1，廠商 |
| 選字鍵（視窗與提示）（`selectionKeys`） | 1 2 3 4 5 6 7 8 9（`digits`） | 數字鍵選字：M1、M2，廠商 |

這四項都和 ZhuyinType 的預設值相同，所以從預設設定套用這個組合時不會改變任何設定。

以下和微軟注音相同，而且是 ZhuyinType 固定的行為，不需要設定：

- 有未完成的注音時，空白是一聲；
- ↓ 開啟候選字清單；
- 清單開著時，空白翻到下一頁，Enter 選取反白的字，Esc 關閉清單。

### 4.2 macOS 注音的習慣

| 設定（頁面） | 值 | 依據 |
|---|---|---|
| 空白鍵（打字）（`spaceAction`） | 開啟候選字視窗（`openCandidates`） | 字已轉換時，空白開啟候選字視窗，清單開著時翻頁，不用空白選字：A1、A2，廠商 |
| 單按 Shift 切換中英（打字）（`shiftToggle`） | 關（`none`） | 注音的設定與切換方式的說明都沒有 Shift：A4、A5，廠商 |
| Caps Lock 打英文（打字）（`KeyFeature.capsLockEnglish`） | 開 | 以 Caps Lock 切換中文與拉丁輸入方式：A5、A9，廠商。系統的這個選項開著時，Caps Lock 由系統處理（DESIGN.md §2.1） |
| 標點按法（打字）（`punctuationKeys`） | macOS（`mac`） | Shift 加符號鍵打全形中文標點：S2，第二手（2017） |
| ⌥ 加數字或符號鍵打鍵上的符號（打字 › 標點與符號）（`KeyFeature.optionSymbols`） | 開 | 按住 Option 打數字：A2，廠商。文件只寫數字，沒有寫符號鍵 |
| 選字鍵（視窗與提示）（`selectionKeys`） | 1 2 3 4 5 6 7 8 9（`digits`） | 數字鍵選字：A2，廠商 |

以下和 Apple 注音相同，而且是 ZhuyinType 固定的行為，不需要設定：

- 有未完成的注音時，空白是一聲；
- 清單開著時，空白翻頁、不選字，Return 選取反白的字。

「標點按法」只有第二手的依據，而且那份資料是 2017 年的。實作前最好在目前的 macOS 上確認一次。

### 4.3 沒有對應設定的行為

下列行為 ZhuyinType 沒有相同的設定，組合不處理。嚮導可以在相關的課程中說明差異。

- 微軟注音的 ↑ 直接換候選字、Tab 與 → 展開表格、Shift + 空白切換全形與半形、關聯字視窗、以 Unicode 或 Big5 碼輸入。
- 微軟注音以反引號 ` 加符號鍵打全形符號。ZhuyinType 的 `` ` `` 預設是「打出注音本身」。這個按鍵可以在「選字 › 改字」更改。
- Apple 注音以 = 與 -、[ 與 ] 移到上一列或下一列。ZhuyinType 不能把沒有修飾鍵的符號鍵指定為按鍵，因為這些鍵用來打注音與標點。
- Apple 注音的 Shift + 空白（結構候選字）、Tab（排序方式）、標點視窗。ZhuyinType 的 ⇧空白 是在組字中放一個空白。
- 兩邊的 Control + 空白都是系統切換輸入方式或開關輸入法的快速鍵，不是輸入法的設定。

## 5. 未確認、沒有放進組合的項目

| 設定 | 缺少的資料 |
|---|---|
| Windows 組合的「空白鍵」 | 微軟注音沒有未完成的注音時按空白會怎樣。現行文件沒寫；舊版說明寫組字中按空白開啟清單，但無法確認現行版本相同 |
| Windows 組合的「標點按法」 | 微軟注音在中文模式下，Shift 加符號鍵打出什麼。文件只寫了 Ctrl 加符號鍵 |
| 「排列」 | 微軟注音的清單用 ↑ ↓ 移動、用 → 展開，看起來是直式，但文件沒有直接寫。Apple 兩種都可以選，預設值找不到說明 |
| 「每頁的候選字」 | 兩邊都找不到現行的數字。舊版說明寫超過 9 個候選字時翻頁 |
| 翻頁的按鍵（Tab、Page Down、⌥↓） | 兩邊都沒有用 Tab 翻頁：微軟用 Tab 展開表格，Apple 用 Tab 換排序方式。Page Down 只有微軟的舊版說明；⌥↓ 兩邊都沒有。目前的預設（三個都開）是 ZhuyinType 自己的設計 |
| 「到頁底時 ↓ 翻到下一頁」 | 兩邊在清單最後一個字按 ↓ 的行為都找不到說明 |
| 「Esc」 | 組字中按 Esc 清除多少：微軟只有舊版說明，Apple 找不到說明 |
| Windows 組合的「Caps Lock 打英文」 | 微軟注音中 Caps Lock 的作用找不到說明 |
| macOS 組合的「⌃ 加符號鍵打中文標點」 | Apple 注音中 Control 加符號鍵的作用找不到說明 |
| 「記住選過的字詞」 | 兩邊都有學習的選項，但現行的預設值都找不到說明 |
| 組字區的長度 | 微軟只有舊版的 31 字，Apple 找不到說明。ZhuyinType 的上限（60 個音節，之後再打的注音鍵不加入）是固定的，沒有設定 |

### 和設定嚮導草稿的差異

[SETUP_WIZARD.md](SETUP_WIZARD.md) §6 原本的表格已依本文件更新：

- macOS 組合的「空白鍵」改為「開啟候選字視窗」。Apple 的文件寫明，字已轉換時空白開啟候選字視窗，所以不是「送出並輸入空白」。
- Windows 組合不再設定「空白鍵」與「標點按法」，改設「⌃ 加符號鍵打中文標點」。
- 兩個組合都不再設定「排列」與翻頁的按鍵。

分鏡畫面中「習慣組合」的卡片文字（`WizardStoryboard.swift`）還是草稿的內容，實作嚮導時一併更新。

## 6. 來源

| 代號 | 來源 | 網址 |
|---|---|---|
| M1 | 微軟支援：Microsoft 繁體中文輸入法（適用於 Windows 11、Windows 10） | https://support.microsoft.com/zh-tw/windows/microsoft-traditional-chinese-ime-ef596ca5-aff7-4272-b34b-0ac7c2631a38 |
| M2 | Microsoft Learn：Traditional Chinese IME（2024-06-20 更新） | https://learn.microsoft.com/en-us/globalization/input/traditional-chinese-ime |
| M3 | 微軟支援：Keyboard shortcuts in Windows（Ctrl + 空白鍵） | https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec |
| M4 | Help of Microsoft IMEs（Office IME 時代的微軟新注音說明，第三方保存；頁碼是 PDF 的頁碼） | https://documentation.help/Microsoft-IME/documentation.pdf （存檔：https://web.archive.org/web/20250614004414/https://documentation.help/Microsoft-IME/documentation.pdf ） |
| A1 | Apple 中文輸入法使用手冊：在 Mac 上使用繁體注音輸入中文 | https://support.apple.com/zh-tw/guide/chinese-input-method/cimzt15531/mac |
| A2 | 同上：使用候選字視窗 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim12992/mac |
| A3 | 同上：繁體中文輸入法的鍵盤快速鍵 | https://support.apple.com/zh-tw/guide/chinese-input-method/cimkeyt36646/mac |
| A4 | 同上：更改中文與廣東話輸入方式設定 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim21aa5fa50/mac |
| A5 | 同上：切換到中文或廣東話輸入方式 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim119a8d473/mac |
| A6 | 同上：使用中文輸入法輸入拉丁字母 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim11841/mac |
| A7 | 同上：編輯中文使用者辭典 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim165f83982/mac |
| A8 | 同上：輸入中文標點符號與特殊字元 | https://support.apple.com/zh-tw/guide/chinese-input-method/cim12987/mac |
| A9 | Mac 使用手冊：輸入方式設定 | https://support.apple.com/zh-tw/guide/mac-help/mchl84525d76/mac |
| S1 | Microsoft Q&A：微軟注音使用者造詞的上限（微軟社群支援人員回答，2024-12-12） | https://learn.microsoft.com/zh-tw/answers/questions/3922708/question-3922708 |
| S2 | 蘋果仁：Mac 如何打出各種標點符號、特殊符號、注音文、全形英文（2017-11-22 更新） | https://applealmond.com/posts/9341 |
| S3 | e 黑馬寫不停：Windows 11 預設輸入法改成英文（2023-02-06；預設輸入模式從「中文」改為「英數字元」） | https://eheima.com/8273/win11inputmode-en |

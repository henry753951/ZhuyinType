# 全專案程式審查（2026-10-28）

依 [../CODE_STANDARDS.md](../CODE_STANDARDS.md) 審查 main（`dcdbcb1`，v0.4.4）。這份報告記錄當天的程式，之後不隨程式更新。

## 範圍與方法

靜態閱讀，沒有執行測試、量測或實機操作。範圍是 `engine/crates/` 全部、`platforms/macos/Sources/` 全部、`ml/src/zhuyintype_ml/` 的 `core`、`eval`、`remote`、`cli` 與 `training/pipeline.py`。六個審查員分頭閱讀，標為 P1 的項目由協調者再讀一次程式確認，並調整等級。macOS 介面的大型檔案（設定頁、嚮導、預覽輸出）只粗看，`ml/training/` 各模型的程式沒有逐行閱讀；沒有列出問題的檔案不代表沒有問題。

## 總評與評分

整體工程水準不錯：命名與註解說明意圖，模型以回呼與 trait 注入，引擎有和 Swift 版逐位元相同的 golden 測試與 parity gate，`ml/` 以登記的方式擴充。主要的弱點有兩類，在各模組反覆出現：

1. **外部可寫的欄位繞過了快取與衍生值的失效**：組字器的計分設定、重排模型的參數、候選清單的頁大小、詞庫的兩個索引。
2. **失敗被吞掉、回報成功**：個人資料與統計的寫入、統計與改字記錄的刪除、遠端成果的取回、評測的續跑與訓練的完成標記。

| 模組 | 功能 | 可讀 | SOLID | 模組化 | 狀態 | 擴充 | 測試 |
|---|---|---|---|---|---|---|---|
| 組字器（`composer.rs`） | 4 | 4 | 2 | 2 | 2 | 3 | 2 |
| 引擎與資料（`engine`、`conversion`、`lexicon`、`language_model`、`binary`） | 3 | 4 | 3 | 3 | 3 | 3 | 4 |
| 按鍵與讀音（`key_channel`、`key_decoder`、`reading_channel`、`personal` 等） | 3 | 4 | 4 | 4 | 3 | 3 | 4 |
| 模型、C 介面、命令列（`zhuyin-models`、`zhuyin-ffi`、`zhuyin-cli`） | 3 | 4 | 3 | 4 | 3 | 4 | 4 |
| macOS 輸入法核心（`ZhuyinEngine`、`IMK`、`Shared`） | 3 | 4 | 3 | 3 | 3 | 3 | 3 |
| macOS 介面（`UI`） | 3 | 4 | 3 | 3 | 3 | 3 | 3 |
| 研發工具（`ml`） | 3 | 4 | 4 | 4 | 3 | 4 | 3 |

## 職責表

**組字器 `Composer`**（約 2,100 行、約 60 個欄位）

| 職責 | 對應程式 |
|---|---|
| 鍵盤事件處理 | `process()`、`handle_candidate_key()` |
| 注音解碼與切分 | `best_split()`、`apply_run()` |
| 字模型評分 | `score_characters()`、`score_variants()` |
| 候選字產生與排序 | `open_candidates()` |
| 整句重排 | `rerank()` |
| 學習與修正 | `record_commit()`、`follow_deletion()` |
| 組字編輯 | `insert()`、`delete()` |
| 畫面資料 | `present()` |
| 自動送出 | `make_room()`、`commit_point()` |

**引擎 `Engine`**（約 2,200 行）：詞庫查詢、讀音比對與容錯、學習與個人資料、背景寫檔。背景寫檔有自己的狀態（待寫入的快照、執行緒），可以獨立成模組。

**其他**：`zhuyin-cli` 的 `parse_arguments` 約 300 行；`CorrectionRecorder` 的 `Session` 同時持有輸入歷史、選字、重打與事件索引；`CandidatePanel.update` 約 168 行，同時決定翻頁、量測文字、配置控制項與動畫。

## 狀態與不變條件

| 狀態 | 不變條件 | 目前怎麼維持 |
|---|---|---|
| 組字器的候選字視窗（6 個欄位） | 關閉時沒有選取；開啟時清單非空、選取在範圍內 | 呼叫端先檢查非空；`page_count - 1`、`choices.len() - 1` 依賴這個條件 |
| 組字器的計分設定（12 個 pub 欄位） | 修改後轉換要重算 | 沒有維持：C 介面改權重後畫面沿用舊結果 |
| `CandidateList`（Rust 與 Swift） | 頁大小至少為 1、選取在範圍內 | 建構函式；欄位仍可直接寫 |
| 詞庫的 `entries` 與 `reverse` | 兩個索引對應同一批資料 | 沒有維持：兩者都 pub |
| 個人模型的 `totals` | 等於各筆明細依目前半衰期的加總 | 更換設定時沒有重建（正式程式沒有呼叫） |
| 重排模型的快取 | 對應目前的參數 | 快取鍵漏了兩個 pub 參數 |
| 按鍵解碼器的 `states`／`alternatives`／`index` | 同一個索引對應同一個狀態 | 插入時同步更新 |
| 統計的 `dirty` | 為 false 表示資料已寫入 | 排程寫入前就清除 |

## SOLID 逐項

| 原則 | 評價 |
|---|---|
| 單一職責 | 組字器最差，引擎次之；其餘模組大致單一 |
| 開放封閉 | `ml/` 以登記擴充最好；引擎的按鍵行為靠大型分派 |
| 里氏替換 | 只有推論後端（Core ML、ONNX）有多個實作，介面一致 |
| 介面隔離 | 組字器的公開方法混合輸入、候選字、模型設定、學習管理 |
| 依賴反轉 | 模型以回呼與 trait 注入，方向正確；組字器直接依賴具體的 `Engine` |

## 發現

| 等級 | 問題 | 位置 | 確認 |
|---|---|---|---|
| P1 | 個人資料寫入失敗只印出錯誤，flush 仍回報完成 | `engine.rs` 的 `write_files` | 協調者 |
| P1 | C 介面修改字模型權重、變體計分後不重算 | `zhuyin-ffi/src/composer.rs` | 協調者 |
| P1 | 自訂詞編輯失敗後重試變成新增，舊詞留著 | `ListControllers.swift` | 協調者 |
| P1 | 統計檔的桶數沒驗證，檔案損壞時記錄按鍵會當掉 | `UsageStats.swift` | 協調者 |
| P1 | 統計寫入失敗後不再重試；統計與改字記錄刪除失敗仍顯示已清除 | `UsageStats.swift`、`CorrectionRecorder.swift` | 審查員 |
| P1 | 重排模型的快取鍵漏了 `context_length`、`unknown_penalty` | `zhuyin-models/src/reranker.rs` | 審查員 |
| P1 | 向量檢查遇到 NaN 會誤報通過 | `zhuyin-models/src/syllable.rs` | 審查員 |
| P1 | 命令列批次讀取失敗仍以 0 結束 | `zhuyin-cli/src/main.rs` | 審查員 |
| P1 | 評測續跑只看檔案是否存在；遠端成果取回失敗仍回報成功；強制重跑失敗後留下舊的完成標記；矩陣評測的報告檔名可能撞名 | `compare.py`、`runner.py`、`pipeline.py`、`sweep.py` | 審查員 |
| P1 | 分析頁開著時字詞數不更新 | `AnalyticsPane.swift` | 審查員 |
| P2 | 按鍵容錯的提前結束假設最小成本 4.5，學過的聲調誤按可降到 1，可能漏掉候選字；Swift 版就是這樣，修正會改變輸出 | `key_channel.rs` | 協調者 |
| P2 | 引擎載入時暫存的按鍵，換 App 後可能送到新的 App（時序未確認） | `ZhuyinInputController.swift` | 審查員 |
| P2 | 讀音表的位移加總可能溢位 | `reading_channel.rs` | 審查員 |
| P2 | 公開查詢在 `start` 超出範圍時 panic | `engine.rs` | 審查員 |
| P2 | 模型與檔案來的分數沒有檢查有限值（組字器回呼、匯入的錯字次數） | `composer.rs`、`engine.rs` | 協調者 |
| P2 | `CandidateList` 頁大小、詞庫索引、`PackedTree` 欄位公開可寫卻有不變條件 | `api.rs`、`API.swift`、`lexicon.rs`、`tree.rs` | 協調者 |
| P2 | 二進位語言模型以二分搜尋查詢，載入時沒驗證排序；映射檔被原地修改時驗證失效 | `binary/` | 審查員 |
| P2 | 嚮導的 `mode` 可直接寫入而繞過索引調整；練習以顯示文字判斷任務是否更換 | `WizardModel.swift`、`PracticeField.swift` | 審查員 |
| P2 | Swift 包裝的部分 setter 沒有清除快取 | `ZhuyinEngine/Composer.swift` | 審查員 |
| P3 | 個人模型更換半衰期不重建總計（正式程式沒有呼叫） | `personal.rs` | 協調者 |
| P3 | 超過 150 行的函式：`process()`、`present()`、`open_candidates()`、`parse_arguments`、`CandidatePanel.update`、`decode` | 多處 | 協調者 |

## 合理之處

- 模型以回呼注入（`Reranker`、`CharacterScorer`），推論後端可以替換；ONNX 與 Core ML 的輸出在 `zhuyin-models` 檢查長度與有限值。
- 轉換以 `Engine::conversion_revision` 判斷快取是否過期；有字模型分數時清除詞圖快取，因為分數依賴整句。
- 二進位詞庫載入時逐一檢查字串、索引與分數；個人資料載入時檢查欄位數與有限值。
- `conversion.rs` 的 `decode` 很長，是為了和 Swift 版逐行對照，註解有說明。
- `ml/` 的模型伺服器檢查輸入形狀與輸出的有限值；成對統計會檢查 ID 重複與缺漏。

## 改造順序

1. 組字器：不變條件檢查 → 計分設定封裝與失效 → 候選字視窗合成一個型別 → 畫面改成純轉換 → 字分數獨立成模組並驗證輸出 → 事件分派改成具名的方法。
2. 失敗回報：個人資料、統計、改字記錄、遠端取回、續跑與完成標記。
3. 公開可寫的不變條件：`CandidateList`、詞庫索引、`PackedTree`、重排模型參數。
4. 引擎：背景寫檔獨立成模組。
5. 需要評測的修正另外處理：按鍵容錯的提前結束、二進位檔的排序驗證（會增加載入時間）。


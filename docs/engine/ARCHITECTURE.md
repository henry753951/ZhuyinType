# ZhuyinType 架構與流程

本文件描述目前程式實際的運作方式：從按下一個鍵，到畫面上出現整句、打開候選字清單、送出與學習。各元件的演算法、參數怎麼選出來的與評測數字見 [MODEL.md](MODEL.md)，使用者看到的操作見 [DESIGN.md](../product/DESIGN.md)，後續規劃見 [ROADMAP.md](../ROADMAP.md)。

## 1. 總覽

```text
按鍵（keyCode、修飾鍵、按下的時間）
 │
 ├─ ZhuyinInputController.handle      IMK 事件；引擎還在載入時先保留文字鍵
 ├─ ControllerCore.keyDown            快捷鍵、中英切換、Caps Lock
 │    └─ prepareContext               組字開始時準備前文（§4）
 │
 ├─ Composer.handle                   組字狀態（§5）
 │    ├─ ① KeyDecoder                按鍵串切成音節，修復順序錯亂與多按的鍵
 │    ├─ ② Engine.matches            音節對到詞：按鍵容錯、模糊音、讀音機率
 │    ├─ ③ SyllableScorer（v3）       每個音節位置的字機率
 │    ├─ ④ Engine.convert／sentences  詞圖 Viterbi 與 N-best：混合語言模型、個人模型
 │    └─ ⑤ NeuralReranker（v2b）      從前 16 句挑一句：保留選過的字詞，打對的鍵優先
 │
 ├─ Presentation                      組字文字、游標、底線、候選字清單（§7、§8）
 ├─ ControllerCore.render             setMarkedText、insertText、候選字視窗
 │    └─ CorrectionRecorder           「記錄改字」開啟時：觀察按鍵、畫面與送出（§9.4）
 │
 └─ 選字與送出                         Engine.learn、recordTypos、weaken（§8、§9）
```

所有元件在同一個行程內直接呼叫。模型都在 CPU 上同步執行，每個鍵在主執行緒處理完。延遲、能耗與記憶體見 [MODEL.md §10](MODEL.md#10-速度能耗與記憶體)。

## 2. 程式模組

| 模組 | 位置 | 負責 |
|---|---|---|
| IMK 層 | `platforms/macos/Sources/ZhuyinTypeApp/IMK/` | `ZhuyinInputController` 接 IMK 事件；`IMKTextClient` 讀寫 App 的文字；`ControllerCore` 處理快捷鍵、中英切換、前文與畫面；`KeyMapping` 把 keyCode 轉成按鍵；`EngineHost` 在背景載入引擎與模型；`UpdateMonitor` 監看檔案變動；`CorrectionRecorder` 與 `CorrectionLog` 記錄改字（§9.4） |
| 介面 | `platforms/macos/Sources/ZhuyinTypeApp/UI/` | 候選字視窗（`CandidatePanel`）、設定視窗（`Settings/`） |
| 共用設定 | `platforms/macos/Sources/ZhuyinTypeApp/Shared/` | `Preferences`（設定與預設值）、`Shortcuts`（可重新指定的快捷鍵） |
| 自動更新 | `platforms/macos/Sources/ZhuyinTypeApp/Update/` | `Updater`（Sparkle） |
| 引擎的 Swift 包裝 | `platforms/macos/Sources/ZhuyinEngine/` | 引擎的值型別（`KeyInput`、`ComposerOptions`、`Presentation` 等）、`Engine`、`Composer`（經 C 介面呼叫 Rust 引擎），以及 `NeuralReranker`、`SyllableScorer` 的 Core ML 推論（由 Rust 回呼）；`EngineBench`（`Sources/EngineBench/`）是不含 App 的引擎路徑，量延遲用 |
| 核心 | `engine/crates/zhuyin-core/`（Rust） | `api`（按鍵、畫面與選項的介面）、`composer`（組字狀態；`candidates` 候選字視窗、`scoring` 字模型分數）、`key_decoder`、`key_channel`（按鍵容錯）、`engine`（詞庫、比對、學習）、`personal_writer`（個人資料的背景寫入）、`binary`（mmap 詞庫與語言模型）、`conversion`（詞圖解碼）、`language_model`（bigram、混合）、`personal`（個人模型） |
| 神經模型 | `engine/crates/zhuyin-models/`（Rust） | `NeuralReranker`（v2b 整句重排，`RerankerTree` 把候選句排成前綴樹）、`SyllableScorer`（v3 前文編碼器與音節對字編碼器）的判斷邏輯；推論在 macOS 由 App 以 Core ML 執行，在 Linux 由 `onnx` 模組以 ONNX Runtime 執行 |
| C 介面 | `engine/crates/zhuyin-ffi/` | `include/zhuyin.h`：引擎、組字器、個人資料、模型的 hook 與主機提供的推論 |
| 命令列 | `engine/crates/zhuyin-cli/` | 與 app 相同的引擎；評測、參數掃描、模型一致性檢查 |
| 測試 | `engine/crates/*/tests/` | 行為與延遲檢查、和 Swift 引擎逐位元相同的 golden 摘要（`tools/zt check engine`） |

## 3. 接收按鍵

`ZhuyinInputController.handle(_:client:)` 收到 keyDown 與 flagsChanged 事件。

- 同一個鍵事件送來兩次（時間戳相同）時，只處理第一次。
- 引擎還在載入時（`EngineHost.isLoading`），沒有 ⌘⌃⌥ 的文字鍵先保留，最多 64 個，載入完成後依序送進組字。英文模式與其他鍵直接交給 App。

`ControllerCore.keyDown(keyCode:modifiers:time:)` 依序處理：

1. 英文模式時，鍵交給 App。
2. Caps Lock 開著時，先送出組字、前文回到句首，鍵交給 App。
3. 比對快捷鍵表（`ShortcutCommand`，可在設定重新指定），對上的鍵換成對應的指令。
4. 其他鍵由 `KeyMapping.input` 依 US 配置的鍵位轉成 `KeyInput`，所以不論使用者選了哪種鍵盤配置，注音鍵位都相同。⌃ 加符號鍵轉成 `.controlChar`（⌃ 加數字、字母或 ⌥ 時除外）。轉不了的鍵（例如 ⌘ 組合鍵）先送出組字、前文回到句首，再交給 App。

單按 Shift（`flagsChanged`，依實體按鍵狀態判斷，0.35 秒內放開）切換中英，切換前先送出組字。

`ControllerCore.handle` 在組字開始時（沒在組字，又按了會打字的鍵）先準備前文（§4），再以 `NSEvent.timestamp` 呼叫 `Composer.handle(_:at:)`。

## 4. 前文

前文是游標前的文字，由 `ControllerCore.prepareContext` 在組字開始時準備，存在 `recentText`。

1. 安全輸入開著時（`IsSecureEventInputEnabled`，例如焦點在密碼欄），不讀也不保留前文，`recentText` 清空，這段組字也不學習（`Composer.learningSuspended`）。送出時同樣不記錄。
2. `recentText` 超過 10 分鐘沒有更新就清空（`contextWindow` = 600 秒）。
3. 設定「參考游標前的文字」關閉時到此為止。這時仍會用同一個輸入框裡剛送出的字，但不讀 App 的文字，也不跨輸入框保留。
4. 向 App 讀游標前最多 128 個 UTF-16 單位（`IMKTextClient.textBeforeInsertion`，用 `selectedRange` 與 `attributedSubstring`）。在文件開頭得到空字串。
   - 讀得到時，這段文字成為 `recentText`，並交給 `Composer.setPrecedingText`。`Engine.adaptDomain` 依它調整聊天與百科的比例（§6），`Engine.trailingWords` 從最後一段中文（最多 12 字）由尾端以詞庫最長比對切出最後兩個詞，語言模型接著這兩個詞算。結尾是標點、空白或換行時從句首算。
   - App 不提供時，若 `recentText` 是空的，改用同一個 App（依 bundle ID）10 分鐘內送出的字（`appContext`）。這段文字只給 v2b 與 v3 當前文，不調整聊天比例，也不改語言模型的接續。

前文的用途：

| 使用者 | 取用的長度 |
|---|---|
| v3 前文編碼器 | 最後 255 字 |
| v2b 整句重排 | 最後 24 字 |
| v2b 候選字排序 | 最後 24 字，前文是 `recentText` 加上組字中候選前面的字 |
| 語言模型的接續 | App 文字的最後兩個詞，或剛送出的最後兩個詞 |
| 聊天與百科的比例 | 讀到的整段 App 文字 |

組字開始之後，前文這樣更新：

| 事件 | `recentText` | 語言模型的接續 |
|---|---|---|
| 送出 | 加上送出的字，保留最後 255 字，並記為這個 App 的 `appContext` | 接在送出的最後兩個詞後面；最後是標點或符號時回到句首 |
| 傳給 App 的空白、Return | 加上空白或換行 | 回到句首 |
| 傳給 App 的 Backspace | 刪掉最後一個字 | 送出後 30 秒內接在還留著的詞後面（§9.3），其他時候回到句首 |
| 傳給 App 的其他鍵（方向鍵等） | 不變 | 回到句首 |
| 點擊或 App 移動游標（`commitComposition`）、中英切換、Caps Lock | 不變 | 先送出組字，再回到句首 |

這段文字只在記憶體中，不寫入磁碟。前文對準確率的影響見 [MODEL.md §8](MODEL.md#8-前文與接續)。

## 5. 組字與轉換

### 5.1 組字狀態

`Composer`（`engine/crates/zhuyin-core/src/composer/`）保存一段組字：

- `tokens`：已完成的音節（讀音與實際按的鍵）與標點、符號；
- `buffer`：還沒完成的音節，分聲母、介音、韻母三格；
- `run`：在組字尾端連續打的按鍵串，每按一鍵都重新切一次音節；
- `fixed`：選過或固定的字詞，轉換時不會變；
- `cursor`：游標位置，單位是音節。

游標在組字尾端、沒開清單也沒有選取時，注音鍵、聲調鍵與結束音節的空白加進按鍵串，刪除未完成音節的 Backspace 從按鍵串移除最後一鍵。其他鍵讓按鍵串固定下來，之後再打的鍵從新的按鍵串開始。標點鍵直接成為標點或符號（`ZhuyinKeyboard.literal`），⇧ 加符號鍵打出什麼依「標點按法」（`ComposerOptions.punctuationKeys`）：Windows 打出鍵上的半形符號，macOS 打出中文標點。⌃ 加符號鍵（`KeyInput.controlChar`）打出中文標點；沒有對應標點的 ⌃ 組合鍵先送出組字，再交給 App。

一段組字最多 60 個音節（`maxSyllables`）。會讓音節數超過上限的鍵（新音節的第一個注音鍵，或按鍵串重切後多出音節的鍵），先送出組字的前段，再照常處理這個鍵（`Composer.makeRoom`），所以每個鍵都會打出來：

- 前段的終點不超過游標（按鍵串在尾端時就是組字尾端），落在目前顯示的整句的詞與詞之間（`Composer.commitPoint`）：
  1. 游標前最後一個句子或子句標點（。！？，、；：… 與半形的 ! ? ;，後面緊接的右括號、右引號一起）之後，剩下的音節（含這個鍵）不超過上限的三分之二（40）時，送到那裡；
  2. 否則取前段不超過上限三分之一（20）個音節的最後一個詞界（至少一個音節；第一個詞就更長時取第一個詞），緊接在後的標點一起送出。
  游標在組字開頭、前面沒有詞界時不送出，這段組字暫時超過上限，游標往後移之後再送。
- 前段的文字就是畫面上的字（含選過的字與記住的字詞），送出時和 Return 一樣（`Composer.recordCommit`）：一樣學習（安全輸入時不學）、成為後段的前文（語言模型的歷史與最後兩個詞），送出後 30 秒內的刪除重打也接著它。前段刪掉後，選過的字詞與按鍵串的位置往前移；前段伸進按鍵串時（上限很小時才會），按鍵串只留未完成的音節與這個鍵。
- 送出的文字在這個鍵的 `KeyResult.commit` 傳回。組字器在重新轉換後段之前先通知主機（`Composer.setCommitListener`，C 介面 `zt_composer_set_commit_hook`），讓模型讀的前文接上它：App 的 `ControllerCore` 把它接在 `recentText` 後面，`zhuyin-cli` 接在一行的前文後面。後段以前段結尾的語言模型歷史重新轉換，只用引擎時就是原本整句的後段，畫面上留下的字不移動。
- 改字記錄把前段與後段記成同一筆（`committed` 是兩者相加，`typing` 重播時一樣分段送出），見 [DESIGN.md §2.7](../product/DESIGN.md#27-改字記錄)。
- `earlyCommitSyllables`（`zhuyin-cli --early-commit N`，預設 0，關閉）：大於 0 時，讓組字超過這個音節數的打字鍵先送出游標前最後一個句末標點（。！？…）為止的部分。`zhuyin-cli --max-syllables N` 改變上限。

按鍵串由 ① 切成音節。之後每次組字改變，`Composer.refresh` 重新轉換（②–⑤），再產生畫面（§7）。

### 5.2 ① 切音節（KeyDecoder）

`Composer.applyRun` 以 `KeyDecoder.decode` 用 Viterbi 搜尋按鍵串的切法，細節見 [MODEL.md §3](MODEL.md#3-按鍵層級解碼器keydecoder)。預設方法是 `timed`，按鍵容錯關閉時改用 `boundaries`（只決定音節在哪裡結束，鍵照實際按的算）。

| 項目 | 成本或數值 |
|---|---|
| 沒按空白的一聲，在下一個聲母前結束 | 4 |
| 同一格再按一次（保留後按的鍵） | 10 |
| 相鄰兩鍵順序顛倒（含聲母打在介音、韻母之後）：35 ms 內一起按下／分開按 | 2／12 |
| 多按一個注音鍵：35 ms 內和相鄰的鄰鍵一起按下／其他 | 4／15 |
| 結束一個不存在的音節 | KeyChannel 最便宜的修正成本 × 容錯強度 |
| 搜尋的 beam | 每個位置 24 個狀態；成本相同時依第一次出現的順序 |

取成本最低的切法。音節數相同、未完成的部分也相同的切法，若成本差在 6 以內（`splitMargin`），最多比較 6 種，取整句的引擎分數減去切法成本後最高的一種（`Composer.bestSplit`）。按鍵串超過 48 鍵時，除了未完成的音節，前面的部分固定下來，不再重切。

### 5.3 ② 比對詞庫（Engine.matches）

每個音節位置往後最多取 6 個音節，連同實際按的鍵交給 `Engine.matches`，找出詞庫中讀音對得上的詞。結果依按鍵與設定快取。

| 來源 | 成本 | 程式 |
|---|---|---|
| 完全符合的讀音 | 0 | `Engine.entries` |
| 沒打輕聲，詞庫是輕聲 | 0.4 | `Engine.entries` |
| 打了輕聲，詞庫不是輕聲 | 1.2 | `Engine.entries` |
| 按到相鄰的鍵 | 11 | `KeyChannel` |
| 3、4 聲調鍵互換／6、7 聲調鍵互換 | 5／4.5 | `KeyChannel` |
| 相鄰兩鍵對調 | 9 | `KeyChannel` |
| 多按或漏按一鍵 | 12.5 | `KeyChannel` |
| 模糊音，每換一組 | 3 | `Engine.fuzzyAlternatives` |

- 每個音節最多取 8 個修正讀音。4 鍵以上（含聲調鍵或空白）的音節可以有兩處修正。
- 容錯強度「強」把 KeyChannel 的成本乘以 0.7。
- 選過修正候選後，把某個鍵按成相鄰另一個鍵的成本降低 min(4, 1.5 × ln(1 + 次數))（§9）。
- 一個詞裡有修正的音節，3 個音節以內的詞最多 1 個，更長的詞最多 2 個。模糊音不計入。
- 模糊音每個音節最多換一組聲母與一組韻母，預設都不開。
- 讀音通道（App 預設開啟，「選字 › 按錯鍵 › 接受常見的其他讀法」可以關閉；`zhuyin-cli` 在 `--models` 的資料有 `channel.bin` 時照 App 開啟，`--channel off` 關閉）另外給每個音節最多 4 個「詞庫音節＋可以這樣打的字」，例如打 ㄕㄡˋ 時 疇 的 ㄔㄡˊ（範疇）。這條路徑只查載入時建好的索引裡、這個位置是這些字的詞，每個詞最多一個音節走它。接受的讀音不標註、不算修正；修正（有邊讀邊）算一次修正並標出詞庫讀音。成本與狀態見 [reading-channel.md](../../resources/formats/reading-channel.md) 與 [MODEL.md §2.5](MODEL.md#25-讀音通道引擎)。

方法與成本的來由見 [MODEL.md §4](MODEL.md#4-容錯keychannel)。

### 5.4 ③ 字分數（v3）

有 v3 模型時，`Composer.scoreCharacters` 收集每個音節位置可能出現的字：詞圖裡每個詞放在那裡的字，依詞庫分數取前 64 個；固定的字詞一定列入。每個音節連同無聲調讀音、聲調、修正成本分桶（打出的讀音存在時為 0；否則依最便宜的修正成本，6 以內為 1、11 以內為 2、更高為 3）與是否省略空白，交給 `SyllableScorer`，得到 log P(字 | 所有音節, 前文)。

讀音通道開啟時，只因變體進來、分數在同位置最好的字 3 nats 以內的字，截斷時不扣變體成本，v3 分數為 0（v3 用標準讀音訓練，看到打的讀音會壓低它們；重訓之前的過渡做法，見 reading-channel.md 第 7、8 點）。

前文編碼結果依前文快取，每段組字通常只算一次。音節與候選字都沒變時（例如只多打一個未完成的注音符號），直接沿用上一次的結果。模型細節見 [MODEL.md §6](MODEL.md#6-音節對字模型v3)。

### 5.5 ④ 詞圖解碼（Conversion）

`Engine.convert` 在詞圖上做 Viterbi，取最好的一句；`Engine.sentences` 取 N-best，不同句只保留一次。每條邊的分數見 §6。

- 每個位置保留 12 條路徑（`Engine.decodingBeamWidth`），N-best 時放寬到 K。
- 固定的字詞在它的範圍內只留這一條邊。
- 歷史從句首或前文的最後兩個詞開始（§4）。
- 沒有 v3 時，前面沒變的部分沿用上一次的解碼結果（`ConversionCache`）。v3 的分數看整段音節，所以有 v3 時每次重算。

### 5.6 ⑤ 整句重排（v2b）

有 v2b 模型時，`Composer.refresh` 這樣決定顯示哪一句：

1. 找出目前首選裡使用者在同一個地方選過的字詞（`Engine.remembers`、`PersonalLanguageModel.remembers`）：
   - 單字：在同一個前一個詞後面選過；
   - 多字詞：在任何地方選過。
2. 取引擎的前 16 句（`config.json` 的 `count`），只留下保留這些字詞的句子。一句都不剩時，維持引擎的首選。
3. `NeuralReranker.choose` 從留下的句子中挑一句：
   - 留下的句子中，前兩名的引擎分數差達 τ = 8 時（`gateMargin`），直接採用第一名，不執行模型；
   - 其他時候，總分 = 引擎分數 + 1.0 × log P_v2b(整句 | 前文)；
   - 最高分的句子有修正過的字時，要比最好的未修正句子多 4 分（`exactMargin`）才採用，否則採用那個未修正的句子。
4. 依選到的句子重建每個詞的語言模型歷史。

句子與前文都和上一次相同時，直接沿用上一次的模型分數。模型細節見 [MODEL.md §7](MODEL.md#7-整句重排v2b)。

## 6. 分數

詞圖每條邊，以及整句的分數：

```
邊分數 = LM(詞 | 歷史)
       + log P(讀音 | 詞)
       − 容錯扣分
       + 自訂詞加分
       + w_v3 × Σ log P_v3(字 | 所有音節, 前文)
句尾   + log(s · P(</s> | 最後的詞) + 1 − s)
重排   總分 = 引擎分數 + w_v2b × log P_v2b(整句 | 前文)
```

| 項目 | 說明 | 程式 |
|---|---|---|
| LM(詞 \| 歷史) | 百科與聊天兩個 bigram 的混合：P = c · P聊天 + (1 − c) · P百科。聊天比例 c 依讀到的前文調整，沒有前文時 c = 0.3（`Engine.chatPrior`）。有個人資料後，再依前一、兩個詞往使用者的一、二、三連詞拉近（§9）。語言模型沒有的詞，用詞庫的機率代替。自訂詞與記住的字詞照打的讀音出現時，這一項為 0。 | `MixtureLanguageModel`、`PersonalizedLanguageModel`、`DecodingEdge.transition` |
| log P(讀音 \| 詞) | 詞庫給的是 log P(詞, 讀音)。載入時拆成詞的機率（取最常見讀音的分數）與讀音比例：最常見的讀音為 0，少見的讀音為負。 | `WordEntry.readingPrior` |
| 容錯扣分 | §5.3 表中的成本加總，KeyChannel 部分乘上容錯強度。 | `Engine.matches` |
| 自訂詞加分 | 自訂詞的分數設為 max(詞庫分數, −7) + 1，加分是它和詞庫分數的差。 | `Engine.addPhrase`、`Engine.scoring` |
| v3 | w_v3 = 0.5（`ControllerCore.characterWeight`）。模型沒問到的字算 −12。 | `CharacterScores` |
| 句尾 | s 是句尾比例（`Engine.sentenceEndShare`），app 用 1，即 log P(</s> \| 最後的詞)。 | `LanguageModel.endScore` |
| 重排 | w_v2b = 1.0，取自 `reranker/config.json` 的 `weight`。引擎分數是整句的邊分數與句尾分數總和。 | `NeuralReranker.choose` |

c 的算法：把前文切詞，算每個詞在兩個模型的平均對數機率差 d，c = σ(logit(0.3) + min(1, 詞數 / 4) × (d + 2.5))。參數怎麼選的見 [MODEL.md §5.3](MODEL.md#53-混合與前文)。

## 7. 顯示

`Composer.refresh` 產生 `Presentation`：

- `text`：組字文字。依「台／臺」設定換成選定的寫法，預設把「臺」寫成「台」。只改顯示與送出的文字，轉換與學習仍用詞庫的寫法。
- `cursor`：游標，未完成的注音符號顯示在游標處。
- `clauses`：每個詞一段底線。
- `focusedClause`：候選字清單要換掉的部分。
- `targetClause`：游標在組字中間、清單沒開時，↓ 要改的字。
- `candidates`、`notice`、`zhuyinHint`：候選字清單、提示文字、清單下方顯示的注音。

`ControllerCore.render` 先以 `insertText` 送出要送出的字，再以 `setMarkedText` 更新組字，只在文字、游標或底線改變時才更新。要換掉的部分是粗的強調色底線加淡色底，↓ 要改的字是粗的強調色底線。設定為浮動組字的 App，組字顯示在候選字視窗上方。有清單、提示或浮動組字時顯示候選字視窗（`CandidatePanelController`）。`Presentation` 不合理時（例如範圍超出文字），把目前的字送出並清空組字。

## 8. 候選字清單與選字

### 8.1 開啟與按鍵

| 狀態 | 按鍵 | 結果 |
|---|---|---|
| 組字中 | ↓ | 開啟清單 |
| 組字中，沒有未完成的音節 | 空白 | 開啟清單（設定可改成送出） |
| 組字中 | ⇧↓／⇧↑ | 開啟只列單字／修正優先的清單 |
| 清單 | 1–9（或 asdfghjkl） | 選本頁第幾個；每頁的數量等於選字鍵的數量 |
| 清單 | Return | 選目前反白的候選 |
| 清單 | ↑↓（直式） | 移動反白 |
| 清單 | ←→（直式） | 換到前一個或後一個字的清單 |
| 清單 | 空白、Tab、⇧Tab | 換頁 |
| 清單 | ⇧↓、⇧↑ | 切換只列單字、修正優先 |
| 清單 | \` | 這段改成注音符號本身 |
| 清單 | Esc、Backspace | 關閉清單 |
| 清單 | 其他鍵 | 關閉清單，再照組字中處理 |
| 組字中 | Return | 送出 |
| 組字中 | Esc | 清空組字（設定可改成只清除注音） |

橫式清單的方向鍵相反：←→ 移動反白，↑↓ 換頁。

### 8.2 清單內容

`Composer.openCandidates` 決定要改的位置：

- 游標在組字尾端時，列出結束在游標的範圍；
- 游標在組字中間時，預設列出從游標右邊開始的範圍，設定可改成左邊。

範圍長度 1–6 個音節，遇到標點就停。標點的清單是同類標點的其他寫法。

清單依序是：

1. 最長範圍目前顯示的字；
2. 由長到短，每個長度先列目前顯示在那裡的詞（它是一個詞或選過的字詞時），再列詞庫裡讀音對得上的詞。

- 預設只列照實際按鍵對得上的詞。⇧↑ 時修正與模糊音的詞排在前面。
- ⇧↓ 只列一個字。
- 同一個詞的兩種寫法（台北、臺北）只列一次。最多 60 項。

同一個長度內的排序分數：

- 引擎分數：前面已轉換的字給的語言模型歷史、讀音機率與扣分；
- v3 在這個位置的字分數；
- v2b 分數：前 24 個再加上 1.0 × log P_v2b(候選 | 前文 + 組字中它前面的字)，重新排序（`candidateScorer`，每批 32 個）。

### 8.3 選字

`Composer.selectCandidate(at:)`：

1. 學習開著時，選了修正過的候選，`Engine.recordTypos` 記下哪個鍵按成了哪個鍵（§9）。
2. 學習開著時，原本顯示在那裡、被換掉的詞以 `Engine.weaken` 減弱，用的是那個詞前面的歷史（`historiesBefore`）。
3. 選的詞成為固定的字詞（`fixed`）。
4. 畫面上其他字詞也固定下來，標為 frozen（`freezesOnPick`，預設開）。選的是長詞裡的一部分時，那個長詞在選字處前面的字也照原樣固定；後面的字不固定，可以跟著選的字一起改對。之後只有新打的字會依前後文調整。

固定的字詞在其中插入或刪除音節時解除。選取 1–6 個字（⇧←、⇧→）後按 Return，`Engine.addUserPhrase` 加入自訂詞，並固定這段文字。

## 9. 送出與學習

`Composer.commitAll` 送出組字。未完成的音節照畫面上的注音符號送出，不猜聲調也不修正。學習開著、也不是安全輸入時，把每個詞交給 `Engine.learn`：選過的詞（不含 frozen）標為手動選字，語言模型接續的兩個詞（前一次送出的，或 App 文字的）作為前文。

### 9.1 學到什麼

| 記住什麼 | 什麼時候 | 程式 | 數字 |
|---|---|---|---|
| 個人一、二、三連詞 | 每次送出 | `PersonalLanguageModel.learn` | 手動選字的權重 0.1、0.3、1.0（一、二、三連詞），直接送出 0.05；短期半衰期 8 小時、長期 90 天；κ = 1000、100、1；最多 50,000 筆 |
| 記住的字詞（照打的讀音記住寫法） | 有手動選字的送出 | `Engine.learn`、`RecallValue` | 四種：整段（1–6 個音節）、修正處連同相鄰的兩個音節、按錯讀音選的字、詞庫沒有的詞；半衰期 8 小時與 90 天；選過 3 次以上保留最低分數；最多 20,000 筆 |
| 按錯習慣 | 選了修正過的候選 | `Engine.recordTypos` | 每次把那一對鍵的次數加 1 |
| 減弱 | 選字換掉了顯示的詞、送出後刪掉重打 | `Engine.weaken`、`PersonalLanguageModel.weaken` | 那個詞在原本前文的二、三連詞減半；記住的同一寫法也減半；選過的次數歸零 |

選過的字詞在整句重排時保留（§5.6）。個人模型的公式與評測見 [MODEL.md §9](MODEL.md#9-個人模型)。

### 9.2 寫入

個人資料存在 `~/Library/Application Support/ZhuyinType/`：`user_phrases.tsv`、`learned.tsv`、`excluded.tsv`、`typos.tsv`、`personal_ngrams.tsv`。改變後 150 毫秒在背景執行緒以原子寫入更新（`personal_writer`）；重新載入引擎與結束程序前會先寫完。寫入失敗時資料保留到下一次寫入，`flush_personal_data`（C 介面 `zt_engine_flush_personal_data`）回報失敗，App 記錄錯誤。

設定的「記住的字詞」可以搜尋、移除、全部清除、把個別字詞設為不記住，也可以匯出、匯入成一個文字檔。「記住選過的字詞」關閉時不學習。

### 9.3 送出後刪掉重打

送出後 30 秒內（`Composer.correctionWindow`），沒在組字時按 Backspace 刪掉剛送出的字，`Composer.followDeletion` 會跟著刪除的位置：

- 下一段組字的前文改成還留著的詞；
- 整個刪掉的詞記下來，連同它前面的歷史。

接著打出和刪掉的詞相同的讀音、送出不同的字時，`Composer.correctedPrefix` 把它當成改字：新的字以手動選字的權重學習，刪掉的字在原本的前文減弱。

### 9.4 改字記錄

「記錄改字」開啟時（[DESIGN.md §2.7](../product/DESIGN.md#27-改字記錄)），`ZhuyinInputController.refreshPreferences` 給每個 `ControllerCore` 一個 `CorrectionRecorder`；關閉時拿掉。記錄器只觀察，不改變任何結果，引擎也不知道它存在，所以 Swift 與 Rust 兩版引擎都不需要修改。

- `ControllerCore.handle`、`selectCandidate`、`toggleCorrections` 與 `commit` 在畫面更新之後呼叫 `CorrectionRecorder.observe`，傳入按鍵（或點按、送出）、按下的時間（`NSEvent.timestamp`；點按用同一個時鐘的現在時間）、處理前後的 `Presentation`、送出的字與按鍵是否交給 App；`breakContext` 也通知它。
- 記錄器依處理前的清單狀態分類按鍵：清單開著時，選字鍵、Return、方向鍵、翻頁、⇧↓ ⇧↑、Esc 與 ⌫ 是清單的鍵；其他鍵關閉清單後照組字中的規則處理。改變組字內容的鍵（注音、聲調、空白、標點、⌫、⌥⌫、⌦、Esc）是打字；開啟清單時順便完成音節的鍵（↓ 在未完成的注音上），以「該鍵加 `<esc>`」記在打字中，重播時效果相同。
- 打字的按鍵存成堆疊，每個鍵之後記下畫面的形狀（轉好的字數與結尾未完成的注音）。⌫、⌥⌫ 或 Esc 之後，回到上一次形狀相同的位置，中間的鍵就是刪掉的鍵；之後補打、直到轉好的字數回到刪除前的鍵是重打的鍵。第一次打的按鍵另存一份，刪掉的鍵留在那份裡、重打的鍵不加入；刪掉後沒有再打，就兩份一起刪。游標不在組字結尾時的修改無法這樣回推，該筆標為無法重播。
- Esc 或 ⌫ 清空組字後，30 秒內再開始的組字接在同一筆記錄；中間有交給 App 的鍵、點進文字或切換中英就放棄。送出後 30 秒內交給 App 的 ⌫ 逐一計數（送出並輸入空白、⇧Return 時先刪掉那個空白或換行），下一段組字開始時記成 `delete_after_commit`。
- 安全輸入時（`Composer.learningSuspended` 或 `IsSecureEventInputEnabled`）不記錄，進行中的記錄捨棄。游標前的文字（`recentText`）只在開啟第二個選項時於組字開始時讀取。
- 送出時，有改字的記錄交給 `CorrectionLog`：在自己的背景佇列（utility）以 `JSONEncoder`（snake_case、排序欄位）編碼，附加到 `corrections.jsonl`（第一次建立時權限 600），完成後在主執行緒通知設定視窗更新筆數。`EngineHost.prepareForExit` 先等佇列寫完。

## 10. 資料與模型

| 內容 | 位置（app bundle 的 `Contents/Resources/`） | 載入 |
|---|---|---|
| 詞庫 `lexicon.bin` | 根目錄 | mmap，不解析（ZTLX/1，`resources/formats/lexicon.md`）；只有 `lexicon.tsv` 時讀文字檔 |
| 百科語言模型 | `lm/lm.bin` | mmap（ZTLM/1，`resources/formats/lm.md`），bigram；也能讀 TSV |
| 聊天語言模型 | `lm_chat/lm.bin` | mmap；兩個模型都是 bigram 時和百科模型混合 |
| v2b 整句重排 | `reranker/RerankerTree.mlmodelc`、`vocab.json`、`config.json` | Core ML，只用 CPU。`config.json` 為 `{"weight": 1.0, "count": 16}`。整句重排與候選字排序（每次最多 32 個、每個 63 字）都排成前綴樹一次計算。同一份權重的批次模型 `Reranker.mlmodelc` 只在模型目錄沒有前綴樹時打包與載入；兩個都在時，前綴樹失敗才用批次模型 |
| v3 字模型 | `v3/ContextEncoder.mlmodelc`、`v3/SyllableEncoder.mlmodelc`、`chars.json`、`syllables.json`、`config.json` | Core ML，只用 CPU |
| 版本清單 | `manifest.json` | 版本與每個資料、模型檔的 SHA-256 |

`EngineHost.reload` 在背景佇列載入引擎與模型，完成後在主執行緒換上。模型目錄 `~/Library/Application Support/ZhuyinType/Models/` 裡的檔案優先於 app 內建的：`lexicon.tsv` 或 `lexicon.bin` 連同同一目錄的 `lm/`、`lm_chat/` 是一組，`reranker/` 與 `v3/` 各自獨立。

`UpdateMonitor` 以 FSEvents 監看 app 所在的資料夾與模型目錄，檔案停止變動 2 秒後由 `EngineHost.checkForUpdates` 判斷：

| 什麼變了 | 做法 |
|---|---|
| app 的版本號或執行檔 | 等所有輸入框都沒有組字（每個鍵之後與每 5 秒檢查），寫入個人資料、記下中英模式後結束程序。下一個按鍵由 macOS 啟動新版，1 分鐘內重新啟動時保留中英模式 |
| 資料或模型 | 在背景載入新的引擎與模型，`generation` 加 1。各輸入框在組字結束時換用新的 |

Sparkle 下載的更新也在所有輸入框都沒有組字時安裝（`Updater.installIfIdle`）。

## 11. 開發流程

1. 語料：維基百科、台灣口語、Taiwan Tongues。
2. 訓練：`ml/src/zhuyintype_ml/training/*`，在 GPU 主機上執行。
3. 產物：`data/build/*` 的 TSV 與 Core ML 模型，版本由 `resources/assets.json` 固定。
4. 評測：`zhuyin-cli` 使用和 app 相同的引擎，`ml/src/zhuyintype_ml/eval/*` 在開發集上調參數，測試集只用來追蹤。
5. 發布：`platforms/macos/scripts/release.sh` 產生 pkg 與 Sparkle 更新檔。

`zhuyin-cli` 的預設值和 app 不同：聊天模型、v3、v2b 要以 `--chat-lm`、`--v3`、`--reranker` 指定，重排權重 0.5、8 句，寫法用「臺」。要和 app 一致時以 `--rerank-weight 1.0 --rerank-count 16 --forms common` 指定。評測方式與指令見 [EVALUATION.md](../ml/EVALUATION.md)。

# Rust 引擎設計

本文件說明 ZhuyinType 的引擎從 Swift 移植到 Rust 的做法：workspace 的結構、每個 Swift 型別對應到哪裡、模型怎麼接上、C ABI 的形狀、App 與 Python 工具怎麼切換，以及每個階段的驗收條件與結果。移植在 2026 年 10 月完成（第 5 階段），Swift 引擎已移除；Rust 引擎和它逐位元相同。引擎本身的演算法見 [docs/engine/ARCHITECTURE.md](../docs/engine/ARCHITECTURE.md) 與 [docs/engine/MODEL.md](../docs/engine/MODEL.md)，這裡不重複。

## 1. 目的與要求

Linux（fcitx5、IBus）與 Windows（TSF）的輸入法要用同一個引擎。Rust 提供乾淨的 C ABI，不需要 Swift runtime，效能也好。

使用者訂的硬性要求：

- **行為完全相同**：同樣的按鍵得到同樣的轉換結果。
- **效能不能變差**：每鍵延遲與記憶體都不高於 Swift 引擎。
- macOS 的 Core ML 模型留在 Swift，由 Rust 引擎透過 callback 呼叫；Linux 與 Windows 用 ONNX Runtime。

移植期間 Swift 引擎（當時的 `engine/swift`）是參考實作，除了 §4 的決定性修正之外不修改；它在第 5 階段移除，最後一次比對見 §8.5。

## 2. Workspace

```text
engine/
├── Cargo.toml            workspace：成員、共用的套件資訊、依賴版本、lint、release 設定
├── rust-toolchain.toml   固定的工具鏈（rustfmt、clippy）
├── rustfmt.toml、deny.toml
├── crates/
│   ├── zhuyin-core/      純 Rust 引擎：資料層、解碼、組字、API 型別（第 1–3 階段）
│   ├── zhuyin-models/    reranker 與 v3 的判斷邏輯、推論的 trait、ONNX Runtime 後端（feature `onnx`）（第 3–4 階段）
│   ├── zhuyin-ffi/       C ABI：cdylib 與 staticlib，`include/zhuyin.h`、C 範例（第 4 階段）
│   └── zhuyin-cli/       取代 Swift 的 zhuyin-cli：相同的旗標與 batch 協定（第 3 階段）
└── parity/               golden 摘要（Swift 引擎的結果）、兩個建置的逐行比對（`gate.sh`、`cli_parity.py`），
                          以及和 Swift 引擎的最後一次比對（`reports/swift-final/`）
```

四個 crate 都已完成（第 1–4 階段）。macOS 的 Swift 包裝在 `platforms/macos/Sources/ZhuyinEngine`（§7）。移植期間另有 Swift 引擎（`engine/swift`）與 Swift 量測工具（`parity/Sources`），第 5 階段移除，都在 git 歷史中。

- Edition 2024，工具鏈固定在 1.98.0（`rust-toolchain.toml`）。
- `[workspace.lints]`：`unsafe_code = "deny"`、`missing_docs`、`unreachable_pub`；clippy `all` 為 deny、`pedantic` 為 warn，少數放寬的項目在 `Cargo.toml` 註明理由。`cargo clippy --all-targets -- -D warnings` 必須通過。
- Release：`lto = "fat"`、`codegen-units = 1`、`panic = "abort"`。輸入法程序裡 panic 不能跨過 C ABI 繼續執行，直接結束程序最安全；測試與 benchmark 不受這個設定影響。
- 依賴只有 `memmap2`（映射資料檔）、`serde_json`（`lm.json`、模型的字表與設定；開啟 `float_roundtrip`，數字的解析和 Swift 一樣正確捨入）與 `onnx` feature 的 `ort`（不含下載二進位檔的預設功能，連結系統的 ONNX Runtime）。開發用 `criterion`。沒有任何會連網的依賴。`cargo deny check`（所有 feature）檢查授權、重複版本與來源。
- `unsafe` 只在 `zhuyin_core::binary`（建立映射，以及把開檔時已驗證過的 UTF-8 區段轉成 `&str`）、`zhuyin-cli` 的資源量測，以及整個 `zhuyin-ffi`（C 邊界）。每個 unsafe 區塊附 SAFETY 說明（clippy `undocumented_unsafe_blocks` 為 deny）。

## 3. 模組對照

| Swift（`engine/swift/Sources`） | Rust | 階段 |
|---|---|---|
| `ZhuyinCore/API.swift`：`KeyInput`、`KeyResult`、`Candidate`、`CandidateList`、`Presentation`、`PunctuationKeys`、`TypoTolerance`、`FuzzySounds`、`CandidateAnchor`、`CharacterModelSyllable`、`SentenceWord`、`ScoredSentence`、`ComposerOptions`、`CharacterForms` | `zhuyin_core::api` | 1 |
| `Keyboard.swift`：`ZhuyinKeyboard`（注音鍵、聲調、標點、Windows／macOS 標點按法、`controlPunctuation`） | `zhuyin_core::keyboard` | 1 |
| `KeyChannel.swift`：`KeyChannel`、`ReadingAlternative` | `zhuyin_core::key_channel` | 1 |
| `KeyDecoder.swift`：`KeyDecoder`、`KeyDecoderMethod`、`Costs` | `zhuyin_core::key_decoder`（`KeyDecoderMethod` 在 `api`） | 1 |
| `LanguageModel.swift`：`LanguageModelHistory`、`LanguageModel`、`VocabularyLanguageModel`、`UnigramLanguageModel`、`BigramLanguageModel`、`MixtureLanguageModel`、`BoundaryFreeLanguageModel`、`scanTSV` | `zhuyin_core::language_model`、`zhuyin_core::tsv` | 1 |
| `MappedLexicon.swift` | `zhuyin_core::binary::MappedLexicon` | 1 |
| `NGramLanguageModel.swift` | `zhuyin_core::binary::NGramLanguageModel` | 1 |
| `Engine.swift`：`WordEntry`、TSV 詞庫載入 | `zhuyin_core::lexicon`（`WordEntry`、`TextLexicon`） | 1 |
| `Engine.swift`：音節是否存在、`validCodes` | `zhuyin_core::syllables::SyllableSet` | 1 |
| `Engine.swift`：其餘（比對、計分、自訂詞、記住的字詞、按錯習慣、模糊音、前文、學習、匯出匯入、寫入），`Engine` 的 static 設定 | `zhuyin_core::engine`（`Engine`、`EngineConfig`、`EngineModel`） | 2 |
| `Conversion.swift`：`CompositionToken`、`ConvertedWord`、`FixedWord`、詞圖 Viterbi、N-best、`ConversionCache`、`CharacterScores` | `zhuyin_core::conversion` | 2 |
| `PersonalLanguageModel.swift`：`DecayingCount`、`PersonalLanguageModel`、`PersonalizedLanguageModel` | `zhuyin_core::personal` | 2 |
| Swift 的 `"\(double)"`、`Character` | `zhuyin_core::swift_format`（`format_double`、`graphemes`） | 2 |
| `Composer.swift` | `zhuyin_core::composer`（`Composer`；Swift 的 `reranker`、`candidateScorer`、`characterScorer` closure 是 boxed `FnMut`） | 3 |
| `ZhuyinML/NeuralReranker.swift`、`RerankerTree.swift`：斷字、前綴樹、閘門與 `exactMargin` 的決定 | `zhuyin_models::reranker`（純 Rust） | 4 |
| `ZhuyinML/SyllableScorer.swift`：字表、音節表、輸入組裝、快取 | `zhuyin_models::syllable`（純 Rust） | 4 |
| `ZhuyinML/ONNXModel.swift`、`ONNXBridge` | `zhuyin_models::onnx`（feature `onnx`） | 4 |
| Core ML 推論 | 留在 Swift，經 `zhuyin-ffi` 的 callback 接入 | 4 |
| `zhuyin-cli/main.swift`（含 refactor-tools 分支的 `--models`）、`Usage.swift` | `zhuyin-cli`（`main.rs`、`usage.rs`） | 3 |
| `zhuyin-cli/ModelServer.swift` | `zhuyin-cli` 的 `--model-server` | 4 |
| `ZhuyinCoreChecks` | `zhuyin-core/tests/` | 1（資料層）、2（引擎、個人資料、轉換）、3（組字） |
| `ZhuyinMLTests/BackendTests.swift` | `zhuyin-models/tests/backend.rs`；`tests/onnx_vectors.rs`（ONNX 對 Core ML 的參考值） | 4 |

型別名稱沿用 Swift（`KeyChannel` 的函式在 `key_channel`，`MappedLexicon` 不改名），方便對照兩邊的程式。

### 3.1 和 Swift 寫法不同的地方

- **沒有全域可變狀態**。Swift 的 `KeyDecoder.costs`、`KeyChannel.hotspot`、`Engine.sentenceEndShare`、`chatPrior`、`domainEvidenceScale`、`domainEvidenceCentre`、`sentenceBoundaries`、`chatModelDirectory` 是 static 變數，CLI 啟動時設定。Rust 改成參數：`KeyDecoder::new(method, costs, …)`、`key_channel::alternatives(keys, hotspot, …)`、`LanguageModel::end_score(history, share)`；第 2 階段的 `EngineConfig` 集中保存，建立 Engine 時傳入。
- **語言模型以 enum 分派**。`VocabularyModel` 是 `NGram`／`Bigram` 的 enum，`MixtureLanguageModel` 以泛型持有兩個模型，每個詞的計分是靜態分派。Swift 是 existential（`any VocabularyLanguageModel`）。
- **零複製的查詢**。`MappedLexicon::entry_refs` 直接回傳映射中的 `&str`；`entries` 另外提供和 Swift 相同、會配置字串的版本。
- **KeyDecoder 的歷史以共用串列保存**。每個狀態的已完成音節是 `Rc` 串列，複製狀態不必複製整串音節；簽章用結構比較，不組字串。結果和 Swift 的字串簽章完全等價（讀音不含空白與 `|`，鍵只有注音鍵、聲調鍵與空白）。
- **混合模型的聊天比例**以 `AtomicU64` 保存，取代 `NSLock`。
- **詞圖的路徑節點放在 arena**（`ConversionCache` 擁有的陣列，以索引連結），取代 Swift 的 class 參考；沿用前一次解碼時只複製可重用位置的節點。邊的快取隨 `ConversionCache` 移交。所以 `Composer` 與它的快取可以在執行緒之間移動（`Send`）。
- **字串以 `Arc<str>` 共用**（`WordEntry`、`Match`、`ConvertedWord`、`CompositionToken`），複製比對結果只增加參考計數，對應 Swift 字串的寫入時複製。
- **個人資料在 Engine 自己的執行緒寫入**（channel 加 150 毫秒延遲），取代 `DispatchQueue`；Engine 釋放時寫完尚未寫入的資料。
- **`Character`**：Swift 以字形叢集計數與走訪（`word.count`、`for character in word`）。Rust 的 `swift_format::graphemes` 把一個字元連同其後的組合符號、異體字選擇器、ZWJ 視為一個叢集；中文、注音與 ASCII 每個字元各自一個，和 Swift 相同。
- **KeyDecoder 的成本**是 `Composer::decoder_costs`（Swift 是所有組字器共用的 `KeyDecoder.costs`），CLI 的 `--decoder-costs` 設定它。
- **字串比較**：Swift 的 `String` 以標準等價比較與排序，Rust 比較 UTF-8 位元組。兩者只在未經 NFC 正規化的文字上不同（例如 CJK 相容表意文字 U+F900–FAFF 與其統一表意文字）；目前的詞庫與模型沒有這種成對的詞（golden 測試的詞庫與 TSV 詞庫比對全部相同）。

## 4. 數值與決定性

所有分數用 `f64`，運算順序與 Swift 逐項相同（例如 `(backoff + α) + P`、`state.cost + (cost + 4)`）。`log`、`exp` 在 macOS 與 Linux 都呼叫系統的 libm，和 Swift 相同。`max`、`min` 依 Swift 的定義實作（NaN 的行為相同）。文字數字的解析依 Swift `Double(String)` 的語法（包括十六進位、不接受前後空白），寫出的數字依 Swift `Double.description`（最短的來回數字、同距時取偶數，`1e+16`、`9.999e-05` 等格式；6 萬個數值與 Swift 逐字相同）。

`DecayingCount` 的衰減 Swift 寫成 `pow(0.5, t)`；Swift 的最佳化（LLVM）把它換成 `exp2(−t)`，發布的引擎實際算的是後者（`-Onone` 的版本 20 萬個值中有 79 個差一個 ulp）。Rust 直接呼叫 `exp2`，所以不論最佳化等級，結果都和發布的 Swift 引擎相同。參考實作一律指 `-c release` 的 Swift。

Swift 的 Dictionary 與 Set 每個程序重新取雜湊種子。原本有幾處結果取決於走訪順序，同一個 Swift 執行檔每次執行的結果不一定相同（例如 49,304 次 KeyDecoder 解碼中，兩次執行之間約 36% 的結果列表不同，1.5%–1.7% 的最佳切法不同）。2026-10-27 起 Swift 引擎與 Rust 都用同樣的決定性順序（`docs/research/log.md`）：

| 地方 | 順序 |
|---|---|
| KeyDecoder 每個位置的狀態 | 成本相同時依簽章第一次出現的順序 |
| `PersonalLanguageModel.rebuildTotals` | 每個歷史的總和依數值由小到大相加 |
| `PersonalLanguageModel.trim`、`serialized` | 同分、同字時依詞 ID 序列 |
| `Engine.rebuildValidCodes` | 每一步中同一個代碼取 UTF-8 順序較大的讀音（目前的詞庫沒有這種衝突） |
| `Engine.entries` | 自訂詞與記住的字詞依詞排序 |
| `Engine.buildMatches` 的排序 | 長度、扣分、詞，最後「只限整段」的排在後面 |
| `Composer.scoreCharacters` 給 v3 的前 64 個字 | 同分時依字的 UTF-8 順序 |

四個開發集各 400 段的轉換在修改前後完全相同，KeyDecoder 的計時不變。

## 5. 模型

Swift 的模型類別已經把「推論」和「其他邏輯」分開：`SyllableScorer.Prediction`、`NeuralReranker.Prediction` 與 `treePrediction` 是純粹的張量推論（Linux 與 `--model-server` 就是換掉這幾個 closure）；字表、斷字、前綴樹、快取、閘門與 `exactMargin` 的判斷都在外面。Rust 沿用這條界線：判斷邏輯在 `zhuyin-models`（三個平台共用，所以行為相同），只有推論經由 trait。

- **`NeuralReranker`**：`vocab.json` 的字表、`<unk>` 的分攤（log 5465）、前文最多 24 字、32 句以內用前綴樹模型（`RerankerTree`）、否則逐句的批次、`candidate_scores` 每 32 句一批；`choose` 依序是閘門 τ（`gate_margin`，8）、上次結果的快取、切換門檻 δ（`switch_margin`，0）與 `exact_margin`（8，App 的設定下調整；main 的 70b14e7）。`configured` 讀 `config.json` 的 `weight` 與 `count`。
- **`SyllableScorer`**：`chars.json`、`syllables.json`、前文最多 255 字（換行是 `<sep>`）、每個音節最多 64 個候選字、上次請求的快取；`check_vectors` 對 `test_vectors.json` 檢查（`--check-v3`）。音節的成本級距、隱含邊界與 −12 的下限在組字器（第 3 階段已移植）。
- 推論的 trait（呼叫由上面兩者以鎖序列化，和 Swift 的 `NSLock` 相同）：

```rust
pub trait RerankerModel: Send {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError>;
    fn has_tree(&self) -> bool { false }
    fn tree_logprob(&mut self, tree: &PackedTree) -> Result<Vec<f64>, ModelError> { … }
}
pub trait SyllableModel: Send {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError>;
}
```

三種實作：

- **ONNX Runtime**（`zhuyin-models` 的 `onnx` feature，Linux、Windows）：`ort` crate 連結系統的 ONNX Runtime 1.24（`ORT_LIB_PATH` 指向它的 `lib/`，`engine/.cargo/config.toml` 要求動態連結，不下載任何二進位檔）；session 設定和 Swift 的 `ONNXBridge` 相同（CPU、intra/inter 各 1 個執行緒、全部圖最佳化、不送遙測），`float32` 輸出轉成 `f64`。前文編碼器的輸出依前文 ID 快取。
- **主機的 callback**（`zhuyin-ffi`，macOS）：Swift 以 Core ML 推論，註冊 `logprob`、`tree_logprob`、`predict` 與 `release`（§6）。Core ML 的呼叫和 `ZhuyinML` 的完全相同，所以分數相同。
- **模型伺服器**（`zhuyin-cli --model-server`，任何平台）：JSON-lines Unix socket，協定和 Swift 的 `ModelServer` 相同。

## 6. C ABI

`zhuyin-ffi`（函式庫名稱 `zhuyin`）產生 cdylib 與 staticlib，標頭 `crates/zhuyin-ffi/include/zhuyin.h` 由 cbindgen 產生後提交（`cbindgen.toml` 寫了指令；`tests/c_program.rs` 檢查標頭宣告了每個匯出的函式，並編譯、執行 `tests/c/smoke.c`）。

- **Handle**：`ZtEngine`（詞庫、語言模型、個人資料）、`ZtComposer`、`ZtReranker`、`ZtSyllableScorer` 都是不透明指標，由 `zt_*_open`／`zt_*_new`／`zt_reranker_configured` 建立、`zt_*_free` 釋放；Composer 持有 Engine 的參考計數，Engine 可以先釋放。
- **字串與資料**：輸入的字串是 UTF-8、以 NUL 結尾，只在呼叫期間借用。傳回的 `char *` 歸呼叫端，以 `zt_string_free` 釋放；`ZtPresentation`、`ZtPhraseList`、`ZtStringList` 有各自的 `_free`。按鍵是 `ZtKey`（`ZT_KEY_*` 種類與 Unicode 純量），選項是 `ZtComposerOptions`（列舉以 `ZT_*` 常數表示，超出範圍時拒絕），呈現是 `ZtPresentation`（UTF-16 的游標與子句、候選字清單、提示）。
- **個人資料**：自訂詞的新增與移除、學到的字詞與排除清單、單一字詞的遺忘、全部遺忘、匯出與匯入、立即寫入。
- **Hook**：組字器的三個模型 hook（`zt_composer_set_reranker`、`_set_candidate_scorer`、`_set_character_scorer`）是 C 函式加上 `context` 與 `release`。Hook 收到的是組字器資料的檢視（`ZtSentences`、`ZtStrings`、`ZtCandidateRows`），可以直接交給 `zt_reranker_choose`、`zt_reranker_candidate_scores_view`、`zt_syllable_scorer_scores_rows`，資料不必經過主機複製。
- **主機的模型**：`ZtRerankerModel`、`ZtSyllableModel` 是推論的 C callback（輸入、輸出都是呼叫端配置的連續陣列）；建立失敗時也會呼叫 `release`。
- **執行緒**：Engine 可以在多個執行緒同時使用；Composer 同一時間只能由一個執行緒使用；Reranker 與 SyllableScorer 可以共用，模型呼叫會序列化。Callback 在呼叫者的執行緒上同步執行，不能回頭呼叫同一個 Composer；檢視只在 callback 期間有效。標頭開頭寫明這些規則。
- **錯誤**：可能失敗的函式回傳 NULL、`false` 或 −1，訊息經 `char **error`。Release 版 panic 直接結束程序（§2）。

## 7. 切換

第 4 階段以建置開關讓 App 兩種引擎並存，逐項比對後在第 5 階段切換，現在 Rust 是唯一的引擎。

**macOS App**：

1. `build-app.sh` 先以 cargo 建置 `engine/target/release/libzhuyin.a`（本機架構），再建置 App。
2. `platforms/macos/Package.swift`：`CZhuyin`（module map 引入 `zhuyin.h`）、`ZhuyinEngine`（Swift 包裝，靜態連結函式庫）、App 與 `EngineBench`。
3. `ZhuyinEngine` 提供引擎的值型別（`KeyInput`、`ComposerOptions`、`Presentation` 等，原本在 Swift 引擎的 `API.swift`），以及 `Engine`、`Composer`、`NeuralReranker`、`SyllableScorer`，介面和 App 原本用的相同，所以 App 的其餘程式碼只換了 `import`。組字器的模型 hook 收到的是組字器資料的檢視（`ScoredSentences`、`CandidateTexts`、`CandidateRows`），直接交回 Rust 的模型判斷，不在 Swift 複製。
4. Core ML 留在 Swift（`Models.swift`），以 callback 交給 Rust 的判斷邏輯。Core ML 的呼叫和原本的 `ZhuyinML` 相同，所以分數相同。
5. `EngineBench`（`platforms/macos/Sources/EngineBench`）是不含 App 的引擎路徑：照 `EngineHost` 載入引擎與模型、照 `ControllerCore` 設定組字器，接受 `zt eval latency` 傳的參數；Mac 上的 `tools/zt eval latency` 量的就是它，`--check-v3` 經 Rust 與 Core ML 檢查 v3 的向量。
6. App 的使用統計與改字記錄（`UsageStatsObserver`、`CorrectionRecorder`）在 `ControllerCore`，只用到引擎的值型別，和引擎無關；兩種引擎下 49 項自我測試都通過。

**Python 工具**：`zt` 從目前的原始碼增量建置 Rust 的 `zhuyin-cli`（`cargo build --release`，Linux 上加 `onnx` feature 與 `ORT_LIB_PATH`），評測不會沿用舊的執行檔；`ZT_CLI` 可以指定其他執行檔。`zhuyin-cli` 重現 Swift CLI 的所有旗標、usage 文字、按鍵跳脫語法、輸出格式、錯誤訊息與結束碼。`tools/zt check engine` 執行 `cargo test --release`（原本的 `ZhuyinCoreChecks` 都已移植）。GPU 主機的準備、評測與 ZhuyinBench 都經由同一個 `zhuyin-cli`。

和 Swift CLI 的差異只剩一項：macOS 上的 `zhuyin-cli` 沒有 Core ML，`--reranker`、`--v3`、`--check-v3` 與含模型的 `--models` 需要 `--model-server` 或 `onnx` feature，否則印出說明後以結束碼 1 結束；`ZHUYIN_UNITS`（Core ML 的運算單元）不適用。Mac 上 App 的模型路徑由 `EngineBench` 量測與檢查。

**比對**：移除 Swift 引擎之後，`parity/gate.sh` 改成比對兩個 Rust 建置（這個分支與 main，`--reference DIR`），規則不變：不應改變行為的修改必須 0 差異。golden 摘要仍是 Swift 引擎的結果，只在有意改變行為時重新記錄。

## 8. 驗收條件

### 8.1 一致性

- **單元與 fixture 檢查**：`ZhuyinCoreChecks` 中每一項屬於已移植模組的檢查，都以 `#[test]` 移植，名稱註明對應的檢查訊息。
- **Golden 測試**（`crates/zhuyin-core/tests/golden.rs`）：Swift 量測工具（`parity/`）把 Swift 引擎的原始碼以 symlink 編進同一個執行檔，對真實資料做大量查詢，輸出每組結果的 FNV-1a 摘要（`parity/golden/digests.tsv`，浮點數以位元表示）。Rust 用同樣的方式產生同樣的查詢，摘要必須相同，也就是結果位元相同。輸入檔大小和產生摘要時不同就略過。
- **轉換比對**：第 2 階段起 golden 測試（`tests/golden_engine.rs`）以開發集的按鍵稿比對 `convert`、`sentences` 的完整結果；第 3 階段起 `zhuyin-cli --batch` 在所有開發集上的輸出逐行相同，`--report-nbest`、`--report-list` 也是。評測在 GPU 主機上執行，Mac 上只做一次性確認。
- **個人資料**：同一串操作寫出的 `user_phrases.tsv`、`learned.tsv`、`excluded.tsv`、`typos.tsv`、`personal_ngrams.tsv` 與匯出檔逐位元組相同。Swift 以 `description` 輸出 Double（例如 `1e+16`、`3e-05`），Rust 要實作同樣的格式。

### 8.2 效能

- 每個階段的 criterion benchmark（`cargo bench`）與 Swift 量測工具（`parity`，`SwiftParity bench`）跑相同的查詢，Rust 每一項都不慢於 Swift。
- 第 4 階段起以 `zhuyin-cli --timing`（`zt eval latency`）比較每鍵延遲的 p50、p95、最大值、每鍵 CPU 時間與能耗、記憶體占用與峰值、引擎載入時間；每項取 5 次執行的中位數，Rust 都不高於 Swift。App 的常駐記憶體另外量。

### 8.3 第 1–3 階段的結果

Golden 測試的輸入：0.4.0 App 內建的 `lexicon.bin`、`lexicon.tsv`、`lm/`、`lm_chat/`；`data/build/lm/test_sentences.txt` 前 2,000 句；三連詞模型 `data/build/lm2`；三個開發集（口語、維基、Taiwan Tongues）各取前 150 段漢字（4–20 字，和 `zt eval conversion` 的切法相同）。

| 項目 | 筆數 | 結果 |
|---|---:|---|
| 詞庫：讀音查詞、詞查讀音與最佳讀音、無聲調查詢、前綴、音節表 | 606,044 | 位元相同 |
| TSV 詞庫載入（讀音 prior、個人 ID 順序） | 229,288 | 位元相同 |
| 語言模型：百科、聊天、混合（0.3、0.7 與兩個分量）、無邊界、三連詞、TSV bigram 的 ID 與分數 | 1,202,622 | 位元相同 |
| 容錯通道：代碼表、修正讀音（含個人折扣、熱點成本） | 11,276 | 位元相同 |
| KeyDecoder（四種方法 × 12,326 串按鍵） | 49,304 | 位元相同 |
| `Double.description` | 59,994 | 逐字相同 |
| 轉換（app 的引擎：mmap 詞庫、混合模型）：首選的詞、範圍、ID 與歷史；16 句與分數；逐音節輸入與中間刪改的快取；固定字詞與分組；v3 字分數；維基的前文（`adaptDomain`、`trailingWords`）；按錯一鍵的版本 | 592 份按鍵稿 × 5 項，2,958 行 | 位元相同 |
| 模糊音與三種容錯強度；TSV 詞庫、不含語言模型的引擎 | 210 行 | 位元相同 |
| 學習：180 次送出（選字、減弱、自訂詞、按錯習慣，時間每次加 5 小時）後的轉換、`remembers`、修正讀音、記住的字詞；五個個人資料檔；匯出、匯入、重新載入 | 9,176 行 | 逐字相同 |
| 組字器：310 份按鍵稿（開發集的段落以送出、選字、修正清單、刪除重打、標點、選取、清單內移動、` 與其復原（再按 ` 或 Esc）等方式結束；快速打錯順序；目標稿）× 5 組選項（app、臺的寫法、舊的鍵盤設定、反向錨點／橫式清單／空白送出並輸入空白、Slots 解碼），每個按鍵的結果與整個 `Presentation`（文字、游標、底線、焦點、候選字清單與註記、提示），每份稿最後的 4 句與記住的字詞 | 10,370 個按鍵 × 5 | 逐字相同 |

CLI 比對（`parity/cli_parity.py`，GPU 主機）：Swift 與 Rust 的 `zhuyin-cli --batch` 在 21 組不含神經模型的設定（預設、容錯關閉與積極、三種非預設解碼器、全部模糊音、聊天混合、無句子邊界、`--end-share`、熱點、Mac 標點、通用字形、`--report-nbest 16`、四種 `--report-list`、選第二句、選字後學習與刪除重打（`--keep-memory`，依序執行））× 9 組輸入（口語、維基、聊天、維基加前文各 400 段，以 `zt eval conversion` 的方式選取；善解的 dev302 266 列、probe 71 列、typing76 49 列、fw 159 列、user-reported 77 列），共 189 組、46,662 行，全部逐字相同。

效能（Apple M5、release、每批的中位數；Swift 為同一份原始碼以 `-c release` 編譯；同一台機器同一段時間依序執行，數字隨機器負載起伏約 10%）：

| 項目 | Swift | Rust | Rust／Swift |
|---|---:|---:|---:|
| 詞庫 `entries`（2,102 個讀音） | 287 µs | 246 µs | 0.86 |
| 詞庫 `entry_refs`（不配置字串，同上） | — | 132 µs | — |
| 詞庫 `wordIndex`（2,485 個詞） | 216 µs | 187 µs | 0.87 |
| 詞庫 `containsPrefix`（2,102 個） | 906 µs | 293 µs | 0.32 |
| 開啟並驗證 `lexicon.bin` | 35.6 ms | 9.2 ms | 0.26 |
| 百科 bigram 計分（5,837 次） | 223 µs | 146 µs | 0.65 |
| 混合模型計分（5,837 次） | 561 µs | 365 µs | 0.65 |
| 三連詞計分（5,837 次） | 347 µs | 287 µs | 0.83 |
| `wordID`（5,837 個詞） | 559 µs | 445 µs | 0.80 |
| 開啟 `lm.bin` | 205 µs | 157 µs | 0.77 |
| 建立混合模型（對應 12 萬個 ID） | 15.2 ms | 10.6 ms | 0.70 |
| 容錯通道，不快取（393 串按鍵） | 13.8 ms | 6.1 ms | 0.44 |
| KeyDecoder `timed`，10／25／40 個音節 × 10 串 | 24.2／59.3／101.1 ms | 4.0／9.8／16.8 ms | 0.16–0.17 |
| 逐音節轉換（每打完一個音節轉換一次，用快取），10／25／40 個音節 × 10 串 | 192／677／1,543 ms | 90／306／714 ms | 0.45–0.47 |
| 16 句，10／25／40 個音節 × 10 串 | 37.4／74.8／237 ms | 16.0／30.9／116 ms | 0.41–0.49 |
| 組字器逐鍵輸入（app 的組字器，含 `bestSplit`），10／25／40 個音節 × 10 串（320／765／1,234 鍵） | 545／1,721／3,544 ms | 183／566／1,170 ms | 0.33 |

逐音節轉換的每次轉換平均：10 個音節 Swift 1.9 ms、Rust 0.9 ms；40 個音節 Swift 3.9 ms、Rust 1.8 ms。組字器每鍵平均：10 個音節 Swift 1.70 ms、Rust 0.57 ms；40 個音節 Swift 2.87 ms、Rust 0.95 ms。KeyDecoder 以整串按鍵解碼一次（40 個音節約 124 鍵）；組字器實際上超過 48 鍵就固定前面的部分。

完整的 CLI 路徑（`ml/eval-sets/latency_sentences.txt` 的 10 句各打 3 次，`zhuyin-cli --timing`，app 的引擎與聊天模型、不含神經模型；每句的中位數再取中位數）：

| 項目 | Swift | Rust |
|---|---:|---:|
| 每鍵 p50 | 1.70 ms | 0.35 ms |
| 每鍵 p95 | 7.15 ms | 2.35 ms |
| 每鍵最大值 | 8.50 ms | 2.65 ms |
| 每鍵 CPU | 2.5 ms | 0.7 ms |
| 每鍵能耗 | 13.8 mJ | 4.4 mJ |
| 每鍵指令數 | 53.3 M | 13.3 M |
| 記憶體占用 | 9.5 MB | 8.0 MB |

引擎檢查的延遲目標（40 個音節每鍵 < 3 ms 等）在 Rust 的 release 測試中同樣檢查，數字見 `tests/real_checks.rs` 的輸出：例如目前的百科模型逐鍵 p95 為 0.37／0.93 ms（一聲／輕聲容錯），`make checks` 中 Swift 為 0.91／2.39 ms（Swift 的檢查讀 `data/build` 的文字檔詞庫與模型，資料內容相同但載入方式不同，僅供參考；上表才是同條件的比較）。App 常駐記憶體的比較在第 4 階段。

### 8.4 第 4 階段的結果

**一致性**

| 項目 | 結果 |
|---|---|
| `zhuyin-models` 對 Swift `BackendTests` 的移植（輸入組裝、`<unk>` 分攤、批次、閘門、快取、切換門檻、`exactMargin`、`configured`、音節輸入與上限） | 全部通過 |
| v3 test vectors（Core ML 的參考值），GPU 主機以 ONNX Runtime：Swift CLI／Rust CLI | 兩邊都是 6/20 在 Core ML 的容許範圍內、最大差距 0.0828，數字完全相同（ONNX fp32 與 Core ML 本來就有這個差距，`zt models parity` 接受 0.1） |
| v3 test vectors，Mac 上 Rust 的判斷邏輯加 Core ML callback（`EngineBench --check-v3`） | 20/20，最大差距 0（Swift CLI 為 20/20、4.3e-19） |
| `onnx_vectors.rs`（GPU 主機）：v3 與 reranker 的 ONNX 輸出對 Core ML 的參考值（`zt models parity` 對 fp32 ONNX 接受 0.1） | v3 最大差距 0.083、reranker 0.061；`NeuralReranker::scores` 由文字組出的張量與參考資料相同 |
| CLI 比對，不含神經模型（`cli_parity.py`，21 組設定 × 9 組輸入） | 189 組、46,662 行，全部相同 |
| CLI 比對，App 的設定（`--models data/build`：reranker 含前綴樹模型、v3、聊天模型，兩邊都以 ONNX Runtime 推論；送出、16 句、兩種候選字清單、選第二句 5 組 × 9 組輸入） | 45 組、11,110 行，全部相同（main 的 70b14e7 之後，`exact_margin` 8） |
| Mac 上 App 的引擎路徑（`EngineBench`，Swift 模式與 Rust 模式，App 內建的資料與 Core ML 模型）：延遲句 10 句加測試句 30 段，每段送出的文字 | 40/40 相同 |
| `` ` `` 的復原（main 的 ece6f8d）：組字器 golden 加入 `` ` ` ``、`` ` Esc `` 的按鍵稿，`ZhuyinCoreChecks` 的新檢查移到 `composer_checks.rs` | 位元相同；通過 |
| C ABI：Rust 測試（組字器逐鍵對照 Rust API、選項檢查、個人資料、hook、主機的模型）與 C 範例 | 全部通過 |
| App 自我測試（`tools/zt check app`），Swift 模式與 Rust 模式 | 都是 40/40 |
| Linux 上的 golden 摘要（`gate.sh` 在主機上以 Swift 工具重新產生，Rust 的 golden 測試對照） | 全部相同 |

**效能**

Mac 上 App 的引擎路徑（`ZT_CLI=…/EngineBench tools/zt eval latency --data dist/ZhuyinType.app/Contents/Resources`：延遲句 10 句，每句一次組字，Core ML 的 reranker 與 v3；5 次執行的中位數）：

| 每鍵 | Swift 引擎 | Rust 引擎 |
|---|---:|---:|
| p50 | 7.4 ms | 3.35 ms |
| p95 | 20.5 ms | 13.55 ms |
| 最大值 | 23.3 ms | 16.7 ms |
| CPU 時間 | 7.75 ms | 4.8 ms |
| 能耗 | 41.67 mJ | 23.73 mJ |
| 記憶體（結束時） | 15 MB | 15 MB |

記憶體峰值（10 句在同一個程序裡打完，`/usr/bin/time -l` 的 peak memory footprint，3 次的中位數）：學習開啟（App 的預設）Swift 42.74 MB、Rust 42.66 MB；學習關閉 47.34 MB、46.99 MB。為了這一項，Rust 引擎的快取改成剛好大小的配置（比對結果、詞條、按錯的讀音），同一讀音的註記共用一份，模型的字表以 `char` 為鍵；修改前學習關閉時 Rust 多 6 MB。

GPU 主機上的 CLI（Linux、ONNX Runtime，`zt eval latency` 的 App 設定 `--models data/build`：延遲句 10 句；5 次執行的中位數；主機同時有其他工作，負載約 50，數字起伏大，但每次 Rust 都較低）：

| 每鍵 | Swift CLI | Rust CLI |
|---|---:|---:|
| p50 | 15.85 ms | 6.75 ms |
| p95 | 76.25 ms | 59.75 ms |
| 最大值 | 102.75 ms | 71.4 ms |
| CPU 時間 | 24.35 ms | 17.6 ms |
| 記憶體（結束時） | 739.0 MB | 720.5 MB |

Linux 上的 golden 摘要：提交的摘要在 macOS 上產生。Linux 的 glibc 與 Apple 的 libm 在少數數學函式上差最後一位（例如語言模型的句尾分數 4 行），所以 Linux 上以提交的摘要比對會不同；`gate.sh` 在主機上用 Swift 工具重新產生摘要（`ZHUYIN_GOLDEN`），再和 Rust 比對，兩邊用同一個 libm。

### 8.5 第 5 階段：切換

移除 Swift 引擎之前，最後一次完整比對（ba5078b，合併 main 的 e0f1e4b 之後）：CLI 比對 234 組、57,772 行，0 行不同；兩邊的 `--check-v3` 報告相同；golden 摘要在主機的資料上相同。報告在 `parity/reports/swift-final/`。合併 main 之後，Mac 上 App 的引擎路徑（`EngineBench`，Core ML）在 Swift 與 Rust 兩種模式下對 40 段文字送出的結果相同，App 自我測試兩種模式都是 49/49。

切換後，Rust 引擎的 App 與保留下來的 Swift 引擎建置（同一份 App 資料）交錯量測，各 5 次的中位數（Apple M5；Mac 上同時有其他 App 在跑，負載約 5）：

| 每鍵 | Swift 引擎 | Rust 引擎 |
|---|---:|---:|
| p50 | 7.35 ms | 3.4 ms |
| p95 | 19.8 ms | 12.85 ms |
| 最大值 | 23.1 ms | 16.8 ms |
| CPU 時間 | 8.2 ms | 4.7 ms |
| 能耗 | 44.61 mJ | 24.11 mJ |
| 記憶體（結束時） | 15 MB | 14 MB |

兩者送出的文字逐字相同（40/40）。

## 9. 階段

| 階段 | 內容 | 完成條件 |
|---|---|---|
| 1 資料層 | API 值型別、鍵盤與標點、詞庫（mmap 與 TSV）、語言模型（mmap、TSV、混合、無邊界）、容錯通道、KeyDecoder、音節表；Swift 量測工具 | 完成：§8.3 |
| 2 解碼 | `Conversion`（詞圖 Viterbi、N-best、快取）、`Engine` 的比對與計分、`PersonalLanguageModel`、學習、自訂詞、記住的字詞、按錯習慣、個人資料檔與匯出匯入、背景寫入；Swift 引擎的決定性順序（§4） | 完成：§8.3 |
| 3 組字、API、CLI | `Composer`、`zhuyin-cli`（所有旗標與 batch 協定、`--models`；模型旗標除外，§7）、`zhuyin-models` 的計分 trait、其餘全部 `ZhuyinCoreChecks`（含延遲檢查） | 完成：§8.3 |
| 4 模型與整合 | `zhuyin-models`（Rust 判斷邏輯、ONNX Runtime）、CLI 的模型旗標、`zhuyin-ffi` 與標頭、macOS App 經 callback 接 Core ML（建置開關）、`EngineBench` | 完成：§8.4 |
| 5 切換 | App、評測工具、GPU 主機改用 Rust；文件更新；移除 Swift 引擎 | 完成：§8.5；0.4.2 以 Rust 引擎發布 |

五個階段都已完成。之後的引擎改變直接在 Rust 上做，以 `parity/gate.sh` 和 main 的建置比對（§7「比對」）。

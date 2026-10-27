# Rust 引擎設計

本文件說明把 `engine/swift`（Swift）的輸入引擎移植到 Rust 的做法：workspace 的結構、每個 Swift 型別對應到哪裡、模型怎麼接上、C ABI 的形狀、App 與 Python 工具怎麼切換，以及每個階段的驗收條件。引擎本身的演算法見 [docs/engine/ARCHITECTURE.md](../docs/engine/ARCHITECTURE.md) 與 [docs/engine/MODEL.md](../docs/engine/MODEL.md)，這裡不重複。

## 1. 目的與要求

Linux（fcitx5、IBus）與 Windows（TSF）的輸入法要用同一個引擎。Rust 提供乾淨的 C ABI，不需要 Swift runtime，效能也好。

使用者訂的硬性要求：

- **行為完全相同**：同樣的按鍵得到同樣的轉換結果。
- **效能不能變差**：每鍵延遲與記憶體都不高於 Swift 引擎。
- macOS 的 Core ML 模型留在 Swift，由 Rust 引擎透過 callback 呼叫；Linux 與 Windows 用 ONNX Runtime。

移植期間 Swift 引擎（`engine/swift`）是參考實作，除了 §4 的決定性修正之外不修改。完成切換（第 5 階段）後才移除。

## 2. Workspace

```text
engine/
├── Cargo.toml            workspace：成員、共用的套件資訊、依賴版本、lint、release 設定
├── rust-toolchain.toml   固定的工具鏈（rustfmt、clippy）
├── rustfmt.toml、deny.toml
├── crates/
│   ├── zhuyin-core/      純 Rust 引擎：資料層、解碼、組字、API 型別（第 1–3 階段）
│   ├── zhuyin-models/    模型的計分 trait（第 3 階段）；ONNX Runtime 實作與 Rust 的判斷邏輯（第 4 階段）
│   ├── zhuyin-ffi/       C ABI：cdylib 與 staticlib，`include/zhuyin.h`（第 4 階段）
│   └── zhuyin-cli/       取代 Swift 的 zhuyin-cli：相同的旗標與 batch 協定（第 3 階段）
├── swift/                Swift 引擎（參考實作，第 5 階段移除）
└── parity/               Swift 量測工具：產生 golden 值與 Swift 端的效能數字
```

目前有 `zhuyin-core`（第 1–3 階段）、`zhuyin-models`（只有計分 trait）與 `zhuyin-cli`（第 3 階段）。`zhuyin-ffi` 在第 4 階段加入。

- Edition 2024，工具鏈固定在 1.98.0（`rust-toolchain.toml`）。
- `[workspace.lints]`：`unsafe_code = "deny"`、`missing_docs`、`unreachable_pub`；clippy `all` 為 deny、`pedantic` 為 warn，少數放寬的項目在 `Cargo.toml` 註明理由。`cargo clippy --all-targets -- -D warnings` 必須通過。
- Release：`lto = "fat"`、`codegen-units = 1`、`panic = "abort"`。輸入法程序裡 panic 不能跨過 C ABI 繼續執行，直接結束程序最安全；測試與 benchmark 不受這個設定影響。
- 依賴只有 `memmap2`（映射資料檔）與 `serde_json`（`lm.json`，之後的模型設定檔）。開發用 `criterion`。沒有任何會連網的依賴。`cargo deny check` 檢查授權、重複版本與來源。
- `unsafe` 只在 `zhuyin_core::binary`：建立映射，以及把開檔時已驗證過的 UTF-8 區段轉成 `&str`。每個 unsafe 區塊個別允許，並附 SAFETY 說明（clippy `undocumented_unsafe_blocks` 為 deny）。

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

## 5. 模型的計分介面

Swift 的模型類別已經把「推論」和「其他邏輯」分開：`SyllableScorer.Prediction`、`NeuralReranker.Prediction` 與 `treePrediction` 是純粹的張量推論（Linux 與 `--model-server` 就是換掉這幾個 closure）；字表、斷字、前綴樹、快取、閘門與 `exactMargin` 的判斷都在外面。Rust 沿用這條界線：判斷邏輯在 Rust（三個平台共用，所以行為相同），只有推論經由 trait。

```rust
/// v2b：每個位置的 log P(target | 前面的字)。
pub trait RerankerModel: Send + Sync {
    fn logprob(&self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError>;
    /// 前綴樹模型（RerankerTree）；沒有時回傳 None，改用逐句的模型。
    fn tree_logprob(&self, tree: &PackedTree) -> Option<Result<Vec<f64>, ModelError>>;
}

/// v3：每個音節在候選字上的 log P。實作可以依前文快取前文編碼器的輸出。
pub trait SyllableModel: Send + Sync {
    fn predict(&self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError>;
}
```

- **macOS**：Swift 以 Core ML 實作推論，透過 `zhuyin-ffi` 註冊一組 C callback（`ctx` 指標、`logprob`、`tree_logprob`、`predict`、`release`）。Rust 端的 `CallbackRerankerModel`、`CallbackSyllableModel` 包裝這組 callback 實作上面的 trait。輸入、輸出都是呼叫端配置的連續陣列，callback 不保留指標。callback 在呼叫 `zt_composer_handle` 的執行緒上同步執行，和現在一樣。
- **Linux、Windows**：`zhuyin-models` 的 `onnx` feature 以 ONNX Runtime 載入 `Reranker.onnx`、`RerankerTree.onnx`、`ContextEncoder.onnx`、`SyllableEncoder.onnx`，以動態載入的方式使用輸入法附帶的 onnxruntime 函式庫。精度與現在 Linux 的 `ONNXBridge` 相同（fp32）。
- **評測**：`zhuyin-cli --model-server` 也實作成這兩個 trait（JSON-lines socket），GPU 主機上的評測流程不變。
- **驗證**：沿用 `--check-v3` 的 test vectors（`ml/src/zhuyintype_ml/training/v3`），並為 v2b 加上同樣的向量；相同輸入的模型輸出差距在現有的容許範圍內（0.002 + 0.002 × |值|），決定（選哪一句）在開發集上逐句比對。

## 6. C ABI

`zhuyin-ffi` 產生 `libzhuyin.dylib`／`.so`／`zhuyin.dll` 與靜態函式庫，標頭 `include/zhuyin.h` 以 cbindgen（開發工具，不是依賴）產生後提交，CI 檢查是否最新，並編譯一個 C 範例確認可用。

```c
typedef struct ZtEngine ZtEngine;       // 不透明
typedef struct ZtComposer ZtComposer;   // 不透明

uint32_t zt_abi_version(void);
ZtEngine *zt_engine_open(const ZtEngineConfig *config, ZtError **error);  // 失敗時回傳 NULL
void zt_engine_flush(ZtEngine *engine);                                   // 寫完個人資料
void zt_engine_free(ZtEngine *engine);
ZtComposer *zt_composer_new(ZtEngine *engine, const ZtComposerOptions *options);
ZtKeyResult zt_composer_handle(ZtComposer *composer, ZtKey key);          // key：種類、Unicode 純量、時間
const ZtPresentation *zt_composer_presentation(ZtComposer *composer);     // 下次呼叫前有效
void zt_string_free(char *string);
void zt_error_free(ZtError *error);
```

- **所有權**：handle 由 `*_open`／`*_new` 建立、`*_free` 釋放；Composer 持有 Engine 的參考計數，Engine 可以先釋放。Rust 配置的字串由 `zt_string_free` 釋放。借出的指標（`zt_composer_presentation`）在同一個 Composer 的下一次呼叫前有效。
- **字串**：一律 UTF-8、以 NUL 結尾。輸入的字串無效時回傳錯誤，不 panic。`Presentation` 的游標與底線範圍保留 UTF-16 單位（IMK 與 TSF 都用 UTF-16），另外提供位元組範圍給 fcitx5、IBus。
- **執行緒**：Engine 可以在多個執行緒共用（內部以鎖保護，和 Swift 的 `NSLock` 相同）；個人資料在 Engine 自己的背景執行緒寫入（150 毫秒延遲、原子取代檔案）。Composer 同一時間只能由一個執行緒使用（IMK 的主執行緒、fcitx5 的事件迴圈、TSF 的 STA 執行緒），不需要鎖。
- **錯誤**：可能失敗的函式回傳 NULL 或錯誤碼，詳細訊息經 `ZtError**`。Release 版 panic 直接結束程序（§2）；ABI 函式內不使用會 panic 的索引或 `unwrap`，除錯版以 `catch_unwind` 檢查。
- **版本**：`zt_abi_version()`。Swift 端在載入時比對，不相符就不使用。

## 7. 切換

**macOS App**：

1. `build-app.sh` 以 cargo 建置 `zhuyin-ffi` 的靜態函式庫（arm64 與 x86_64，再以 `lipo` 合併）。
2. `clients/macos` 加入一個 Swift 包裝模組（system library target，以 module map 引入 `zhuyin.h`），提供和 `ZhuyinCore` 相同的 `Engine`、`Composer`、`Presentation` 介面，`ControllerCore` 幾乎不用改。
3. Core ML 推論改寫成註冊給 Rust 的 callback（§5）。
4. 第 4 階段以建置旗標並存兩個引擎，同一份按鍵檔比對結果與延遲；第 5 階段移除 Swift 引擎與 `engine/swift`。

**Python 工具**：`zt eval` 的工具都以 `--cli PATH` 指定執行檔，並透過 `--batch` 的標準輸入、輸出溝通。Rust 的 `zhuyin-cli` 重現所有旗標、usage 文字、按鍵跳脫語法（`<space>`、`<S-x>`、`<dt:MS>`…）、輸出格式、錯誤訊息與結束碼。切換時只要把 `--cli` 指到 `engine/target/release/zhuyin-cli`；確認一致後再改各工具的預設路徑與 GPU 主機（`ml/src/zhuyintype_ml/remote`）的建置（cargo，Linux 上加 `onnx` feature）。

第 3 階段的 Rust CLI 支援所有不需要神經模型的旗標，`--models DIR`（refactor-tools 分支）也照該分支的規則設定詞庫、`lm_chat/` 與 reranker 的 `count`。尚未支援、在第 4 階段加入的：`--reranker`、`--v3`、`--model-server`、`--check-v3`，以及目錄裡有 `reranker/` 或 `v3/` 時的 `--models`；用到時 CLI 印出說明後以結束碼 1 結束。只對模型有作用的 `--rerank-weight`、`--rerank-count`、`--exact-margin`、`--rerank-gate`、`--rerank-switch`、`--v3-weight`、`--v3-list`、`--v3-boundaries` 照 Swift 的規則檢查數值，沒有模型時不影響結果。usage 文字已包含 `--models`（main 分支的 Swift CLI 尚未加入）。`--timing` 在 macOS 上和 Swift 一樣回報延遲、CPU、能耗、指令數與記憶體。

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
| 組字器：303 份按鍵稿（開發集的段落以送出、選字、修正清單、刪除重打、標點、選取、清單內移動等方式結束；快速打錯順序；目標稿）× 5 組選項（app、臺的寫法、舊的鍵盤設定、反向錨點／橫式清單／空白送出並輸入空白、Slots 解碼），每個按鍵的結果與整個 `Presentation`（文字、游標、底線、焦點、候選字清單與註記、提示），每份稿最後的 4 句與記住的字詞 | 10,398 個按鍵 × 5 | 逐字相同 |

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

## 9. 階段

| 階段 | 內容 | 完成條件 |
|---|---|---|
| 1 資料層 | API 值型別、鍵盤與標點、詞庫（mmap 與 TSV）、語言模型（mmap、TSV、混合、無邊界）、容錯通道、KeyDecoder、音節表；Swift 量測工具 | 完成：§8.3 |
| 2 解碼 | `Conversion`（詞圖 Viterbi、N-best、快取）、`Engine` 的比對與計分、`PersonalLanguageModel`、學習、自訂詞、記住的字詞、按錯習慣、個人資料檔與匯出匯入、背景寫入；Swift 引擎的決定性順序（§4） | 完成：§8.3 |
| 3 組字、API、CLI | `Composer`、`zhuyin-cli`（所有旗標與 batch 協定、`--models`；模型旗標除外，§7）、`zhuyin-models` 的計分 trait、其餘全部 `ZhuyinCoreChecks`（含延遲檢查） | 完成：§8.3 |
| 4 模型與整合 | `zhuyin-models`（Rust 判斷邏輯、ONNX Runtime）、`zhuyin-ffi` 與標頭、macOS App 經 callback 接 Core ML、Linux 建置 | 模型向量檢查通過；加上模型的 batch 輸出相同；`mac_metrics.py` 的延遲、能耗、記憶體不高於 Swift |
| 5 切換 | App、評測工具、GPU 主機改用 Rust；文件更新；移除 Swift 引擎 | 一個版本以 Rust 引擎發布，ZhuyinBench 成績相同 |

第 4 階段從 `zhuyin-models` 開始：先把 `NeuralReranker` 與 `SyllableScorer` 的判斷邏輯（斷字、前綴樹、快取、閘門、`exactMargin`、字表與音節表）移到 Rust，以 `--check-v3` 的 test vectors 與 reranker 的向量檢查；接著是 ONNX Runtime 後端與 CLI 的 `--reranker`、`--v3`、`--model-server`，在 GPU 主機上比對 app 設定（含模型）的 batch 輸出；最後是 `zhuyin-ffi` 與 macOS App 的整合。

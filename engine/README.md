# ZhuyinType 引擎

所有平台共用的輸入法引擎，以 Rust 寫成。macOS App 經由 C 介面（`zhuyin-ffi`）使用它，神經模型在 App 裡以 Core ML 執行；評測工具與 GPU 主機經由 `zhuyin-cli`，模型以 ONNX Runtime 執行；之後的 Linux、Windows 輸入法也用同一個引擎。

引擎原本以 Swift 寫成，2026 年 10 月移植到 Rust：每個結果都和 Swift 版逐位元相同，每鍵反應更快、耗電更少。移植的做法、模組對照、各階段的驗收數字見 [DESIGN.md](DESIGN.md)；引擎的流程見 [docs/engine/ARCHITECTURE.md](../docs/engine/ARCHITECTURE.md)，方法與評測數字見 [docs/engine/MODEL.md](../docs/engine/MODEL.md)。

| Crate | 內容 |
|---|---|
| `zhuyin-core` | 鍵盤、按鍵解碼、容錯、詞庫與語言模型（mmap）、詞圖解碼、個人模型、組字器 |
| `zhuyin-models` | 整句重排（v2b）與音節對字模型（v3）的判斷邏輯；推論接在 trait 後面，`onnx` feature 以 ONNX Runtime 執行 |
| `zhuyin-ffi` | C 介面（`include/zhuyin.h`）：引擎、組字器、個人資料、模型的 hook 與主機提供的推論 |
| `zhuyin-cli` | 從終端機打字；所有評測都經由它（`--batch`、`--models`、`--timing` 等） |

## 開發

在這個目錄（`engine/`）執行；工具鏈由 `rust-toolchain.toml` 固定（rustup 會自動安裝）。

```sh
cargo fmt --check                               # 格式
cargo clippy --all-targets -- -D warnings       # lint，不允許任何警告
cargo test                                      # 單元、fixture 與 golden 測試（golden 與延遲目標建議加 --release）
cargo bench                                     # criterion benchmark（需要下方的資料）
cargo deny check                                # 授權、重複版本、來源（cargo install --locked cargo-deny）
cargo build --release -p zhuyin-cli             # target/release/zhuyin-cli
cargo build --release -p zhuyin-ffi             # target/release/libzhuyin.{a,dylib,so}：C 介面
```

在 repository 根目錄，`tools/zt check engine` 執行 `cargo test --release`，`tools/zt build cli` 建置 `zhuyin-cli`（Linux 上含 ONNX Runtime）；評測一律從目前的原始碼增量建置 `zhuyin-cli`，不會沿用舊的執行檔。

- **ONNX Runtime**（Linux、Windows 上的模型）：`ORT_LIB_PATH=$ONNXRUNTIME_ROOT/lib cargo build --release -p zhuyin-cli --features onnx`。連結系統的 ONNX Runtime 1.24（`.cargo/config.toml` 要求動態連結，不下載任何二進位檔），執行檔與測試記錄該目錄為 rpath。`cargo test -p zhuyin-models --features onnx` 以 `data/build`（或 `ZHUYIN_MODELS`）的模型對 Core ML 的參考值檢查。
- **C 標頭**：改了 `zhuyin-ffi` 的介面後執行 `cbindgen --config crates/zhuyin-ffi/cbindgen.toml --crate zhuyin-ffi --output crates/zhuyin-ffi/include/zhuyin.h`（`cargo install --locked cbindgen`）。`cargo test -p zhuyin-ffi` 檢查標頭是否最新，並以 `cc` 編譯、執行 `tests/c/smoke.c`。
- **macOS App**：`build-app.sh` 先建置 `libzhuyin.a`，`platforms/macos/Sources/ZhuyinEngine` 把它包成 App 用的 Swift 型別，並以 Core ML 執行模型。App 的引擎路徑（不含 App）是 `EngineBench`：`tools/zt build bench`；Mac 上的 `tools/zt eval latency` 量的就是它，`EngineBench --check-v3 data/build/v3` 經 Rust 與 Core ML 檢查 v3 的向量。

## 比對

引擎的改變在 GPU 主機上和 main 的建置逐行比對，從 repository 根目錄執行（需要 ONNX Runtime；`data/` 指向評測資料；`DIR` 是另一份 main 的 checkout）：

```sh
engine/parity/gate.sh --quick --reference DIR     # 快速比對：開發時每次修改都跑，幾分鐘
engine/parity/gate.sh --reference DIR             # 完整比對：合併前、合併 main 之後各跑一次
```

`gate.sh` 先以 32 個編譯工作同時建置兩邊的 `zhuyin-cli`（各自的 `engine/target`），再同時執行：`cli_parity.py` 的兩組比對（不含神經模型的 21 組設定，以及 App 的設定 `--models data/build`，以 ONNX Runtime 推論）、兩邊的 `--check-v3` 報告是否相同，完整比對再加上 `cargo test --release`。結果寫在 `gate-out/`，全部通過時結束碼為 0。

- 不應改變行為的修改（重構、效能）必須 0 差異；有意改變行為的修改，差異的範圍就是它的影響，要附上評測。
- 快速比對每組輸入只取前 50 行；完整比對取全部（四個開發集各 400 段，加上善解的列）。其他參數直接交給 `cli_parity.py`，例如只比對改動相關的設定：`--only default app delete-and-retype`。
- `cli_parity.py` 的所有（設定, 輸入）組合共用一個最多 `--jobs`（預設 24）個程序的池：每 50 行一個程序，`--keep-memory` 的組合一個程序依序執行。
- 比對優先於主機上的其他工作：不加 `nice`。完整比對跑的時候，另一份 checkout 可以繼續跑快速比對與 `cargo` 的檢查。
- `--check-v3` 的向量是 Core ML 的輸出；ONNX Runtime 的 fp32 結果和 Core ML 本來就有差距，所以只有部分向量在 Core ML 的容許範圍內（目前 6/20，最大差距 0.083），比對的是兩邊的報告完全相同。Mac 上的 Core ML 路徑由 `EngineBench --check-v3` 檢查（20/20）。
- 移除 Swift 引擎前最後一次和它的完整比對記錄在 `parity/reports/swift-final/`。

### Golden 測試

`parity/golden/digests.tsv` 是 Swift 引擎在 macOS、真實資料上的結果摘要（詞庫、語言模型、容錯、按鍵解碼、數字格式、轉換、學習、組字器逐鍵的畫面）。Rust 的 golden 測試產生同樣的查詢，摘要必須相同，也就是和 Swift 版逐位元相同。只有在有意改變行為時才重新記錄（以 `ZHUYIN_GOLDEN` 指向新的摘要檔，比對差異後再取代）。摘要標記產生的系統（`#system`）：glibc 與 Apple 的 libm 在少數數學函式上差最後一位，所以 Linux 上會略過這些測試。

Golden 測試與 benchmark 使用真實資料，沒有時自動略過：

| 變數 | 內容 | 預設 |
|---|---|---|
| `ZHUYIN_DATA` | 含 `lexicon.bin`、`lexicon.tsv`、`lm/`、`lm_chat/` 的目錄（App 的 Resources 結構） | `data/build` |
| `ZHUYIN_PARITY_TEXT` | 語言模型比對用的句子，一行一句 | `data/build/lm/test_sentences.txt` |
| `ZHUYIN_TRIGRAM_LM` | 三連詞模型目錄，比對三連詞的分支 | 無 |
| `ZHUYIN_PARITY_DEV` | 口語、維基、聊天三個開發集，以 `:` 分隔，比對轉換與學習 | 無 |
| `ZHUYIN_GOLDEN` | 另一份摘要檔 | `parity/golden/digests.tsv` |

```sh
ZHUYIN_DATA=$PWD/../data/build/homophone2/mac-runtime \
ZHUYIN_TRIGRAM_LM=$PWD/../data/build/lm2 \
ZHUYIN_PARITY_DEV=$PWD/../data/build/corpus/dev_colloquial.txt:$PWD/../data/build/corpus/dev_wikipedia.txt:$PWD/../data/build/lm_chat/dev_chat_tt.txt \
cargo test --release -- --nocapture
```

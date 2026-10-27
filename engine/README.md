# ZhuyinType 引擎

所有平台共用的輸入法引擎。這個目錄是 Rust workspace；`swift/` 是目前 App 使用的 Swift 引擎，移植期間作為參考實作，切換（第 5 階段）後移除。Rust 版本供 Linux、Windows 的輸入法與 macOS App 共用。設計、模組對照、移植階段與驗收條件見 [DESIGN.md](DESIGN.md)。目前完成第 1–4 階段：資料層、引擎與詞圖解碼、組字器與 `zhuyin-cli`、神經模型（Rust 的判斷邏輯，ONNX Runtime 或主機的推論）、C ABI（`zhuyin-ffi`）與 macOS App 的建置開關。App 預設仍用 Swift 引擎，第 5 階段切換。

## 開發

在這個目錄（`engine/`）執行；工具鏈由 `rust-toolchain.toml` 固定。

```sh
cargo fmt --check                               # 格式
cargo clippy --all-targets -- -D warnings       # lint，不允許任何警告
cargo test                                      # 單元、fixture 與 golden 測試（golden 建議加 --release）
cargo bench                                     # criterion benchmark（需要下方的資料）
cargo deny check                                # 授權、重複版本、來源（cargo install --locked cargo-deny）
cargo build --release -p zhuyin-cli             # target/release/zhuyin-cli，旗標與 --batch 協定和 Swift 版相同
cargo build --release -p zhuyin-ffi             # target/release/libzhuyin.{a,dylib,so}：C ABI
```

- **ONNX Runtime**（Linux、Windows 上的模型）：`ORT_LIB_PATH=$ONNXRUNTIME_ROOT/lib cargo build --release -p zhuyin-cli --features onnx`。連結系統的 ONNX Runtime 1.24（`.cargo/config.toml` 要求動態連結），執行檔記錄該目錄為 rpath。`cargo test -p zhuyin-models --features onnx` 以 `data/build`（或 `ZHUYIN_MODELS`）的模型對 Core ML 的參考值檢查。
- **C 標頭**：改了 `zhuyin-ffi` 的介面後執行 `cbindgen --config crates/zhuyin-ffi/cbindgen.toml --crate zhuyin-ffi --output crates/zhuyin-ffi/include/zhuyin.h`（`cargo install --locked cbindgen`）。`cargo test -p zhuyin-ffi` 檢查標頭是否最新，並以 `cc` 編譯、執行 `tests/c/smoke.c`。
- **macOS App 用 Rust 引擎**：在 repository 根目錄 `ZHUYIN_ENGINE=rust make app`（建置目錄 `.build-rust`）；`tools/zt check app` 的自我測試兩種模式都適用。App 的引擎路徑可用 `EngineBench` 量測：`ZHUYIN_ENGINE=rust swift build --package-path platforms/macos --scratch-path .build-rust -c release --product EngineBench`，再 `ZT_CLI=.build-rust/release/EngineBench tools/zt eval latency --data dist/ZhuyinType.app/Contents/Resources`（Swift 模式用 `.build`）。`EngineBench --check-v3 data/build/v3` 經 Rust 與 Core ML callback 檢查 v3 的向量。

## 比對

Rust 版的輸出必須和 Swift 版逐字相同。比對在 GPU 主機上、從 repository 根目錄執行（需要 ONNX Runtime 與 Swift 工具鏈；`data/` 指向評測資料）：

```sh
engine/parity/gate.sh --quick     # 快速比對：開發時每次修改都跑，幾分鐘
engine/parity/gate.sh             # 完整比對：合併前、合併 main 之後各跑一次，0 差異才合併
```

`gate.sh` 先以 32 個編譯工作建置兩個 CLI，再同時執行：`cli_parity.py` 的兩組比對（不含神經模型的 21 組設定，以及 App 的設定 `--models data/build`，兩邊都以 ONNX Runtime 推論）、兩個 CLI 的 `--check-v3` 結果是否相同，完整比對再加上 golden 摘要（Swift 量測工具在主機的資料上重新產生，Rust 的 golden 測試對照）。結果寫在 `gate-out/`，全部通過時結束碼為 0。

- 快速比對每組輸入只取前 50 行；完整比對取全部（四個開發集各 400 段，加上善解的列）。其他參數直接交給 `cli_parity.py`，例如只比對改動相關的設定：`engine/parity/gate.sh --quick --only default app delete-and-retype`。
- `cli_parity.py` 的所有（設定, 輸入）組合共用一個最多 `--jobs`（預設 24）個程序的池：每 50 行一個程序，`--keep-memory` 的組合一個程序依序執行。
- 比對優先於主機上的其他工作：不加 `nice`。完整比對跑的時候，另一份 checkout（各自的 `target/`、`.build/`）可以繼續跑快速比對與 `cargo` 的檢查。
- `--check-v3` 的向量是 Core ML 的輸出；Linux 上兩個 CLI 都用 ONNX Runtime，所以都只有部分向量在 Core ML 的容許範圍內（目前 6/20，最大差距 0.083，ONNX fp32 與 Core ML 本來的差距），比對的是兩邊的結果完全相同。Mac 上的 Core ML 路徑由 `EngineBench --check-v3` 檢查（20/20）。

### Golden 測試

Golden 測試與 benchmark 使用真實資料，沒有時自動略過：

| 變數 | 內容 | 預設 |
|---|---|---|
| `ZHUYIN_DATA` | 含 `lexicon.bin`、`lexicon.tsv`、`lm/`、`lm_chat/` 的目錄（App 的 Resources 結構） | `data/build` |
| `ZHUYIN_PARITY_TEXT` | 語言模型比對用的句子，一行一句 | `data/build/lm/test_sentences.txt` |
| `ZHUYIN_TRIGRAM_LM` | 三連詞模型目錄，比對三連詞的分支 | 無 |
| `ZHUYIN_PARITY_DEV` | 口語、維基、聊天三個開發集，以 `:` 分隔，比對轉換與學習 | 無 |
| `ZHUYIN_GOLDEN` | 另一份摘要檔（`golden.sh` 寫入、測試讀取；`gate.sh` 用它在主機的資料上比對，不動提交的摘要） | `parity/golden/digests.tsv` |

```sh
ZHUYIN_DATA=$PWD/../data/build/homophone2/mac-runtime \
ZHUYIN_TRIGRAM_LM=$PWD/../data/build/lm2 \
ZHUYIN_PARITY_DEV=$PWD/../data/build/corpus/dev_colloquial.txt:$PWD/../data/build/corpus/dev_wikipedia.txt:$PWD/../data/build/lm_chat/dev_chat_tt.txt \
cargo test --release -- --nocapture
```

Golden 值由 Swift 引擎的原始碼產生：`ZHUYIN_PARITY_DEV=… parity/golden.sh DATA TEXT [TRIGRAM_LM]` 重新產生 `parity/golden/digests.tsv`，完整結果寫在 `parity/out/`（不提交，比對失敗時測試會指出第一個不同的行）。Swift 端的效能數字：`parity/.build/release/SwiftParity bench DATA TEXT`。

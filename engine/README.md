# ZhuyinType 引擎

所有平台共用的輸入法引擎。這個目錄是 Rust workspace；`swift/` 是目前 App 使用的 Swift 引擎，移植期間作為參考實作，切換（第 5 階段）後移除。Rust 版本供 Linux、Windows 的輸入法與 macOS App 共用。設計、模組對照、移植階段與驗收條件見 [DESIGN.md](DESIGN.md)。目前完成第 1 階段（資料層）、第 2 階段（引擎、個人模型、詞圖解碼）與第 3 階段（組字器、`zhuyin-cli`）；神經模型在第 4 階段加入（DESIGN.md §7 列出尚未支援的 CLI 旗標）。

## 開發

在這個目錄（`engine/`）執行；工具鏈由 `rust-toolchain.toml` 固定。

```sh
cargo fmt --check                               # 格式
cargo clippy --all-targets -- -D warnings       # lint，不允許任何警告
cargo test                                      # 單元、fixture 與 golden 測試（golden 建議加 --release）
cargo bench                                     # criterion benchmark（需要下方的資料）
cargo deny check                                # 授權、重複版本、來源（cargo install --locked cargo-deny）
cargo build --release -p zhuyin-cli             # target/release/zhuyin-cli，旗標與 --batch 協定和 Swift 版相同
```

兩個 CLI 的輸出比對（在 GPU 主機上、從 repository 根目錄執行）：`PYTHONPATH=ml/src python3 engine/parity/cli_parity.py --swift .build/release/zhuyin-cli --rust engine/target/release/zhuyin-cli --jobs 8 --output …`。

Golden 測試與 benchmark 使用真實資料，沒有時自動略過：

| 變數 | 內容 | 預設 |
|---|---|---|
| `ZHUYIN_DATA` | 含 `lexicon.bin`、`lexicon.tsv`、`lm/`、`lm_chat/` 的目錄（App 的 Resources 結構） | `data/build` |
| `ZHUYIN_PARITY_TEXT` | 語言模型比對用的句子，一行一句 | `data/build/lm/test_sentences.txt` |
| `ZHUYIN_TRIGRAM_LM` | 三連詞模型目錄，比對三連詞的分支 | 無 |
| `ZHUYIN_PARITY_DEV` | 口語、維基、聊天三個開發集，以 `:` 分隔，比對轉換與學習 | 無 |

```sh
ZHUYIN_DATA=$PWD/../data/build/homophone2/mac-runtime \
ZHUYIN_TRIGRAM_LM=$PWD/../data/build/lm2 \
ZHUYIN_PARITY_DEV=$PWD/../data/build/corpus/dev_colloquial.txt:$PWD/../data/build/corpus/dev_wikipedia.txt:$PWD/../data/build/lm_chat/dev_chat_tt.txt \
cargo test --release -- --nocapture
```

Golden 值由 Swift 引擎的原始碼產生：`ZHUYIN_PARITY_DEV=… parity/golden.sh DATA TEXT [TRIGRAM_LM]` 重新產生 `parity/golden/digests.tsv`，完整結果寫在 `parity/out/`（不提交，比對失敗時測試會指出第一個不同的行）。Swift 端的效能數字：`parity/.build/release/SwiftParity bench DATA TEXT`。

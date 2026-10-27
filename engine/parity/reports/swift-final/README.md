# 和 Swift 引擎的最後一次完整比對

移除 Swift 引擎（第 5 階段）之前，在 GPU 主機上以 `engine/parity/gate.sh` 的完整比對（當時比對 Swift CLI 與 Rust CLI）跑的最後一次結果。程式是 rust-engine 分支的 ba5078b（合併 main 的 e0f1e4b 之後，`exact_margin` 8），2026-10-28。

| 項目 | 結果 | 檔案 |
|---|---|---|
| CLI 比對：不含神經模型的 21 組設定 × 9 組輸入（189 組），加上 App 的設定（`--models data/build`，ONNX Runtime）5 組 × 9 組輸入（45 組） | 57,772 行，0 行不同 | `cli-parity.json`、`cli-parity.log` |
| `--check-v3`（Core ML 的參考向量，兩邊都以 ONNX Runtime 推論） | 兩邊的報告完全相同：6/20、最大差距 0.0828 | `check-v3-swift.log`、`check-v3-rust.log` |
| golden 摘要：Swift 量測工具在主機（Linux）的資料上產生，Rust 的 golden 測試對照 | 全部相同 | `golden-digests-linux.tsv` |

`cli-parity.json` 裡的 `swift`、`rust` 欄位是當時的名稱；現在的 `cli_parity.py` 比對兩個 Rust 建置，欄位是 `reference`、`candidate`。

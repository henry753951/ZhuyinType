# ml/src/zhuyintype_ml/eval/systems

把 ZhuyinType 和其他公開的注音輸入法放在同一個介面後面，讓每個系統收到同一批按鍵、在每個評測集與情境上量測。

## 用途

和其他輸入法比較時，數字只在條件相同時才有意義：同樣的句子、同樣的按鍵、同樣的正規化、每句重設學習。這個模組把每個輸入法包成一個 `System`（策略模式），比較程式（`zt bench compare`，`eval/bench/compare.py`）對所有系統做同一件事，結果寫成一份紀錄檔，[docs/benchmarks/competition.md](../../../../../docs/benchmarks/competition.md) 由它產生。

每個轉接器只執行對方未修改的程式，透過對方自己文件記載的介面，從介面重新撰寫，沒有複製或 import 對方的程式碼：

| 系統 | 設定 | 介面 |
|---|---|---|
| ZhuyinType | `app`（App 的設定）、`engine`（只有詞庫與語言模型） | `zhuyin-cli --batch`（`EnginePool`） |
| 靈犀注音（0c，AGPL-3.0） | `statistical`、`neural`（D512 選字模型，預設 100 ms 時限） | `0c-server` 的 socket 協定：4 位元組小端序長度加 JSON，逐鍵送出 |
| 善解（shanjie，Apache-2.0） | `chat`、`formal` | 讀音：評測程式 `shanjie-eval`；按鍵：它的 App 使用的 C 介面（`core/include/shanjie.h`），原始碼不修改，建置成共享程式庫 |

## 指令

```sh
tools/zt --remote systems setup                     # 依固定的 commit 取得並建置 0c 與善解（GPU 主機）
tools/zt systems status
tools/zt --remote --export --detach bench compare   # 全部系統 × 全部集合，可中斷後接續
tools/zt --remote --export bench compare --tasks '^plain/smoke$' --systems zhuyintype-app 0c-statistical --no-page
tools/zt remote pull docs/benchmarks/competition.json
tools/zt bench compare --render                     # 由紀錄檔產生 competition.md（不跑評測）
```

`zai-test`、`zai-dev` 只在本機，執行前加 `--push data/build/homophone2/test_zai.txt --push data/build/homophone2/dev_zai.txt`。

主機上的位置來自環境：`ZT_SYSTEMS_DIR`（預設 `data/build/systems`）、`ZT_GO`（現成的 Go）、`ZT_BUILD_JOBS`。不在追蹤的檔案中寫主機或路徑。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build`）。

- `systems/<名稱>/src/`：固定 commit 的 checkout，`setup.json` 記錄 commit、工具鏈與每個二進位檔、模型的 SHA-256。
- `systems/shanjie/rows/`：善解 `eval/` 底下每個評測集轉成的 `context|truth|syllables` 檔，`manifest.json` 記錄來源、雜湊與內容相同的別名。沒有讀音的集合由善解自己的讀音工具（`reference/proto/ime.py`，在獨立的程序中執行）補上。
- `bench/compare/<label>/cells/<系統>/<task>.json`：每格的輸出，做完立刻存檔；`cache/<系統>.jsonl`：同一系統相同輸入只轉換一次；`latency/`、`systems/`。
- `bench/compare/<label>/record.json`：完整的紀錄，含每個 task 的參考答案與每個系統每一句的輸出（不放進版本庫）。
- `docs/benchmarks/competition.json`（追蹤）：只有產生頁面所需的數字：S 與 V 兩種正規化的整句全對率、句數、對 ZhuyinType（App 設定）的成對 McNemar 檢定、延遲摘要、各系統的版本與設定。`zt bench compare --render` 由它逐位元組重現 `competition.md`。

情境：`plain`（沒有前文）、`rows`（善解的集合，照原樣帶前文）、`context`（同一行前面的文字）、`split`（分兩次送出）、`typo/0.1`（每音節 10% 打錯）、`typing/<情境>`（`zt eval typing-errors` 的 15 種情境）。不接受任意前文的系統在 `context` 是 N/A，在 `rows` 收到沒有前文的同一列，格子記下筆數。

## 檔案

| 檔案 | 說明 |
|---|---|
| `base.py` | `System` 介面、`Task`／`Case`／`Segment`／`Outcome`、按鍵腳本的解析、每鍵時間的統計 |
| `zhuyintype.py` | ZhuyinType：`zhuyin-cli` 批次模式；延遲用 `--timing` |
| `zeroc.py` | 靈犀注音：協定的封包、`0c-server` 程序與 session、逐鍵輸入；分段送出時每句重新啟動 |
| `shanjie.py` | 善解：`shanjie-eval` 的列檔、dump、選字與摘要的解析（找不到解碼路徑的列記為 no-path）；C 介面的 ctypes 綁定與逐鍵輸入 |
| `references.py` | 兩個專案的固定 commit 與 `zt systems setup`：clone、下載發布的資料與模型（核對 SHA-256）、建置、轉換善解的評測集 |
| `test_systems.py` | 以假程式測試協定封包、session、列檔與 dump 的解析、找不到解碼路徑時的重跑 |

比較程式本身在 `../bench/compare.py`（tasks、矩陣、紀錄）與 `../bench/competition.py`（頁面），測試在 `../bench/test_compare.py`。

## 相關文件

- [docs/benchmarks/competition.md](../../../../../docs/benchmarks/competition.md)：比較的結果、條件與限制
- [docs/research/related-work.md](../../../../../docs/research/related-work.md)：兩個專案的做法與我們學到的
- [docs/product/WRITING.md](../../../../../docs/product/WRITING.md)：提到其他專案的寫法

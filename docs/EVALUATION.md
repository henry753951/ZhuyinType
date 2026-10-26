# 評測

所有評測都用 `zhuyin-cli` 逐鍵打字，經過和 app 相同的 Composer、引擎與模型，比對送出的結果和原文。數字的意義與目前的成績見 [MODEL.md](MODEL.md)。

## 1. 原則

- **開發集調參數，測試集只在決定採用時跑一次。** 兩邊都不能出現在訓練資料裡：所有訓練語料都排除和評測集有連續 8 字相同的句子。
- 主要指標是**整句全對率**：一段組字（4–20 個中文字）的結果和原文完全相同。另外報字準確率。
- 「台／臺」「妳／你」「裏／裡」這類兩種寫法都對的字，另外報一個「視為相同」的整句全對率（`conversion_eval.py` 的 `counting variants as equal`）。
- 打字的讀音由詞庫最長比對推得，每個詞用最常見的讀音、一聲按空白，不是人工標注。
- 引擎正在修改時，評測前先建一份固定的 CLI（例如 `.build-dev`），整組評測用同一份。
- 每個 `zhuyin-cli` 只用一個 CPU 核心，而且批次輸入的每一行互相獨立；`conversion_eval.py --jobs N` 把一組評測分給 N 個程序平行跑，結果和單一程序相同（每個程序約 400 MB 記憶體）。
- **評測與訓練都在遠端 GPU 主機（64 核心、H100）上跑**，不佔用開發用的 Mac：引擎在 Linux 上編譯，模型以 ONNX Runtime 執行（`training/remote_eval/`）。Mac 只負責建置 app、Core ML 匯出與編譯，以及確認遠端結果和 Mac 一致（輸出不同的行數不超過 1%）。

## 2. 資料集

| 檔案 | 內容 | 用途 |
|---|---|---|
| `data/build/corpus/dev_colloquial.txt` | 台灣口語句子 2,000 句 | 開發 |
| `data/build/corpus/dev_wikipedia.txt` | 維基百科 2,000 句，同一行前面的文字可作為前文 | 開發 |
| `data/build/lm_chat/dev_chat_tt.txt` | Taiwan Tongues zh-TW 轉寫 1,500 句（朗讀句子，偏日常） | 開發 |
| `data/build/lm/test_sentences.txt` | 維基百科測試句 | 測試 |
| `data/build/corpus/test_colloquial.txt` | 台灣口語測試句 | 測試 |
| `data/build/lm_chat/test_chat_tt.txt` | Taiwan Tongues 測試 1,500 句 | 測試 |
| `training/eval/colloquial_smoke.txt` | 日常短句 | 測試 |
| `data/build/homophone/{dev,test}_zai.txt` | 「在／再」等同音字（建置中，見 [ROADMAP.md](ROADMAP.md)） | 開發／測試 |

`data/build/` 不在 git 裡；產生方式見 `training/corpus/README.md`、`training/lm_chat/README.md`。

## 3. 工具

| 工具 | 量什麼 |
|---|---|
| `training/eval/conversion_eval.py` | 整句全對率：打對的整句；`--line-context` 以同一行前面的文字為前文；`--split continue` 從中間切成兩次送出；`--typo`、`--fuzzy` |
| `training/eval/error_analysis.py` | 依打字習慣（標準、一不變調、輕聲打一聲、一聲不按空白、5% 按錯）分組，報「顯示正確」與「前 16 句命中」 |
| `training/eval/typing_errors.py` | 有按鍵時間的打字錯誤：對調、幾乎同時按下、熱區誤觸、多按、漏按、一聲省略空白、混合；見 `training/eval/TYPING_ERRORS.md` |
| `training/eval/typo_eval.py` | 依比例隨機按錯（10%、20%）的整句全對率 |
| `zhuyin-cli --timing` | 每鍵延遲（p50、p95、最大）、CPU 時間、能耗（macOS）、每鍵指令數、記憶體占用；批次模式把摘要印在標準錯誤 |
| `zhuyin-cli --check-v3 DIR` | v3 Core ML 模型和訓練端輸出是否一致 |
| `make checks` | 核心行為與延遲檢查（3,000 多項） |

## 4. 常用指令

```sh
# 建一份固定的 CLI
swift build -c release --scratch-path .build-dev --product zhuyin-cli

# 目前 app 的設定：整句重排 + v3 + 聊天模型
python3 training/eval/conversion_eval.py --cli .build-dev/release/zhuyin-cli -n 400 --jobs 8 \
    --reranker data/build/reranker --rerank-count 16 \
    --cli-arg=--v3 --cli-arg=data/build/v3 \
    --cli-arg=--chat-lm --cli-arg=data/build/lm_chat \
    --test data/build/corpus/dev_colloquial.txt data/build/lm_chat/dev_chat_tt.txt

# 有前文
python3 training/eval/conversion_eval.py --cli .build-dev/release/zhuyin-cli -n 400 --line-context \
    --reranker data/build/reranker --rerank-count 16 --test data/build/corpus/dev_wikipedia.txt

# 打字習慣
python3 training/eval/error_analysis.py --cli .build-dev/release/zhuyin-cli --reranker data/build/reranker \
    --test data/build/corpus/dev_colloquial.txt data/build/corpus/dev_wikipedia.txt -n 400
```

`zhuyin-cli` 調參數用的選項：

| 選項 | 意思 |
|---|---|
| `--reranker DIR`、`--rerank-weight W`、`--rerank-count N`、`--exact-margin M` | 整句重排模型、權重、句數、打對優先的差距 |
| `--v3 DIR`、`--v3-weight W` | 音節對字模型與它在詞圖的權重 |
| `--chat-lm DIR`、`--chat-prior P`、`--domain-scale K`、`--domain-centre D` | 聊天模型、沒有前文時的聊天比例、前文影響的強度與中心 |
| `--end-share S` | 句尾比例 |
| `--decoder slots\|boundaries\|reorder\|timed`、`--decoder-costs …`、`--hotspot` | 按鍵解碼方法、成本、熱點成本 |
| `--typo off\|standard\|aggressive`、`--fuzzy zh,ch,…` | 容錯、模糊音 |
| `--context TEXT` | 游標前的文字 |
| `--timing` | 印出每次模型呼叫的時間，最後加上延遲、CPU、能耗、記憶體摘要 |

批次輸入每行一段按鍵；`<dt:MS>` 指定和前一鍵的間隔，`<enter>`、`<left>`、`<sdown>`（⇧↓）、`<sup>`（⇧↑）等表示特殊鍵；「文字<tab>按鍵」把前面的文字當作前文。

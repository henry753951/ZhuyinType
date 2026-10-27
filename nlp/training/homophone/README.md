# homophone：同音字評測集與 0.4.0 的 v3

建立在／再與另外 26 組常見同音字的開發、測試集，並從 0.3.0 的 v3 繼續訓練出 0.4.0 起 app 使用的 v3。

## 用途

- 評測集：從維基百科與 Taiwan Tongues zh-TW 的句子挖出含同音字的完整子句，依位置分成詞內、主詞後、句首、動詞前、地點時間前與其他。在／再的開發、測試集各 1,000 句，其他組各約 100 句，另有 18 句日常短句（smoke）。這些集合也登記在 `nlp/common/sets.py`（`zai-dev`、`zai-test`）。
- 0.4.0 的 v3：從 `v3_030/` 的音節對字編碼器繼續訓練，前文編碼器不動。30% 的批次集中在混淆字，混淆位置的損失乘 3；四分之一的樣本取自真實口語（立法院公報、維基百科討論頁、Common Voice），不使用 Taiwan Tongues。選定第 500 步。結果寫成 `v3_homophone2/`，發布時以 `v3/` 的名稱打包（`nlp/data/assets.py`）。
- 走到發布的這條路徑就是 `zt train homophone` 的步驟。試過但沒有採用的做法（以 Taiwan Tongues 訓練的第一次實驗、針對性重試、參數中點）只留在 REPORT.md。

## 指令

```sh
zt --remote train homophone                    # GPU 主機：test 到 evaluate；judge 需要先啟動 Qwen3-8B 伺服器
zt --remote train homophone evaluate-test      # 寫好 homophone2/dev-decision.json 之後，只跑一次
zt train homophone export check                # Mac：寫好 homophone2/choice2.json（選定檢查點的 SHA256）之後
zt --remote train homophone parity             # GPU 主機：nlp/remote 匯出 ONNX、取回 Mac 的 test_vectors.json 之後
zt --remote train homophone judge --option judge=http://127.0.0.1:8102/v1/chat/completions
zt --remote train homophone prepare --option workers=4
python -m nlp.training.homophone.evaluate sets --set data/build/homophone2/dev_zai.txt --context line
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試 | 任一 |
| `snapshot` | 把詞庫、六個保留集與 Taiwan Tongues `train.tsv` 複製到 `homophone/`（已存在就保留） | 任一 |
| `inventory` | 在整個語料上計算 27 組種子同音字的次數，保留 26 組 | GPU 主機 |
| `mine` | 抽樣候選子句：維基百科每 20 行取 1 行，加上 Taiwan Tongues | GPU 主機 |
| `judge` | 遮住每個在／再，由 Qwen3-8B 獨立填字，不一致或不確定就排除 | GPU 主機（需要判斷伺服器） |
| `split` | 開發、測試集與 smoke；`review100.jsonl` 供訓練前人工檢查 | 任一 |
| `prepare` | 篩選 `v3_030` 分片的索引、收集真實口語子句、獨立重掃 | GPU 主機 |
| `train` | 繼續訓練到開發集不再進步 | GPU 主機（CUDA） |
| `evaluate` | 原本的 v3 與繼續訓練版本：同音字開發集、一般開發集、善解列 | GPU 主機 |
| `evaluate-test` | 同音字測試集，只在開發集決定之後跑一次 | GPU 主機 |
| `export` | Core ML、編譯、20 組介面向量、來源聲明 | 只限 macOS |
| `check` | `zhuyin-cli --check-v3` 比對 Swift 與向量 | 只限 macOS |
| `parity` | ONNX 與 Mac 的 Core ML 輸出各自和 PyTorch 比對 | GPU 主機 |

兩個人工決定把流程分成三段：看完開發集結果，寫 `homophone2/dev-decision.json` 才能跑測試集；決定採用哪個檢查點，寫 `homophone2/choice2.json`（`{"checkpoint": SHA256}`）才能匯出。`evaluate` 用 app 的設定（`zhuyin-cli --models`），只把整句權重固定為 0.4.0 比較時用的 0.5，並以 `--v3` 換成要比較的模型；報告是 `Report` JSON，可以用 `zt eval stats` 成對比較。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `lexicon.tsv`；保留集 `lm/test_sentences.txt`、`corpus/{test,dev}_colloquial.txt`、`corpus/dev_wikipedia.txt`、`lm_chat/{dev,test}_chat_tt.txt`；`../raw/taiwan-tongues/train.tsv`；
- `v3_030/`：起點檢查點、詞彙表、`wiki.txt`、`colloquial.txt`、編碼後的分片、`test_vectors.json`、`articles.tsv.gz`、`example.json`；
- `lm_chat2/real_dialogues.jsonl`、`splits.json`、`source-manifest.json`、`attribution.jsonl`、`dev_dialogue.txt`、`test_dialogue.txt`；`corpus/train_commonvoice.txt`；`corpus/raw/Qwen3-8B-LICENSE`；
- 版本庫的 `nlp/eval/**/*.txt`（含匯入的善解資料列）：全部當作保留集排除。

寫入：

- `homophone/`：`inputs/`、`lexicon.tsv`、`inventory.json`、`candidates.jsonl`、`mining.json`、`judged.jsonl`、`judge-summary.json`、`{dev,test}_{zai,pairs}.{txt,jsonl}`（每行 `前文<TAB>子句`）、`smoke.txt`、`review100.jsonl`、`splits.json`；
- `homophone2/`：上述集合的副本、`inputs.json`（所有來源與保留集的雜湊）、`filtered/`、`filtered.json`、`real-train.jsonl`、`real-train.json`、`leakage-audit.json`、`v3-baseline.json`、`v3-training.json`、`v3-complete.json`、`reports/`、`check-v3.log`、`onnx-chosen-audit.json`、`coreml-chosen-audit.json`；舊的 `results/` 是 0.4.0 決定時的原始結果，保留不動；
- `v3_homophone2/`：`syllable.pt`、`context.pt`、詞彙表、`config.json`、Core ML 套件與編譯結果、`test_vectors.json`、`lexicon.tsv`、`NOTICE.md`、`attribution.jsonl`、`source-manifest.json`、`articles.tsv.gz`、`Licenses/`。

授權：模型與衍生語料為 CC BY-SA 4.0，另含 OGDL v1（立法院公報，經 OpenFun LYAPI，CC BY 4.0）與 CC0 1.0（Common Voice）的來源；程式碼為 AGPL-3.0-only。再散布時保留 `NOTICE.md` 與所列的出處檔。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train homophone` 的步驟 |
| `common.py` | 目錄、保留集排除、混淆位置與類別 |
| `mine.py` | 種子組計數與候選子句抽樣 |
| `judge.py` | Qwen3-8B 遮字判斷 |
| `split.py` | 開發、測試集切分 |
| `filter.py` | 分片的整份文件排除（`compatible.py` 的 worker） |
| `compatible.py` | 繼續訓練的資料：索引、真實口語子句、洩漏稽核 |
| `train.py` | 繼續訓練與開發集選擇 |
| `evaluate.py` | 混淆位置評測（`Evaluator`）與開發、測試集比較 |
| `export.py` | Mac 上的 Core ML 匯出、介面向量與來源聲明 |
| `check_onnx.py` | ONNX 或已存的 Core ML 輸出和 PyTorch 的一致性 |
| `test_homophone.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：0.4.0 採用的繼續訓練、被否決的嘗試與第一次實驗的完整數字
- [../v3/README.md](../v3/README.md)：起點的 0.3.0 v3
- [docs/MODEL.md](../../../docs/MODEL.md) §6、[docs/EVALUATION.md](../../../docs/EVALUATION.md)、[docs/research/log.md](../../../docs/research/log.md)
- [../README.md](../README.md)：所有訓練流程

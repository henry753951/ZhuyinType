# 台灣華語口語語料

台灣口語與維基百科的開發集與測試集，以及 Qwen3-8B 產生的口語句子。

## 用途

- 在產生任何語料之前固定評測集：Common Voice zh-TW 的口語開發集與測試集（`corpus/dev_colloquial.txt` 2,000 句、`corpus/test_colloquial.txt` 3,000 句，評測集 `colloquial-dev`、`colloquial-test`）、維基百科開發集（`corpus/dev_wikipedia.txt` 2,000 句，`wiki-dev`），以及其餘的 Common Voice 訓練句 `train_commonvoice.txt`。
- 以 Qwen3-8B 產生原創的台灣口語訊息，嚴格過濾與去重後得到 600 萬句（前 150 萬句為 `synthetic_phase1.txt`）。0.3.0 的 v3 口語語料（`v3_030/colloquial.txt`）與 0.3.0 聊天模型由它建成。
- 來源、規則與生成設定見 [REPORT.md](REPORT.md)。

## 指令

```sh
zt --remote train corpus splits        # 需要維基百科的訓練分片（zt train lm train）
zt --remote train corpus generate      # GPU 主機：Qwen3-8B 伺服器、生成與清理，到 600 萬句為止
cat data/build/corpus/clean-progress.json
python -m unittest discover -s ml/src/zhuyintype_ml/training/corpus -t .
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：清理規則、評測集排除、去重、提示網格 | 任一 |
| `splits` | 下載 Common Voice、固定口語與維基百科評測集；`splits.json` 已存在就不再執行 | 任一 |
| `generate` | 啟動 vLLM 伺服器（記錄在 `corpus/vllm.log`），同時執行生成與清理，結束後停止伺服器 | GPU 主機（GPU 約 0.92 的記憶體） |

要提早停止 `generate`：建立 `data/build/corpus/STOP_GENERATION`，生成程式結束後再建立 `STOP_CLEANER`；移除兩個檔案後再執行同一個步驟即可續跑。檔案只會附加，重播安全。

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- Common Voice 的 `sentence-collector.txt`（下載時的最新 commit，CC0）；
- `lm/work/sentences/`：維基百科模型的訓練分片（已排除維基百科測試集）；
- `lm/test_sentences.txt`：清理時排除；
- `../raw/Qwen3-8B/`：固定版本的 Qwen3-8B 檢查點（revision `b968826d9c46dd6066d109eabc6255188de91218`，Apache-2.0）。

寫入 `corpus/`：

- 評測集與訓練句：`test_colloquial.txt`、`dev_colloquial.txt`、`dev_wikipedia.txt`、`train_commonvoice.txt`、`splits.json`（來源網址、commit、雜湊、授權證明）；
- 原始資料：`raw/sentence-collector.txt`、`raw/common-voice-submission-policy.md`、`raw/synthetic.jsonl`（每個請求的原始回應）、`raw/Qwen3-8B-LICENSE`；
- 產生的語料：`synthetic_phase1.txt`、`PHASE1_READY`、`synthetic_background.txt`、`generation-config.json`、`clean-checkpoint.json`、`clean-progress.json`。

授權：程式碼為 AGPL-3.0-only；Common Voice 文字為 CC0-1.0；生成器權重不重新散布。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train corpus` 的步驟 |
| `prepare.py` | 評測集與 Common Voice 訓練句；`exclusion` 供清理使用 |
| `serve.py` | 以 vLLM 在本機回送位址提供 Qwen3-8B |
| `generate.py` | 主題 × 語域 × 人物 × 長度網格的生成請求 |
| `clean.py` | 繁體與台灣用詞過濾、評測集排除、完全與近似去重 |
| `requirements.lock` | 產生語料時的套件版本紀錄 |
| `test_corpus.py` | 單元測試 |

## 相關文件

- [REPORT.md](REPORT.md)：來源、規則、生成設定與沒有採用的做法
- [docs/ml/EVALUATION.md](../../../../../docs/ml/EVALUATION.md)：評測集
- [../v3/README.md](../v3/README.md)：使用這份語料的 v3

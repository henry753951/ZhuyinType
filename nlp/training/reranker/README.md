# reranker：整句重排的字級語言模型

訓練 v2b 字級 Transformer 語言模型，並匯出成整句重排與前綴樹評分兩種 Core ML 模型。

## 用途

- 引擎先列出最可能的幾個整句，再以這個模型依前文為每個整句評分，兩個分數合起來決定顯示哪一句；候選字清單也用它排序。
- 出貨的是 v2b（0.3.0 起）：8 層、寬度 512、8 個 head、FFN 2048，上下文 64 字，詞彙 12,000 字，訓練 90,000 步。app 的整句權重 1.0、重排 16 句，寫在 `config.json`。
- 0.4.0 起，同一份權重另外匯出成前綴樹評分模型（`RerankerTree`）：16 句排成一棵樹，相同前綴只算一次。
- `train.py` 的 `CharLM`、`Config` 與 `export_tree.py` 的 `TreeScorer` 也由 `nlp/remote`（ONNX 匯出、模型伺服器）與 ZhuyinBench 匯入。Linux 用的 ONNX 檔由 `nlp/remote` 的匯出程式產生。

## 指令

```sh
zt --remote train reranker                     # GPU 主機：test 到 tree；到 compile 會停止（compile 只能在 macOS 執行）
zt train reranker export tree compile          # Mac：取回 reranker/charlm.pt 之後
zt --remote train reranker --option workers=60 # prepare 的程序數（預設 32）
python -m nlp.training.reranker.train --data data/build/reranker_v2c --steps 90000 \
    --layers 10 --width 640 --heads 10 --ffn 2560          # 其他大小（REPORT_v2.md 的 v2c）
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：保留集過濾、前綴樹與批次模型一致 | 任一 |
| `prepare` | 詞彙表與 token 串流，排除和開發、測試集有 8 字重疊的句子 | GPU 主機 |
| `train` | v2b，90,000 步，每 5,000 步存檔並量測開發集困惑度 | GPU 主機（CUDA） |
| `export` | `Reranker.mlpackage`（fp16、macOS 14）與 `config.json` | 任一（需要 coremltools） |
| `tree` | `RerankerTree.mlpackage`：深度 RoPE、只看祖先的注意力；匯出前先和批次模型比對 | 任一（需要 coremltools） |
| `compile` | 以 `xcrun coremlcompiler` 編譯兩個套件 | 只限 macOS |

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：

- `lm/work/unique.txt`：`zt train lm` 去重後的維基百科句子（訓練當時在主機的 `work/unique.txt`；可用 `--wiki` 指定）；
- `corpus/synthetic_phase1.txt`、`corpus/synthetic_background.txt`、`corpus/train_commonvoice.txt`：口語句，重複兩次；
- 保留集 `lm/test_sentences.txt`、`corpus/test_colloquial.txt`、`corpus/dev_wikipedia.txt`、`corpus/dev_colloquial.txt`：只用來排除；兩個開發集另外編成驗證串流。

寫入 `reranker/`：

- `vocab.json`（0 = `<pad>`、1 = `<bos>`、2 = `<unk>`）、`train.bin`、`val_wiki.bin`、`val_colloquial.bin`、`prepare.json`；
- `charlm.pt`、`train-log.json`；
- `Reranker.mlpackage`／`.mlmodelc`：輸入 `ids`、`targets` `[B, L]`，輸出每個字的 `logprob` `[B, L]`；
- `RerankerTree.mlpackage`／`.mlmodelc`：輸入一棵打包的前綴樹（`ids`、`positions` `[1, N]`，`ancestors` `[N, N]`，`parents`、`targets` `[E]`），輸出每個目標在父節點上的 `logprob` `[E]`；
- `config.json`：`{"weight": 1.0, "count": 16}`。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train reranker` 的步驟 |
| `prepare.py` | 詞彙表與 token 串流 |
| `train.py` | 字級語言模型（`CharLM`、`Config`）與訓練 |
| `export.py` | 整句評分的 Core ML 匯出 |
| `export_tree.py` | 前綴樹評分的 Core ML／ONNX 匯出（`TreeScorer`） |
| `test_reranker.py` | 單元測試 |

## 相關文件

- [REPORT_v2.md](REPORT_v2.md)：v1、v2a、v2b、v2c 的比較與採用 v2b 的決定
- [REPORT_tree.md](REPORT_tree.md)：前綴樹評分的實作、一致性與延遲
- [docs/MODEL.md](../../../docs/MODEL.md)：整句重排在引擎中的用法
- [../README.md](../README.md)：所有訓練流程

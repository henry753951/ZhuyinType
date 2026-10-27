# 聊天詞 bigram

在既有的 Linux 主機上，從 `~/zhuyintype` 執行，使用既有的 uv 環境。不需要安裝任何東西，
也不需要更動原本的流程。

```sh
PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \
  .venv/bin/python nlp/training/lm_chat/test_train.py
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \
  .venv/bin/python -u nlp/training/lm_chat/train.py --workers 48 \
  > logs/lm-chat-train-20261027.log 2>&1 < /dev/null &
```

固定的輸入放在 `data/build/lm_chat/inputs/`：目前 Mac 上的 `lexicon.tsv`、Taiwan Tongues 的
`train.tsv`／`test.tsv`、`train_commonvoice.txt`、`train.py` 中列出的四個既有 dev／test 檔、`wiki/`
底下的維基百科 TSV、兩份發布者聲明、Qwen 的 LICENSE，以及來源與傳輸收據。`NOTICE.md` 記錄來源條款。
`transfer_receipt.json` 記錄原本在 Mac 上的輸入路徑與雜湊。
遠端既有的 `data/build/v3/colloquial.txt` 直接讀取並計算雜湊。

holdout 放在 `data/build/lm_chat/`，遵守本任務嚴格的寫入範圍；不會在 `data/build/corpus/` 中放入任何
檔案。`splits.json` 記錄 seed、過濾條件、選取規則與雜湊。holdout 在訓練前排除；只對 dev 分割計分。

`work/` 包含可續跑的句子分片、斷詞分數與詞彙表、計數，以及完整的輸入與程式碼設定。設定不符時會中止；
變更輸入或演算法之前，請保留舊的執行結果，並使用新的輸出與工作目錄。已完成且未變更的執行可以再次
呼叫以進行驗證。

交付內容：三個自然對數 TSV（合計 <=60,000,000 位元組）、聲明、兩個新的 holdout、`stats.json`、
`splits.json`、`validation.json`、逐行的 `line_scores.tsv`，以及 `sentence_starts.json`。`REPORT.md`
是給人閱讀的結果。perplexity 以兩個序列化後的模型在完全相同的最終聊天 E-step 詞 token 上計算，
包括 EOS、不含 BOS。逐行的原始 delta 是序列的對數概似比；要解讀為後驗機率，仍需要領域先驗，並在
具代表性的前文上校準。

來源 TSV 只以 `csv.DictReader` 讀取；音訊路徑與參與者中繼資料一律忽略。既有的來源、測試與 Swift
檔案永遠不會被改寫。

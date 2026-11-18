# 整句重排模型（reranker）的來源

此模型（`RerankerTree.mlmodelc`、`vocab.json`、`config.json`）是 ZhuyinType 訓練的字級語言模型 v2b：8 層 Transformer，上下文 64 字，詞彙 12,000 字。0.3.0 起隨 App 提供（data-1），0.4.4 起改為同一份權重的 int8 壓縮（data-5），並匯出成前綴樹評分的 Core ML 模型。0.7.2 起的權重改以下面的資料重新訓練（data-11），前綴樹以 fp32 計算。模型以 CC BY-SA 4.0 授權提供：https://creativecommons.org/licenses/by-sa/4.0/

訓練資料：

- 中文維基百科貢獻者：百科語言模型（`lm/`）同一流程去重後的條目句子，CC BY-SA 4.0。來源快照、處理方式與每篇條目的頁面與修訂編號見 `lm/NOTICE.md` 與 `lm/articles.tsv.gz`。依 Wikimedia 的再利用條款，以這些連結署名：https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content
- Common Voice 句子收集器貢獻者：zh-TW 句子，CC0 1.0，https://commonvoice.mozilla.org/
- Qwen3-8B 產生的台灣口語句子：生成器權重依 Apache-2.0 授權（全文見 `v3/Licenses/Qwen3-8B-LICENSE`），對產生的文字沒有額外限制；權重不隨此模型提供。
- 0.7.2 起另外加入聊天語言模型（`lm_chat/`）的訓練句，重複四次，約占訓練文字的 11%：立法院公報、維基百科討論頁、Common Voice，以及 PTT 八卦版的標題與推文（zake7749/Gossiping-Chinese-Corpus 2.0，整理者標示 Apache-2.0，文字屬各 PTT 作者）、海大 TOCP v1.0（CC BY-SA 4.0）、篩選過的 zh-meme-sft-8k、LCCC-base 與 MNBVC 知乎子集（轉成正體後只留像台灣用法的句子），以及 Claude Haiku 5.5 依主題產生的台灣口語句子（依 Anthropic 的使用條款；不含使用者的文字）。每份來源的網址、版本、SHA-256 與授權見 `lm_chat/NOTICE.md` 與 `lm_chat/source-manifest.json`。

修改：句子去重，排除與開發集、測試集有 8 字重疊的句子；口語句重複兩次、聊天句重複四次；編碼為字元序列後訓練；權重以 int8（每列一個 scale）壓縮後匯出。訓練程式見 ZhuyinType 原始碼的 `ml/src/zhuyintype_ml/training/reranker/`。

再散布此模型時，請保留本聲明、`lm/NOTICE.md`、`lm/articles.tsv.gz`、`lm_chat/NOTICE.md` 與 `lm_chat/source-manifest.json`。各來源機構與平臺未背書此模型，原文不隨此模型散布。

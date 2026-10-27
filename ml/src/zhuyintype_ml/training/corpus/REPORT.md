# 台灣華語口語語料：做法與紀錄

評測集與產生語料的來源、規則與生成設定。指令與檔案見 [README.md](README.md)。

## 來源與評測集

程式碼：AGPL-3.0-only。Common Voice 輸入文字：CC0-1.0。混合維基百科的模型與詞庫：CC BY-SA 4.0。
原始的生成器權重不重新散布。

確切的來源 URL、repository revision、SHA256、投稿授權政策與分割大小記錄在
`data/build/corpus/splits.json`。只使用 Common Voice 的 **sentence-collector.txt**，排除另外取得的
chatlogs／government 檔案。這些是捐贈的**朗讀語音提示句**，可能包含正式或不常見的句子；它們不是經過
驗證的自然聊天測試集。舊的 5,000 行維基百科測試集維持不變。dev 集由前一版維基百科訓練分片中以確定性
reservoir 抽出的 2,000 行組成。

`prepare.py` 在**生成之前**執行。它做 NFC 正規化，保留 4–60 個字元的繁體行，去重，以 seed 20261026 打亂，
並固定 3,000 行 test 與 2,000 行 dev。訓練時移除與這些 holdout 完全相同或有任何 8 字元重疊的內容。
之後的訓練前處理會從**兩個**訓練來源中排除全部四個 holdout；挖掘新詞也套用相同的排除。

生成器：**Qwen/Qwen3-8B**，revision
`b968826d9c46dd6066d109eabc6255188de91218`。該檢查點的 LICENSE 為 Apache-2.0，SHA256
`832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e`。
授權全文存於 `raw/Qwen3-8B-LICENSE`；它對輸出用途沒有限制。
不使用 Llama／Gemma 模型、論壇爬取資料、字幕或非商業來源。
生成器產生原創訊息，而不是被要求引用來源。

`generate.py` 使用 **200 個主題 × 8 種語域 × 10 種人物設定 × 4 種長度**的笛卡兒積網格，另外加上以 seed
產生的情境。每個請求要求 32 則獨立訊息，temperature .95、top-p .96、presence penalty .45，並設定
`enable_thinking=false`。檢查點權重為 BF16；vLLM 綁定 loopback 連接埠 8102。
`VLLM_USE_FLASHINFER_SAMPLER=0` 讓這台主機不需要為 FlashInfer JIT 安裝全域 CUDA toolkit。當時安裝的環境固定在 `requirements.lock`（uv，Python 3.12.15），與訓練環境彼此獨立；現在由 `pyproject.toml` 的相依群組提供：`train`（清理與生成請求）、`generate`（vLLM 伺服器）。

`clean.py` 接受 4–80 個字元、漢字至少占 60%、**`s2t` 轉換後完全不變**的行，並套用台灣用詞過濾，以及對
**全部四個** dev／test 集的完全相同與 8 字元重疊排除。這條較嚴格的不變規則高於所要求的 90% 繁體門檻。
OpenCC 也會改變一些有效的台灣異體寫法，包括吃→喫與台→臺：這類句子會被拒絕，而不是默默正規化。這會降低
覆蓋率，是所要求過濾條件的明確限制。`s2twp` 的輸出只有在漢字序列不變時才接受；否則只在中國大陸使用的
詞會導致拒絕。完全去重使用 BLAKE2b-128。近似去重使用 64-permutation MinHash LSH，以字元 trigram 計算，
門檻 .85，並以精確 Jaccard 確認候選。LSH 是機率性的，不保證找到每一筆近似重複。

在恰好達到 **1,500,000 個乾淨句子**時，固定 `synthetic_phase1.txt` 並寫入 `PHASE1_READY`。之後接受的行只
附加到 `synthetic_background.txt`，目標是**總共 6,000,000 個乾淨句子**。一個回應可能讓總數超過最終目標
幾行。原始回應、request ID、完成原因、token 用量與時間戳記附加到 `raw/synthetic.jsonl`。
檢查點的 offset 只指向完整且已 flush 的回應。續跑時，清理程式從兩個乾淨檔案重建 MinHash 索引，再從其
原始 offset 繼續；生成器略過已完成的 request ID。生成器重新啟動時，會截掉最後一筆不完整的原始紀錄。

## 停止與續跑

`generate` 步驟同時執行生成與清理。要提早停止：建立 `data/build/corpus/STOP_GENERATION`，等生成程式結束後再
建立 `STOP_CLEANER`（清理程式會先處理完所有完整的回應）；兩個程式結束後步驟會停止模型伺服器。續跑時移除兩個
停止檔再執行同一個步驟。檔案只會附加，加上完全去重，重播是安全的。生成與清理在總數達 6M 時自動停止。

## 沒有採用的做法

- **三元模型 `lm2` 的新詞挖掘**：`mine_words.py` 使用頻率 >=200、最小切分 PMI 2、兩側邊界熵皆 >=.6，最多新增 2,000 個詞。讀音來自**每個**字
  都只有唯一種子讀音的情況，或 CC-CEDICT 中沒有歧義的詞讀音，並優先採用台灣讀音。它從
  `data/tools/build_lexicon.py` 載入既有的純拼音轉換函式，不修改該檔案，也不加入 wordfreq。
  `new_words.tsv` 保留計數、讀音、證據與凝聚度，供審查使用。
  它只服務 `lm2`（沒有採用，見 [../lm/REPORT.md](../lm/REPORT.md)），程式已移除，見 commit 1341eb7。
  `lm2` 用的是 `synthetic_phase1.txt` 截至當時的快照 `synthetic_snapshot1.txt`（1,199,954 句，見
  `synthetic_snapshot1.json`）。

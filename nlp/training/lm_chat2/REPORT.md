# Chat2 實驗：2026-10-27

**依所要求的重複失敗規則停止。訓練與 perplexity 已通過驗證；生成未通過品質審查，轉換評估尚未完成。**
重新訓練的模型 perplexity 比舊聊天 LM 低很多，在立法院對話上尤其明顯，但混合模型在兩個一般類口語的
dev 集上仍不如維基百科模型。新的多輪生成兩次樣本審查都沒有通過，已經停止；**沒有使用任何新生成的
發言**，因此沒有達到一百萬則發言的目標。

## 來源與品質

下列計數是經過文件保留、去重與所有 holdout 排除之後保留的訓練句。原始大小是下載的 HTTP 回應本體，
不是模型大小。完整 URL、取得日期、各檔 SHA256 與生成器權重雜湊見
[source-manifest.json](../../../data/build/lm_chat2/source-manifest.json)；
兩個收據包的雜湊與原始大小見
[source-bundles.json](../../../data/build/lm_chat2/source-bundles.json)。

| 來源 | 版本／授權 | 原始大小 | 保留句數 |
|---|---|---:|---:|
| 立法院質詢問答，經由 [OpenFun LYAPI](https://v2.ly.govapi.tw/) 與[公報議程中繼資料](https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda/tree/16860178b7fbf234201db7d5c030f3fbab869a7a) | 2024–2025 文件；中繼資料 revision `16860178…`；OpenFun CC BY 4.0，原始資料 [Taiwan OGDL v1](https://data.gov.tw/license) | 506 份文件，63.00 MB | 203,621 |
| [中文維基百科討論頁](https://zh.wikipedia.org/w/api.php) | 2026-10-27 API 快照；保留確切的 revision 與歷史連結；[CC BY-SA 4.0](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content) | 30 個回應，21.02 MB | 7,297 |
| [Common Voice sentence collector](https://github.com/common-voice/common-voice/blob/20fd76cd7d3b1e937fe99359b2ef3649b9c466ac/server/data/zh-TW/sentence-collector.txt) | Commit `20fd76cd…`；CC0 1.0 | 既有訓練檔 0.22 MB | 8,309 |
| 相容的舊 v3 口語語料 | Qwen／Common Voice 系譜，SHA256 `787ae7aa64dcf88a…`；[Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B/tree/b968826d9c46dd6066d109eabc6255188de91218) 權重 Apache-2.0，對輸出用途沒有限制 | 320.39 MB | 5,894,993 |
| 新的 Qwen 多輪試跑 | 同一個固定檢查點；每次試跑 8 個請求，兩次都被拒絕 | 保留原始回應 | **0** |
| [data.gov.tw 會議紀錄](https://data.gov.tw/dataset/16827) | OGDL v1；2015-06-01 ODT 樣本，SHA256 `2906815305965d9d…` | 0.24 MB 樣本 | **0：只有決議摘要，沒有發言** |
| Taiwan Tongues | TRAIL 不在本任務的授權允許清單內 | 只用於既有基準與評估 | **兩個新版本中皆為 0** |

每個真實來源最初各取 20 對樣本，發現其中有主席、議程、出席名單等文字，以及維基簽名與引文的殘留。
擬合模型之前已收緊過濾條件；初步的分割與前處理結果已封存。公報樣本包括
「這樣不好啊！」與「是啦，這個目前都正在改善當中」，也有很長的政策答詢。
維基樣本包含真實的來回編輯討論，但也有地區性用詞、爭執與生硬的原文。與既有的維基流程一樣使用 OpenCC
**s2twp**，並不能讓這些內容都變成自然的台灣訊息用語。
保留有簽名的來源順序；維基的相鄰關係不是經過驗證的回覆樹邊。
目前的四字元下限也會捨棄「對」或「好啊」這類極短的回覆；本實驗並未確立它們在對話上的覆蓋程度。
見 [source_samples.json](../../../data/build/lm_chat2/source_samples.json) 與
[quality-review.json](../../../data/build/lm_chat2/quality-review.json)。

第一次生成器試跑經機械過濾後保留了 225 則發言，但其中有虛構的設施與前後不一致的對話。改為每個請求只
生成一段對話的版本，經機械過濾後只保留了六則發言：格式錯誤的巢狀結構、重複的 JSON 與不連貫的對話依然
存在。兩次試跑都在樣本審查後排除；之後沒有進行大規模生成。去重與模板上限的程式碼已經存在，但**尚未**在
百萬則發言的規模上驗證。

## 模型與 holdout

| 版本 | 不重複訓練句 | 詞 token | 儲存的 bigram | 三個 TSV |
|---|---:|---:|---:|---:|
| `lm_chat2` | 6,114,220 | 80,276,183 | 1,147,406 | 36,403,019 位元組 |
| `lm_chat2/no_generated` | 219,227 | 5,563,978 | 363,934 | 13,753,556 位元組 |

重用既有流程的三輪詞庫 Viterbi hard EM 與剪枝後的 MKN；計數門檻為 2，自然對數 TSV，每個版本都低於
60 MB。73,435 個乾淨的相鄰發言對貢獻補充的「最後一個詞／第一個詞」邊界計數。沒有訓練新的神經模型。

dev／test **各包含 1,000 個真實發言對**，900 個公報對加 100 個維基對，前一則發言在 TSV 第 1 欄，目標在
第 2 欄。seed 20261027；整份文件只保留給單一分割。在計數之前，兩欄內容與全部 16 個固定的舊評估檔
（包括同音字 TXT／JSONL 與冒煙測試集）都用來擋下完全相同與八字元重疊的內容，包括正規化後的台／臺寫法。
兩個模型都通過了各自的完整語料與上下文重新掃描，以及序列化後的機率正規化檢查。取得資料後，也檢查了
dev／test 雜湊、文件分離與跨分割的 8-gram 分離。test 仍未計分。這些 holdout 主要是**議會發言**，不是
私人聊天的基準；防洩漏的保證涵蓋新的詞 LM，不會回溯排除沿用的維基、v3 或 reranker 模型中的內容。

## 詞 perplexity

所有模型都使用同一個固定的**舊聊天模型最終 E-step 斷詞器**；預測目標詞與 EOS，分母不含 BOS 與上下文。
既有三個集合的維基與聊天模型數字與前一份報告一致。對話只預測目標文字，若前一則的最後一個詞以漢字結尾，
就以它作為起始上下文，否則用 BOS。

| Dev 集 | 行數 | 預測數 + EOS | 維基 | 舊聊天 | Chat2 | 不含生成文字 |
|---|---:|---:|---:|---:|---:|---:|
| 維基百科 | 2,000 | 52,831 | 455.540 | 29,584.005 | 7,975.715 | 4,063.979 |
| 口語 | 2,000 | 12,113 | 1,349.236 | 4,863.629 | 2,451.426 | 1,444.970 |
| Taiwan Tongues | 1,500 | 8,894 | 1,242.673 | 3,084.437 | 2,276.882 | 1,718.976 |
| 對話 | 1,000 | 25,324 | 1,596.623 | 1,932.647 | 393.939 | 254.579 |

新出現的 token 對應到各模型的 `<unk>`；未知詞數量與所有逐行對數概似都有保留。維基模型在對話目標中有
1,034 個未知詞，每個聊天模型則各有 11 個，因此詞彙與 fallback 的差異仍是這項指標的一部分。
不含生成文字的消融模型在每個 dev 集上的詞 perplexity 都低於混合模型，但在一般口語與 Taiwan Tongues 文字上
仍高於維基模型。

## 轉換評估與建議

**已停止，尚未完成：24 列中完成 7 列。**在 `P=0.5` 時，對話評估有一個 CLI worker 被 SIGKILL 終止。kernel journal 記錄到該列執行期間反覆發生全域 OOM kill，其中也包括另一個工作的 Python 程序。記憶體壓力是可能的原因；最初的子程序包裝沒有記錄被終止程序與批次的確切對應。新模型沒有任何轉換結果，因此**不建議新的聊天 prior，也不建議變更預設值**。

每個集合 400 個成對的 4–20 漢字測試片段，seed 7，60 個 worker，固定的 CLI 與模型；reranker count 16，v3 weight 0.5，使用所要求的聊天 prior。維基使用行內上下文；對話只對目標計分，並以前一則發言作為上下文。表格中的數值為完全相同／寫法視為相同的百分比。`—` 表示未完成。

| 聊天 LM | Prior | 口語 | Taiwan Tongues | 維基 + 上下文 | 對話 + 上下文 |
|---|---:|---:|---:|---:|---:|
| lm_chat | 0.3 | 85.50 / 85.50 | 70.50 / 84.75 | 84.00 / 84.00 | 85.75 / 85.75 |
| lm_chat | 0.5 | 85.50 / 85.50 | 70.00 / 84.00 | 84.25 / 84.25 | — |
| lm_chat | 0.7 | — | — | — | — |
| lm_chat2 | 0.3 | — | — | — | — |
| lm_chat2 | 0.5 | — | — | — | — |
| lm_chat2 | 0.7 | — | — | — | — |

基準 P=0.3 的平均為完全相同 81.44%、寫法視為相同 85.00%。這不能用來為 chat2 選擇 prior。不含生成文字的消融模型只有詞 perplexity 結果。有序的輸入、目標、預測、部分摘要與失敗記錄都有保留；沒有評估任何 test 分割。

記錄的 26 個檔案的執行環境指紋全部維持不變。隔離工作區中沒有本實驗擁有的評估、CLI 或模型伺服器程序在執行。帶有已存批次診斷的重試在同步時結束：共用 runner 同時進行的修改現在預期 `nlp/engine`，而本實驗的快照保留的是原本的根目錄版面。原始啟動腳本保留為 `data/build/lm_chat2/frozen-runner.sh`；主機有足夠資源後要續跑前，請先參閱下方「當時的執行方式」。

## 執行與失敗

- 所有擬合、計分與轉換都在 GPU 主機上執行；已完成的七列轉換使用 ONNX CPU、`JOBS=60`，位於
  `~/zhuyintype/work/onnx-eval-chat2`。另外的 CUDA 試跑說明如下。Python 3.12.15、
  uv 0.13.5、NumPy 2.5.4、OpenCC 1.4.2、mwparserfromhell 0.7.2、
  pyahocorasick 2.3.1、ONNX Runtime 1.24.4、Swift 6.4.1。
- 遠端的 HTTP 429 讓最初的下載程式停止。改由 Mac 以較慢的請求速度快取，取回了維基快照；上游反覆回傳
  HTTP 500，使公報請求停在 506／800 份文件。已快取的回應本體與收據固定下來，沒有默默替換。抽取前修正了
  一個無效的 API 列舉值。
- 主機 RAM 壓力讓本實驗第一次啟動 Qwen 失敗。之後本實驗的伺服器在確認資源後使用私有連接埠 8112 與 0.80 的
  GPU 使用率；試跑失敗後以其 PID 停止。停止訊號只送給本實驗擁有的程序 PID。之後的 kernel OOM 事件終止了
  共用主機上的其他程序。
- 一個舊格式 tab 字元的 assertion 曾讓混合語料的前處理停止一次。重用舊訓練程式的 NFC 與空白正規化後解決；
  未完成的目錄已封存，混合語料的前處理、擬合與稽核都順利完成。
- 傳輸緩慢之後，以遠端既有、雜湊相同的檢查點初始化隔離工作區。匯出與建置記錄、執行環境識別、有序預測、
  雜湊、驗證 JSON 與 nohup 記錄都保留在 `data/build/lm_chat2/`；原始下載內容在 `data/raw/lm_chat2/`。
- 私有的 fp32 CUDA 批次試跑與已完成的全部 800 個 CPU 預測一致，但在共用負載下，它的兩列分別花了
  121.8／125.9 秒，CPU 則為 66.0／66.4 秒。它以本實驗擁有的評估工具與 worker PID 停止；其伺服器由 runner
  清理。部分比較保留未變更的 CPU 結果。
- 五項針對 parser、上下文、洩漏、MKN 與錯誤收據的測試通過，shell 語法檢查與實際的隔離 runner 也都通過。
  本任務沒有變更 App、Swift、文件或 data-SOURCES，沒有變更既有環境的相依套件，沒有 Git
  commit／stash／reset／checkout，沒有對 test 分割計分，也沒有在 Mac 上評估模型。本任務對共用 runner 只新增了
  chat2 進入點與可設定的本機記錄位置。

現在的指令與檔案配置見 [README.md](README.md)。重新散布這些模型時，請保留
[NOTICE.md](../../../data/build/lm_chat2/NOTICE.md) 與精簡的 0.49 MB `attribution.jsonl` 清單。

## 當時的執行方式

以下是 2026-10-27 實驗時的做法，保留作為紀錄。現在的對應指令是 `zt --remote train lm_chat2 perplexity conversion`；
`conversion` 直接使用 `nlp.common.zhuyin` 的讀音與按鍵（與當時 `conversion_eval.py` 的做法相同），模型路徑改為
資料目錄中的絕對路徑，所以既有的逐列結果會被視為過期。

### 遠端評估（隔離工作區）

依 README 的設定方式建立 `nlp/remote`，位置為 `~/zhuyintype/work/onnx-eval-chat2/.venv`。其 `.pth`
檔以唯讀方式重用 `.venv-gen` 的 Torch，以及 `.venv` 的 parser／OpenCC／Aho-Corasick 套件。
在這個隔離環境中只安裝 runner 固定版本的需求套件。共用 runner 新增了 `chat2`（只對目標做 TSV 評估）
與 `LOCAL_RESULTS`（讓收集到的記錄留在本實驗內）。`EXTRA_SYNC` 處理新檔案。

`ENGINE_SNAPSHOT=data/build/lm_chat2/engine` 固定既有的引擎與轉換評估工具；其 SHA256 清單為
`engine-manifest.json`。比對雜湊後，可以用遠端既有、完全相同的檢查點副本初始化隔離目錄，
省去第二次網路傳輸。

將下列已完成的目錄與檔案複製到隔離工作區中相同的相對路徑：模型 TSV、`no_generated/` TSV、兩個對話檔、
`inputs/`（包括舊的聊天斷詞器）、當時的 `nlp/training/lm/pipeline.py` 與 `nlp/training/lm_chat/train.py`。
由 Mac 同步時，將它們列在 `EXTRA_SYNC`：

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-chat2 JOBS=60 BACKEND=cpu \
  ENGINE_SNAPSHOT=data/build/lm_chat2/engine \
  LOCAL_RESULTS=data/build/lm_chat2/remote-logs \
  EXTRA_SYNC="nlp/training/lm_chat2 nlp/training/lm/pipeline.py nlp/training/lm_chat/train.py data/build/lm_chat2/inputs data/build/lm_chat2/dev_dialogue.txt data/build/lm_chat2/test_dialogue.txt data/build/lm_chat2/unigram.tsv data/build/lm_chat2/bigram.tsv data/build/lm_chat2/backoff.tsv data/build/lm_chat2/no_generated/unigram.tsv data/build/lm_chat2/no_generated/bigram.tsv data/build/lm_chat2/no_generated/backoff.tsv" \
  bash -c "$(cat data/build/lm_chat2/frozen-runner.sh)" \
    nlp/remote/run.sh chat2 --phase conversion -n 400
```

這會以原本的腳本位置作為 `$0` 呼叫保留下來的原始啟動腳本，因此即使線上 runner 同時遷移，仍維持固定快照
的根目錄版面。只在目前的輸入檔與記錄的雜湊一致時使用；否則請另開新實驗，不要重用已快取的七列。

runner 以 `nohup` 啟動評估工具並記錄其 PID。評估在遠端 GPU 主機上使用 ONNX CPU，並拒絕在 macOS 上執行。
選用的 `BACKEND=cuda` 使用 runner 私有的 2 GiB 批次伺服器；在本次執行的共用負載下它比較慢。部分比較使用
CPU。評估工具設定為所要求的 v2b count 16／v3 weight 0.5 組態，兩個聊天 LM 的 prior 分別為 0.3／0.5／0.7，
60 個 worker，每個集合 400 次，seed 7。標準集合使用既有評估工具的選取規則；對話集只以目標發言產生測試
片段，以前一則發言加上較早的目標文字作為上下文（最後 32 個字元，與 `conversion_eval.py` 一致）。
有序的按鍵、目標、輸出與寫法視為相同的準確率逐列存於 `conversion/`。沒有評估任何 test 分割。

perplexity 對四個模型（包括消融模型）都使用**同一個固定的舊聊天模型最終 E-step 斷詞器**，在所有 dev 行上
計算。分母包括 EOS，不含 BOS 與上下文。新出現的目標與上下文 token 對應到各模型的 `<unk>`；未知詞數量
另行回報。對話分數只預測目標，若前一則的最後一個詞以漢字結尾，就以它作為起始上下文，否則用 BOS。
這些不是新模型以自己的斷詞器重新斷詞後的訓練 perplexity。計數與逐行對數概似可用於重建結果。

### 多輪生成試跑

`serve.sh` 使用私有連接埠 8112，並要求至少 40,500 MiB 的 GPU 記憶體空閒；啟動前**也要檢查主機 RAM**。
絕不重新啟動既有的伺服器，也不停止其他工作。已停止的伺服器 PID 與其記錄都保留。
`generate.py` 送出有上限的請求，並在三次請求錯誤後停止。
`clean.py` 檢查 4–12 則發言、是否完整結束、台灣用詞與長度，接著進行完全相同與模板去重、64-permutation
MinHash 加上完全 5-gram Jaccard >=.8，以及 v3 的前六字／後四字家族上限 100。被拒絕的發言會切斷相鄰關係。
MinHash 檢索是機率性的。這些過濾無法保證語意正確或完全沒有錯字：**兩次實際試跑都沒有通過人工樣本審查**。
非空的生成訓練文字另外需要帶有已審查 `approved_sha256` 的 `generation-review.json`；本實驗沒有核准任何生成文字。

試跑用的 `generate.py`、`clean.py` 與 `serve.sh` 在兩次試跑都沒有通過審查後移除，見 commit 1341eb7；`prepare` 在沒有 `synthetic_dialogues.jsonl` 時與檔案為空時相同。

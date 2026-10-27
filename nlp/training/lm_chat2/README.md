# Chat2 語料與詞 bigram

CPU 前處理、訓練與**所有評估都在實驗室的 Linux 主機上執行**。Mac 可以快取公開的下載內容並傳輸檔案。
不安裝任何全域工具。程式碼採 AGPL-3.0-only 授權；模型與語料的出處標示見 `data/build/lm_chat2/NOTICE.md`。

**已停止，尚未完成：**反覆發生的全域 kernel OOM kill 中斷了 60 個 worker 的轉換掃描（24 列中完成 7 列）。
目前不能得出任何 chat2 轉換或 prior 的結論。請等主機有足夠的專用資源後再續跑。同時進行的 runner 版面變更
（改為 `nlp/engine`）也讓重試時與原始快照的同步失敗。請保留隔離的執行環境，並驗證其
`evaluation-manifest.json` 的雜湊；不要把新的線上來源或模型混入這份部分比較。

這個實驗在兩次品質試跑失敗後，停止了新的 Qwen 生成。兩次試跑都保留在原始資料儲存區，並已排除。
`lm_chat2` 指相容的舊 Qwen／Common Voice 文字，加上經過過濾的新真實轉寫；`no_generated/` 只包含真實
轉寫與 Common Voice。兩者都不含 Taiwan Tongues 的訓練文字。不要把舊模型的 TRAIL 授權視為相容。

## 固定資料

- 原始 HTTP 回應本體，以及各自的 URL、日期、授權與 SHA256 收據：
  `data/raw/lm_chat2/`。政府 API 錯誤在嘗試三次後停止。
  維基請求遵守十秒間隔與 HTTP 429 的 Retry-After。
- `real_dialogues.jsonl`：原始來源文件、發言者、發言順序、維基討論串、revision 與出處連結。無效的發言
  保留為空的分隔；上下文不會跨越它們串接。維基的上下文是來源順序中前一則有簽名的發言，不是經過驗證的
  回覆樹邊。
  沿用的四字元下限會捨棄極短的回覆；這仍是自然訊息覆蓋範圍的一項限制。
- `source_samples.json`、`quality-review.json`：訓練前的樣本審查。
  只有摘要的 data.gov.tw 會議紀錄與兩次生成器試跑都被拒絕。
- `dev_dialogue.txt`／`test_dialogue.txt`：各 1,000 個**發言對**，TSV
  `previous_turn<TAB>target_turn`；每個分割有 900 個公報對與 100 個維基對。
  整份文件只保留給一個分割，或從訓練中排除。seed、逐列的來源 URL、來源雜湊與既有評估雜湊記錄在
  `splits.json`。test 文字從未被計分，也未用於選擇 prior。
- `inputs/evaluation/`：固定的既有 dev／test 文字，包括目前所有的同音字 `dev_*`／`test_*` TXT 與 JSONL
  檔，以及小型冒煙測試集。`inputs/local-input-manifest.json` 對應回原始路徑與雜湊。

所有訓練版本都會移除與兩個新 holdout 的**兩欄**以及每個固定的舊評估檔完全相同，或有任何八字元重疊的
內容。過濾也會擋下經 OpenCC 或常用「台」字正規化後的變體。另有一次匯出後掃描，檢查每個保留的訓練句與
上下文對。`source-manifest.json` 記錄舊語料、中繼資料、估計器與生成器權重的雜湊。請保留它與完整的出處
標示清單。`attribution.jsonl` 是精簡的 0.49 MB 重新散布清單；App 套件不需要完整的轉寫文字。

## 重現

使用遠端既有的 `.venv`（Python／NumPy／OpenCC／parser／Aho-Corasick）與 `.venv-gen`
（vLLM／aiohttp／datasketch）。不要在任何一個環境中安裝套件。
原始來源下載工具只使用標準函式庫，可以在 Mac 上執行：

```sh
.venv/bin/python nlp/training/lm_chat2/sources.py government --documents 800
.venv/bin/python nlp/training/lm_chat2/sources.py wikipedia --batches 20
.venv/bin/python nlp/training/lm_chat2/sources.py similar
```

800 份文件的政府請求在上游回傳 HTTP 500 時停止；本實驗固定使用已快取、經收據驗證的 506 份文件。
若要發表比較結果，不要默默重新下載或重新抽樣。大量請求中斷時，`prepare.py extract` 可以從這些
快取收據建立索引。

在遠端主機上審查樣本之後：

```sh
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
.venv/bin/python nlp/training/lm_chat2/test_pipeline.py
.venv/bin/python nlp/training/lm_chat2/prepare.py extract
# 固定分割與計算訓練文字之前，先審查 source_samples.json。
.venv/bin/python nlp/training/lm_chat2/prepare.py split
nohup bash nlp/training/lm_chat2/run_train.sh \
  > logs/lm-chat2-train-final.log 2>&1 < /dev/null &
```

前處理不會覆寫已完成的語料，也不會重新抽樣已固定的分割。訓練不接受既有工作目錄中變更過的輸入、
程式碼或設定。變更資料前，請保留舊目錄並另開一個實驗。兩個模型的輸出格式相同，都是自然對數的
`unigram.tsv`、`bigram.tsv`、`backoff.tsv`，各自上限為 60,000,000 位元組。重用
`nlp/training/lm/pipeline.py` 的三輪 Viterbi hard EM、0.1 pseudocount、MKN 與計數門檻剪枝。
每個乾淨的相鄰發言對額外貢獻一個「前一則最後一個詞／下一則第一個詞」的 bigram；發言內的句子保留
BOS／EOS 事件。這是補充性的邊界計數估計，不是學習而得的長上下文神經模型。

## 遠端評估

依 README 的設定方式建立 `nlp/remote`，位置為 `~/zhuyintype/work/onnx-eval-chat2/.venv`。其 `.pth`
檔以唯讀方式重用 `.venv-gen` 的 Torch，以及 `.venv` 的 parser／OpenCC／Aho-Corasick 套件。
在這個隔離環境中只安裝 runner 固定版本的需求套件。共用 runner 新增了 `chat2`（只對目標做 TSV 評估）
與 `LOCAL_RESULTS`（讓收集到的記錄留在本實驗內）。`EXTRA_SYNC` 處理新檔案。

`ENGINE_SNAPSHOT=data/build/lm_chat2/engine` 固定既有的引擎與轉換評估工具；其 SHA256 清單為
`engine-manifest.json`。比對雜湊後，可以用遠端既有、完全相同的檢查點副本初始化隔離目錄，
省去第二次網路傳輸。

將下列已完成的目錄與檔案複製到隔離工作區中相同的相對路徑：模型 TSV、`no_generated/` TSV、兩個對話檔、
`inputs/`（包括舊的聊天斷詞器）、`nlp/training/lm/pipeline.py` 與 `nlp/training/lm_chat/train.py`。
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

## 生成器的使用範圍

`serve.sh` 使用私有連接埠 8112，並要求至少 40,500 MiB 的 GPU 記憶體空閒；啟動前**也要檢查主機 RAM**。
絕不重新啟動既有的伺服器，也不停止其他工作。已停止的伺服器 PID 與其記錄都保留。
`generate.py` 送出有上限的請求，並在三次請求錯誤後停止。
`clean.py` 檢查 4–12 則發言、是否完整結束、台灣用詞與長度，接著進行完全相同與模板去重、64-permutation
MinHash 加上完全 5-gram Jaccard >=.8，以及 v3 的前六字／後四字家族上限 100。被拒絕的發言會切斷相鄰關係。
MinHash 檢索是機率性的。這些過濾無法保證語意正確或完全沒有錯字：**兩次實際試跑都沒有通過人工樣本審查**。
非空的生成訓練文字另外需要帶有已審查 `approved_sha256` 的 `generation-review.json`；本實驗沒有核准任何生成文字。

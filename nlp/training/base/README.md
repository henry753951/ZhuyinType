# 統計基底實驗（A1）

程式碼採用 AGPL-3.0-only 授權。所有訓練、模型檢查與評估都在 Linux GPU 主機上執行。
開發用的 Mac 只負責編輯與傳輸檔案、取回產出物。這裡不包含引擎、App、共用評估器、
資產清單或文件的整合。以 dev 選出的組合在 TT 與冒煙測試上有退步；A1 的驗收標準
仍未達成。整合這些研究資產之前，請先閱讀 REPORT.md。

`data/build/base/` 存放凍結的引擎與評估器快照、現行的打字詞庫、來源雜湊值、
增補用的小麥注音詞庫、準備好的語料、候選模型、依序排列的預測結果與成對統計。
最終交付物是 `lexicon.tsv`、`lm/`、`lm_chat/` 及其聲明檔。未選用的候選模型留在
`models/` 下。現行資產從不被覆寫。

## 方法

增補後的詞庫完整保留現行的每一筆 `(reading, word, score)`。
只匯入每個字都對應一個有效注音音節的漢字條目。
小麥注音的 log10 分數轉為自然對數。新詞使用同長度共有詞的分數偏移中位數
（長度 >=5 合併計算）；既有詞的新讀音不能高於該詞目前最佳讀音減 1 nat。因此既有的
台灣讀音與多音字比例都維持不變。最終合併保留來源提供的一／不變體，不為序數或數字名稱
推論新的變體。接受擴充平面的漢字。來源歸屬、被拒絕的類別、校準偏移與 SHA256 都有記錄。

斷詞使用現行最終 E 步的 unigram 分布（依領域使用 wiki 或 chat），新引入的詞則使用
校準後的詞庫機率。在相互對照的硬計數與期望計數模型之間，斷詞器保持不變。
期望計數加總每一種斷詞（含 BOS 與 EOS）下精確的 forward/backward 邊邊際機率與
相鄰邊的聯合後驗機率。不設逐邊的後驗門檻。計數在各分片與來源間加總，生成文字
加權後，計數只**一次**四捨五入，以便沿用既有的整數 MKN 估計器及其剪枝與 backoff
正規化。這樣可以單獨隔離最後的計數方式變更。第二組相互對照的 chat 對照組使用
`--fresh`：從正規化的詞庫先驗初始化，執行三次 E 步，中間穿插兩次經驗 unigram M 步
（偽計數 0.1），再比較硬計數與後驗計數。生成文字的權重在這些 M 步中同樣適用。
這把語料適應與現行斷詞器對生成文字的使用歷史分開。

維基百科沿用現行已清理、全域去重的訓練分片。
真實聊天文字沿用 chat2 以文件為單位保留、不含生成文字的分片（立法院、維基百科
討論頁、Common Voice），只另外加入 Taiwan Tongues 的 `train.tsv`。
生成文字沿用既有的 v3 Qwen／Common Voice 資料池，扣除與真實來源完全重複的部分。
它以權重 0.1 測試，不重新生成。
候選模型保留 Taiwan Tongues 的 TRAIL 條款，在完成整合與授權審查之前屬於研究資產；
單憑 CC BY-SA 無法描述它們的授權。

擬合之前，每個來源都會再與目前所有 dev／test 檔案、chat2 對話的兩個欄位、
同音字檔案、冒煙測試與匯入的外部資料列重新比對。完全相同以及任何連續八個字相同的
內容都會排除，包括台／臺正規化後的版本。保留測試資料的轉換只用來產生排除用的形式，
從不作為訓練樣本。測試集的預測結果從不用於選擇模型。

所有設定都使用相同的每組 400 次執行、seed 7、現行打字詞庫與凍結的按鍵。Dev 集：
口語、不含與含行上下文的維基百科，以及 Taiwan Tongues（同時報告完全相同與變體視為
相同）。引擎模式使用同樣的 chat 先驗 0.5，但不含神經模型。App 模式加入未變更的
v2b top-16（權重 1）與 v3（權重 0.5）。CLI 批次會儲存診斷資訊，遇到任何非零回傳值
或輸出數量錯誤就停止。結果包含模型與 CLI 的 SHA256。`stats.py` 提供成對 McNemar、
bootstrap 區間與破壞率上下界。詞庫稽核區分逐讀音的資料列、每個詞偏好的讀音，以及
在每個讀音下都落敗的詞：同音字不可能全部排第一。使用習慣分析沿用 `error_analysis.py`
的規則，套用在凍結的口語 dev 執行結果上，並報告顯示準確率與引擎 oracle@16。

模型選擇只用 dev：主要分數是口語完全相同、含上下文維基百科完全相同與 Taiwan Tongues
寫法視為相同這三者的等權平均。在維持 App 主要分數的前提下，優先採用引擎有進步的
設定；不含上下文的維基百科作為額外的防護指標。所有逐步數字與取捨都寫在 REPORT.md。
在對測試集做唯一一次評估之前，先凍結最終設定。

選定的組合是修正後的小麥注音詞庫、`wiki_expected_wide` 與 `chat_fresh_expected`，
不含生成文字的計數。三者必須一起使用；新的語言模型搭配舊的解碼詞庫時表現不佳。
整合時要一起重新產生編譯後的詞庫與語言模型資產：這些實驗使用的是 TSV，沒有會覆蓋
它們的二進位附屬檔。Mac 上的 mmap 記憶體用量與延遲尚未量測。

所有現行 TSV 放在 `inputs/baseline/`，七個評估文字檔放在 `inputs/evaluation/`。
`frozen-inputs.json` 固定這些檔案、打字詞庫、CLI 與神經網路資產。評估器會拒絕
已變更的輸入與過期的快取結果。乾淨的使用習慣分析執行必須完全重現它的主要 dev 擷取。
共用的 `data/build/lm_chat/` 不是不可變的基準：其他工作可能會取代它。

## 重現

使用遠端既有的 Python 3.12 環境，不另外安裝套件。獨立的 `.venv` 以 `.pth` 參照
既有的 parser／NumPy／Torch／ONNX 環境，不修改這些環境。最初的執行腳本已複製到
`data/build/base/frozen-runner.sh`；它唯一的本機分派新增項目，是以 `nohup` 呼叫
`nlp/training/base/evaluate.py`，並把紀錄寫到 `~/zhuyintype/logs/` 下。
呼叫這份複本時要傳入原本的 `$0`，讓根目錄的查找仍然有效：

```sh
REMOTE_DIR=zhuyintype/work/onnx-eval-base JOBS=8 BACKEND=cpu \
  ENGINE_SNAPSHOT=data/build/base/snapshot LOCAL_RESULTS=data/build/base/runner-logs \
  EXTRA_SYNC='nlp/training/base nlp/training/lm/pipeline.py nlp/training/lm_chat/train.py data/build/base/inputs data/build/base/baseline.json nlp/eval/stats.py' \
  bash -c "$(cat data/build/base/frozen-runner.sh)" nlp/remote/run.sh \
  base --matrix data/build/base/baseline.json --phase dev -n 400
```

把 `nlp/training/base/` 以及小麥注音的來源檔與聲明檔傳到該工作區後，在遠端執行：

```sh
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
.venv/bin/python nlp/training/base/test_base.py
.venv/bin/python nlp/training/base/train.py lexicon --source-root "$HOME/zhuyintype"
nohup .venv/bin/python -u nlp/training/base/train.py prepare \
  --source-root "$HOME/zhuyintype" --workers 8 > "$HOME/zhuyintype/logs/base-prepare.log" 2>&1 < /dev/null &
echo $! > data/build/base/prepare.pid
nohup bash nlp/training/base/run_train.sh > "$HOME/zhuyintype/logs/base-train.log" 2>&1 < /dev/null &
echo $! > data/build/base/training.pid
```

模型訓練會拒絕已存在的輸出目錄；保留任何未完成的目錄，重試前另選一個新的模型名稱。
不要終止其他工作，也不要用範圍過寬的程序名稱比對。重複失敗時停止並記錄。

記錄下來的語言模型對照組使用封存的第一版詞庫（`lexicon_mcbopomofo.tsv`，
232,700 列）。修正後、只含來源條目的詞庫（`lexicon_source.tsv`）是另一個 dev 消融實驗：
它移除推論出的變調資料列，並接受擴充平面的漢字。詞模型的擬合與輸入讀音無關；最終組合
已用修正後的詞庫在 dev 上重新檢查。`train-v1.py` 保留最初的合併實作；`run_train.sh`
明確指定那個凍結的種子。目前的訓練程式預設使用修正後的詞庫進行後續實驗。
資料封存中的 `train-fresh-v2.py` 與選定 chat 擬合所用訓練原始碼的指紋完全一致；
目前的訓練程式在那次擬合之後只加了關於四捨五入上限的註解。
`run_fresh.sh` 執行三個經語料適應的 chat 對照組。`prior.py` 是被否決的、不考慮上下文的
頻率對照組；`prune.py` 執行唯一一次較寬的維基百科剪枝消融，不重新計數語料。
矩陣、凍結紀錄與依序排列的結果檔隨本機資料包一起提供。

`fallback.py --source MODEL --output NEW_MODEL` 是只動資料的消融實驗。它只在沒有任何
已儲存的 bigram／backoff 參照時，移除處於零延續下限的詞。這些詞之後改用引擎既有的
詞庫退回機制。設移除的 unigram 質量為 M、上下文 backoff 為 alpha，將已儲存的機率
限制為 Z = 1 - alpha*M，把 unigram 乘以 1/(1-M) 重新縮放，並把新的 backoff 設為
alpha*(1-M)/Z。這樣能在保留的詞彙上維持條件機率的正規化。舊模型維持原樣，仍可使用。

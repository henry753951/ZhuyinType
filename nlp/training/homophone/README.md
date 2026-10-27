# 同音字繼續訓練

## 相容重跑（v3_homophone2）

交付的 `v3_homophone2` 是沿用原始配方、訓練 500 步的相容候選。

後續決定：0.4.0 採用這個模型。它沒有改壞在／再測試集的任何句子，一般開發集的差異在雜訊範圍內，詳見 [研究紀錄](../../../docs/research/log.md)。以下是實驗當時的建議，保留作為紀錄。

實驗當時的建議是不採用：目前的 app 組合仍會把「我在吃一碗麵」改壞，也沒有修好「再試試看」。它從原本的 `data/build/v3` 開始，從未使用以 TRAIL 訓練的檢查點。另外嘗試的針對性訓練與中點版本都在開發集上被否決。結果與發布建議見 REPORT.md。下方的歷史說明只描述實驗 1；相容重跑不要沿用它的 TT 取樣器或 60 個 worker 的設定。

在 GPU 主機上使用既有的環境（不安裝相依套件）：

```sh
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python nlp/training/homophone/compatible.py test
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python nlp/training/homophone/compatible.py prepare --workers 4
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python nlp/training/homophone/test_compatible.py
nohup env PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
  .venv-gen/bin/python -u nlp/training/homophone/train_v3.py --compatible \
  --out data/build/homophone2 --model data/build/v3_homophone2 --steps 10000 \
  > logs/homophone2-train.log 2>&1 < /dev/null &
```

`compatible.py prepare` 拒絕寫入已存在的輸出目錄。它篩選不可變的維基百科／口語分片，預留 chat2 的文件，標注完整的真實來源子句，並獨立重新掃描保留的文件以及前文與目標的接合。它記錄輸入與保留集的雜湊。僅供評測的 TT 與所有匯入的善解文字都不得進入訓練。只有短前文時，前文只貢獻 8-gram，不作為子字串比對。一般批次也有 25% 取自真實來源的樣本。以主要字為重點的批次中，一半針對句首的字，在與再各半；其他取樣、損失、資料增強、凍結前文與開發集停滯規則不變。

在 Mac 上，依序執行 `run_compatible.py baseline dev` 與 `chosen dev`。它們呼叫目前共用 runner 的暫時副本，設定為 `REMOTE_DIR=zhuyintype/work/onnx-eval-homophone3`、`V3_MODEL=v3|v3_homophone2`、ONNX CPU、預設 JOBS=24（可用環境變數覆寫，最多 40），並使用 `data/build/homophone2_snapshot` 中凍結的目前引擎快照。隔離環境以唯讀方式重用既有的 portable 與 Torch site-packages。每個評測階段只能執行一次；有部分結果時先檢查，再進行明確稽核過的復原。在這個工作區中不要讓 runner 呼叫重疊。相容派送器以 nohup 在新的 session 中執行，記錄檔在 ~/zhuyintype/logs，PID 收據在 remote_eval/compatible-evaluation.pid。SSH 用戶端斷線不代表可以重播已封存的列：先檢查完成封存檔與自己擁有的程序。確認沒有評測程序仍在執行後，`RECOVER_TEST=1` 只允許完全相同的 manifest／快取重用，以及沒有開始紀錄的列；已開始但缺少完整原始預測的列一律不重播。衍生的附屬檔可以不解碼而重建。設定與目前 app 相同：整句重排候選數 16、權重 0.5，v3 權重 0.5，聊天先驗 0.3，預設整句重排門檻 8。每個評測列都會等到主機可用記憶體至少 48 GiB 才開始。基準與選定模型的有序輸出交給 `nlp/eval/stats.py`（成對 bootstrap 與 exact McNemar；同音字的信賴區間依來源行分群）。`choice2.json` 鎖定檢查點；兩個開發集完成封存檔與 `dev-decision.json` 都存在後，才能執行 `baseline test` 與 `chosen test`，每個角色／前文條件各一次。舊的測試集保持逐位元組相同，從不用於選擇檢查點。

開發集選定後，`export_v3.py --compatible` 使用未修改的上游 fp16 匯出器與 Xcode 編譯器，保留出處與各元件的授權，並產生原本的 20 個介面案例。`check_onnx.py --coreml-vectors ... --diagnostic` 記錄原始與新模型已儲存的 Core ML 輸出與 PyTorch 之間的差異，不套用歷史上固定的 0.1 上限。這項診斷不主張數值等價：拿它的差異大小與 argmax 翻轉和原始 v3 比較，再於 Mac CLI 上用同樣已儲存的 400 筆真實開發集輸入，做經授權的單次一致性檢查。`--check-v3` 與獨立的數值／真實輸出檢查要分開。這項工作不修改發布資產，也不安裝 app。

## 受控的後續嘗試

第一個相容檢查點改善了主要開發集，但重現已知的進行貌／重複退步。`compatible.py target` 重用相同的允許索引與真實子句，加入兩個重點子集來建立 `homophone2b`：動詞前的句首「再」，以及子句中任何位置的「主詞 + 在 + 動詞」。不產生新的訓練文字。中點版本在整句重排權重 0.5 下同樣保留「我在吃一碗麵」的退步，因此被否決。最初只取前綴的子集在訓練前就已封存，因為它漏掉了前面有連接詞的子句。

```sh
# GPU 主機，完成上方的相容準備之後：
.venv-gen/bin/python nlp/training/homophone/compatible.py target
nohup env PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
  .venv-gen/bin/python -u nlp/training/homophone/train_v3.py --compatible --targeted \
  --out data/build/homophone2b --model data/build/v3_homophone2b --steps 10000 \
  > logs/homophone2b-train.log 2>&1 < /dev/null &
.venv-gen/bin/python nlp/training/homophone/stabilize.py
```

針對性重試另外以三個已知指定範例的模型單獨首選結果作為選擇門檻：再試試看、我再吃一碗、我在吃一碗麵。這些是明確的開發用防護，不參與梯度訓練，也不是未見資料上的確認證據。模型單獨正確不代表 app 正確：重試版本在目前 app 權重 0.5 下把「我再吃一碗」改壞，因此被否決。早期的快速探測誤用了 conversion_eval 的整句重排預設值 1；這些結果另外封存，不納入 app 設定的比較。所有正式矩陣都使用 CLI 預設的 0.5；轉接程式現在明確固定這個值。

`stabilize.py` 只檢查原始 v3 與第一個合乎授權的繼續訓練版本之間的一個參數中點。它確認詞彙表、張量形狀與凍結的前文完全相同、輸出權重都是有限值、父模型與當初選定的版本一致，接著在 GPU 主機上量測原本全部八個一般開發集條件。它不改動任何 app 評分旗標。`stabilization.json` 記錄兩個父模型與 0.5 係數。app 仍使用 v3 權重 0.5。這是額外的穩定化步驟，不是未經修改的歷史配方，也不是係數掃描。

`homophone2_stable` 只在 CLI、模型、詞庫／語言模型與評測雜湊都相符時，才重用已認證的基準結果；這些結果與 `baseline-dev-complete.json` 一起放在該次執行的 `results/`。在 Mac 上：

```sh
JOBS=40 data/build/v3/.venv/bin/python nlp/training/homophone/run_compatible.py chosen dev homophone2_stable
# 中點版本在開發集上被否決；兩個替代方案都不開啟測試集。
# 凍結預設 homophone2 的開發集決定，然後每個角色各評分一次：
JOBS=40 data/build/v3/.venv/bin/python nlp/training/homophone/run_compatible.py baseline test
JOBS=40 data/build/v3/.venv/bin/python nlp/training/homophone/run_compatible.py chosen test
# 在 Mac 上為交付的預設候選匯出、檢查，並做單次一致性檢查：
data/build/v3/.venv/bin/python nlp/training/homophone/export_v3.py --compatible
.build-codex5/release/zhuyin-cli --check-v3 data/build/v3_homophone2
data/build/v3/.venv/bin/python nlp/training/homophone/mac_parity.py chosen
```

一致性檢查指令只能執行一次。它跑 400 筆已儲存的 Linux 輸入：前 397 筆是凍結的維基百科執行（含同一行的前文），加上三個已知的關鍵 smoke 輸入，並確認交付的檢查點與選定的執行相符。它暫時使用的詞庫／語言模型二進位檔在 `homophone2/` 底下重建，雜湊與 Linux 的二進位檔相符。原始 v3 對照組保留全部 400 筆維基輸入（其中 397 筆與最終檢查共用）。不要在 Mac 上跑語料評測來選檢查點。「再試試看」在中點探測中仍是已知失敗；最終量測的建議見 REPORT.md。

## 歷史實驗 1

開發集選出的候選：未修改的 v2b + `data/build/v3_homophone`，v3 權重 1，重排數量 16。`data/build/homophone/choice.json` 不可變更。量測、被否決的嘗試與限制見 REPORT.md。不要重跑已封存的測試。PyTorch／ONNX 開發集門檻通過。編譯後的 Core ML 通過 Swift 介面向量，但在一個合成案例上未達沿用的 fp32 數值容許誤差；不要把這些 Core ML 產物當作已完整驗證而升級採用。見 `coreml-pytorch-audit.json`。最終 smoke 維持 17/19 個主要位置，但出現新的進行貌／重複退步；這個實驗不是完成的正式修正。

所有訓練、模型品質評分與端到端評測都在 Linux 的 `~/zhuyintype` 下執行，以 `ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST` 連線。重用 `.venv-gen`；portable 工作區重用已驗證的 ONNX 環境。不安裝相依套件。遠端 Python 指令前加上 `PYTHONDONTWRITEBYTECODE=1`；長時間的工作使用 nohup 與 `logs/homophone-*.log`。失敗時先檢查原因，不自動重試；只停止已確認屬於自己的 PID。Mac 只匯出與編譯最終選定的 Core ML 模型，並執行指定的那一次凍結 CLI `--check-v3`（已通過）。

## 資料與訓練

1. `mine.py --reuse-inventory`：保留已完成的全語料計數，依文件順序每 20 行取一行維基百科，以及所有符合條件的 Taiwan Tongues zh-TW train.tsv 列。保留 4–31 個漢字的完整子句；排除與英數字相鄰的片段。依字／類別／來源各自做 reservoir 取樣：在、再各 800，其他 160。清查計數包含去重後的口語語料；它編碼後的訓練樣本也供應一般與重點繼續訓練批次。
2. `judge.py --previous PATH`：以 Qwen3-8B 對每個在／再獨立遮蔽並判斷，排除意見不一致、不確定以及與強固定搭配矛盾的結果。只快取文字與前文完全相同的判斷。`judge_job.sh VERIFIED_PID ...` 在判斷完成後立即停止那個已確認的模型／埠程序。絕不讀取 TT test.tsv。
3. `split.py`：交替分配到開發集與測試集，正規化空白前文，排除句尾懸空的在／再與無法打出的子句。每個分割需要 1,000 個主要子句，每個字至少 400 個位置；次要組的目標是每組 100。訓練前檢查 `review100.jsonl`，並把發現記在 `manual-review.json`。
4. `filter.py`：編碼後的文件若含有保留集的 8-gram，或短的實際句子／smoke 子字串，就整份排除。涵蓋先前的保留集、新的開發集／測試集、前文以及前文與目標的接合。檢查原始文字與 UNK 正規化後的文字。只有短前文時只貢獻 8-gram。把索引存入不可變的 mmap 分片，並保存不重複、有信心的 TT 訓練子句。被否決的早期分割與索引留在稽核目錄中。
5. `train_v3.py`：從原始 v3 以 2e-5 繼續訓練，batch 64，30% 重點批次，混淆位置損失乘 3，資料增強與前文 dropout 不變。重點選擇中一半針對平衡的在／再；符合條件的 TT 提供這部分的 25%。每 500 步檢查開發集，連續四次未改善就停止。只有在原本八個一般開發集條件都下降不超過 0.2 點時，才儲存主要開發集 NLL 改善的版本。

## Portable 評測

使用 **JOBS=60**、專用的遠端目錄與凍結的引擎／評測器快照。並行的工作移動了現行引擎的路徑；`remote.py` 使用已知可用的 `data/build/homophone/runner-pinned.sh`，先檢查語法，再在記憶體中執行它完整的原始碼。現行 runner 仍供其他實驗使用。

開發集執行範例（已存在的列需要明確稽核過的 `--resume`）：

```sh
ENGINE_SNAPSHOT=data/build/homophone/portable \
REMOTE_DIR=zhuyintype/work/onnx-eval-homophone-new \
EXTRA_SYNC='nlp/training/homophone nlp/training/v3/common.py nlp/training/v3/readings.py nlp/training/v3/reference.py data/build/homophone/inventory.json data/build/homophone/dev_zai.txt data/build/homophone/dev_zai.jsonl data/build/homophone/dev_pairs.txt data/build/homophone/dev_pairs.jsonl' \
V3_MODEL=v3_homophone BACKEND=cuda JOBS=60 \
data/build/v3/.venv/bin/python nlp/training/homophone/remote.py homophone new --min-free-gb 64
```

runner 可選用的 `EXTRA_SYNC`、`ENGINE_SNAPSHOT`、`V3_MODEL` 與 `RERANKER_MODEL` 不改變它的預設行為。具名的 v3 模型直接由 `context.pt`、`syllable.pt`、`chars.json`、`syllables.json`、`config.json` 匯出。CUDA 在整次呼叫中只服務一組檢查點：V3_MODEL 要與矩陣相符。CPU 支援混用版本。同一個遠端目錄樹中不要讓呼叫重疊。同音字結果要明確從該目錄樹的 `data/build/homophone/results/` 取回；runner 另外收集它的一般記錄檔。

`experiment.py baseline` 在兩個開發集、兩種前文條件下執行指定的三種組合，另外加上原始 v3 單獨在主要開發集上的診斷（14 列）。`new` 執行繼續訓練的 v3 單獨以及搭配 v2b 的組合（八列）。`diagnose.py --new` 檢驗剩下的主要錯誤是否多半由重排引入。只有在那種情況下才需要繼續訓練 v2b。`choose.py` 在所有開發集門檻通過後封存勝出的設定。不調整聊天先驗或句尾比例。

`final --final-role baseline` 與 `final --final-role chosen` 都需要同一個選擇檔與相符的 V3_MODEL。每個角色在兩個測試集上各跑有前文與無前文，smoke 只跑一次無前文。開始／完成封存檔防止重播；完整最終模式在任一角色開始後也拒絕重播。最終階段不支援續跑。

GPU 評測把 60 個 worker 分給 runner 的評分器與一個相同的額外評分器，讓每個伺服器都容得下 32 組前文的快取。自己擁有的額外評分器在 finally 中依 PID 終止並等待結束。`--min-free-gb 64` 在 worker 啟動前等待主機記憶體。主機忙碌時要讓工作依序執行：准入控制無法防止之後無關程序造成的 OOM。九項回歸檢查涵蓋讀音／排除、分割、對齊／指標、派送／清理、記憶體等待與最終重播防護。

`eval.py --model-only --backend torch --v3 DIR ...` 在遠端匯入未修改的參考 runner。前 64 個候選池會加入正確答案：這量的是條件式選擇，不是解碼器的召回率。CLI 混淆指標使用編輯距離對齊（刪除算錯）；整句正確率要求完全相同。類別優先順序固定為：詞庫固定詞、可辨識的主詞、句首、動詞前、地點／時間前、其他。類別是啟發式的；重疊的旗標與人工檢查的發現都保留。

## 最終產物

開發集選定後，`export_v3.py` 在 Mac 上重用上游匯出器，以 Xcode 編譯，並為原本的 20 個合成介面案例產生 golden 輸出。凍結 CLI 的一致性檢查已記錄在 `homophone/check-v3.log`。`check_onnx.py` 在遠端獨立比較 ONNX 與選定的 PyTorch 檢查點；`--coreml-vectors PATH` 則改為比較已儲存的 Mac 輸出與 PyTorch，記錄全部 20 個案例，未達參考容許誤差時以非零狀態結束。`latency.py` 在遠端以相同的合成形狀計時單執行緒 ONNX 呼叫，暖身 10 次、呼叫 100 次；共用主機的負載限制了結果的解讀。模型的聲明、授權與 `articles.tsv.gz` 出處要與產物放在一起。原始預訓練可能已包含新挖出的保留集；排除只對繼續訓練有保證。不包含 app 部署或介面驗證。

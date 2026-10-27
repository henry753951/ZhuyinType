# ZhuyinType v3 神經網路訓練

所有新程式碼都放在這裡，所有產出物都放在 `data/build/v3/`。既有的訓練檔案只以唯讀方式匯入。
實際結果與限制見 `REPORT.md`。這裡的內容不會安裝進 App。

## 既有環境

遠端：`ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST`，專案在 `~/zhuyintype`，Python 為 `.venv-gen/bin/python`（已安裝 Torch 2.13/CUDA、coremltools 9、NumPy、OpenCC、datasketch、pyahocorasick）。維基百科擷取會把既有的 `.venv/lib/python3.12/site-packages` 加到 Python 模組搜尋路徑的最後，以沿用 mwparserfromhell 0.7.2。兩個環境都不做任何變更。

Mac：用以下指令還原獨立、已鎖定版本的環境：

```sh
UV_PROJECT_ENVIRONMENT="$PWD/data/build/v3/.venv" uv sync --project nlp/training/v3 --python 3.12.14
```

這會把 Mac 上的版本固定為 Torch 2.7／coremltools 9。既有的專案 `.venv` 不受影響。

## 資料與工作

把目前的 `data/build/lexicon.tsv` 快照到遠端的 `data/build/v3/lexicon.tsv`。`common.py` 中列出的四個保留測試檔必須都存在於遠端，且雜湊值與 Mac 上一致。不得修改這些檔案，也不得用它們訓練。

在遠端根目錄啟動兩個彼此獨立的準備工作，各啟動一次：

```sh
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 .venv-gen/bin/python -u nlp/training/v3/prepare.py wiki > logs/v3-wiki.log 2>&1 < /dev/null &
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 .venv-gen/bin/python -u nlp/training/v3/prepare.py colloquial > logs/v3-colloquial.log 2>&1 < /dev/null &
nohup env PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 .venv-gen/bin/python -u nlp/training/v3/pipeline.py > logs/v3-pipeline.log 2>&1 < /dev/null &
```

用 `data/build/v3/status.json`、各階段的紀錄檔，以及 `pgrep -f '[p]ython -u .*nlp/training/v3/|[p]ython -u nlp/training/v3/'` 檢查程序狀態。不要重複啟動工作。流程會等兩個資料工作都完成，再依序執行 vocab → polyphone → encoding → leakage audit → self-check → context LM → syllable training → dev quality → export。任一階段失敗就停止整個佇列，不會自動重試。重新啟動單一階段前，先查明失敗原因。預設流程是全新執行：重新執行就會重新訓練。context 階段完成後，`pipeline.py --from-syllable` 會接續剩下的階段而不重新訓練 context；syllable 訓練則從頭開始。如果某一段 syllable 訓練在達到平穩前就用完步數上限，流程會自動以每段 10,000 步繼續，使用 `train.py syllable --resume --steps 10000 --batch 64`：新的最佳化器、新的取樣種子，初始學習率 6e-5。累計的歷史紀錄與時間會保留；要連續四次驗證沒有進步，才會進入後續的匯出。這是接續訓練，不是對失敗階段的自動重試。準備階段同樣是全新執行；變更擷取上限或輸入時，不要沿用舊的分片目錄。

XML 傾印檔依文件順序讀取。既有的 `lm/pipeline.py` 負責移除標記與 s2twp 轉換；4–200 字、漢字比例 60% 的句子篩選在每個原始段落內進行。被接受的句子保留原本的標點。文章結尾與被排除的段落會中斷上下文。`articles.tsv.gz` 保留原始的頁面、修訂版本與標題出處。擷取在第一個使保留段落數達到一千萬的批次後停止。

範本正規化會移除標點，替換數字（包括中文數字）與明確的人名欄位，再刪除正規化後完全相同的行。以 64 個排列的 MinHash LSH 找出 5-gram Jaccard >=0.8 的疑似重複，再做精確確認。家族的定義是正規化後的前六個字加最後四個字；每個家族上限 100。這是近似的範本偵測器，不保證能找出語意上的重複。

語料中的每個字元都視為不同的字，因此詞彙量可能超過預期的漢字數。ID 0–3 依規格保留。詞庫提供不含聲調的音節清單。標籤取自最長匹配的詞；有歧義的單字只在 p>=0.9 時採用上下文視窗分類器的結果。低信心的樣本仍保留給語言模型使用，但不納入 syllable 訓練。syllable 階段中 context LM 保持凍結。交叉注意力的 K/V 投影權重由 syllable 損失訓練，並與 context 模型一起匯出。

不在語料詞彙中的字元共用 UNK id 2；當某個讀音有未見過的詞庫候選字時，訓練會保留一個 UNK 桶。參考推論同樣把未見過的字元對應到 UNK。訓練使用完整的候選聯集，包括三個相近讀音的所有候選字與正確字，數量可能超過 64。依照需求，匯出的模型接受呼叫端選出、最多 64 個的子集。dev 數字衡量的是在保證含有正確答案的完整聯集中選字的表現，不是檢索或解碼器的準確率。填充用的記憶向量為零並被遮罩；空的上下文只有 BOS。

## Mac 上的收尾與推論

`data/build/v3/.venv/bin/python nlp/training/v3/finish.py` 會監看遠端、更新 REPORT.md、取回檢查點／詞彙表／套件，用 Mac 上固定版本的 Torch 重新匯出，用 Xcode 編譯，寫出 20 組向量，檢查一致性，並量測只用 CPU 的延遲。遠端無法連線或某階段失敗時，它會停止並記錄。避免同時啟動兩次。原始的有序語料與 mmap 訓練分片留在遠端；交付用的模型、詞彙表、檢查點與統計資料會複製到 Mac。

複製最終的檢查點與詞彙表後，手動收尾：

```sh
data/build/v3/.venv/bin/python nlp/training/v3/export.py
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile data/build/v3/ContextEncoder.mlpackage data/build/v3/
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun coremlcompiler compile data/build/v3/SyllableEncoder.mlpackage data/build/v3/
data/build/v3/.venv/bin/python nlp/training/v3/validate.py
```

參考輸入 JSON：

```json
{"context":"我們住在", "syllables":[["ㄊㄞ",2,0,0],["ㄨㄢ",1,0,0]], "candidates":[["臺","台"],["灣","彎"]]}
```

```sh
data/build/v3/.venv/bin/python nlp/training/v3/reference.py --input data/build/v3/example.json
# 或使用對應的 fp32 檢查點：
data/build/v3/.venv/bin/python nlp/training/v3/reference.py --backend torch --input data/build/v3/example.json
```

`config.json` 描述維度、名稱、ID 與介面限制。上下文文字只取最後 255 個字元，並在前面加上 BOS。段落換行對應到 SEP id 3，必須保留這些邊界。記憶輸出為 `[6,Lc,512]` float16，已經包含最後每一層 syllable 的 K/V 投影，不要再投影一次。context 與 syllable 套件必須來自同一個檢查點。

`test_vectors.json` 包含整數型態的模型輸入，以及 CPU Core ML 的精確 logprob。預期的記憶陣列是 base64 編碼、little-endian、連續的 float16 位元組，並附上形狀與 SHA256；請直接解碼到記憶張量。每個案例都附上完整的原文、輸入的讀音與候選字串。不要把記憶字串當成十進位陣列解讀。呼叫端可以拿 context 輸出與這些位元組比對，也可以把這些位元組直接餵入，單獨檢查 syllable 的一致性。fp16 執行與 fp32 PyTorch 並非逐位元相同；容許誤差與實測差異都有記錄。

模型與資料授權：與既有語料相同，為 CC BY-SA 4.0；程式碼為 AGPL-3.0-only。再散布訓練產出物時，請保留既有的來源聲明與維基百科頁面／修訂版本出處清單。

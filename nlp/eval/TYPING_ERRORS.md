# 依時間的打字錯誤評測

這套評測在開發集上評估送出的文字，以及純引擎的 recall@16。它不寫入任何引擎或模型檔案。請在儲存庫根目錄執行。

## 重現

因為引擎同時在修改，**每次完整執行前**都要重新建置：

```sh
mkdir -p .build-eval
uv run --no-project python nlp/eval/typing_errors.py --self-test
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-eval --product zhuyin-cli > .build-eval/typing-errors-build.log 2>&1 && \
env -u ZHUYIN_UNITS uv run --no-project python nlp/eval/typing_errors.py \
  -n 300 --seed 7 \
  --config current '' \
  --config current+reranker '--reranker data/build/reranker --rerank-weight 1 --rerank-count 16' \
  --examples 2 --json .build-eval/typing-errors.json \
  > .build-eval/typing-errors-table.md 2> .build-eval/typing-errors.log
```

`--test` 接受語料路徑；`--self-test` 執行內建的斷言，不需要資料或 CLI。`-n` 預設為每個資料集 300，**在適用性篩選之前計算**。設定接受以 shell 引號包住的 CLI 參數，例如 `--config timed '--decoder timed'`。目前的建置提供 `slots|boundaries|reorder|timed`；兩組實測的設定都使用它的預設值 `slots`。評測程式中沒有寫死任何解碼器名稱。

上面的完整執行指令對 `--test` 使用兩個預設的 dev 路徑，並把輸出導向暫存目錄。Core ML 使用 CLI 的預設值，只用 CPU。試跑使用 `-n 3` 與 `-n 1`；另一次 30 次執行的計時探測發現，在這台 Mac 上 CPU 比 `ZHUYIN_UNITS=all` 或 `ane` 快。這些試跑不計入下方結果。

## 模型與計數規則

- 以與 `conversion_eval.py` 相同的 `[\u4e00-\u9fff]{4,20}` 比對、詞庫最長比對讀音，以及一聲以 Space 輸入的慣例，挑出可讀的漢字段；每個參考字必須恰有一個讀音。以 seed 7 打亂順序。每個條件使用同一批選出的段落，並濾除不適用者。讀音是推斷出來的，不是人工標註的發音真值。
- 使用美式 Mac 鍵盤的按鍵中心點，各列偏移 0、0.25、0.5、1 個鍵寬，採用標準指法的左右手與手指分配，Space 由右手拇指按。每個按鍵都加上 `<dt:MS>` 前綴。按鍵間隔服從對數常態分布，對數標準差 0.35，基準中位數 120 ms（快速為 60 ms）；左右手交替時乘以 0.75，同一手指時乘以 1.4。exact 與 exact-fast 共用相同的隨機計時抽樣，以 2 倍縮放。
- 交換（swap）對調兩個相鄰且不同的按鍵，包括聲調鍵與一聲的 Space。`swap-in-syllable` 限於同一音節內；`swap-across` 跨越音節邊界。rollover 從這兩種中抽樣，並把被對調的這對按鍵之間的間隔設為 0–15 ms 均勻分布。一般交換保留一般抽樣的計時。
- hotspot 以目標按鍵為中心的高斯分布抽樣落點（水平 sigma 0.45，垂直 sigma 0.30），對齊到最近的實體按鍵中心，並捨棄未改變或非相鄰的落點（距離超過 1.6 個鍵寬）。實體標點鍵仍可能被誤觸。寬的 Space 不作為 hotspot／extra-key 的錨點；它仍可能被交換或漏打。extra-key 在錨點之前或之後插入一個抽樣的相鄰鍵，該對按鍵間隔為 0–20 ms 均勻分布。
- 條件 3–9 在每個適用的段落中恰好注入一個錯誤。`swap-across-2`、`rollover-2` 與 `hotspot-2` 在互不重疊的位置注入兩個錯誤。missing-key 漏掉一個按鍵；tone 只對調 3↔4 或 6↔7。
- nospace 只省略**後面還有其他音節**的標準一聲 Space；最後的 Space 保留。`nospace-all` 移除所有符合條件的 Space；`nospace-30` 以 30% 機率對每個 Space 獨立抽樣。只有實際發生省略的段落才計入，因此這一列是以觀察到錯誤為條件。
- mixed 在每個音節最多選一種錯誤：3% hotspot、2% rollover、1% extra、1% missing、1% tone，其餘不加錯誤。不適用的錯誤直接略過，不以其他類型替代。它另外獨立省略 30% 符合條件的一聲 Space，並在 30% 的段落選用快速設定。mixed 包含沒有任何改變的段落；JSON 記錄每個段落實際的錯誤數與設定。
- 固定的每段落／每條件 RNG 讓各設定使用相同的按鍵腳本。兩個資料集在每個條件／設定下共用一個批次。最多同時有三個 CLI 行程；批次模式在每一行輸入時重設上下文與學習。送出用的腳本以 Enter 結尾；recall 用的腳本不加。recall 會去除 reranker 選項，並在等價的設定之間重用同一份純引擎結果。
- 整段準確率要求送出的文字完全相同。字元準確率為 `max(0, 1 - total Levenshtein distance / reference characters)`，沿用 `typo_eval.py`。Recall@16 要求完整的參考答案出現在引擎清單中。誤修正指輸出與**該設定在正常、標準輸入下的 exact 輸出**不同，不論基準本身是否正確；因此 exact 依定義為 0%，exact-fast 衡量的是速度造成的改變。這與 typo_eval 中以正確性為條件的 off-mode 指標不同。

JSON 包含每一個輸入腳本、輸出、N-best 清單、指標、產出物雜湊值與實際耗時；每完成一列結果就寫入一次檢查點。沒有樣本的條件回報 N=0 與未定義的指標，不計入分母。

## 結果

量測於 2026-10-27（Asia/Taipei），開始時間 2026-10-26T19:34:17.982916+00:00。實際耗時：**24 min 33 s**（1472.6 s），包含兩組設定、所有條件、語料載入與雜湊計算。量測期間沒有任何追蹤中的產出物改變（量測前後檢查了 26 個 CLI／語料／詞庫／模型檔案）。

- CLI SHA-256：`170b82f6ad951185d6c494f83fe968eec92771c290460af82ca498420011aa2b`。
- 詞庫 SHA-256：`9e651e711661ae942a3bc4c4cd7c9422ec3794e7b828a82095c566ecc2f3ae60`。
- Python 3.15.0，透過 uv 0.12.7；預設只用 CPU 的 Core ML。
- 完整記錄：`.build-eval/typing-errors.json`（暫存產出物）。
- 自我檢查、小型批次試跑，以及實際的 `--decoder slots` / `timed` 參數傳遞檢查都通過。全部 60 列指標都從記錄的輸出獨立重新計算過。全部 8,535 個不同的條件／段落輸入都由 seed 重新產生；成對設定的腳本與引擎 N-best 都相符。

Whole 與 Char 描述送出的輸出；Recall@16 一律不使用 reranker。破折號表示該條件不計算此指標。

| 資料集 | 條件 | 設定 | N | Whole | Char | Recall@16 | 誤修正 |
|---|---|---|---:|---:|---:|---:|---:|
| colloquial | exact | current | 300 | 75.7% | 95.3% | 95.7% | 0.0% |
| colloquial | exact | current+reranker | 300 | 84.7% | 97.4% | 95.7% | 0.0% |
| colloquial | exact-fast | current | 300 | 75.7% | 95.3% | 95.7% | 0.0% |
| colloquial | exact-fast | current+reranker | 300 | 84.7% | 97.4% | 95.7% | 0.0% |
| colloquial | swap-in-syllable | current | 300 | 47.0% | 87.4% | 62.7% | — |
| colloquial | swap-in-syllable | current+reranker | 300 | 54.0% | 89.6% | 62.7% | — |
| colloquial | swap-across | current | 300 | 0.3% | 70.3% | 1.7% | — |
| colloquial | swap-across | current+reranker | 300 | 1.3% | 72.1% | 1.7% | — |
| colloquial | swap-across-2 | current | 300 | 0.0% | 50.8% | 0.3% | — |
| colloquial | swap-across-2 | current+reranker | 300 | 0.3% | 52.2% | 0.3% | — |
| colloquial | rollover | current | 300 | 29.7% | 83.2% | 45.0% | — |
| colloquial | rollover | current+reranker | 300 | 35.3% | 85.5% | 45.0% | — |
| colloquial | rollover-2 | current | 300 | 14.3% | 71.0% | 20.0% | — |
| colloquial | rollover-2 | current+reranker | 300 | 18.0% | 73.6% | 20.0% | — |
| colloquial | hotspot | current | 300 | 40.3% | 86.2% | 60.3% | — |
| colloquial | hotspot | current+reranker | 300 | 48.7% | 88.7% | 60.3% | — |
| colloquial | hotspot-2 | current | 300 | 16.3% | 75.5% | 27.7% | — |
| colloquial | hotspot-2 | current+reranker | 300 | 22.0% | 78.4% | 27.7% | — |
| colloquial | extra-key | current | 300 | 59.3% | 89.7% | 78.7% | — |
| colloquial | extra-key | current+reranker | 300 | 68.0% | 92.2% | 78.7% | — |
| colloquial | missing-key | current | 300 | 6.3% | 78.3% | 9.3% | — |
| colloquial | missing-key | current+reranker | 300 | 8.0% | 80.2% | 9.3% | — |
| colloquial | tone | current | 300 | 66.7% | 93.4% | 89.0% | — |
| colloquial | tone | current+reranker | 300 | 75.0% | 95.7% | 89.0% | — |
| colloquial | nospace-all | current | 243 | 0.0% | 60.0% | 0.0% | — |
| colloquial | nospace-all | current+reranker | 243 | 0.0% | 61.8% | 0.0% | — |
| colloquial | nospace-30 | current | 108 | 0.0% | 71.0% | 0.0% | — |
| colloquial | nospace-30 | current+reranker | 108 | 0.0% | 74.6% | 0.0% | — |
| colloquial | mixed | current | 300 | 34.7% | 79.5% | 48.3% | — |
| colloquial | mixed | current+reranker | 300 | 42.7% | 82.1% | 48.3% | — |
| wikipedia | exact | current | 300 | 71.7% | 95.7% | 94.0% | 0.0% |
| wikipedia | exact | current+reranker | 300 | 81.0% | 97.2% | 94.0% | 0.0% |
| wikipedia | exact-fast | current | 300 | 71.7% | 95.7% | 94.0% | 0.0% |
| wikipedia | exact-fast | current+reranker | 300 | 81.0% | 97.2% | 94.0% | 0.0% |
| wikipedia | swap-in-syllable | current | 300 | 48.0% | 90.0% | 62.0% | — |
| wikipedia | swap-in-syllable | current+reranker | 300 | 55.0% | 91.6% | 62.0% | — |
| wikipedia | swap-across | current | 300 | 0.0% | 75.4% | 0.0% | — |
| wikipedia | swap-across | current+reranker | 300 | 0.0% | 77.4% | 0.0% | — |
| wikipedia | swap-across-2 | current | 300 | 0.0% | 60.0% | 0.0% | — |
| wikipedia | swap-across-2 | current+reranker | 300 | 0.0% | 61.0% | 0.0% | — |
| wikipedia | rollover | current | 300 | 35.7% | 86.3% | 47.7% | — |
| wikipedia | rollover | current+reranker | 300 | 40.3% | 87.7% | 47.7% | — |
| wikipedia | rollover-2 | current | 300 | 13.3% | 76.9% | 19.3% | — |
| wikipedia | rollover-2 | current+reranker | 300 | 15.7% | 78.2% | 19.3% | — |
| wikipedia | hotspot | current | 300 | 42.0% | 88.9% | 60.7% | — |
| wikipedia | hotspot | current+reranker | 300 | 50.7% | 90.7% | 60.7% | — |
| wikipedia | hotspot-2 | current | 300 | 19.0% | 80.6% | 31.3% | — |
| wikipedia | hotspot-2 | current+reranker | 300 | 28.7% | 83.2% | 31.3% | — |
| wikipedia | extra-key | current | 300 | 58.0% | 92.1% | 78.0% | — |
| wikipedia | extra-key | current+reranker | 300 | 65.7% | 93.6% | 78.0% | — |
| wikipedia | missing-key | current | 300 | 6.7% | 82.6% | 10.0% | — |
| wikipedia | missing-key | current+reranker | 300 | 8.3% | 84.5% | 10.0% | — |
| wikipedia | tone | current | 300 | 65.0% | 94.7% | 91.3% | — |
| wikipedia | tone | current+reranker | 300 | 75.7% | 96.4% | 91.3% | — |
| wikipedia | nospace-all | current | 255 | 0.0% | 62.5% | 0.0% | — |
| wikipedia | nospace-all | current+reranker | 255 | 0.0% | 64.3% | 0.0% | — |
| wikipedia | nospace-30 | current | 129 | 0.0% | 79.2% | 0.0% | — |
| wikipedia | nospace-30 | current+reranker | 129 | 0.0% | 80.4% | 0.0% | — |
| wikipedia | mixed | current | 300 | 26.3% | 79.5% | 34.7% | — |
| wikipedia | mixed | current+reranker | 300 | 29.7% | 81.9% | 34.7% | — |

reranker 把乾淨輸入的整段準確率在口語上從 75.7% 提升到 84.7%，在 Wikipedia 上從 71.7% 提升到 81.0%。exact-fast 在兩組設定下都重現了 exact 的輸出：在這些樣本上，速度造成的誤修正為 0%。exact 的 0% 依定義是同一設定的基準，不代表轉換完全正確。

主要的失敗來自音節邊界：一次跨音節交換的 recall 只有 1.7% / 0%；漏打一個按鍵為 9.3% / 10.0%。兩種省略 Space 的條件，在實際發生省略的段落中，整段準確率與 recall 都是 0%。只提升 reranker 的品質無法找回這些不在清單中的候選。相對地，聲調錯誤保有 89.0% / 91.3% 的 recall，加上 reranker 後的準確率為 75.0% / 75.7%。

兩個 hotspot 使加上 reranker 的整段準確率從 48.7% / 50.7%（一個）降到 22.0% / 28.7%（兩個）。mixed 加上 reranker 為 42.7% / 29.7%，不加為 34.7% / 26.3%；實際抽到快速設定的段落數為 94/300 與 75/300。30% 的快速比例是抽樣結果，不是配額。rollover 與一般交換條件的位置各自獨立抽樣；兩者的差異不是只改變計時的成對消融比較。要做這種比較，請在同一條件內比較不同設定。

這些是開發集上的合成錯誤結果，讀音由詞庫推斷，不是依真人打字校準，也不是保留集上的驗收測試。即使之後同時修改中的 Swift 原始碼有所變動，二進位檔的雜湊值仍可識別這份快照。


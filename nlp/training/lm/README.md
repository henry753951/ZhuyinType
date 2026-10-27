# 重現台灣正體中文詞語言模型

這個目錄的訓練程式碼採 AGPL-3.0-only 授權。所有產生的模型、重新校準的詞庫與測試句皆為 **CC BY-SA 4.0**，
因為它們衍生自中文維基百科與專案中採 CC BY-SA 授權的種子詞庫。重新散布模型產物時，請一併保留
產生的 `NOTICE.md` 與 `articles.tsv.gz`。

只有這個目錄與被忽略的 `data/build/lm/` 目錄是本機輸出。原始詞庫、Swift 原始碼、資料工具與專案
文件都不會被修改。這個流程產生所需的 TSV 訓練產物；App 整合與 `docs/MODEL.md` 描述的量化 mmap
二進位檔是另外的工作。

## Linux 環境設定與執行

需求：Linux、`curl`、GNU `sort`，約 50 GB 可用磁碟空間，以及 64 個 worker 至少需要 64 GB RAM。
正式執行使用 CPU，不使用 GPU。Python 3.12 與所有相依套件都隔離在 uv 管理的虛擬環境中。
不需要變更任何系統套件或服務。

```sh
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST 'mkdir -p ~/zhuyintype/training/lm ~/zhuyintype/data/build ~/zhuyintype/logs'
rsync -a -e "ssh -i $ZHUYIN_REMOTE_KEY" nlp/training/lm/ \
  $ZHUYIN_REMOTE_HOST:zhuyintype/training/lm/
scp -i $ZHUYIN_REMOTE_KEY data/build/lexicon.tsv \
  $ZHUYIN_REMOTE_HOST:zhuyintype/data/build/lexicon.tsv
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST

cd ~/zhuyintype
# 只在沒有 uv 時執行：
curl -LsSf https://astral.sh/uv/install.sh | sh
~/.local/bin/uv python install 3.12.15
~/.local/bin/uv venv --python 3.12.15 .venv
~/.local/bin/uv pip install --python .venv/bin/python -r nlp/training/lm/requirements.txt
nohup bash nlp/training/lm/run.sh > logs/pipeline.log 2>&1 < /dev/null &
echo $!
```

以簡短的 SSH 指令輪詢進度，不需要保持 SSH 連線：

```sh
ssh -i $ZHUYIN_REMOTE_KEY $ZHUYIN_REMOTE_HOST \
  'tail -8 ~/zhuyintype/logs/pipeline.log; ps -u <remote-user> -o pid,etime,pcpu,rss,args --sort=-pcpu | head -8'
```

預設為 `WORKERS=64`。`LM_PYTHON=/path/to/venv/bin/python` 可覆寫直譯器。BLAS／OpenMP 限制為每個
worker 一個執行緒。擷取、句子分片、三輪斷詞與完整的分離稽核最多使用 64 個程序。GNU sort 也最多
使用 64 個執行緒；合併與最終平滑是在單一協調程序中以向量化的 NumPy 運算完成。

`stats.json` 回報 `"status": "validated"` 之後，把輸出複製回來：

```sh
mkdir -p data/build/lm
rsync -av -e "ssh -i $ZHUYIN_REMOTE_KEY" \
  $ZHUYIN_REMOTE_HOST:zhuyintype/data/build/lm/ data/build/lm/
```

`run.sh` 先執行回歸測試，接著驗證或下載 dump、訓練，再獨立驗證序列化後的檔案。任何一步失敗就結束。
完成的階段會在遠端 `work/` 留下標記；重新執行時會從這些標記繼續。輸入雜湊與參數必須與儲存的設定一致。
若清理或斷詞程式碼有變更、輸入不同或設定不同，請使用**新的工作目錄**。演算法變更後絕不重用舊的
檢查點。匯出成功時會覆寫輸出；原始輸入永遠不會被改寫。

個別指令，適合用來重現某個固定的歷史版本：

```sh
.venv/bin/python nlp/training/lm/pipeline.py \
  --dump raw/zhwiki-latest-pages-articles.xml.bz2 \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm \
  --workers 64 --holdout 5000 --seed 20261026 --max-bigrams 3000000
.venv/bin/python nlp/training/lm/verify.py \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm --workers 64
```

要精確重現歷史執行，請下載**帶日期的 URL**，並以交付的 `stats.json` 中的雜湊驗證，不要下載會變動的
`latest` 別名。將該來源的收據存為 `raw/download.json`；`pipeline.py` 需要這個檔案。
相依套件版本固定在 `requirements.txt`；執行環境與套件版本、輸入雜湊、設定與來源收據也記錄在 stats 中。

## 語料、授權與出處標示

必要來源：

- 中文維基百科 `pages-articles.xml.bz2`，namespace 0，非重新導向的條目。
  最新版 URL：<https://dumps.wikimedia.org/zhwiki/latest/zhwiki-latest-pages-articles.xml.bz2>。
  [Wikimedia 的再利用條款](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content)
  規定文字採 CC BY-SA 4.0。輸出授權：
  <https://creativecommons.org/licenses/by-sa/4.0/>。
- `download.py` 記錄由 RSS 取得的帶日期快照 URL、dump 日期、下載日期、HTTP Last-Modified、位元組大小、
  Wikimedia 公布的 SHA1、實際 SHA1 與 SHA256。checksum 不符時拒絕，包括別名在下載期間變動的情況。
- `articles.tsv.gz` 列出每個在去重與 holdout 之前提供了乾淨句子的條目的 page ID、revision ID 與標題。
  其中刻意包含部分句子後來被移除的條目。`NOTICE.md` 說明用於出處標示的確切版本與作者歷史 URL。
- 種子詞庫：`data/build/lexicon.tsv`，CC BY-SA 4.0，來自
  [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict)、
  [Unicode Unihan](https://www.unicode.org/charts/unihan.html) 與
  [wordfreq](https://github.com/rspeer/wordfreq)；見未修改的 `data/SOURCES.md`。

選用的網頁語料不納入，讓這次執行的規模有限、來源明確。不使用字幕、論壇、合成對話、受限的研究語料，
也不使用 `references/` 中的程式碼。

## 擷取、清理與防止資料洩漏

1. 以串流方式讀取 bzip2 XML。用 `mwparserfromhell` 分批解析 wikitext，批次大小有上限。
   略過模板、參考資料、表格、code／math／gallery 標籤、媒體與跨語言連結、外部連結、標題，
   以及結尾的參考資料與外部連結段落。模板不展開；這是純文字語料，不是 MediaWiki 的渲染結果。
2. 將 `-{zh-tw:...;zh-cn:...}-` 變體解析為 `zh-tw`／`zh-hant`，以 OpenCC **s2twp** 轉換，並將 Unicode
   正規化為 NFC。轉換結果維持 OpenCC 的原始輸出；範例存於 `conversion_spotchecks.json`。
   特別是它通常寫成**臺灣**。詞庫同時有台灣與臺灣，兩者既有的拼寫與列保持分開。
3. 以 `。！？；\n` 分句，合併句內空白，保留 **4–200 個字元**且漢字至少占 **60%** 的句子。
   排除殘留的 URL、標記、表格分隔符號與明顯的程式碼。本任務明確指定的長度與完全去重規則，
   優先於 MODEL.md 中較短的長度與 MinHash 建議。
4. 以位元組排序（`LC_ALL=C`）的 GNU sort 進行全域的完全句子去重。
5. 在任何斷詞或語料頻率計數之前，以均勻 reservoir sampling 抽出 **5,000 個不重複句子**，seed 為
   **20261026**。存為 `test_sentences.txt`。
6. 移除這些句子，以及與任一測試句共有**任何連續 8 字元子字串**的訓練句。完整掃描以 Aho-Corasick
   執行。較短的測試句仍以完全相等的方式移除。

測試集不用於選擇折扣、剪枝或斷詞參數。文件數指的是 namespace 0 的輸入文件，以及在去重之前提供了
乾淨句子的文件；句數與字元數的訓練總量是 holdout 與重疊移除之後的數字。一個字元指一個 Unicode
code point。

## 斷詞與 modified Kneser–Ney

讀取所有 `(reading, word, logp)` 列，並對每個詞把各讀音的 `exp(logp)` 加總。將這些質量正規化後，
對詞 trie 執行精確的 unigram Viterbi。每個不在詞庫中的字元都是單字元 token，包括通過過濾的標點、
拉丁字母、數字與空白。

先執行以詞庫加權的初始斷詞，再做**兩次 M／E 更新**，共三輪斷詞。每次更新使用實際的 token 計數，
並對每個詞彙項目加上 0.1 的 pseudocount，讓未出現的詞仍可被選到。最後一次 E-step 的斷詞器在
測試評估時固定不變。只有最後一輪計算詞 bigram，每句各加一個 `<s>` 與 `</s>`；bigram 不跨越句子邊界。

對所有觀察到的詞對類型，計算 count-of-counts `n1..n4` 與 Chen–Goodman 折扣：

```text
Y = n1 / (n1 + 2*n2)
Dr = r - (r+1)*Y*n[r+1]/n[r]      for r=1,2,3; D3 applies to counts >=3
Pcont(w) ∝ number of distinct preceding tokens of w
lambda(h) = sum_w D(count(h,w)) / count(h)
PMKN(w|h) = (count(h,w)-D(count(h,w)))/count(h) + lambda(h)*Pcont(w)
```

缺少的 count-of-count 區間使用折扣 0.75；數值上限讓折扣嚴格介於 0 與其計數類別之間。unigram 是 KN
continuation 分布。每個 continuation 為零的詞得到 0.1 個 pseudo-continuation，`<unk>` 至少得到 1 個，
之後重新正規化。`<s>` 只作為上下文，永遠不作為 unigram 預測；`</s>` 則包含在預測目標中。
觀察到的其他 fallback 字元與所有詞庫詞一起匯出。

使用最小的全域計數門檻（**>=2**），使詞對數量不超過 **3,000,000**。保留的詞對維持其內插後的 MKN 機率。
對保留的後繼詞 `K(h)`，在剪枝後重新計算匯出的 backoff：

```text
alpha(h) = (1 - sum_{w in K(h)} PMKN(w|h)) / (1 - sum_{w in K(h)} Pcont(w))
```

查詢方式完全如下：

```text
logP(w|h) = bigram[h,w]                       if stored
            backoff[h] + unigram[w]          otherwise
```

不在 `backoff.tsv` 中的上下文，log-backoff 使用 **0**。不在 `unigram.tsv` 中的目標詞，以 `<unk>` 代替。
這是剪枝後的內插 MKN 模型；刪除詞對後，重新分配的質量經過明確正規化，而不是沿用原本的內插係數。

`lexicon_tw.tsv` 完整保留種子詞庫的 `(reading,word)` 列與列順序。
對語料中出現過的詞，將該詞匯出的 KN unigram 質量依種子詞庫的讀音機率比例分配。因此詞內各個破音讀法
之間的比值不變。未出現的詞保留原本的列 logp，再加上共用的尺度位移：對語料中出現過的詞庫詞取
`new_word_logp - old_word_logp` 的中位數。這是刻意為未出現的詞所做的例外，因此詞庫所有列的質量總和
不必等於一；LM 的 unigram 與條件分布則都有正規化。

## 輸出與驗證

| 檔案 | 內容 |
| --- | --- |
| `unigram.tsv` | `word<TAB>natural-log probability`；所有詞庫詞、fallback 字元、`</s>`、`<unk>` |
| `bigram.tsv` | `w1<TAB>w2<TAB>natural-log probability`；剪枝後的內插 MKN |
| `backoff.tsv` | `w1<TAB>natural-log backoff`；恰為已儲存的 bigram 上下文 |
| `lexicon_tw.tsv` | 原始的檔頭格式與完全相同的讀音／詞列，logp 經重新校準 |
| `test_sentences.txt` | 固定的 5,000 句測試集 |
| `stats.json` | 計數、覆蓋率／OOV、perplexity、來源收據／授權、大小／雜湊、時間、執行環境、決策 |
| `validation.json` | 匯出文字檔的驗證結果與完整的洩漏掃描 |
| `examples.json` | 我、今天、台灣、的，以及臺灣的確切前五名條件預測；標示結果來自已儲存的 bigram 或 backoff |
| `conversion_spotchecks.json` | 實際的 `s2twp` 轉換範例 |
| `articles.tsv.gz`、`NOTICE.md` | 來源出處標示與重新散布聲明 |

所有 TSV 機率都使用自然對數，取到小數點後 12 位。`stats.json.files` 記錄 stats 本身以外各交付檔案的
位元組數與 SHA256。評估時 unigram 與 bigram 使用相同的固定詞 token，EOS 計入 perplexity 的分母，
BOS 不計入。詞庫 OOV 包括 fallback 標點與其他非漢字 token；`test_model_unk_rate` 量測不在訓練後模型
詞彙表中的字元。`test_unseen_training_token_rate` 另外計入在訓練中從未被選到的詞庫詞。

回歸檢查涵蓋：以窮舉比對 Viterbi、擷取與台灣用字轉換、以手算驗證 MKN 內插、剪枝後的正規化、
holdout 排除、讀音比例、未出現詞的校準，以及 JSON 序列化。端對端的冒煙測試可使用較小的 dump、
`--holdout 100 --workers 4` 與同一個驗證工具。最終驗證會重新載入 TSV、檢查每個條件分布、對 24 個
上下文明確列舉目標詞彙、驗證所有詞庫列、讀音比例與校準、檢查產物雜湊，並重新掃描**所有**訓練句，
確認沒有與 holdout 相等或 8 字元重疊的句子。

轉換為台灣用字的維基百科仍是百科式、來源混雜的語言：這些計數不能代表台灣日常對話的用法。
維基百科 holdout 的 perplexity 不能推論出打字準確率、macOS 整合或 mmap 大小。

## 混合語料 trigram（`lm2`）

後續版本的流程是 `run2.sh`／`train2.py`；它不更動舊流程及其固定的維基百科測試集。固定版本的 CC0
來源、確切的 Apache-2.0 生成器檢查點、清理規則，以及停止與續跑的指令見
[../corpus/README.md](../corpus/README.md)。在既有的 Linux 主機上，從 `~/zhuyintype` 執行：

```sh
bash nlp/training/lm/build_kenlm.sh  # 只在 work2/kenlm/build/bin/lmplz 不存在時執行
nohup bash nlp/training/lm/run2.sh > logs/pipeline2.log 2>&1 < /dev/null &
```

這個腳本會等待固定的 1.5M 合成階段完成，只挖掘讀音確定的詞庫新增詞，從兩個來源中排除所有 dev／test
8-gram，並重用 `pipeline.py` 完全相同的詞庫 Viterbi 斷詞器。初始斷詞加上兩次 hard-EM 更新，
維基百科與口語語料**各使用一次**。之後這個斷詞器在所有候選 LM 與所有評測集上都固定不變。

若要在第一階段完成前開始訓練，先把目前的合成檔複製到暫存檔，截掉最後一行未完成的內容，再以原子方式
重新命名為固定的快照。生成與清理繼續執行。明確傳入該快照：

```sh
bash nlp/training/lm/run2.sh data/build/corpus/synthetic_snapshot1.txt
```

這樣會略過 `PHASE1_READY` 的等待。挖掘與訓練都接受相同的 `--synthetic PATH`（相對於專案根目錄）；
預設值不變。稽核與報告從 `stats.json` 讀取所選的輸入，驗證其雜湊與完整句數，而不要求剛好 1.5M 行。
訓練設定包含快照與 Common Voice 輸入的雜湊；輸入變更時仍需要新的訓練檢查點。在固定的 holdout 與
前處理程式碼不變時，可以重用已準備好的維基百科分片。

KenLM（固定 commit `4cb443e60b7bf2c0ddf3c745378f76cb59e254e5`）估計 3 階的 modified Kneser–Ney。
候選模型將口語語料重複 **k=1,4,12** 次，維基百科維持一次。明確加上 `--discount_fallback`，因為重複
語料可能使 count-of-count 區間為空。選擇**以 token 加權的聯集 dev perplexity** 最小者，包括 EOS、
不含 BOS；各來源的 dev 分數也一併保留。兩個測試集都不用於選擇模型參數。unigram、bigram 與 trigram 的
測試 perplexity 使用所選模型完全相同的固定詞 token；它們與前一版使用不同斷詞器得到的分數不能直接比較。

計數剪枝為 `--prune 0 8 12`（KenLM 移除計數 <= 門檻者），必要時再加上全域的明確 trigram 對數機率上限。
這個上限計入每個 bigram 的 trigram 指標，目標是 <148 十進位 MB，並保留餘裕。`lmbin.py` 在剪枝後重新
計算低階與 trigram 的 backoff，讓被刪除的機率回到 fallback 分布。Float32 分數為自然對數，規格見
**未修改的** [FORMAT.md](FORMAT.md)。

舊的 `prefer_tai.py` 在估計之後提高機率質量。新流程改為對**90% 的訓練句**做臺→台對應，且只在台字詞
存在於擴充詞庫時才對應，其餘 10% 保持原文。這樣兩種寫法都保留，也不破壞正規化。holdout 的寫法不變。
實際訓練出的偏好請查看模型報告。

FORMAT.md 同時要求 UTF-8 位元組排序與保留 ID 0–2。fallback 斷詞產生的 ASCII 標點與空白會排在這些保留
拼寫之前。這類 token 在訓練與評估時對應為 **U+FFFD**；中文詞彙不變，原始語料文字也保留。如此三個特殊
拼寫就確實排在最前面；寫入工具會拒絕不相容的 ARPA，而不是默默違反格式。

交付的 `data/build/lm2/lexicon.tsv` 保留種子詞庫的 `(reading,word)` 順序與破音字比例，再附加挖掘到的
條目。出現過的條目使用模型的 unigram 質量；未出現的條目保留種子詞庫的尺度。`unknown_word_shift` 是
在出現過的共有詞上，模型減種子詞庫的詞 logp 的中位數，讓引擎可以校準未出現詞庫條目的 fallback。
它只在引擎的未知詞路徑上加**一次**。不要再對未出現的詞庫列做位移。

`lm2` 包含 `lm.bin`、`lm.json`、重新校準的 `lexicon.tsv`、`new_words.tsv`、`stats.json`、`NOTICE.md`、
條目出處標示、生成器 LICENSE，以及驗證與抽樣收據。`stats.json` 回報候選模型的 dev perplexity、所選模型的
測試 perplexity 與 OOV、剪枝後大小、語料計數、時間與雜湊。這個 Python 流程不對 Swift 的轉換準確率
做任何宣稱。

寫入／驗證工具與小型回歸測試：

```sh
.venv/bin/python nlp/training/lm/lmbin.py write model.arpa lm.bin
.venv/bin/python nlp/training/lm/lmbin.py verify lm.bin --samples 256
.venv/bin/python nlp/training/lm/test_lmbin.py
```

驗證工具檢查檔頭、計數與長度、區段對齊與零補齊、UTF-8 詞彙排序與保留 ID、每個指標與排序後的後繼詞組、
有限的對數機率、空上下文的 backoff、unigram 正規化、抽樣的 bigram／trigram 條件正規化，以及純量與向量
計算的 backoff 是否一致。寫入時使用已 flush 的暫存檔，驗證後再以原子方式重新命名。
它不執行 Swift 讀取端，也不量測輸入法的轉換準確率。

# 維基百科詞 bigram：方法與紀錄

0.3.0 起出貨的百科語言模型（`data/build/lm/`）怎麼建出來：來源、清理、防止資料洩漏、斷詞、平滑與驗證。
指令與檔案見 [README.md](README.md)。

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
  [wordfreq](https://github.com/rspeer/wordfreq)；見 `resources/SOURCES.md`。

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

## 臺／台寫法

`s2twp` 把語料幾乎都寫成「臺」（臺灣 349,339 次，台灣 64 次）。`verify` 之後，`prefer_tai.py` 把每個含「臺」、
且「台」寫法也在詞庫裡的詞的機率合併給「台」寫法，「臺」寫法保留原本的機率再減 1.5（自然對數），兩種寫法都
打得出來，「台」排在前面。這一步直接改寫三個 TSV，所以 `stats.json` 記錄的雜湊是改寫之前的。出貨的
`lm/` 經過這一步（2026-10-26）。

## 沒有採用的做法

- **混合語料三元模型 `lm2`**：維基百科加上口語語料（Common Voice 與 Qwen3-8B 產生的句子快照
  `corpus/synthetic_snapshot1.txt`），挖掘新詞（`corpus/mine_words.py`），以 KenLM 估計 3 階 MKN，
  依開發集困惑度選口語重複次數，寫成 ZTLM/1 三元 `lm.bin`（`lmbin.py`）。開發集較好、測試集較差，
  決定維持二元模型（`docs/research/log.md`「快照三元模型」、`docs/engine/MODEL.md` §11）。產物與報告在
  `data/build/lm2/`（`REPORT.md`、`EXPERIMENT.md`、`stats.json`）。訓練、稽核與報告程式
  （`train2.py`、`audit2.py`、`report2.py`、`run2.sh`、`build_kenlm.sh`、`corpus/mine_words.py`）已移除，
  見 commit 1341eb7。
- **混入詞庫的一般詞頻**：`P(w|h) = λ·P_corpus(w|h) + (1−λ)·P_general(w)`（`mix_general.py`，λ = 0.3），
  讓常用詞依詞頻排序。沒有用在出貨的模型；後來改以聊天模型混合解決百科偏向（`docs/engine/MODEL.md` §5.3）。
  程式已移除，見 commit 1341eb7。

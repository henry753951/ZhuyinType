# 善解（Shanjie）：研究紀錄閱讀筆記

感謝善解的維護者 Nanako0129 公開詳細的研究紀錄，包括負面結果、契約文件與評測資料。很少有專案把過程記錄得這麼公開，下面規劃的許多項目都來自這份紀錄。以下所有數字都是善解的，在它自己的評測集、設定與硬體上測得，不能直接與 ZhuyinType 的數字比較。沒有複製任何程式碼。善解採用 Apache-2.0；我們閱讀了它的文件與程式碼，並只在各檔案授權允許的範圍內使用它的評測集（見其 `LICENSES/data.md`），且註明出處。提到 0c（靈犀注音，AGPL-3.0）的地方，數字是善解對 0c 的量測；0c 只為量測而建置與執行，閱讀範圍僅限其文件。

**閱讀的資料來源。** 位於 `references/shanjie/` 下：
- `docs/research-log.md`，全部 9,353 行，2026-10-03 至 10-26。分成七段由多位讀者平行閱讀，結果在此合併並去除重複。
- `docs/PLAN.md`（全部）、`docs/methodology.md`（所有有日期的章節）、`docs/iteration-plan.md`、`docs/triage.md`、`docs/verification.md`、`docs/kpi.md`、`docs/benchmark.md`（結構與外部模型表）。
- README 中的表格（「已出貨」與「試過、沒出貨」），以及 `docs/releases/*.md`（v0.1–v0.5.0、模型與資料發布，以及命名 README）。

**善解概述。** Rust 核心加 Swift 外殼。引擎在 McBopomofo 詞庫加上一層 overlay 所建的詞 bigram 詞圖上解碼。有兩種設定檔：
- **Formal** 加上 trigram（「連文」），並關閉 KN 平滑。
- **Chat** 加上口語 bigram 表（「常談」）與讀音通道項。

在詞圖之上，一個 14M 參數的字元語言模型重排模型（「會意」）對 n-gram 前 8 個結果重新評分，門檻為 α/τ/δ。學習以上下文為鍵。這一切背後是一套非常嚴格、事先登記的評測流程。

**v0.5.0 的主要數字：**
- discordtune（使用者的 Discord 聊天），chat 設定檔：87.90%。
- wikitune-clean，formal 設定檔：75.29%。
- Holdout shard-1（300 列）：chat 237，formal 241。
- 每鍵 p95 2.05 ms。

**狀態欄**（ZhuyinType）：**有**、**有（做法不同）**、**沒有**、**不適用**，或 **待確認**（無法從我們的文件或程式碼確認）。以下幾項說法已對照程式碼確認：
- `nlp/engine/Sources/ZhuyinCore/Conversion.swift` 以語言模型歷史狀態解碼，排序時納入結尾分數。
- `zt eval stats`（`nlp/eval/stats.py`）已經有精確 McNemar、配對 bootstrap、Poisson 改壞上界、MDE 與叢集。
- `zt eval rows`（`nlp/eval/rows.py`）報告 oracle@16 與 CER，並套用善解式的異體合併。
- pkg 安裝到 `/Library/Input Methods`。
- `RecallKind` 有四種：whole、span、typo 與 phrase。

**判斷欄：** 採用、調整後採用（說明如何調整）、不採用（說明原因），或 **已涵蓋**（ZhuyinType 已有對等功能或不同做法）。

「檔案」欄的路徑相對於 `references/shanjie/`。日期皆為 2026 年，寫法與紀錄相同。

---

## A. 詞庫與資料

| # | 日期 | 名稱 | 做法 | 結果（數字） | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| A1 | 10-03 | S1 Wikipedia/Wiktionary overlay | 加入 2–4 字的純漢字詞。篩選：所有字都已是單字條目，不含只有簡體才有的字，Wikipedia 標題只在它是「詞庫詞加一個字」的複合詞時才收。建置鎖定 dump 的 SHA；`--check` 重建；不得從評測集挑詞。 | oracle@64 96.9→97.8%。加入全部 1.04M 個標題反而變差，因為人名污染了候選。holdout 的詞彙外子集救回 0 列，假設被推翻，事後更換指標並公開說明。RSS 127→375 MiB。 | `tools/build_overlay.py`、`docs/contracts/s1.md` | 有（CEDICT+wordfreq） | 調整後採用：若要加入標題，使用複合詞篩選與鎖定雜湊的 `--check` 建置 |
| A2 | 10-03 / 10-14 | 無分數詞的分數；手動詞清單 | 沒有頻率的詞取同長度詞的第 25 百分位分數（2 字 −7.17）。手動詞在此之上加 0.5，後來改為 0.75，並以 golden test 檢查每個詞都是 top-1。 | 手動詞 3/9 → 9/9。所有評測集逐位元組相同。 | `data/lexicon/overlay-manual.tsv`、`cli/tests/golden.rs` | 沒有（沒有人工整理的清單） | 採用：處理使用者要求加詞的低成本做法 |
| A3 | 10-03 / S2c | overlay 分數以語料頻率為上限 | overlay 分數 = min(lp, log10 c/N)；語料中從未出現的詞取 −1。 | wikitune 11/0，p=0.001；chat log-loss −0.0088 [−0.0137, −0.0038]。把語料頻率混入先驗（β 0.3/0.5）讓 chat 損失 100+ 列。後來（10-25）發現上限也擋住人工加權（君悅 −6.58 vs 軍樂 −6.30，即使 +1 也一樣）。 | `core/src/lm.rs`（`CappedLexicon`） | 沒有 | 調整後採用：對證據薄弱的來源設上限，但要明確決定人工加權能否勝過語料計數 |
| A4 | 10-03 | OpenCC 取第一個對應的錯誤 | `convert()` 把 STCharacters 的第一個對應套用到已經是正體的文字。72 個字被改掉（吃→喫、台→臺、里→裏），參考答案也受影響。 | 喫 54,153 vs 吃 4,283 次。第 4–7 輪調參建立在被污染的參考答案上。修正後：吃 5,022 vs 喫 4。 | `experiments/s2/build_counts.py` | 待確認 | 採用：以 吃/喫 這類字對稽核我們 Wikipedia 的簡轉繁步驟 |
| A5 | 10-14 | OpenCC TWPhrases 套用在正體文字上 | 詞語轉換套用在已經是正體的句子上：插件→外掛、程序→程式、對象→物件。 | 10 句探測句中有 2 句因此解碼錯誤。修正排在 model-v7。 | `build_counts.py` L166–171 | 待確認 | 採用：詞語轉換只用在簡體來源 |
| A6 | 10-05 | S2n 簡體殘留 | 逐句判斷是否為簡體，搭配台灣白名單（吃 皂 唇 岩 岳 咸）。提議的發布檢查：n-gram 表中只有簡體才有的字為零。 | 七個殘留詞 125,635→0 次；cvtune formal 20/1，p=2e-5。 | `docs/contracts/s2n-simplified-residue.md` | 待確認 | 採用「簡體字為零」的發布檢查 |
| A7 | 10-07 / 10-12 | 台灣字形被誤標為簡體 | OpenCC 把 床 與 秘 當成簡體（起牀）。SIMP_ONLY 集合也含 14 個台灣常用字（么峰庄床晒洒灶痒秘粽群肴虱霉）。同讀音的異體現在合併計數。 | 字形探測 10/8 → 16/16。Cofacts 被刪掉的句子從 79,779 降到修正後的 47,333。規則：印出觸發每條刪除規則最多的字。 | `docs/contracts/s2f-variant-forms.md`、`experiments/s2f/` | 待確認 | 採用：檢查我們的篩選對 群、床、秘 的處理 |
| A8 | 10-06/08 | S2w：MediaWiki 自己的 zh-tw 轉換 | 以 zhconv-rs 套用 MediaWiki 的規則加 NoteTA 群組轉換，zhconv-rs 遇到 `=>` 會 panic。 | 100 次人工檢查中 MediaWiki 對 69 次 vs 16 次，但打字沒有增益。它改變了字形政策（佔→占），的市佔 變成 的士站。沒有出貨。 | `experiments/s2w/` | 不適用 | 不採用：轉換器同時也是字形政策 |
| A9 | 10-06 | 期望值（分數式）切詞計數 | 在切詞詞圖上以期望值計算 n-gram（Mori 2006），取代硬性最佳路徑。 | c(後,到) 0→206，c(好,吧) 0→9。cvtune chat 26/11，p=0.020。建置約 8 分鐘。 | `tools/build_lm.py`、`experiments/s2/build_counts_text.py --expected` | 沒有（我們的切詞是硬性 EM） | **採用**：修正被較長詞庫詞吸收掉的 bigram |
| A10 | 10-05 | 姓氏切分（照建議→趙建議） | 單獨的 趙 有 26,281 次，88% 後接單一人名用字，照 則是 7,873 次。 | 把姓氏計數除以 4 或 10：淨 −6 至 −14，否決。規劃：NER 人名類別。 | — | 待確認（可能有相同的 Wikipedia 偏差） | 調整後採用：把人名算成一個類別 token |
| A11 | 10-04 | S2r 一/不/法/和 的變調列 | 教育部變調讀音，分數 −0.5。排除詞尾的 一（133 個詞）與序數或人名（41 個）。 | 探測：cvtune 39/3，p=5.6e-9；discordtune 64/2，p=6e-17。原讀音不變。 | `tools/build_sandhi.py`、`data/lexicon/sandhi-add.tsv` | 有（一/不） | 調整後採用：加入 法 ㄈㄚˇ、排除清單（統一、北一女、星期一）與 −0.5 的偏移 |
| A12 | 10-26 | 輕聲容錯列 | 含輕聲的多字詞，加一列全聲調讀音（同韻母，取最佳的非輕聲讀音），懲罰 δ=2.0。的/了/著 除外。共 1,502 列。 | 輕聲檢查集：chat 177→277（+100/0），formal 161→264（+104/1）。一個使用者回報的改壞案例（都留→都流）。 | `docs/contracts/neutral-tolerance.md`、`experiments/neutral/build_checkset.py` | 待確認（KeyChannel 二聲↔輕聲成本 4.5；v3 訓練時 30% 輕聲→一聲） | **採用**對方的檢查集建構工具，量測我們的覆蓋情形 |
| A13 | 10-17 | 考慮讀音的 n-gram（A1/A2/B） | A1：把從全聲調分數複製來的輕聲列降權（44 個字）。A2：懲罰一聲的句尾助詞。B：以規則標註器依讀音計算 n-gram。 | A1：3/0（改對 吧）。A2：lytune chat −213。B：崩潰，5/1462，因為標註器有 40.8% 的機率標錯（地、得）。 | `docs/contracts/reading-aware-0.md` | 有（做法不同：P(reading\|word)） | 把 A1 調整後採用為稽核；B 不採用 |
| A14 | 10-07 | SW：特定讀音的降權清單 | 只在非標準讀音下對詞扣 δ=2.0（睪丸 讀作 ㄍㄠˇ ㄨㄢˊ）。 | 搞完這波 在 4/4 種設定下改對；一次重選即可覆蓋（8/8）。 | `data/lexicon/demote.tsv`、`docs/contracts/sw-sensitive-demote.md` | 沒有 | 採用：一小份令人尷尬的撞音清單 |
| A15 | 10-22 | 口語讀音 紮 ㄓㄚ | 為 29 列加入 ㄓㄚ，分數照抄。 | 所有測試集 0/0；CER −0.08 pp；沒有通過 net>0 規則（淨 0）。 | `docs/contracts/zha-reading.md` | 待確認 | 調整後採用：對原本打不出來的字逐一核准 |
| A16 | 10-26 | SJ-LM-7 的讀音來源與新字體移除 | 讀音優先順序：教育部簡編本 → 萌典 → 規則。以 OpenCC 移除日文字形。由輔導 agent 批改樣本。 | 移除 9,339 個日文字形。規則讀音修正後 22/34 → 28/34。候選字視窗原本把 変移/辨疑 排在 變 前面。 | `tools/readings.py`、`tools/build_overlay.py` | 待確認 | 對 CEDICT/wordfreq 採用新字體篩選 |
| A17 | 10-14 | 粵語修剪 | 刪除含粵語專用字的 overlay 條目，除非萌典有收錄。 | 移除 848 列（冇、唔該、我哋）；評測結果逐位元組相同。 | `docs/contracts/canto-prune.md` | 待確認（wordfreq 可能含有這些詞） | 調整後採用 |
| A18 | S2r-2 | 破音字讀音規則（來自 zaoseq） | 破音字只從明確的詞讀音取得讀音。zaoseq 以此移除了 10,813 個假同音。 | 善解稽核：50 個 overlay 破音字中 4 個錯；約 8% 猜錯。 | `experiments/s2/overlay_polyphones.py` | 待確認 | 採用為 Unihan 衍生讀音的稽核 |
| A19 | 10-10 | 「每個詞單獨輸入時會贏嗎？」稽核 | 對每個詞庫／詞包中的詞，在沒有上下文的情況下輸入它的讀音，檢查是否為 top-1。 | 16,082/60,798 會輸（26.5%）。二字詞：98.7% 輸；3 字 38.9%；4 字 7.9%。Wiktionary 二字詞 78.1%，基本詞庫 20.8%。99.9% 是輸在語言模型項，中位數 −1.97。 | `experiments/acg-audit/` | 沒有 | **採用**：成本低，依來源與長度分析 |
| A20 | 10-09 | 缺詞與排序錯誤（M/R） | 每個錯誤列標為 M（缺詞）或 R（排序錯誤）。 | 使用者回報：7 M / 24 R。正確答案的中位排名為 2，差距約 0.5。只有 2 列在前 64 名之外。 | `eval/dev/user-reported.txt` | 沒有 | 採用為分類的第一步 |
| A21 | 10-09 → 10-23 | ACG 詞包（切片 A、A2、A3、A4） | Wikidata（sitelinks ≥3）、中文 Wikipedia 轉換群組、400 篇動畫條目、年度列表頁、含 · 的人名。選用，可在選單開關，分數取第 25 百分位。 | 14,357 → 44,552 個詞。works-chat 113→124（p=0.001）；間隔號人名 21→39/45；近期作品 14→22。+25–30 MiB。抽樣 100 個中 95–96 個確實是正確詞。 | `tools/build_acg_pack.py`（47 個測試） | 沒有 | 若我們要出詞包則調整後採用：選用、可重現的快取建置、100 個樣本的精確率關卡 |
| A22 | 10-09 → 10-15 | 新增詞彙的撞音規則 | 新詞不得搶走既有詞的 top-1（規則 c；NEW_BEHIND_SHIPPED=3.0；`first_named`）。另做句子上下文的搶奪檢查。 | 移除 105 個搶奪（家母→加姆、牧笛→穆迪）。偵測器當時是在 KN 關閉的情況下執行，也就是並非出貨設定。 | `acg-collisions.tsv`、`tools/build_overlay.py` | 不適用 | 任何詞庫新增都**採用**；以出貨時的旗標執行檢查 |
| A23 | 10-10 | model-v5：切詞器納入詞包詞 | 把詞包詞放進切詞詞彙會重新切分語料（萊+恩 1,336→0）。改用累加計數並保留舊的詞類別來修正。 | 第一版被否決：wikitune 2,133→2,112，p=0.026。 | `docs/contracts/model-v5.md` | 不適用 | 採用這個教訓：新詞彙必須讓一般計數逐列不變 |
| A24 | 10-24 | pack-oov | 詞包詞略過一般語言模型，以加分 B 評分。 | 確認測試 13/7 未通過；V1c 變體 15/2。 | `docs/contracts/pack-oov.md` | 不適用 | 不採用 |
| A25 | 10-11 → 10-25 | 名稱層（N1、創作者、店家） | 來源：NSI（BSD-3）、Wikidata CC0、教育部學校名錄。NAMES_BONUS=2。只有在 PTT 使用次數高於詞庫同音詞時才收錄名稱；去除被灌水的短名稱（辛巴）。 | 名稱集 chat 44→56（12/0）。創作者 chat 53→103，0 個改壞。店家 18→199/225。+0.5 MiB。 | `tools/build_names.py`、`docs/contracts/names-*.md` | 沒有 | **採用**（品牌、公司、學校） |
| A26 | 10-23 | IT+AI 詞包 | 國家教育研究院開放資料（15275/15416）、CGroup/IT。只加入解碼器目前會打錯的詞。門檻 50 × 加分 3。 | 151 個手動詞中 141 個本來就對。Chat 46/60（通過），formal 39/60（未通過，因此 formal 只作為防護）。discordtune 0/3。 | `docs/contracts/it-pack.md`、`tools/build_it_pack.py` | 沒有 | 採用：國教院的 CSV 是開放資料；加分是主要槓桿 |
| A27 | 10-24/25 | 網路用語探勘 | 在 PTT 上以 PMI 加相鄰熵探勘，再交給 Sonnet 分類。若詞庫同音詞在 PTT 上用得更多，就捨棄該網路用語。 | 已知網路用語的中位排名 1,589，原始計數則為 20k。第 1 輪：507 個詞，discordtune 2/0。第 2 輪：910 個詞，86.0%（關卡 90%）。 | `docs/contracts/net-slang*.md` | 沒有 | 採用此探勘方法 |
| A28 | 10-24/25 | 重排模型抵銷新詞彙 | 分別在重排模型開啟與關閉時量測檢查集。 | 網路用語：重排開啟 779/906 vs 關閉 818/906（48 個改壞 / 9 個改對）。店家：10 個改壞 / 8 個改對。 | — | 有此風險（v2b、v3） | **採用**：以完整的出貨流程測試新詞 |
| A29 | 10-13 | 領域詞包調查（醫學、法律） | 覆蓋率 AI 11/30、IT 35/49、醫學 27/34、法律 36/39。沒有計數的條目會輸（插件 −8.17 vs 執事 −5.85）。教育部辭典為 CC BY-ND，因此不採用。 | 規劃：IT/AI 預設開啟，醫學／法律預設關閉。 | PLAN | 沒有 | 調整後採用 |
| A30 | 10-12 → 10-26 | 授權分級與來源稽核 | 核心只使用 CC0/BY/BY-SA／政府資料；NC 來源放到另一個 repo。robots.txt 回傳 HTTP 500 視為不允許。中文維基新聞 CC BY 可用；TED 與報紙排除。目前不存在授權乾淨的台灣聊天語料（MozTW 26,393 句）。 | 政策 | `LICENSES/data.md`、`docs/data-files.md` | 部分（Taiwan Tongues TRAIL 尚未解決） | 出貨聊天模型前採用分級制度 |
| A31 | 10-18 | 公開口語語料池 | WildChat-TW（ODC-BY）、YODAS zh000（CC BY 3.0）、Tatoeba、MASSIVE zh-TW。切句，4–40 個漢字，漢字比例 ≥70%，去除個資與程式碼，窄範圍的簡體篩選（3,795 字），去重，檢查洩漏。 | 143,895 句，3.0M 個漢字。YODAS 有 45% 因簡體被刪除。洩漏檢查移除 2,237 句（1,112 句與 cvtune 完全相同）。 | `experiments/p2-data/` | 沒有 | **採用**於我們的 chat 語言模型 |
| A32 | 10-25 | OPUS zh_TW 字幕 | 使用原始 zip（9,891 個檔案，10.73M 句），不設每部片的上限。中國大陸用語篩選以詞邊界保護；每 1,000 行由兩個 Sonnet 審閱者檢查，再以文字比對回原句。 | 4,432,866 句，42.8M 個漢字。經七次校對，殘留率 6.7% → 1.7%（Wilson 0.9–3.0%）。 | `experiments/sj-lm-7/clean_subs.py`、`review_subs.py` | 沒有 | **採用**：最大的開放口語來源 |
| A33 | 10-25 | PTT 與 g0v 清理 | PTT：只合併自動換行的行（77–80 個顯示欄位）。g0v #rand0m（CC0）：刪除引言與程式碼。以最小長度 6 對評測集去重。 | 3,000 篇 PTT 文章的句數 21,387→36,896。g0v：15,318 句。最小長度設 4 會刪掉約 530k 個常見片語。 | `ptt_clean.py`、`g0v_clean.py`、`dedupe_eval.py` | 沒有 | 調整後採用（確認 PTT 使用條款；只用計數） |
| A34 | 10-12 / 10-18 | S7f：以真實與生成的口語文字重新訓練 | 真實文字（archive.tw CC0、Cofacts、Wikipedia 討論頁）加上 LLM 生成的句子（US$16.83）。 | 在非聊天集上調參時失敗（A1 −126、A2 −107）。固定參數時，改壞數 163→131。seed 雜訊為 32 列。3 個 seed：A2 +104..+119 vs 對照組 +55..+76，但低於 MDE。 | `docs/contracts/s7f-colloquial.md` | 沒有 | 調整後採用：在口語資料上調參，並使用 ≥3 個 seed |
| A35 | 10-03 | 針對易混淆字對的合成句 | 針對 在/再 等字對生成的 LLM 句子（29,810 → 篩選後 21,611）。 | 在調參集與 Discord 上都退步。 | `experiments/s2/` | 不適用 | 不採用（負面結果） |
| A36 | 10-14 | model-v6 以 LLM 補充文字並加篩選 | 由 gpt-oss-120b 生成 500k 字（US$2.87），篩除與評測重疊（90）、種子詞未使用（405）、簡體（67）與混入詞（312）。 | 併入 model-v6。 | `experiments/model-v6/generate.py` | 有（生成的聊天句） | 調整後採用這些篩選到我們生成的聊天語料 |
| A37 | 10-20 | 以網路長文作為評測集 | HPLT 論壇與部落格；維基文庫散文。 | HPLT 有 12.7/5.3/6.7% 不良（SEO 工具插入真實詞）。維基文庫 13.7% 不良（簡體原文、文言引文）。放棄。 | `experiments/longctx-eval/` | 不適用 | 避免：使用經過編輯的文字或私人聊天 |

## B. 語言模型與解碼

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| B1 | 10-03 | 內插先驗加上句尾評分 | 以 λ=0.7 內插詞庫先驗與語言模型，並為 `</s>` 評分。 | +112 cvtune / +207 wikitune（p 1e-16、1e-31）。單獨 EOS +17/+19。 | `reference/proto/lm.py` | 有（句尾占比 s） | 已涵蓋 |
| B2 | 10-03 | 保留上下文總量的修剪 | 保留修剪前的總量；被修剪的質量歸給 back-off。 | 單純重新正規化損失 61/137 列。Chat ≥2：23.2M→8.8M 個條目，損失 11 列。 | `tools/build_lm.py` | 待確認 | 若要修剪則採用 |
| B3 | 10-15/16 | Trigram：大小掃描、熵修剪、下限 | 依計數修剪的表（100–1,133 MB）。相對熵修剪。下限 P3 = max(P3, 0.5·P2)，因為稀疏儲存的上下文會讓每個未見過的詞都被扣分（搞完這波→睪丸這波）。 | 起初只有未修剪的 1,133 MB 表通過。熵修剪 H3：+64 vs T3 +35。下限：調參集 +73，改壞 141→120。 | `experiments/derisk-1015/e1/`、`core/src/trigram.rs`（`TRI_FLOOR`） | 沒有（我們快照版 trigram 未通過測試集，MODEL §11） | **調整後採用**：我們失敗的 trigram 可能缺少下限與熵修剪；只對 Wikipedia 成分重試 |
| B4 | 10-16/17 | Trigram 只在 formal 設定檔出貨，mmap | 有 trigram 時 formal 設定檔關閉 KN；chat 不變。 | wikitune +49、lytune +36、discordtune +40、cvtune +24，沒有一項變差。安裝檔 +48 MB。mmap 後佔用 444→304 MB；p95 +0.01 ms。 | `docs/contracts/trigram-core.md`、`trigram-mmap.md` | 沒有 | 調整後採用（見 B3） |
| B5 | 10-10 → 10-23 | Kneser–Ney 接續旁檔 | 內插 N′ back-off。 | +33/13,236 列；holdout 平手。KN 與 trigram 交互作用不佳（KN 關閉時 formal FT +106）。chat 關閉 KN −33（p 0.012）。檢定力：+0.25% 需要約 20k 列。 | `docs/contracts/kn-*.md`、`tools/kn_cont.py` | 有（modified KN） | 已涵蓋；注意 KN×trigram 的交互作用 |
| B6 | 10-08 / 10-16 | Brown 詞類別 back-off（K=512） | 對未保留的 bigram 套用類別 n-gram 項 μ。 | cvtune +21/+26（p 0.002）。μ 0.8 搭配 KN 造成 lytune −45（p 7e-5）。μ 0.2：lytune +92。 | `tools/build_classes.py`、`docs/contracts/classes-v5.md` | 沒有 | 不採用（v3 已涵蓋泛化） |
| B7 | 10-16 | 沿路徑歸因回歸 | 逐一開啟各元件（class、KN、trigram）以拆解一次回歸。 | 找到 class×KN：KN 開啟時 −39/−56，關閉時 −10/−4。 | `docs/contracts/lytune-attrib.md` | 沒有 | **採用**此方法 |
| B8 | 10-13 | 依（字串，最後一個詞）合併 | 解碼器原本每個字面字串只保留一條路徑（夏來分析）。 | lytune 與 discordtune 在 4/4 個格子中變好（p ≤ 0.017）；n-best 中不同字串的數量變少。 | `docs/contracts/dec-recombine.md` | 有（Conversion 狀態 = 語言模型歷史；排序納入 `endScore`） | 已涵蓋 |
| B9 | S2c | 固定詞之間的空隙 | 每個固定詞之間的空隙，以左側固定詞為歷史、進入右側固定詞的轉移為結尾項來解碼。 | 隨一個必須失敗的測試一起出貨。 | `core/src/engine.rs` | 待確認（FixedWord 路徑） | 採用為測試 |
| B10 | 10-11 | P1a：在/再 混淆分類器 | 以 s+μf 加入稀疏線性分類器。 | 再>在 的錯誤 11→10（目標是減半）；+0.05 pp。錯誤：ln 與 log10 混用（μ 大了 2.30 倍），以及受類別上限影響的先驗。 | `docs/contracts/p1a-confusion.md` | 不適用 | 不採用；採用其檢查清單：對數底數一致，移除訓練先驗 |
| B11 | 10-13 | 功能字同音集（fw） | 在 6 個測試集上的 在/再、作/做、以/已、是/試。作說明 是可接受的正式用法。 | 重排模型 vs n-gram 30/1，但 fw 仍有 23/94 錯（差距小於 δ）。 | fw 測試集 | 部分（`nlp/training/homophone/`） | 採用：把合理的替代寫法算對 |
| B12 | 10-14 | 三道門檻的模糊輸入 | G0 只在完全無法解析時；G1 在精確結果的前 5 名之後；G2 在詞圖內並加 δ。δ 以乾淨輸入改壞 ≤0.5% 選定。 | 模糊音 top-1 91–95%。聲調 62–67%。相鄰鍵改壞了 4/76 句乾淨輸入（5.3%）。 | `docs/contracts/fuzzy-research.md` | 有（做法不同：KeyDecoder、KeyChannel、精確優先門檻） | 已涵蓋；採用「合法讀音錯誤」與「不合法讀音錯誤」的區分 |
| B13 | 10-15 / 10-10 | 只打聲母的縮寫 | E3 整句只打聲母；V3 韻母可省略。 | top-1 比例 0.789/0.808（<0.9）；oracle@64 289/300；每列 148 ms。實機測試：按鍵卡住，因此隱藏。 | `docs/contracts/v3-engine.md` §12 | 沒有 | 不採用 |
| B14 | 10-18 | E-C：聯合重新調整解碼器權重與融合 | λ、KN β、trigram 下限、α/τ/δ 一起調整。 | 最佳格子只有 +5；其他格子只是交換列（UB 0.67–2.67%）。 | `experiments/e-c/` | 有（已調整） | 備註：重新調參很少划算 |
| B15 | 10-18 | E-D：放寬每個讀音的詞庫上限 | PER_KEY 12→24/48/1000。 | Chat oracle@64 ≤ +0.03 pp；formal −2 至 −5。 | `experiments/e-d/` | 不適用 | 不採用 |
| B16 | 10-20 | 差一點答對的診斷（D2/D3） | 差距直方圖、混淆字對集中度、Wikipedia 偏差指標。 | 40% 的排序錯誤差距 <0.5，69% <1；88% 在前 8 名內。前 10 個字對只占 19.8%（因此不做字對表）。Wikipedia 偏差 20.2%（開啟 P4）。 | `experiments/diag-near-miss/d23.py` | 部分（`diagnose.py`） | 在任何針對性修正之前採用 |
| B17 | 10-20/21 | P4→P4d 口語項 | 口語 bigram 表「常談」（68,055 個詞）作為獨立的 log-linear 項，權重 μ。只在 overlay 中出現的詞懲罰 γ。在重排層套用，以 n-gram 第 1 名為錨，並加軟性門檻。 | 融合前套用（P4b）：128/92，否決。P4d：74/43（p 0.0053）。在 chatjudge-2 上確認：108/39，+69，p 1.1e-8，CER 2.85→2.61%。RSS +29 MiB。 | `core/src/colloquial.rs`、`docs/contracts/colloquial-term.md`、`docs/releases/sj-cl-1-68k.md` | 部分（chat 語言模型混入詞圖） | **調整後採用**：把真實口語表加為獨立的項，以首選為錨並加門檻 |
| B18 | 10-21 | P5 讀音通道項 | 以單字的次要讀音輸入時，相對其主要讀音給予懲罰，上限 4（改對 變快→便會）。 | discordtune 23/0（p 2.4e-7）。一個改壞（長歪了→掌歪了），來自 −6.8 的占位分數。softmax 估計量對 9,404 個同分詞有偏差。 | `core/src/colloquial.rs`（`channel_of`） | 有（做法不同：P(reading\|word)） | 已涵蓋；稽核次要讀音的占位分數 |
| B19 | 10-22 | 不受 α 影響的額外項 | 融合分數內的係數是 μ/α 與 ν/α，因此每個強度都要用自己的 α。 | 核心與離線結果：每個格子都 0 列不同。 | `docs/contracts/colloquial-levels.md` | 不適用 | 若我們加入強度等級則調整後採用 |
| B20 | 10-08/13 | 不看讀音的 bigram 計數 | 空 ㄎㄨㄥˋ 借用了 空間 的計數（版空）；啦 的輕聲計數被算到一聲。 | 規劃：考慮破音字的搭配詞。 | PLAN | 部分（bigram 不看讀音；v3 看得到音節） | 透過 v3 調整後採用；不要用規則標註器（A13） |
| B21 | 10-22 | 組字上限 128 並凍結前綴 | 超過 W = context−3（61）個音節後，逐詞凍結已解碼的前綴，保留 16 個音節的緩衝區；凍結部分不再解碼。 | 按鍵路徑 p95 26–28 → 13.4 ms；`top1_sha256` 相同。 | `core/tests/engine_cap.rs`、`docs/contracts/composition-cap.md` | 部分（KeyDecoder 在 >48 鍵時凍結；v2b/v3 仍對整句執行） | 調整後採用：超出模型視窗的部分凍結詞圖與重排 |

## C. 神經重排與融合

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| C1 | 10-03 | N0：LLM 重排 vs 讀音受限的生成 | 以 Gemma 重排 N-best vs 逐字生成。 | 重排 91.4%，p95 848 ms；生成 82.5%（偏好通順的詞）。天花板：oracle@16 94.7%。 | `experiments/n0/` | 有（做法不同） | 已涵蓋；一律報告 oracle 天花板 |
| C2 | 10-03 | N1：雲端評審只回傳索引 | OpenAI 相容端點；32 種設定。 | gpt-6-luna 92.7%（p95 3.1 s）；Cerebras gpt-oss-120b 92.4%（740 ms）。US$2.24。 | `experiments/providers/` | 沒有 | 暫不採用；設計方向正確（只回傳索引，逾時退回本機） |
| C3 | 10-05/06 | LLM 評審：位置偏差 | 從清單中選擇的 prompt vs 逐候選的對數機率。 | Apple FM 83.5→75.4%，72% 選第一項；Clef top-1 在第一位時被選 31.6%，在最後一位時 76.9%。Qwen3-8B 有上下文：87.6 vs 84.4（p 0.037），每列 0.5 s。 | `experiments/s5-*` | 有（做法不同） | 已涵蓋 |
| C4 | 10-03 | 在前 64 名上的判別式感知器 | 稠密加稀疏特徵；以使用者的聊天 ×16 加一般資料訓練。 | 第一個讓三個測試集都進步的設定（+24/+58/+67）。字對特徵過擬合（−30 至 −54）。 | `experiments/s2/rerank.py` | 沒有（學習式融合規劃中） | 調整後採用：在本機以使用者的送出內容加權訓練融合 |
| C5 | 10-06/07 | 離線融合研究 | 在儲存的分數上比較加權融合與 τ 門檻；參考文獻（Kerrigan 2021、McBopomofoLM β=0.3）。 | 加權 89.2 vs τ 87.8（p 0.17）。單參數的貝氏規則等同於差距門檻。要偵測 2 個百分點需要約 1,100 列。 | `experiments/fusion/` | 沒有 | 採用檢定力規劃 |
| C6 | 10-11 | S7a：小型字元語言模型重排（M，14.4M） | 只在 top-1/top-2 差距 < τ 時重排 n-gram 前 8 名；2 個字的上下文。 | discordtune +67（p 0.0008）；holdout 180→200（p 0.0003）。S（5.7M）未通過。 | `experiments/s7a-tinylm/`、`core/src/tinylm.rs` | 有（做法不同：v2b 31M 前 16 名，v3 在詞圖內） | 調整後採用 τ 不確定性門檻 |
| C7 | 10-11 / 10-17 | 更大的重排模型（L 30M、XL 65M） | 相同資料，主要是 Wikipedia。 | L 在調參集上 +5.64 pp，但在聊天上 −30（相對 M 為 −97）。XL 在 Holm 校正下沒有任何一項通過。重排 p95：M 8.6、L 16、XL 32 ms。每參數 token 數：17 / 8 / 4。 | `docs/contracts/rerank-size.md` | 有 | 不採用：沒有新資料就不加大模型 |
| C8 | 10-11 / 10-19 | int8 權重 | 逐列對稱 int8，以 McNemar 非劣性檢定檢查。對 0c 而言逐列失敗；逐輸出欄 int8 加 f32 嵌入則可行。 | M 57.7→14.7 MB，12 個格子中沒有損失。0c 逐列 296/300，p99 0.437（未通過）；逐欄 300/300。 | `experiments/s7a-tinylm/int8check.py`、`tools/convert_oc.py` | 沒有（fp16 Core ML 129 + 60 MB；ROADMAP §4.6） | **採用**逐通道 int8、fp32 嵌入，並以配對檢定驗證 |
| C9 | 10-12 | 目標函數的天花板（「1 個改壞 = 10 個改對」） | 計算在此加權下可達到的最佳淨值。 | 即使直接在評審集上挑選：+17..+23 / 4,091，低於 32 列的雜訊。改用 1:1 淨值加上改壞率上限。 | `docs/methodology.md` 10-12 | 不適用 | 採用：選定加權方式前先計算天花板 |
| C10 | 10-12/13 | 在私人聊天集上調整融合參數 α/τ/δ | α 權重，τ 不重排的差距，δ 切換所需的門檻。依設定檔存在模型標頭中。 | R3′（0.3/2/0.25）：125/16，淨 +109，改壞率 0.24%。R1：2.16%。Holdout chat 180→191（11/0）。 | `experiments/e2-chateval/judge.py`、`docs/contracts/s7h-tinylm-v3.md` | 沒有（v2b 權重 1.0，沒有 δ；4.0 的門檻只涵蓋修正） | **採用**：為 v2b 加上 δ/τ 門檻 |
| C11 | 10-14 / 10-22 | 強度等級（保守／標準／積極） | 每個等級在各自的改壞上限下選定；積極等級若在防護集上退步就降級。 | Chat 積極等級崩潰（lytune 236/374），因此與標準相同。Formal 積極等級：holdout 12/1。 | `docs/contracts/rerank-strength.md` | 沒有 | 調整後採用：只提供通過關卡的等級 |
| C12 | 10-14/15 | 重排失敗的 D2 拆解 | 正確答案在前 8 名但沒選到：D2a 被門檻擋掉，D2b 門檻太小，D2c 重排模型判斷錯誤。 | (i)/D2c = 0.275–0.342，因此融合與模型兩方面都還有可做的事。 | `experiments/e2-chateval/d2c_split.py`、`docs/triage.md` | 沒有 | 在決定改融合還是重新訓練之前**採用** |
| C13 | 10-17/18 | 以點估計設定的改壞上限無法轉移 | 在 0.5% 上限下選定的值。 | 0.50% 變成 1.72%（formal）；0.43% 變成 0.54%（XL）；調參集 0.33 → dev 0.58 改壞/改對。修正：Poisson 95% UB，以 0.3% 調參，以評審指標選擇（改對 − 2×改壞）。 | `select_weighted`、`docs/methodology.md` 10-18/20 | 有工具（`stats.py` Poisson 上界） | 採用此流程 |
| C14 | 10-13 / 10-18 | 以外部 0c 模型（32.6M、讀注音、64 字上下文）為量尺 | 在善解自己的前 8 名上評分，沙盒執行，鎖定雜湊，前綴打包。 | Chat +142/+127；Wikipedia −88；lytune −56。選對率 66.5% vs 45.1%。P1 discordtune +105（p 1.6e-12），但 UB 0.61%。 | `experiments/x0c/`、`docs/benchmark.md`（外部模型章節） | 有（做法不同：v3 讀音節） | 已涵蓋；採用「外部模型在我們的 N-best 上評分」作為量尺 |
| C15 | 10-17/18 | 以讀音為條件的小型語言模型 | 因果讀音輸入；讀音前綴格式 `[CTX][RD][OUT]`。 | 因果：−71（p 0.0002）。讀音前綴 +32，對照組 +91。 | `train_r.py`、`experiments/e-b/` | 有（做法不同：v3 對所有音節雙向編碼） | 已涵蓋；對方的負面結果與我們的設計一致 |
| C16 | 10-18 | E-F：學習式 logistic 門檻 | 特徵：n-gram／語言模型差距、排名、top-2 差距、softmax 機率。 | 調參集 +79 vs 相同規則 +63；在 dev 上輸給出貨版（+10/−36，p 0.00016）；UB 0.61%。 | `experiments/e-f/` | 沒有 | 調整後採用：加入字串特徵（字元類別、句首、常見多字詞） |
| C17 | 10-18 | L0：列表式目標 | 在 8 個候選上做交叉熵，並有「維持第 1 名」選項。 | 崩潰：保留率 98.5–99.4%，選對率 19.1/9.6%。discordtune −38。 | `experiments/l0-listwise/` | 沒有 | 謹慎地調整後採用：重新平衡（81.6% 的列本來就對） |
| C18 | 10-18/19 | 把 0c 蒸餾成小型學生模型（P2、B、C、L、T、R） | 在前 8 名上做 KL、讀音輸入、不含 Wikipedia、L 尺寸、逐位置 KL、困難列 ×4。 | discordtune −2、−2、−35、+7、−41、−4。逐位置 KL 改善了，句子排序卻變差。此方向結案。 | `experiments/p2*` | 不適用 | 不採用 |
| C19 | 10-19 / 10-18 | 前綴共用與延遲期限 | 每列的上下文+讀音 KV 只算一次。pinyin-ime-reranker-4M 跨按鍵重用前綴（p95 20.64→8.61 ms）。0c 超過 100 ms 時退回詞庫順序。4M 模型從不改寫完全相符的整詞詞庫命中。 | 0c Rust p95 74.5→33.9 ms；RSS +273→+227.5 MiB。 | `core/src/oc.rs`、`docs/iteration-plan.md` §8 | 有（同一按鍵內的前綴樹） | **採用**跨按鍵狀態重用，以及帶備援的硬性期限 |
| C20 | 10-20 | D1：融合前緣 | τ 2 下的 24 個 α×δ 格子。 | 沒有任何格子全面優於出貨版的 +107/−26。融合方向結案。 | `experiments/diag-near-miss/` | 沒有 | 採用：畫出我們 v2b 權重的改對／改壞前緣 |
| C21 | 10-11 / 10-18 | 逐字對的改對／改壞與剩餘空間診斷 | 每個混淆字對的改對／改壞數；在可修正的列上計算選對率與保留率。 | 在→再 15 個錯誤 / 改對 5 / 造成 7（淨損失）。她→他 1/182。M 選對率 44.1%，保留率 87.3%。 | 臨時腳本、`judge.py` | 部分 | 採用 |
| C22 | 10-19/20 | 重排模型的長上下文 | 以 63 字上下文訓練學生模型（S64）；給出貨的 M 餵 64 字。 | S64 −39（p 0.0005）。M@64：−1 / +7，不顯著。上下文長度現在寫在模型標頭中。 | `experiments/p3-context/` | 有（v3 255，v2b 24） | 採用標頭欄位；對方的小模型從上下文得到的增益很小 |
| C23 | 10-18 → 10-21 | 重排模型的學習防護 | n-gram 第 1 名帶有學習加權時跳過重排；0c 只否決使用者選過的片段。 | 防護擋掉了 35 個使用者回報的列；關閉它：+1/0。 | `core/src/engine.rs`（`boosted`） | 待確認（v2b vs recall） | 調整後採用：只保護使用者選過的片段 |

## D. 上下文與領域（chat/formal）

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| D1 | 10-03 | 依 App 區分 chat 與 formal | 聊天 App 用 λ 0.5，其他用 0.7。 | discordtune +84（p 3e-10），但 cvtune −37、wikitune −53。Discord：善解 84.0 vs macOS 88.2。 | — | 有（做法不同：比例依前文調整） | 調整後採用：以最前方的 App 作為 c₀ 的先驗 |
| D2 | 10-12 | 設定檔介面與逐 App 覆寫 | 自動／一律 chat／一律 formal，另可在選單中逐 App 覆寫。 | 14 個測試，抓到 9 個突變。 | `docs/contracts/profile-choice.md` | 沒有（覆寫） | 採用覆寫功能 |
| D3 | 10-06 | n-gram 的左側上下文，以及真實 App 的讀取 | 上下文詞作為第一個詞的歷史。真實 App 回傳的 markedRange 為 `{1,0}` 或 `{NSNotFound,NSNotFound}`，因此上下文從未被讀到；假的客戶端掩蓋了這個問題。 | cvtune chat 277/22（p 2.5e-57）。 | `docs/contracts/s2h-left-context.md`、`docs/methodology.md` §7 | 有 | **採用**只含數字的真實 App 上下文 log 檢查 |
| D4 | 10-08 / 10-15 | 跨越數字、英文與標點的上下文 | 數字或英文之後用 `<X>` token；逗號處不重設。 | `<X>`：chat 36/18（p 0.02），但 wikitune 防護未通過。跨越標點：未通過。 | `experiments/derisk-1015/` | 有（讀取 128 字） | 備註：量測我們的做法沒有造成傷害 |
| D5 | 10-18 | E-1：離線與 App 上下文 | 每個聊天集分別在有上下文與沒有上下文的情況下執行。 | 淨 −6..+5，不顯著；lytune −22。原則：離線上下文要完全依 App 看到的樣子建構。 | — | 部分 | 採用 |
| D6 | R4 / 10-25 | 上下文回傳規則 | ≤256 個 UTF-16 單位，在換行處截斷，≤64 個字素，終端機只取漢字，排除 marked range，只存在記憶體中。洩漏未送出的文字讓評審信心從 0.98 降到 0.44。 | `top1_sha256` 相同；p95 +0.05–0.09 ms。 | `docs/contracts/context-return.md` | 有（128 字） | 採用 marked range 與終端機的檢查 |
| D7 | 10-25 | 英文上下文後的中文接續表 | 英文 token（2–32 個字母）→ 可能接續的中文。 | 只有工具。 | `core/src/english_next.rs` | 沒有 | 之後再說 |
| D8 | PLAN Q | 新手引導測驗 | 選擇領域；輸入 10–20 句以偵測 和/一/輕聲 與 ㄣ/ㄥ 的習慣；選擇字形偏好。 | 尚未開始。 | PLAN | 沒有 | 調整後採用：用來初始化打錯習慣成本與模糊音開關 |
| D9 | 10-20 | 想法清單 | 依話題追蹤加權詞包；「兩個模型都同意才改」；Enter 前預覽；送出後復原；計算 AI 修改次數。 | 只是想法。 | research-log 10-20 | 部分 | 採用「計算 AI 修改／復原次數」指標 |

## E. 個人學習

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| E1 | 10-05 | 單字污染與全域層 | 一次選了 再 就變成全域設定，破壞了每個 ㄗㄞˋ。規則：單字永不進入全域；只在上下文完全相同時；永不在 `^`。ε 掃描。回歸定義為「原本完全正確，現在不對」。 | 污染 24 個回歸 → 0。ε 0.1：學到 4/9，污染 1；ε 2：9/9，10；ε 6：28。全域層關閉。 | `core/tests/engine_learn.rs`、`docs/contracts/s4-learning.md` §12–13 | 有（κ back-off），沒有這些測試 | **採用**污染與鏡像測試 |
| E2 | 10-13/14 | 句首學習（規則 7/8） | 在 `^` 學到的 我試 分數 −0.89，在所有地方都勝過 我是（−3.89）。現在句首條目需要下一個音節相符；單字需要 ≥2 個漢字的上下文。 | 4 個失敗的重現測試通過。維護者的紀錄中 64/273 筆被捨棄。p95 +0.14 ms。 | `docs/contracts/learn-single-ctx.md`、`core/src/learn.rs` | 待確認（我們的 recall／個人語言模型在組字開頭的行為） | **採用**此測試 |
| E3 | 10-10 | s4-bayes：Dirichlet 後驗 | 以後驗取代固定加分；40 種設定。 | 0 個通過；鏡像測試 0–3/8 vs 7/8。q≈1 時，一次選字無法勝出。 | `docs/contracts/s4-bayes.md` | 有（κ back-off） | 調整後採用：為我們 κ=1 的 trigram 加入「教一次，下次就會」測試 |
| E4 | 10-05 | 學習資料的寫入路徑 | 附加 ≤3 筆紀錄且不 fsync；每 1,024 行以 F_BARRIERFSYNC 完整重寫。 | Enter p95 16–19.7 → 0.17–0.20 ms。 | `docs/contracts/s4-learning.md` §11 | 待確認 | 採用 |
| E5 | 10-10 / 10-17 | 新詞 B，含子片段 | 從逐字重選學習 2–4 字的連續片段，以及詞庫中沒有的子片段。陰影規則 −6.5（詞庫已有強勢同音詞時跳過）。S ∈ [−3,−1]；上限 2,000。 | 探測 8→18/19（p 0.002），CER 13.0→2.1%。cvtune chat 22/4；wikitune 99/6；0 個搶奪。 | `docs/contracts/newword-b.md`、`core/tests/engine_newword.rs` | 部分（phrase/span recall） | 採用子片段學習與陰影規則 |
| E6 | 10-09 | Apple 重選一次就學會新詞 | 落檔 學一次後，在 記得 之後也能打出。 | 目標行為。 | `tools/imeshot.swift` | 有（recall） | 測試相同流程 |
| E7 | 10-17 / 10-18 | 學習的使用體驗 | 預測選字會被學習；在候選字視窗按 ⌘⌫ 忘記；沒有 bundle ID 的 App 暫停學習；清除前要求確認。 | 已出貨。 | `macos/.../Learning.swift` | 有（不記住清單） | 採用 ⌘⌫ 快速忘記 |
| E8 | R3/R5 | 學習的隱私關卡 | 安全輸入或封鎖清單會停止學習與上下文讀取；檔案權限 0600，只儲存（上下文、讀音、詞、權重、日期）；以標記字串測試證明打的字不會進入 log；說明 Time Machine 會備份。 | 已出貨。 | PLAN §7 | 沒有（ROADMAP P5） | **採用** |
| E9 | 10-13 | 依類別或領域泛化學習 | 只是想法。 | — | PLAN | 沒有 | 持續觀察 |

## F. 預測與補全

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| F1 | 10-07 | SP/V3：依上下文與部分注音預測 | 候選為與目前按鍵相容的詞庫字串。排序「S」：上下文詞的後繼詞優先，再接其餘。Tab 進入預測列。 | 上下文讓第一鍵的 hit@9 從 4.6→25.4%。省下按鍵 42–47%。只有後繼詞 43.5% vs 64.4%；S 63.9%。+43 MB；p95 0.8→1.8 ms。 | `core/src/predict.rs`、`docs/contracts/sp*.md` | 沒有（0.6.0 規劃） | **採用** S 排序與各階段的 hit@9 |
| F2 | 10-09 | 起點最多往回 5 個音節 | 詞的起點最多往回 5 個音節；詞中間的起點排最後。 | typing76 8/0；ACG 名稱 57.9→71.7（453/0）。 | `docs/contracts/v3-engine.md` §11 | 沒有 | 採用 |
| F3 | 10-09 | 後繼詞排序可能埋沒詞 | λ·logP(w\|prev) + (1−λ)·lp 把 繳 排到第 8。 | — | `core/src/predict.rs` | 沒有 | 調整後採用 |
| F4 | 10-17 | 向下預測的一致性 | ↓ 先顯示預測項目；⇥ 符號；讀音與已打音節相同的預測列項目必須算作重選。 | 合併後才發現錯誤。 | `docs/contracts/predict-down.md` | 沒有 | 採用 |
| F5 | 10-17 | 數字上下文預測 | 統計數字串後面接哪些詞（1.56 MB）。 | 數字後的 hit@9：chat 54.8→85.7%，formal 57.4→87.8%。 | `data/lexicon/digits.tsv`、`docs/contracts/digit-context.md` | 沒有 | 採用 |

## G. 標點

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| G1 | 10-04 | 標點留在組字中直到按 Enter | 使用者在聊天 App 中習慣按 Enter，會直接送出訊息，因此標點留在組字中。標點作為語言模型的句子邊界。 | 抓到兩個突變。 | `docs/contracts/s3d-punctuation.md` | 待確認 | 採用 |
| G2 | v0.2.0 | 標點與 Apple 一致 | 候選命名與 Apple 相同；`[ ] \ \`` 不用 Shift 就能打出標點；空緩衝區上按聲調鍵會打出聲調符號；Ctrl+\ 打出 、。使用自己的對照表，未複製 Apple 的資料。 | 已出貨。 | `docs/contracts/s3e-*.md`、`s3f-*.md` | 部分（`Keyboard.swift` 對照表） | 採用一致的行為 |
| G3 | 10-19 | 中英自動空白（盤古之白） | 送出時在漢字與拉丁字母／數字之間插入空白，也會檢查游標前的字元。預設關閉。 | 13 個測試，抓到 8 個突變。 | `docs/contracts/cjk-spacing.md` | 沒有 | 採用（成本低） |
| G4 | — | 標點預測 | 不在善解已公開的工作範圍內。 | — | — | 規劃中（v3 分類頭，0.5.0） | 不適用：範圍不同；目前沒有可比較的內容 |

## H. 評測方法與統計

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| H1 | 10-03/08 | 配對的逐列統計 | `--rowstats`；精確 McNemar；CER；配對 bootstrap CI。解析度表（dev302 需要 2.3 pp）。 | 10-08 起成為標準。 | `tools/evalstats.py` | 有（`nlp/eval/stats.py`、`rows_eval.py`） | 已涵蓋 |
| H2 | 10-03 | 寬鬆異體（教育部 也作/亦作） | 2,072 個同讀音異體加上 8 個合併規則（她→他 等），依辭典固定，不從評測錯誤中挑選。 | 善解 83.4 / macOS 88.2 / McBopomofo 76.6 / 自然 72.2（Discord 845）。 | `eval/variants.tsv` | 有（`--shanjie-variants`） | 有；注意它掩蓋了一個 它/他 錯誤 |
| H3 | 10-03 | 與其他輸入法的共同測試比較 | 把相同按鍵輸入 Apple、McBopomofo 與 自然。Apple 學到的 2,647 個片語中，有 847 個是從測試本身學來的。 | macOS 內建表現最好。 | `tools/imetype.swift`、`docs/typing-test.md` | 沒有 | 採用：每個次要版本都在乾淨的 VM 帳號中測試 |
| H4 | 10-05 | 挑戰集會誤導 | dev302（由 LLM 與專案撰寫）vs 真實聊天。 | Apple 評審 dev302 +27 vs discordtune −81；Qwen3 +35 vs −17。 | `docs/methodology.md` §2 | 有（dev/test） | 採用此規則：以真實語域的測試集做決定 |
| H5 | 10-12/13 / 10-18 | E2：私人聊天評測 | 取自維護者自己訊息的子句：只用公開頻道，時間在使用善解之前的幾個月，兩種上下文，答案以盲測方式校對並以雜湊凍結，調參月份在評審月份之前凍結。去除 markup 後套用個資刪除規則（email、電話、6 位以上數字、ID、token）。10% 抽查，修正率 ≥1% 即停止。每個決策用一組評審集。 | 調參集 5,761 列；評審月份 6,559–6,843；chatjudge-2 5,783 列（抽查 0.34%）。 | `docs/contracts/e2-chat-eval.md`、`experiments/e2-chateval/build_chatclause.py` | 沒有 | **採用**（首要行動） |
| H6 | 10-12 | 公開的聊天防護集 | lytune：立法院發言，4,811 個子句（Apache-2.0）。wikitypo：155 個打錯字對。 | R1 在調參集上看起來正常，卻改壞了 lytune 的 10.08%。 | `build_lytune.py`、`build_wikitypo.py` | 沒有 | 採用 lytune |
| H7 | 10-10 / 10-18 | 事先登記與雙軌制 | 量測前先提交契約。探索軌與確認軌。各分支間做 Holm 校正。先算 MDE。≥3 個 seed。叢集 bootstrap。非劣性防護（90% 下界 ≥ −1.0 pp）。「依登記未通過」不等於「結案」。 | MDE 範例：61 個不一致列 → 在 α 0.05 下需 +23。 | `docs/methodology.md` 10-18、`docs/iteration-plan.md` | 部分（`stats.py` MDE 與叢集） | 採用此流程 |
| H8 | 10-15/16 | Holdout 是消耗品 | 3 個互不重疊的 300 列分片（新聞加對話）。用 8 次或做一次決策後退役。只由全新的驗證者使用，且在打 tag 之前。 | 分片間的雜訊：最多 13 列（約 4%）。舊的 holdout 已被使用 ≥7 次。 | `docs/contracts/holdout-refresh.md`、`eval/holdout/` | 部分（測試集只跑一次，單一測試集） | 採用分片與使用次數計數 |
| H9 | 10-15 / 10-18 | 正規化後的污染防護 | 在簡轉繁與異體合併之後，對漢字連續片段做包含檢查；計數器跳過命中的列。 | 7,460 列資料池中 508 列命中。cvtune 13.3% 與 wikitune 22.9% 原文出現在訓練資料中；乾淨版本為 3,286/2,181。 | `tools/holdout_guard.py` | 有（8 字重疊排除） | 採用：在正規化之後檢查 |
| H10 | 10-13 / 10-18 | 跨版本基準與 KPI 表 | 每個版本以它自己出貨的模型量測；測試集雜湊；tag commit 以空的 `git diff` 綁定；KPI 列包含錯誤比、KySS、改寫次數與 holdout 兩半。這條規則存在但被跳過了好幾週，因此改為寫進工具。 | KPI v0.4.0→v0.5.0：discordtune 84.89→87.90%；wikitune-clean 70.38→75.29%。 | `tools/bench.py`、`tools/kpi.py`、`docs/kpi.md`、`docs/contracts/sbench-v2.md` | 部分 | **採用**為發布關卡 |
| H11 | 10-14 / 10-18 | 錯誤分類 | 類別：A1 缺讀音；B1 差異 ≥2 字；B2 不在清單中；C1–C5 排序錯誤；D1 重排改壞；D2a/b/c；E1/E2 上下文；F 離線正確。以 60 列做盲測檢查。修正方向依 列數 × 救回比例 ÷ 成本 排序。 | 60/60 一致。C 占錯誤的 89–93%。最優先的修正方向：更強的小模型（1,659 列）。 | `tools/triage.py`、`docs/triage.md`、`docs/contracts/triage.md` | 部分（`diagnose.py`） | **採用** |
| H12 | 10-04 → 10-26 | 使用者回報的回歸集 | 164 個真實錯誤，附上 App 當時看到的上下文。只作為線索，從不作為訓練資料；用作線索的列不計分。讀音檢查：畫面上錯誤的字串必須能由該列的讀音拼出。記錄 App、設定檔與版本。 | Chat top-1 停在 29，而 n 從 110 增加到 135。F 類比例依版本 41→27→11%。 | `eval/dev/user-reported.txt`、`docs/methodology.md` 10-13/10-21 | 沒有 | **採用** |
| H13 | 10-18 | 以 KySS 做真實引擎重播 | 逐鍵重播：第一次顯示是否正確、送出是否正確、KySS、每 100 字的重選次數、顯示後的改寫、學習鏈。 | dev302 chat KySS 0.9739。每鍵重排 vs 只在送出時重排：改寫 110 vs 17 次，送出結果相同。 | `docs/contracts/engine-replay.md`、`shanjie-replay` | 部分（`typing_errors.py`） | **採用** KySS 與閃動次數 |
| H14 | 10-18 / 10-21 | 人工輸入的讀音 | 維護者實際輸入的 200 句 vs `readings.py`（2.72% 不一致）。與讀音相關的功能需要人工讀音：P5 懲罰了 4/327 個正確答案的字。 | — | `tools/readings.py` | 沒有（我們的評測讀音來自最長匹配） | 採用 |
| H15 | 全程 | 突變測試、golden 一致性、指紋 | 每個測試都必須在突變下失敗（exit 101）。Python 參考實作逐位元組一致。`top1_sha256`。跨平台：候選完全相同或差距 ≤1e-9。選門檻時使用同一顆 CPU。 | 50,859 個 fuzz 過的標頭中 0 次 panic。 | `cli/tests/golden.rs`、`docs/verification.md` | 部分（`check_onnx`、Mac vs 遠端 0.17%） | 採用 `top1_sha256`，並對解碼器與 KeyChannel 做突變測試 |
| H16 | S2r | 探測集 | 相同的列改用另一個合法讀音（本調 ↔ 變調）。 | 作為變調相關工作的關卡。 | `experiments/s2/build_probe.py`、`eval/probe/` | 部分（`error_analysis` 群組） | 採用 |
| H17 | 10-13 / 10-17 | 模擬與消融的陷阱 | 全部一聲與空上下文的消融都超出分布。模擬器修的是自己的模型，而不是畫面。離線數字上下文使用了正確答案文字（在真實重播中第 196 列沒抓到）。 | 已記錄教訓。 | `docs/methodology.md` | 不適用 | 採用：消融保持在分布內；模擬畫面實際顯示的內容 |
| H18 | 10-13 | 工作流程分級 | 打錯字的列：只跑 golden test（6.8 s）。文件：一次審閱。程式碼：本機只測改動的部分，其餘交給 CI。 | 完整 `make test` 368 s。 | Makefile | 不適用 | 調整後採用 |

## I. 延遲與效能

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| I1 | 10-03 | 以純重構壓縮詞庫 | 輸出逐位元組相同，先寫防護測試。 | 392→88 MB；p95 1.65 ms。 | `docs/contracts/l.md` | 有（做法不同：mmap 29 MB） | 已涵蓋 |
| I2 | 10-11 | 推論後端調查 | Rust + Accelerate `cblas_sgemm` + vForce vs MLX vs Core ML；GPU 短時爆發。 | GPU 在短爆發上慢約 2 倍；閒置後 p95 ≈ 2× p50；以打字節奏量測 M 的 p50/p95 為 13.6/35.0 ms。 | 研究紀錄 10-11 | 有（CPU Core ML） | 採用打字節奏基準（間隔 150 ms） |
| I3 | 10-11 | 非同步兩階段重排 | 按鍵路徑建立工作；`.utility` worker 計算；主執行緒套用結果。以世代計數器丟棄過時的工作。 | 按鍵路徑 +0.16 ms（p95 2.65）。重排 p95 8.3 ms。改寫 14.5%→9.3%；來回翻轉 2.15%。 | `docs/contracts/s7c2a-core-rerank.md`、`s7c2b-shell-rerank.md` | 沒有（同步；25 音節 p95 33 ms，目標 30） | **採用** |
| I4 | 10-09 → 10-14 | 計時的基本功 | 移除每筆紀錄的字串配置（p95 6.9→2.9 ms）。記錄負載平均；只在同一個工作階段內比較；標明是最大值還是 p95。 | 一個「p95 308 ms」其實是最大值。 | 研究紀錄 | 部分（`latency.py`） | 採用 |
| I5 | 10-15 | 語言模型壓縮 | varint 無損；8 位元與 4 位元計數。 | 無損 83.4→28.6 MB。4 位元顯著變差（40/69）。 | `experiments/derisk-1015/` | 有（mmap） | 調整後採用：varint 只用於縮小下載大小 |
| I6 | 10-08 / 10-14 | 載入路徑與確定性 | 每次載入都檢查模型 SHA 要花 0.4–0.6 s（已移出）。不穩定的 XMP 記憶體造成位元翻轉（2^45 的配置）。`classes.sjc` 與順序有關。 | 3 台機器上重建結果逐位元組相同。 | `docs/contracts/model-v6.md` | 待確認 | 採用具確定性、以雜湊為錨的建置 |
| I7 | 10-22 | 組字上限（見 B21） | — | — | — | — | — |

## J. 使用體驗與設定

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| J1 | 10-08 → 10-17 | 重選後凍結 | 重選後其他詞保持不變。在詞中刪除會把它拆成凍結的單字。預測選字也會凍結左側。 | 第 4 版退步 1500→1438/2165，暫停。第 5 版模擬流程：chat 458→590（+151/−19，p 1e-26）；所有列 +220/−30。 | `docs/contracts/repick-freeze.md`、`core/tests/engine_repick.rs` | 待確認 | **採用**，並以模擬的刪除—重打—重選重播作為關卡 |
| J2 | 10-09 | Enter、Shift+Enter、Esc 行為一致 | Enter 依顯示內容送出尚未完成的注音；Shift+Enter 送出後插入換行；Esc 只關閉候選字視窗。 | 在 Discord 上收到回報後修正。 | `docs/contracts/enter-pending.md` | 待確認 | 採用 |
| J3 | 10-05 → 10-10 | 候選字視窗與 Apple 一致 | 以像素量測對照 Apple（`imeshot`）。透過 `viewEffectiveAppearance` 跟隨客戶端外觀。直式排列：9 列、28 pt 行距、寬度只增不減。 | 逐列與 Apple 相符。 | `tools/imeshot.swift`、`docs/contracts/candidate-vertical.md` | 待確認 | 調整後採用 |
| J4 | 10-10 / 10-15 | 設定視窗 | 行程內視窗（10.15 起 prefPane 顯示空白）、提供貼上用的編輯選單、側邊欄限制在 `visibleFrame` 內。 | — | `docs/contracts/settings-*.md` | 有 | 設定變多時調整後採用 |
| J5 | 10-05 / 10-20 | 安全輸入讓輸入法變灰 | 安裝在 `~/Library` 的輸入法在安全輸入期間會變灰；安裝在 `/Library` 的仍可使用。選單會顯示佔用安全輸入的 App。 | 在 v0.5.0 中以安裝選項出貨。 | `docs/contracts/system-install.md`、`tools/secure-input.swift` | 有（pkg 安裝到 `/Library`） | 已涵蓋；加入「誰佔用安全輸入」的提示 |
| J6 | 10-14 → 10-26 | 中英混合 | 先量測需求：22.8% 的訊息含有英文。SymSpell ∩ SCOWL（82,730 字）；空白觸發的視窗；Shift+字母輸入英文；依按鍵順序切換英文（取自 vChewing）。 | Recall 99.48%；誤觸發每 1,000 個音節 0 次。v0.5.0 預設開啟。 | `docs/contracts/zhen-*.md`、`english-sim` | 沒有 | 採用；依按鍵順序判斷的證據要以時間資訊把關，因為我們的解碼器會修正按鍵順序 |
| J7 | 10-17 | 數字列上的數字 | 以計數表加上 θ=3 的偏置決定是數字還是注音，延後判斷；表格在載入時建立。 | 90.8% vs 規則的 87.0%；延遲建立時第一個按鍵花了 367 ms。 | `core/src/digits.rs`、`docs/contracts/digits.md` | 沒有 | 採用 |
| J8 | 10-26 | 罕用詞排在常用字之前 | 変移 與 辨疑 排在 變 前面。 | overlay 中有 1,203 個日文字形。 | — | 待確認 | 採用此篩選 |
| J9 | 10-22 | 示範與自我測試方法 | 新舊版本並排輸入相同按鍵。單獨的執行檔沒有 bundle ID，因此上下文與學習在無聲無息中被關閉。 | — | `tools/imeshot.swift` | 沒有 | 採用此檢查 |

## K. 發布、CI 與更新

| # | 日期 | 名稱 | 做法 | 結果 | 檔案 | ZT 狀態 | 判斷 |
|---|---|---|---|---|---|---|---|
| K1 | 10-04 / 10-08 | Homebrew cask 的陷阱 | `postflight` 在沙盒中執行；`on_upgrade` 被當成字串讀取；以 `terminate_process` 修正。 | 舊行程在 0.03 s 內結束。 | cask、`docs/contracts/s3b.md` §14–15 | 不適用 | 不採用 |
| K2 | 10-09 / 10-21 | 更新時的 TIS 註冊 | bundle 被替換後重新註冊會讓 Caps Lock 切換失效。`TISEnableInputSource` 回傳 noErr 卻沒有效果。行程內的 TIS 清單會過時（2 分鐘，全新子行程則為 20 秒）。 | 改為已啟用時跳過註冊來修正。 | `docs/contracts/installer-v2.md` §9、`system-install.md` | 待確認（我們的熱重載重新啟動） | **採用**：已啟用的輸入來源永不重新註冊；從全新行程查詢 TIS |
| K3 | 10-08 / 10-16 | 版本說明與模型命名 | 版本說明放在 repo 中，缺少時 CI 關卡失敗。`--latest` 只用於 App 發布。SJ-XX-版本-規模 命名。`data/models.tsv` 提供「關於」頁面的模型清單。 | 已出貨。 | `docs/releases/README.md`、`.github/workflows/release.yml` | 沒有 | 採用 |
| K4 | 10-13 → 10-22 | 簽章、公證、Sparkle | Developer ID 只存在 CI 的 `release` 環境中。驗證 codesign、spctl、stapler、appcast 長度與 SHA。`umask 077` 會產生 0700 的 bundle，因此 staple 之後以 `tar -xpf` 解開並修正權限。Sparkle：只信任 EdDSA、不用阻擋式對話框、浮動視窗、≥2.10（安全公告）。 | 已出貨。 | `docs/contracts/about-update.md`、`release.yml` | 沒有（ad-hoc；ROADMAP §4.12） | **採用** |
| K5 | 10-09 → 10-20 | App 沙盒與獨立更新程式 | 容器遷移會搬移資料（selftest ID 會把資料搶走）。launchd 依路徑快取 XPC，因此完全移除 XPC。 | v0.5.0。 | `docs/contracts/app-sandbox.md`、`standalone-updater.md` | 沒有 | 暫不採用；導入沙盒前先閱讀 |
| K6 | 10-19 / 10-22 | VM 升級矩陣 | 從真實的舊設定開始測試，中間不重新開機（情境 a–g）。 | 抓到 XPC 快取錯誤。 | `docs/methodology.md` 10-19 | 沒有 | 採用 |
| K7 | 10-17 / 10-22 | CI 速度 | 拆成 5 個 job；Python 在 Linux 上跑；模型測試在 macOS 上跑（Linux 沒有 Accelerate，慢 2.4 倍）。 | 904→476–640 s；822→522 s。 | `docs/contracts/ci-speed*.md` | 待確認 | 調整後採用 |
| K8 | 10-12 | 模型完整性 | 在建置、CI 與發布時檢查 SHA-256 manifest。標頭有版本、有長度檢查，並經過 fuzz 測試。 | 50,859 個案例中 0 次 panic。 | `data/*.sha256`、`core/src/tinylm.rs` | 部分（`manifest.json`） | 採用：對我們的二進位標頭做 fuzz 測試 |
| K9 | 10-09 / 10-14 | repo 中的隱私衛生 | 某個 agent 的 User-Agent 洩漏了維護者的 email（約 40 個請求）。CLAUDE.md 從歷史中清除。每秒 1 個請求，`maxlag=5`。 | — | `LICENSES/data.md` | 待確認（我們的文件含有遠端主機的細節） | 採用 |

## L. 從對方紀錄學到的流程教訓

| # | 日期 | 教訓 | 證據 | 判斷 |
|---|---|---|---|---|
| L1 | 10-08 | 在做介面上的權宜修補之前，先檢查是不是缺詞 | 重選後凍結做了四輪；真正的原因是缺少 版控 這個詞。 | 採用 |
| L2 | 10-20 | 「依登記未通過」不代表這個方向結束 | 某個 agent 結束了口語方向；維護者重新開啟後 P4d 通過（確認 +69）。35 個舊的結案中有 4 個被重新開啟。 | 採用 |
| L3 | 10-13 | 沒有人遵守的規則就不是關卡 | 跨版本基準被要求「每次發布前」執行，卻被跳過了好幾週。 | 採用：把關卡放進 CI |
| L4 | 10-10 | 流程中的陷阱 | 依 locale 的 `sort -u` 合併了讀音（78→19）。共用的計數階段遺失了下游的篩選。評測用錯模型檔（v4 vs v5），因此先跑 `shasum -c`。CLI 在編譯時寫死的資料路徑讓 3 個平行格子的結果完全相同。zsh 不會對變數做分詞。 | 採用 |
| L5 | 10-09 | 不要忽視重複出現的使用者回報 | 「重現不了 N 次」在 Caps Lock 問題上是錯的；與使用者一起做單一變數二分法後確認了問題。 | 採用 |
| L6 | 10-21 | 在小型公開集上看到的建議性勝出不能當作證據 | P5「mc」變體在小型集上看起來乾淨，但正式登記後每個格子都改壞 1–6 列。 | 採用 |
| L7 | S3a 審閱 | 把緩衝區歸零只會帶來虛假的安心感 | AppKit 與客戶端 App 持有相同的文字；縮短保留時間才有幫助。 | 採用這個推論 |

---

## ZhuyinType 的前 15 項行動

依對整句準確率的預期影響排序，其次是使用體驗，再來是工程品質。

1. **從維護者自己送出的訊息建立私人真實聊天評測集。** 使用開始用 ZhuyinType 之前的月份。以盲測方式校對答案並以雜湊凍結。去除 markup 後刪除個資。較早的月份用於調參，每個決策使用一個較晚的月份。只記錄計數。
   - 原因：在善解的紀錄中，在 Wikipedia、朗讀語音或 LLM 測試集上的增益沒有轉移到聊天上（S2、S5j、S7b、S7f）。我們 chat 語言模型的 perplexity 比 Wikipedia 語言模型差，我們的 dev 集也有相同的弱點。
   - 善解：`docs/contracts/e2-chat-eval.md`、`experiments/e2-chateval/build_chatclause.py`、`judge.py`、`docs/iteration-plan.md` §2。
2. **以開放的口語文字取代我們薄弱的生成聊天語料，並作為獨立的項評分。** 來源：WildChat-TW、YODAS、Tatoeba、MASSIVE、OPUS zh_TW 字幕（42.8M 個漢字）、g0v。以窄範圍簡體篩選與兩輪 LLM 審閱清理。加入以詞圖首選為錨、帶軟性門檻的口語 bigram，而不是在解碼前混合。
   - 證據：P4d 確認淨 +69（p 1.1e-8）；同一張表在融合前套用則改壞 92 列。
   - 善解：`experiments/p2-data/`、`experiments/sj-lm-7/clean_subs.py`、`review_subs.py`、`core/src/colloquial.rs`、`docs/contracts/colloquial-term.md`。
3. **以 τ（只在詞圖不確定時）與 δ（切換門檻）為 v2b 重排模型把關。** 依設定檔以 改對 − 2×改壞 選出權重、τ 與 δ，並要求改壞的 Poisson 95% 上界 ≤ 0.5%（以 0.3% 調參）。先畫出改對／改壞前緣。可選擇提供強度等級。
   - 證據：R1 vs R3′ 的改壞率從 2.16% 降到 0.24%。我們的 `stats.py` 已經能計算這個上界。
   - 善解：`docs/contracts/s7e-fewer-breaks.md`、`rerank-strength.md`、`experiments/e2-chateval/judge.py`、`experiments/diag-near-miss/`。
4. **執行低成本的詞庫與語言模型稽核。**
   - 「每個詞單獨輸入時會贏嗎？」（對方 26.5% 的詞會輸）。
   - 期望值計數切詞（c(好,吧) 0→9；chat +15）。
   - 輕聲檢查集（+100/0）。
   - overlay 頻率上限。
   - 新字體、粵語與簡體篩選檢查。
   - 已是正體的文字上的 OpenCC 轉換。
   - 善解：`experiments/acg-audit/`、`tools/build_lm.py` / `build_counts_text.py --expected`、`docs/contracts/neutral-tolerance.md`、`experiments/s2/build_counts.py`、`docs/contracts/canto-prune.md`。
5. **加入個人學習的污染與鏡像測試，以及由此衍生的上下文規則。**
   - 句首條目需要下一個音節相符；單字需要 ≥2 個漢字的上下文；單字不進入全域。
   - 加入帶陰影規則的子片段新詞學習。
   - 為我們的 κ back-off 測試「教一次，下次就會」。
   - 善解：`docs/contracts/s4-learning.md` §12–13、`learn-single-ctx.md`、`newword-b.md`、`s4-bayes.md`、`core/tests/engine_learn.rs`、`engine_newword.rs`。
6. **重選後其他詞保持不變，包含從日後預測列選的字。** 以模擬的刪除—重打—重選重播驗證。
   - 證據：dev302 chat 上 +151/−19。較早的版本曾經退步，因此重播關卡很重要。
   - 善解：`docs/contracts/repick-freeze.md` §11–13、`core/tests/engine_repick.rs`。
7. **錯誤分類加上真實錯誤回歸集。**
   - 錯誤分類：不在清單中／排序錯誤／被重排模型改壞／重排模型正確但被門檻擋掉。
   - 保留使用者回報的錯誤，附上 App 當時實際看到的上下文，從不用於訓練。
   - 加入一小組人工輸入的讀音集。
   - 善解：`tools/triage.py`、`docs/contracts/triage.md`、`docs/triage.md`、`eval/dev/user-reported.txt`、`tools/readings.py`、`docs/methodology.md`（10-13、10-21）。
8. **以世代計數器讓重排非同步，並只在送出時（或閒置時）套用。** 這會把 v2b 與 v3 移出按鍵路徑；我們 25 音節的 p95 為 33 ms，目標為 30 ms。
   - 證據：對方的按鍵路徑 +0.16 ms；在送出時重排，送出結果相同，改寫次數從 110 降到 17。
   - 善解：`docs/contracts/s7c2a-core-rerank.md`、`s7c2b-shell-rerank.md`、`core/src/engine.rs`（`RerankJob`）。
9. **擴充重播測試工具：KySS、顯示後的改寫、每 100 字的重選次數、學習鏈。** 放進每個版本的 KPI 列，與準確率並列。
   - 善解：`docs/contracts/engine-replay.md`、`cli`（`shanjie-replay`）、`tools/kpi.py`、`docs/kpi.md`。
10. **加入名稱層與 IT/AI 詞包，並套用撞音規則。**
    - 來源：NSI 品牌、Wikidata、學校名錄、國教院詞彙。
    - 新詞永遠不得搶走既有詞的 top-1，且語料使用次數必須高於其同音詞。
    - 只加入解碼器目前會打錯的詞。
    - 以完整的出貨流程檢查（重排模型開啟）。
    - 善解：`tools/build_names.py`、`docs/contracts/names-lexicon.md`、`names-creators.md`、`it-pack.md`、`net-slang-2.md`、`acg-pack.md`。
11. **依對方量測過的設計建立預測列（Tab 補全）。**
    - 後繼詞優先，再接相容字串；起點最多往回 5 個音節；數字上下文表。
    - ↓ 與預測列一致；讀音完全相符的選字算作重選。
    - 量測各打字階段的 hit@9 與省下的按鍵數。
    - 善解：`core/src/predict.rs`、`docs/contracts/sp3-successor-first.md`、`v3-engine.md` §11、`predict-down.md`、`digit-context.md`。
12. **把測試集當成消耗性資源。**
    - 數個互不重疊的分片（對方的雜訊下限為 4%）、使用次數計數、用 8 次或做一次決策後退役。
    - 在簡轉繁正規化之後檢查污染。
    - 跨版本基準綁定 tag commit，並在 CI 中強制執行。
    - 善解：`docs/contracts/holdout-refresh.md`、`tools/holdout_guard.py`、`tools/bench.py`、`docs/contracts/sbench-v2.md`、`docs/verification.md`。
13. **以逐輸出通道 int8 縮小 v3/v2b，嵌入保留 fp32。** 只在通過配對 McNemar 非劣性檢定，且與 fp32 的 top-1 一致時才接受。加入跨按鍵的上下文狀態重用，以及退回詞圖的硬性期限。
    - 證據：對方的逐列 int8 在 0c 上失敗；逐欄通過 300/300。M 從 57.7→14.7 MB。
    - 善解：`experiments/s7a-tinylm/int8check.py`、`tools/convert_oc.py`、`core/src/oc.rs`、`docs/contracts/oc-experimental.md`。
14. **隱私與安全輸入路徑。**
    - 在安全輸入期間或封鎖清單中的欄位暫停學習與上下文讀取；顯示暫停狀態。
    - 加入標記字串測試，確保打的字永遠不會進入 log 或檔案。
    - 以只含數字的 log 檢查真實 App 中的上下文讀取（markedRange 錯誤）。
    - 善解：PLAN §7（R1–R10）、`docs/methodology.md` §7、`docs/contracts/system-install.md`、`tools/secure-input.swift`。
15. **發布工程，之後再做中英混合。**
    - 發布：
      - 在 CI 的發布環境中做 Developer ID 簽章與公證。
      - 版本說明放在 repo 中作為關卡；「關於」頁面列出模型清單。
      - 已啟用的 TIS 輸入來源永不重新註冊（Caps Lock 錯誤）。
      - 從真實的舊設定開始做 VM 升級測試。
    - 中英混合：先量測需求，再做空白觸發的英文視窗，並以模擬的誤觸發關卡把關（每 1,000 個音節 0 次）。中英自動空白與數字列上的數字是低成本的附加項目。
    - 善解：`.github/workflows/release.yml`、`docs/contracts/about-update.md`、`installer-v2.md` §9、`release-notes.md`、`zhen-mixed.md`、`zhen-inline.md`、`digits.md`、`cjk-spacing.md`。

**與 ZhuyinType 的差異：**
- 打錯字：ZhuyinType 有按鍵層級的計時解碼與 KeyChannel。善解把模糊音、聲調與相鄰鍵錯誤當成詞圖中的容錯讀音處理；在它的測試中，相鄰鍵關卡改壞了 5.3% 的乾淨輸入（B12）。
- 考慮讀音的模型：ZhuyinType 在詞圖內使用 v3。善解的因果與讀音前綴實驗在 14M 規模下沒有通過它的關卡，而 0c 相關的設計在善解的聊天集上分數不錯（C14、C15）。
- 上下文：ZhuyinType 讀取 128 字（v3 為 255）；善解的重排模型自 context-return 起最多讀取 64 字。
- 記憶體：ZhuyinType 以記憶體映射載入語言模型與詞庫（常駐 29 MB）。
- 重排模型大小：v2b 為 31M 參數；善解出貨的是 14M 模型，這是在較大模型沒有通過聊天測試之後的選擇（C7）。
- 共同點：兩邊的解碼器都依語言模型歷史合併路徑（B8）；兩個專案都有配對 McNemar、Poisson 上界與 MDE 工具（H1）；兩邊都提供 `/Library` 安裝，讓輸入法在安全輸入期間仍可使用（J5）。

**我們從對方學到的：** 上面大多數行動來自善解的評測流程（配對統計、私人真實聊天評審集、holdout 輪替、事先登記），以及它仔細記錄的負面結果。

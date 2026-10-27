# 善解（shanjie）：契約、實驗、核心、外殼與工具的閱讀筆記

感謝善解的維護者 Nanako0129 公開專案的契約、實驗、負面結果、工具與評測資料。這套事先登記的嚴謹流程讓我們學到很多，下文大多數想法都來自它。下文所有數字都是善解的數字（包括它對 0c 的量測），在善解自己的評測集、設定與硬體上測得，不能直接和 ZhuyinType 的數字比較。我們沒有複製任何程式碼。善解採用 Apache-2.0；我們閱讀它的文件與程式碼，並只在各檔案授權允許的範圍內使用它的評測集（見其 `LICENSES/data.md`），且註明出處。0c（靈犀注音）採用 AGPL-3.0：我們只為了量測而建置、執行它，並只閱讀它的文件。

研究的儲存庫：`references/shanjie`（程式碼為 Apache-2.0；資料採混合授權，見 `LICENSES/data.md`）。研究的版本：v0.5.0（2026-10-22），加上截至 2026-10-26 已合併但尚未發布的內容。

**本檔範圍：** 全部約 123 份 `docs/contracts/*.md`；每個 `experiments/*/` 資料夾（README、摘要、腳本 docstring；略過大型結果檔）；`core/src/{lib,lm,tinylm,colloquial,predict,learn,learn_store,oc,digits,english_next,trigram,engine,ffi}.rs` 的文件註解；`macos/Sources/*`；每個 `tools/*`；`data/`；`.github/workflows/`；`packaging/`；`scripts/`；`Makefile`。契約裡的數字多半是計畫值；實際結果取自對應的 `docs/research-log.md` 段落與實驗 README。`docs/research-log.md` 與 `docs/methodology.md` 由另一個 agent 以主題方式整理，本檔不重複那份回顧。

**路徑：** 除非路徑以 `ml/`、`engine/`、`platforms/` 或 `docs/` 等開頭（屬於 ZhuyinType），否則都相對於 `references/shanjie/`。ZhuyinType（ZT）的程式碼位於 `engine/swift/Sources/{ZhuyinCore,ZhuyinML,zhuyin-cli,ZhuyinCoreChecks}` 與 `platforms/macos/Sources/ZhuyinTypeApp/`。

**編號：** 前綴表示該列由哪一輪審閱產生（A = 詞庫／LM／解碼，B = 神經模型／profile，C = 資料／詞庫包／工具，D = 學習／預測／輸入／引擎，E = 外殼／隱私／發布／CI／評測）。多輪都找到的重複項目已合併，合併表在文末。

## 如何閱讀這些數字

- 善解的分數是 **log10**，我們的是自然對數。log10 的 δ 1.0 約等於 2.3 nat。
- **fixed/broken（改對／改壞）** = 某項變更相對於配對基準改對／改錯的列數；**net** = fixed − broken；除非另有說明，**p** 為精確雙尾 McNemar；**UB** = 改壞率的 95% Poisson 上界。
- **Profile：** 善解有兩個固定 profile，**chat** 與 **formal**，依 App 選擇。大致對應我們 chat／wiki 混合權重 c。
- **評測集：** dev302（手寫開發集）、typing76（真實打字）、錯字回報／user-reported（維護者的錯誤回報，單向）、cvtune 與 wikitune（Common Voice／Tatoeba 與 Wikipedia 調參集，讀音以規則產生）、lytune（立法院發言，4,811 列）、discordtune（維護者私人的 Discord 聊天，4,958 列）、chatclause／chatjudge／chatjudge-2（維護者自己的訊息，分成調參集與一次性評審集）、holdout（227 列，之後改為 3 份各 300 列的分片）。
- **他們的架構**（v0.5.0）：McBopomofo 基礎詞庫加上 overlay 與詞庫包 → 詞 bigram（model-v6，9.1M），以 Brown 類別 backoff、Kneser–Ney continuation 附屬檔、僅 formal 使用的 trigram（7.7M）→ n-gram top-8 → 14.4M 字元 LM「會意」SJ-SR-6-14M（6L/384，int8），以閘控融合（α、τ、δ）加上口語項與讀音通道項。核心以 Rust 撰寫；外殼為 Swift InputMethodKit。

## 摘要

**與 ZhuyinType 的差異。** 兩個專案做了不同的模型選擇。善解的重排模型是 14.4M 的因果字元 LM，context 最多 2 個漢字（context-return 之後為 64）。ZhuyinType 則在 lattice 內使用以音節為條件的模型（v3）、按鍵層級的錯字與時序解碼器，以及 KeyChannel。善解以配對統計測試了更大的模型、蒸餾、LLM 評審與貝氏學習，並公開負面結果，這些對我們也是有用的證據。

**我們從中學到的。** 善解的**流程與產品工程**嚴謹且文件完整，也是我們最值得學習的部分：

- **統計：** 每項變更都做配對 McNemar 與 bootstrap、設改壞率上限、輪替 holdout，以及私人真實聊天評審集。
- **閘控融合：** 只有在 n-gram 差距夠小（τ）且重排模型自身增益夠大（δ）時，重排模型才能改動句子。
- **行為對齊 Apple 注音：** 重選凍結、在組字中輸入數字、英文連打、標點名稱。
- **範圍窄、測試充分的個人學習：** 鏡像測試與污染測試。
- **資料包：** 具碰撞閘門的詞庫包、人名層，以及針對台灣的語料轉換修正。
- **macOS 底層：** 安全輸入隱私閘、不影響 Caps Lock 的安裝、強化過的 Sparkle、經公證的發布流程、含 mutation test 的 CI、KPI 表。

下文標為「採用」的項目多半是低成本的工程。模型面最大的三個教訓：

- 以改壞數而非平均準確率來閘控重排模型。
- 把口語證據加成一個相對於 top-1 的閘控重排特徵。
- 以期望（forward-backward）計數訓練 LM，而不是單一硬性斷詞。

列數：五輪審閱共 249 筆原始項目，合併重複後為 228 筆。

## 神經模型章節詞彙表，與 0c 移植說明

- **M／「會意」`sj-sr-6-14m`**：他們出貨的重排模型。字元層級因果 decoder，6 層，d 384，6 heads，14.4M 參數，詞彙 9,717（9,714 漢字 + PAD/UNK/BOS），ctx 64，pre-LN，tied embedding，GELU-tanh。int8 `.sjtl` 14.6 MB。輸入的 context = `context_key` = **≤ 2 個漢字**（遇到任何標點或非漢字即停止）。訓練 1 個 epoch，255M tokens（94.3% Wikipedia，口語 ×5）。
- **融合 `pick(ng, ll, α, τ, δ)`**：只重排 n-gram top-8；若 `ng[0]-ng[1] ≥ τ` 則保留第 1 名；否則 j = argmax(ng + α·ll)；只有在 fused(j) − fused(0) ≥ δ（log10 單位）時才切換。
- **fixed／broken（改對／改壞）**：相對於基準，該變更改對／改錯的列；「net」= fixed − broken；「改壞率」= broken ÷ n；「UB」= broken ÷ n 的 95% 精確 Poisson 上界。
- 評測集：**discordtune**（維護者真實的 Discord 聊天，4,958 列，開發層，已多次重複使用）、**chatclause**（維護者自己的訊息，調參 5,761／評審 6,559）、**chatjudge／chatjudge-2**（全新的一次性私人評審集，2,972／5,783）、**cvtune／wikitune**（Common Voice+Tatoeba／Wikipedia 調參）、**dev302**、**typing76**、**user-reported**（錯誤回報集，單向）、**lytune**（立法院口語發言，4,811）、**wikitypo**（155）、**wikijudge1-4**（全新的 Wikipedia 條目區段）。
- **我們的**：v2b 重排模型 = 字元因果 Transformer 8L/512，31M 參數，12k 詞彙，ctx 64，top-16，`score = engine + 1.0·logP`，只有 +4.0 的「按鍵完全相符者優先」差距（`engine/swift/Sources/ZhuyinML/NeuralReranker.swift`，沒有 τ/δ 閘門，也沒有已學詞保護）。v3 = 音節→字元模型（6L/512 因果 context encoder ≤255 字 + 6L/512 雙向音節 encoder），在 lattice 內以權重 0.5 加入每個位置的 log P(char | syllables, context)。

### 「0c」說明（他們對 Funmula 0c 模型的移植）

**它是什麼。** 0c（靈犀注音 0c，BigGo／Funmula 開發）是一套 macOS 注音輸入法，架構與善解幾乎相同（詞庫 + Viterbi 候選 → Transformer 重排 top-8，context ≤64 字）。它的重排模型 `Funmula/0c-sequence-d512`（HF revision `d512-pf512-20260926`，commit `66eb4a81…`，`model.onnx` 130,723,134 bytes，SHA `71af1f0e…`）的**權重採 MIT 授權**；輸入法 App 原始碼（`github.com/0c-biggo/0c-ime`）採 **AGPL-3.0**（善解只閱讀，從未複製程式碼）。他們公開的數字：6,000 句台灣新聞句子，讀音由字典合成，統計引擎 top-1 76.9%，加上 Transformer 後為 86.0%；P95 約 7–16 ms，超過 100 ms 時退回詞庫順序。他們自己的融合方式：engine + 2 × model，差距 > 0 時切換。

**架構（由 ONNX 圖逆向整理，404 個節點，見 `tools/convert_oc.py` 檔頭）：**
- 每個序列的輸入：`ids, bases, tones, types, positions`、causal mask、`proj_pos`、`target_node/target_ids`。序列排列：`[BOS] context[-64:] [SEP] reading×n [OUT] candidate[:-1]`；分數取自 `[OUT]` 起的 n 個位置（因此模型在輸出任何字之前**已看過整個子句的讀音**，計算的是 P(candidate | context, full reading)）。
- Embedding：讀音位置（`types==1`）使用 `base_embedding[base] + tone_embedding[tone]`（聲母＋韻母「base」與聲調分開表示），其他位置使用 tied 的 `decoder.weight[id]`；再加上 `type_embedding`；接著 LayerNorm（只有 weight，eps 1e-5）。
- 8 層，d 512，8 heads × 64，q/k/v/o 512×512 無 bias，**RoPE**（rotate-half），第 0 層 attention 前沒有 LN（第 1–7 層有）。FFN：LN → 512×1536 → 切半 → GELU(erf)(前半) × 後半（GEGLU）→ 768×512，殘差。
- Head：最後的 LN → 在 `proj_pos` gather → dense 512×512（無 bias）→ GELU → LN → × tied `decoder.weight[out_ids]ᵀ` + bias `val_779[out_ids]` → 對 27,859 欄做 LogSoftmax（`out_ids` = 除 0–3、5、7 以外的所有 id；字元 id c ↦ 第 c−6 欄）。32,638,433 個參數；詞彙 27,857 字（M 為 9,714）。
- 計分規則（`experiments/x0c/score_0c.py`）：最後一個音節若其 base 在 `neutral_tolerant_last` 中，另外以輕聲再試一次（取較大值）；音節數不在 2–32 之間或含 OOV 字元的列會略過（佔 1–2%）；batch 固定為 1，因此 8 個候選以前綴樹方式打包（與逐一計算相差 ≤ ln 3.1e-5）。
- 值得了解的 App 行為（讀自 AGPL 原始碼，僅以自己的話重述）：在按鍵當下**同步重排，期限 100 ms**；以**逐候選否決**保護使用者的選字（只排除會改寫使用者所選區段的候選），而不是整段略過；context 只取輸入法自己送出且經確認的文字，≤64 字；一個只記數字、預設關閉的飛行記錄器。

**善解的移植（`oc-experimental.md`、`core/src/oc.rs`、`tools/convert_oc.py`、`tools/oc_ref.py`）：** ONNX → 自訂容器 `SJOC0001`（120 byte 檔頭，含來源 SHA、vocab／bases／輕聲清單／out_ids，f32 張量 4 byte 對齊）→ numpy 參考實作（與 ORT 相比 max |Δlog10| 3.15e-5，300/300 argmax 相同）→ 零相依的 Rust 前向計算（macOS 上用 Accelerate `cblas_sgemm`，libm `erff`，權重**直接從唯讀 mmap 讀取**，因此頁面計入 RSS 但不計入 phys_footprint）。int8 未通過一致性檢查（296/300，p99 0.437 log10；最佳變體為逐輸出欄量化 + f32 embedding，300/300 但 p99 0.109 且約 76 MB），因此使用 f32 130.7 MB mmap。逐候選前向計算在 64 字 context 下測得 p95 74.5 ms、RSS +273 MiB（兩者都超過 50 ms／250 MiB 的閘門）；加入**前綴 KV 共用**（context + 讀音 + [OUT] 每列只算一次，每個候選只跑自己的 n−1 個字）後，p95 為 33.9 ms（64 字 context）／27.2 ms（無 context），RSS 峰值 +227.5 MiB（0c 本身 +146.2），Rust 與 ORT 相差 4.07e-5，300/300；7 + 7 個 mutation test 都被抓到。規劃為預設關閉的「實驗開關」（slice B 未完成：依他們對第三方／下載權重的 R8 規則需要 XPC helper，以及在檔案損壞時停用的 `try_score`）。

**在他們資料上的結果：** X0c（相對於他們的 tinylm-v3，chat，0c 參數 α 1.5 τ 1 δ 2）：評審期 net +142，discordtune +127（189/62），user-reported +11，但 **wikitune −88、lytune −56**（偏向聊天）。P1（目前的解碼器）：在 discordtune 上 0c 對出貨的 M 為 165/60，net **+105**，p 1.6e-12；在可修正的列上單次選字準確率 66.5%，M 為 45.1%，保留率 91.6% 對 86.8%；它 64 字的 context 只貢獻約 +20 列（47 次選字改變）。五次蒸餾到 M 大小學生模型的嘗試都失敗（見第 26–28 列）。維護者選擇不把 0c 設為預設（「我們會失去自己的研究，也不能說是自己的成果」）；它被用作量尺與教師模型。

**與 ZhuyinType 的關聯：** 0c 是與我們 v3 最接近的公開對照（以讀音為條件、長 context、8L/512 約與 v2b 同大小）。它是一項外部證據：約 32M 參數的 P(cand | context, full reading) 在善解的聊天集上得分遠高於純字元 LM；善解自己在 14M 規模下以讀音為條件的小型實驗沒有達到這個結果（B29、B30）。我們使用相近的形式（v3），但還沒有和 0c 在共同的測試上量測；它採 MIT 授權的權重使它成為有用的參考模型與教師模型。


## 1. 詞庫、語料與詞庫包

基礎詞庫、overlay、語料轉換、讀音、選用詞庫包、人名、流行語、授權、模型登記。

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| A1 | 以 McBopomofo 為底的 unigram 基準 | s0.md | 將 Python 原型移植到 Rust。在 McBopomofo `mcbpmf-data.txt`（MIT）上做 beam N-best（BEAM 32，PER_KEY 12），並有逐 byte 相同的 golden。 | Unigram 在既有 109 列中對 74 列；教育部辭典例句 57.3%；每句 p95 低於 5 ms。之後：dev302 unigram top-1 52–53%，oracle@32 96.7%。 | 有（做法不同：LM + v3 + rerank） | 不適用。只當作下限：unigram 在他們的開發集上約 52%，LM 能提升到約 80%。 |
| A3 | Wikipedia／Wiktionary 標題 overlay 詞庫 | s1.md（V0–V3、V2t） | 以 CC BY-SA 標題補上 OOV 詞（氣泡水、空氣炸鍋、行動電源…），分數取同長度的第 25 百分位（2 字 −7.17）。收錄所有標題（V3，1.04M 詞）的效果比篩選過的複合詞子集（V2t，343k）差。 | 沒有任何單一來源涵蓋 ≥50% 的 25 個開發集 OOV 詞（wiki 標題 44%）。V2t：oracle@64 97.7→98.3，top-1 51.7→54.0%（dev302）。**Holdout 未能重現**：top-1 94→93/227，且 OOV 子集原本就已達 25/25 上限。 | 有（做法不同：CC-CEDICT + wordfreq） | 調整後採用：OOV 詞來源值得做，但專有名詞標題會污染候選。篩選為複合詞，並以 holdout 評判，不以驅動篩選設計的開發集詞評判（holdout 結果顯示，部分開發集增益來自於在這些詞上設計篩選規則）。 |
| A4 | 精簡的共用詞庫排列 | l.md | 音節 id 內部化、entry 陣列與 UTF-8 詞池，以二分搜尋查找；輸出逐 byte 相同。 | 含 overlay 時峰值 RSS 375 MiB → 87 MiB，不含時 127 → 37 MiB。 | 有（做法不同：ZTLX/1 mmap） | 已涵蓋。 |
| A7 | 轉換前以句子為單位分類簡繁 | s2n §2.1、experiments/s2n | Wiki 是簡繁混雜。只轉換純簡體字會讓 之后／由于／哪里 留在語料中（單獨的 后 加權計數 150k）。修正：逐句分類；簡體句套用完整的 STPhrases + STCharacters + TW 字形；繁體句只轉換純簡體字。針對 皇后／公里／里長／台灣／干涉 設保護計數。 | 在 500 篇樣本中殘留由 125,635 降到約 70（4.2%），在 model E 中為 0。正確字形增加了移除量的 +92%。受保護詞維持在 ±2% 內（里長 +16.5%）。 | **沒有／有風險**：我們對每一句都執行 `OpenCC("s2twp")` | **調整後採用**：檢查我們的 wiki 語料是否有 s2t 作用在已是繁體的文字上造成的 裡長／鄰裡／皇後／幹涉／後里→後裡 類損害，再於 `pipeline.py clean()` 加入他們的簡繁句子閘門。 |
| A8 | 台灣字形表（TWVariants）與受保護詞 | s2f-variant-forms.md、experiments/s2f | OpenCC STCharacters 會把 床→牀、秘→祕、灶→竈，而且連繁體句也套用，於是 起床 輸給 起牀。修正：一張 `TW_VARIANTS` 表，含 4 個例外，最後套用。有五個字（脣 泄 棱 覈 齶）受保護，因為全面套用 TWVariants 讓 排泄（422→0）與 棱錐 歸零；竈門 只在繁體句中受保護。 | Model E→F4：床／牀 0/13,897 → 13,879/0；字形探測在兩個 profile 都是 16/16；guard：沒有否決（dev302 +1）。F→F2→F3→F4 每一輪修正都需要約 11 分鐘的完整重建。 | 部分有（s2twp 包含 TWVariants），但可能遭受受保護詞損害（排泄→排洩 等） | 調整後採用：在我們的語料中統計 排泄／排洩、嘴脣／嘴唇、棱／稜。若教育部標準詞計數為 0，加入這 5 字保護清單。 |
| A9 | 計數時合併同音異體字 | s2f §2.4、§6.2、§7.2 | 佔／占、佈／布、祕／秘、臺／台 等，只有在成員讀音集合完全相同時才合併為同一計數類別。Bigram 質量歸給一個代表成員，N 與 eos 每類只計一次。 | 第一次嘗試（寫回每個成員）讓 t(v) 倍增：900 個負的 backoff 權重，N +1.46%，以及 9 個近乎平手的翻轉（世界綫）。修正後的 F2–F4 沒有否決。 | 沒有（異體字只透過 OpenCC 處理） | 調整後採用，優先度低。若要合併異體字，不要把類別總數寫回每個成員：t(v) 會被灌大。 |
| A10 | 從 overlay 移除簡體條目 | s2n §2.2 | 之后／最后／由于 等 Wiktionary 標題進入詞庫，解碼器因此能直接輸出它們。當基礎詞庫已有轉換後字形、且該條目不是教育部辭典字詞時移除。共移除 3,730 筆。 | 使用者句子「十分鐘後到」修正（與 #6 一起）。 | 不適用（CC-CEDICT 繁體欄） | 不採用。我們的風險在語料（#7），不在詞庫。 |
| A11 | 以 zhconv-rs 做 MediaWiki zh-tw 轉換（NoteTA、-{H}-、CGroup） | s2w-mediawiki-zhtw.md、experiments/s2w | 重現 zh.wikipedia 的 zh-tw 頁面轉換：站台轉換表、NoteTA 群組與行內規則以 `-{H\|…}-` 置於前面，再交給 zhconv-rs。之後還需要一輪台灣字形處理（處理前 爲 82.7/M、裏 38.7/M 字）。 | 閘門 1：人工評判 100 筆差異，MW 較好（通過符號檢定）。在無偏的 cvtune-native-v3 上 Model W2 對 v3：chat 4/1，formal 6/3（不顯著）。有偏的評測集結果不一致：wikitune-mw +17/+21（p ≤ 0.006），舊 wikitune −9/−3。**未出貨。** | 有（較簡單）（`variant()` 只取行內 -{}- 的 zh-tw 選項） | 不採用：沒有無偏的增益。教訓：參考答案由某種轉換產生的評測集，會偏好該轉換。 |
| A13 | 教育部 一／不／法 變調異讀與例外（R1–R5） | s2r-sandhi-variants.md、research-log 2026-10-04 | 為 一／不 加上另一個合法讀音（本調↔變調）。一 在 第／數字之後、數字／月之前、詞尾、重疊動詞之間保持本調，另加 133 個「詞尾 一」詞（統一、單一…）與 41 個列舉項目（北一女、查理一世）。另有 法 ㄈㄚˋ→ㄈㄚˇ。50 列稽核允許 ≤2 個規則違反。 | 探測（輸入另一個讀音）：cvtune 454→490（39/3，p 5.6e-9），discordtune 652→714（64/2，p 6e-17）。原讀音：−1/−2，不顯著。RSS +1.5 MB。 | 有（一／不 變調列） | 調整後採用：對照他們的例外（第一、數詞、統一 類詞尾 一、重疊）檢查我們的產生器。錯誤的異讀會製造假同音詞（一人 ㄧˋ = 藝人 弄壞了一列）。 |
| A14 | Overlay 讀音正規化（和 ㄏㄢˋ→ㄏㄜˊ、不 ㄅㄨˊ 修正） | s2r2-overlay-readings.md | 以貪婪斷詞猜出的讀音有系統性錯誤：813 列在非去聲前的 不 ㄅㄨˊ，576 列人名中的 和 ㄏㄢˋ。予以正規化，並以 −0.5 加入變調列。overlay 中 36.3% 的破音字讀音是猜的。 | 改動 2,195 列，新增 2,933 列。探測 cvtune +3/0，wikitune +2/0。修正 6 個實際案例（三不沾、上和下睦…）。 | 不適用（CEDICT 每個條目各有讀音） | 不採用，但要注意：連接詞 和 的 ㄏㄢˋ 讀音必須限定詞義（我們已經這樣做）。 |
| A15 | 口語非教育部讀音（紮 ㄓㄚ，R6） | zha-reading.md | 任何字典都沒有的讀音會讓該字打不出來，個人學習也幫不上忙（oracle@64 = 0）。新增 29 列；原則：口語讀音需逐字核准。 | 錯誤回報集 oracle 153→154；所有閘門集逐列相同（0/0）。登記的「net > 0」閘門在形式上未通過。 | 部分（Unihan／CEDICT 可能涵蓋一部分） | 調整後採用：在我們的錯誤回報列中找出 oracle@N = 0 的列，視為缺少讀音，而非排序錯誤。 |
| A39 | 未見 overlay 懲罰與語料上限 | model-v5.md §0、§2.4、§9 | Overlay／詞庫包詞：若在語料中出現過，分數 = min(檔案分數, log10(c/N))，否則為檔案分數 − 1.0。語料中出現過的罕見人名反而被壓到檔案分數*以下*：宿儺 期望計數 2 → −7.86 < −7.17，因此輸給 素娜。 | 懲罰掃描 1.0/1.5/2.0/3.0：tune +4/+7/+5（選 2.0），但 3 字詞庫包人名的損失由 16.5% 升到 29.7%。**維持 1.0。** | 做法不同（wordfreq 分數） | 教訓：以語料頻率限制詞庫先驗，會扼殺罕見專有名詞。為整理過的人名清單設下限。 |
| A40 | 擴充斷詞詞彙時採加法計數 | model-v5.md §8 | 將 30k 個 ACG 名稱加入斷詞詞庫，搶走了常見字對的計數（萊+恩 1,336→0，拉+斯 11,964→2,849）。修正：計數兩次，保留基礎詞庫計數不變，只加入詞庫包詞的 unigram 與涉及它們的 bigram。並排除讀音衝突的詞。 | 修正前：wikitune formal −21（30/51，p 0.026，否決）。修正後：沒有否決，s2r 探測恢復。 | 目前不適用；若在 `pipeline.py` 斷詞中加入領域詞則相關 | **採用此規則**：每次擴充 hard-EM／期望計數詞庫時，以加法計數，不重新斷詞。 |
| A42 | 以罕見口語詞為種子的 LLM 補充語料 | model-v6.md §2.1b | 以罕見口語詞與量詞為種子，由 gpt-oss-120b 生成句子。篩除簡體字、與評測集重疊者，以及「入侵」（中國用語）清單。權重 ×5。 | 500k 字花費 US$2.87；篩選移除 258 筆重複、90 筆評測重疊、312 個中國用語。原始 S2：合成口語 k=5 讓 dev302 219→231、typing76 58→65；k=10/20 較差。 | 有（做法不同：5.9M 句 Qwen3-8B 合成） | 量上已涵蓋。採用他們的篩選（中國用語清單、評測重疊），以及 ×5 權重勝過 ×10／×20 的發現。 |
| A43 | 真實口語語料：PTT 推文／內文、g0v CC0、OpenSubtitles zh_tw | sj-lm-7.md、experiments/sj-lm-7 | PTT（推文 ×5、內文 ×1、每單位上限 50 句；同一帳號 3 分鐘內的連續推文合併）、g0v #rand0m（CC0）、OPUS OpenSubtitles 原始 zh_tw，以 Sonnet 審閱的句子篩選。繁體句略過 TWPhrases（保留 插件）。 | 凍結大小：PTT 內文 5.34M、推文 8.67M、g0v 15k、字幕 4.43M 句。字幕殘留經 7 次校對由 6.7% 降到 1.7%。模型結果待定。 | 沒有（路線圖：「真實聊天語料」） | 採用（資料）：PTT 推文與字幕是最便宜的大型真實聊天來源。沿用他們的清理規則與殘留檢查。 |
| A44 | 以私人聊天為目標的 Moore–Lewis 篩選 | sj-lm-7 §2.4 | 字元 4-gram KN，領域內（私人聊天 7/8 切分）對領域外樣本，雙向交叉計分，每個來源保留最佳的 50%。保留 1/8 訊息 ID 雜湊桶供未來評審集使用。 | 待定。 | 沒有 | 調整後採用：可以用我們自己的聊天開發集為 PTT／字幕計分，決定加權對象。 |
| A45 | 教育部簡編本讀音優先；日本新字體篩選；倒置的佔位值 | sj-lm-7 §2.3（第 4 版） | Overlay 讀音先取簡編本（口語讀音），其次為 MoEDict 第一讀音，最後才用規則。含日本新字體且不在教育部辭典中的詞被移除（9,267 個，例如 一挙両得）。修正 9 個以倒置 −6.8 佔位值處理的破音字。輕聲詞另加一列本調版本。 | 讀音稽核：簡編本 32/33 正確，MoEDict 31/33，規則 22/34（未通過，之後修補）。新字體篩選：0/50 誤刪。 | **沒有**：CC-CEDICT 讀音源自中國大陸（只有標註時才有「Taiwan pr.」） | **採用**：以簡編本（CC BY-ND，只作為建置輸入，從不再散布）覆寫 CEDICT 的破音字讀音，並移除只屬於日文的字形。 |
| A46 | 刪除粵語字條目 | canto-prune.md | 移除含 33 個粵語字（冇 嘅 啲 喺 佢…）的 overlay 詞，保留教育部辭典字詞。俾／咩／喎／嘞 視為華語保留。 | 移除 721 詞；所有評測集都沒有變差。 | 不明（CEDICT 很少） | 調整後採用（低成本稽核）：在我們的詞庫中 grep 這份字表，移除不在教育部辭典中的結果。 |
| A47 | 新聞語料切分：以政府新聞稿 + Wikinews 訓練，以報紙驗證 | news-corpus.md | 政府新聞稿（著作權法第 9 條）與 Wikinews（CC BY 4.0）用於 formal 訓練。Wikinews 以標題雜湊首 byte < 32 切出驗證集。一份報紙（沒有訓練權利）只作為私人驗證集。 | 事先登記 N00–N12 網格；結果待定。 | 沒有（wiki 是我們唯一的正式語域） | 採用：政府新聞稿是免費、正式、反映當前台灣用語的資料，適合 wiki 端 LM。 |
| A49 | 模型指紋條目 | model-v4.md | 在公開的 bigram 中放入私人 canary 條目，用以辨識再散布。已確認受影響情境的解碼與預測不變。 | 12 種解碼設定逐 byte 相同；holdout 0/0。 | 沒有 | 選用；除非在意再散布，否則不採用。 |
| C1 | 選用詞庫包（可切換的層） | `docs/contracts/acg-pack.md` A.2、`it-pack.md` §3、`meme-pack.md` §3；`data/packs/*-add.tsv` | 每個領域詞庫包是獨立的 overlay 檔 `reading<TAB>word<TAB>log10score<TAB>tag`，有自己的版本與 manifest（`acg.json`、`it.json`）、位元遮罩（`PACK_ACG=1`、`IT=4`、`MEME=8`），預設開啟，可在設定中切換；切換時重建引擎（詞庫不可變）。測試：詞庫包關閉時與沒有詞庫包檔案逐 byte 相同（解碼、候選、預測）；把切換改成無作用的 mutation test 必須失敗。選單顯示引擎實際載入的遮罩，而不是偏好設定值。 | ACG 詞庫包 45,016 列（44,710 `acg` + 306 `acg-oov`），2.7 MB；RSS 約 +20 MB（262.7→285 MB，門檻 40 MiB）；IT 詞庫包 1,021 列。 | 沒有 | 採用：`data/packs/<name>-add.tsv` 在載入時併入 mmap 詞庫，遮罩放在偏好設定；先出 IT 與人名。 |
| C2 | 從 zhwiki 轉換群組 + 條目取得 ACG 詞 | `acg-pack.md` A.1、A2、A3；`data/packs/acg-groups.tsv`、`acg-exclude.tsv`、`acg-manual.tsv`；`tools/build_acg_pack.py` | 來源：`Module:CGroup/*` 的 zh-tw 值（只取人工清單中標記 `include` 的群組；排除電影／電視／音樂／名人；未知的新群組不收錄並回報）、前 400 部動畫作品的 zh-tw 顯示標題（Wikidata sitelinks）、每年的「YYYY年日本動畫列表」（2020–2026，只取標題欄，依 rowspan 對齊）、以 `action=parse&variant=zh-tw` 解析的角色段落。嚴格的名稱篩選：原文假名／拉丁名必須緊接在名稱後的括號中；所有出現處都要判斷；間隔號名稱拆成各部分；角色列表頁以整頁計；標題後綴篩選（女王、王子…）只用於新詞。 | 原型：Wikidata 作品 142 筆 chat top-1 113→120，角色 227 筆 152→161，在 dev302／打字／錯字集上 0 退步。A2：使用者回報的 9 個近期漏字修正了 8 個。A3 間隔號集 chat 21→40/45（閘門 30）；A3 找到 5,180 個新的間隔號名稱 + 6,991 個列表頁名稱。要求抽樣精確度 ≥90/100。 | 沒有 | 調整後採用：用同樣的 Wikipedia + Wikidata（CC0）來源做 ACG／遊戲詞庫包；不用萌娘百科（NC）。 |
| C3 | 可重現的離線建置 | `acg-pack.md` A.1「可重現」、`it-pack.md` §2a、`names-lexicon.md` §2 | 擷取步驟與建置分開；每個請求快取成一個檔案（`~/.cache/shanjie/sources/<pack>/`），`--offline` 在快取缺漏時中止；≤1 req/s、具描述性的 User-Agent、`maxlag=5`、主機允許清單、請求數（3,000–5,000）與快取大小（0.5–1.5 GB）上限。版本 = `<latest source revision date>-<sha256[:8] of add.tsv>`（從不使用建置日期或系統時鐘；`LAST_YEAR` 是具名常數）。相同快取 → 逐 byte 相同的輸出，有測試。 | 逐位元重建抓到出貨後才加的讀音覆寫（唯）以及 A4 的解碼器漂移（10 詞）。 | 部分（ZT 在 `ml/src/zhuyintype_ml/training/lm*` 中固定了部分雜湊） | 採用於每個 ZT 資料建置工具（`ml/src/zhuyintype_ml/data/tools/build_lexicon.py`、未來的詞庫包建置工具）。 |
| C4 | 逐詞出處檔 | `data/packs/acg-sources.tsv`、`it-sources.tsv`、`data/lexicon/names-sources.tsv` | 每個詞記錄其來源頁面 + revision id（或 NSI 檔案 + commit、教育部學年度、Wikidata QID）。用於標示出處、稽核擷取錯誤，以及 90% 精確度抽樣。 | ACG 來源 46,313 列；人名 2,119 列。 | 沒有 | 採用：每個詞庫包一個 `*-sources.tsv`，不隨 App 出貨。 |
| C5 | 以字串跨層去重（不論讀音） | `acg-pack.md` A.1「去重」、`names-lexicon.md` §2；`build_names.py --check` | 若字串已以任何讀音存在於任何一層，就捨棄該詞庫包詞，因為核心依字串限制條目（`CappedLexicon`），而重複的 (reading, word) 會讓核心拒絕載入（`OverlayDuplicate`）。有測試讀取已提交的檔案並斷言交集為空。 | 人名因 `in_lexicon` 捨棄 317 個。 | 不適用（目前只有單一詞庫檔） | 有詞庫包後採用：建置時檢查 + CI 測試，確保 pack ∩ base = ∅，或明確定義覆寫語意。 |
| C6 | 以實際解碼在建置時偵測碰撞 | `acg-pack.md` A.1 (a)(b)(c)；`tools/build_acg_pack.py detect_collisions` | 每個詞庫包詞的讀音以剛建好的評測 CLI 解碼兩次（詞庫包關／開，chat 與 formal）。只有在 top-1 改變且符合下列條件時才列出：(a) 另一個詞庫包詞勝出（包內同音詞），(b) 原本的 top-1 在「參考清單」中，該清單包含篩選前見過的所有來源名稱（例如 楓之谷 搶走 風之谷），(c) 該詞庫包詞把既有詞庫詞擠下 top-1 → 自動捨棄，最多重跑 5 輪。Top-1 不變 = 只是多一個候選，不處理。 | v1 依 (c) 捨棄 5 個（卡札克、小馬鎮…）；A3 找到 254 個新的 (a)(b) 群組（閘門：>50 時詢問如何批次處理）。 | 沒有 | 採用：ZT 可以對每個讀音執行 `zhuyin-cli`；這是防止詞庫包破壞一般打字最有用的一道防護。 |
| C7 | 人工處置檔 + 合併閘門 | `data/packs/acg-collisions.tsv`（388 行）、`it-collisions.tsv`、`data/lexicon/names-collisions.tsv`、`acg-keep-both.tsv`、`acg-kept-c.tsv`、`acg-yield.tsv`、`acg-removed.tsv` | 欄位：讀音、保留詞、排除詞（`+word` = 兩者都保留，依列排序）、附日期與決定者的理由。建置後 `first_named` 必須是 top-1；`unresolved_collision_readings` 必須為 0 才能合併。讓位清單減 3.0，使詞庫包詞仍是候選，但詞庫詞排第一。決定只能附加，從不修改。 | A4：新增 43 列，移除 15+4 個詞並附理由；it-pack 9 個群組「既有常用寫法優先」。 | 沒有 | 調整後採用：每個詞庫包一個 `dispositions.tsv`，含 keep／exclude／keep-both／yield；不拆成多個檔案。 |
| C8 | 確定性的平手排序 | `build_acg_pack.py` `TIE=1e-6`、`NEW_BEHIND_SHIPPED=3.0` | 量測發現解碼器的平手判定與檔案順序無關，因此同讀音的詞庫包列分數 − k×1e-6。若某讀音已有出貨的詞，新詞 −3.0，讓更新永遠不會改變使用者已在打的字。 | 沒有這條規則時，model-v5 的計數讓 波伊德 勝過已出貨的 波依得。 | 不適用 | 在任何 ZT 詞庫包建置工具中採用這兩條規則；也讓 ZT lattice 的平手判定明確化。 |
| C9 | 固定「規則模型」+ 對照基準重新檢查 | `acg-pack.md` A3.3、A3.5 #6、A4 #5；`--rule-lm`、`--recheck --base` | 碰撞規則 (c) 以固定模型（以 sha 固定）解碼，使詞庫包的決定不會在 LM 重建時漂移；另外 `--recheck` 以出貨模型解碼每個已出貨的 (word, reading)，要求 top-1 與前一版詞庫包相同（總數必須為 0，或每個差異都經核准）。 | GCP 上以新解碼器重建時，默默捨棄了 keep-both 的 沃茲 與已出貨的 亞莉莎 → 因而有「只重新評分」（idea 13）。 | 沒有 | 採用重新檢查（成本低，使用 zhuyin-cli）；只有在 ZT LM 經常變動時才固定規則模型。 |
| C10 | 稽核：詞庫包／overlay 詞為何落敗 | `experiments/acg-audit/summary.md`、`analyze.py`、`dropped-c.tsv` | LM-OOV 詞的解碼總分剛好是 `capped entry lp + K`（K chat −1.42，formal −1.99）：自己沒有任何機率。勝出者通常是常見詞彙內單字的串接（露娜→路+那、宿儺→素+娜），而不是另一個對應詞。LM 項決定了 99.9% 的落敗。 | 依長度的落敗率：2 字 98.7%、3 字 38.9%、4 字 7.9%、5 字以上 2.6%（佔全部列的 26.5%）。+1.0 可救回 34.0%、+2.0 救回 60.9%、+3.0 救回 85.1%。整個詞庫都有相同效應：overlay-wikt 的 2 字條目落敗 78.1%，基礎詞庫為 34.5%。 | 沒有 | 在 ZT 加入任何詞之前採用此稽核方法（ZT 以權重 0.5 加入 v3 字元 log P，可能更不利於罕見字人名）。 |
| C11 | Pack-OOV：只有在已決定處，詞庫包詞才略過 LM 統計 | `docs/contracts/pack-oov.md` §1、§1b；`tools/gen_pack_oov.py`；核心 `CappedLexicon::with_packs` | 第一版：所有詞庫包字串 → `word_id = None`，條目分數不設上限，沒有未見懲罰。失敗：46% 的 ACG 詞庫包詞（20,393/44,448）在 LM 詞彙中，因此失去自己的 bigram。最終版（V1c）：只有人工處置過之讀音上的字串才「強制」（`acg-oov` 標籤）；只有在 LM 計數為 0 或 log10(c/N) < S_len−0.5 時才強制；其他詞庫包詞保留 LM 統計，但不套用 −1.0 未見懲罰。 | V1 對 main：22 改壞／25 改對（否決）。V1c 在 51 個稽核讀音上：出貨路徑 43/51，main 26/51，沒有任何一個變差。重排層仍覆寫了 8/51。RSS 額外 ≤1 MiB。 | 沒有 | 調整後採用：ZT 條目需要一個逐條目旗標「信任條目分數，略過 LM／v3 項」，只用於審核過的人名；絕不全面套用。 |
| C12 | 無計數詞的基礎計分 | `tools/build_overlay.py` `length_scores`、`MANUAL_BONUS`、`VARIANT_PENALTY`；`build_names.py NAMES_BONUS`；`build_it_pack.py BONUS` | 條目分數 = 同長度基礎條目的第 25 百分位（條目少於 1,000 時退回前一個長度：5 字以上）；加分以事先登記的小網格選定：人工 +0.75、人名 +2、IT／迷因 +3；一／不 變調異讀列 −0.5。 | p25：2 字 −7.17、3 字 −7.04、4 字 −6.61。IT：門檻 20／加分 0 時檢查集 22/60；最終 50／+3 通過 chat ≥45/60。人工加分在 model-v5 上最低 0.5，在 v6 上為 0.75。 | 部分（ZT 分數來自 wordfreq） | 調整後採用：依長度取 p25 + 依來源加分，在網格上以保留的檢查集選定。 |
| C13 | 凍結計數 + 「只重新評分」 | `pack-oov.md` §1.3a；`data/packs/acg-counts.tsv`（檔頭固定模型 sha 與 N） | `g(w) = max(S_len, log10(c/N) + B)`，使用凍結的計數表（工具中固定 sha）；`regrade` 子命令只改寫分數，(reading, word) 集合不變且有測試確認相等，讀音內的順序維持不變。 | B 網格 {0, 0.5, 1.0}，guard：調參集改壞率 ≤0.5%。 | 沒有 | 調整後採用：在 ZT 詞庫包建置中把「成員」與「分數」分開，讓 LM 更新只重新計分。 |
| C14 | 以 LM 頻率限制 overlay 詞的分數 | `pack-oov.md` §0；`names-stores.md` §2（`capped_score`） | Overlay 條目分數上限為 LM 自己的 log10(c/N)，因此 overlay 無法灌大已知詞；副作用：加分對模型已知的詞沒有作用。 | 店家檢查集因此上限卡在 201/225（89.3%）；維護者接受。 | 不適用 | 不採用為規則；若 ZT 將來混用檔案分數與 LM unigram，要記住這個取捨。 |
| C15 | 以政府開放資料建立 IT + AI 詞庫包 | `it-pack.md`；`tools/build_it_pack.py`、`it_sources.py`、`it_wiki_counts.py`；`data/packs/it-*.tsv`、`it.json` | 來源：國家教育研究院電腦名詞（data.gov.tw 15416，123,432 個名詞）與兩岸電腦名詞（15275）、zhwiki `CGroup/IT`、`CGroup/Electronics` 的 zh-tw 值、往下兩層的 AI 分類標題、人工清單（151 詞：提示詞、上下文視窗、合併請求…）。只收 2–10 個漢字；頻率條件為在全部 zhwiki ns0 中不重疊出現 ≥50 次（在 GCP 上計數一次，檔頭固定 dump sha、候選 sha、工具 commit）；AI 標題以 Wikidata P31 篩除人物／組織／公司／作品。 | 國家教育研究院 123,432 個名詞中有 114,771 個不在詞庫中；頻率規則捨棄 109,760 個；最終 1,021 列。檢查集（seed 20261024，60 詞）閘門 chat ≥45。記憶體閘門 ≤40 MiB，每鍵 p95 ≤16 ms。 | 沒有 | 採用：對 ZT 使用者（開發者）價值最高的詞庫包；國家教育研究院資料採 OGDL（只需標示出處）。 |
| C16 | 中國大陸寫法篩選 | `it-pack.md` §2「大陸寫法」；`it.json` `markers` | 封鎖清單 = 15275 的「中國大陸譯名」欄 + CGroup zh-cn 值（轉為 zh-hant）；只要任何台灣來源有該字串就仍允許。子字串標記（網絡、信息、算法、軟件、數據庫、默認、視頻、內存、緩存、服務器、文件夾、激活、兼容…），並先遮蔽 演算法。只阻擋新增，從不刪除。 | 標記阻擋例如 信息 48、網絡 28、算法 26、數據庫 12。 | 沒有（且 ZT 的基礎是 CC-CEDICT，中國大陸用語偏多） | 採用：先對 ZT 源自 CC-CEDICT 的詞庫跑標記 + 兩岸篩選並產生報告，再降權／移除。 |
| C17 | 「已經打得對 → 不加入」 | `it-pack.md` §2；`meme-pack.md` §2；`names-stores.md` §2 | 用出貨的 CLI 單獨解碼每個候選的讀音；若 top-1 在兩個 profile（店家只看 chat）都已等於該候選，就略過。讓詞庫包保持精簡並避免分數副作用。 | 店家：探測的 77 個常見名稱中，61 個已正確，16 個錯誤（馬辣→馬臘、晶華→精華、君悅→軍樂…）。 | 沒有 | 採用：用 `zhuyin-cli` 很容易做到，可大幅縮小詞庫包。 |
| C18 | 「把第一名讓給較常用的字串」 | `net-slang.md` §2、`net-slang-2.md` §2a、`meme-pack.md` §2、`names-stores.md` §2；`experiments/names-creators/count_usage.py` | 兩輪計數：計算候選次數，解碼得到目前的 top-1 X，再以同一工具與同一語料快照（檔頭記錄 tree hash）計算 X 的次數；只有候選的文章數 > X 時才收錄。同讀音的新詞依次數排序，後面的 −1.0×k（`SLANG_BEHIND`）。 | 例：築間 勝過解碼器產生的 竹間 而收錄。被拒的列記錄兩者的次數。 | 沒有 | 調整後採用：ZT 沒有 PTT；改用它的聊天語料 + wiki（或公開的論壇 dump）計數；可重用的是兩輪計數的結構。 |
| C19 | 以文件為單位的使用量門檻 | `names-creators.md` §2、`net-slang.md` §2a、`net-slang-2.md` §2a；`count_usage.py` | 次數 = 含該子字串的文章數（內文 + 推文），加上不同看板數。門檻在計數前就固定並提交：人名／迷因 看板 ≥3 且文章 ≥10；流行語第 1 輪 ≥5/≥20；LLM／人工整理的候選 ≥2/≥3。`--jobs N` 的輸出與單一行程逐 byte 相同。被較長字串灌大的子字串次數以人工排除（辛巴／辛巴威、力宏／王力宏）。 | PTT 快照 110 個看板 × 300 頁，593,546 頁。 | 沒有 | 調整後採用：依 ZT 語料調整門檻；保持「事先登記門檻」的紀律。 |
| C20 | 常駐的人名層（品牌、公司、學校） | `names-lexicon.md`；`tools/build_names.py`；`data/lexicon/names-*.tsv`、`names.json`；`tools/testdata/names/*` | 來源：name-suggestion-index（BSD-3，固定 commit，17 個類別，`locationSet.include` 含 `tw`，名稱鍵順序 `name:zh-Hant`→`zh-TW`→`zh`→`brand:*`，拒收純簡體字）、NSI 品牌 Wikipedia 頁面的 zh-tw 標題、wiki 分類（台灣手搖茶飲品牌…）、教育部大專校院 + 高級中等學校名錄（只取最新學年度），學校簡稱只在有 wiki 重新導向時收錄。破音字（樂、行、長、重…）使用整詞讀音表。分數 p25 + 2。起因：使用者輸入 酷澎 得到 酷朋。 | 2,014 列；捨棄：in_lexicon 317、title_not_han 75、NSI 無法使用 43、(c) 15。精確度抽樣 ≥90/100。 | 沒有 | 採用：ZT 沒有專有名詞層；NSI + 教育部 CSV 的授權清楚。留意 NSI 中的日文字形（丸亀製麵）。 |
| C21 | 由 LLM 提議、經語料驗證、只附加的創作者與店家名稱 | `names-creators.md`、`names-stores.md`；`data/lexicon/names-creators.tsv`（171 行）、`names-stores.tsv`（226） | 由 LLM（Meta「Muse」）依類別列出實況主／YouTuber／VTuber、節目、店家；只保留字串，收錄依據為 PTT 使用量 +「目前會打錯」+ 讓位規則；選用的第 4 欄加分（+1.0）只用於某讀音的第一列。附加在 `names-add.tsv` 尾端，並有一個不變式：移除附加的標籤即可還原舊檔案 sha（`77f70524…`），因此已出貨的列永遠不會移動。 | 店家：重排關閉時 chat 201/225（89.3%）；user-reported 第 161 列（馬辣）修正。創作者：統神 201 篇 PTT 文章、國動 222、鬼鬼 143 原本都缺。 | 沒有 | 調整後採用：LLM 只用來產生候選，以語料次數驗證；加上只附加的不變式測試。 |
| C22 | 把網路流行語加入主詞庫 | `net-slang.md`、`net-slang-2.md`；`data/lexicon/slang-add.tsv`（1,422 行）；`build_overlay.py manual_rows(tag="slang")` | 公開清單（zhwiki 台灣網路用語列表、台語流行語／髒話清單、中英文 Wiktionary 分類、PTT鄉民百科 Fandom 分類）+ PTT 探勘（2–6 個漢字的字串，最低 PMI 與左右分支熵，門檻校準為保留 76 個已知流行語中的 90%，保留最長者）。第 2 輪只保留目前會打錯的字串；LLM 子 agent 分成 9 類（流行語、片語、迷因、專有名詞、人物、錯字、中國用語、片段、冒犯用語）；冒犯用語 → 迷因詞庫包；人物 → 人名層。髒話依使用量保留。 | overlay 中有 1,481 列 `slang`。第 2 輪檢查集閘門 chat ≥90%；停止規則為記憶體 +5 MiB／p95 +1 ms。第 1 輪只靠公開清單只得到 76 詞。 | 沒有 | 調整後採用：ZT 的聊天模型是合成資料；探勘得到的流行語列正好能幫助 ZT 鎖定的「口語同音字」錯誤。需要有授權的論壇語料。 |
| C23 | 量化的流行語加分欄 | `net-slang-2.md` §2；`slang-add.tsv` 第 4 欄 | 輸給無意義組合的詞，加分 = 分數差距 + 0.25，以 0.5 為單位向上取整，上限 3；只在競爭字串 X 不是真實片語時才加（由維護者篩檢）。對 LM 已知的詞沒有作用。 | 實際值 0.5–1.0。 | 沒有 | 調整後採用：量化、有上限、經審閱的逐條目加分，勝過臨時的加權。 |
| C24 | 主詞庫與可切換詞庫包的分流原則 | `net-slang.md` §0、`meme-pack.md` §0、`net-slang-2.md` §0 | 有用且不冒犯的口語片語放進常駐詞庫；冒犯性迷因、政治綽號、有出處的語錄放進選用的「迷因」詞庫包（預設開啟，可切換）；短期事件詞放進不載入的熱詞清單。 | 迷因詞庫包契約已通過，但此 checkout 中尚未出貨 `meme-add.tsv`。 | 沒有 | 採用這個分流原則。 |
| C25 | 依 (reading, word) 降權敏感詞 | `docs/contracts/sw-sensitive-demote.md`；`data/lexicon/demote.tsv`；`experiments/sw/sensitive-reading.txt`（11 列探測，sha 凍結） | 針對特定非標準讀音，從最終詞分數（不是詞庫分數）減去 δ；收錄條件：詞在教育部辭典中、讀音不在教育部辭典中、不是輕聲／變調，且有使用者回報或探測證據。可切換（預設開啟），CLI `--no-demote`。沒有政治審查清單。 | 只降詞庫分數無效（LM 差距 1.52/1.39 > 詞庫差距 0.95）。一列：ㄍㄠˇ-ㄨㄢˊ 睪丸 δ 2.0 → 搞完這波。5,817 個條目有非教育部讀音，多為變調／輕聲，因此不做大量降權。醫學詞 δ≥1.5 會讓 左側睪丸→左側高玩。 | 部分（使用者排除詞清單） | 採用：出貨一個以讀音為鍵的小型 `demote.tsv`，在 LM／v3 計分之後套用，並以探測檔防止退步。 |
| C26 | 讀音產生層級 + 覆寫 | `tools/readings.py`、`tools/reading_overrides.tsv`（25 列）、`data/lexicon/names-readings.tsv`、`data/packs/it-readings.tsv` | 順序：人工覆寫 → 一／不 以本調輸入 → 若 MOEdict 第一讀音是詞庫讀音則採用 → 單字用 McBopomofo `heterophony1.list` → 唯一或明顯勝出者（≥0.5）→ 否則標為 CHECK 交由人工。UNREADABLE 捨棄。每個領域使用整詞讀音表，避免覆寫外洩到評測集。 | A4：工具在 358 個人名中有 278 個把 麗 判為 ㄌㄧˊ；覆寫 麗 ㄌㄧˋ、虎 ㄏㄨˇ、拓 ㄊㄨㄛˋ、条、繆 取代了 454 個錯誤的 (word, reading) 列。 | 部分（ZT 使用 CEDICT 拼音 + kHanyuPinlu 機率） | 採用：每個新詞來源都有 `reading_overrides.tsv` 與 CHECK 報告。 |
| C28 | Wikimedia 標題 dump overlay | `tools/build_overlay.py`；`data/lexicon/overlay-add.tsv`（341,765 列：wikt 269,957、zhwiki 70,320） | zhwiki、enwiktionary、zhwiktionary 的全標題 dump，以 URL + sha256 固定；2–4 個漢字，每個字都必須是基礎單字，不含純簡體字，日本新字體篩選（OpenCC 表，只用於篩選），除教育部辭典字詞外篩除粵語字，移除異體字（臺／台）；讀音取自教育部簡編本（CC BY-ND，只用來挑選讀音，不再散布）。 | 稽核（idea 10）顯示 wikt 的 2 字條目落敗 78%，且很多是雜訊。 | 沒有 | 謹慎地調整後採用：zhwiki 標題可以；大量 Wiktionary 不行（精確度低）。 |
| C29 | 模型登記 + 雜湊綁定 | `data/models.tsv`、`data/*.sha256`、`docs/data-files.md` | 大型模型放在與 App 發布分開的 GitHub Release 資產中；儲存庫只存 sha256；CI、打包與測試在使用前都會驗證。代號 `SJ-LM-6-9.1M`（家族-版本-大小）。衍生檔案內嵌上層檔案的 sha（classes、KN 附屬檔、trigram、tinylm 內嵌 bigram sha），不符時載入失敗並回傳代碼 3。下載到暫存位置再 `mv`（就地覆寫已 mmap 的檔案會導致 SIGBUS）。 | 記錄的陷阱：替換 `tinylm.sjtl` 會默默停用口語項（配對 sha）。 | 部分（ZT 有 manifest.json、mmap、熱重載；跨檔案綁定沒有文件） | 採用：在每個衍生模型的檔頭內嵌上層 sha；更新時以原子方式重新命名。 |
| C31 | 逐檔授權帳冊 | `LICENSES/data.md` | 每個資料檔一列：來源、授權、備註、是否出貨。套用的規則：相同方式分享的授權只限於資料（程式碼為 Apache-2.0）；ND 來源（教育部辭典）只用來決定讀音或評測，從不再散布；排除 NC 來源（萌娘百科、偽基百科、街頭字典只作為候選提示）；Fandom 的 CC BY-SA 3.0 依「後續版本」條款改以 4.0 授權；政府 OGDL 的出處文字逐字照錄；LLM 輸出記錄供應商條款快照（雜湊、日期）。 | — | 部分（`resources/SOURCES.md`、`platforms/macos/Resources/Licenses/`） | 採用此帳冊格式；重新檢查 ZT 使用的 Taiwan Tongues（TRAIL 授權）是否允許再散布衍生的 `lm_chat` 二進位檔。 |
| C32 | 逐單位出處檔 + 打包檢查 | `tools/build_attribution.py`；`colloquial-attribution.tsv`；`scripts/check-app.sh`（被引用） | CC BY 語料逐單位標示出處（YODAS 影片 id + URL、Tatoeba 句子 id + 作者、WildChat／MASSIVE 以資料集為單位），不含文字；各來源加總必須等於記錄的數量。App 內附 `CC-BY-SA-4.0-attribution.txt`；`check-app.sh` 會 grep 每個詞庫包的出處行，缺少則失敗。 | 143,895 句的語料池：WildChat 3,491、YODAS 90,080、Tatoeba 36,839、MASSIVE 13,485。 | 沒有 | 在 ZT 的 pkg 建置中採用打包 grep 檢查。 |
| C33 | 詞庫包評測集與閘門 | `eval/dev/acg/*`、`eval/dev/it/words.txt`、`eval/dev/names/*`（經由契約） | 每個詞庫包有自己的小型檢查集，來自 CC0／Wikidata 或詞庫包的 seed 抽樣（seed 在量測前提交）；從不用於調參；一般評測集必須 0 退步；新詞庫包加入時，所有先前的詞庫包評測集都必須 0 退步；holdout 只由新的驗證者執行。只含詞的詞庫包在 broken = 0 時即通過錯字回報標準。 | ACG 5 組（作品 142、角色 227、部分 105、近期 23、間隔號 45），人名 61，IT 60。 | 部分（ZT 有 dev／test 集，沒有詞庫包評測集） | 隨每個詞庫包採用。 |
| C37 | YouTube 事件索引、熱詞、CC BY ASR 語料 | `docs/contracts/youtube-events.md` | 事件詞取自單一頻道的 metadata + zhwiki 年度大事記，需有 ±7 天內來自 ≥2 家媒體的 ≥3 則 Google 新聞標題驗證；「長期」需要 PTT 跨度 ≥180 天；短期詞放進不載入的熱詞清單。ASR 路線：只用 `status.license=creativeCommon` 的影片（COSCUP 1,066 部／760 小時、PTSTalk 89 部／84.2 小時）；閘門為每小時 ≥400 句，以及 100 句參考句上 CER ≤5%。 | 只有計畫（儲存庫中沒有結果）。 | 沒有 | 暫不採用；CC BY YouTube 頻道這條路值得記住，可作為有授權的聊天語料。 |

## 2. 語言模型與解碼

N-best 寬度、左側 context、期望計數、容錯列、詞類、KN、trigram、路徑合併、讀音通道、模糊輸入。

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| A2 | 交給評審的 N-best 加寬（16→64） | s1.md §補充量測 | Beam 大小與 PER_KEY 對 oracle@16 沒有影響，因為前 16 名由 unigram 分數決定。交給重排模型 64 個候選可以提高上限。 | 所有 beam 設定下 oracle@16 都是 94.7%；oracle@64 96.7%（beam 32）→ 97.7%（beam 64 以上）。其餘漏掉的是 5 個 OOV 詞與 2 列同音詞順序。 | 有（top-16 重排，每個長度 top-24） | 調整後採用：在調 v2b 之前，先在開發集上量測我們的 oracle@16 對 @32／@64。他們發現限制在重排模型的上限，而不在 beam。 |
| B19 | 候選涵蓋率 8 對 16 對 64 | research-log 2026-10-18 | 正確答案在 n-gram 清單中的名次 | chatjudge：第 1 名 2,497，第 2–8 名 379，第 9–16 名 32，第 17–64 名 34，>64 為 30 | 有（做法不同：top-16／top-24） | 已涵蓋；維持 16 |
| A5 | 左側 context 輸入 n-gram（歷史詞） | s2h-left-context.md、research-log 2026-10-06 | 組字的第一個詞以 history = 最後 1–2 個已送出字元計分（若它們在 LM 中，`lm::history`），而不是 `<s>`。尾端切分評測集（`experiments/s2h/split_tail.py`）把每句最後一個詞切下作為 context。 | 尾端集：cvtune chat 3,254→3,509（277/22，p 2.5e-57），formal 296/31；wikitune chat 145/31，formal 160/50（p ≤ 1e-14）。Guard 集 ±1。好+ㄅㄚ˙ → 吧。每鍵 p95 不變（約 1.3–1.7 ms）。 | 有（128 字 context，`--split continue`） | 已涵蓋（context 更長）。保留一組尾端切分評測集作為退步閘門。 |
| A6 | 在斷詞 lattice 上計算期望（forward-backward）計數 | s2n-simplified-residue.md §6（model E）、research-log 2026-10-06 | 訓練時以後驗機率加權計算每一種斷詞，而不是只算一個最大斷詞（Mori/Takuma/Kurata 2006）。最大斷詞從不計入切開路徑的 bigram（十分鐘／後到 代表 c(後,到)=0；好吧 作為一個詞代表 c(好,吧)=0），但解碼器兩條路徑都會走。低於 0.01 的邊會被剪除；計數四捨五入為整數。 | c(後,到) 0→206，c(好,吧) 0→9。「大概十分鐘後到」在兩個 profile 都修正（差距 0.52）。cvtune chat 3311→3326（26/11，p 0.020），formal +13（p 0.079），沒有 guard 否決。Bigram 種類 +1.4×；200k 篇文章在 10 個 worker 上花 8 分鐘，峰值約 22–28 GB（估計）。模型 80.7 MB。 | **沒有**（pipeline.py 做 Viterbi hard-EM；2 次更新） | **採用**：把 `ml/src/zhuyintype_ml/training/lm/pipeline.py`（`segment`／`em`）最後一輪 hard-EM 換成 forward-backward 期望計數，並在四捨五入或小數計數上保留 MKN。這能修正一整類「詞在詞庫中，但其切開形式的 bigram 為 0」的錯誤。 |
| A16 | 以資料列實現輕聲容錯（δ 2.0） | neutral-tolerance.md、research-log 2026-10-26 | 3,220 個基礎詞只有輕聲讀音（桌子 打成 子ㄗˇ 得到 捉子）。以 −δ 加入本調列，使用同韻母中分數最高的聲調。沒有同韻母聲調時不加列（的／了：核心拒收無聲調的 ㄉㄜ）。 | 300 詞檢查集：δ 2.0 時 chat 177→277（+100/0），formal 161→264（+104/1）。δ 0.5 得 +110，但弄壞 1 列 wikitune。錯誤回報集：formal 改壞 1 列（視為例外接受）。 | **有（做法不同）**：在核心層處理，任何聲調，懲罰 0.4/1.2（`Engine.swift` `entries(for:)`） | 已涵蓋。他們的 δ 網格暗示太小的懲罰會破壞斷詞（兩個 ㄌㄧㄤˇ ㄍㄜˋ 被切開）。用類似的 300 詞集檢查我們的 0.4。 |
| A17 | Bigram 詞類 backoff（Brown exchange，K=512） | s2k-word-classes.md、experiments/s2-classes | 非監督式 exchange 分群（N=40k，K=512，12 輪，約 12–38 分鐘）得到類別 bigram × 類內發射機率。只在 backoff 分支混入：`back·((1−μ)pb + μ·P(c(w)\|c(v))·emit(w))`。1.7 MB 附屬檔。 | Model v1，μ 0.95：dev302 chat 15/3（p 0.0075），formal 22/5（p 0.0015），wikitune +84/+88（p ≤ 1e-12）。Model v3，μ 0.8：dev302 +6/+8 不顯著，cvtune-native +21/+26（p 0.002），holdout 177→179／183→183。使用者指出的錯誤型態：5 個修正 1 個。有部分改壞（再過去→在過去）。 | **沒有** | 調整後採用（實驗）：成本低（1.7 MB，只在 backoff 時約 2 次查找）。在我們的 chat 與 wiki bigram 上、*在* v3／v2b *之後*試用。增益可能被我們的神經層吸收，所以要在 v2b 開啟時量測改對／改壞。 |
| A18 | μ 0.8 → 0.2（classes-v5） | classes-v5.md、research-log 2026-10-16 | 類別權重在領域間互有取捨：μ 0.8 對私人聊天有幫助，但傷害 formal／lytune。只改檔頭。 | μ 0.2 對 0.8（出貨架構）：lytune chat +27（100/73，p 0.048），formal +65（130/65，p < 0.0001）。類別開對關：judge5 −42/−51，但 discordtune +51/+45。 | 沒有 | 教訓：在類 Wikipedia 調參集上調出的類別／backoff 權重，會讓真實聊天往反方向移動。依 profile 分別調參。 |
| A19 | 重新分群時保留舊群（`--keep-from`） | model-v5.md §9 | 詞彙增加後重新分群，每個類別都漂移，並失去 3 列探測（69→66）。保留舊的分配、只把新詞放進去，便恢復了。 | 探測 formal 66→69；互資訊大致相同（0.8351）。 | 不適用 | 若採用 #17 則*採用*：模型升版時不要從頭重新分群。 |
| A20 | 把量詞強制放進 個 的類別 | model-v6.md §2.2b、experiments/model-v6 | 罕見量詞（坨，加權計數低於 1,000）被移到 個 的類別，讓 一整坨 的 backoff 行為與 一整個 相同。 | 目標為 一整坨 的名次（沒有公布彙總數字）。 | 沒有 | 調整後採用此想法（很小）：若 #17 落地，加一個人工整理的量詞類別。 |
| A21 | Kneser–Ney continuation backoff（附屬檔，θ=1，β=1） | kn-smoothing.md、kn-core.md、research-log 2026-10-10 | 他們的 backoff pb 原本是*詞庫*分數。改以 N′(w)/ΣN′ continuation 計數取代（θ = 最小配對計數）。在四捨五入後的期望計數上近似計算。 | 調參 4 格在 13,236 列中 +33 列（0.25%）。只有 wikitune formal 顯著（38/20，p 0.025）。Holdout 180→182／184→183。5 個探測不變。 | **有（做法不同）**（pipeline.py 中的 modified KN） | 已涵蓋。注意他們後來發現，一旦有 trigram，KN 會*傷害* formal profile（tune −53，p 0.0001）：平滑方法會與其他項交互作用。 |
| A22 | 學到的詞略過 KN／trigram | model-v6.md §2.8、trigram-core §A | 開啟 KN 後解碼器替換了 pb，因而吞掉個人學習的加分（約 3.0 → 0.9–1.5 log10）。Mirror-teach 由 7/8 降到 4/8。修正：加分的條目使用非 KN 公式。Trigram 也用同一規則（否則加分會被 back3 縮小）。人工詞加分 0.5→0.75。 | Mirror teach ≥80% 恢復；人工詞在兩個 profile 都是 15/15。 | 有類似風險（混合模型內個人 n-gram 的 kappa 內插） | 調整後採用：每次更改平滑方法、混合方式或 trigram 時，重跑個人學習驗收（教過後重打必須勝出）。確認個人加分不會被 backoff 權重除掉。 |
| A23 | 只用於 formal、關閉 KN、以相對熵剪枝的 trigram（H3） | trigram-core.md、research-log E1/E1e–h | 開啟 KN 的 trigram 在各種大小下：只有未剪枝的 T1（1.13 GB）通過。**關閉 KN** + formal 上的 trigram：在相同條目數下，Stolcke 相對熵剪枝勝過計數截斷（H3 +64 對 T3 +35）。出貨的 H3：7.7M 條目，143.6 MB。Chat：沒有訊號。 | 調參（6,618 列）+64（205/141，p 0.0007）；judge5 +36（p 0.04）；discordtune +35。「只關閉 KN」：−53（p 0.0001）。增益集中在 wikitune／lytune；cvtune −11..+13 不顯著。延遲 p95 +0.01 ms。 | 試過，在測試集上落敗 | 調整後採用：若要重試，(a) 以相對熵剪枝，而非計數截斷；(b) 只用在 wiki／formal 混合端；(c) 評估時考慮 KN／backoff 的交互作用；(d) 見 #24。 |
| A24 | Trigram 下限與單詞結尾項 | trigram-core §A2 | 加入 trigram 後的退步（好+ㄅㄚ˙ → 爸；搞完這波 → 睪丸這波）來自只存有少數後續的 context，未見的後續會乘上很小的 back3。修正：P3 = max(P3, 0.5·P2)。此外，在未知 u context 之後的 1 詞片段使用 bigram 結尾項。 | 下限 0.5 在調參集上選定：tune +73（193/120），judge5 +42（141/99），discordtune +33（140/107），改壞數比沒有下限時少。 | 沒有（與我們失敗的 trigram 有關） | 在任何 trigram 重試中**採用**：下限只有一行，直接針對我們可能遇到的失敗模式。 |
| A25 | 以 mmap 載入 trigram 表 | trigram-mmap.md | 用 `std::fs::read` 讀入 143.6 MB 會計入活動監視器的記憶體。唯讀 mmap 則是可回收的乾淨頁面。 | 加上該表後 footprint 只 +2 MB（原本 +136 MB）；最大 RSS 仍 +152 MB；輸出相同。 | 有（ZTLM／ZTLX `.alwaysMapped`） | 已涵蓋。 |
| A26 | 以 (surface, last word) 合併路徑 | dec-recombine.md | 他們的解碼器每個表面字串只保留一條路徑，因此 下｜來（−6.42）輸給 下來（−5.06），雖然 來→分析 比較好。修正：以 (surface, last word) 為鍵；最後再依表面字串對 N-best 去重。 | Python 試驗：每個 guard 格都 ≥ 0（cvtune chat +5、formal +3；wikitune +7/+7）。下來分析 案例修正。 | **有（做法不同）**（`Conversion.swift` 以 LM history `stateKey` 為鍵） | 已涵蓋。保留一個類似他們的單元測試（一個玩具詞庫，合併後的路徑必須存活）。 |
| A28 | 單字同音混淆很分散 | diag-near-miss D2/D3 | 在私人聊天上，前 10 名（錯、對、讀音）字對只涵蓋 19.8% 的單字錯誤（chatclause 上為 24.1%）。因此逐對偏差表幫不上忙，只有語意模型能處理。D3：20.2% 的錯誤有 overlay／語料與詞庫方向衝突。 | D2 < 40% 門檻 → 結案。D3 20.2%（只算詞數相同的列為 13.9%）。 | 與 `ml/src/zhuyintype_ml/training/homophone/`（在／再）相關 | 調整後採用：在投入逐對同音字分類器之前，先在真實錯誤記錄上量測我們的字對集中度。他們的約為 20%。 |
| A29 | 融合中的讀音通道 P(reading\|word)（P5） | reading-channel.md、experiments/p5-reading-channel | 只用於單字詞：t = s(r,w) − max_r' s(r',w)，下限 −4，以 ν/α 縮放，相對於錨定路徑。Softmax 正規化被否決，因為 13,256 個破音詞中有 9,404 個的讀音平手，softmax 會因為「有 k 個讀音」就扣 log10(k)。 | 疊在口語項之上：chatclause 23/0，discordtune 23/0（p 2.4e-7），cvtune-clean 9/0。對出貨版：discordtune 88/34。唯一的改壞（長歪了→掌歪了）來自 −6.8 的佔位分數。 | **有（做法不同）**（lattice 中來自 kHanyuPinlu 的讀音先驗） | 已涵蓋，但**調整正規化方式**：讓 P(reading\|word) 相對於最大讀音（最大值 → 0），而不是 softmax 或加一的比例，避免有幾個同樣常見讀音的詞（什麼 ㄕㄣˊ／ㄕㄜˊ）被懲罰。為懲罰設上限。 |
| A30 | 降權複製來的輕聲分數（A1） | reading-aware-0.md §3.2、research-log 2026-10-17 | 詞庫給 巴 ㄅㄚ˙ 的分數與 ㄅㄚ 相同（複製而非量測），因此 巴 勝過 吧。對 44 個輕聲讀音只出現在詞尾的字降權 1.0。 | 每個評測集與 profile 都 0 改壞。修正：錯誤回報 2/2、lytune 1/1、discordtune 3/0。 | 有（做法不同：kHanyuPinlu 讀音先驗） | 已涵蓋。若 CEDICT 有複製分數，檢查我們對 巴／西／公 ˙ 讀音的處理。 |
| A31 | 懲罰以一聲輸入語助詞（A2） | reading-aware-0 §3.3 | 吧／啦／嗎 以一聲輸入時額外懲罰。 | **負面結果**：δ 0.25 時 lytune chat −213，discordtune −7（p 0.016）；δ 0.5 時 lytune −508。使用者會把 啦 打成 ㄌㄚ（好ㄌㄚ）。 | 有（吧 打成 ㄅㄚ 時以最低成本對應到 ㄅㄚ˙） | 不採用：已證實有害。讓語助詞以一聲輸入時維持低成本。 |
| A32 | 讀音拆分計數（字⊕讀音單位）（B） | reading-aware-0 §3.4 | 把破音字當成 (char, reading) 單位計數，以 context 規則標註（得／地／還／重／空／長／行）。 | **失敗**：口語文字上標註錯誤 40.8%（地 53 個錯 43 個）。judge5 chat 5/1462，formal 15/1394。共用路徑也會失去下一個詞的 context。 | 沒有 | 在有可靠的標註器（g2pW）之前不採用。規則式標註雜訊太多。 |
| A33 | 模糊輸入研究：聲調／模糊音對／相鄰鍵 × 閘門 G0/G1/G2 | fuzzy-research.md、research-log 2026-10-14 | 以 p 0.05–0.2 注入錯誤。G2 以 −δ（log10）把容錯讀音放進 lattice。δ 在調參集上以乾淨輸入改壞 ≤0.5% 為條件選定：聲調 3、模糊音 1、相鄰鍵 2。調查了 AquaType、librime（每音節 log 0.5）與 libime。 | 合法讀音錯誤（使用者自己察覺不到的那種）：模糊音 top-1 救回 91–95%，聲調 62–67%，相鄰鍵 52–82%。乾淨輸入改壞：dev299 0；typing76 相鄰鍵 4/76。G2 延遲：模糊音 1.4×，聲調 2.3–2.9×，相鄰鍵 1.8–2.1×。 | 三者都有（模糊音關閉，聲調只有 3↔4 與 2↔˙，按鍵通道） | 調整後採用：他們的資料支持 (a) 模糊音對在約 1 log10（≈2.3 nat）時幾乎不會改壞，因此可考慮 ㄣ／ㄥ 預設開啟；(b) 以高成本（約 3 log10）容許*任何*聲調互換。我們只允許 3↔4 與 2↔˙。 |
| A35 | 整串只打聲母的縮寫（E3） | research-log 2026-10-15 E3 | 每個字以機率 p 只輸入聲母。 | p 0.2：top-1 比例 0.789/0.808（閘門 0.9），**未通過**。oracle@64 仍為 289/300，所以瓶頸在排序。每個縮寫約 63 個讀音；每列 148 ms，原本 10 ms。 | 沒有（無聲調輸入規劃於 0.7） | 調整後採用：0.7 的無聲調／縮寫輸入，預期限制在排序而非候選召回。據此安排重排模型的預算。 |
| A36 | 句中標點不重設 context（E4a） | research-log E4 (a) | 在組字內跨越 ，。 保留 LM history。 | −1（chat），−8（formal），不顯著。**沒有增益。** | 有（context 跨送出延續） | 不採用，已知的無效結果。 |
| A37 | 數字／拉丁字母之後的 `<X>` context（E4b） | research-log E4 (b) | 數字／英文連打之後的字，以特殊的 `<X>` history 而非 `<s>` 計數與解碼。 | 主集 chat +18（p 0.02），但 wikitune guard 顯著為負（2/12，p 0.013）→ **未通過**。 | 沒有 | 依測試結果不採用。若嘗試混合文字的 context，要以 wiki 作 guard。 |
| A38 | 模型壓縮（varint 無損；8／4 bit） | research-log E1c | Bigram 檔 83.4 MB → 28.6 MB 無損（zip 27.9 → 20.0 MB）。8 bit 無害但只省 0.5%。**4 bit 顯著有害**（lytune 40/69，p 0.007）。 | 如上 | 有（mmap ZTLM/1；常駐 29 MB） | 若下載大小重要則調整後採用：效益來自 varint 計數；避免 4 bit 量化機率。 |
| B20 | `PER_KEY = 12` 探測（E-D） | research-log 2026-10-18 | 每個音節嘗試的詞庫條目數 12→24/48/1000 | chat 上 oracle 增益 <0.5 個百分點；formal top1 −2…−5 | 不適用 | 不採用 |

## 3. 神經重排（含 0c 移植）與 LLM 評審

見上方的 0c 說明。「M」是他們出貨的 14.4M 字元 LM；我們的是 v2b（31M）加上 v3。

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| B1 | 小型字元 LM 重排（S7a） | contracts/s7a-tinylm.md；experiments/s7a-tinylm/README.md；research-log 2026-10-11 | 以與 n-gram 相同的文字訓練 S（4L/256，5.66M）與 M（6L/384，14.4M）字元 LM，在 τ 閘門下以 `ng + α·LL` 重排 n-gram top-8 | M：discordtune chat 在 4,958 列中 +67 列（228/161），p 0.0008，CI [29,105]，ΔCER −0.23 個百分點；S：+31，p 0.149（未通過）。每字 ppl M 53.1（cv）／59.4（wiki）。numpy fp32 p95 44.8 ms | 有（做法不同：v2b 31M，12k 詞彙，ctx 64，top-16） | 已涵蓋（模型較大）；採用他們的融合閘門（第 2 列） |
| B2 | 閘控融合 `pick(α, τ, δ)` | core/src/tinylm.rs `pick`；s7c2c-delta.md | 只有在 n-gram 差距 < τ 時才重排，且只有在融合增益 ≥ δ 時才切換；平手時保留 n-gram 名次 | 出貨的 chat α 0.3 τ 2 δ 0.25 → 評審期 125/16（+109，改壞率 0.24%）；沒有 δ（0.7/2/0）時 273/142（改壞率 2.16%） | 沒有（我們：engine + 1.0·logP，只有 +4.0 按鍵完全相符差距） | **採用**：在 `NeuralReranker.swift` 加入 τ（engine top1−top2 差距）與 δ（融合增益），在聊天集上以 1:1 + 改壞率上限調參 |
| B3 | 更大的 LM L／XL（S7b） | s7b-larger-lm.md | 相同資料與做法，L 8L/512 30M，XL 12L/640 65M | L 在 discordtune chat 上對 n-gram −30（p 0.24），對 M −97（p<0.0001）；XL 在 8 GB GPU 上記憶體不足 | 不適用（我們的 v2b 是他們「L」的形狀） | **待確認**：L 約等於我們 v2b 的形狀，在相同資料下於真實聊天上輸給 M → 在我們的聊天開發集上以配對統計比較 v2b 與 6L/384 |
| B4 | int8 權重已足夠 | s7b §2.3；int8check.py | 逐列對稱 int8，f32 scale，載入時反量化 | M int8 對 fp32：12 格中沒有顯著差異；Rust int8 holdout 180→200/227（p 0.0003） | 有（fp16 CoreML） | 不採用：fp16 已經沒問題 |
| B5 | 推論後端調查 | research-log 2026-10-11（MLX） | 對按鍵當下的工作負載，排名為 Rust+Accelerate fp32 > MLX GPU > Core ML fp16 | Accelerate GEMM 下限 M 5.94 ms／L 13.0／XL 29.2 ms（8×17）；GPU 無法預先拉高時脈，Metal 每次安裝的首次呼叫都要編譯 | 有（CoreML CPU，Linux 上用 ONNX） | 調整後採用：維持 CPU；量測「打字節奏」下的 p95（呼叫間隔 150 ms），而不是連續呼叫 |
| B6 | Rust int8 推論 + `.sjtl`（S7c-1） | s7c-core-tinylm.md；tools/gen_tinylm_tiny.py；core/tests/tinylm_tiny.rs | 自訂的嚴格二進位格式，綁定 n-gram SHA（+ KN 附屬檔 SHA），以隨機小模型做跨語言 golden，8 個 mutation test | Rust 對 numpy \|ΔLL\| 2.05e-5；重排 p95 11–25 ms；int8 載入 10–21 ms；RSS +55–70 MiB | 有（CoreML + manifest） | 調整後採用：在模型檔頭綁定重排的 α/τ/δ + n-gram SHA，讓模型與 LM 不符時停用重排，而不是默默算錯分數 |
| B7 | 非同步兩階段重排工作（S7c-2a） | s7c2a-core-rerank.md | 引擎每次更新輸出一個只含資料的 `RerankJob`；外殼在 utility QoS 佇列上執行；以世代計數器 + 逐片段詞檢查丟棄過期結果；從不阻擋按鍵 | 按鍵 p95 2.65 ms（比沒有模型 +0.16）；執行 p95 8.3 ms；套用 p95 0.27 ms；閃動 11.37%（沒有模型為 14.50%），先還原再套用佔重新解碼按鍵的 2.15% | 沒有（同步） | 只有在我們 v2b 同步延遲 p95 > 約 16 ms 時才調整後採用；否則維持同步（0c 以 100 ms 期限同步執行） |
| B8 | 已學詞保護 | s7c2a §2.2；research-log 2026-10-20（0c App） | 若某片段的 n-gram 最佳路徑含有學習加分的詞，就不重排該片段；0c 則只否決會改寫使用者所選區段的候選 | 沒有單獨量測（離線評測集沒有學習檔） | 沒有 | 在 `NeuralReranker.swift` 中**採用**（0c 式逐候選否決），讓回想／個人 n-gram 的選擇不被覆寫 |
| B9 | 外殼佇列 + 自我測試（S7c-2b） | s7c2b-shell-rerank.md | 一次一個工作，只保留最新的，對 session／engine 使用弱參照，free-count 測試，內附的自我測試會輸入一列已知句子並斷言重排確實改變了它 | TSan 11/11 乾淨；實機檢查通過 | 部分 | 採用：一個斷言重排模型確實改變了已知句子的自我測試（可抓到模型默默沒有載入） |
| B10 | 調參集與訓練集重疊去除（S7d） | s7d-tune-overlap.md | 捨棄正確答案逐字出現在訓練文字中的調參列，重新選 α/τ | cvtune 重疊 13.4%，wikitune 23.7%；選出的值不變（M 0.7/2，L 2/2） | 不明 | 採用為檢查項目：回報我們開發集與 v2b／v3 訓練文字的重疊 |
| B11 | 10:1 改壞權重（S7e） | research-log 2026-10-12 S7e | 選擇使 fixed − 10×broken 最大的 (α,τ,δ) | Chat 0.2/1/0.25：discordtune 70/10，但與先前的值相比 dev302 276→249，user-reported 34→18 | 不適用 | 不採用：太保守；他們後來改用 1:1 + 改壞率上限 |
| B12 | 預設開啟，改壞率 ≤ 0.5%（S7h） | s7h-tinylm-v3.md | 重排預設開啟出貨；參數在自己的聊天調參月份上以 1:1 net 與改壞率上限選定 | R3′ 0.3/2/0.25：評審月份 125/16，UB 0.37%；dev302 chat 242→259 | 有（開啟） | 採用此上限作為發布閘門 |
| B15 | 三段強度（SJTL0003） | rerank-strength.md | 每個 profile 在檔頭存保守／標準／積極三組 (α,τ,δ) + C ABI + 設定中的分段控制 | Chat 積極 0.5/2/0 在 lytune 上 241/369 未通過 → 改寫為標準；formal 積極 0.5/1/0.25 保留 | 沒有 | 之後調整後採用：一個「強度」旋鈕 = δ/τ 預設組 |
| B17 | 在全新 wiki 資料上重選 formal 參數 | rerank-formal.md | 在 trigram 解碼器之後重選 formal 的 α/τ/δ；在從未用過的條目區段上評審（wikijudge 310k–315k、wikijudge2 …） | 標準 0.5/2/0.25 通過主評審，但改壞率 1.72%（上限 0.5%）；保守 0.7/1/1.5：wikijudge 上 +20/−0，改壞率 0.14% | 沒有 | 在我們的 wiki 開發集採用「每次決定都用全新的 Wikipedia 條目區段」這個做法 |
| B18 | 重排模型大小 L／XL 重新評審 | rerank-size.md；research-log 2026-10-17 | L188、LCOLAB（平台雜訊）、XL 對 M，在未見過的 chatjudge／wikijudge3 上，Holm 校正 | 5 項全部未通過（最佳 p 0.063）；XL 改壞率 0.54%／0.52%。診斷：XL 單獨選對率在 wiki 上 59.7% 對 M 50.9%，但 chat 上 46.2% 對 44.1%；tokens／參數 M 17、L 8、XL 4。延遲 p95 M 8.9／L 17.9／XL 33.9 ms；RSS +68／+141／+245 MiB | 不適用 | 備註：模型越大只在 wiki 上越好；聊天需要的是資料，不是參數 |
| B21 | X0c 離線比較 | x0c-compare.md；research-log 2026-10-13 | 以相同的選擇規則，在善解的 top-8 上以 0c 計分 | Chat 對 tinylm-v3：評審 +142，discordtune +127，user-reported +11（p 0.027）；wikitune −88，lytune −56。Formal X0C-F 對 v3：discordtune +51，lytune +61。0c 自己的融合（α 2，τ ∞，δ 0）在 lytune 上 −706 | 不適用 | **採用為量尺**：在我們的 top-16 上，以口語／wiki／Taiwan Tongues 開發集為 0c 計分 |
| B22 | 0c 的 Rust 移植 | oc-experimental.md；core/src/oc.rs；tools/convert_oc.py、oc_ref.py | 見「0c 說明」 | f32 mmap 130.7 MB；前綴 KV 共用後 p95 33.9 ms（64 ctx）（之前 74.5）；RSS +227.5 MiB | 不適用（我們有 ONNX Runtime 橋接） | 調整後採用：若要執行 0c，使用我們的 `ONNXBridge`；把他們的前綴共用想法用於 v2b 的批次處理（第 23 列） |
| B23 | 候選之間共用前綴 KV | oc-experimental §2 A3 | 共用前綴每列只算一次；每個候選的後綴對快取的 K/V 做 attention | 0c p95 74.5 → 33.9 ms；RSS 273 → 227 MiB；metasequoia 4M 重排模型回報跨按鍵前綴重用使 p95 20.6 → 8.6 ms | 有（v2b 的前綴樹批次處理，`RerankerTree.swift`） | 已涵蓋；可考慮跨按鍵快取 |
| B24 | 0c 用在目前的候選上（P1） | research-log 2026-10-18 P1 | 在 0.5% UB 規則下重新量測 0c 對出貨的 M | discordtune 0c 對 M 165/60，net +105；但 0c 的 UB 0.61%（>0.5）→「未通過」；調參 UB 0.46% → 開發集 0.61% | 不適用 | 備註上限無法轉移的問題（第 41 列） |
| B25 | 把 0c 蒸餾到 M（P2） | experiments/p2-distill/README.md | 在 8 個候選上做 KL(q‖p)，教師 = 0c，從 M 起始，287,914 列（wiki 200k–300k + 語料池） | 保留集 KL 1.27→0.38，top1 一致率 68.2→79.8%；discordtune 對出貨的 M 55/57（−2） | 不適用 | 不採用：候選層級的 KL 無法轉移 |
| B26 | 蒸餾 B／C／L | research-log 2026-10-19 | B = 加入讀音 embedding；C = 去掉 wiki 列；L = 30M 學生模型 | 在 discordtune 上對出貨的 M：B −2，C −35，L +7（全部未通過） | 不適用 | 不採用：對因果學生模型而言，瓶頸既不是讀音也不是容量 |
| B27 | 位置層級蒸餾（T／R） | research-log 2026-10-19 | 教師 = 0c 在同音字集合上的逐位置分布；R = 意見不一致的列加權 ×4 | 位置 top1 一致率 87.6→92.5%，但 discordtune T −41（p 0.0001），lytune −110；R −4 | 不適用 | 不採用：逐位置的增益無法組合成句子排序 |
| B28 | Listwise 目標 L0 | research-log 2026-10-18 L0 | 以 M 為 encoder，在「保留第 1 名」+ 第 2–8 名中選擇，以 n-gram 差距與名次為特徵，listwise CE | 退化為保留第一名：保留率 98.5%，選對率 19.1%（M 45.1%）；discordtune 對出貨版 −38 | 沒有 | 已做過，不採用；若我們訓練 listwise，要拿掉 n-gram 特徵或加正則化（他們懷疑是走捷徑） |
| B29 | 以讀音為條件的因果 LM（derisk E2） | derisk-1015 §E2；experiments/derisk-1015/e2/README.md；research-log 2026-10-17 | 在每個因果位置加入*下一個*字的音節 embedding（+0.54M 參數） | N2 對 C2 chat 144/215，net −71，p 0.0002；formal −70。缺陷：Σ log P(c_i \| c_<i, R_≤i) ≠ P(c \| R) | 有（做法不同：v3 雙向音節 encoder） | **已涵蓋**；他們的負面結果與「一開始就提供完整讀音，而不是以因果方式逐步輸入」的做法一致 |
| B30 | 讀音前綴 LM（E-B） | research-log 2026-10-18 E-B | `[BOS] k [RD] r_1..r_n [OUT] c_1..c_n`，softmax 限制在同讀音的字 | 調參 net +32，對照組 +91；選對率 41.5% 對 44.4%；discordtune P 對 C 39/67 | 有（做法不同：v3） | 已涵蓋；但要注意：在 14M／255M tokens 的規模下，只靠讀音前綴沒有幫助 |
| B31 | 重排模型的長 context | P3（experiments/p3-context）；E2 rev7 C16；research-log E-1 | 以 ≤63 字、跨越標點的 context 重新訓練學生模型；或不重新訓練，直接餵給出貨的 M 16／64 字 | S64 重新訓練：discordtune −39（p 0.0005）。M@64 未重新訓練：chat ±0，lytune +20（p 0.03）。C16：79/78。0c 的 context 約值 +20 列 | 有（做法不同：v2b ctx 64；v3 ≤255；讀取 128 字） | 已涵蓋；他們結果的提醒：訓練與評測的 context 長度不一致會損失列數，確認我們的評測集帶有實際的 context（我們有 `--line-context`） |
| B33 | 學習式閘門（E-F） | research-log 2026-10-18 E-F | 以數值特徵（n-gram／LM 差距、名次、softmax 機率、候選數）做 L2 邏輯迴歸，預測「切換是否有幫助」 | 在 chat 上比同規則多 +13 net，但在 discordtune 上對出貨參數 −26；UB 0.61–0.63%（無法轉移）；lytune 改壞 55 | 沒有 | 調整後採用：若加入學習式閘門，要包含字串特徵（差異字類別、句首、多字詞庫詞），只用數值特徵已失敗 |
| B34 | 解碼器權重與融合聯合調參（E-C） | research-log 2026-10-18 E-C | λ × KN β 網格，並重選融合參數 | Chat 最佳格比出貨版 +5（停止）；formal 最佳 = 出貨版 | 有（混合權重 c） | 不採用 |
| B35 | 以 LLM 評審做離線貝氏／加權融合 | fusion-offline.md；experiments/fusion/README.md | 比較 n-gram、評審單獨、τ 閘門、加權 `ng + w·judge`、單參數貝氏 | Qwen3-8B 在 discordtune B 半：加權 89.2% 對 n-gram 84.4%（33/9，p 0.0003）；對 τ 閘門 13/6，p 0.17；貝氏 ≡ τ = T·ln(7a/(1−a)) 的 τ 閘門；ECE 0.05–0.10（過度自信） | 有（加權） | 維持加權融合；閘門在其之上增益很小 |
| B36 | Apple Foundation Models 作為評審（S5j） | s5j-judges.md；experiments/s5-judges/README.md | 裝置上的 LLM 以編號從 8 個中選 1 個 | discordtune A-fwd −81（57/138）；順序反轉 −342；正序時 72% 選第一個，反序 12%；加入 context 更差（17/108）；p95 470 ms。dev302 +27（有誤導性） | 不適用 | 不採用；教訓：清單選擇式 prompt 有位置偏差；由作者自己撰寫的開發集會誤導 |
| B37 | Jev（TypeSafe）雲端評審 | experiments/jev/README.md；s5j | 8 選 1，逐位置變體 | J1：unigram 52% → 81%（dev300）；S5j cvtune J-sent-fwd +46（67/21），翻轉 11%；每個請求 p50 約 210 ms | 不適用 | 不採用（雲端；隱私） |
| B38 | 本機小型 LLM 作為 log-lik 計分器（S5k） | s5k-local-scorers.md；experiments/s5-local/README.md | Laya（322M mmBERT）、1-bit Bonsai 1.7B/4B、Qwen3-1.7B 4-bit，以 log P(prefix+cand) − log P(prefix) 計分 | Laya 11–20%（無用）；1-bit 比 4-bit 低 11 個百分點；Qwen3-1.7B cvtune +41（p 0.0003），discordtune 閘控 +14（p 0.081）；context 有幫助 32/8；p95 510–640 ms | 有（做法不同：自有 v2b／v3，在數十 ms 以內） | 不採用；我們裝置上的模型以更低延遲涵蓋 |
| B39 | Qwen3-8B nf4 計分器（Q8-ll） | s5k §11；research-log 2026-10-06 | 在 RTX 3070 上以相同方式計分 | 閘控 + context 半集 B 34/18，p 0.037（唯一通過，未校正約 14 項檢定）；1.7B 為 17/15，p 0.86；每列 0.5–0.6 s | 不適用（我們只用 Qwen3-8B 產生合成資料） | 不採用為評審；只可能作為離線教師模型 |
| B40 | Clef-flash 9B／Clef 27B（S5c） | s5c-clef-cloud.md；experiments/s5-clef/README.md | Cloudflare 決策模型，相容 Jev 的 API | flash 正序 62.3% 對 rank1 85.1%（89/317）；反序 88.3%；翻轉 54%（27B 36%）；比 Jev 差；每個請求 p50 0.4–1.8 s | 不適用 | 不採用 |
| B41 | 雲端 prompt 變體（S5p） | s5p-cloud-prompts.md；experiments/s5-prompts/README.md | 具名 JSON 狀態 {context, reading, candidates}、反引號路徑、中文對英文、選擇式對逐候選 noul | 最佳 = 結構化選擇（Jev V4 中文 A 半 91.2%）；逐候選 noul 最弱；flash V4 對 V0 +125；全部約比 rank1 +20–33；每 1k 列 $0.04–0.12 | 不適用 | 只在未來若有雲端「修正句子」功能時調整後採用：送出讀音 + 結構化狀態，兩種順序都要測 |
| A50 | LLM N-best 重排對讀音限制生成（N0） | experiments/n0 | M1 在句尾以本機 LLM 重排 unigram top-32。M2 以讀音限制的 beam search 生成。 | dev302：Gemma-4-E2B M1 91.4%（p50 295 ms，RTX 3070），Qwen3-1.7B M1 87.1%（153 ms），M2 最佳 82.5%。Unigram 51.7%。Mac MLX Qwen 4-bit p50 487–893 ms。 | 有（v2b 重排模型，31M） | 與我們的設計選擇一致：重排 N-best，而不是限制式生成。M2 也偏好流暢但錯誤的寫法（試鏡布、藍芽）。 |
| A51 | 雲端 LLM 在 16 個候選上評審 | experiments/providers | 在溫度 0 下，對 lattice 前 16 個候選只回答索引的 JSON。 | 最佳 92.7%（gpt-6-luna low；oracle@16 94.7%），每 1k 句 US$0.036–0.99，p50 0.3–4 s。 | 不適用（隱私、離線） | 不採用。只適合作為離線評測標註時的上限 oracle。 |
| B43 | 在／再 分類器（P1a） | p1a-confusion.md；experiments/p1a-confusion/README.md | 每個讀音一個邏輯迴歸，使用遮罩位置特徵（左右字、詞、Brown 類別），以字元位置計分，包含多字詞內部 | 第 1 階段 STOP：再>在 11→10；分類器在 ㄗㄞˋ 位置上 692/729 < n-gram 711；根本原因在 n-gram：P(在\|<s>) 是 P(再\|<s>) 的 26 倍，以 在／再 結尾的詞庫詞為 570 對 36 | 有（ml/src/zhuyintype_ml/training/homophone 在／再 的工作） | 調整後採用他們的診斷：在訓練分類器之前，先檢查標點／數字之後的句首重設與詞庫結尾偏差；類別平衡抽樣會抹掉先驗 |
| B52 | LM 用的合成 + 真實口語語料（S7f、E-A） | s7f-colloquial.md；experiments/s7f-corpus/README.md | 20M 字真實資料（archive.tw CC0、Cofacts、zhwiki 討論頁）+ 5M 字合成資料（gpt-oss-120b、Gemma 4 31B，以中國用語「入侵」篩選），固定步數預算，以 A0′ 重新訓練作為雜訊底線 | 10:1 計分：A0 −21，A0′ −53，A1 −126，A2 −107（未通過）；A0 與 A0′ 相差 32 = 平台／seed 雜訊。之後 1:1、3 個 seed：A2 調參 net 總是 > 對照組最大值（+104 對 +76），但 chatjudge 上的效果只有幾列（< MDE +32） | 有（5.9M 句合成 + CV + Taiwan Tongues 聊天 bigram） | 調整後採用：在認定某語料有效之前，一定要訓練一個相同資料、換 seed 的對照組，並至少跑 3 個 seed |

## 4. Chat／formal profile 與口語證據

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| B44 | 口語表，n-gram 層（P4） | experiments/p4-colloquial/README.md | 獨立的僅 chat 用 bigram T2（model-v6 口語計數 + 143,895 句公開語料池），作為對數線性項 μ·lc − γ·k(overlay 詞)，在融合之前套用 | n-gram 層 discordtune 168/89（+79）；D3 (iii) 救回 24/97 | 有（c·P_chat + (1−c)·P_wiki 內插） | 見 #46 |
| B45 | 融合前的口語項失敗（P4b／P4c） | experiments/p4b-、p4c-colloquial/README.md | 先依 s 重排，再與 LM 融合 | P4b discordtune 對出貨版 128/92（64 個改壞來自 s 本身：一戰→一站、聖地→勝地、致病→治病）；P4c（α 0.5，δ 0）129/78，比例 0.60 | 不適用 | 教訓：沒有閘門、可直接取代 top-1 的聊天先驗會「搶走錨點」 |
| B46 | 在重排層加入以錨點為準的口語項（P4d → 常談 SJ-CL-1-68K） | experiments/p4d-colloquial/README.md；colloquial-term.md；core/src/colloquial.rs | `ll_eff_i = ll_i + g_i·(μ/α)·(col_i − col_0)`，col = lc − 0.5·k，軟閘門 g = σ((ll_i − ll_0)/T)，n-gram 第 1 名維持為錨點，接著照常 `pick`；μ 0.5，T 1.0，α 0.5，τ 2，δ 0.25 | chatjudge-2（全新）108/39，net +69，p 1.1e-8，CER 2.85→2.61%；dev302 chat 9/2；RSS +9.2 MiB，每列 +0.17 ms；「只有融合」對照組 61/38 → 此項約貢獻 +46 | 部分（chat LM 混在 lattice 中，不是閘控的重排特徵） | **採用**：在重排模型*內*加入只用 chat 的 bigram 對數線性特徵，相對於 top-1，並以神經模型的一致程度閘控（`NeuralReranker.swift` + `NGramLanguageModel.swift`） |
| B47 | 各種強度下的口語項 | colloquial-levels.md | 搭配口語項，每種強度各自的 (α,τ,δ) | 積極（0.5,2,0.1）對舊積極 discordtune 108/42；保守（1,2,3）17/4 | 不適用 | 若加入強度選項，與 #15 一起採用 |
| B49 | 依 App 決定 chat／formal + 逐 App 覆寫 | profile-choice.md | 依 bundle ID 決定 profile（8 個內建聊天 App），有 自動／一律 chat／一律 formal，以及逐 App 的選單切換，切換時重新解碼目前的組字；只儲存切換過的 bundle ID | 實機通過；子系統記錄中含 bundle ID 的行數為 0 | 部分（我們依前文調整 c，沒有逐 App 覆寫） | 調整後採用：保留我們依文字自適應的混合方式，加入逐 App 的「chat／formal」覆寫來固定 c |
| B50 | lytune 退步歸因 | lytune-attrib.md；research-log 2026-10-16 | 在固定集上做 10 組因子設計（模型、類別、KN、重排模型） | v0.4.3→0.4.4 chat −45：其中 −36 在重排階段；n-gram 的損失來自 Brown 類別 × KN 的交互作用（formal −56）；修正為 μ 0.8→0.2 | 不適用 | 採用此方法：guard 集退步時做因子歸因 |

## 5. 個人學習

善解的學習範圍窄（只學重選、≤2 個漢字的 context 鍵、固定加分）。ZT 的學習範圍廣（每次送出都更新個人 uni／bi／tri-gram、回想、錯字習慣）。

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| DL1 | 只在重選時學習 | docs/contracts/s4-learning.md §1.2；core/src/learn.rs | 只學習經候選字視窗選字修正、且顯示文字 ≠ 所選詞的片段（加上預測列的選字，v3-engine §10.1）。在 Enter、規則 21 或達上限自動送出時學習。重設、Esc 或焦點切換時從不學習，因為這些常與安全輸入切換同時發生。標點從不學習。 | 反事實「每次送出都學習」（newword-b §0）：317 句原本正確的句子中有 80 句學到片段，因而弄壞 3–6 句 | 有（做法不同）（ZT 的個人 n-gram 每次送出都學；修正會成為回想） | **調整後採用**：保留 ZT 的廣泛 n-gram，但在 ZT 上跑善解的片段反事實實驗。依序教開發集，統計「原本對、現在錯」。若此數 > 0，讓沒有經過選字就送出的文字權重遠低於選字 |
| DL2 | ≤2 個漢字的 context 鍵，共用函式 | s4-learning §1.1；learn.rs `context_key`、`record_key` | 鍵是詞前面最多 2 個漢字的尾段，遇到任何非漢字就停止。解碼器與學習器呼叫**同一個**函式。有 4 種情況的單元測試：單字詞之後、固定詞之後、標點之後、句首。查找順序：完全相符 → 最後一字 → 全域。 | 在 `eval/learn/cases.tsv` 上鍵的可達率：13/14（92.9%）。先前的鍵設計只達到 11/14 | 有（做法不同）（ZT 以 history 詞為鍵，context 為 2 個已送出的詞） | **採用此測試**：在 `engine/swift/Sources/ZhuyinCore/Engine.swift` `learn(commit:)` 中，對這 4 種情況斷言學習時的 history 等於解碼時的 history |
| DL3 | 固定加分 + 鏡像測試 | lm.rs `LEARN_EPS` 文件；s4-learning §1.4、§6.2 | 學到的詞分數變為 `max(own, best_same_reading + ε·w/(w+1))`。即使學到的詞在 top-12 `PER_KEY` 之外仍會被列舉。「鏡像測試」在同一位置重選該對中的另一個詞，再檢查相同 context 的罕見句是否跟著改變。 | ε=2.0 → 4/10，4.0 → 8/10，6.0 與 8.0 → 9/10，因此選 ε=6.0。之後的變更後：7/8 | 沒有（沒有鏡像測試或相同 context 學習率測試） | **採用**鏡像測試框架作為 `zhuyin-cli` 的一個模式：教一次選字，重播相同 context 與不同 context 的列，回報學習率與退步 |
| DL4 | 全域層污染 → 關閉 | s4-learning §12–13；lm.rs `LEARN_EPS_GLOBAL=0` | 在 ≥2 個不同 context 下出現的記錄會變成不分 context 的「全域」記錄。在 可以 與 ^ 下教 再（取代 在）後，各處的 在 都被翻轉。 | 修正前：54 個不衝突的 ㄗㄞˋ 列中，35 列改變、24 列退步（另外現在在做 → 現再再作）。ε_global 掃描，以（學習率，退步數）表示：0 → 0/7, 0；0.5 → 6/7, 0；1.0 → 7/7, 1；1.5 → 4；2.0 → 6；6.0 → 21。加入詞類後，即使 0.1–0.5 也退步 1–3 列，因此關閉全域層。 | **有風險**：ZT 的個人 unigram 與長期層不分 context，作用類似這個全域層 | **採用** ㄗㄞˋ 污染測試：在 2 個 context 下各教 再 一次，再重播 現在在做、我現在在家、他在學校 與每一列 ㄗㄞˋ 開發集。要求不衝突的列 0 退步。若未通過，閘控 unigram／長期層的貢獻 |
| DL5 | 單字需要恰好 2 個漢字的 context | learn-single-ctx.md 規則 7；learn_store.rs `valid` | 1 字詞只在完全相符的 2 漢字鍵下學習與加分。從不使用最後一字層級、^ 或全域。 | 起因：記錄 你｜ㄗㄞˋ｜再 把 你在 變成 你再。套用此規則後，維護者的檔案少了 18 筆記錄。代價：在一個漢字之後重選的單字不再被記住 | 不明（ZT 對單字的個人 bigram 有相同風險） | **採用**為測試案例。考慮要求單字的個人證據必須有 trigram context |
| DL6 | 句首鍵帶上下一個音節 | learn-single-ctx.md 規則 8；learn.rs `next_key` | `^` 涵蓋句首、標點之後、英文之後與「沒有讀到 context」，太粗糙。在 ^ 學到的 ≥2 字詞，以 `^>` + 下一個音節的讀音為鍵（結尾為 `^>$`）。英文與數字鄰居正規化為 `^>_punct_en`／`^>_punct_num`。 | 以真實按鍵重現三個使用者錯誤（我試買新的、我再牛與、我試打這）。以此規則修正 | 沒有（ZT 把句首 history 視為單一狀態） | **調整後採用**：ZT 在 `<s>` 的個人 bigram 也以右側鄰居為條件，或在 `<s>` 不學習短於 3 字的 token |
| DL7 | 在選字當時的鍵下減半被取代的詞 | s4-learning §1.3、§14 規則 10–12 | 重選時舊詞權重減半，新詞 +1。若鍵在選字與送出之間改變，也在選字當時的鍵下減半。計算全域化的 context 數時，所有 `^>…` 鍵視為同一個。 | 若沒有這條規則，「在結尾重選後繼續打字」產生的錯誤記錄永遠不會減弱（程式碼審閱發現） | 有（ZT：刪除重打會減半；選字時 `weaken`） | 隱性修正（30 秒內刪除重打）**已涵蓋**。檢查 ZT 的 `weaken` 使用的是該詞**顯示**當時有效的 history |
| DL8 | 貝氏後驗學習（負面結果） | s4-bayes.md §1、§8 | 以 `p = (α_c·g + c_x)/(α_c + C_x)`、`g = (α_g·q + c_g)/(α_g + C_g)` 取代固定 ε，其中 q 為 LM 的分布。兩個變體：R（只提高）與 D（重新分配）。40 組網格設定。 | **沒有一組**符合要求。鏡像測試：R 0–2/8，D 0–3/8，基準為 7/8。D 在 α_g=4 時另有 1/62 的全域污染。已還原（commit e469768） | 有類似做法（κ 內插 1000/100/1） | 重選加分**不採用**純後驗：單次選字無法勝過有信心的 LM。在鏡像式測試集（L3）上重新測試 ZT 的 κ。若一次選字無法翻轉相同 context，加入像善解那樣的固定差距加分 |
| DL9 | 衰減、剪除、容量 | learn.rs 常數 | 半衰期 14 天；啟用門檻 0.5；低於 0.05 剪除（約 60 天未使用）；容量 50,000；新詞上限 2,000。估計重度使用者的穩態約 3,000 筆記錄（天數 × 不同選字數 × 60）。 | 50k 筆記錄時每鍵 p95：1.8–2.1 ms，空的時候 1.3–1.5 ms | 有（做法不同）（8 小時短期 + 90 天長期） | **已涵蓋**（雙時間尺度）。若 ZT 的檔案無限制成長，借用 PRUNE_FLOOR 加硬上限的做法 |
| DL10 | 只附加的日誌式儲存 | learn_store.rs 檔頭；s4-learning §4、§11 | 每次 Enter 附加 ≤3 行（`O_APPEND\|O_NOFOLLOW`，fstat 檢查）。在當天第一次寫入、日誌達 1,024 行、遺忘或失敗之後做完整重寫（tmp + `F_BARRIERFSYNC` + rename）。載入時以最後一行為準。 | 每次 Enter 都以 F_FULLFSYNC 完整重寫：50k 筆記錄時 p95 16.0–19.7 ms，觸及停止條件。F_FULLFSYNC：p50 4–7、p99 35、最大 55–115 ms。附加：p95 ≤ 0.2 ms。BARRIERFSYNC：p50 0.9–1.6 ms。序列化 50k 筆記錄約 7 ms | 有（做法不同）（ZT 每次學習都在鎖內為**所有**檔案做快照並序列化，再於其他執行緒以原子方式寫入） | **調整後採用**：以 20k 筆回想條目加上 n-gram 量測 `Engine.swift` 中 `scheduleWrite()` 的時間。若在按鍵路徑上超過約 2 ms，把序列化移出鎖，或改用附加式日誌 |
| DL13 | 就地遺忘（⌘⌫）與清除對話框 | s4-learning §1.5、§4；v3-engine §10.3 | 在反白的候選或預測項目上按 ⌘⌫，會在所有鍵下遺忘該 (reading, word)、重新解碼，並捨棄它尚未處理的學習。遺忘一定會強制完整重寫檔案。清除對話框預設為取消，並說明 Time Machine 中的副本仍會保留。 | 程式碼審閱發現的錯誤：被遺忘的詞在下一次 Enter 時，因一個仍待處理的選字而被重新學習 | 部分有（ZT 從設定清單中遺忘） | 在 `Composer.swift` + `CandidatePanel.swift` **採用**候選中的 ⌘⌫，並捨棄被遺忘詞的待處理選字 |
| DL14 | 新詞 B：從逐字重選學習 OOV 詞 | newword-b.md；learn.rs `NEWWORD_*` | 逐字重選後，取包含這些選字的連續單字詞段，在功能字（`NEWWORD_BREAK` 清單）處切開，2–4 個音節、尚未在詞庫中，且不會遮蔽分數 > −6.5 的詞庫詞。以不分 context 的 `#` 儲存，分數 S ∈ [−3, −1] 依權重而定。再次使用 → 每天最多 +1 一次。 | 約 25% 的使用者錯誤回報是缺詞。19 句探測：基準 7 → 16（chat）、17（formal），p 0.004／0.002。錯誤回報：+4/−0。dev302：0/0 與 +1/0。S 掃描 Σ net：[−5,−3] 181，[−4,−2] 186，[−3,−1] 190 | 部分有（ZT 回想會儲存修正時實際輸入的讀音；沒有多次選字的區段組合） | 在 `Engine.swift learn(commit:)` **採用**區段組合規則加上 `NEWWORD_SHADOW` 防護。沿用 19 句探測的想法 |
| DL16 | 端到端重播中的學習效果 | research-log 2026-10-18；engine-replay.md §5 | 以模擬的重選與 `--learn on/off`，透過真實引擎重播各列。 | dev302 chat：+1/−1。dev302 → 錯誤回報串接（437 列）：首次顯示 chat 301 → 304（4/1），formal 310 → 313。KySS 0.9455 → 0.9467。在公開評測集上學習效果很小；需要依時間排序的真實輸入 | 沒有（沒有學習開／關的重播） | **採用**：`zhuyin-cli` 依檔案順序以 `--learn` 重播，回報「原本對、現在錯」 |

## 6. 預測與補全

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| DP1 | Context + 部分注音預測（SP1） | experiments/sp/README.md、predict.py | 候選是與已輸入單位相容（前綴或縮寫）的詞庫字串，以 `lm.word(λ, v, W, lp_max)` 計分，v = 來自 context 的 history。位置：P1 = 第一個按鍵，P3 = 第一個音節完成。 | cvtail chat，n=390，a（詞庫分數）／b（LM + `<s>`）／c（LM + context）：P1 hit@9 6.4／4.6／**25.4%**，P3 43.6／42.6／**65.1%**，KS(9) 33.5 → 45.0%。discordtune：P1 9.9 → 20.8，P3 54.2 → 64.1，A1 81.4%，A2 97.1%，第一名翻轉率 37.8%。查詢 p95 約 40–60 ms（Python） | 沒有（Tab 補全在路線圖 0.6） | **採用**：讓預測有用的是 context（第一個按鍵 +20 個百分點） |
| DP2 | 只用後繼詞作為候選（負面結果） | sp2-successor-prediction.md；README「第二片」 | 候選限制為 LM 中 v 的後繼詞。 | P1 大致不變。P3 在 8 格中有 7 格顯著變差（−5 到 −19 個百分點；discordtune 43.5 對 64.4%）。顯示的列中有 30–44% 不含答案 | 沒有 | **不採用**：絕不把候選限制為 bigram 後繼詞 |
| DP3 | 後繼詞優先的分層（S） | sp3-successor-first.md；predict3.py；core/src/predict.rs `predict` | 第 1 層 = v 的後繼詞（依分數）；第 2 層 = 其他相容的詞（依分數）。確定性的平手鍵（−score, length, reading, word）。 | 從未顯著差於全部依分數排序（c）。P1 在 4 格中 +1.6–7.0 個百分點。對只用後繼詞：P3 +6.1–18.5 個百分點。discordtune P1 22.4 對 21.3（44/17）。Python p95 ≤ 105 ms | 沒有 | **採用**此排序。在 ZT 中，第 1 層依個人 + chat／wiki bigram 後繼詞排序 |
| DP4 | Rust 移植與 Python 位元一致 | v3-core-predict.md；predict.rs `Index`；`eval/golden/sp-predict.txt` | 以第一個音節的字建前綴桶；延遲建立詞庫反向索引；與 Python 產生的 golden 逐位元比較（f64），並有必須失敗的 mutation (a)–(c)。 | 要求 p95 ≤ 20 ms | 不適用 | **調整後採用**此方法：任何新預測器都從 ZT 的 Python 參考實作產生 golden 檔 |
| DP5 | 被動預測列的 UX | v3-engine.md §1–§5；predict-down.md B.3.2 | 從第一個按鍵就顯示。未進入的列：沒有反白；第一格顯示 ⇥（U+21E5），按 Tab 後變成 1。Shift+Tab 或 Esc 離開。點選未進入的列直接選字。在已進入的列中按 Backspace 會離開並刪除最後一個符號。只在注音、聲調或 Backspace 鍵之後重新計算；其他任何狀態變化都會清除。 | 使用者在實機上驗證（TextEdit、Discord、VS Code）：我想喝一杯 + ㄋ → 奶茶 | 沒有 | 在 `platforms/macos/Sources/ZhuyinTypeApp/UI/CandidatePanel.swift` + `ControllerCore.swift` **採用**：使用相同的 ⇥ 字符與固定寬度的編號欄，讓格子不會跳動 |
| DP6 | 猜測詞的起點 | v3-engine §1.2、§11 | 查詢多個起點：游標（待處理音節）、最後解碼的詞、前一個詞、5 個音節內的每個詞起點（`PREDICT_BACK`），以及詞中起點（最後附加）。長起點在前段最多 3 項（`PREDICT_LONG_CAP`）。從不在使用者固定的詞內部起始。 | 原規則對 oracle 起點，typing76 n=217（第一鍵／第 2 音節第一鍵／第 2 音節完成）：26.7/82.5/98.2% 對 29.0/95.9/99.5%。往回 5：typing76 第一音節完成 64.6 → 67.5（8/0）；錯誤回報 50.8 → 55.4（6/0）；ACG 名稱 57.9 → 71.7%（453/0），往回 6 為 73.0%。每個狀態的 `predict` 呼叫次數 2.34 → 4.21。50k 筆學習記錄時 p95 2.92–3.18 ms | 沒有 | **採用**，往回 5，詞中起點放最後（其他排序讓新詞第一鍵退步 −4） |
| DP7 | 學到的選字跳到最前面 | v3-engine §10.2 | 預測的選字會被學習（與重選共用儲存）。學到的項目依權重穩定排序到最前面。當可能有學習記錄相符時，以上限 27（`PREDICT_SCAN`）而非 9 查詢 `predict`。 | 分數加成無法跨層：權重 1 的加成 = +3.0 lp，只把第 9 名移到第 7 名。因此改為「移到最前面」。14 個測試（T1–T14） | 沒有 | **採用**：學到的項目排在最前面，而不是加分數 |
| DP8 | ↓ 視窗 = 預測列 + 詞庫 | predict-down.md B.3.1 | 預測列顯示時按 ↓／Space，開啟的視窗先列出預測列項目（相同順序），再列出去重後的詞庫項目。有待處理音節時只列預測列項目。讀音等於已輸入音節的項目視為重選（減半、凍結、新詞學習）。 | 修正錯誤回報第 133 列（可以做第六步）經由預測列路徑退步的問題 | 沒有 | 預測列落地時**採用**，讓 ZT 的兩份清單永遠一致 |
| DP9 | 縮寫組字（出貨後隱藏） | v3-engine §6、§12 | 「省略韻母」：ㄋㄔ → 奶茶。兩個以上的單位會吃掉聲調鍵與 Space。`ABBR_MAX_UNITS` = 10。 | 實機測試後隱藏：不能整句縮寫，且有按鍵卡住的回報。已知缺口：沒有聲母的音節（ㄢ）無法開啟一個單位，因此 13–16% 的 2 字以上詞無法這樣輸入 | 沒有 | **不採用**（或只作研究）。無聲調輸入（ZT 0.7）是我們規劃的路線 |
| DP10 | 預測開關與 Tab 語意 | v3-engine §3、§10.5 | 預測列為空時 Tab 維持舊行為（送出 + 傳遞按鍵）。選單開關「即時預測」，預設開啟。詞中起點讓每 1000 個狀態中有 1–5 個非空，改變了 Tab 的作用。 | typing76 2/904 個狀態，錯誤回報 1/432 個狀態 | 不適用 | **調整後採用**：ZT 的 Tab 接受必須精確定義預測列為空時的 Tab |
| DP11 | 英文 → 中文下一詞表 | english-next.md；core/src/english_next.rs；predict.rs `predict_english` | `P(w\|e) = (N(e→w) + γ·P(w)) / (N(e) + γ)`，γ=1。有待處理單位時：第 1 層 = e 的後續詞，以 `λ·logP + (1−λ)·lp` 計分。沒有時：只有後續詞。閘門：≥20 次事件、≥5 個 PTT 看板、每個 (e,w) ≥3、每個 e ≤20 個 w。也使用游標前 App 文字中的英文詞（context-return）。 | 設計目標：Joe → 是要對決。契約的出貨規則：在 PTT 1/8 holdout 上 McNemar 不變差，chat net > 0。契約中沒有結果 | 沒有 | 之後**調整後採用**（需要先有中英混打） |
| DP12 | 給長 context 模型的 context-return | context-return.md；composition-cap.md | 外殼傳入同一行 ≤64 個字素。核心只保留模型宣告的長度（`.sjtl` 檔頭欄位 L）。舊模型視為 L=2，因此輸出逐 byte 相同。資料最小化：`left_full` 從不跨越 ABI，也從不記錄。 | 延遲預算：此分支在 leftContext + setLeftContext + 第一鍵上增加的 p95 不得 ≥2 ms | 有（ZT 的 v3 與 v2b 已使用 128-255 字） | **已涵蓋**。採用「模型宣告 context 長度」的檔頭欄位與結尾只取漢字的規則 |

## 7. 數字、英文與混合輸入

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| DD1 | 數字列按鍵依分數判為數字 | digits.md A.3.1–A.3.4；core/src/digits.rs；experiments/digits | 字母表 `0-9 . / -`。最近 ≤4 個按鍵留在緩衝區（較舊的輸出為數字 token）。每種切分「m 個數字 + t 鍵的合法音節尾段」都計分：`lpD = logP(D\|C) + logP_len + ΣlogP_ch`，`lpZ = log Σ P(w\|C′)`，對相容的詞求和。已完成音節的純注音切分得到 θ=3。數字 token 的讀音為 `_punct_<d>`（句子邊界）。 | 新評審集 1,494 列：最終準確率 **89.8%**，結構規則為 85.2%。dev302／typing76 上的誤判數字 = **0**。暫時顯示為數字：每 1000 個音節 2.74 與 7.95 次。翻轉 0/1/2/3+：755/443/278/18（結構規則 313/403/337/441）。θ 掃描誤判數字（c）：0→61、0.5→18、1→4、2→2、3→0 | 沒有（ZT 把數字鍵只當注音） | **採用**：對聊天價值高（14.0% 的訊息含數字串，每 100 則訊息 25.9 串）。與 ZT KeyDecoder 的錯字修復有交互作用，因此把數字當作 lattice 中的一條競爭路徑計分，而不是預先切分 |
| DD2 | 數字 context 預測列 | digit-context.md；digits.md A.3.5；`data/lexicon/digits.tsv` | 數字之後，預測列使用附屬表中的 `P(w\|v,D)`（詞、數字串、下一個詞）（1.56 MB，k=3，以 200k 篇 wiki 條目 + 口語資料建立）。從不影響解碼。 | hit@9 chat 54.8 → **85.7%**（462/1，p 3.9e-137），formal 57.4 → 87.8%（455/2）。輸入任何注音之前：63.5%。例：下午3 → 點、價格300 → 元 | 沒有 | 與 D1 一起**採用** |
| DD3 | 不可逆的按鍵也套用 θ | digits.md A.9 | 非字母表的注音鍵會立即決定切分且無法還原，因此也套用 θ。 | 之前：dev302 第 196 列變成 比賽用嗎1咬計時（301/302），因為畫面上的 context 用嗎 還沒確定 | 不適用 | **採用**這個教訓：任何不可逆的決定都需要保守偏差 |
| DD4 | Enter 與 ↓ 依畫面為準 | digits.md A.12 | 緩衝區完全依顯示內容切分：顯示為數字者成為 token，顯示為注音的尾段維持待處理。句首 `2⏎` 送出 ㄉ；`2026⏎` 送出 2026。 | 修正了顯示的 ㄅ 被送出為「1」的退步 | 不適用 | **採用**「送出畫面顯示的內容」作為硬性規則 |
| DD5 | 模擬器必須依畫面修復 | digits.md A.10 | 英文與數字模擬器的 Backspace 修復依據引擎的組字內容，而不是模擬器自己猜的欄位。 | 每 1000 個音節殘留的數字：1.13（cvtune）、1.59（typing76）→ 修正後 0。產品代價：ㄅ／ㄉ／ㄓ 音節第 2 鍵打錯時要多按一次 Backspace（302 列開發集中 11 次） | 部分有（`typing_errors.py` 含時序） | **採用**：ZT 的錯字模擬器應依畫面顯示內容修復 |
| DD6 | 中英混打 v1：影子與覆寫證據 | zhen-mixed.md；research-log 2026-10-14 | 原始鍵帽保留為「影子」。按 Space 時，若影子在 82,730 詞清單中（SymSpell 截斷 3000 + 全部）且覆寫次數 ≥1（有殘留音節時 ≥2），開啟英文視窗，列出 小寫／首字大寫／全大寫，加上一個中文項目。 | 需求：22.8% 的訊息含 ≥3 個字母的 token，每 100 則訊息 79.5 個；token 之後：空白 36%、中文 28%。召回率 99.48%。p=0.02 時每 1000 個音節的誤觸發：在 cvtune、wikitune、dev302、typing76 上，各種清單大小都是 **0.000** | 沒有（路線圖） | **採用**：覆寫證據是幾乎零誤判的英文偵測器。ZT KeyDecoder 的覆寫／修復邏輯不能先把它吃掉 |
| DD7 | 行內英文：打字時顯示字母、自動保留、Shift 連打 | zhen-inline.md §1 | 當影子有 ≥3 個字母、有覆寫證據且影子是某個詞的前綴時，顯示字母。當下一鍵無法延伸時，自動保留會把完整的詞變成英文 token。Shift+字母開始一段可編輯的英文連打（Space 結束；第二個 Space 插入空白單位）。Backspace 逐字母刪除。粗底線加上「空白：英文斷開」提示。英文 token 留在組字中。 | 閘門：p=0.02 時顯示誤觸發 ≤2/1000，自動保留 ≤0.5/1000（覆寫）。維護者實機回饋六輪（`rusr` 案例、逐字母刪除） | 有（極簡）（ZT：Shift+字母 = 單一大寫字面字元；沒有英文連打） | **調整後採用**：Shift 連打 + 逐字母 Backspace + 英文 token 留在組字中，是低成本的第一步 |
| DD8 | 按鍵順序證據（vChewing 的想法） | zhen-keyorder.md；research-log 2026-10-26 | 不依欄位順序的輸入（例如 `ls`：先韻母後聲母）算作英文證據。19 詞的 `TWO_LETTER` 清單（ok ls cd mv ip …）。窮舉測試乾淨地輸入每個詞庫讀音，要求零觸發。獨立開關，預設關閉。 | 24 格中開／關輸出相同（在模擬器上沒有可量測的成本或效益）。`two` 召回 19/19 | 沒有 | **調整後採用**：不論 ZT 用哪種偵測器，都沿用「乾淨輸入每個讀音 → 零英文觸發」的窮舉測試想法 |
| DD9 | 中英文間距（盤古之白） | cjk-spacing.md | 只在外殼處理。在送出文字中，於漢字（第一個 scalar，可含變體選擇符）與 ASCII [A-Za-z0-9] 之間插入 U+0020，另依前一個字 p 決定是否加前導空白。p 依 session 保存，在組字開始與傳遞按鍵時清除。結尾從不加。 | 3 位使用者提出要求。已知限制：游標文字不可靠的 App（VS Code、Chrome） | 沒有 | **採用**（小）：一個設定開關加上 `ControllerCore.swift` 中的純函式 |
| DD10 | 學習鍵中的數字與英文鄰居 | learn.rs `next_key`；zhen-inline §1.0 | 英文 token 與空白單位正規化為 `^>_punct_en`，數字為 `^>_punct_num`，因此學習鍵從不儲存英文或數字文字本身。 | 隱私：R2 標記測試 | 不適用 | 若 ZT 讓個人學習以非漢字鄰居為條件則**採用**：儲存類別，不儲存文字 |

## 8. 標點

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| DU1 | 標點放在組字內，作為邊界 | s3d-punctuation.md | 標點是 1 格的 token（`_punct_X`），對應固定的詞。對 LM 而言是句子邊界：左段以 `</s>` 結束，右段以 `<s>` 開始，與建立計數的方式一致。 | 測試對：他長，在換（若把「，」當成轉移，結果會變成 他常／再換） | 有（ZT 字面 token；「詞從不跨越字面 token」） | **已涵蓋**。檢查 ZT 的左段以 `</s>` 結束，而不是轉移到字面 token |
| DU2 | 由系統表取得標點替代選項 | s3e-punctuation-candidates.md | 執行時讀取 Apple 的 `/System/Library/PrivateFrameworks/CoreChineseEngine.framework/.../CIMPunctuationCandidates.plist`（不再散布），以 McBopomofo 的 MIT 表作為備援。選擇後保留原讀音，因此重新開啟時顯示相同清單。支援多字元替代（⋯⋯）。 | CI runner 映像檔（xcode-27-arm64）上有此 plist | 部分有（ZT `literalList`） | **調整後採用**：執行時讀取相同的系統 plist 以完全對齊 Apple，ZT 的表作為備援 |
| DU3 | 標點名稱（Apple 風格） | s3f-punctuation-names.md | 讀取 `CIMPunctuationDescription_zh_Hant.strings`（141 筆，最長 8 字：全形西式左單引號）。 | `IMKCandidates` 會忽略 attributed 字型（名稱顯示為 0.73 倍過大），也無法展開成網格，這是善解自建面板的原因 | 沒有（ZT 有自己的 CandidatePanel，不受此限制） | **採用**：在 `CandidatePanel.swift` 中以小字的次要文字顯示名稱 |
| DU4 | 標點與其選字從不教學 | s4-learning §1.2；repick-freeze §11 | 以 `_punct_` 開頭的讀音不學習。選擇標點替代或英文拼寫仍會凍結外側的詞（Apple 行為）。 | — | 不明 | **採用**（很簡單） |
| DU5 | ↓ 清單中的全形數字變體 | digits.md A.3.4 | 數字 token 之後，↓ 清單 = 數字 context 列，接著是半形與全形（０–９ ．／－），再來是另一個注音讀音。 | — | 沒有 | 與 D1 一起採用 |

## 9. UX、候選字視窗、設定與組字行為

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| E1 | 自建的玻璃候選面板 | `docs/contracts/s3b2-glass-panel.md` §0、§2.1；`macos/Sources/ShanjieKit/CandidateCells.swift` | 以無邊框、不啟用的 `NSPanel` 搭配 `NSGlassEffectView` 取代 `IMKCandidates`：level popUpMenu+1，`canBecomeKey/Main=false`，`hidesOnDeactivate=false`。改用的原因：IMKCandidates 會忽略 attributed string 字型、沒有網格，而私有的 `IMKCandidate` 會畫出空框。 | — | 有（`UI/CandidatePanel.swift` `InputPanel` + `NSGlassEffectView`） | 我們已經有 |
| E2 | 與 Apple 注音逐像素對齊的尺寸 | s3b2 §1、§3；`ShanjieKit/CandidateCells.swift`、`PanelPlacement.swift` | 以 `tools/imeshot.swift` 在 1x（1 px = 1 pt）下擷取 Apple 的畫面。每個常數都附註解，註明量測所依據的截圖。 | 膠囊高 24 pt、寬 37 pt（單字）；格距 41 pt；列 30 pt；行下方間隔 6 pt（4 pt 會高 2 pt）。2 輪之後全部與 Apple 相差在 1 pt 以內。 | 部分（有 `UIPreviewRenderer`，沒有以 Apple 為基準的量測） | 調整後採用：以 imeshot 式的工具（#53）把我們的面板與 Apple 比對量測，並在每個常數註明出處 |
| E3 | 展開 = 更多 9 格寬的列，欄只加寬 | s3b2 §9；`ShanjieKit/GridLayout.swift` | 按 ↓ 展開成每列一頁 9 項；第一列等於收合時的頁面。欄位只會加寬（純函式），因此捲動時網格永遠不會左右位移。使用者拒絕了 Apple 的 6 欄網格，因為第一列會重新排列。 | — | 沒有（沒有展開網格） | 若加入展開檢視則調整後採用：遵守「第一列 = 收合頁」與只加寬的規則 |
| E4 | 格子重用與繪製效能測試作為發布閘門 | s3b2 §9.1；`CandidateCellTests/testBenchmark` | 每格一個 view，直接繪製 attributed string（不用 `NSTextField`）。捲動時重用格子，只建立新進入的列。效能測試只在 `SHANJIE_BENCH=1` 時執行，且發布前必須在負載 < 8 時通過。 | 捲動一列：新版 p95 0.57 ms，舊版 11.6 ms（快 7–49 倍）。展開 45 格：p95 2.67 ms。目標：≤ 2 ms 與 ≤ 5 ms。 | 不明（沒有面板效能測試） | 採用：選擇性執行、有 p95 預算的面板繪製效能測試 |
| E5 | 面板跟隨客戶端 App 的淺色／深色 | s3b2 §10 | 只在 IMK 客戶端物件的 `viewEffectiveAppearance`（退回 `windowEffectiveAppearance`）型別編碼為「無參數、回傳物件」時才呼叫，並在每次顯示時套用。這修正了深色系統下淺色 App 中面板周圍的黑邊。 | 在 4 種淺色／深色組合下與 Apple 的面板一致 | 部分（`viewDidChangeEffectiveAppearance` 跟隨系統，而非客戶端） | 採用（小，有防護的準私有呼叫） |
| E6 | 以純函式決定位置 | s3b2 §2.3；`ShanjieKit/PanelPlacement.swift` | 面板放在行下方 6 pt，空間不足時翻到上方，並限制在 `visibleFrame` 內。螢幕取包含該行矩形的那一個。沒有矩形時重用上一次的行矩形（不是上一次的原點）；若它在螢幕外，則用主螢幕左下角。`sideHeight` 讓高度可變的面板維持在同一側。 | 以 2 個螢幕的案例做單元測試 | 有（`InputPanelPlacement`），但備援是寫死的 `1440×900` 矩形 | 調整後採用：把寫死的備援換成上一次的行矩形，再退回 `NSScreen.main` |
| E7 | Marked text 內的行矩形 | s3b2 §2.2 | 在 marked text 內以索引呼叫 `attributes(forCharacterIndex:lineHeightRectangle:)`，原點為 (0,0) 時往回找（McBopomofo 的方法）。 | — | 有（多來源 `caretRect`，以滑鼠位置備援） | 已涵蓋 |
| E8 | 具 Apple 按鍵語意的直式視窗 | `candidate-vertical.md` §1–2；`ShanjieKit/VerticalLayout.swift` | 在 Apple 的直式視窗上量測：Space = 下一項（不是下一頁），在第 9 列按 ↓ 捲動一列並重新編號 1–9，PageUp/Down 換頁。只有在直式視窗開啟且沒有按修飾鍵時，PageUp/Down 才會送到核心。 | 列距 28 pt；膠囊寬約 226 pt | 有（直式／橫式排列偏好設定） | 調整後採用：對照這些量測檢查我們直式模式的 Space／捲動語意 |
| E9 | 格子的輔助使用 | s3b2 §2.6 | 每格有 `accessibilityLabel`（候選字加上其名稱）與 `isAccessibilitySelected`。 | 未驗證 VoiceOver | 部分（只有 `accessibilityPerformPress`） | 採用 |
| E10 | 可即時預覽的玻璃色調滑桿 | `settings-window.md` §2.3；`ShanjieKit/GlassTint.swift` | 0–1 滑桿以 數值×0.5 的不透明度疊上黑色（深色）或白色（淺色）。0 表示不指定，也就是量測到的 Apple 外觀。預覽列重用真正的格子程式碼與同一個 `Applier`，它也負責清除回 nil。 | 0.5 上限是偏好值（Syrtis） | 沒有（只有字級） | 調整後採用（選用的潤飾） |
| E11 | 以弱參照保存組字擁有者 | s3b §5；`ShanjieKit/Shell.swift` | 一個引擎。擁有者 controller 以弱參照保存。非擁有者的 `deactivateServer`／`commitComposition` 不動任何東西。`deinit` 發現擁有者為 nil 時，捨棄組字。點擊會變成送進核心的數字鍵。 | 測試會對擁有者檢查做 mutation | 有（`private static weak var activeController`） | 我們已經有 |
| E12 | 確實會跳到前景的設定視窗 | `settings-window.md` §0、§2.1；`Shanjie/SettingsWindow.swift` | 自 macOS 14 起，`NSApp.activate()` 只是請求；在實機上視窗開在 layer 0、位於前景 App 後方。修正：`level=.floating` + `orderFrontRegardless()`（vChewing）。關閉時焦點回到記住的 App，但只在我們處於啟用狀態時。視窗開在滑鼠所在螢幕的中央。SwiftUI 的 `Settings` scene 無法從輸入法選單開啟（macSKK 的備註）。 | 調查：8 套開源輸入法中有 6 套在同一行程內開啟設定 | 部分（`NSApp.activate(ignoringOtherApps:)`，已棄用） | 採用 `.floating` + `orderFrontRegardless` + 焦點返回 |
| E13 | 高度以螢幕為上限的側欄設定 | `settings-sidebar.md` §2 | 每頁的 Form 在螢幕外量測（+4 pt 餘裕），視窗高度 = min(最高頁, `visibleFrame`)，在 `didChangeScreenParametersNotification` 時重新計算。側欄是固定寬度的 `List`（不是 `NavigationSplitView`，它的側欄可能收合後無法復原）。 | 修正的錯誤：v0.4.3 的視窗在筆電上延伸到 Dock 下方 | 有（7 個窗格） | 調整後採用：在 13 吋螢幕上檢查我們的高度上限 |
| E14 | 選單與視窗共用一個儲存 | `settings-window.md` §2.2；`ShanjieKit/Settings.swift` | 選單與視窗呼叫相同的 `Shell` setter；行程內通知會重新讀取值。Mutation test 破壞儲存／通知時必須失敗。 | — | 有（`Preferences.didChange`） | 我們已經有 |
| E16 | 「關於」頁面，模型清單綁定雜湊 | `about-update.md` §1；`about-models.md`；`ShanjieKit/ModelList.swift`；`scripts/check-models.sh` | `data/models.tsv`（檔案、SHA-256、中文名稱、公開名稱）是內附模型的唯一清單。建置時只複製這些檔案，`check-app.sh` 雙向檢查 bundle↔tsv，「關於」頁面顯示這些列。版本 `0.0.0` 顯示為「開發版」。 | 列出 6 個模型 | 部分（AboutPane；`manifest.json`） | 採用：雙向的模型 manifest 檢查加上「關於」頁面的列 |
| E17 | 「不備份學習資料」開關 | `ShanjieKit/Learning.swift` | 在學習資料目錄（不是檔案，因為清除時檔案會被刪除）上設定 `isExcludedFromBackup`。清除學習資料有確認與失敗對話框。 | — | 沒有 | 採用（低成本的隱私選項） |
| E18 | 單一輸入模式，鍵盤配置在選單中選擇 | s3b §13 | 一個 TIS 模式 `<ID>.zhuyin`；鍵盤配置是儲存在 UserDefaults 的選單選項。不覆寫 `setValue`，否則系統每次啟用都會重設配置。升級時停用舊模式。 | 2 種模式的版本會在輸入來源清單中同時顯示兩者 | 有（單一模式） | 加入倚天／許氏（ROADMAP P5）時採用：不增加 TIS 模式 |
| DE1 | 重選凍結其他詞（Apple 行為） | repick-freeze.md §1、§9、§11、§15 | 選字之後，所有其他顯示中的詞都被凍結。部分重疊的詞，其外側的字各自凍結為單字。新音節可以與凍結的鄰居合併。在詞內按 Backspace 會保留其餘的字。Backspace 留下的片段在輸入下一個音節時釋放。Cmd+Backspace 只在被遺忘的記錄正在生效時才釋放。所有規則都以 `imeshot` 在 Apple 注音上量測（他是 → 她是記者；他世界 ⌫ → 他世；他世 + ㄏㄣˇ → 他是很）。 | 使用者回報的錯誤：板控 → 選 版 → 版空。繼續打字模擬，整句正確：dev302 chat main 458 → rev4 558（不重新凍結 590），formal 465 → 578（605） | **沒有**（ZT `selectCandidate` 只固定所選範圍；其他部分重新解碼） | 在 `engine/swift/Sources/ZhuyinCore/Composer.swift`（`selectCandidate`、`fix`、插入／Backspace 路徑）**採用**。採用第 5 版的規則，不用第 4 版（合併後重新凍結會退步：她世紀者、找到達案） |
| DE2 | 預測選字也會凍結 | repick-freeze §13 | 從預測列選字會凍結其範圍外的詞。 | ㄧˇ ㄅㄟˋ + ㄕ，選 時：已被時 → 以備時 | 沒有 | 與 P5 一起採用 |
| DE3 | Enter 送出顯示的內容 | enter-pending.md | 有待處理音節時，Enter 送出整個顯示的字串（待處理符號留在原位，包括游標在中間的情況）。Shift+Enter 送出後再傳遞按鍵（在聊天中換行）。失去焦點時的 `reset(commit)` 也包含待處理符號。待處理符號從不學習。 | 量測 Apple：ㄅ⏎ → ㄅ；你好ㄇ⏎ → 你好ㄇ；你ㄅ好嗎 | **有**（ZT 自我測試「未完成的尾段在 Enter、快捷鍵與失去焦點後保留」） | **已涵蓋**。加上游標在中間與「待處理符號從不學習」的斷言 |
| DE4 | 組字上限 128 + 前段凍結 | composition-cap.md；engine.rs `CAP_*` | 上限由 40 改為 128 個單位（滿了就送出；Apple 停在 101）。長於 W = ctx−3 = 61 字的自由段，其前段逐詞凍結，直到剩 W−H（H=16）。距編輯點 M=4 個音節內的詞不凍結（保護正在輸入的 電腦）。凍結的前段從不重新解碼。 | 重新解碼整段組字每個音節花 0.28 ms：128 時 p95 26.0（重排開）與 28.5（關）ms。加上前段凍結：**13.4／13.3 ms**；RerankJob p95 39.9 ms。最差的單一按鍵 24.5 ms | 有（做法不同）（ZT `maxSyllables=60` 拒絕新符號；v3 context 255，v2b ctx 64） | **調整後採用**：量測 ZT 在 60 個音節時的每鍵 p95。若呈線性成長，在重排視窗（64）之外加入前段凍結並保留 16 字緩衝，再提高上限 |

## 10. 隱私、安全輸入與沙盒

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| E19 | 安全輸入只會讓 `~/Library` 中的輸入法變灰 | `system-install.md` §0–1；research-log 2026-10-20 | 只要有任何行程持有安全輸入，macOS 就會讓安裝在 `~/Library/Input Methods` 的第三方輸入法變灰；相同的建置放在 `/Library/Input Methods`（root:wheel）則能繼續運作。已排除子模式是原因。 | VM 26.6.2 + Mac 27.0.1；移動並登出後 Caps Lock 正常 | 有（pkg 安裝到 `/Library/Input Methods`） | 維持 `/Library`；提醒開發者 `install-dev.sh` 裝到 `~/Library` 會變灰 |
| E20 | 安全輸入下停用時捨棄（不送出） | s3b §5；`ShanjieKit/Shell.swift:1126` | 在 `deactivateServer` 中，若 `IsSecureEventInputEnabled()` 為真則以捨棄方式重設，讓組字內容永遠不會落入剛取得焦點的密碼欄位。只在這裡檢查，因為此旗標是全系統的（到處檢查會默默丟掉文字）。 | 以注入的 closure 測試 | 沒有（`deactivateServer` → `endSession`，沒有檢查） | 採用 |
| DL11 | 隱私閘門，失敗時關閉 | s4-learning §3、§10 | 在 `IsSecureEventInputEnabled()`、密碼管理工具拒絕清單（1Password `com.1password.1password`、`com.agilebits.onepassword7`、Bitwarden、KeePassXC、`com.apple.Passwords`、`com.apple.keychainaccess`），或 bundle ID 不明時暫停學習。引擎啟動時學習為**關閉**，外殼在每次呼叫前重新取樣。暫停期間做的選字之後也不會學習。選單顯示固定字串。 | 安全審查：修正 9 個 P2 問題。終端機仍會學習（維護者的決定） | **沒有**（ZT 路線圖：「隱私／安全輸入欄位」） | 在 `platforms/macos/Sources/ZhuyinTypeApp/IMK/ControllerCore.swift`／`ZhuyinInputController.swift` **採用**：每次 `learn` 前檢查 `IsSecureEventInputEnabled()` 與 bundle ID 拒絕清單 |
| DL12 | 強化 context 讀取 | s4-learning §2；context-return.md §1 | 只在組字開始時讀取一次，組字中途從不讀取。請求 ≤256 個 UTF-16 單位（原本 32），截斷過大的回應，捨棄被切斷的第一個字素，在最後一個換行處切開（`Character.isNewline`，NUL 視為換行），保留 64 個字素。終端機類 App（8 個 bundle ID）只傳入漢字。 | 在實機上量測 `markedRange`：VS Code、Chrome 與 Finder 搜尋回傳 `{ins,0}`；有一個 App 回傳 `{NSNotFound,NSNotFound}`。把長度 0 視為「沒有 marked text」修正了這些 App 的 context 遺失 | 部分有（ZT 讀取 128 字 + 同 App 備援） | 在 `ZhuyinInputController.swift` **採用** markedRange 長度規則、截斷與終端機只取漢字的規則 |
| E23 | 記錄隱私規則 R2，有測試 | s3b §9、§10.5、§11 | 只用 `os.Logger` 搭配靜態字串與回傳碼：不用 `.public`，不用 `print`／`NSLog`。持有輸入的型別既不是 `Error` 也不是 `CustomStringConvertible`。測試以 PID 過濾執行 `log stream --style ndjson`，加上開始／結束 nonce 標記，並同時擷取 stdout／stderr，因為行程有 stderr 時 `NSLog` 會略過統一記錄。Mutation（`.public`、`.private`、`NSLog`）必須讓它失敗。 | 2026-10-03 量測到 NSLog 只寫到 stderr | 違反（`ZhuyinInputController.swift:63` 以 `.public` 記錄 bundle ID + 矩形；按鍵記錄器） | 採用：移除 `.public` 的輸入／bundle 記錄；加入記錄測試 |
| E24 | 輸入法使用 App Sandbox 並遷移資料 | `app-sandbox.md` §0–2.9；`scripts/check-entitlements.sh`、`test-sandbox-migration.sh`、`test-sandbox-mutations.sh`；`docs/sandbox-restore.md` | 恰好 3 個 entitlement：`app-sandbox`、`network.client`、mach-register `<ID>_Connection`（必須等於 `InputMethodConnectionName`）。`container-migration.plist` 搬移學習資料目錄；只有出貨 ID 才附上。CI 在一次性的 runner 上證明「沒有清單 ⇒ 不搬移」。有降版用的還原文件。他們明白指出沙盒無法阻止按鍵外洩。 | 24 份 mutation 副本都必須失敗；實機上 0 次沙盒拒絕，只有一次允許的 `-spks` 查詢 | 沒有 | 暫不採用：成本高（需要處理 CoreML、模型目錄、更新 helper）；公證之後再重新評估 |
| E25 | 開發、自我測試與出貨的 bundle ID 分開 | s3b §2.1；app-sandbox §2.3；`Makefile`；`scripts/build-app.sh` | 預設的本機建置使用 `.dev`，有自己的顯示名稱；自我測試使用 `.selftest-$RUN_ID`。`build/` 中的 `.metadata_never_index` 讓 LaunchServices 不會啟動建置副本。`make clean-bundle` 執行 `lsregister -u`。出貨 ID 從不在本機執行。 | — | 部分（`install-dev.sh`，單一 ID） | 採用：`.dev` bundle ID，並在 `dist/` 放 `.metadata_never_index` |
| E26 | 嚴格的 argv 與內附資源自我測試 | s3b §9；`ShanjieKit/Selftest.swift`；`Shanjie/main.swift` | 只接受完全相符的 `install` 或 `--selftest`；其他一律以非零結束且沒有副作用。自我測試透過 `Resources/` 輸入 dev302 第 284 列，並要求 chat／formal 輸出不同、出現詞庫包詞、重排有改變、英文視窗與口語句子。不用 TIS，不寫入。 | CI：把 `bigram.sjlm` 改名必須讓它失敗 | 有（`ControllerCoreSelfTest.swift`），argv 較寬鬆（7 個旗標） | 調整後採用：在 CI 中從建置好的 `.app` 執行自我測試；拒絕未知參數 |

## 11. 安裝程式、更新程式、發布與 CI

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| E27 | 替換 bundle 後絕不重新註冊／啟用（Caps Lock 錯誤） | `installer-v2.md` §9；`ShanjieInstall/Registration.swift`、`InstallCommand.swift` | Bundle 被替換後呼叫 `TISRegisterInputSource`／`TISEnableInputSource` 會破壞 Caps Lock 切換輸入來源。對已知但未被接受的輸入法呼叫 enable 會回傳 noErr 但什麼都不做。修正：`decide(modeListed, accepted, legacyEnabled)` 只在系統不認識該模式時註冊，否則什麼都不改；接著輪詢 accepted（CLI 5 秒、App 30 秒），否則引導使用者到系統設定。 | 每種情況量測一次；需要慢速按鍵修正；VM 確認 enable 無作用 | **有風險**：`--install` 一律註冊 + 啟用，而 pkg 的 `postinstall` 每次升級都會執行它 | 採用（首要行動） |
| E28 | 讀取 TIS 狀態，而不是 `defaults` | s3b §15.6；installer-v2 §9 | 輸入法已啟用時，`defaults read com.apple.HIToolbox AppleEnabledInputSources` 回傳 0 列。只信任 `TISCreateInputSourceList`。行程自己的 TIS 清單可能過期（安裝程式讀到「未接受」長達 2 分鐘，另一個行程在 20 秒內就看到「已啟用」），因此用 `--accepted-check` 子行程。 | — | 有（`--status` 使用 TIS；postinstall 執行新的行程） | 若從 App 內部輪詢，採用子行程重新檢查 |
| E29 | 「到系統設定中加入」深層連結 | installer-v2 §9.2(3) | `NSWorkspace.open(x-apple.systempreferences:com.apple.Keyboard-Settings.extension)`，加上 3 個簡單步驟（+ → 繁體中文 → 名稱 → 加入）。登出的建議因為錯誤而移除。 | — | 部分（只有 postinstall／InstallCommand 中的文字） | 在 pkg 結尾頁／首次執行 UI 採用此深層連結 |
| E30 | 有「試打」步驟的 GUI 安裝程式 | `s3c-installer.md`；installer-v2 §1；`ShanjieInstaller/main.swift`、`ShanjieInstall/InstallerPlan.swift`、`InstallerFlow.swift` | 4 個步驟（歡迎、安裝、啟用、試打）。計畫（安裝／更新／只啟用／不降版）與結果→畫面的對應是經過測試的純函式。`--render-steps` 輸出 16 張淺色／深色 PNG 供審閱。「切換到善解」之後會以 `TISCopyCurrentKeyboardInputSource` 檢查。內附的輸入法是 zip（Resources 中展開的 `.app` 會被 LaunchServices 註冊）。 | — | 有 pkg（zh_TW 歡迎／結尾頁） | 不採用 GUI App；採用「把安裝／引導畫面輸出成 PNG」與不降版規則 |
| E31 | 以固定的 root 命令做全系統安裝 | `system-install.md` §2.3；`ShanjieInstall/RootStage.swift` | `NSAppleScript … with administrator privileges` 執行一個固定命令。Root 階段：ditto 到 root 擁有的暫存目錄、chown／chmod、以 Developer ID CA + leaf + team 要求執行 `codesign --verify --strict`、略過符號連結的 ACL 清除、版本 ≥ 已安裝版本，然後以原子 `mv` 換上並保留 `.previous`。 | 驗證者發現 `chmod -RN` 會跟隨植入的符號連結，讓 root 清除外部的 ACL | 不適用（我們的 pkg 做 root 安裝） | 不採用；若之後在 pkg 之外做暫存，套用符號連結／ACL 的教訓 |
| E32 | 位置改變 ⇒ 強制註冊 + 一次登出 | system-install §1（P1）、§2.4 | 在 `~/Library` 與 `/Library` 之間搬移後，系統仍指向舊路徑，直到註冊 + 登出。相同位置的更新略過註冊。 | VM：登出後正常；學習資料雜湊不變 | 不適用（一律 `/Library`） | 備註：給從 `~/Library` 開發安裝遷移出來的使用者 |
| E33 | 具暫存、`.previous` 與 stub 測試的安裝腳本 | s3b §4、§13.3；`scripts/install-ime.sh`、`test-install-ime.sh` | mktemp 暫存 + ditto → 把舊的改名為 `.previous`，之後才 `lsregister -u` → 把新的移到定位 → `lsregister -f` → 結束舊行程 → 必要時註冊。trap 會還原狀態。只處理檔案的測試模式以 stub 取代 lsregister／pkill／pgrep，並斷言它們從未被呼叫。 | 14 個案例 | 部分（`install-dev.sh`、`reload.sh`、`uninstall.sh`，沒有測試） | 調整後採用：為我們的腳本加入以 stub 執行、只處理檔案的測試 |
| E34 | 升級時真的會重新啟動的 Homebrew cask | s3b §14–15；`packaging/Casks/shanjie.rb`；`scripts/render-cask.sh`、`test-render-cask.sh` | 當 cask 從 `INSTALL_RECEIPT.json` 讀回時（是字串而非 Symbol），`uninstall on_upgrade: :signal` 從不觸發。修正：`postflight_steps terminate_process '^[^ ]*/Library/Input Methods/…/MacOS/shanjie$', match: :full`（沒有 `:full` 時 brew 會用 `killall`）。測試透過 brew 的載入器載入 cask，並比對 6 個 stub 行程；CI 中執行 `brew style`。另有 `auto_updates true` 與 `zap` 清單。 | 通知後 0.01–0.03 秒舊 PID 消失；6–20 秒後出現新行程 | 沒有（沒有 cask） | 透過 brew 發布時採用（若用 `pkg` stanza，保留結束行程的步驟） |
| E35 | 寫成文件的 Sparkle 信任模型 | `about-update.md` §0.1；`release.yml` 檔頭 | zip 不帶程式碼簽章，因此實際上只有 EdDSA 決定是否接受更新，之後 Gatekeeper／公證不會再檢查。EdDSA 金鑰 + `appcast.xml` 寫入權限 = 能在每個安裝中執行程式碼。緩解措施：`SURequireSignedFeed`、`SUVerifyUpdateBeforeExtraction`、`SUAllowsAutomaticUpdates=false`，金鑰只在 3 個簽署步驟中經由 stdin 使用。輪替時必須以舊金鑰簽署一個帶有新公鑰的建置。 | — | 進行中（`Package.swift` 固定 Sparkle 2.10.0，`Sources/` 中尚未接上） | 採用相同的金鑰設定，以及寫成文件的信任模型 + 輪替操作手冊 |
| E36 | 每天靜默探測，打字時從不跳出視窗 | `about-update.md` §1–2；`Shanjie/Updater.swift`；`ShanjieKit/Updates.swift` | `SUEnableAutomaticChecks=false`（並從 defaults 中移除相關鍵）。自己的計時器每天最多執行一次 `checkForUpdateInformation`；結果只顯示在「關於」頁面與選單。「更新到 X」會先靜默重新檢查。`startingUpdater:false` 再 `start()`，因為自動啟動失敗會顯示 `runModal` 警告，讓輸入法卡住。`ShanjieUpdateBuild=release/test/local` 讓本機建置永遠不讀取正式的 feed。 | 實機測試 (a)–(h) 通過 | 沒有（只有 FSEvents 自我替換） | 全部採用 |
| E37 | 固定 feed 並清除略過版本 | `standalone-updater.md` §1.2 S2、§2.1 | Sparkle 會先從宿主的 user defaults 讀 `SUFeedURL`，再讀 Info.plist，因此 feed 可以被覆寫。Delegate `feedURLString(for:)` 回傳 Info.plist 的值。每次探測前刪除 `SUSkippedVersion*` 鍵（重放的舊 feed 或極大的略過版本會凍結更新）。 | — | 沒有 | 採用 |
| E38 | Sparkle 替換保留 Caps Lock 與執行中的客戶端 | research-log 2026-10-16 (a)–(d) | Sparkle 不呼叫任何 TIS。更新後：Caps Lock 正常；TextEdit、Safari、Discord、Orca 不需重新啟動就能繼續打字。 | 實機執行 1 次 | 部分（我們的自我替換：同樣是替換檔案，但 pkg 路徑會重新註冊，見 #27） | 備註：任何更新路徑都不要呼叫 TIS |
| E39 | 「此路徑沒有 XPC」會被記住直到重新開機 | app-sandbox §1（2026-10-19 VM） | 若某個 framework 路徑曾在沒有 XPC 服務的情況下執行，之後 launchd 在重新開機前都不會在同一路徑註冊 `Installer.xpc`（出現「connecting to the installer」錯誤）。他們把 framework 移到 `Contents/Frameworks/Updater/`，之後完全不用 XPC。 | 4 步驟乾淨重現 | 目前不適用 | 採用為規則：不要在版本之間於同一 framework 路徑改變 Sparkle 的 XPC 組成 |
| E40 | 沙盒化的輸入法 + 不受沙盒限制的更新 helper | `standalone-updater.md`；`ShanjieUpdater/main.swift`、`UpdateHost/UpdateHost.swift` | `Contents/Helpers/善解更新.app` 只更新包含它的 bundle，且只在該 bundle 的 realpath 是 2 個允許的 Input Methods 路徑之一時才更新。它會檢查宿主的 Developer ID 要求，不接受 argv／env／IPC，只接受 `application` 更新類型。輸入法在啟動 helper 前會關閉自己的 `.floating` 設定視窗（helper 原本被擋在它後面）。 | VM 上 V1–V5；`/Library` 更新會詢問一次管理員權限 | 不適用（沒有沙盒） | 除非採用 #24，否則不採用 |
| E41 | release.yml：閘門 → 建置 → 簽署 → 發布 | `.github/workflows/release.yml`；`scripts/check-ci-gate.sh` | **閘門**：v* tag 或 main dispatch；先檢查發布說明檔（1 秒內失敗）；該 commit 的 `ci.yml` 必須為綠燈（會跳過只改 appcast 的 commit）。**建置**：不用 secret；ad-hoc 建置 + 未簽署的安裝程式骨架；`tar` 保留權限模式。**簽署**：`environment: release`，不執行儲存庫中的程式碼；一次性的 keychain（隨機密碼、`-x` 不可匯出、依 team 選身分）；由內而外 `--options runtime --timestamp`，從不用 `--deep`；entitlements 來自字面值；notarytool 的判定從 JSON 讀取（被拒時它可能以 0 結束）；staple；staple 後 `chmod -R a+rX,go-w`；從解開的 zip 驗證（`spctl` = "Notarized Developer ID"、`stapler validate`、entitlements 完全相符、權限）。**Sparkle**：以 CryptoKit 推導的公鑰對 zip 檢查金鑰對；從以 SHA-256 固定的 tarball 執行 `generate_appcast`，參數 `--download-url-prefix --embed-release-notes --maximum-versions 0`。**發布**：以比對 `.sha256` 達成可重跑的冪等性；只有最新版才標為 Latest（`sort -V`）；appcast 透過 contents API 以 blob SHA 寫入（競爭時 409）；以 deploy key 更新 tap，從不 `--force`。Dispatch 試跑會留下保存 1 天的已簽署產物供 VM 測試。 | 2 次公證的逾時設為 150 分鐘 | 沒有（ad-hoc、沒有公證、沒有 workflow） | 採用精簡版（簽署 + 公證 + 驗證 + appcast） |
| E42 | 儲存庫中手寫的發布說明 | `release-notes.md`；`docs/releases/<tag>.md` | 以 `--notes-file` 取代 `--generate-notes`；沒有非空檔案的 tag 會被閘門拒絕；說明以 markdown 內嵌到 appcast 項目中。 | — | 沒有 | 採用 |
| E44 | 不會騙過發布閘門的「只改文件就略過」 | `ci.yml` `changes` job；`scripts/ci-changes.sh` | 只改文件、網站或 `eval/bench/results/` 時略過測試。輸入為空時照常執行。在 main 上，只有前一個 main commit 的執行成功時才允許略過。Concurrency 會取消被取代的 PR 執行。 | 在此之前發布閘門要等約 11 分鐘 | 沒有（沒有 CI） | 採用 |
| E45 | 把 job 分散到不同 runner；只保留量測上確實較快的做法 | `ci-speed.md`、`ci-speed-2.md`；`ci.yml` | macOS：Rust release 測試；Swift 測試（`--parallel`，數量取自 xUnit）；shell（bundle／檢查）。Linux `ubuntu-24.04`（固定映像檔）：debug + Python。Rust 快取被移除，因為沒有省下時間（workspace crate 反正都會重建）。工具鏈固定 1.97.1 以維持位元一致的 golden；action 以 SHA 固定。取捨：`--parallel` 抓不到測試間的狀態外洩，因此序列執行的 `make test-swift` 留在本機。 | 822 s → 目標約 8.5 分鐘；macOS 3 核心、7 GiB；cli 測試在 Linux 上慢 2.4 倍（沒有 Accelerate） | 沒有 | 採用（起步：在 `macos-*` 上跑 `swift build` + 測試 + 自我測試 + 打包檢查） |
| E46 | CI 中的每鍵延遲斷言 | `ci.yml` core job 註解 | Release 模式的重播斷言每鍵 p95 < 16 ms。 | 效能測試：p95 約 2.0 ms，載入 905 ms，RSS 441 MB（v0.5.0，閒置） | 部分（有 `--timing`，沒有斷言） | 採用：在 CLI 重播上加入 p95 預算測試 |
| E47 | 打包檢查的 mutation test | `scripts/test-sandbox-mutations.sh`；s3b §10 | 出貨 bundle 的 24 份修改副本（entitlements、`.xpc`、Info.plist 鍵、多餘的 helper、chmod 700、g+w…）都必須讓 `check-app.sh`／`check-entitlements.sh` 失敗。契約中的每項檢查都有「mutation 必須失敗」條款。 | 24 個 mutation | 部分（`check-packaging.swift`，沒有 mutation） | 調整後採用：在暫存目錄中修改我們的 bundle／pkg，並要求 `check-packaging.swift` 失敗 |

## 12. 評測工具、統計與 KPI

| ID | 簡稱 | 來源檔案 | 做法／發現 | 量測結果 | ZhuyinType 狀態 | 結論 |
|---|---|---|---|---|---|---|
| E48 | 在不含文字的 rowstats 上做配對統計 | `eval-stats.md`；`tools/evalstats.py` | `rowstats` = `row\tok\terrors\tgold_len`（只有數字，因此 holdout 輸出不含文字）。`compare` 回報：精確雙尾 McNemar、CER、Δtop1 與 ΔCER 的配對 bootstrap 95% CI（seed 20261008，10k 次重抽，輸出逐 byte 相同）。`--net` 加上 fixed−broken、改壞率與 broken 的 Garwood Poisson 上界。`--cluster` 依訊息／條目重抽。`mde` 給出 D 個不一致列下可偵測的最小 net。 | 227 句中 ±1 句是雜訊：需要約 6:0 或 9:1 的不一致列才會 p < 0.05 | 沒有（說明：沒有顯著性檢定） | **採用（首要行動）** |
| C34 | 以配對統計訂定出貨標準 | `it-pack.md` §3.4；`tools/evalstats.py` | 四個條件：聊天集沒有顯著變差（精確 McNemar p ≥0.05 或 net ≥0），且 broken ≤26 或 broken/fixed ≤0.6；退步集相同或 broken ≤2 且 net ≥0；Δtop1 的 90% 配對 bootstrap 下界 ≥ −1.0 個百分點；錯字集 net >0 且 broken/fixed ≤0.6。`evalstats.py` 也提供 MDE 與群集 bootstrap。 | — | 沒有（說明：沒有顯著性檢定） | 在 ZT rowstats 上採用 `evalstats.py` 式的比較（評測部分可能也已涵蓋）。 |
| B13 | 以自己的訊息建立聊天評測集（E2） | e2-chat-eval.md；experiments/e2-chateval/build_chatclause.py | Discord／Threads／FB 匯出；子句 2–30 個漢字；context = 前一個子句或同頻道 ≤10 分鐘內的訊息；PII／token／URL／私訊篩選；調參月份與評審月份以雜湊凍結；排除以善解輸入的月份 | 評審 6,559 列，調參 5,761 列；檢定力計算約需 1,720 列，選 2,500；修正率 0.34–0.67% | 沒有（我們有合成口語 2000 句開發集；「真實聊天語料」在路線圖上） | **採用**：以這些篩選條件，從使用者在使用 ZhuyinType 之前的訊息建立私人聊天開發／評審集 |
| A12 | 轉換方式改變時的「原生」評測列 | s2f §3.4、s2w §4.3、cv_native.py | 當某項變更改變了文字轉換時，只在原始文字經新舊轉換都不變的列上評判（cvtune 3,756 → 2,117 列）。 | 把有偏的 ±20 列分歧轉為可用的閘門（#11）。 | 沒有 | 任何語料轉換變更（#7、#8）都**採用**。我們的 wiki 開發集本身就是 s2twp 的輸出。 |
| A48 | Holdout 輪替與計數層級的外洩防護 | holdout-refresh.md | 三份互不重疊的 300 列分片（150 新聞、150 公開談話）。使用 8 次後，或在因它而做出任何決定後輪替。使用記錄放在雜湊旁。若某段經轉換、異體字合併後的漢字串*包含*某句 holdout 句子，就捨棄該語料列。分片提供雜訊底線。 | 舊的 227 列 holdout 至少已用過 7 次。n 227 時檢定需要 6:0 或 9:1 才顯著。 | 部分（測試集只跑一次；pipeline 中有 holdout 排除） | **採用**：在我們的語料防護中加入使用計數、分片輪替，以及轉換後的包含比對。 |
| E54 | Holdout 污染防護 | `tools/holdout_guard.py`；holdout-refresh §8 | 在計數層（轉換與異體字合併之後）以包含比對檢查保留句子是否出現在語料中。只輸出數量與行號。計數腳本接受 `--holdout-guard`。 | — | 沒有 | 對我們的 Qwen 合成／Common Voice／Tongues 建置相對於測試集採用 |
| B16 | D2c 錯誤拆分 | rerank-strength §11.5；d2c_split.py | 把列分類：D2a（差距 ≥ τ）、D2b（LM 對但 < δ）、D2c-(i) LM 單獨對但融合後落敗、D2c-(ii) LM 錯 | (i)/D2c = 0.275 →「融合與模型都是候選改進方向」 | 部分（error_analysis.py 依打字習慣分類） | 在 ml/src/zhuyintype_ml/eval/error_analysis.py **採用**：可判斷該調融合還是重新訓練 |
| E53 | 錯誤分桶分類，以盲標驗證 | `triage.md`（契約 + 文件）；`tools/triage.py` | 每個錯誤列歸入一個桶：A1 詞缺讀音；B1/B2 正確答案不在 top-64；C 排序（C1 單字同音、C2 詞輸給同音詞、C3 斷詞、C4 多處錯誤、C5 其他）；D1 重排弄壞；D2a/b/c 重排沒救回；另有 E1/E2 context 旗標。修正方向依 桶大小 × 可救回比例 ÷ 成本 排序。在 60 列隨機盲標上驗證：主桶一致 ≥ 90%，C 子類一致 ≥ 80%。 | — | 部分（`ml/src/zhuyintype_ml/eval/error_analysis.py` 依打字習慣分類） | 調整後採用：加入 oracle 名次的分桶（在 top-64 內嗎？是哪一層翻轉的？） |
| DE5 | 真實引擎重播與 KySS | engine-replay.md；research-log 2026-10-18 | 透過一個長時間存在的引擎（與 App 相同）做按鍵層級重播。可選擇模擬重選（游標移動 + Space + 換頁 + 數字）。回報首次顯示 top1、final_ok、每 100 字選字次數、KySS = Σ理想／Σ實際，以及重排的 `rewrites`。`--learn on` 讓學習跨列延續。 | dev302 chat：首次顯示 262/302，KySS **0.9739**，每 100 字選字 2.07 次。錯誤回報：39/135，KySS 0.8808。重排時機逐鍵對最終：送出相同，但「顯示後又改變」為 110 對 17（dev302 chat） | 部分有（`zhuyin-cli` 批次 + `typing_errors.py`；「免選字率」在路線圖上） | **採用**：KySS 與每 100 字選字次數**就是**免選字率指標。在 `engine/swift/Sources/zhuyin-cli/main.swift` 加入 `--correct` 與 `--learn` |
| E49 | 逐版本 KPI 表 | `kpi.md`（契約 + 文件）；`tools/kpi.py` | 每個發布版本一列，資料來自凍結評測集的 JSON：discordtune chat top1、錯誤比（broken ÷ fixed，相對前一版）、user-reported 重播 top1（學習關閉）與學習開啟時的 broken、KySS、每 100 列 rewrites、holdout 談話／新聞兩半、wikitune-clean formal、lytune（只警告），速度只在負載 ≤ 2.0 時記錄（否則為「—」）。列檔案雜湊改變時拆分欄位。 | v0.5.0：discordtune 87.90%；錯誤比 0.39（34/88）；重播 41.25%；KySS 0.9773 dev302／0.8947 回報集；每 100 列 62.3 次 rewrites；holdout 76.67%／80.00%；wikitune-clean 75.29%（v0.4.0：84.89%／70.38%） | 沒有 | 採用：以相同形式套用到我們的口語／wiki／Tongues 開發集 |
| E51 | 有設定標籤防護的跨版本效能測試 | `sbench.md`、`sbench-v2.md`；`tools/bench.py` | 對每個 ref 執行 `git archive` 並建置它自己的 CLI。評測組以寫死的 SHA-256 凍結。每個版本以它出貨時的設定執行（`VERSIONS2`），CLI 摘要的設定標籤必須等於表中的標籤。速度：同一 session、3 輪、取 p95 中位數，並記錄平均負載；重播必須等於 CLI 302/302。以 `sandbox-exec` deny 執行（任何存取都會以 137 結束），證明沒有碰到私人資料。 | 決定性：同一版本跑兩次得到逐 byte 相同的 JSON | 部分（我們每次變更都評測，沒有跨版本表） | 採用凍結評測組 + 設定標籤防護 |
| E52 | 發布閘門 = 只看使用者面向的評測集 | sbench-v2 §2.3 rev. 2026-10-15；e2-chat-eval §0 | 只有在 holdout、user-reported 或 discordtune 出現 fixed < broken 且 p < 0.05（任一 profile）時才擋下 tag。其他評測集（lytune、wikitune、cvtune、dev302）只警告並做歸因切片。以 1:1 net 計分並設改壞率上限 ≤ 0.5%，取代「1 個改壞 ≈ 10 個改對」。Holdout 每次發布只跑一次，由新的驗證者執行，只輸出數字。 | lytune（立法院語域）多次與聊天反向移動 | 部分（測試集只跑一次） | 把此規則採用到 `docs/ml/EVALUATION.md` |
| E55 | 真實輸入法比較工具 | `tools/imetype.swift`、`tools/imeshot.swift`、`docs/typing-test.md` | `imetype` 以 `CGEvent.postToPid`（只對自己的 pid）把各列輸入到自己的視窗，對 N 套輸入法執行，驗證每次切換並讀回結果。`imeshot` 擷取每個步驟的畫面，並錄製帶按鍵顯示的示範。發現的注意事項：Apple 注音會在測試中學習（2,647 個學到的片語中有 847 個來自測試輸出），因此要在測試前備份 `DynamicPhraseLexicon_zh_Hant_zhuyin.db`、之後還原。 | 第 1 輪（23 個子句）：Apple 錯 3、McBopomofo 10、自然 8–10、早期的善解 9；人工與自動一致 21–23/23 | 沒有 | 採用（與其他輸入法的比較表 + 以 Apple 為基準的 UI 量測） |
| E56 | 小型探測工具：安全輸入與 TIS | `tools/secure-input.swift`、`tools/tis.swift` | 持有安全事件輸入 N 秒以重現變灰；註冊／啟用／停用／顯示輸入來源。 | 用於 #19、#27 | 部分（`InstallCommand --status`） | 採用 `secure-input.swift` 來測試 #20／#21 |
| DE7 | 以 mutation 測試的契約 | 每份契約的 §驗收 | 每條規則都附有一個具名的 mutation，必須讓某個測試失敗（因斷言失敗，而非建置錯誤）。等價的 mutation 有文件說明。 | 例如 predict-down：6 個 mutation；repick-freeze：每版 3 個 | 部分有（ZT 自我測試） | **調整後採用**：ZT 引擎規則（凍結、數字、Enter）在 `ZhuyinCoreChecks/main.swift` 中每條規則加一個 mutation |
| DE8 | 引擎重播 == CLI 一致性 | engine-replay.md §6.1 | 學習關閉且沒有選字時，逐鍵重播必須等於一次性的 CLI 解碼（`top1_sha256`）。 | 6/6 格相同（dev302、typing76、錯誤回報 × chat／formal） | 有（ZT CLI = 同一個引擎） | ZT 有了搭配非同步重排的逐鍵重播後，**採用** sha 檢查 |

## 參考 A：每個 `tools/*` 檔案

| 檔案 | 用途 | 與我們相關？ |
|---|---|---|
| `bench.py` | 跨版本效能測試：匯出每個 git ref，建置其 CLI + 按鍵重播，對凍結評測組計分，寫出 JSON 與 `docs/benchmark.md`。 | 是（逐版本退步表） |
| `bench/replay/src/main.rs` | 按鍵重播程式：把每列以標準鍵盤配置按鍵 + Enter 輸入，記錄送出內容與每鍵 p95 延遲。 | 是（ZT 有 `--timing`；逐版本重播的想法） |
| `build_acg_pack.py` | ACG 詞庫包建置工具：wiki 擷取／快取、名稱擷取、讀音、分數、碰撞偵測 (a)(b)(c)、重新檢查、`freeze-counts`、`regrade`。 | 是（詞庫包建置工具的範本） |
| `build_attribution.py` | CC BY 口語語料池的逐單位出處 TSV（不含文字）。 | 是（授權） |
| `build_classes.py` | 寫出綁定 LM sha 的 `classes.sjc`（512 個詞類、類別 bigram、μ）。 | 低 |
| `build_digits.py` | 以 wiki + 口語資料建立數字 context 計數表 `digits.tsv`，排除 holdout。 | 低／中（數字處理） |
| `build_english_next.py` | 建立英文詞之後接的中文詞表（混合輸入預測）。 | 中（中英混打落地時） |
| `build_english_words.py` | 從 SymSpell 建立固定版本的英文詞表。 | 中 |
| `build_it_pack.py` | IT+AI 詞庫包建置工具（國家教育研究院、CGroup、AI 分類、人工清單；中國用語篩選；頻率下限；已打得對篩選）。 | 是 |
| `build_lm.py` | 從加權計數建立二進位詞 bigram `.sjlm`（格式寫在檔頭）。 | 低（ZT 有自己的 LM 格式） |
| `build_meme_pack.py` | 迷因詞庫包建置工具：三個階段（輸出候選、輸出 X、以讓位規則建置）。 | 是（兩輪讓位模式） |
| `build_names.py` | 人名層建置工具（NSI、wiki、教育部學校）+ 用於創作者／店家的 `--append-creators`；`--check` 交集測試。 | 是 |
| `build_overlay.py` | 從 Wikimedia 標題 dump + 人工 + 流行語列建立主 overlay；教育部讀音；篩選（簡體、日文、粵語、異體字）。 | 是（篩選、流行語尾段） |
| `build_sandhi.py` | 產生 一／不／法／紮 的異讀列與輕聲詞的本調列。 | 低（我們已有） |
| `build_trigram.py` | 把 trigram 表重新對應到另一個 bigram 模型的 id；以 sha 綁定。 | 低 |
| `build_variants.py` | 從教育部重編國語辭典建立異體字對照表，只用於評測時的寬鬆比對。 | 中（臺／台 類異體字的評測寬鬆比對） |
| `convert_oc.py` | 把外部的 ONNX 字元模型轉成 SJOC 二進位檔。 | 低 |
| `evalstats.py` | 在 rowstats 上計算配對 McNemar、bootstrap CI、net／改壞率、群集 bootstrap、MDE。 | 是（高） |
| `gen_kn_tiny.py` | KN 玩具模型的跨語言 golden（Python 對 Rust）。 | 低 |
| `gen_oc_tiny.py` | 0c 模型的跨語言 golden。 | 低 |
| `gen_pack_oov.py` | pack-OOV 解碼的跨語言 golden。 | 低（只參考模式） |
| `gen_tinylm_tiny.py` | 小型字元 LM 的跨語言 golden。 | 低 |
| `gen_tri_tiny.py` | Trigram 表的跨語言 golden。 | 低 |
| `holdout_guard.py` | 以包含比對檢查保留句子是否出現在計數文字中。 | 是 |
| `imeshot.swift` | 實機截圖／錄影探測：透過輸入法在自己的視窗中輸入按鍵，擷取候選字視窗。 | 是（示範與 UI 退步檢查） |
| `imetype.swift` | 打字測試自動化：透過多套輸入法把評測列輸入到自己的視窗，讀回輸出。 | 是（與 Apple 注音等比較） |
| `it_sources.py` | IT 詞庫包來源層：擷取、快取，與計數共用的候選切分。 | 是（搭配 build_it_pack） |
| `it_wiki_counts.py` | 在全部 zhwiki 條目上計算候選字串次數（檔頭固定 dump／候選／commit）。 | 是 |
| `kn_cont.py` | 計算綁定模型 sha 的 KN continuation 計數附屬檔。 | 低 |
| `kpi.py` | 逐版本 KPI 表（建置舊版本，重播各列）。 | 中 |
| `oc_ref.py` | SJOC 模型的讀寫工具 + numpy 前向計算。 | 低 |
| `readings.py` | 讀音產生層級，含 CHECK／UNREADABLE 報告。 | 是 |
| `reading_overrides.tsv` | 25 個人工確認的讀音（那邊、什麼、唯 ㄨㄟˊ、麗 ㄌㄧˋ、虎 ㄏㄨˇ…），由 `readings.py` 最先套用。 | 是 |
| `secure-input.swift` | 持有安全事件輸入 N 秒，以重現密碼欄位的行為。 | 是（ZT 路線圖：安全輸入欄位） |
| `tis.swift` | TIS 輔助工具：註冊／啟用／停用／顯示輸入來源。 | 是（安裝測試） |
| `triage.py` | 把出貨設定下的每個錯誤列歸入一個錯誤桶；私人評測集只輸出數量。 | 是（類似 ZT 的 error_analysis.py） |
| `testdata/names/*` | 給 `test_build_names.py` 用的精簡 NSI JSON 與教育部 CSV fixture。 | 是（fixture 模式） |
| `test_bench.py`、`test_build_acg_pack.py`、`test_build_attribution.py`、`test_build_classes.py`、`test_build_digits.py`、`test_build_english_next.py`、`test_build_english_words.py`、`test_build_it_pack.py`、`test_build_lm.py`、`test_build_meme_pack.py`、`test_build_names.py`、`test_build_overlay.py`、`test_build_sandhi.py`、`test_build_trigram.py`、`test_evalstats.py`、`test_extra_overlay.py`、`test_holdout_guard.py`、`test_it_sources.py`、`test_it_wiki_counts.py`、`test_kn_cont.py`、`test_kpi.py`、`test_oc_ref.py`、`test_pack_oov.py`、`test_readings.py`、`test_triage.py` | 對應工具的離線單元測試（fixture、假 API、假解碼器）；其中幾個斷言已提交的輸出等於重新產生的輸出，並包含 mutation 案例。 | 是，作為模式參考（資料工具有單元測試，不連網） |

## 參考 B：`scripts/*.sh`，每個一行

- `build-app.sh`：把 Rust 核心 + Swift 外殼建置成輸入法 bundle，包含資料、模型（對照 `models.tsv`／sha256 檢查）、授權、Sparkle 與更新 helper；以 3 個沙盒 entitlement 做 ad-hoc 簽署。用於本機與 CI；從不碰 keychain。
- `build-installer.sh`：建置 `安裝善解輸入法.app`（執行檔、plist、lproj、內附的 `install-ime.sh`、授權、圖示）。有提供輸入法 zip 時做 ad-hoc 簽署，否則產生供發布簽署用的未簽署骨架。
- `check-app.sh`：對建置好的 App 執行 s3b 驗收 2/3/6（plist、資源、entitlements、Sparkle 結構／鍵、權限、`--selftest`、拒收的 argv、TIS 清單不變）。拒絕在 CI 之外執行出貨 ID 的建置。
- `check-ci-gate.sh`：發布閘門；等待並要求該 commit 在 main 上的 `ci.yml` push 執行為綠燈，略過只改 appcast 的 commit。
- `check-entitlements.sh`：輸入法的 entitlements 必須恰好等於那 3 個鍵，mach 名稱由其 bundle ID 推導。
- `check-installer.sh`：安裝程式 bundle 結構、內附的 zip 與腳本逐 byte 相同、Resources 中沒有 `.app`、圖示、沒有 entitlements、hardened runtime。
- `check-models.sh`：`data/models.tsv` 對 `data/*.sha256` 與 `data/lm/` 檔案（建置模式），以及 bundle Resources ↔ tsv 雙向檢查（bundle 模式）。
- `ci-changes.sh`：從 stdin 讀取變更的路徑，只有在每個路徑都是文件、網站或效能測試結果時才輸出 `run=false`。
- `compile-app-icon.sh`：以 `actool` 把 `packaging/AppIcon.icon` 編譯成 `Assets.car` + `AppIcon.icns`，再檢查兩者都存在（名稱不符時 actool 仍以 0 結束）。
- `deploy-site.sh`：以 keychain 中的 token 把 `site/` 發布到 Cloudflare Pages。
- `fetch-models.sh`：CI 中唯一一份「下載模型 release 資產 + `shasum -c`」。
- `install-ime.sh`：使用者帳號安裝，含暫存、`.previous`、lsregister 順序、結束舊行程、有條件註冊；有只處理檔案與略過註冊兩種模式。
- `installer-version.sh`：輸出內嵌在安裝程式執行檔 `__TEXT,__shanjie_ver` 區段中的版本。
- `render-cask.sh`：把版本 + sha256 填入 cask 範本，並嚴格驗證。
- `test-check-ci-gate.sh`：以 stub 的 `gh` 測試 CI 閘門（只改 appcast 的 commit 使用其上層 commit 的執行結果）。
- `test-check-models.sh`：在假資料夾上測試 `check-models.sh`。
- `test-install-ime.sh`：在暫存的 HOME 中以 stub App 測試 `install-ime.sh` 的檔案處理；斷言 lsregister／pkill／pgrep 從未被呼叫。
- `test-render-cask.sh`：測試 `render-cask.sh` 的錯誤處理、brew 解析後 cask 的 `postflight terminate_process`，以及其樣式對 6 個 stub 行程的比對。
- `test-sandbox-migration.sh`：只在 CI 執行；出貨建置會把學習資料與偏好設定一次搬進 container，沒有清單的建置則什麼都不搬。
- `test-sandbox-mutations.sh`：出貨建置的 24 份 mutation 副本，必須如預期地讓 entitlement 與 App 檢查失敗（或通過）。

（另有 `make-app-icon.swift`：以 SHA 固定的 Noto Serif CJK TC Black 重新產生圖示 SVG。`make-icon.swift`：以 CoreText 繪製選單列樣板 TIFF。）


## 參考 C：合併的重複項目

| 移除的列 | 保留為 |
|---|---|
| A27（融合 δ 閘門與「1 個改壞 = 10 個改對」目標） | B2、B11 |
| A34（部分注音 + context 預測研究（SP）） | DP1 |
| A41（常駐人名層（+2 log10）） | C20 |
| B14（配對統計工具） | E48 |
| B32（context-return 到核心（SJTL0005）） | DP12 |
| B42（N0／N1 早期 LLM 解碼） | A50、A51 |
| B48（讀音通道項 P(reading\|char)（P5）） | A29 |
| B51（重播工具與閃動指標） | DE5 |
| C27（變調與輕聲異讀列） | A13 |
| C30（公開 LM 中的指紋條目） | A49 |
| C35（從逐字重選學習新詞） | DL14 |
| C36（Holdout 污染防護） | E54 |
| C38（backoff 用的詞類） | A17 |
| C39（混合輸入用的英文詞表） | DD6 |
| DE6（在最終分數上降權非標準讀音） | C25 |
| DL15（學習保護對神經重排） | B8 |
| E15（逐 App 的 chat／formal profile） | B49 |
| E21（學習與 context 的 privacyGate） | DL11 |
| E22（終端機 context 規則與 context 截斷） | DL12 |
| E43（模型產物以獨立 Release 發布並追蹤雜湊） | C29 |
| E50（模擬使用者重播：KySS 與 rewrites） | DE5 |

## 13. 善解回報的負面結果與陷阱（彙整）

### 資料與詞庫
- **LM 訓練使用硬性斷詞，會讓切開的路徑沒有任何 bigram。** 詞庫中有 後到 時，c(後,到)=0，因此「十分鐘後道」勝出。任何平滑方法的改變都無法修正從未進入計數表的次數（A6）。
- **對已經是繁體的文字執行 s2t 或 TWPhrases 會造成損壞。** 后／里／干 變成 裡長／鄰裡／皇後。床 變成 牀。全面套用 TWVariants 讓 排泄 歸零（422→0）。插件 變成 外掛。每一輪修正都需要完整重建（A7、A8）。
- **用其中一種轉換產生的參考答案來評估轉換方式的變更**，會偏好該轉換：wikitune −9 對 wikitune-mw +17。只在兩種轉換都不會改變的列上評判（A11、A12）。
- **沒有 LM 證據且條目分數平坦的詞幾乎總是落敗。** 2 字詞庫包名稱落敗 98.7%，3 字 38.9%。它們輸給的是一串常見字（宿儺 → 素娜），而不是另一個對應詞（C10）。
- **把每個詞庫包詞都標為 LM 未知**，弄壞 22 列、修正 25 列。只有審核過的讀音才應略過 LM（C11）。
- **以語料頻率限制詞庫先驗，會埋沒罕見人名。** 把未見懲罰提高到 2.0，3 字人名的落敗率幾乎加倍（A39）。
- **以擴大後的詞彙重新斷詞語料，會搶走常見字對的計數**（萊+恩 1,336→0），並導致顯著的 wikitune 否決（A40）。
- **以較新的解碼器重建詞庫包，默默捨棄了已出貨的名稱。** 把詞表與分數分開，並對照前一版詞庫包重新檢查已出貨的詞（C9、C13）。
- **大量降權「非教育部」讀音**，會影響 5,817 個大多是合法變調或輕聲讀音的條目（C25）。
- **以規則標註的讀音計數**標註錯誤率 40.8%，judge5 net −1,457（A32）。
- **懲罰以一聲輸入的語助詞**，讓 lytune 損失 −213 到 −508，因為使用者會把 啦 打成 ㄌㄚ（A31）。
- **以 softmax 正規化讀音機率**，會讓每個有 k 個平手讀音的詞被扣 log10(k)（A29）。

### 模型與融合
- **沒有閘門的重排模型會弄壞太多列。** 沒有 δ 時，出貨的 α/τ 設定在評審期弄壞 142 列；δ 0.25 時弄壞 16 列（B2）。
- **在調參集上達到的改壞率上限無法轉移。** 調參時 0.46–0.50% 的格，在全新資料上落在 0.54–1.72%。以更嚴的上限（0.3%）選擇，並以 Poisson UB 評判（B12、B17、B24）。
- **「1 個改壞 = 10 個改對」的計分方式**丟掉了真正的增益（dev302 276→249）。1:1 net 加上改壞率上限效果較好（B11）。
- **在相同資料上用更大的模型對聊天沒有幫助。** L 30M（我們 v2b 的形狀）在真實聊天上輸給 M 14M，相差 −97。XL 只對 Wikipedia 有幫助（每參數 token 數由 17 降到 4）（B3、B18）。
- **把強重排模型（0c）蒸餾到因果字元 LM 的五種方式全部失敗**：候選 KL、加上讀音、加大容量、位置層級 KL，以及重新加權（B25–B27）。
- **以 n-gram 差距與名次為特徵的 listwise 目標**退化為「保留第 1 名」（選對率 19%）（B28）。
- **逐位置把讀音餵給因果 LM**，計算的是錯誤的量 Σ log P(c_i | c_<i, R_≤i)，並損失 71 列。讀音必須一開始就完整提供，如我們的 v3（B29）。
- **訓練與評測的 context 長度不一致**（訓練長、評測短）損失 39 列（B31）。
- **在融合前套用口語證據會「搶走錨點」**：64 個改壞，例如 一戰→一站、致病→治病。以 top-1 為錨點，並以神經模型閘控此項（B45、B46）。
- **LLM 清單選擇式評審有位置偏差。** Apple FM 有 72% 的時間選第 1 個，順序反轉會讓 36–54% 的選擇翻轉。作者自己撰寫的開發集指向錯誤方向（dev302 +27 對真實聊天 −81）（B36、B40）。
- **沒有相同資料、換 seed 的對照組就認定新語料有效**並不安全：光是重新訓練就讓 net 變動 32 列（B52）。

### 學習
- **不分 context 的學習加分會污染常用字。** 在兩個 context 中各選一次 再，使 54 列 ㄗㄞˋ 中有 24 列退步（現在在做 → 現再再作），因此關閉了全域層（DL4）。
- **在弱 context 下學習單字**產生了 我試買新的 與 你再。善解現在只在完全相符的 2 漢字 context 下學習單字，句首鍵則帶上下一個音節（DL5、DL6）。
- **每次送出都學習會學到片段**：317 句正確句子中有 80 句學到雜訊（DL1、DL14）。
- **以 LM 為先驗的貝氏後驗從一次選字學到的太少**：鏡像測試通過 0–3/8，固定加分則為 7/8（DL8）。
- **平滑方法或 trigram 的變更會默默縮小個人加分。** 在 KN 下鏡像測試由 7/8 降到 4/8（A22）。
- **每次 Enter 都以 F_FULLFSYNC 重寫整個檔案**，p95 花費 16–20 ms（DL10）。

### 產品、外殼與發布
- **替換 bundle 後重新註冊或重新啟用輸入來源，會破壞 Caps Lock 切換。** 啟用系統尚未接受的輸入法會回傳 noErr 但什麼都不做（E27）。
- **過度積極的凍結**（合併後重新凍結）讓繼續打字退步。先量測 Apple，再完全對齊（DE1）。
- **沒有保守偏差的不可逆按鍵決定**：比賽用嗎1咬計時，以及顯示的 ㄅ 被送出為「1」。送出的內容必須與顯示的完全相同（DD3、DD4）。
- **只用後繼詞的預測**在「第一個音節完成」時損失約 20 個百分點。學到的預測加分無法跨層，因此改為重新排序（DP2、DP7）。
- **縮寫組字**出貨後又被隱藏：按鍵卡住，且 13–16% 的詞無法打出（DP9）。
- **每個按鍵都重新解碼整段組字**，每個音節約花 0.28 ms。在重排視窗之外凍結前段（DE4）。
- **Sparkle 的陷阱：** `startingUpdater:true` 可能顯示讓輸入法卡住的強制回應警告。`SUFeedURL` 會先從 user defaults 讀取。略過版本的鍵可能凍結更新。launchd 會記住「此路徑沒有 XPC」直到重新開機（E36、E37、E39）。
- **不可信的結束碼：** notarytool 在送審被拒時以 0 結束；actool 在圖示名稱不符時以 0 結束。簽署 job 的 umask 077 讓 `/Library` 中的副本無法讀取（E41）。
- **LSUIElement App 的視窗會開在前景 App 後方**，比螢幕還高的設定視窗會延伸到 Dock 下方（E12、E13）。
- **有 stderr 時 `NSLog`／`print` 會略過統一記錄**，因此記錄隱私測試也必須擷取 fd 1 與 2（E23）。
- **與 Apple 注音比較測試時沒有還原它的動態詞庫**，會灌高 Apple 的分數：2,647 個學到的片語中有 847 個來自測試本身（E55）。
- **在本機執行出貨 ID 的沙盒建置**，會把真實的學習資料不可逆地遷移進 container（E25）。
- **自訂的絕對閘門**（數字「≥90%」，差 3 列未達）會擋下好的設計。優先使用配對的相對閘門（DD1）。

## 14. ZhuyinType 的前 15 項行動（依預期影響排序）

| 排名 | 行動 | 原因（善解的證據） | 要閱讀的善解檔案 | ZhuyinType 中的位置 |
|---|---|---|---|---|
| 1 | **每項評測都做配對統計並設發布閘門。** 從 `zhuyin-cli`／`conversion_eval.py` 輸出只含數字的 rowstats。加入精確 McNemar、Δtop1／ΔCER 的配對 bootstrap CI、fixed／broken／net、改壞率的 Poisson UB、群集 bootstrap 與 MDE。只有在使用者面向的評測集出現 fixed < broken 且 p < 0.05 時才擋下發布。以使用計數輪替測試分片。 | 善解自 S2 以來的每個決定都以此為基礎。227 列上 ±1 句的變化是雜訊（需要 6:0 或 9:1）。我們的文件只回報落在 ±400 列雜訊範圍內的準確率差異。 | `tools/evalstats.py`、`tools/test_evalstats.py`、`docs/contracts/eval-stats.md`、`docs/contracts/sbench-v2.md` §2.3、`docs/contracts/holdout-refresh.md`、`tools/holdout_guard.py` | `ml/src/zhuyintype_ml/eval/evalstats.py`（新增）、`ml/src/zhuyintype_ml/eval/conversion_eval.py`、`ml/src/zhuyintype_ml/remote/`、`docs/ml/EVALUATION.md` |
| 2 | **私人的真實聊天開發／評審集（加上 0c 量尺）。** 從使用者在使用 ZhuyinType 之前的訊息建立 2–30 個漢字的子句，context = 前一個子句或 10 分鐘內的訊息，並做 PII／URL／token 篩選。切成調參月份與一次性評審月份，以 SHA 凍結。接著在我們的 top-16 上以 0c（MIT 權重）計分，與 v2b／v3 並列。 | 我們的聊天開發集是合成資料或朗讀語音。他們的真實聊天集推翻了幾個由公開評測集得出的結論（dev302 +27 對聊天 −81；lytune 與聊天反向移動）。在善解的量測中，0c 在聊天上比它出貨的重排模型多 +105 net。它是唯一公開、以讀音為條件的重排模型，因此是 v3 有用的外部參考。 | `docs/contracts/e2-chat-eval.md`、`experiments/e2-chateval/build_chatclause.py`、`judge.py`、`d2c_split.py`；`docs/contracts/x0c-compare.md`、`experiments/x0c/score_0c.py`、`tools/convert_oc.py`（檔頭記錄了 ONNX 圖） | `ml/src/zhuyintype_ml/eval/`（新的聊天集建置工具）、`ml/src/zhuyintype_ml/remote/`（透過 ONNX Runtime 的 0c 計分器） |
| 3 | **與 Apple 注音相同的重選凍結。** 選字之後，所有其他顯示中的詞都被凍結；部分重疊的詞，其外側的字各自凍結為單字。新音節可以與凍結的鄰居合併而不重新凍結。Backspace 保留其餘片段，直到下一個音節。預測選字也會凍結。 | 使用者回報的錯誤 板控 → 版空。繼續打字的整句正確數：chat 458 → 558（rev 4 → 590），formal 465 → 578。我們的 `selectCandidate` 只固定所選範圍，其他部分都重新解碼。 | `docs/contracts/repick-freeze.md`（採用第 5 版的規則，§9、§11、§13、§15）、`core/src/engine.rs` 凍結程式碼、`tools/imeshot.swift`（量測 Apple 的方法） | `engine/swift/Sources/ZhuyinCore/Composer.swift`（`selectCandidate`、`fix`、插入／Backspace）、`ZhuyinCoreChecks/main.swift` 案例（他是 → 她是記者、他世 + ㄏㄣˇ → 他是很、以備時） |
| 4 | **以 τ／δ 閘控 v2b 融合並保護學到的詞。** 當 engine top1 − top2 ≥ τ 時略過重排。只有在融合增益 ≥ δ 時才切換。對經由回想或個人 n-gram 選定的區段使用逐候選否決（0c 式）。在聊天集上以 1:1 net、調參時改壞率上限 ≤ 0.3% 調參。 | 光是 δ 就讓評審期的改壞由 142 降到 16，net +109。目前 v2b 只是單純的 `engine + 1.0·logP`，只有 +4.0 的按鍵完全相符差距，沒有任何機制防止它覆寫學習結果。 | `core/src/tinylm.rs`（`pick`）、`docs/contracts/s7c2c-delta.md`、`s7h-tinylm-v3.md`、`s7c2a-core-rerank.md` §2.2、`engine-replay.md` §2（`learn_guard`）、`experiments/e2-chateval/judge.py --select --cap` | `engine/swift/Sources/ZhuyinML/NeuralReranker.swift`、`RerankerTree.swift`、`ml/src/zhuyintype_ml/training/reranker/` 參數掃描 |
| 5 | **把口語項作為閘控的重排特徵。** 保留 lattice 中的 c 混合，但在重排模型內加入 `g·(μ/α)·(lc_chat(cand) − lc_chat(top1))`，其中 `g = σ((logP_i − logP_0)/T)`，以 top-1 句子為錨點。可選擇加入相對於最佳讀音的單字讀音通道項（截斷於 −4）。 | 這是善解唯一在全新聊天評審集上確認有效的模型端變更：net +69（108/39），p 1.1e-8，CER 2.85 → 2.61%。讀音通道項之後在 discordtune 上再增加 23/0。 | `docs/contracts/colloquial-term.md`、`core/src/colloquial.rs`、`experiments/p4d-colloquial/README.md`、`experiments/p5-reading-channel/README.md`、`docs/contracts/reading-channel.md` | `NeuralReranker.swift` + `engine/swift/Sources/ZhuyinCore/NGramLanguageModel.swift`（來自 `ml/src/zhuyintype_ml/training/lm_chat2/` 的聊天 bigram） |
| 6 | **個人學習的退步測試框架，再收緊單字與不分 context 的證據。** 加入 `--teach/--learn` 重播模式與三項測試：鏡像測試；ㄗㄞˋ 污染測試（在 2 個 context 下教 再，再重播 現在在做、我現在在家 與每一列 ㄗㄞˋ 開發集）；以及每次送出都學習的反事實實驗。若未通過，要求單字證據需有 2 字／trigram context，並讓句首學習以下一個音節為條件。 | 他們不分 context 的層讓 24/54 列退步。「每次送出都學習」在 80/317 句正確句子中學到雜訊。我們的個人 unigram 與長期層不分 context，而且每次送出都學習。 | `docs/contracts/s4-learning.md` §6、§12–14、`learn-single-ctx.md`（規則 7–8）、`s4-bayes.md`、`core/src/learn.rs`（`context_key`、`next_key`）、`eval/learn/cases.tsv`（經由契約） | `engine/swift/Sources/zhuyin-cli/main.swift`、`ZhuyinCore/PersonalLanguageModel.swift`、`Engine.swift learn(commit:)` |
| 7 | **以期望計數訓練 LM。** 把最後一輪 Viterbi hard-EM 換成在每段漢字詞 lattice 上的 forward-backward 後驗（剪除 < 0.01），再做 MKN。加入領域詞時，保留基礎計數，只加入新詞的計數。 | 詞庫內詞的切開路徑得到 c = 0（c(後,到) 0 → 206，c(好,吧) 0 → 9）。cvtune chat 26/11，p 0.02；使用者回報的句子已修正。成本：每 200k 篇文章在 10 個 worker 上約 8 分鐘。 | `docs/contracts/s2n-simplified-residue.md` §6（model E）、`experiments/s2/build_counts.py`、`docs/contracts/model-v5.md` §8（加法計數） | `ml/src/zhuyintype_ml/training/lm/pipeline.py`（`segment`、`em`）、`ml/src/zhuyintype_ml/training/lm_chat2/` |
| 8 | **安全輸入與隱私閘門。** 在 `IsSecureEventInputEnabled()` 下於 `deactivateServer` 捨棄組字。在安全輸入下、遇到密碼管理工具的 bundle ID，以及 bundle ID 不明時（失敗時關閉），暫停學習與 context 讀取。終端機只傳入漢字尾段。把長度為 0 的 `markedRange` 視為沒有 marked text，並在最後一個換行處切開 context。移除 `.public` 的 bundle ID 記錄。加入 log stream 隱私測試。 | 這是 ROADMAP P5 的缺口。目前 `ZhuyinInputController.swift:63` 以 `.public` 記錄 bundle ID 與矩形，且沒有任何地方呼叫 `IsSecureEventInputEnabled`。善解的版本通過了安全審查（修正 9 個問題）。 | `macos/Sources/ShanjieKit/Learning.swift`（`learningPause`、`learningDenylist`、`LeftContext`）、`ShanjieKit/Shell.swift`（deactivate）、`docs/contracts/s3b.md` §5、§9–11、`s4-learning.md` §2–3、`context-return.md`、`tools/secure-input.swift` | `platforms/macos/Sources/ZhuyinTypeApp/IMK/ZhuyinInputController.swift`、`ControllerCore.swift`、`EngineHost.swift` |
| 9 | **升級時不再重新註冊輸入來源（Caps Lock 錯誤）。** `--install` 應讀取 TIS 狀態，只在模式未知時呼叫 register／enable；否則不做任何改變並輪詢是否被接受。加入系統設定的深層連結。讓所有更新路徑都不呼叫 TIS。 | 善解追查到 Caps Lock 切換失效正是這個原因。我們的 `InstallCommand.swift` 每次執行都呼叫 `TISRegisterInputSource` + `TISEnableInputSource`，而 pkg 的 postinstall 每次升級都會執行它。修正約需 1 小時。 | `docs/contracts/installer-v2.md` §9、`macos/Sources/ShanjieInstall/Registration.swift`（`decide`）、`InstallCommand.swift`、`tools/tis.swift` | `platforms/macos/Sources/ZhuyinTypeApp/Install/InstallCommand.swift`、`platforms/macos/scripts/pkg/postinstall`、pkg `conclusion.html` |
| 10 | **數字列按鍵在組字中產生數字。** 把「m 個數字 + 合法注音尾段」與純注音以 θ ≈ 3（log10）作為 lattice 中的競爭路徑計分，而不是在 KeyDecoder 之前預先切分。為預測列加入數字 context 表，並在 ↓ 中加入全形變體。Enter 送出的內容與顯示的完全相同。 | 14% 的聊天訊息含數字串。依分數切分達到 89.8%，結構規則為 85.2%，在純中文評測集上誤判數字為 0。數字 context 列讓 hit@9 由 54.8% 升到 85.7%（p 3.9e-137）。 | `docs/contracts/digits.md`（A.3–A.12）、`digit-context.md`、`core/src/digits.rs`、`tools/build_digits.py`、`experiments/digits/README.md` | `engine/swift/Sources/ZhuyinCore/KeyDecoder.swift`、`Composer.swift`、由 `ml/src/zhuyintype_ml/training/` 語料建立的新附屬表 |
| 11 | **詞庫勝率稽核，再加入人名層與具碰撞閘門的 IT／AI 詞庫包。** 先單獨解碼每個條目的讀音，依長度與來源回報落敗率，包括 v3 以權重 0.5 對罕見字人名的影響。接著加入品牌、店家與學校（NSI BSD-3、教育部 CSV）以及國家教育研究院的 IT 名詞。每個新詞都需要：頻率下限；「已經打得對 → 略過」；把既有詞擠下 top-1 時自動捨棄；處置檔；epsilon 平手排序；讀音上已有出貨詞時新詞 −3.0。 | 沒有 LM 證據時 2 字人名落敗 98.7%。人名層修正了 酷澎／馬辣 類的漏字，guard 改壞為 0。IT 詞庫包在公開評測集上 0 退步。我們以 CC-CEDICT 為基礎的詞庫沒有專有名詞層。 | `experiments/acg-audit/summary.md`、`docs/contracts/names-lexicon.md`、`names-stores.md`、`it-pack.md`、`pack-oov.md`、`acg-pack.md` A.1、`tools/build_names.py`、`build_it_pack.py`、`build_acg_pack.py`（`detect_collisions`、`regrade`、`--recheck`）、`data/packs/*-collisions.tsv` | `ml/src/zhuyintype_ml/data/tools/build_lexicon.py`、`engine/swift/Sources/ZhuyinCore/MappedLexicon.swift`、偏好設定中的詞庫包遮罩 |
| 12 | **讓文字與讀音符合台灣用法。** 稽核我們 `s2twp` 語料中 裡長／皇後／排洩／外掛 類的損害，再逐句閘控轉換（簡體或繁體），並加入 5 字字形保護清單。只在「原生」列上評判。以教育部簡編本（只作為建置輸入）覆寫 CC-CEDICT 的破音字讀音，移除日本新字體與只屬粵語的條目，並標記中國大陸寫法（國家教育研究院兩岸對照清單 + 標記）。讓 P(reading\|word) 相對於最佳讀音。 | 他們修正後殘留由 125,635 降到 0，床／牀 由 0/13,897 變為 13,879/0。簡編本讀音 32/33 正確，規則為 22/34。國家教育研究院 114k 個名詞顯示詞庫偏向中國大陸用語。 | `docs/contracts/s2n-simplified-residue.md`、`s2f-variant-forms.md`、`sj-lm-7.md` §2.3、`canto-prune.md`、`it-pack.md` §2（標記）、`experiments/s2f/`、`experiments/s2w/cv_native.py`、`tools/readings.py`、`tools/reading_overrides.tsv` | `ml/src/zhuyintype_ml/training/lm/pipeline.py clean()`、`ml/src/zhuyintype_ml/data/tools/build_lexicon.py`、`Engine.swift`／`MappedLexicon.swift`（讀音先驗） |
| 13 | **0.6 的 Tab 預測列。** 預測列先排後繼詞，再補其他詞。查詢往回最多 5 個音節的詞起點，詞中起點放最後。把學到的選字移到最前面，而不是加分。以 ⇥ 字符顯示被動預測列。↓ 先顯示預測列項目再顯示詞庫，讓兩份清單永遠一致。量測 P1–P4 hit@9 與 KS(k)。 | Context 讓第一鍵 hit@9 由 4.6% 升到 25.4%。只用後繼詞會損失約 20 個百分點。往回 5 的起點：ACG 名稱 57.9% → 71.7%（453/0）。 | `docs/contracts/sp-partial-zhuyin.md`、`sp3-successor-first.md`、`v3-engine.md`、`predict-down.md`、`core/src/predict.rs`、`experiments/sp/README.md` | 新增 `engine/swift/Sources/ZhuyinCore/Prediction.swift`、`UI/CandidatePanel.swift`、`IMK/ControllerCore.swift` |
| 14 | **模擬使用者重播（KySS = 我們的「免選字率」）、錯誤分類與逐版本 KPI 表。** 重播以 `--correct` 選字器逐鍵輸入，回報首次顯示 top-1、KySS、每 100 字選字次數與顯示後的改寫次數。`--learn on/off` 量測學習造成的改壞。錯誤分桶區分「缺讀音／不在 top-64／排序／重排弄壞」。每個版本以凍結、SHA 固定的評測集建立一列 KPI。 | 他們的 KPI 表逐版本追蹤 KySS 0.9773、錯誤比 0.39 與每 100 列 62.3 次改寫。錯誤分類依可救回列數排序修正方向。 | `docs/contracts/engine-replay.md`、`kpi.md`、`triage.md`、`sbench.md`、`tools/kpi.py`、`tools/triage.py`、`tools/bench.py`、`tools/bench/replay/src/main.rs` | `engine/swift/Sources/zhuyin-cli/main.swift`、`ml/src/zhuyintype_ml/eval/error_analysis.py`、新增 `docs/KPI.md` |
| 15 | **強化的 Sparkle 更新程式、經公證的發布流程與 CI。** Sparkle：`startingUpdater:false` + `start()`、自己的每日靜默探測、在 delegate 中固定 feed、清除 `SUSkipped*` 鍵、`SURequireSignedFeed`，以及讓開發建置永遠不讀正式 feed 的建置標記。發布：發布說明檔閘門 → 不用 secret 的建置 → 不執行儲存庫程式碼的簽署 job（一次性 keychain、由內而外簽署、讀取 notarytool 的 JSON 判定、staple、chmod）→ EdDSA 金鑰對檢查 → 固定版本的 `generate_appcast`。CI：只改文件時略過、建置與測試、內附自我測試、打包 mutation、每鍵 p95 預算。 | `Package.swift` 已固定 Sparkle 2.10.0 但尚未接上。我們沒有 `.github/workflows`，以 ad-hoc 簽署且沒有公證（ROADMAP §4 第 12 項）。善解記錄了這條路徑上約 12 種失敗模式。 | `docs/contracts/about-update.md`、`standalone-updater.md`、`release-notes.md`、`ci-speed.md`、`ci-speed-2.md`、`.github/workflows/release.yml`、`.github/workflows/ci.yml`、`macos/Sources/Shanjie/Updater.swift`、`ShanjieKit/Updates.swift`、`scripts/check-ci-gate.sh`、`ci-changes.sh`、`test-sandbox-mutations.sh` | `platforms/macos/Sources/ZhuyinTypeApp/Update/`、`platforms/macos/Resources/Info.plist`、新增 `.github/workflows/{ci,release}.yml` |

**排在前 15 名之後的項目：**
- 以 Shift+字母與逐字母 Backspace 做英文連打，加上以覆寫證據偵測英文（DD6–DD8）。
- 中英文自動間距開關（DD9）。
- 在候選中以 ⌘⌫ 遺忘（DL13）與「不備份學習資料」（E17）。
- 在重排視窗之外凍結組字前段（DE4）。
- 以相對熵剪枝與 0.5·P2 下限重試只用於 formal 的 trigram（A23、A24）。
- 在我們依文字自適應的 c 之上，加入逐 App 的 chat／formal 覆寫（B49）。
- 標點名稱與 Apple 的標點 plist（DU2、DU3）。
- 授權帳冊與出處打包檢查（C31、C32）：先確認 Taiwan Tongues 的 TRAIL 授權允許再散布衍生的 `lm_chat`。

# 可重用的善解（shanjie）評測資料列

來源：[Nanako0129/shanjie](https://github.com/Nanako0129/shanjie/blob/bd3b6095e170012e5675b15b9443786a04103790/eval)，commit `bd3b6095e170012e5675b15b9443786a04103790`。
下表中的原始檔案為逐位元組複製。旁邊的 `*.clean.rows.txt` 檔案是評測輸入：已移除重疊的資料列；trap/daily 另外加上原始引擎的評測讀音。沒有匯入任何模型檔案或上游程式碼。

授權依據：[LICENSES/data.md](https://github.com/Nanako0129/shanjie/blob/bd3b6095e170012e5675b15b9443786a04103790/LICENSES/data.md)。data.md 沒有逐檔列出 eval/bench/；凍結的 dev302/typing76/probe/user-reported80/fw 快照是從 eval/dev 最上層的 *.txt（不含子目錄）、eval/probe、eval/rerank 的資料列建構（tools/bench.py 第 38 行），表中的 CC0 是依這些來源檔案的授權推得，不是上游逐檔的標示；eval/dev 子目錄中標為 CC BY-SA 4.0 的 it、names、acg 檔案不在其中，也沒有引入；快照的建構方式記錄在 tools/bench.py 與 docs/benchmark.md。

CC0 資料列：歸功於 shanjie 專案及其貢獻者（包括同意以 CC0 釋出其回報句子的使用者）。[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)。

holdout talk 資料列維持 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)：Wikipedia 貢獻者；The Computational Democracy Project（vTaiwan，CC BY 4.0）；國家發展委員會（National Development Council，公共參與平臺，OGDL-1.0）。頁面／修訂版本或留言的識別資訊保留在未修改的 sources.tsv 中。本資料由國家發展委員會依 [Open Government Data License](https://data.gov.tw/license) 提供。

holdout news 資料列各自保留其授權：政府新聞稿（依上游紀錄不屬於著作權保護標的；有標示者為 OGDL-1.0）、Wikinews 貢獻者（[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)），以及每位 PeoPo 作者（[CC BY 3.0 Taiwan](https://creativecommons.org/licenses/by/3.0/tw/)）。請保留 eval/holdout/next/sources.tsv，其中記錄每一列的作者／來源、URL 或標題／修訂版本，以及授權。政府資料由該檔所列的各提供機關依 OGDL-1.0 提供。

只匯入未來的 shard 2 與 3。固定版本的 eval/README.md 說明它們尚未啟用（過去確實有彙總的雜訊底限量測）；本基準把它們開放用於評測，因此之後不得再把它們當作未見過的確認資料呈現。啟用中的 shard 1 在遠端進行評測，但沒有匯入。

所有 clean 檔案僅供評測，不要用它們訓練。原始檔是來源封存，可能與訓練資料重疊；做控制洩漏的評測時請使用 clean 檔案。彼此重疊的 challenge／probe／快照集不要當作獨立觀察值合併計算。

重疊檢查（containment）在遠端主機（`$ZHUYIN_REMOTE_HOST`）上執行，對象是 corpus、Wikipedia LM、v3、lm_chat 與 lm_chat2 已準備好的訓練文字（另含可取得的歷史訓練 shard）。它在 NFC 正規化並取兩種字元摺疊的聯集後，掃描 truth 或 context 是否與訓練文字有任何共同的 8 字元子字串；完全相同的短 truth 也會捨棄。來源路徑、位元組大小、SHA-256、時間戳記與被移除的原始行號記錄在 containment.json。每個檔案只掃描到其記錄的起始大小；有變動或持續增長的檔案揭露在這份收據中。這無法證明資料不存在於無法取得的歷史語料或未來的訓練資料中。

沒有匯入任何 BY-ND 檔案。若要在評測時使用 shanjie 的完整正規化，請在被忽略的評測目錄中讀取固定 commit 中未修改的 eval/variants.tsv，並以 `zt eval rows --shanjie-variants PATH` 傳入。原始的 moedict.txt 同樣只保留在隔離的遠端參考 checkout 中；任何評測讀音都只留在被忽略的基準輸出中。

| 檔案 | 授權 | 資料列：原始／捨棄／保留 | SHA-256 |
|---|---|---:|---|
| [eval/bench/suite-v1/dev302.txt](eval/bench/suite-v1/dev302.txt) | CC0-1.0 | original | `603850e444efee38b8c121e3d666d6c540acd3a27d5a6a5070a78a962dd98456` |
| [eval/bench/suite-v1/dev302.clean.rows.txt](eval/bench/suite-v1/dev302.clean.rows.txt) | CC0-1.0 | 302 / 36 / 266 | `d68d7996eecf834d30cd3219ae10285b7615202567cec4b4fc9208566389dc6c` |
| [eval/bench/suite-v1/typing76.txt](eval/bench/suite-v1/typing76.txt) | CC0-1.0 | original | `c061f66dfade21996803137320aa986a81a3e72e3e4e1dd559abd5dc04b68d08` |
| [eval/bench/suite-v1/typing76.clean.rows.txt](eval/bench/suite-v1/typing76.clean.rows.txt) | CC0-1.0 | 76 / 27 / 49 | `4b82c95fe7955c44f47c62ffff2d50a7f8d6cb4d28f6913044730c561efdbc7f` |
| [eval/bench/suite-v1/probe.txt](eval/bench/suite-v1/probe.txt) | CC0-1.0 | original | `75aac33877ddd7907cdd6789c6da6a0f3b7c775c8c6dfa6c752a5f3e87ab6801` |
| [eval/bench/suite-v1/probe.clean.rows.txt](eval/bench/suite-v1/probe.clean.rows.txt) | CC0-1.0 | 83 / 12 / 71 | `06cdc0e9f77d05ece3279ad1232a45ef79c645d579b53484e09f6274dacf00c9` |
| [eval/bench/suite-v2/user-reported.txt](eval/bench/suite-v2/user-reported.txt) | CC0-1.0 | original | `9fd08e50f13eccea94a76faf5260e7edcf49826bd3dcf7ba4e2cf6de17bcce9a` |
| [eval/bench/suite-v2/user-reported.clean.rows.txt](eval/bench/suite-v2/user-reported.clean.rows.txt) | CC0-1.0 | 80 / 3 / 77 | `00b972fd2e41fe2755c993642eb093003f9c96e9ae966c52e87992bfdef95ab0` |
| [eval/bench/suite-v2/fw.txt](eval/bench/suite-v2/fw.txt) | CC0-1.0 | original | `6b853a329aee569ad51c731d6f188541b9670e9617838725f45a26d57845d18d` |
| [eval/bench/suite-v2/fw.clean.rows.txt](eval/bench/suite-v2/fw.clean.rows.txt) | CC0-1.0 | 186 / 27 / 159 | `dc1501ed53b02970789340f44910e9de989033eff399d5283108ee1c8305ece6` |
| [eval/dev/user-reported.txt](eval/dev/user-reported.txt) | CC0-1.0 | original | `2f3047150abcaf8115fb6b0adf44b4f5620f6bdca853a478bd063b29e30a8bd0` |
| [eval/dev/user-reported.clean.rows.txt](eval/dev/user-reported.clean.rows.txt) | CC0-1.0 | 164 / 5 / 159 | `3a5f0a9c54b13ab916a54093e22efdfa3fd0d18e083c3635be40f686912776a8` |
| [eval/sets/trap.txt](eval/sets/trap.txt) | CC0-1.0 | original | `9657119f08c4e3550b247ac082324cde0a1cd916adbf7e7eb17523045e69702c` |
| [eval/sets/trap.clean.rows.txt](eval/sets/trap.clean.rows.txt) | CC0-1.0 | 64 / 8 / 56 | `0fff433a922069af7aa5b0d3afb0a515d6d7ed3b128d2904a6203634d4b09fce` |
| [eval/sets/daily.txt](eval/sets/daily.txt) | CC0-1.0 | original | `ccdbb649a51f3a5d5b8916b020782c3d9aba46ebdb275cb7c7f0656b756cc7d7` |
| [eval/sets/daily.clean.rows.txt](eval/sets/daily.clean.rows.txt) | CC0-1.0 | 45 / 13 / 32 | `dc47dc84994962826e42d008b53e1e6c0ead6223403c445f5595abd68cd44933` |
| [eval/holdout/next/shard-2/news.txt](eval/holdout/next/shard-2/news.txt) | Mixed redistributable: government releases/OGDL-1.0, CC BY 4.0 Wikinews, CC BY 3.0 TW PeoPo (per-row sources.tsv) | original | `1c9c928c307e54d29768450f1f936f16ea426c8758cedf2fc430bf1812d8eca4` |
| [eval/holdout/next/shard-2/news.clean.rows.txt](eval/holdout/next/shard-2/news.clean.rows.txt) | Mixed redistributable: government releases/OGDL-1.0, CC BY 4.0 Wikinews, CC BY 3.0 TW PeoPo (per-row sources.tsv) | 150 / 48 / 102 | `824170f94910e4362da028c0455f5aa34c7775ed04db9aae6959389ea8c1ca16` |
| [eval/holdout/next/shard-2/talk.txt](eval/holdout/next/shard-2/talk.txt) | CC BY-SA 4.0 (Wikipedia, vTaiwan/Computational Democracy, NDC; per-row sources.tsv) | original | `71f8a5e1a422c00a37598d237744ff49a229ce04dc3f37a1170ea98fe8b6e67e` |
| [eval/holdout/next/shard-2/talk.clean.rows.txt](eval/holdout/next/shard-2/talk.clean.rows.txt) | CC BY-SA 4.0 (Wikipedia, vTaiwan/Computational Democracy, NDC; per-row sources.tsv) | 150 / 15 / 135 | `b5103c1744578a22e59fef28c43e9a92e2242d7bc7f370246884408ef56e8cad` |
| [eval/holdout/next/shard-3/news.txt](eval/holdout/next/shard-3/news.txt) | Mixed redistributable: government releases/OGDL-1.0, CC BY 4.0 Wikinews, CC BY 3.0 TW PeoPo (per-row sources.tsv) | original | `fc1477681a6f97aaef83b4df388de3f0bc066eb3ef3b936677fcd94ade14e1b4` |
| [eval/holdout/next/shard-3/news.clean.rows.txt](eval/holdout/next/shard-3/news.clean.rows.txt) | Mixed redistributable: government releases/OGDL-1.0, CC BY 4.0 Wikinews, CC BY 3.0 TW PeoPo (per-row sources.tsv) | 150 / 36 / 114 | `6be77fd08d2375274dbe18263c0cac1f0e1c8fe657b54f797e5d0699ec4dfd57` |
| [eval/holdout/next/shard-3/talk.txt](eval/holdout/next/shard-3/talk.txt) | CC BY-SA 4.0 (Wikipedia, vTaiwan/Computational Democracy, NDC; per-row sources.tsv) | original | `743d3687f74e43c9a0d5ea40f7c6b463cb360b0e60bd7bd5b11265f291d984d3` |
| [eval/holdout/next/shard-3/talk.clean.rows.txt](eval/holdout/next/shard-3/talk.clean.rows.txt) | CC BY-SA 4.0 (Wikipedia, vTaiwan/Computational Democracy, NDC; per-row sources.tsv) | 150 / 11 / 139 | `0fbc29d3f9e4f10787328c26f8bf0de5e4782e7193af3e9a1df435c578b65db4` |
| [eval/holdout/next/sources.tsv](eval/holdout/next/sources.tsv) | CC0-1.0 | original | `f58939d776d6c5132004878a3ed2a700fc9d4f8f044b175f6d704448d599eb5d` |

其他收據雜湊值：
- `manifest.json`: `2e0fd54ec20711ffea6ed128f0ce0074141098883c60d30f26dc8fc8b1e289a9`
- `containment.json`: `ea33d24b40ffa488711a654dc447f699a9b5670c34b3527a0226a94cb275c3b7`

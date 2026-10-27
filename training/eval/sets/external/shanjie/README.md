# Reusable shanjie evaluation rows

Source: [Nanako0129/shanjie](https://github.com/Nanako0129/shanjie/blob/bd3b6095e170012e5675b15b9443786a04103790/eval), commit `bd3b6095e170012e5675b15b9443786a04103790`.
Original files below are byte-for-byte copies. The adjacent `*.clean.rows.txt` files are the evaluation inputs: overlapping rows were removed; trap/daily additionally received the original engine's evaluation readings. No model files or upstream code are imported.

License authority: [LICENSES/data.md](https://github.com/Nanako0129/shanjie/blob/bd3b6095e170012e5675b15b9443786a04103790/LICENSES/data.md). The frozen dev302/typing76/probe/user-reported80/fw snapshots inherit the CC0 license of their source rows (eval/dev, eval/probe, eval/rerank); the snapshot construction is documented in tools/bench.py and docs/benchmark.md.

CC0 rows: attribution to the shanjie project and contributors (including the users who consented to CC0 for their reported sentences). [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

Holdout talk rows remain [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/): Wikipedia contributors; The Computational Democracy Project (vTaiwan, CC BY 4.0); National Development Council (public participation platform, OGDL-1.0). Page/revision or comment identity is retained in the unmodified sources.tsv. This data is supplied by the National Development Council under the [Open Government Data License](https://data.gov.tw/license).

Holdout news rows retain their individual licenses: government press releases (not copyright subject matter as recorded upstream; OGDL-1.0 where specified), Wikinews contributors ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)), and each PeoPo author ([CC BY 3.0 Taiwan](https://creativecommons.org/licenses/by/3.0/tw/)). Preserve eval/holdout/next/sources.tsv for each row's author/source, URL or title/revision, and license. Government data is supplied by each providing agency identified there under OGDL-1.0.

Only future shards 2 and 3 are imported. The pinned eval/README.md says they have not yet been activated (historical aggregate noise-floor measurements did exist); this benchmark exposes them for evaluation, so they must not subsequently be presented as unseen confirmation data. The active shard 1 is benchmarked remotely but is not imported.

All clean files are evaluation-only. Do not train on them. Originals are provenance archives and can contain training overlaps; use the clean files for leakage-controlled evaluation. Do not aggregate overlapping challenge/probe/snapshot sets as independent observations.

Containment ran on gpu-lab over the prepared training text for corpus, Wikipedia LM, v3, lm_chat and lm_chat2 (plus available historical training shards). It scans any shared 8-character substring of truth or context after NFC and the union of both character folds; exact short truths are also dropped. The source paths, byte sizes, SHA-256, timestamps and removed original line numbers are in containment.json. Each file is bounded to its recorded starting size; changed/growing files are disclosed in that receipt. This does not prove absence from unavailable historical corpora or future training.

No BY-ND files are imported. For full shanjie normalization at evaluation time, read the unmodified eval/variants.tsv from the pinned commit in an ignored evaluation directory and pass --shanjie-variants PATH to rows_eval.py. The original moedict.txt is likewise retained only in the isolated remote reference checkout; any evaluation readings stay in ignored benchmark output.

| File | License | Rows: original / dropped / kept | SHA-256 |
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

Additional receipt hashes:
- `manifest.json`: `2e0fd54ec20711ffea6ed128f0ce0074141098883c60d30f26dc8fc8b1e289a9`
- `containment.json`: `ea33d24b40ffa488711a654dc447f699a9b5670c34b3527a0226a94cb275c3b7`

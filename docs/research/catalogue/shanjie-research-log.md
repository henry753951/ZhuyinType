# Shanjie (善解) research record: catalogue of ideas, and what ZhuyinType should take

**Sources read.** Under `references/shanjie/`:
- `docs/research-log.md`, all 9,353 lines, 2026-10-03 to 10-26. It was read in seven chunks by parallel readers, and the results were merged and de-duplicated here.
- `docs/PLAN.md` (all), `docs/methodology.md` (all dated sections), `docs/iteration-plan.md`, `docs/triage.md`, `docs/verification.md`, `docs/kpi.md`, `docs/benchmark.md` (structure and the external-model table).
- The README tables ("已出貨" and "試過、沒出貨"), and `docs/releases/*.md` (v0.1–v0.5.0, the model and data releases, and the naming README).

**Shanjie in one paragraph.** A Rust core plus a Swift shell. The engine decodes a word bigram lattice built on the McBopomofo lexicon plus an overlay. Two profiles:
- **Formal** adds a trigram ("連文"), with KN smoothing turned off.
- **Chat** adds a colloquial bigram table ("常談") and a reading-channel term.

On top of the lattice, a 14M-parameter character-LM reranker ("會意") rescores the n-gram top 8, with a gate of α/τ/δ. Learning is keyed by context. Behind all of it sits a very strict, pre-registered evaluation process.

**Their headline numbers at v0.5.0:**
- discordtune (the user's Discord chat), chat profile: 87.90%.
- wikitune-clean, formal profile: 75.29%.
- Holdout shard-1 (300 rows): chat 237, formal 241.
- Per-key p95 2.05 ms.

**Status column** (ZhuyinType): **have**, **have better**, **lack**, **n/a**, or **verify** (I could not confirm from our docs or code). I checked several claims against our code:
- `nlp/engine/Sources/ZhuyinCore/Conversion.swift` decodes with an LM-history state and includes the end score when ranking.
- `training/eval/stats.py` already does exact McNemar, paired bootstrap, a Poisson break bound, MDE and clusters.
- `training/eval/rows_eval.py` reports oracle@16 and CER, and applies Shanjie-style variant folds.
- The pkg installs to `/Library/Input Methods`.
- `RecallKind` has four kinds: whole, span, typo and phrase.

**Verdict column:** adopt, adapt (how), skip (why), or **we lead**.

Paths in the "Files" column are relative to `references/shanjie/`. Dates are 2026 and written as in the log.

---

## A. Lexicon and data

| # | Date | Name | What they did | Result (numbers) | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| A1 | 10-03 | S1 Wikipedia/Wiktionary overlay | Added 2–4-char pure-Han words. Filters: all characters already single entries, no simplified-only characters, Wikipedia titles only when the title is a compound of a lexicon word plus one character. Build pinned to dump SHA; `--check` rebuild; no words may be picked from eval. | oracle@64 96.9→97.8%. Adding all 1.04M titles was worse, because names polluted the candidates. The holdout OOV subset rescued 0 rows, so the hypothesis was refuted, and the metric was changed afterwards with disclosure. RSS 127→375 MiB. | `tools/build_overlay.py`, `docs/contracts/s1.md` | have (CEDICT+wordfreq) | adapt: if we add titles, use the compound filter and a hash-pinned `--check` build |
| A2 | 10-03 / 10-14 | Scores for unscored words; manual-word list | A word with no frequency gets the 25th-percentile score of same-length words (2 chars −7.17). Manual words get that plus a bonus of 0.5, later 0.75, and a golden test checks each one is top-1. | Manual words 3/9 → 9/9. Every eval set byte-identical. | `data/lexicon/overlay-manual.tsv`, `cli/tests/golden.rs` | lack (no curated list) | adopt: cheap fix for user-requested words |
| A3 | 10-03 / S2c | Cap overlay scores at corpus frequency | Overlay score = min(lp, log10 c/N); words never seen in the corpus get −1. | wikitune 11/0, p=0.001; chat log-loss −0.0088 [−0.0137, −0.0038]. Mixing corpus frequency into the prior (β 0.3/0.5) cost chat 100+ rows. Later (10-25), the cap also blocks curated boosts (君悅 −6.58 vs 軍樂 −6.30 even at +1). | `core/src/lm.rs` (`CappedLexicon`) | lack | adapt: cap low-evidence sources, but decide explicitly whether a curated boost may beat the corpus counts |
| A4 | 10-03 | OpenCC first-mapping bug | `convert()` applied the first STCharacters mapping to text that was already Traditional. 72 characters were changed (吃→喫, 台→臺, 里→裏), including in the reference answers. | 喫 54,153 vs 吃 4,283 counts. Rounds 4–7 of tuning rested on corrupted references. After the fix: 吃 5,022 vs 喫 4. | `experiments/s2/build_counts.py` | verify | adopt: audit our Wikipedia s2t step on character pairs such as 吃/喫 |
| A5 | 10-14 | OpenCC TWPhrases applied to Traditional text | Phrase conversion ran on sentences that were already Traditional: 插件→外掛, 程序→程式, 對象→物件. | 2/10 probe sentences then decoded wrongly. Fix scheduled for model-v7. | `build_counts.py` L166–171 | verify | adopt: run phrase conversion only on Simplified sources |
| A6 | 10-05 | S2n simplified residue | Decide per sentence whether it is Simplified, with a Taiwan whitelist (吃 皂 唇 岩 岳 咸). Proposed release check: zero simplified-only characters in the n-gram table. | Seven residue words 125,635→0 counts; cvtune formal 20/1, p=2e-5. | `docs/contracts/s2n-simplified-residue.md` | verify | adopt the zero-simplified release check |
| A7 | 10-07 / 10-12 | Taiwan glyphs mislabelled as simplified | OpenCC treats 床 and 秘 as simplified (起牀). The SIMP_ONLY set also held 14 common Taiwan characters (么峰庄床晒洒灶痒秘粽群肴虱霉). Same-reading variants now merge counts. | Glyph probe 10/8 → 16/16. Cofacts lost 79,779 sentences vs 47,333 after the fix. Rule: print the top characters that triggered each drop rule. | `docs/contracts/s2f-variant-forms.md`, `experiments/s2f/` | verify | adopt: check our filters for 群, 床, 秘 |
| A8 | 10-06/08 | S2w: MediaWiki's own zh-tw conversion | Converted with MediaWiki's rules plus NoteTA groups via zhconv-rs, which panics on `=>`. | MediaWiki right 69 vs 16 on 100 manual checks, but no typing gain. It changed glyph policy (佔→占), and 的市佔 became 的士站. Not shipped. | `experiments/s2w/` | n/a | skip: a converter is also a glyph policy |
| A9 | 10-06 | Expected (fractional) segmentation counts | Count n-grams as expected values over the segmentation lattice (Mori 2006) instead of the hard best path. | c(後,到) 0→206, c(好,吧) 0→9. cvtune chat 26/11, p=0.020. Build takes about 8 min. | `tools/build_lm.py`, `experiments/s2/build_counts_text.py --expected` | lack (our segmentation is hard EM) | **adopt**: fixes bigrams absorbed by a longer lexicon word |
| A10 | 10-05 | Surname splits (照建議→趙建議) | Lone 趙 has 26,281 counts, 88% followed by a single name character, vs 照 7,873. | Dividing surname counts by 4 or 10: net −6 to −14, rejected. Plan: an NER name class. | — | verify (same Wikipedia bias likely) | adapt: count person names as one class token |
| A11 | 10-04 | S2r sandhi rows for 一/不/法/和 | MOE sandhi variants at −0.5. Excludes word-final 一 (133 words) and ordinals or names (41). | Probe: cvtune 39/3, p=5.6e-9; discordtune 64/2, p=6e-17. Original readings unchanged. | `tools/build_sandhi.py`, `data/lexicon/sandhi-add.tsv` | have (一/不) | adapt: add 法 ㄈㄚˇ, the exclusion list (統一, 北一女, 星期一) and the −0.5 offset |
| A12 | 10-26 | Neutral-tone tolerance rows | For multi-character words with a neutral tone, add a full-tone row (same final, best non-neutral reading) with penalty δ=2.0. 的/了/著 exempt. 1,502 rows. | Neutral-tone check set: chat 177→277 (+100/0), formal 161→264 (+104/1). One user-report break (都留→都流). | `docs/contracts/neutral-tolerance.md`, `experiments/neutral/build_checkset.py` | verify (KeyChannel 二聲↔輕聲 cost 4.5; v3 trained with 30% neutral→tone-1) | **adopt** their check-set builder to measure our coverage |
| A13 | 10-17 | Reading-aware n-gram (A1/A2/B) | A1: demote neutral-tone rows copied from the full-tone score (44 chars). A2: penalise tone-1 sentence-final particles. B: count n-grams per reading using a rule labeller. | A1: 3/0 (fixed 吧). A2: lytune chat −213. B: collapsed, 5/1462, because the labeller was wrong 40.8% of the time (地, 得). | `docs/contracts/reading-aware-0.md` | have better (P(reading\|word)) | adapt A1 as an audit; skip B |
| A14 | 10-07 | SW: reading-specific demotion list | Subtract δ=2.0 from a word only under a nonstandard reading (睪丸 read as ㄍㄠˇ ㄨㄢˊ). | 搞完這波 fixed in 4/4 settings; one repick overrides it (8/8). | `data/lexicon/demote.tsv`, `docs/contracts/sw-sensitive-demote.md` | lack | adopt: small embarrassing-collision list |
| A15 | 10-22 | Colloquial reading 紮 ㄓㄚ | Added ㄓㄚ to 29 rows with copied scores. | All sets 0/0; CER −0.08 pp; failed the net>0 rule (net 0). | `docs/contracts/zha-reading.md` | verify | adapt: approve one at a time for characters that otherwise cannot be typed |
| A16 | 10-26 | Reading sources and shinjitai removal for SJ-LM-7 | Reading priority: MOE concise dictionary → MoeDict → rules. Japanese forms removed via OpenCC. Tutor agents grade samples. | 9,339 Japanese forms removed. Rule readings 22/34 → 28/34 after fixes. Candidate window had shown 変移/辨疑 above 變. | `tools/readings.py`, `tools/build_overlay.py` | verify | adopt the shinjitai filter for CEDICT/wordfreq |
| A17 | 10-14 | Cantonese pruning | Drop overlay entries with Cantonese-only characters unless they are in MoeDict. | 848 rows removed (冇, 唔該, 我哋); evals byte-identical. | `docs/contracts/canto-prune.md` | verify (wordfreq may contain them) | adapt |
| A18 | S2r-2 | Polyphone reading rule (from zaoseq) | A polyphone gets its reading only from an explicit word reading. zaoseq removed 10,813 false homophones this way. | Shanjie audit: 4/50 overlay polyphones wrong; ~8% guessed wrong. | `experiments/s2/overlay_polyphones.py` | verify | adopt as an audit of Unihan-derived readings |
| A19 | 10-10 | "Does each word win when typed alone?" audit | For every lexicon/pack word, type its reading with no context and check that it is top-1. | 16,082/60,798 lose (26.5%). Two-character words: 98.7% lose; 3 chars 38.9%; 4 chars 7.9%. Wiktionary 2-char 78.1%, base 20.8%. 99.9% lose on the LM term, median −1.97. | `experiments/acg-audit/` | lack | **adopt**: cheap, by source and length |
| A20 | 10-09 | Missing word vs ranking error (M/R) | Label each wrong row M (missing word) or R (ranking error). | User reports: 7 M / 24 R. Correct answer median rank 2, gap about 0.5. Only 2 rows outside the top 64. | `eval/dev/user-reported.txt` | lack | adopt as the first triage step |
| A21 | 10-09 → 10-23 | ACG pack (slices A, A2, A3, A4) | Wikidata (sitelinks ≥3), zh-Wikipedia conversion groups, 400 anime articles, yearly list pages, names with ·. Optional, menu toggle, 25th-percentile scores. | 14,357 → 44,552 words. works-chat 113→124 (p=0.001); interpunct names 21→39/45; recent 14→22. +25–30 MiB. 95–96/100 sampled genuine. | `tools/build_acg_pack.py` (47 tests) | lack | adapt if we ship packs: optional, cached reproducible build, 100-sample precision gate |
| A22 | 10-09 → 10-15 | Collision rule for added vocabulary | A new word may not take top-1 from an existing word (rule c; NEW_BEHIND_SHIPPED=3.0; `first_named`). Sentence-context hijack check. | Removed 105 hijacks (家母→加姆, 牧笛→穆迪). The detector had run with KN off, i.e. not the shipped setting. | `acg-collisions.tsv`, `tools/build_overlay.py` | n/a | **adopt** for any lexicon addition; run the check with shipped flags |
| A23 | 10-10 | model-v5: pack words in the segmenter | Putting pack words in the segmentation vocabulary re-segmented the corpus (萊+恩 1,336→0). Fixed with additive counting plus keeping the old word classes. | First version vetoed: wikitune 2,133→2,112, p=0.026. | `docs/contracts/model-v5.md` | n/a | adopt the lesson: new vocabulary must leave general counts row-identical |
| A24 | 10-24 | pack-oov | Pack words bypass the general LM and are scored with bonus B. | Confirmation 13/7 failed; V1c variant 15/2. | `docs/contracts/pack-oov.md` | n/a | skip |
| A25 | 10-11 → 10-25 | Names layer (N1, creators, stores) | Sources: NSI (BSD-3), Wikidata CC0, MOE school lists. NAMES_BONUS=2. Accept a name only if its PTT usage exceeds its lexicon homophone; strip inflated short names (辛巴). | Names set chat 44→56 (12/0). Creators chat 53→103, 0 breaks. Stores 18→199/225. +0.5 MiB. | `tools/build_names.py`, `docs/contracts/names-*.md` | lack | **adopt** (brands, companies, schools) |
| A26 | 10-23 | IT+AI pack | NAER open data (15275/15416), CGroup/IT. Add only words the decoder gets wrong. Threshold 50 × bonus 3. | 141 of 151 manual words were already right. Chat 46/60 (pass), formal 39/60 (fail, so formal is guard-only). discordtune 0/3. | `docs/contracts/it-pack.md`, `tools/build_it_pack.py` | lack | adopt: NAER CSVs are open; the bonus is the main lever |
| A27 | 10-24/25 | Net slang mining | PMI plus adjacency entropy over PTT, then Sonnet classification. Drop a slang word if a lexicon homophone has more PTT usage. | Median known-slang rank 1,589 vs 20k by raw counts. Round 1: 507 words, discordtune 2/0. Round 2: 910 words, 86.0% (gate 90%). | `docs/contracts/net-slang*.md` | lack | adopt the mining method |
| A28 | 10-24/25 | Reranker undoes new vocabulary | Check sets measured with the reranker on and off. | Slang: reranker on 779/906 vs off 818/906 (48 breaks / 9 fixes). Stores: 10 breaks / 8 fixes. | — | have the risk (v2b, v3) | **adopt**: test new words with the full shipped pipeline |
| A29 | 10-13 | Domain-pack survey (medicine, law) | Coverage AI 11/30, IT 35/49, medicine 27/34, law 36/39. Entries without counts lose (插件 −8.17 vs 執事 −5.85). MOE dictionary is CC BY-ND, so rejected. | Plan: IT/AI on by default, medicine/law off. | PLAN | lack | adapt |
| A30 | 10-12 → 10-26 | Licensing tiers and source audit | Core uses only CC0/BY/BY-SA/government data; NC sources go to a separate repo. robots.txt returning HTTP 500 counts as disallow. zh Wikinews CC BY OK; TED and newspapers excluded. No licence-clean Taiwan chat corpus exists (MozTW 26,393 sentences). | Policy | `LICENSES/data.md`, `docs/data-files.md` | partial (Taiwan Tongues TRAIL unresolved) | adopt the tier split before shipping a chat model |
| A31 | 10-18 | Public colloquial pool | WildChat-TW (ODC-BY), YODAS zh000 (CC BY 3.0), Tatoeba, MASSIVE zh-TW. Split, 4–40 Han, ≥70% Han, PII and code stripped, narrow simplified filter (3,795 chars), deduplicated, leakage-checked. | 143,895 sentences, 3.0M Han characters. YODAS lost 45% as simplified. Leakage removed 2,237 (1,112 identical to cvtune). | `experiments/p2-data/` | lack | **adopt** for our chat LM |
| A32 | 10-25 | OPUS zh_TW subtitles | Use the raw zip (9,891 files, 10.73M sentences) with no per-film cap. Mainland phrase filter protected by word boundaries; two Sonnet reviewers per 1,000 lines, matched back by text. | 4,432,866 sentences, 42.8M Han characters. Residual 6.7% → 1.7% over seven proofreads (Wilson 0.9–3.0%). | `experiments/sj-lm-7/clean_subs.py`, `review_subs.py` | lack | **adopt**: biggest open colloquial source |
| A33 | 10-25 | PTT and g0v cleaning | PTT: join only auto-wrapped lines (77–80 display columns). g0v #rand0m (CC0): drop quotes and code. Deduplicate against eval sets with min length 6. | PTT sentences 21,387→36,896 on 3,000 posts. g0v: 15,318 sentences. Min length 4 would drop ~530k common phrases. | `ptt_clean.py`, `g0v_clean.py`, `dedupe_eval.py` | lack | adapt (check PTT terms; counts only) |
| A34 | 10-12 / 10-18 | S7f: retraining with real and generated colloquial text | Real text (archive.tw CC0, Cofacts, Wikipedia talk) plus LLM-generated sentences (US$16.83). | Failed when tuned on non-chat sets (A1 −126, A2 −107). At fixed values, breaks went 163→131. Seed noise is 32 rows. With 3 seeds: A2 +104..+119 vs control +55..+76, but below MDE. | `docs/contracts/s7f-colloquial.md` | lack | adapt: tune on colloquial data and use ≥3 seeds |
| A35 | 10-03 | Targeted synthetic sentences for confusable pairs | LLM sentences aimed at 在/再 etc. (29,810 → 21,611 after filtering). | Regressed on both the tuning set and Discord. | `experiments/s2/` | n/a | failure to avoid |
| A36 | 10-14 | model-v6 LLM top-up text with filters | 500k characters from gpt-oss-120b (US$2.87), filtered for eval overlap (90), seed word unused (405), Simplified (67) and intruding words (312). | Folded into model-v6. | `experiments/model-v6/generate.py` | have (generated chat sentences) | adapt those filters to our generated chat corpus |
| A37 | 10-20 | Web long text as an eval set | HPLT forums and blogs; Wikisource prose. | HPLT 12.7/5.3/6.7% bad (SEO tools insert real words). Wikisource 13.7% bad (simplified originals, classical quotes). Abandoned. | `experiments/longctx-eval/` | n/a | avoid: use edited text or private chat |

## B. Language model and decoding

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| B1 | 10-03 | Interpolated prior plus a scored end-of-sentence term | λ=0.7 interpolation of lexicon prior and LM, plus scoring `</s>`. | +112 cvtune / +207 wikitune (p 1e-16, 1e-31). EOS alone +17/+19. | `reference/proto/lm.py` | have (end share s) | we have it |
| B2 | 10-03 | Pruning that keeps context totals | Keep pre-pruning totals; pruned mass goes to back-off. | Naive renormalisation lost 61/137 rows. Chat ≥2: 23.2M→8.8M entries for −11 rows. | `tools/build_lm.py` | verify | adopt if we prune |
| B3 | 10-15/16 | Trigram: size sweep, entropy pruning, floor | Count-pruned tables (100–1,133 MB). Relative-entropy pruning. Floor P3 = max(P3, 0.5·P2), because a sparsely stored context taxed every unseen word (搞完這波→睪丸這波). | Only the unpruned 1,133 MB table passed at first. Entropy pruning H3: +64 vs T3 +35. Floor: tune +73, breaks 141→120. | `experiments/derisk-1015/e1/`, `core/src/trigram.rs` (`TRI_FLOOR`) | lack (our snapshot trigram failed the test set, MODEL §11) | **adapt**: our failed trigram may have lacked the floor and entropy pruning; retry for the Wikipedia component only |
| B4 | 10-16/17 | Trigram shipped in the formal profile only, mmap | Formal profile turns KN off when the trigram is present; chat unchanged. | wikitune +49, lytune +36, discordtune +40, cvtune +24, none worse. Installer +48 MB. Footprint 444→304 MB with mmap; p95 +0.01 ms. | `docs/contracts/trigram-core.md`, `trigram-mmap.md` | lack | adapt (see B3) |
| B5 | 10-10 → 10-23 | Kneser–Ney continuation side file | Interpolated N′ back-off. | +33/13,236 rows; holdout tied. KN interacts badly with the trigram (formal FT +106 with KN off). KN off in chat −33 (p 0.012). Power: +0.25% needs ~20k rows. | `docs/contracts/kn-*.md`, `tools/kn_cont.py` | have (modified KN) | we lead; note the KN×trigram interaction |
| B6 | 10-08 / 10-16 | Brown word-class back-off (K=512) | Class n-gram term μ applied to unkept bigrams. | cvtune +21/+26 (p 0.002). μ 0.8 with KN caused lytune −45 (p 7e-5). μ 0.2: lytune +92. | `tools/build_classes.py`, `docs/contracts/classes-v5.md` | lack | skip (v3 covers generalisation) |
| B7 | 10-16 | Regression attribution along a path | Turned components on one at a time (class, KN, trigram) to split a regression. | Found class×KN: −39/−56 with KN on vs −10/−4 with KN off. | `docs/contracts/lytune-attrib.md` | lack | **adopt** the method |
| B8 | 10-13 | Recombine by (string, last word) | The decoder had kept one path per surface string (夏來分析). | lytune and discordtune better in 4/4 cells (p ≤ 0.017); n-best has fewer distinct strings. | `docs/contracts/dec-recombine.md` | have (Conversion state = LM history; ranks include `endScore`) | we already do this |
| B9 | S2c | Pinned-word gaps | Decode each gap between pinned words with the left pin as history and the transition into the right pin as the end term. | Shipped with a must-fail test. | `core/src/engine.rs` | verify (FixedWord path) | adopt as a test |
| B10 | 10-11 | P1a: 在/再 confusion classifier | Sparse linear classifier added as s+μf. | 再>在 errors 11→10 (needed halving); +0.05 pp. Bugs: ln vs log10 (μ 2.30× too big) and a class-capped prior. | `docs/contracts/p1a-confusion.md` | n/a | skip; adopt the checklist: same log base, remove the training prior |
| B11 | 10-13 | Function-character homophone set (fw) | 在/再, 作/做, 以/已, 是/試 across 6 sets. 作說明 is acceptable formal usage. | Reranker vs n-gram 30/1, but fw still 23/94 wrong (margin below δ). | fw set | partial (`training/homophone/`) | adopt: score legitimate alternatives as correct |
| B12 | 10-14 | Fuzzy input with three gates | G0 only when nothing parses; G1 after the exact top-5; G2 in the lattice with δ. δ chosen with clean-input breaks ≤0.5%. | Fuzzy-sound top-1 91–95%. Tone 62–67%. Adjacent key broke 4/76 clean sentences (5.3%). | `docs/contracts/fuzzy-research.md` | have better (KeyDecoder, KeyChannel, exact-first margin) | we lead; adopt the split into legal-reading vs illegal-reading errors |
| B13 | 10-15 / 10-10 | Initials-only abbreviation | E3 whole-sentence initials-only; V3 optional finals. | Top-1 ratio 0.789/0.808 (<0.9); oracle@64 289/300; 148 ms/row. Device test: keys got stuck, so hidden. | `docs/contracts/v3-engine.md` §12 | lack | skip |
| B14 | 10-18 | E-C: joint retune of decoder weights and fusion | λ, KN β, trigram floor, α/τ/δ tuned together. | Best cell only +5; other cells swap rows (UB 0.67–2.67%). | `experiments/e-c/` | have (tuned) | note: retuning rarely pays |
| B15 | 10-18 | E-D: wider per-reading lexicon cap | PER_KEY 12→24/48/1000. | Chat oracle@64 ≤ +0.03 pp; formal −2 to −5. | `experiments/e-d/` | n/a | skip |
| B16 | 10-20 | Near-miss diagnostics (D2/D3) | Gap histograms, confusion-pair concentration, a Wikipedia-bias index. | 40% of ranking errors have gap <0.5 and 69% <1; 88% in the top 8. Top-10 pairs only 19.8% (no pair table). Wikipedia bias 20.2% (opened P4). | `experiments/diag-near-miss/d23.py` | partial (`diagnose.py`) | adopt before any targeted fix |
| B17 | 10-20/21 | P4→P4d colloquial term | Colloquial bigram table "常談" (68,055 words) as a separate log-linear term with weight μ. Penalty γ for overlay-only words. Applied in the reranker layer, anchored on the n-gram #1, with a soft gate. | Before fusion (P4b): 128/92, rejected. P4d: 74/43 (p 0.0053). Confirmation on chatjudge-2: 108/39, +69, p 1.1e-8, CER 2.85→2.61%. RSS +29 MiB. | `core/src/colloquial.rs`, `docs/contracts/colloquial-term.md`, `docs/releases/sj-cl-1-68k.md` | partial (chat LM mixed into the lattice) | **adapt**: add a real-colloquial table as its own term, anchored and gated |
| B18 | 10-21 | P5 reading-channel term | Penalty for typing a single character under a minor reading, relative to its top reading, capped at 4 (fixed 變快→便會). | discordtune 23/0 (p 2.4e-7). One break (長歪了→掌歪了) from a −6.8 placeholder score. The softmax estimator was biased against 9,404 tied words. | `core/src/colloquial.rs` (`channel_of`) | have better (P(reading\|word)) | we lead; audit placeholder minor-reading scores |
| B19 | 10-22 | Extra terms independent of α | Coefficients inside the fused score are μ/α and ν/α, so each strength must use its own α. | Core vs offline: 0 rows differ in every cell. | `docs/contracts/colloquial-levels.md` | n/a | adapt if we add strength levels |
| B20 | 10-08/13 | Reading-blind bigram counts | 空 ㄎㄨㄥˋ borrowed counts from 空間 (版空); 啦 neutral-tone counts went to tone 1. | Planned: heteronym-aware collocations. | PLAN | partial (bigram is reading-blind; v3 sees syllables) | adapt via v3; don't use rule labellers (A13) |
| B21 | 10-22 | Composition cap 128 with a frozen prefix | Beyond W = context−3 (61) syllables, freeze the decoded prefix word by word, keeping a 16-syllable buffer; never decode it again. | Key path p95 26–28 → 13.4 ms; `top1_sha256` identical. | `core/tests/engine_cap.rs`, `docs/contracts/composition-cap.md` | partial (KeyDecoder freezes >48 keys; v2b/v3 still run on the whole sentence) | adapt: freeze lattice and rerank past the model window |

## C. Neural reranking and fusion

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| C1 | 10-03 | N0: LLM rerank vs reading-constrained generation | Gemma rerank of the N-best vs character generation. | Rerank 91.4% at p95 848 ms; generation 82.5% (prefers fluent words). Ceiling: oracle@16 94.7%. | `experiments/n0/` | have better | we lead; always report the oracle ceiling |
| C2 | 10-03 | N1: cloud judges return an index only | OpenAI-compatible endpoint; 32 configurations. | gpt-6-luna 92.7% (p95 3.1 s); Cerebras gpt-oss-120b 92.4% (740 ms). US$2.24. | `experiments/providers/` | lack | skip for now; the design is right (index only, timeout falls back to local) |
| C3 | 10-05/06 | LLM judges: position bias | Pick-from-list prompts vs per-candidate log-probability. | Apple FM 83.5→75.4%, 72% pick the first item; Clef top-1 picked 31.6% first vs 76.9% last. Qwen3-8B with context: 87.6 vs 84.4 (p 0.037) at 0.5 s/row. | `experiments/s5-*` | have better | we lead |
| C4 | 10-03 | Discriminative perceptron over the top 64 | Dense plus sparse features; trained on the user's chat ×16 plus generic data. | First configuration to improve all three sets (+24/+58/+67). Character-pair features overfit (−30 to −54). | `experiments/s2/rerank.py` | lack (learned fusion planned) | adapt: train the fusion locally on user commits, upweighted |
| C5 | 10-06/07 | Offline fusion study | Weighted fusion vs a τ threshold on stored scores; literature (Kerrigan 2021, McBopomofoLM β=0.3). | Weighted 89.2 vs τ 87.8 (p 0.17). A one-parameter Bayes rule equals a gap threshold. ~1,100 rows needed to detect 2 points. | `experiments/fusion/` | lack | adopt the power planning |
| C6 | 10-11 | S7a: tiny character-LM reranker (M, 14.4M) | Reranks the n-gram top 8 only when the top-1/top-2 gap < τ; 2 characters of context. | discordtune +67 (p 0.0008); holdout 180→200 (p 0.0003). S (5.7M) failed. | `experiments/s7a-tinylm/`, `core/src/tinylm.rs` | have better (v2b 31M top 16, v3 in the lattice) | adapt the τ uncertainty gate |
| C7 | 10-11 / 10-17 | Bigger rerankers (L 30M, XL 65M) | Same data, mostly Wikipedia. | L +5.64 pp on tuning sets but −30 on chat (−97 vs M). XL passed nothing under Holm. Rerank p95: M 8.6, L 16, XL 32 ms. Tokens per parameter: 17 / 8 / 4. | `docs/contracts/rerank-size.md` | have | skip widening without new data |
| C8 | 10-11 / 10-19 | int8 weights | Per-row symmetric int8 with a McNemar non-inferiority check. For 0c, per-row failed; per-output-column int8 with f32 embedding worked. | M 57.7→14.7 MB, no losses in 12 cells. 0c per-row 296/300, p99 0.437 (fail); per-column 300/300. | `experiments/s7a-tinylm/int8check.py`, `tools/convert_oc.py` | lack (fp16 Core ML 129 + 60 MB; ROADMAP §4.6) | **adopt** per-channel int8, fp32 embedding, verified by a paired test |
| C9 | 10-12 | Objective ceiling ("1 break = 10 fixes") | Computed the best achievable net under that weighting. | Even choosing on the judge set: +17..+23 / 4,091, below the 32-row noise. Switched to net 1:1 plus a break-rate cap. | `docs/methodology.md` 10-12 | n/a | adopt: compute the ceiling before choosing a weighting |
| C10 | 10-12/13 | Fusion parameters α/τ/δ on the private chat set | α weight, τ no-rerank gap, δ margin needed to switch. Stored per profile in the model header. | R3′ (0.3/2/0.25): 125/16, net +109, break rate 0.24%. R1: 2.16%. Holdout chat 180→191 (11/0). | `experiments/e2-chateval/judge.py`, `docs/contracts/s7h-tinylm-v3.md` | lack (v2b weight 1.0, no δ; the 4.0 margin covers only corrections) | **adopt** a δ/τ gate on v2b |
| C11 | 10-14 / 10-22 | Strength levels (conservative / standard / aggressive) | Each level picked under its own break cap; aggressive demoted if it loses on guard sets. | Chat aggressive collapsed (lytune 236/374), so it equals standard. Formal aggressive: holdout 12/1. | `docs/contracts/rerank-strength.md` | lack | adapt: offer only levels that survive the gate |
| C12 | 10-14/15 | D2 split of reranker failures | Correct answer in the top 8 but missed: D2a gated off, D2b margin too small, D2c reranker wrong. | (i)/D2c = 0.275–0.342, so both fusion and model work remain candidates. | `experiments/e2-chateval/d2c_split.py`, `docs/triage.md` | lack | **adopt** before choosing fusion vs retraining |
| C13 | 10-17/18 | A point-estimate break cap doesn't transfer | Values picked under a 0.5% cap. | 0.50% became 1.72% (formal); 0.43% became 0.54% (XL); tune 0.33 → dev 0.58 break/fix. Fix: Poisson 95% UB, tune at 0.3%, select on the judging metric (fixes − 2×breaks). | `select_weighted`, `docs/methodology.md` 10-18/20 | have the tool (`stats.py` Poisson bound) | adopt the procedure |
| C14 | 10-13 / 10-18 | External 0c model (32.6M, reads zhuyin, 64-char context) as a yardstick | Scored on Shanjie's own top 8, sandboxed, hashes pinned, prefix-packed. | Chat +142/+127; Wikipedia −88; lytune −56. Pick-correct 66.5% vs 45.1%. P1 discordtune +105 (p 1.6e-12), but UB 0.61%. | `experiments/x0c/`, `docs/benchmark.md` (external section) | have better (v3 reads syllables) | we lead; adopt "external model on our N-best" as a ruler |
| C15 | 10-17/18 | Reading-conditioned small LMs | Causal reading input; reading-prefix format `[CTX][RD][OUT]`. | Causal: −71 (p 0.0002). Reading-prefix +32 vs control +91. | `train_r.py`, `experiments/e-b/` | have better (v3 bidirectional over all syllables) | we lead; their failure supports our design |
| C16 | 10-18 | E-F: learned logistic gate | Features: n-gram/LM gaps, rank, top-2 gap, softmax prob. | Tune +79 vs +63 same-rule; on dev lost to shipped (+10/−36, p 0.00016); UB 0.61%. | `experiments/e-f/` | lack | adapt: add string features (character class, sentence start, frequent multi-char word) |
| C17 | 10-18 | L0: listwise objective | Cross-entropy over 8 candidates with a "keep #1" option. | Collapsed: retention 98.5–99.4%, pick-correct 19.1/9.6%. discordtune −38. | `experiments/l0-listwise/` | lack | adapt with care: rebalance (81.6% of rows already right) |
| C18 | 10-18/19 | Distilling 0c into small students (P2, B, C, L, T, R) | KL over the top 8, reading input, without Wikipedia, L size, per-position KL, hard rows ×4. | discordtune −2, −2, −35, +7, −41, −4. Per-position KL improved while sentence ranking got worse. Line closed. | `experiments/p2*` | n/a | skip |
| C19 | 10-19 / 10-18 | Prefix sharing and a latency deadline | Context+reading KV computed once per row. pinyin-ime-reranker-4M reuses the prefix across keys (p95 20.64→8.61 ms). 0c falls back to lexicon order beyond 100 ms. The 4M model never overrides an exact whole-word lexicon hit. | 0c Rust p95 74.5→33.9 ms; RSS +273→+227.5 MiB. | `core/src/oc.rs`, `docs/iteration-plan.md` §8 | have (prefix tree within a key) | **adopt** cross-key state reuse and a hard deadline with fallback |
| C20 | 10-20 | D1: fusion frontier | 24 α×δ cells at τ 2. | No cell dominates shipped +107/−26. Fusion line closed. | `experiments/diag-near-miss/` | lack | adopt: plot our v2b weight's fix/break frontier |
| C21 | 10-11 / 10-18 | Per-pair fix/break and headroom diagnostics | Fix/break per confusion pair; pick-correct and retention on fixable rows. | 在→再 15 errors / 5 fixed / 7 caused (net loss). 她→他 1/182. M pick-correct 44.1%, retention 87.3%. | ad hoc, `judge.py` | partial | adopt |
| C22 | 10-19/20 | Long context for the reranker | Student trained with 63-char context (S64); shipped M fed 64 characters. | S64 −39 (p 0.0005). M@64: −1 / +7, not significant. Context length is now declared in the model header. | `experiments/p3-context/` | have (v3 255, v2b 24) | adopt the header field; their small model gained little from context |
| C23 | 10-18 → 10-21 | Learning guard on the reranker | Skip the rerank when the n-gram #1 holds a learned boost; 0c vetoes only spans the user picked. | The guard blocked 35 user-reported rows; turning it off: +1/0. | `core/src/engine.rs` (`boosted`) | verify (v2b vs recall) | adapt: protect only user-picked spans |

## D. Context and domain (chat/formal)

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| D1 | 10-03 | Chat vs formal by app | λ 0.5 for chat apps, 0.7 elsewhere. | discordtune +84 (p 3e-10), but cvtune −37 and wikitune −53. Discord: Shanjie 84.0 vs macOS 88.2. | — | have better (ratio adapted from preceding text) | adapt: use the frontmost app as a prior for c₀ |
| D2 | 10-12 | Profile UI with per-app override | Auto / always chat / always formal, plus a per-app override in the menu. | 14 tests, 9 mutations caught. | `docs/contracts/profile-choice.md` | lack (override) | adopt the override |
| D3 | 10-06 | Left context in the n-gram, and real-app reads | Context words as the first word's history. Real apps return markedRange `{1,0}` or `{NSNotFound,NSNotFound}`, so context was never read; the fake client hid it. | cvtune chat 277/22 (p 2.5e-57). | `docs/contracts/s2h-left-context.md`, `docs/methodology.md` §7 | have | **adopt** a numeric-only real-app context log check |
| D4 | 10-08 / 10-15 | Context across digits, English and punctuation | `<X>` token after digits or English; no reset at commas. | `<X>`: chat 36/18 (p 0.02) but the wikitune guard failed. Across punctuation: fail. | `experiments/derisk-1015/` | have (128-char read) | note: measure that ours doesn't hurt |
| D5 | 10-18 | E-1: offline vs app context | Ran each chat set with and without context. | Net −6..+5, not significant; lytune −22. Principle: build offline context exactly as the app sees it. | — | partial | adopt |
| D6 | R4 / 10-25 | Context-return rules | ≤256 UTF-16 units, cut at newline, ≤64 graphemes, terminals Han-only, marked range excluded, in memory only. Leaking uncommitted text dropped judge confidence 0.98→0.44. | `top1_sha256` identical; p95 +0.05–0.09 ms. | `docs/contracts/context-return.md` | have (128 chars) | adopt the marked-range and terminal checks |
| D7 | 10-25 | English-context next-Chinese table | English token (2–32 letters) → likely Chinese continuations. | Tooling only. | `core/src/english_next.rs` | lack | later |
| D8 | PLAN Q | Onboarding quiz | Choose domains; type 10–20 sentences to detect 和/一/neutral-tone and ㄣ/ㄥ habits; choose glyph preferences. | Not started. | PLAN | lack | adapt: seed typo-habit costs and fuzzy toggles |
| D9 | 10-20 | Idea backlog | Topic tracking boosting packs; "change only when two models agree"; preview before Enter; undo after commit; count AI edits. | Ideas only. | research-log 10-20 | partial | adopt the "count AI edits / undos" metric |

## E. Personal learning

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| E1 | 10-05 | Single-character pollution and the global layer | One 再 pick became global and broke every ㄗㄞˋ. Rules: singles never global; exact context only; never at `^`. ε sweep. Regression defined as "was exactly right, now not". | Pollution 24 regressions → 0. ε 0.1: 4/9 learned, 1 pollution; ε 2: 9/9, 10; ε 6: 28. Global layer off. | `core/tests/engine_learn.rs`, `docs/contracts/s4-learning.md` §12–13 | have (κ back-off), lack the tests | **adopt** pollution and mirror tests |
| E2 | 10-13/14 | Sentence-start learning (rules 7/8) | Learned 我試 at `^` scored −0.89 and beat 我是 (−3.89) everywhere. Now sentence-start entries need a matching next syllable; singles need ≥2 Han characters of context. | 4 failing repro tests pass. 64/273 of the maintainer's records dropped. p95 +0.14 ms. | `docs/contracts/learn-single-ctx.md`, `core/src/learn.rs` | verify (our recall/personal LM at the start of a composition) | **adopt** the test |
| E3 | 10-10 | s4-bayes: Dirichlet posterior | Replaced the fixed bonus with a posterior; 40 configurations. | 0 passed; mirror 0–3/8 vs 7/8. A one-shot pick cannot win when q≈1. | `docs/contracts/s4-bayes.md` | have (κ back-off) | adapt: a "taught once, wins next time" test for our κ=1 trigram |
| E4 | 10-05 | Write path for the learning store | Append ≤3 records with no fsync; full rewrite at 1,024 lines using F_BARRIERFSYNC. | Enter p95 16–19.7 → 0.17–0.20 ms. | `docs/contracts/s4-learning.md` §11 | verify | adopt |
| E5 | 10-10 / 10-17 | New word B, with sub-spans | Learn 2–4-char runs built from per-character repicks, plus sub-spans that have no lexicon word. Shadow rule −6.5 (skip if the lexicon already has a strong homophone). S ∈ [−3,−1]; 2,000 cap. | Probe 8→18/19 (p 0.002), CER 13.0→2.1%. cvtune chat 22/4; wikitune 99/6; 0 hijacks. | `docs/contracts/newword-b.md`, `core/tests/engine_newword.rs` | partial (phrase/span recall) | adopt sub-span learning and the shadow rule |
| E6 | 10-09 | Apple learns a new word after one repick | 落檔 learned once, and works after 記得. | Target behaviour. | `tools/imeshot.swift` | have (recall) | test the same flow |
| E7 | 10-17 / 10-18 | Learning UX | Prediction picks are learned; ⌘⌫ in the candidate window forgets; apps without a bundle ID pause learning; clearing asks for confirmation. | Shipped. | `macos/.../Learning.swift` | have (do-not-learn list) | adopt ⌘⌫ quick-forget |
| E8 | R3/R5 | Privacy gates for learning | Secure input or a denylist stops learning and context reads; file is 0600 and stores only (context, reading, word, weight, date); marker-string tests prove typed text isn't logged; Time Machine disclosed. | Shipped. | PLAN §7 | lack (ROADMAP P5) | **adopt** |
| E9 | 10-13 | Generalise learning by class or domain | Idea only. | — | PLAN | lack | watch |

## F. Prediction and completion

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| F1 | 10-07 | SP/V3: prediction from context plus partial zhuyin | Candidates are lexicon strings compatible with the keys so far. Ordering "S": successors of the context word first, then the rest. Tab enters the row. | Context lifts first-key hit@9 4.6→25.4%. Keystroke saving 42–47%. Successor-only 43.5% vs 64.4%; S 63.9%. +43 MB; p95 0.8→1.8 ms. | `core/src/predict.rs`, `docs/contracts/sp*.md` | lack (0.6.0 plan) | **adopt** the S ordering and hit@9 per stage |
| F2 | 10-09 | Start points up to 5 syllables back | Word starts up to 5 syllables back; mid-word starts last. | typing76 8/0; ACG names 57.9→71.7 (453/0). | `docs/contracts/v3-engine.md` §11 | lack | adopt |
| F3 | 10-09 | Successor ranking can bury words | λ·logP(w\|prev) + (1−λ)·lp put 繳 8th. | — | `core/src/predict.rs` | lack | adapt |
| F4 | 10-17 | Predict-down consistency | ↓ shows prediction items first; ⇥ glyph; a row item whose reading equals the typed syllables must count as a repick. | Bug found only after merging. | `docs/contracts/predict-down.md` | lack | adopt |
| F5 | 10-17 | Digit-context prediction | Counted which words follow digit strings (1.56 MB). | hit@9 after digits: chat 54.8→85.7%, formal 57.4→87.8%. | `data/lexicon/digits.tsv`, `docs/contracts/digit-context.md` | lack | adopt |

## G. Punctuation

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| G1 | 10-04 | Punctuation stays in composition until Enter | Users press Enter by habit in chat apps, which sends messages, so punctuation stays in composition. Punctuation acts as an LM sentence boundary. | Two mutations caught. | `docs/contracts/s3d-punctuation.md` | verify | adopt |
| G2 | v0.2.0 | Punctuation parity with Apple | Candidates named like Apple's; `[ ] \ \`` give punctuation without Shift; a tone key on an empty buffer types the tone mark; Ctrl+\ gives 、. Own table, Apple data not copied. | Shipped. | `docs/contracts/s3e-*.md`, `s3f-*.md` | partial (`Keyboard.swift` map) | adopt parity |
| G3 | 10-19 | CJK auto-spacing (盤古之白) | Insert a space between Han and Latin/digits on commit, looking at the character before the cursor too. Off by default. | 13 tests, 8 mutations caught. | `docs/contracts/cjk-spacing.md` | lack | adopt (cheap) |
| G4 | — | Punctuation prediction | Not attempted by Shanjie. | — | — | planned (v3 head, 0.5.0) | we lead in ambition |

## H. Evaluation methodology and statistics

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| H1 | 10-03/08 | Paired per-row statistics | `--rowstats`; exact McNemar; CER; paired bootstrap CIs. Resolution table (dev302 needs 2.3 pp). | Standard since 10-08. | `tools/evalstats.py` | have (`training/eval/stats.py`, `rows_eval.py`) | we have it |
| H2 | 10-03 | Lenient variants (MOE 也作/亦作) | 2,072 same-reading variants plus 8 folds (她→他 etc.), fixed from the dictionary, not chosen from eval errors. | Shanjie 83.4 / macOS 88.2 / McBopomofo 76.6 / 自然 72.2 (Discord 845). | `eval/variants.tsv` | have (`--shanjie-variants`) | have; beware it hid a 它/他 error |
| H3 | 10-03 | Head-to-head vs other IMEs | Typed the same keys into Apple, McBopomofo and 自然. Apple learned 847 of its 2,647 learned phrases from the tests themselves. | macOS built-in was best. | `tools/imetype.swift`, `docs/typing-test.md` | lack | adopt: clean VM account each minor release |
| H4 | 10-05 | Challenge sets mislead | dev302 (LLM- and project-written) vs real chat. | Apple judge dev302 +27 vs discordtune −81; Qwen3 +35 vs −17. | `docs/methodology.md` §2 | have (dev/test) | adopt the rule: decide on real-register sets |
| H5 | 10-12/13 / 10-18 | E2: private chat evaluation | Clauses from the maintainer's own messages: public channels only, months before using Shanjie, two kinds of context, answers proofread blind and hash-frozen, tuning months frozen before the judge month. PII drop rules (email, phone, 6+ digits, ID, token) applied after stripping markup. 10% audit with a stop at ≥1% corrections. One judge set per decision. | Tuning set 5,761 rows; judge month 6,559–6,843; chatjudge-2 5,783 rows (audit 0.34%). | `docs/contracts/e2-chat-eval.md`, `experiments/e2-chateval/build_chatclause.py` | lack | **adopt** (top action) |
| H6 | 10-12 | Public chat guard sets | lytune: Legislative Yuan speech, 4,811 clauses (Apache-2.0). wikitypo: 155 typo pairs. | R1 broke 10.08% of lytune while looking fine on tuning sets. | `build_lytune.py`, `build_wikitypo.py` | lack | adopt lytune |
| H7 | 10-10 / 10-18 | Pre-registration and two tracks | Contract committed before measuring. Exploration vs confirmation tracks. Holm across arms. MDE first. ≥3 seeds. Cluster bootstrap. Non-inferiority guard (90% lower bound ≥ −1.0 pp). "Failed as registered" is not "closed". | MDE example: 61 discordant rows → +23 at α 0.05. | `docs/methodology.md` 10-18, `docs/iteration-plan.md` | partial (`stats.py` MDE and clusters) | adopt the process |
| H8 | 10-15/16 | Holdout as a consumable | 3 disjoint 300-row shards (news plus talk). Retire after 8 uses or one decision. Fresh verifier only, before a tag. | Noise across shards: up to 13 rows (about 4%). The old holdout had been used ≥7 times. | `docs/contracts/holdout-refresh.md`, `eval/holdout/` | partial (test run once, single set) | adopt shards and a use counter |
| H9 | 10-15 / 10-18 | Contamination guard after normalisation | Containment on Han runs after s2t and variant folding; the counter skips hits. | 508 of 7,460 pool rows hit. cvtune 13.3% and wikitune 22.9% verbatim in training; clean versions 3,286/2,181. | `tools/holdout_guard.py` | have (8-char overlap exclusion) | adopt: check after normalisation |
| H10 | 10-13 / 10-18 | Cross-version benchmark and KPI table | Each release measured with its own shipped models; set hashes; tag commit bound by empty `git diff`; KPI row includes error ratio, KySS, rewrites and holdout halves. The rule existed and was skipped for weeks, so it was moved into tooling. | KPI v0.4.0→v0.5.0: discordtune 84.89→87.90%; wikitune-clean 70.38→75.29%. | `tools/bench.py`, `tools/kpi.py`, `docs/kpi.md`, `docs/contracts/sbench-v2.md` | partial | **adopt** as a release gate |
| H11 | 10-14 / 10-18 | Error-bucket triage | Buckets: A1 reading missing; B1 diff ≥2 chars; B2 not in list; C1–C5 mis-ranked; D1 rerank broke; D2a/b/c; E1/E2 context; F offline-right. Blind check on 60 rows. Fix families ranked by rows × rescue ÷ cost. | 60/60 agreement. C = 89–93% of errors. Top fix family: stronger small model (1,659 rows). | `tools/triage.py`, `docs/triage.md`, `docs/contracts/triage.md` | partial (`diagnose.py`) | **adopt** |
| H12 | 10-04 → 10-26 | User-reported regression set | 164 real errors with context as the app saw it. Used as clues, never as training data; rows used as clues are not scored. Reading check: the wrong on-screen string must be spellable from the row's reading. Record app, profile and version. | Chat top-1 stuck at 29 while n grew 110→135. F share by version 41→27→11%. | `eval/dev/user-reported.txt`, `docs/methodology.md` 10-13/10-21 | lack | **adopt** |
| H13 | 10-18 | Real-engine replay with KySS | Key-by-key replay: first display correct, committed correct, KySS, repicks/100, rewrites after display, learning chain. | dev302 chat KySS 0.9739. Rerank on every key vs only at commit: 110 vs 17 rewrites, identical commits. | `docs/contracts/engine-replay.md`, `shanjie-replay` | partial (`typing_errors.py`) | **adopt** KySS and flicker counts |
| H14 | 10-18 / 10-21 | Human-typed readings | 200 sentences typed by the maintainer vs `readings.py` (2.72% divergence). Reading-sensitive features need human readings: P5 penalised 4/327 gold characters. | — | `tools/readings.py` | lack (our eval readings come from longest match) | adopt |
| H15 | throughout | Mutation tests, golden parity, fingerprints | Every test must fail under a mutation (exit 101). Python reference byte-exact. `top1_sha256`. Cross-platform: identical candidates or ≤1e-9. Same CPU for threshold selection. | 0 panics in 50,859 fuzzed headers. | `cli/tests/golden.rs`, `docs/verification.md` | partial (`check_onnx`, Mac vs remote 0.17%) | adopt `top1_sha256` and mutations on decoder and KeyChannel |
| H16 | S2r | Probe sets | Same rows with the alternative legal reading (base tone ↔ sandhi). | Used as gates for sandhi work. | `experiments/s2/build_probe.py`, `eval/probe/` | partial (`error_analysis` groups) | adopt |
| H17 | 10-13 / 10-17 | Simulation and ablation pitfalls | All-tone-1 and empty-context ablations are out-of-distribution. The simulator repaired its own model, not the screen. Offline digit context used gold text (a row-196 miss in real replay). | Lessons recorded. | `docs/methodology.md` | n/a | adopt: in-distribution ablations; simulate what the screen shows |
| H18 | 10-13 | Workflow tiers | Typo rows: golden test only (6.8 s). Docs: one review. Code: touched area locally, the rest in CI. | Full `make test` 368 s. | Makefile | n/a | adapt |

## I. Latency and performance

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| I1 | 10-03 | Compact lexicon as a pure refactor | Byte-identical outputs, guard tests first. | 392→88 MB; p95 1.65 ms. | `docs/contracts/l.md` | have better (mmap 29 MB) | we lead |
| I2 | 10-11 | Inference backend survey | Rust + Accelerate `cblas_sgemm` + vForce vs MLX vs Core ML; GPU bursts. | GPU ~2× slower on short bursts; p95 ≈ 2× p50 after idle; M p50/p95 13.6/35.0 ms at typing rhythm. | research log 10-11 | have (CPU Core ML) | adopt the typing-rhythm benchmark (150 ms gaps) |
| I3 | 10-11 | Async two-stage rerank | Key path builds a job; a `.utility` worker computes; main thread applies. A generation counter discards stale jobs. | Key path +0.16 ms (p95 2.65). Rerank p95 8.3 ms. Rewrites 14.5%→9.3%; flip-back 2.15%. | `docs/contracts/s7c2a-core-rerank.md`, `s7c2b-shell-rerank.md` | lack (synchronous; 25-syllable p95 33 ms vs 30 target) | **adopt** |
| I4 | 10-09 → 10-14 | Timing hygiene | Removed per-record string allocation (p95 6.9→2.9 ms). Record load average; compare only within one session; label max vs p95. | A "p95 308 ms" turned out to be the max. | research log | partial (`latency.py`) | adopt |
| I5 | 10-15 | LM compression | varint lossless; 8-bit and 4-bit counts. | 83.4→28.6 MB lossless. 4-bit lost significantly (40/69). | `experiments/derisk-1015/` | have (mmap) | adapt varint for download size only |
| I6 | 10-08 / 10-14 | Load path and determinism | SHA check of the model on every load cost 0.4–0.6 s (moved out). Bit flip from unstable XMP RAM (2^45 allocation). `classes.sjc` order-dependent. | Byte-identical rebuilds on 3 machines. | `docs/contracts/model-v6.md` | verify | adopt deterministic, hash-anchored builds |
| I7 | 10-22 | Composition cap (see B21) | — | — | — | — | — |

## J. UX and settings

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| J1 | 10-08 → 10-17 | Repick-freeze | After a repick, other words stay. Deleting inside a word splits it into frozen characters. A prediction pick also freezes the left side. | Rev 4 regressed 1500→1438/2165 and was paused. Rev 5 simulated flow: chat 458→590 (+151/−19, p 1e-26); all rows +220/−30. | `docs/contracts/repick-freeze.md`, `core/tests/engine_repick.rs` | verify | **adopt**, gated on a simulated delete-retype-repick replay |
| J2 | 10-09 | Enter, Shift+Enter, Esc parity | Enter commits pending bopomofo as shown; Shift+Enter commits then inserts a newline; Esc only closes the candidate window. | Fixed after Discord complaints. | `docs/contracts/enter-pending.md` | verify | adopt |
| J3 | 10-05 → 10-10 | Candidate window parity | Pixel measurement against Apple (`imeshot`). Follows the client's appearance via `viewEffectiveAppearance`. Vertical layout: 9 rows, 28 pt pitch, width only grows. | Matches Apple row for row. | `tools/imeshot.swift`, `docs/contracts/candidate-vertical.md` | verify | adapt |
| J4 | 10-10 / 10-15 | Settings window | In-process window (prefPane blank since 10.15), Edit menu for paste, sidebar clamped to `visibleFrame`. | — | `docs/contracts/settings-*.md` | have | adapt if it grows |
| J5 | 10-05 / 10-20 | Secure input greys out IMEs | IMEs in `~/Library` grey out during secure input; `/Library` installs keep working. The menu names the app holding secure input. | Shipped as an install choice in v0.5.0. | `docs/contracts/system-install.md`, `tools/secure-input.swift` | have (pkg installs to `/Library`) | we already avoid it; add the "who holds secure input" hint |
| J6 | 10-14 → 10-26 | Chinese–English mixing | Measured demand first: 22.8% of messages contain English. SymSpell ∩ SCOWL (82,730 words); space-triggered window; Shift+letter English run; key-order English toggle (from vChewing). | Recall 99.48%; false triggers 0 per 1,000 syllables. Default on in v0.5.0. | `docs/contracts/zhen-*.md`, `english-sim` | lack | adopt; gate key-order evidence with timing, since our decoder repairs key order |
| J7 | 10-17 | Digits on the number row | Count table plus a θ=3 bias decides digit vs zhuyin, decided late; tables built at load. | 90.8% vs 87.0% for the rule; lazy build showed a 367 ms first key. | `core/src/digits.rs`, `docs/contracts/digits.md` | lack | adopt |
| J8 | 10-26 | Rare words above common characters | 変移 and 辨疑 shown ahead of 變. | 1,203 Japanese forms in the overlay. | — | verify | adopt the filter |
| J9 | 10-22 | Demo and self-test method | Old and new builds typing the same keys side by side. A bare binary has no bundle ID, so context and learning were silently off. | — | `tools/imeshot.swift` | lack | adopt the check |

## K. Release, CI and updates

| # | Date | Name | What they did | Result | Files | ZT status | Verdict |
|---|---|---|---|---|---|---|---|
| K1 | 10-04 / 10-08 | Homebrew cask pitfalls | `postflight` runs sandboxed; `on_upgrade` read as a string; fixed with `terminate_process`. | Old process gone in 0.03 s. | cask, `docs/contracts/s3b.md` §14–15 | n/a | skip |
| K2 | 10-09 / 10-21 | TIS registration on update | Re-registering after the bundle is replaced breaks Caps Lock switching. `TISEnableInputSource` returns noErr without effect. The TIS list is stale in-process (2 min vs 20 s in a fresh subprocess). | Fixed by skipping registration when already enabled. | `docs/contracts/installer-v2.md` §9, `system-install.md` | verify (our hot-reload relaunch) | **adopt**: never re-register an enabled source; query TIS from a fresh process |
| K3 | 10-08 / 10-16 | Release notes and model naming | Notes in the repo, with a CI gate if missing. `--latest` only for app releases. SJ-XX-version-scale names. `data/models.tsv` feeds the About page model list. | Shipped. | `docs/releases/README.md`, `.github/workflows/release.yml` | lack | adopt |
| K4 | 10-13 → 10-22 | Signing, notarisation, Sparkle | Developer ID only in the CI `release` environment. Verify codesign, spctl, stapler, appcast length and SHA. `umask 077` makes 0700 bundles, so `tar -xpf` and fix permissions after stapling. Sparkle: EdDSA-only trust, no blocking modal, floating window, ≥2.10 (advisories). | Shipped. | `docs/contracts/about-update.md`, `release.yml` | lack (ad-hoc; ROADMAP §4.12) | **adopt** |
| K5 | 10-09 → 10-20 | App sandbox and standalone updater | Container migration moves data (selftest IDs would steal it). launchd caches XPC by path, so they removed XPC entirely. | v0.5.0. | `docs/contracts/app-sandbox.md`, `standalone-updater.md` | lack | skip for now; read before sandboxing |
| K6 | 10-19 / 10-22 | VM upgrade matrix | Test from the real old configuration, without rebooting in between (scenarios a–g). | Caught the XPC cache bug. | `docs/methodology.md` 10-19 | lack | adopt |
| K7 | 10-17 / 10-22 | CI speed | Split into 5 jobs; Python on Linux; model tests on macOS (Linux has no Accelerate, 2.4× slower). | 904→476–640 s; 822→522 s. | `docs/contracts/ci-speed*.md` | verify | adapt |
| K8 | 10-12 | Model integrity | SHA-256 manifest checked at build, CI and release. Versioned, length-checked, fuzzed headers. | 0 panics in 50,859 cases. | `data/*.sha256`, `core/src/tinylm.rs` | partial (`manifest.json`) | adopt fuzzing of our binary headers |
| K9 | 10-09 / 10-14 | Privacy hygiene in the repo | An agent's User-Agent leaked the maintainer's email (about 40 requests). CLAUDE.md purged from history. 1 req/s, `maxlag=5`. | — | `LICENSES/data.md` | verify (our docs contain remote host details) | adopt |

## L. Failures and process lessons worth avoiding

| # | Date | Lesson | Evidence | Verdict |
|---|---|---|---|---|
| L1 | 10-08 | Check for a vocabulary gap before building UI workarounds | Repick-freeze took four rounds; the real cause was the missing word 版控. | adopt |
| L2 | 10-20 | A miss "as registered" is not the end of a line | An agent closed the colloquial line; the maintainer reopened it and P4d passed (+69 confirmed). 4 of 35 old closures were reopened. | adopt |
| L3 | 10-13 | A rule nobody follows is not a gate | The cross-version bench was required "before each release" and skipped for weeks. | adopt: put gates in CI |
| L4 | 10-10 | Pipeline traps | Locale `sort -u` merged readings (78→19). A shared counting stage lost downstream filters. Wrong model file in eval (v4 vs v5), so `shasum -c` first. The CLI's compile-time data path made 3 parallel cells identical. zsh does not word-split variables. | adopt |
| L5 | 10-09 | Don't dismiss repeated user reports | "Couldn't reproduce N times" was wrong about Caps Lock; single-variable bisection with the user confirmed it. | adopt |
| L6 | 10-21 | Advisor wins on small public sets aren't evidence | The P5 "mc" variant looked clean on small sets but broke 1–6 rows per cell when registered. | adopt |
| L7 | S3a review | Zeroising buffers gives false comfort | AppKit and the client app hold the same text; shortening retention is what helps. | adopt the reasoning |

---

## Top 15 actions for ZhuyinType

Ranked by expected impact on whole-sentence accuracy first, then user experience, then engineering quality.

1. **Build a private real-chat eval set from your own sent messages.** Use months before you used ZhuyinType. Proofread answers blind and freeze them by hash. Drop PII after stripping markup. Use the earlier months for tuning and one later month per decision. Record counts only.
   - Why: every Shanjie gain on Wikipedia, read-speech or LLM sets failed to transfer to chat (S2, S5j, S7b, S7f). Our chat LM's perplexity is worse than the Wikipedia LM's, and our dev sets have the same weakness.
   - Shanjie: `docs/contracts/e2-chat-eval.md`, `experiments/e2-chateval/build_chatclause.py`, `judge.py`, `docs/iteration-plan.md` §2.
2. **Replace our weak, generated chat corpus with open colloquial text, and score it as its own term.** Sources: WildChat-TW, YODAS, Tatoeba, MASSIVE, OPUS zh_TW subtitles (42.8M Han chars), g0v. Clean with the narrow simplified filter and two-pass LLM review. Add a colloquial bigram anchored on the lattice top-1 with a soft gate, rather than mixing it before decoding.
   - Evidence: P4d confirmed +69 net (p 1.1e-8); applying the same table before fusion broke 92 rows.
   - Shanjie: `experiments/p2-data/`, `experiments/sj-lm-7/clean_subs.py`, `review_subs.py`, `core/src/colloquial.rs`, `docs/contracts/colloquial-term.md`.
3. **Gate the v2b reranker with τ (only when the lattice is unsure) and δ (a switch margin).** Pick weight, τ and δ per profile by fixes − 2×breaks, with the Poisson 95% upper bound on breaks ≤ 0.5% (tune at 0.3%). Plot the fix/break frontier first. Optionally offer strength levels.
   - Evidence: R1 vs R3′ went from a 2.16% to a 0.24% break rate. Our `stats.py` already computes the bound.
   - Shanjie: `docs/contracts/s7e-fewer-breaks.md`, `rerank-strength.md`, `experiments/e2-chateval/judge.py`, `experiments/diag-near-miss/`.
4. **Run cheap lexicon and LM audits.**
   - "Does each word win when typed alone?" (26.5% of their words lost).
   - Expected-count segmentation (c(好,吧) 0→9; chat +15).
   - Neutral-tone check set (+100/0).
   - Overlay frequency cap.
   - Shinjitai, Cantonese and simplified-filter checks.
   - OpenCC on already-Traditional text.
   - Shanjie: `experiments/acg-audit/`, `tools/build_lm.py` / `build_counts_text.py --expected`, `docs/contracts/neutral-tolerance.md`, `experiments/s2/build_counts.py`, `docs/contracts/canto-prune.md`.
5. **Add personal-learning pollution and mirror tests, plus the context rules they motivate.**
   - Sentence-start entries need a matching next syllable; single characters need ≥2 Han characters of context; no global singles.
   - Add sub-span new-word learning with the shadow rule.
   - Test "taught once, wins next time" for our κ back-off.
   - Shanjie: `docs/contracts/s4-learning.md` §12–13, `learn-single-ctx.md`, `newword-b.md`, `s4-bayes.md`, `core/tests/engine_learn.rs`, `engine_newword.rs`.
6. **Keep the other words fixed after a repick, including picks from a future prediction row.** Validate with a simulated delete-retype-repick replay.
   - Evidence: +151/−19 on dev302 chat. An earlier version regressed, which is why the replay gate matters.
   - Shanjie: `docs/contracts/repick-freeze.md` §11–13, `core/tests/engine_repick.rs`.
7. **Error triage plus a real-error regression set.**
   - Bucket errors: not in the list / mis-ranked / reranker broke it / reranker right but gated.
   - Keep user-reported errors with the context the app actually saw, never trained on.
   - Add a small human-typed reading set.
   - Shanjie: `tools/triage.py`, `docs/contracts/triage.md`, `docs/triage.md`, `eval/dev/user-reported.txt`, `tools/readings.py`, `docs/methodology.md` (10-13, 10-21).
8. **Make reranking asynchronous with a generation counter, and apply it only at commit (or when idle).** This takes v2b and v3 off the key path; our 25-syllable p95 is 33 ms against a 30 ms target.
   - Evidence: their key path +0.16 ms; reranking at commit kept identical commits with 17 rewrites instead of 110.
   - Shanjie: `docs/contracts/s7c2a-core-rerank.md`, `s7c2b-shell-rerank.md`, `core/src/engine.rs` (`RerankJob`).
9. **Extend the replay harness: KySS, rewrites after display, repicks per 100 characters, a learning chain.** Put them in a per-release KPI row next to accuracy.
   - Shanjie: `docs/contracts/engine-replay.md`, `cli` (`shanjie-replay`), `tools/kpi.py`, `docs/kpi.md`.
10. **Add a names layer and an IT/AI pack with the collision rule.**
    - Sources: NSI brands, Wikidata, school lists, NAER terms.
    - A new word may never take an existing word's top-1, and needs corpus usage above its homophone.
    - Add only words the decoder currently gets wrong.
    - Check with the full shipped pipeline (reranker on).
    - Shanjie: `tools/build_names.py`, `docs/contracts/names-lexicon.md`, `names-creators.md`, `it-pack.md`, `net-slang-2.md`, `acg-pack.md`.
11. **Build the prediction row (Tab completion) using their measured design.**
    - Successors first, then compatible strings; start points up to 5 syllables back; digit-context table.
    - ↓ consistent with the row; an exact-reading pick counts as a repick.
    - Measure hit@9 per typing stage and keystroke saving.
    - Shanjie: `core/src/predict.rs`, `docs/contracts/sp3-successor-first.md`, `v3-engine.md` §11, `predict-down.md`, `digit-context.md`.
12. **Treat the test set as a consumable resource.**
    - Several disjoint shards (their noise floor was 4%), a use counter, retirement after 8 uses or one decision.
    - Contamination checked after s2t normalisation.
    - Cross-version bench bound to the tag commit, enforced in CI.
    - Shanjie: `docs/contracts/holdout-refresh.md`, `tools/holdout_guard.py`, `tools/bench.py`, `docs/contracts/sbench-v2.md`, `docs/verification.md`.
13. **Shrink v3/v2b with per-output-channel int8, keeping the embedding in fp32.** Accept only with a paired McNemar non-inferiority test and top-1 agreement against fp32. Add cross-key reuse of context state and a hard deadline with lattice fallback.
    - Evidence: their per-row int8 failed on 0c; per-column passed 300/300. M went 57.7→14.7 MB.
    - Shanjie: `experiments/s7a-tinylm/int8check.py`, `tools/convert_oc.py`, `core/src/oc.rs`, `docs/contracts/oc-experimental.md`.
14. **Privacy and the secure-input path.**
    - Pause learning and context reads during secure input or for denylisted fields; show the paused state.
    - Add marker-string tests so typed text never reaches logs or files.
    - Check context reads in real apps with numeric-only logs (the markedRange bug).
    - Shanjie: PLAN §7 (R1–R10), `docs/methodology.md` §7, `docs/contracts/system-install.md`, `tools/secure-input.swift`.
15. **Release engineering, and later English mixing.**
    - Release:
      - Developer ID signing and notarisation in a CI release environment.
      - Release notes in the repo as a gate; a model list in About.
      - Never re-register an already-enabled TIS source (Caps Lock bug).
      - VM upgrade tests from the real old configuration.
    - English mixing: measure demand first, then a space-triggered English window with a simulated false-trigger gate (0 per 1,000 syllables). CJK auto-spacing and digits on the number row are cheap extras.
    - Shanjie: `.github/workflows/release.yml`, `docs/contracts/about-update.md`, `installer-v2.md` §9, `release-notes.md`, `zhen-mixed.md`, `zhen-inline.md`, `digits.md`, `cjk-spacing.md`.

**Where we already lead:**
- Key-level timed decoding and KeyChannel (Shanjie has no adjacent-key or ordering repair; adjacent-key fuzziness broke 5.3% of clean input for them).
- A reading-aware v3 inside the lattice (their causal and reading-prefix models failed; 0c's similar design won on chat).
- 128/255-character context.
- mmap memory (29 MB resident).
- A larger reranker.
- Decoder recombination by LM history, already correct.
- Paired McNemar, Poisson bound and MDE tooling.
- `/Library` install, so secure input does not grey us out.

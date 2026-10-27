# ml/src/zhuyintype_ml/eval：評測

用 `zhuyin-cli` 逐鍵打字，量轉換、選字與打字錯誤的結果，並做成對比較與每版的 ZhuyinBench。

## 用途

- 在開發集上比較一項改變前後的成績，作為是否採用的依據（`zt eval stats`）。
- 每次發布後，在 GPU 主機上跑固定的整套評測，結果寫在 `docs/benchmarks/`。
- 在 Mac 上量 app 設定的每鍵延遲、能耗與記憶體。

方法、評測集與指標的定義見 [docs/ml/EVALUATION.md](../../../../docs/ml/EVALUATION.md)。

## 指令

```sh
zt eval sets                                         # 評測集與是否已下載
zt eval conversion --set colloquial-dev wiki-dev     # 整句轉換（app 設定）
zt eval conversion --config engine --cli-args "--typo off" --json data/build/eval/off.json
zt eval candidates | repick | errors | typo | typing-errors | rows | latency
zt eval corrections corrections.jsonl                # App 的改字記錄重播（每種改字）
zt eval stats base.json new.json                     # 兩份報告的成對比較
zt eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json      # 設定 × 評測集
zt bench run v0.5.0                                  # ZhuyinBench（GPU 主機）
zt bench history                                     # 重建 docs/benchmarks/ 的頁面
zt systems setup                                     # 取得並建置靈犀注音與善解（GPU 主機）
zt bench compare                                     # 和其他輸入法的比較（GPU 主機）；--render 產生頁面
zt --remote eval conversion --set dev                # 在 GPU 主機上執行
```

每個評測的選項見 `zt eval NAME --help`。

## 輸入與輸出

- 讀：資料目錄（預設 `data/build`）的詞庫、語言模型與神經模型；評測集（`ml/src/zhuyintype_ml/core/sets.py` 登錄的名稱，或任意檔案）；`ml/src/zhuyintype_ml/eval/sets/` 的外部評測集；`zhuyin-cli`（預設 `.build/release/zhuyin-cli`，`ZT_CLI` 可指定）。
- 寫：`--json FILE` 的報告；`zt eval sweep` 的報告預設在 `eval/sweeps/<矩陣>/`；`zt bench run` 的報告在 `bench/<版本>/`（以上相對於資料目錄），紀錄在 `docs/benchmarks/<版本>.json`，頁面在 `docs/benchmarks/`；`zt bench compare` 的每格結果在 `bench/compare/<label>/`，紀錄在 `docs/benchmarks/competition.json`。

## 檔案

| 檔案 | 說明 |
|---|---|
| `base.py` | 所有評測共用的介面 `Evaluator` 與共用選項 |
| `registry.py` | 評測的名稱登錄；`zt eval`、ZhuyinBench 與 sweep 都從這裡找評測 |
| `conversion.py` | 整句轉換：整句全對率、字準確率、寫法視為相同；前文與分兩次送出 |
| `candidates.py` | 候選字清單中正確答案的名次 |
| `repick.py` | 免選字：模擬使用者選字到全對需要幾次；`--relearn` 量選過的句子是否記住 |
| `errors.py` | 打字習慣（一不變調、輕聲、一聲不按空白、按錯）下的正確率與前 K 句命中 |
| `typo.py` | 依比例隨機按錯，各容錯模式的正確率與誤修正率 |
| `typing_errors.py` | 有按鍵時間的打字錯誤；方法與結果見 `TYPING_ERRORS.md` |
| `rows.py` | 善解格式的評測集：整句全對率、CER、前 16 句命中，三種正規化 |
| `corrections.py` | App 的改字記錄重播：每種改字，引擎現在不靠改字就打對、仍是改字前的字或其他 |
| `latency.py` | 每鍵延遲、CPU、能耗、記憶體（Mac） |
| `stats.py` | 成對比較：McNemar、bootstrap、改壞率上界、最小可偵測效果；`zt eval stats` |
| `sweep.py` | 設定 × 評測集的掃描；`zt eval sweep` |
| `matrices/` | sweep 的矩陣：`validation.json`（引擎、整句重排、v3）、`tree-validation.json`（樹狀重排模型） |
| `bench/suite.py` | ZhuyinBench 的固定內容，產生每版紀錄 |
| `bench/history.py` | 由紀錄產生 `docs/benchmarks/` 的每版頁面與歷代表 |
| `bench/compare.py` | 和其他輸入法的比較：tasks（集合 × 情境）、可接續的矩陣、紀錄 `docs/benchmarks/competition.json` |
| `bench/competition.py` | 由比較的紀錄產生 `docs/benchmarks/competition.md` |
| `systems/` | 比較的對象（ZhuyinType、靈犀注音、善解）共用的 `System` 介面與轉接器，見其中的 README |
| `fake_cli.py` | 測試用的假 `zhuyin-cli` |
| `test_*.py` | 單元測試（`python -m unittest discover -s ml/src/zhuyintype_ml/eval -t .`） |
| `colloquial_smoke.txt`、`latency_sentences.txt` | 日常短句 68 句、量測延遲的 10 句 |
| `sets/external/` | 外部評測集（善解，CC0），來源與授權見其中的 README |
| `TYPING_ERRORS.md` | 有按鍵時間的打字錯誤：模型、計數規則與結果 |

## 相關文件

- [docs/ml/EVALUATION.md](../../../../docs/ml/EVALUATION.md)：評測的原則、評測集、工具與 ZhuyinBench
- [docs/benchmarks/](../../../../docs/benchmarks/)：每版成績
- [docs/engine/MODEL.md](../../../../docs/engine/MODEL.md)：方法與數字
- [ml/src/zhuyintype_ml/remote/README.md](../remote/README.md)：GPU 主機上的建置與 ONNX 模型
- [ml/CONVENTIONS.md](../../../CONVENTIONS.md)：程式與文件慣例

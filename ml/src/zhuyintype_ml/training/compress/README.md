# compress：不重新訓練的模型壓縮

把 reranker（v2b）與 v3 的權重改用 int8、int4 或調色盤（palette）儲存，產生較小的 Core ML 與 ONNX 模型，架構與訓練都不變。任何一份 reranker 或 v3 檢查點都可以重跑。

## 用途

- App 的模型大小：v3 兩個 Core ML 模型 fp16 共 129 MB，reranker 每個 60 MB。0.4.3 起出貨的是這裡的 `int8-channel`（`data-5`），v3 65 MB、reranker 30 MB；`checkpoints` 元件是同樣的權重，主機上的評測量的就是出貨的模型。
- 每種壓縮方法先在 GPU 主機上以「假量化」評測：把權重壓縮再解壓縮，存成檢查點，匯出 fp32 ONNX，用 App 設定跑開發集。這些權重和 Core ML 解壓縮後的值逐位元相同（`parity` 檢查），所以主機上的數字就是這些權重的準確率。
- Mac 只做 Core ML 的匯出、壓縮、編譯，一次性的一致性檢查，以及延遲、能耗與載入時間的量測。
- 方法、數字與決定見 [docs/research/log.md](../../../../../docs/research/log.md) 2026-10-28「模型壓縮」與 [MODEL.md](../../../../../docs/engine/MODEL.md) §10、§11。

## 指令

```sh
zt --remote --nice 10 train compress                     # GPU 主機：test、quantize、onnx（之後的步驟只能在 macOS 執行）
zt --remote --backend cpu --export --nice 10 eval sweep ml/src/zhuyintype_ml/eval/matrices/compress.json
zt eval stats data/build/eval/sweeps/compress/current-colloquial.json data/build/eval/sweeps/compress/int8-channel-colloquial.json
zt remote pull data/build/compress/onnx.json data/build/compress/int8-channel/compression.json \
    data/build/compress/int8-channel/reference.json   # Mac：只取回紀錄，不取回檢查點
zt train compress quantize coreml parity measure --option methods=int8-channel   # Mac
zt train compress measure --option methods=int8-channel --option rounds=3 \
    --option "base=$HOME/Library/Input Methods/ZhuyinType.app/Contents/Resources"
ZT_DATA=data/build/compress/int8-channel zt data pack data-N v3 reranker checkpoints   # 資料版本（data-5）
zt --remote train compress --option v3=v3_new             # 新的 v3 檢查點（資料目錄裡的 v3_new/）
```

| 步驟 | 內容 | 執行位置 |
|---|---|---|
| `test` | 單元測試：線性量化和 coremltools 的算法逐位元相同、再量化一次不變、調色盤的值數 | 任一（`export` 相依群組） |
| `quantize` | 每種方法的假量化檢查點（fp16 儲存）與 `compression.json`；已有同一來源的紀錄時，每個矩陣的 SHA-256 必須相同 | 任一 |
| `onnx` | 每種方法的 ONNX、ONNX Runtime 的 int8 模型、主機上的參考輸出與單執行緒耗時 | GPU 主機 |
| `coreml` | 匯出、壓縮、編譯 Core ML，紀錄大小與載入時間 | 只限 macOS |
| `parity` | 壓縮後的 Core ML 對主機的參考輸出、對未壓縮的匯出；寫入該方法的 `v3/test_vectors.json` | 只限 macOS |
| `measure` | App 的引擎路徑（EngineBench）每鍵延遲、CPU 時間、能耗，和目前的模型交錯量 | 只限 macOS |

選項：`reranker=DIR`、`v3=DIR`（資料目錄裡的來源，預設 `reranker`、`v3`），`methods=a,b`（預設全部；Mac 的步驟需要指定），`rounds=N`、`base=DIR`（`measure`）。

Mac 上的 `quantize` 和主機上算出的權重逐位元相同（SHA-256 比對），所以 Mac 不需要取回每種方法 200 MB 的檢查點，只取回 `compression.json` 與 `reference.json`。

## 方法

`methods.py`。只壓縮 nn.Linear 與 nn.Embedding 中至少 65,536 個值的矩陣；layer norm、聲調等小嵌入、每字的偏差與旋轉位置表維持 fp16。一列是 PyTorch 權重的輸出通道（Linear）或一個字（Embedding），和 coremltools 對 linear、matmul、gather 選的軸相同。

| 方法 | 儲存 | 最低 macOS |
|---|---|---|
| `fp16` | 全部 fp16（目前出貨的格式） | 14 |
| `int8-channel` | 對稱 int8，每列一個 scale | 14 |
| `int8-block32` | 對稱 int8，每 32 欄一個 scale | 15 |
| `int4-block32` | 對稱 int4，每 32 欄一個 scale | 15 |
| `pal6`、`pal4` | 每個矩陣一張 64 或 16 個值的 k-means 表 | 14 |
| `pal6-group16`、`pal4-group16` | 每 16 列一張表（列數不能整除時改為每 16 欄） | 15 |
| `onnx-int8`、`onnx-int8-tensor` | ONNX Runtime 動態 int8（權重每通道或整個矩陣一個 scale，執行時量化輸入） | Linux |
| `onnx-int8-weights` | `int8-channel` 的權重以 int8 加 DequantizeLinear 儲存，fp32 計算 | Linux |

## 輸入與輸出

路徑相對於資料目錄（預設 `data/build/`）。

讀取：`reranker/charlm.pt`、`reranker/vocab.json`、`reranker/config.json`、`v3/syllable.pt`、`v3/context.pt`（只記錄雜湊值）、`v3/test_vectors.json` 的輸入，以及 v3 的 JSON 與授權檔。

寫入 `compress/`：

- `<方法>/reranker/`、`<方法>/v3/`：假量化檢查點（`charlm.pt`、`syllable.pt`）、引擎讀的 JSON、ONNX（`onnx`）、Core ML 的 `.mlpackage`、`.uncompressed.mlpackage` 與 `.mlmodelc`（`coreml`）、v3 的 `test_vectors.json`（`parity`）；目錄可直接給 `zhuyin-cli --reranker DIR --v3 DIR` 或 App 使用；
- `<方法>/compression.json`：每個矩陣的形狀、誤差、值數、SHA-256 與估計的 Core ML 大小；`<方法>/reference.json`：主機上 ONNX 的參考輸出與耗時；`<方法>/coreml.json`：對應的常數、大小與載入時間；`<方法>/latency-N.json`：延遲；`<方法>/app/`：`measure` 用的模型目錄（連結）；
- `onnx.json`、`coreml.json`、`parity-mac.json`：所有方法的 ONNX 大小、Core ML 大小與載入時間、一致性；
- `no-reranker/`：沒有 reranker 的模型目錄（連結），評測矩陣用。

## 檔案

| 檔案 | 說明 |
|---|---|
| `pipeline.py` | `zt train compress` 的步驟；`measure` 的模型目錄 |
| `methods.py` | 壓縮方法與輸出目錄（只用標準函式庫） |
| `quantize.py` | 假量化：線性量化（和 coremltools 相同的 fp16 算法）、一維 k-means 調色盤、紀錄與跨機器的雜湊比對 |
| `onnx_models.py` | 每種方法的 ONNX 匯出、ONNX Runtime 的 int8 模型、參考輸出與耗時、`no-reranker/` |
| `coreml.py` | Core ML 匯出、逐值對應常數後以 coremltools 壓縮、編譯、大小與載入時間（macOS） |
| `parity.py` | Mac 上的一致性檢查與壓縮模型的 golden vectors（macOS） |
| `test_compress.py` | 單元測試 |

評測矩陣在 `ml/src/zhuyintype_ml/eval/matrices/compress.json`。

## 相關文件

- [docs/research/log.md](../../../../../docs/research/log.md) 2026-10-28「模型壓縮」：做法、完整數字與決定
- [docs/engine/MODEL.md](../../../../../docs/engine/MODEL.md) §10、§11：採用與沒採用的做法
- [../v3/README.md](../v3/README.md)、[../reranker/README.md](../reranker/README.md)：被壓縮的模型與匯出程式
- [../../remote/README.md](../../remote/README.md)：主機上的 ONNX 評測

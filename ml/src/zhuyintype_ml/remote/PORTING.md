# 平台邊界與未來的輸入法

引擎是共用的 Rust workspace（`engine/`），不依賴任何前端；模型的推論接在 trait 後面（ONNX Runtime，或前端提供的回呼）。前端把原生按鍵事件與游標前的文字轉成 Composer 呼叫，提供模型回呼，再繪製 Presentation 並套用送出的文字。C ABI 已完成（`zhuyin-ffi`，見 `engine/DESIGN.md` §6），macOS App 經由它使用引擎；Linux 與 Windows 的輸入法還沒實作。

## 執行環境邊界與最佳化

| 平台 | 引擎／前端 | 模型執行環境與調校 |
|---|---|---|
| macOS | Rust 引擎經由 C ABI；InputMethodKit、原生 AppKit 介面 | 只用 Core ML，預設 CPU；保留 CPU／ANE 調校、有狀態的上下文模型、int8／palettized 權重，作為未來 macOS 專屬選項。Core ML 在 App（`ZhuyinEngine`）裡執行，由 Rust 引擎回呼。macOS 上不連結 ONNX Runtime。 |
| Linux（CLI 已實作） | 同一套 Rust 引擎；未來的 fcitx5 或 IBus 轉接層經由 C ABI | ONNX Runtime C API，fp32 CPU，每個 CLI worker 使用一個 intra-op／inter-op 執行緒。啟用圖最佳化、快取上下文 K/V，並在單次 reranker 呼叫內批次處理候選。fp16 另外測試；不要只為了檔案較小就取代參考模型。選用的 fp32 PyTorch 批次伺服器（`zt --remote --backend cuda`）已在完整評測矩陣上實測，Torch 配置上限 2 GiB，在這台主機上較快；它是評測用的最佳化，原生 Linux 推論仍為 ONNX CPU。未來的 ONNX CUDA／OpenVINO execution provider 同樣必須連同派送、啟動與資料載入一起做效能測試。 |
| Windows（規劃中） | 同一套 Rust 引擎；未來的 TSF 文字服務經由 C ABI | ONNX Runtime CPU 或 DirectML，各自調校。透過 C API 建立 session 時使用 UTF-16 路徑；執行環境 DLL 與 bridge 放在一起發佈。不使用 Core ML／IMK／AppKit。TSF COM apartment 的整合需要實際的 Windows 建置與測試。 |

精簡的模型呼叫介面是 `RerankerModel`（扁平的 int32 ids／targets、B、L -> 扁平的 logprob；前綴樹另有一個呼叫）與 `SyllableModel`（上下文 ids、音節／聲調／成本／邊界 ids 與候選列 -> 候選 logprob），C ABI 上是 `ZtRerankerModel`、`ZtSyllableModel` 回呼。分數混合、精確邊際偏好、24／255 字元的上下文上限、padding、未知字懲罰、候選截斷與排序都留在共用的 Rust 程式碼中（`zhuyin-models`）。模型執行環境只實作張量呼叫。v3 投影後的 K/V 會快取並直接傳入，不做第二次投影。v3 模型在提供的候選子集上正規化，而不是整個詞彙表。

隨附的 Core ML 套件使用 fp16 運算；ONNX 的 fp32 權重仍保留 v3 編碼器之間的 fp16 記憶介面。`*.fp16.onnx` 實驗模型的外部輸入／輸出型別完全相同。模型必須來自相配的一對檢查點。Linux 上模型載入錯誤為致命錯誤；共用計分器的推論失敗則沿用參考 API 的 nil／後備行為。發佈產出物前先執行一致性與冒煙檢查。

## C ABI

已實作為 `engine/crates/zhuyin-ffi`（標頭 `include/zhuyin.h`）：不透明 handle、明確的所有權與執行緒規則、UTF-8 字串與 UTF-16 游標，模型由前端以回呼提供。設計見 `engine/DESIGN.md` §6。

## 執行緒

Composer 與 Engine 含有可變的組字、學習、快取與領域狀態。同一個 handle 的所有呼叫須在前端的同一個事件佇列上依序進行；不要同時呼叫同一個 Composer。大型資料與模型在 UI／TSF 執行緒以外載入，完成後再發佈可用的 handle。模型 session 有自己的鎖；這些鎖不會讓 Composer 成為執行緒安全。Linux 評測基準使用彼此獨立的行程，並在每一行重設。未來共用的執行環境或伺服器可以重用不可變的權重，但批次處理必須完整保留上下文、雙向音節長度、padding 遮罩與候選正規化。

TSF 呼叫必須封送到所屬的 COM apartment；fcitx5／IBus 整合即使在背景載入，也必須保留同步的已處理／放行語意。時間單位與 UTF-8、UTF-16 索引須在文件中明確寫出。

## 資料檔案、授權與封裝

隨附 lexicon.tsv；lm 與選用的 lm_chat unigram／bigram／backoff 檔案；reranker 的 vocab／config 與該平台的模型；v3 的 chars／syllables／config 與相配的一對編碼器。保留模型與來源的 NOTICE、來源紀錄、授權與 Wikipedia 姓名標示清單。執行期推論需要的是匯出的模型與 JSON，不需要訓練檢查點或原始語料。GPU 主機從固定版本的資料（`zt data fetch checkpoints`）取得檢查點，並在 Linux 上匯出（`zt models export`）；Core ML 套件留在 Mac。

* macOS：維持目前的 .app／.pkg 套件、簽署與 InputMethodKit 生命週期。這項工作不更動封裝腳本。
* Linux：把引擎的共用函式庫（`libzhuyin.so`）、ONNX Runtime .so 與 notice 連同帶版本的資料目錄一起封裝；可發佈的套件使用可重新定位的載入路徑（評測建置使用 `ORT_LIB_PATH` 與絕對 rpath）。fcitx5／IBus 註冊與桌面整合留給未來的前端套件。引擎與模型產出物各自有檢查碼與版本。
* Windows：簽署過的 TSF DLL 與註冊／解除註冊、引擎的 `zhuyin.dll` 與 ONNX Runtime／DirectML DLL、UTF-16 路徑、學習狀態存放在各使用者的 AppData；須在實際應用程式中驗證升級、焦點與行程關閉的行為。`zhuyin-cli --model-server`（Unix socket）是實驗性的評測路徑，在 Windows 上必須排除或替換。

原生前端與 C ABI 的實作刻意延後。這裡不宣稱已有 Windows 建置或 Linux 桌面輸入整合。

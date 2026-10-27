# 平台邊界與未來的輸入法

ZhuyinCore 是共用的 Swift 引擎，只依賴 Foundation，不依賴任何模型執行環境或前端。前端把原生按鍵事件與游標前的文字轉成 Composer 呼叫，提供模型回呼，再繪製 Presentation 並套用送出的文字。CLI 是可執行的參考轉接層。這次變更沒有實作輸入法或 C ABI。

## 執行環境邊界與最佳化

| 平台 | 引擎／前端 | 模型執行環境與調校 |
|---|---|---|
| macOS（參考實作） | Swift；InputMethodKit、原生 AppKit 介面 | 只用 Core ML，預設 CPU；保留 CPU／ANE 調校、有狀態的上下文模型、int8／palettized 權重，作為未來 macOS 專屬選項。既有的 `ZHUYIN_UNITS` CLI 實驗仍可使用。macOS 上不連結任何 ONNX target。 |
| Linux（CLI 已實作） | 同一套 Swift 引擎；未來的 fcitx5 或 IBus 轉接層 | ONNX Runtime C API，fp32 CPU，每個 CLI worker 使用一個 intra-op／inter-op 執行緒。啟用圖最佳化、快取上下文 K/V，並在單次 reranker 呼叫內批次處理候選。fp16 另外測試；不要只為了檔案較小就取代參考模型。選用的 fp32 PyTorch 批次伺服器（`BACKEND=cuda`）已在完整評測矩陣上實測，Torch 配置上限 2 GiB，在這台主機上較快；它是評測用的最佳化，原生 Linux 推論仍為 ONNX CPU。未來的 ONNX CUDA／OpenVINO execution provider 同樣必須連同派送、啟動與資料載入一起做效能測試。 |
| Windows（規劃中） | 平台驗證後使用同一套 Swift 引擎；未來的 TSF 文字服務 | ONNX Runtime CPU 或 DirectML，各自調校。透過 C API 建立 session 時使用 UTF-16 路徑；執行環境 DLL 與 bridge 放在一起發佈。不使用 Core ML／IMK／AppKit。Swift 工具鏈與 Foundation 的可用性，以及 TSF COM apartment 的整合，需要實際的 Windows 建置與測試。 |

精簡的模型呼叫介面是 `NeuralReranker.Prediction`（扁平的 int32 ids／targets、B、L -> 扁平的 logprob）與 `SyllableScorer.Prediction`（上下文 ids、音節／聲調／成本／邊界 ids 與候選列 -> 候選 logprob）。分數混合、精確邊際偏好、24／255 字元的上下文上限、padding、未知字懲罰、候選截斷與排序都留在共用的 Swift 程式碼中。模型執行環境只實作張量呼叫。v3 投影後的 K/V 會快取並直接傳入，不做第二次投影。v3 模型在提供的候選子集上正規化，而不是整個詞彙表。

隨附的 Core ML 套件使用 fp16 運算；ONNX 的 fp32 權重仍保留 v3 編碼器之間的 fp16 記憶介面。`*.fp16.onnx` 實驗模型的外部輸入／輸出型別完全相同。模型必須來自相配的一對檢查點。Linux 上模型載入錯誤為致命錯誤；共用計分器的推論失敗則沿用參考 API 的 nil／後備行為。發佈產出物前先執行一致性與冒煙檢查。

## 建議的 C ABI（新 target，尚未建置）

新增獨立的 `ZhuyinC` target，依賴 ZhuyinCore 與該平台的模型模組。透過 `@_cdecl` 只匯出固定寬度的 C 型別、不透明 handle、明確的所有權與帶版本的函式；不暴露 Swift／Foundation 物件，也不依賴 Swift struct 的記憶體配置。

* `zt_engine_create(data_path_utf8, options_json, error_out)` / `zt_engine_destroy`：載入不可變的引擎與執行環境套件；每位使用者的學習目錄須明確啟用，預設不寫入，以便評測。
* `zt_composer_create(engine_handle)` / `zt_composer_destroy`：一個組字 session 與其上下文。須定義引擎是共用還是獨占；初期每個輸入 session 使用一個引擎，避免共用可變的學習與領域狀態。
* `zt_set_context(handle, utf8, byte_length)`、`zt_key(handle, key_code, modifiers, monotonic_seconds)`、`zt_reset(handle)`、`zt_break_context(handle)`：按鍵與焦點的生命週期，包含依時間的按鍵解碼。定義穩定的按鍵列舉，而不是使用平台的 key code。原生按鍵對應留在前端。
* `zt_presentation_json(handle)` / `zt_string_free`：回傳 UTF-8 的 presentation／commit／result 記錄，附明確的位元組長度、已處理／放行狀態、候選頁與選取、子句、通知，以及 UTF-16 游標位移。以輸入法的事件頻率而言，第一版用 JSON ABI 已足夠；經過量測後才加入二進位檢視。
* 候選選取、翻頁與編輯使用既有的 Composer 按鍵 API。學習排除／匯入／匯出等到前端真的需要時，再另外以帶版本的呼叫提供。
* 每個匯出函式都攔截 Swift 錯誤，回傳狀態碼與一個由呼叫端擁有的 UTF-8 錯誤字串；例外、trap 與 Swift 配置器都不跨越 ABI。須定義函式庫 ABI 版本、資料與模型格式版本，以及取消與關閉的順序。

## 執行緒

Composer 與 Engine 含有可變的組字、學習、快取與領域狀態。同一個 handle 的所有呼叫須在前端的同一個事件佇列上依序進行；不要同時呼叫同一個 Composer。大型資料與模型在 UI／TSF 執行緒以外載入，完成後再發佈可用的 handle。模型 session 有自己的鎖；這些鎖不會讓 Composer 成為執行緒安全。Linux 評測基準使用彼此獨立的行程，並在每一行重設。未來共用的執行環境或伺服器可以重用不可變的權重，但批次處理必須完整保留上下文、雙向音節長度、padding 遮罩與候選正規化。

TSF 呼叫必須封送到所屬的 COM apartment；fcitx5／IBus 整合即使在背景載入，也必須保留同步的已處理／放行語意。時間單位與 UTF-8、UTF-16 索引須在文件中明確寫出。

## 資料檔案、授權與封裝

隨附 lexicon.tsv；lm 與選用的 lm_chat unigram／bigram／backoff 檔案；reranker 的 vocab／config 與該平台的模型；v3 的 chars／syllables／config 與相配的一對編碼器。保留模型與來源的 NOTICE、來源紀錄、授權與 Wikipedia 姓名標示清單。執行期推論需要的是匯出的模型與 JSON，不需要訓練檢查點或原始語料。遠端 runner 傳送檢查點並在 Linux 上匯出；Core ML 套件留在 Mac。

* macOS：維持目前的 .app／.pkg 套件、簽署與 InputMethodKit 生命週期。這項工作不更動封裝腳本。
* Linux：把 Swift 執行環境函式庫、C bridge、ONNX Runtime .so 與 notice 連同帶版本的資料目錄一起封裝；可發佈的套件使用可重新定位的載入路徑（評測建置使用 `ONNXRUNTIME_ROOT` 與絕對 rpath）。fcitx5／IBus 註冊與桌面整合留給未來的前端套件。引擎與模型產出物各自有檢查碼與版本。
* Windows：簽署過的 TSF DLL 與註冊／解除註冊、相容的 Swift 執行環境與 ONNX Runtime／DirectML DLL、UTF-16 路徑、學習狀態存放在各使用者的 AppData；須在實際應用程式中驗證升級、焦點與行程關閉的行為。Unix socket 的 `ModelServer.swift` 是實驗性的評測路徑，在 Windows 上必須排除或替換。

原生前端與 C ABI 的實作刻意延後。這裡不宣稱已有 Windows 建置或 Linux 桌面輸入整合。

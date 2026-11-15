# ZhuyinType：Linux 與 Windows 版（GPUI）

Linux 與 Windows 的前端是一個 Rust 行程：用 [GPUI](https://www.gpui.rs/)（透過 `gpui-kit`）繪製設定視窗、候選字視窗、中英提示與設定嚮導，持有引擎，並接受原生輸入法介面的連線。它取代了 Qt 客戶端（2026-11 移除）。

**功能與版面和 macOS 版相同**：頁面、區塊、設定列、子頁面、嚮導步驟、候選字視窗的各種樣式與頁面之間的連結，順序與文字都一樣（`platforms/macos/Sources/ZhuyinTypeApp/UI/`）。視覺是 ZhuyinType 自己的一套現代設計語言（`crates/zhuyintype/src/theme.rs` 的 token），不模仿 macOS，也不模仿 Windows 原生外觀：暖色中性的表面、單色線條圖示、以細分隔線區隔設定列而不是填色卡片、強調色只用在少數地方，拉丁文字與數字用內嵌的 Inter，中文用系統字型。動態沿用 macOS 版的節奏（140 ms 翻頁、子頁面推入與返回）。macOS 版維持原生外觀，不受影響。視窗不使用系統邊框：標題列與視窗按鈕由 App 自己繪製，和 Discord、Claude 桌面版一樣。

## 目錄

```text
platforms/desktop/
  settings.json             所有頁面與嚮導共用的設定描述
  IPC.md                    服務與原生輸入法介面之間的通訊協定
  crates/zt-core/           不含介面：設定、路徑、引擎工作階段、輸入控制、快捷鍵、使用統計、改字記錄、
                            試打工作階段、通訊服務（Unix socket／named pipe）
  crates/zhuyintype/        GPUI App（執行檔 `zhuyintype`）：theme、ui/ 元件、pages/、candidate/、wizard/
  crates/zt-tsf/            只用於 Windows：TSF 文字服務 DLL 與登記，以 Rust（`windows` crate）撰寫
  packaging/windows/        NSIS 安裝程式（正體中文介面、一次 UAC、登記 TSF、登入啟動、解除安裝）
  packaging/linux/          nfpm 產生 .deb／.rpm、桌面項目、自動啟動、Fcitx5 外掛設定
  scripts/                  remote.sh（Mac → GPU 主機）、build.sh（主機：Linux、Windows 與安裝檔）、screenshot.sh
  tools/gpui-shaders/       在 Windows 上把 GPUI 的 HLSL 編成 bytecode（交叉編譯用）
  vendor/gpui-pre-windows/  GPUI 的 Windows 後端，改了 build.rs 以使用事先編譯的著色器（packaging/windows/README.md）
platforms/linux/            Fcitx5 外掛（C++，透過通訊服務連線；協定不變）
```

## 建置：只在 GPU 主機上

Mac 上不建置任何東西，不占用使用者的硬碟。在 Mac 上執行：

```sh
platforms/desktop/scripts/remote.sh cargo build                      # 在主機上建置 Linux 版（debug）
platforms/desktop/scripts/remote.sh cargo test -p zt-core
platforms/desktop/scripts/remote.sh scripts/screenshot.sh ~/zhuyintype/shots/NAME   # 每一頁的淺色與深色截圖
platforms/desktop/scripts/remote.sh --fetch zhuyintype/shots/NAME /tmp/shots       # 把截圖取回來看
```

Windows 執行檔在主機上以 `cargo xwin build --target x86_64-pc-windows-msvc` 交叉編譯（MSVC ABI、clang-cl／lld，Windows SDK 由 xwin 下載；32 位元程式用的 TSF DLL 以 `i686-pc-windows-msvc`），C runtime 靜態連結；安裝程式在 Linux 上用 `makensis` 產生（`packaging/windows/build.sh`）。Windows 上的行為在 Proxmox 的測試 VM（ssh 別名 `zt-windows`，Windows 11 正體中文）上驗證。

## 執行緒與核心／介面的分界

```text
 Fcitx5 外掛／TSF DLL ──IPC.md──▶ 通訊執行緒（zt-core）──▶ InputController ──▶ EngineSession（zhuyin-core）
                                        │ UiEvent（非同步 channel）
                                        ▼
                              GPUI 主執行緒：候選字視窗、中英提示、設定視窗
```

- 通訊服務在自己的執行緒上執行，從不等待 GPUI 執行緒，所以 App 自己的文字欄位也能使用輸入法（同一個行程裡的 TSF DLL 連到自己的服務）而不會互相卡住。
- `zt_core::service::Service` 把共用狀態（`Settings`、`EngineSession`、`InputController`、`Shortcuts`、`UsageStats`、`CorrectionLog`）放在 `Arc`／`Mutex` 後面，複製的成本很低。介面直接呼叫它處理設定、個人資料、統計與候選字視窗上的點選（`Service::popup_action`）。
- `Service::start` 回傳 `async_channel::Receiver<UiEvent>`，介面在 GPUI 的前景 executor 上讀取：
  - `UiEvent::Popup(Option<PopupState>)`：顯示、更新或隱藏候選字視窗（`PopupState` 是畫面資料、以螢幕邏輯像素表示的游標矩形，以及應用程式是否自己顯示組字）；
  - `UiEvent::ModeToast { english, anchor }`、`UiEvent::ShowSettings`、`UiEvent::Boundary(direction)`。
- `zt_core::presentation::Presentation` 是所有畫面共用的一份資料：組字、注音、UTF-16 游標、分段、目前這一頁的候選字、選取、頁碼、單字與修正清單、提示。浮動的候選字視窗、「視窗與提示」的預覽、「介紹」的試打與嚮導都用同一個候選字元件畫它。
- `PracticeSession` 是行程內的沙盒（自己的 `InputController`、只在記憶體中的個人資料、共用的唯讀引擎資料），用於「介紹」的試打欄與嚮導的練習，和 macOS 的 `PracticeField` 相同。
- 按鍵以 `zt_core::keys::Key`（Qt 按鍵碼、Qt 修飾鍵旗標、文字、左右 Shift）跨過分界，和通訊協定使用的數值相同；介面把 GPUI 的按鍵轉成它。

## 引擎資料與使用者檔案

引擎（`zhuyin-core`，直接呼叫）從下列位置中第一個有詞庫的目錄載入：`ZT_DATA_DIR`、設定的資料目錄、`<執行檔目錄>/data`（Windows）或 `/usr/share/zhuyintype/data`（Linux），以及版本庫的 `data/build`（`zt_core::engine::DataFiles::search_path`）：

- `lexicon.bin`（或 `lexicon.tsv`），必要；
- `lm/lm.bin`（或 `lm/{unigram,bigram,backoff}.tsv`），語言模型；
- `lm_chat/lm.bin`（或其 TSV），聊天語料的模型，有附帶時使用；
- `channel.bin`，讀音通道（「接受常見的其他讀法」），有附帶時使用；
- 旁邊的說明檔，供「關於」頁使用（`lm/NOTICE.md`、`lm/articles.tsv.gz`、`channel/NOTICE.md` 等）。

`tools/zt data fetch engine && tools/zt data compile` 會在 `data/build` 產生這些檔案，和 macOS App 相同。神經模型（重排序與字模型）在 macOS 上用 Core ML，這裡不載入；接口留在 composer 的 hook（`EngineSession::composer_mut`）。個人資料、`stats.json` 與 `corrections.jsonl` 放在 `zt_core::paths::data_dir`；設定與按鍵放在 `zt_core::paths::settings_file`。試打工作階段在同一份 `EngineData` 上開自己的引擎，個人資料只在記憶體中；對應到記憶體的檔案是共用的。

## 介面規則

- 頁面只用 `ui/` 元件與 `theme` token 組成；`pages/` 裡不寫顏色、尺寸或時間的字面值。
- 每一頁與子頁面的設定列、順序、文字與連結都和 macOS 版相同。參考資料：Swift 原始碼，以及 macOS App 產生的截圖（`ZhuyinType --render-ui DIR`，放在不進 git 的 `.reference/`）。
- 動畫使用 `theme::motion`，系統要求減少動態效果時略過。
- 候選字視窗與中英提示從不取得鍵盤焦點（Windows：`WS_EX_NOACTIVATE`；X11：override-redirect、不接受輸入焦點）。
- 不顯示畫面的執行（截圖、測試）不在使用者的桌面開視窗，而在主機的虛擬顯示器上執行。

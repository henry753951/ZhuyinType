# ZhuyinType：Linux 與 Windows 版（GPUI）

Linux 與 Windows 的前端是一個 Rust 行程：用 [GPUI](https://www.gpui.rs/)（透過 `gpui-kit`）繪製設定視窗、候選字視窗、中英提示與設定嚮導，持有引擎，並接受原生輸入法介面的連線。它取代了 Qt 客戶端（2026-11 移除）。

**功能與版面和 macOS 版相同**：頁面、區塊、設定列、子頁面、嚮導步驟、候選字視窗的各種樣式與頁面之間的連結，順序與文字都一樣（`platforms/macos/Sources/ZhuyinTypeApp/UI/`）。視覺是 ZhuyinType 自己的一套現代設計語言（`crates/zhuyintype/src/theme.rs` 的 token），不模仿 macOS，也不模仿 Windows 原生外觀：暖色中性的表面、單色線條圖示、以細分隔線區隔設定列而不是填色卡片、強調色只用在少數地方，拉丁文字與數字用內嵌的 Inter，中文用系統字型。動態沿用 macOS 版的節奏（140 ms 翻頁、子頁面推入與返回）。macOS 版維持原生外觀，不受影響。視窗不使用系統邊框：標題列與視窗按鈕由 App 自己繪製，和 Discord、Claude 桌面版一樣。

## 目錄

```text
platforms/desktop/
  settings.json             所有頁面與嚮導共用的設定描述
  IPC.md                    服務與原生輸入法介面之間的通訊協定
  crates/zt-panel/          候選字視窗本身，不依賴介面框架：顯示內容、樣式、版面、配色、鍵帽寫法與視窗位置；
                            GPUI 與 TSF DLL 共用，兩邊畫出來的視窗一樣
  crates/zt-core/           不含介面：設定、路徑、引擎工作階段、輸入控制、快捷鍵、使用統計、改字記錄、
                            試打工作階段、通訊服務（Unix socket／named pipe）
  crates/zhuyintype/        GPUI App（執行檔 `zhuyintype`）：theme、ui/ 元件、pages/、candidate/、wizard/
  crates/zt-tsf/            只用於 Windows：TSF 文字服務 DLL 與登記，以 Rust（`windows` crate）撰寫；
                            候選字視窗與中英提示在應用程式內以 Direct2D 畫（panel_window.rs、panel_draw.rs）
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

`tools/zt data fetch engine && tools/zt data compile` 會在 `data/build` 產生這些檔案，和 macOS App 相同。神經模型（重排序與字模型）在 macOS 上用 Core ML，這裡不載入；接口留在 composer 的 hook（`EngineSession::composer_mut`）。個人資料、`stats.json` 與 `corrections.jsonl` 放在 `zt_core::paths::data_dir`；設定與按鍵放在 `zt_core::paths::settings_file`。這些檔案都先寫到旁邊的暫存檔、寫進磁碟後再換上（`zt_core::settings::write_atomically`），當機或斷電後只會是舊的或新的內容。設定檔無法讀取時（寫到一半、手動改壞）先用預設值，第一次變更設定前把原檔留成 `settings.unreadable.json`，不會直接蓋掉使用者的設定與按鍵。試打工作階段在同一份 `EngineData` 上開自己的引擎，個人資料只在記憶體中；對應到記憶體的檔案是共用的。

## 檢查輸入法

`zt_core::health` 分成讀取系統與判斷兩部分：`probe_windows`（登錄、服務管理員、`SystemParametersInfo`、`wevtutil`）與 `probe_linux`（`fcitx5-setup status`、環境變數、`gnome-extensions`）只讀取、回傳單純的資料結構；`windows::checks`、`linux::checks`、`common` 的函式把資料變成檢查結果，不碰系統，以假資料在任何平台上測試。App（`crates/zhuyintype/src/health.rs`）在背景執行緒執行一次，結果放在全域狀態供「關於 › 檢查輸入法」與側欄使用；設定視窗打開與每次回到前景時重新檢查。服務是否有回應以一個它不支援的操作（`health`）試連線，任何回應都表示它在；這個操作不改變任何狀態。App 的記錄器把警告與錯誤另存一份在記憶體（`health::recent`，最多 40 行），供「回報問題」使用。

## 介面規則

- 頁面只用 `ui/` 元件與 `theme` token 組成；`pages/` 裡不寫顏色、尺寸或時間的字面值。
- 每一頁與子頁面的設定列、順序、文字與連結都和 macOS 版相同。參考資料：Swift 原始碼，以及 macOS App 產生的截圖（`ZhuyinType --render-ui DIR`，放在不進 git 的 `.reference/`）。
- 動畫使用 `theme::motion` 與 `motion.rs`（選取的淡入、展開箭頭、按下與鍵盤焦點框、浮動視窗的出現）。系統要求減少動態效果時（Windows 的「動畫效果」、桌面入口的 `reduced-motion`、GNOME 的 `enable-animations`；`ZT_REDUCE_MOTION=1` 可強制）略過位移，淡入淡出照常。候選字視窗與中英提示在每次更新時只讀一次這個設定，動畫不延後任何按鍵的結果。
- 「主題」與「透明效果」（`zt_core::appearance`）：每個視窗在 `render` 開頭呼叫 `theme::sync`，依設定與系統換掉全域的 `Palette`；設定視窗與嚮導再呼叫 `platform::backdrop::apply`，向視窗系統要背景材質（Windows 的 Mica、KDE 的模糊）與深淺色的邊框。材質存在時，視窗的根不畫底色，框架與頁面畫 `Palette::frame`、`Palette::page` 兩層半透明的顏色；其他顏色不變。細節見 [DESKTOP.md](../../docs/product/DESKTOP.md)「主題與透明效果」。
- 候選字視窗與中英提示從不取得鍵盤焦點（Windows：`WS_EX_NOACTIVATE`；X11：override-redirect、不接受輸入焦點）。
- Windows 上的候選字視窗由 TSF DLL 在打字的應用程式行程內畫（IPC.md〈候選字視窗〉）：以該應用程式的視窗為擁有者、`CS_IME` 視窗類別，所以出現在開始功能表搜尋框與沉浸式 App 之上。系統會替視窗畫圓角時（Windows 11 且不是虛擬機的基本顯示卡），邊緣交給系統：圓角、細框、陰影與 Acrylic 背景（`DWMSBT_TRANSIENTWINDOW`，和系統選單相同；「透明效果」關閉時畫不透明的背景）；其他情況（Windows 10、虛擬機）改用分層視窗，自己畫圓角、細框與柔和陰影，所以任何環境都是圓角。繪圖優先用 GPU；某些沙箱 App（開始功能表的搜尋）不給建立 GPU 繪圖目標，改以 Direct2D 軟體繪圖畫到 GDI。字型是安裝目錄 `fonts\` 的 Inter，中文由 DirectWrite 退回微軟正黑體。欄位取得焦點、處理完焦點之後，文字服務先在背景建立中英提示的視窗與繪圖目標，並在畫面外畫一次「中」「英」（`PanelWindow::warm`）：硬體繪圖目標第一次在一個行程裡建立時要載入 Direct3D 與顯示卡驅動程式，這段時間不再落在使用者第一次切換中英的時候。Linux 仍由 App 畫浮動視窗。
- 組字在應用程式裡的標示和 macOS 相同：每個詞一條細底線，候選字清單正在替換的詞反白並加粗底線，清單沒開、游標在組字中間時 ↓ 會改的詞加粗底線。回應的 `clauses`、`focus`、`target`（IPC.md）帶來分段；TSF DLL 是顯示屬性的提供者（`GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`，`crates/zt-tsf/src/display_attributes.rs`），把屬性寫進組字範圍的 `GUID_PROP_ATTRIBUTE`，反白用系統的選取色（`COLOR_HIGHLIGHT`）；相鄰的詞輪流用兩個外觀相同的屬性，讓會在屬性交界留空隙的應用程式（瀏覽器、Word）分出每個詞。Fcitx5 外掛以 `fcitx::Text` 的格式表達同樣的三種標示（`Underline`、加上 `HighLight`、加上 `Bold`）。
- 快速設定（IPC.md〈快速設定〉）在服務裡是 `InputController` 的兩個狀態：`learning_paused`（只在記憶體，`unlearned()` 讓每個欄位當成私人欄位）與 `simplified()`（設定 `outputScript`）。簡體只在輸出端換：`committed()` 送出的 `Commit`、`Host::snapshot` 的組字與 `output_presentation()`（候選字視窗與 `panel`），引擎與觀察者（學習、改字記錄、使用統計）看的都是繁體。快捷鍵的 `QuickPanel` 事件在 App 畫視窗時成為 `UiEvent::QuickPanel`，GPUI 在游標旁開不取得焦點的小視窗（`crates/zhuyintype/src/candidate/quick.rs`），下一個按鍵或換欄位時服務送 `UiEvent::HideQuickPanel`。Fcitx5 外掛把四個勾選項目加在輸入法選單（`fcitx::SimpleAction`，可勾選），狀態取自每個回應的 `english`、`fullWidth`、`simplified`、`learningPaused`。
- Windows 工作列輸入指示器的中／英（`GUID_LBI_INPUTMODE`，`crates/zt-tsf/src/mode_button.rs`）只在有可以輸入中文的欄位時可用：沒有取得焦點的欄位（桌面、沒有文字欄位的視窗）、安全欄位、App 沒有回應時，按鈕停用並顯示「英」，和微軟注音相同，因為這時按鍵照原樣交給程式。
- Windows 的快速設定（`crates/zt-tsf/src/quick_menu.rs`）：中／英按鈕的右鍵選單最上面是「英文」「半形標點」「簡體」「暫停學習」四個勾選項，再來是設定的各個位置；勾選狀態在選單打開前以空的 `quick` 讀一次，選了就送只帶那一項的 `quick`。沒有可以輸入中文的欄位時四項變灰。「開啟快速設定」快捷鍵的回應帶 `quickPanel` 時，文字服務在按鍵處理結束、修飾鍵放開之後，在游標下方顯示同一份選單（沒有「設定嚮導…」等項目），擁有者是文字服務自己隱藏的視窗，不搶焦點，組字照常保留。按鈕在簡體輸出時顯示「简」，暫停學習時提示文字加上「暫停學習」。
- 不顯示畫面的執行（截圖、測試）不在使用者的桌面開視窗，而在主機的虛擬顯示器上執行。

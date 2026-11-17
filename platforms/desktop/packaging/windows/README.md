# Windows 安裝程式

`ZhuyinType-<版本>-windows-x64-setup.exe` 是 NSIS 安裝程式（Modern UI 2、繁體中文介面）。使用者按兩下、在 UAC 視窗按一次「是」（一般使用者帳號則輸入系統管理員的帳號密碼），其餘自動完成；從「設定 › 應用程式」可以移除。支援 64 位元的 Windows 10 與 Windows 11；ARM 版 Windows 的程式無法載入 x64 的文字服務，安裝程式會拒絕安裝。

所有建置都在 GPU 主機（Linux）上交叉編譯，不在 Mac 上執行。

## 建置

在 Mac 的 worktree 根目錄執行：

```sh
platforms/desktop/scripts/remote.sh packaging/windows/build.sh
platforms/desktop/scripts/remote.sh --fetch zhuyintype/work/desktop-<checkout 名稱>/dist/desktop /tmp/zt-windows
```

`build.sh` 依序：

1. `cargo xwin build --release --locked -p zhuyintype -p zt-tsf --target x86_64-pc-windows-msvc`，再以 `--target i686-pc-windows-msvc` 建置 32 位元的 `zt-tsf`。C runtime 靜態連結（`.cargo/config.toml`），不需要另外安裝 Visual C++ 可轉散發套件。
2. `tools/zt data fetch engine` 與 `tools/zt data compile --no-channel` 產生引擎資料。
3. 複製授權檔，列出 Windows 建置用到的 Rust 套件與授權（`licenses/Rust-crates.txt`）。
4. 以 ImageMagick 從 App 圖示做安裝程式的側圖與頁首圖（沒有 ImageMagick 時用 NSIS 預設圖）。
5. `makensis -INPUTCHARSET UTF8 installer.nsi`，輸出到版本庫的 `dist/desktop/`。版本取自 `platforms/desktop/Cargo.toml` 的 `[workspace.package]`。

主機需要 `cargo-xwin`（`cargo +1.98.0 install cargo-xwin --locked`）、LLVM 的 `clang-cl`、`lld-link`、`llvm-rc`，以及 `makensis`（`apt install nsis`）。cargo-xwin 第一次執行時下載 MSVC 與 Windows SDK 的標頭與程式庫到 `~/.cache/cargo-xwin`；`XWIN_ARCH=x86,x86_64` 只下載這裡用到的兩種。

### 安裝的檔案

預設安裝到 `C:\Program Files\ZhuyinType`。安裝位置必須是新的或空的資料夾，或先前的安裝位置：安裝程式會把資料夾交給系統管理員，解除安裝時刪除它，所以不能是 `C:\`、Program Files 或使用者自己的資料夾。

| 檔案 | 內容 |
|---|---|
| `zhuyintype.exe` | App：設定、候選字視窗、設定精靈與輸入法服務（broker） |
| `zhuyintype-tsf-x64.dll`、`zhuyintype-tsf-x86.dll` | TSF 文字服務，64 位元與 32 位元程式各自載入（`crates/zt-tsf`） |
| `onnxruntime.dll` | ONNX Runtime，App 執行神經模型時載入 |
| `logo.ico` | 語言清單與工作列輸入指示器裡的輸入法圖示 |
| `zhuyintype.ico` | 開始功能表、「設定 › 應用程式」的圖示 |
| `background.lnk` | 以 `--background` 啟動 App 的捷徑（更新服務無法使用時，在安裝者的工作階段重新啟動用） |
| `data\` | 引擎資料：詞庫、讀音通道、兩個語言模型與神經模型，以及它們的授權說明（`build.sh`〈引擎資料〉） |
| `fonts\` | 候選字視窗的字型，文字服務在各程式內讀取 |
| `updates\` | 更新服務下載的安裝程式（下一次更新或重新開機時刪除） |
| `licenses\` | AGPL-3.0、NOTICE、資料來源（`SOURCES.md`、`sources.json`）、資料授權、Rust 套件清單 |
| `uninstall.exe` | 解除安裝程式 |

App 在 `<執行檔目錄>\data` 找引擎資料。

## 安裝程式做的事

1. 設定安裝資料夾的權限：擁有者為 Administrators，只有 SYSTEM 與 Administrators 可以修改，其他使用者與 Store App（`ALL APPLICATION PACKAGES`）只能讀取與執行，資料夾內的檔案改為繼承這些權限。更新服務以 SYSTEM 執行這裡的程式，文字服務在每個程式內執行，所以不能讓其他帳號修改；在 Program Files 外另選的資料夾原本會繼承 `C:\` 讓所有登入的使用者修改的權限。
2. 記下 App 在哪些工作階段執行（PowerShell 的 `Get-Process`；無法取得時記為全部），再結束它（先以 `taskkill` 要求結束，1.5 秒後強制結束，最多再等 10 秒到它真的結束）。
3. 複製檔案。應用程式正在使用的舊 DLL 無法覆寫，但可以改名：先改名為 `*.old`，再排定下次開機刪除；正在使用它的程式繼續使用舊版，重新開啟後載入新版。不需要重新開機，完成頁也不詢問。
4. 以 `regsvr32` 登記兩個 DLL（64 位元用 `System32\regsvr32.exe`，32 位元用 `SysWOW64\regsvr32.exe`）。`DllRegisterServer` 登記本機的 COM 類別（`InprocServer32`、`ThreadingModel=Apartment`）、zh-TW（0x0404）語言設定檔與類別：鍵盤、顯示屬性提供者（組字的底線與反白）、Store／沉浸式 App、工作列輸入指示器、UI-less。CLSID 與設定檔 GUID 和先前的 C++ 版相同。
5. 把「ZhuyinType 注音」加入安裝者的輸入法清單（`regsvr32 /n /i:user`，在 DLL 內呼叫 `InstallLayoutOrTip`）。原有的語言、輸入法與預設輸入法不變。
6. 登記 Active Setup：這台電腦的其他使用者下次登入時，各自的清單也加入一次。版本固定為 1，之後更新不會替已經自行移除的人再加回去。App 啟動時也做同樣的檢查（`enable_input_method_for_user`）：使用者的 HKCU 沒有 Active Setup 的版本時，以該使用者身分執行同一個 `regsvr32 /n /i:user` 並寫下版本。一般使用者輸入系統管理員密碼安裝時，安裝程式以那位系統管理員的身分執行，第 5 步只加到系統管理員的清單；App 在使用者的工作階段啟動後，使用者自己的清單立即加入，不必等下次登入。
7. 登入時啟動：`HKLM\Software\Microsoft\Windows\CurrentVersion\Run\ZhuyinType` = `"…\zhuyintype.exe" --background`。
8. 開始功能表捷徑「ZhuyinType 設定」（所有使用者）。
9. 「設定 › 應用程式」的項目：名稱、圖示、發行者、版本、大小、解除安裝與安靜解除安裝指令。
10. 更新服務「ZhuyinType 更新」（`ZhuyinTypeUpdate`）：手動啟動、LocalSystem，執行 `zhuyintype.exe --update-service`。安全性描述元 `D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;LCRP;;;AU)` 讓登入的使用者只能查詢與啟動它。它自己取得並驗證簽章過的更新清單與安裝程式（[DESKTOP.md](../../../../docs/product/DESKTOP.md)〈更新〉），放進 `updates\` 後以 `/S /UPDATE` 執行。服務先確認安裝資料夾與 `zhuyintype.exe` 只有 SYSTEM、Administrators 與 TrustedInstaller 能擁有與修改，否則不安裝；執行前以不允許他人寫入或刪除的方式開啟下載的安裝程式，從這個 handle 重新驗證簽章，直到程序啟動後才關閉。以 `relaunch` 參數啟動服務時（`sc start ZhuyinTypeUpdate relaunch`）不更新，只依安裝程式寫在 `HKLM\Software\ZhuyinType` 的要求重新啟動 App（見下一步），不採用呼叫者的任何輸入。
11. 重新啟動 App：在第 2 步記下的每個工作階段，以該工作階段的使用者身分在背景啟動 App（`--background`）。只有 SYSTEM 能替每個登入的使用者啟動程式，所以安裝程式把工作階段寫進 `HKLM\Software\ZhuyinType` 的 `RelaunchSessions`，再以 `relaunch` 參數啟動更新服務；帶 `/UPDATE` 的安裝本身就是 SYSTEM，直接執行 `zhuyintype.exe --relaunch-sessions`。第一次安靜安裝時（之前沒有安裝、App 也沒有執行）在每個登入中的工作階段啟動，讓安裝的人立即可以打字。安裝失敗（例如登記失敗而中止）時也重新啟動。
12. 完成頁預設勾選「開啟 ZhuyinType」，開啟設定。安裝程式的身分就是螢幕前的使用者時（系統管理員按「是」）透過 Explorer 開啟，視窗會在前景；一般使用者輸入系統管理員密碼時，安裝程式是那位系統管理員，改由更新服務以 `OpenSession` 在這個工作階段、以螢幕前的使用者身分開啟。

App 一律以登入的使用者身分啟動、不提升權限：broker 的 named pipe 以使用者的 SID 命名、屬於該使用者，以其他帳號啟動的 App 服務不到這位使用者的輸入法。

### 為什麼用 HKLM Run

輸入法是整台電腦安裝、所有使用者都能選用，所以每個登入的人都需要自己的 App（broker）。HKLM Run 對每個使用者都生效，解除安裝時在一個地方移除，不會在各使用者的設定檔留下項目；使用者仍可在「工作管理員 › 啟動應用程式」停用。由提升權限的安裝程式寫入 HKCU 只會寫到提升的那個帳號。

## 解除安裝

「設定 › 應用程式 › ZhuyinType 注音輸入法 › 解除安裝」，或 `uninstall.exe /S`：

1. 停止並刪除更新服務；結束 App；從解除安裝者的輸入法清單移除（`regsvr32 /u /n /i:user`）。
2. `regsvr32 /u` 兩個 DLL：移除語言設定檔、類別、TSF 登錄機碼與 COM 類別。
3. 移除 Active Setup、登入啟動、開始功能表捷徑、「設定 › 應用程式」項目與 `HKLM\Software\ZhuyinType`。
4. 刪除程式檔案。仍被應用程式載入的 DLL 改名後排定下次開機刪除，安裝資料夾也在那時刪除。
5. 個人資料（`%APPDATA%\ZhuyinType` 的設定、`%LOCALAPPDATA%\ZhuyinType` 的自訂詞、學習記錄、使用統計與改字記錄）預設保留；在「選取元件」頁勾選「同時刪除我的自訂詞、學習記錄與設定」才刪除。刪除的是執行解除安裝程式的帳號的資料：一般使用者輸入系統管理員密碼解除安裝時，刪除的是那位系統管理員的資料，使用者自己的資料要手動刪除。從輸入法清單移除（第 1 步）也只對這個帳號，其他使用者的清單不變（文字服務已取消登記）。

## 發布

推送 `vX.Y.Z` 標籤時，`release.yml` 呼叫 `desktop.yml` 建置安裝程式，附加到 GitHub release：`ZhuyinType-<版本>-windows-x64-setup.exe`，以及固定檔名的 `ZhuyinType-Windows-x64-setup.exe`（README 的下載連結指向 `releases/latest/download/` 的這個檔名），檢查碼在 `SHA256SUMS-desktop.txt`。

## 安靜安裝與參數

```bat
ZhuyinType-Windows-x64-setup.exe /S
ZhuyinType-Windows-x64-setup.exe /S /D=D:\Apps\ZhuyinType
"C:\Program Files\ZhuyinType\uninstall.exe" /S
```

`/D=` 必須放在最後、不加引號。結束代碼 0 表示成功；登記 64 位元文字服務失敗時為 2；不支援的系統（32 位元、Windows 10 之前、ARM）或不能使用的安裝位置為 3。安靜執行時不顯示任何對話框（更新服務在工作階段 0 執行安裝程式，那裡的對話框沒有人看得到，會讓安裝停住）。

更新服務執行的安裝程式自己替換掉服務的執行檔，服務不等它結束，所以安裝程式的結束代碼不會回報給 App；失敗時 App 照常重新啟動，下次檢查時再試。

## 程式碼簽章（尚未設定）

TODO：取得憑證後設定 `ZT_SIGN_COMMAND`，`%1` 代表要簽章的檔案，例如 `osslsigncode sign -pkcs12 cert.pfx -pass "$PASS" -t http://timestamp.digicert.com -in %1 -out %1.signed && mv %1.signed %1`。`build.sh` 會用它簽 App 與兩個 DLL，並交給 NSIS 的 `!finalize`／`!uninstfinalize` 簽安裝程式與解除安裝程式。未簽章的安裝程式第一次執行時，SmartScreen 會顯示「Windows 已保護您的電腦」。

## GPUI 的 Windows 著色器

GPUI 的 Windows 後端只在 Windows 主機上以 `fxc.exe` 編譯 HLSL，在 Linux 交叉編譯時沒有著色器。`vendor/gpui-pre-windows` 是 crates.io 的 `gpui-pre-windows 0.3.12`（Apache-2.0），只改了 `build.rs`：目標是 Windows、建置主機不是 Windows 時，改用事先編譯好的 `shaders/shaders_bytes.rs`，並比對 HLSL 原始檔的雜湊。`Cargo.toml` 以 `[patch.crates-io]` 使用這份。

更新 GPUI（或著色器改變而建置失敗）時：

1. 用新版的 crate 取代 `vendor/gpui-pre-windows/src` 與 `Cargo.toml`，保留 `build.rs` 的修改。
2. `scripts/remote.sh cargo xwin build --release -p gpui-shaders --target x86_64-pc-windows-msvc`，把 `gpui-shaders.exe`、`src/shaders.hlsl` 與 `src/alpha_correction.hlsl` 複製到 Windows。
3. 在 Windows 執行 `gpui-shaders.exe shaders.hlsl > shaders_bytes.rs`（使用系統的 `d3dcompiler_47.dll`，與 `fxc /O3` 相同），結果放回 `vendor/gpui-pre-windows/shaders/`。

## 驗證紀錄

2026-11-16，Windows 11 Pro zh-TW（10.0.26300）VM，內建 Administrator 帳號自動登入，GUI 測試以排程工作在工作階段 1 執行。

| 項目 | 結果 |
|---|---|
| 安靜安裝 | 13 秒完成，結束代碼 0；兩個 DLL 的 COM（64 位元與 WOW64）、設定檔、四個類別、Run、Active Setup、捷徑與「設定 › 應用程式」項目都在 |
| 互動安裝 | 歡迎、授權、安裝位置、完成四頁；完成後 App 由 explorer.exe 啟動 |
| 輸入法清單 | `Get-WinUserLanguageList` 的 zh-Hant-TW 有 `0404:{25BC3548-…}{E8207CF3-…}`；Win+空白鍵可選取；工作列顯示「中」與 ZhuyinType 圖示 |
| Notepad（Store 版，64 位元） | 以真正的 App broker 打「ㄋㄧˇㄏㄠˇ」得到「你好」，候選字視窗出現在游標下方，Notepad 保持前景 |
| 32 位元程式 | `SysWOW64\charmap.exe` 的欄位打出「中」（以測試 broker） |
| 中／英 | 單按 Shift 與按一下工作列的「中」都會切換，顯示「英」標示；右鍵選單有「切換成中文／英文」與「ZhuyinType 設定…」，後者送出 `show-settings` |
| broker 不存在 | 按鍵交回 Notepad，四個鍵 714 ms（每鍵本身等 160 ms） |
| broker 600 ms 才回覆 | 200 ms 後放棄，該鍵交回 Notepad |
| 登入時啟動 | 重新開機自動登入後 `zhuyintype.exe --background` 在工作階段 1 執行 |
| 更新（Notepad、Explorer、搜尋與 App 正在使用 DLL） | 舊 DLL 改名為 `*.old` 並排定開機刪除；開著的 Notepad 繼續打字；App 結束後重新啟動（安靜更新時為 `--background`）；不要求重新開機 |
| 解除安裝 | COM、TSF 設定檔、Run、Active Setup、捷徑、「設定 › 應用程式」項目與輸入法清單都已移除；個人資料保留；只剩仍被載入、排定開機刪除的 DLL |
| 重新安裝 | 安裝後立即可以打字 |

尚未驗證：一般使用者帳號經 UAC 提升的安裝（測試帳號是內建 Administrator，不會出現 UAC 視窗）、勾選刪除個人資料的互動解除安裝、多螢幕與混合 DPI、Windows 10。

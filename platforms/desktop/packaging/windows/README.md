# Windows 安裝程式

`ZhuyinType-<版本>-windows-x64-setup.exe` 是 NSIS 安裝程式（Modern UI 2、繁體中文介面）。使用者按兩下、在 UAC 視窗按一次「是」，其餘自動完成；從「設定 › 應用程式」可以移除。支援 64 位元的 Windows 10 與 Windows 11。

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

安裝到 `C:\Program Files\ZhuyinType`：

| 檔案 | 內容 |
|---|---|
| `zhuyintype.exe` | App：設定、候選字視窗、設定精靈與輸入法服務（broker） |
| `zhuyintype-tsf-x64.dll`、`zhuyintype-tsf-x86.dll` | TSF 文字服務，64 位元與 32 位元程式各自載入（`crates/zt-tsf`） |
| `logo.ico` | 語言清單與工作列輸入指示器裡的輸入法圖示 |
| `zhuyintype.ico` | 開始功能表、「設定 › 應用程式」的圖示 |
| `background.lnk` | 以 `--background` 啟動 App 的捷徑（安靜更新後重新啟動用） |
| `data\lexicon.bin`、`data\lexicon.json` | 詞庫（mmap 二進位格式）與它的來源雜湊 |
| `data\lm\lm.bin`、`data\lm\lm.json` | 語言模型與它的來源雜湊 |
| `data\lm\NOTICE.md`、`data\lm\articles.tsv.gz` | 語言模型的授權說明與訓練文字的出處清單 |
| `licenses\` | AGPL-3.0、NOTICE、資料來源（`SOURCES.md`、`sources.json`）、資料授權、Rust 套件清單 |
| `uninstall.exe` | 解除安裝程式 |

App 在 `<執行檔目錄>\data` 找引擎資料。

## 安裝程式做的事

1. 結束執行中的 ZhuyinType（先以 `taskkill` 要求結束，1.5 秒後強制結束），並記下它是否在目前的工作階段執行。
2. 複製檔案。應用程式正在使用的舊 DLL 無法覆寫，但可以改名：先改名為 `*.old`，再排定下次開機刪除；正在使用它的程式繼續使用舊版，重新開啟後載入新版。不需要重新開機，完成頁也不詢問。
3. 以 `regsvr32` 登記兩個 DLL（64 位元用 `System32\regsvr32.exe`，32 位元用 `SysWOW64\regsvr32.exe`）。`DllRegisterServer` 登記本機的 COM 類別（`InprocServer32`、`ThreadingModel=Apartment`）、zh-TW（0x0404）語言設定檔與類別：鍵盤、Store／沉浸式 App、工作列輸入指示器、UI-less。CLSID 與設定檔 GUID 和先前的 C++ 版相同。
4. 把「ZhuyinType 注音」加入安裝者的輸入法清單（`regsvr32 /n /i:user`，在 DLL 內呼叫 `InstallLayoutOrTip`）。原有的語言、輸入法與預設輸入法不變。
5. 登記 Active Setup：這台電腦的其他使用者下次登入時，各自的清單也加入一次。版本固定為 1，之後更新不會替已經自行移除的人再加回去。
6. 登入時啟動：`HKLM\Software\Microsoft\Windows\CurrentVersion\Run\ZhuyinType` = `"…\zhuyintype.exe" --background`。
7. 開始功能表捷徑「ZhuyinType 設定」（所有使用者）。
8. 「設定 › 應用程式」的項目：名稱、圖示、發行者、版本、大小、解除安裝與安靜解除安裝指令。
9. 完成頁預設勾選「開啟 ZhuyinType」。安靜安裝（`/S`）時，只有安裝前 App 在同一個工作階段執行（例如更新）才在背景重新啟動；其他情況等下次登入。

App 一律以登入的使用者身分啟動、不提升權限：安裝程式本身以系統管理員權限執行，所以透過 Explorer 開啟執行檔或捷徑。broker 的 named pipe 屬於該使用者。

### 為什麼用 HKLM Run

輸入法是整台電腦安裝、所有使用者都能選用，所以每個登入的人都需要自己的 App（broker）。HKLM Run 對每個使用者都生效，解除安裝時在一個地方移除，不會在各使用者的設定檔留下項目；使用者仍可在「工作管理員 › 啟動應用程式」停用。由提升權限的安裝程式寫入 HKCU 只會寫到提升的那個帳號。

## 解除安裝

「設定 › 應用程式 › ZhuyinType 注音輸入法 › 解除安裝」，或 `uninstall.exe /S`：

1. 結束 App；從解除安裝者的輸入法清單移除（`regsvr32 /u /n /i:user`）。
2. `regsvr32 /u` 兩個 DLL：移除語言設定檔、類別、TSF 登錄機碼與 COM 類別。
3. 移除 Active Setup、登入啟動、開始功能表捷徑、「設定 › 應用程式」項目與 `HKLM\Software\ZhuyinType`。
4. 刪除程式檔案。仍被應用程式載入的 DLL 改名後排定下次開機刪除，安裝資料夾也在那時刪除。
5. 個人資料（`%APPDATA%\ZhuyinType` 的設定、`%LOCALAPPDATA%\ZhuyinType` 的自訂詞、學習記錄、使用統計與改字記錄）預設保留；在「選取元件」頁勾選「同時刪除我的自訂詞、學習記錄與設定」才刪除。

## 安靜安裝與參數

```bat
ZhuyinType-0.5.3-windows-x64-setup.exe /S
"C:\Program Files\ZhuyinType\uninstall.exe" /S
```

結束代碼 0 表示成功；登記 64 位元文字服務失敗時為 2。

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

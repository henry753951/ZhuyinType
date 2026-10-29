# ZhuyinType：Linux 輸入法

Fcitx5 addon 負責作業系統焦點、按鍵、組字與文字送達，共用 Qt host 負責 `EngineSession`、`InputController`、設定、快捷鍵與候選字視窗。addon 透過本機 Unix socket 使用 IME broker v1，依賴 Fcitx5 與 nlohmann/json；Qt 與 Rust 在獨立 host 行程執行。

## 建置與安裝

需要 Linux、C++20、CMake 3.24、Fcitx5 5.1.9 以上、nlohmann/json 3.11 以上。Ubuntu 開發套件：

```sh
sudo apt-get install libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev nlohmann-json3-dev
cmake -S platforms/linux -B platforms/linux/build \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
cmake --build platforms/linux/build --parallel
sudo cmake --install platforms/linux/build
```

本目錄透過 `find_package(nlohmann_json 3.11 REQUIRED)` 使用系統提供的 header-only 函式庫，未隨版本庫附帶其原始碼。實測 Ubuntu 套件 `nlohmann-json3-dev` 的標頭版本為 3.11.3；上游 [nlohmann/json](https://github.com/nlohmann/json) 使用 [MIT 授權](https://github.com/nlohmann/json/blob/v3.11.3/LICENSE.MIT)。[第三方授權告知](LICENSES/nlohmann-json.txt) 包含已安裝套件列出的 Niels Lohmann、Florian Loitsch 與 Bjoern Hoehrmann 告知。CMake 將此告知與專案 AGPL-3.0 授權安裝至 `${CMAKE_INSTALL_DATADIR}/licenses/zhuyintype/`；重散布編譯 addon 時應隨套件保留。ZhuyinType 的專案授權維持 AGPL-3.0。

共用 Qt 桌面程式的建置方式見 [Qt 說明](../qt/README.md)。設定開發用 host 的使用者自動啟動：

```sh
bash platforms/linux/scripts/install-host.sh \
  "$PWD/.build-qt/zhuyintype-desktop" "$PWD/data/build/lexicon.tsv"
~/.local/bin/zhuyintype-ime-host
```

RDP 或缺乏可靠 3D 驅動的測試環境，可在安裝 helper 時加上 `QT_QUICK_BACKEND=software`；helper 會將這個 Qt 設定寫入自己的 launcher，僅套用於此 host 行程。一般安裝不指定時沿用 Qt 預設 renderer。

host helper 寫入自己的 launcher 與 autostart 檔案；桌面輸入法啟用依下節設定。路徑綁定目前的桌面程式建置，搬移建置目錄後須重新執行。

## 啟用 Xfce／X11 桌面

1. 先備份 `~/.config/fcitx5/profile`，在 `fcitx5-configtool` 的目前群組加入「ZhuyinType」，保留原有鍵盤與其他輸入法項目。
2. Ubuntu 執行 `im-config -n fcitx5`，設定此使用者的 X11 登入輸入法。Fcitx5 由 `im-config` 的登入流程啟動，host 由前節的 XDG autostart 啟動。
3. 重新登入後，在桌面輸入法選單選用「ZhuyinType」；於文字欄位有焦點時，`fcitx5-remote -n` 應回報 `zhuyintype`，`fcitx5-remote` 回報 `2` 表示已啟用。

本次新測試 VM 已把實際 `ztdev` profile 的 `DefaultIM` 設為 `zhuyintype`，同時保留 `keyboard-us`；目前桌面已啟動正式服務並完成應用程式送達，詳見 [部署驗證](VALIDATION.md)。GNOME Wayland 的全域設定維持原狀。其他應用程式輸入模組依 [Fcitx5 官方安裝說明](https://fcitx-im.org/wiki/Setup_Fcitx_5) 設定。

詞庫、模型、個人資料、設定與快捷鍵均由共用 host 管理；資料版本與授權見 [resources/SOURCES.md](../../resources/SOURCES.md)。原生 addon 不使用 `ZT_FCITX_DATA_DIR`、`ZT_FCITX_USER_DIR` 或獨立 Rust 靜態函式庫。

## broker 與焦點

- 每個焦點週期使用獨立 context，先向舊 context 送 `blur`，再對新 context 送 `focus`。一般失焦的提交送回原欄位；`reset` 清除組字。欄位銷毀時關閉連線。
- 預設連到 `$XDG_RUNTIME_DIR/zhuyintype/ime-v1`；未設定時使用 `/tmp/zhuyintype-UID/ime-v1`。host 建立私人目錄與 user-only socket。`ZT_IME_SOCKET` 可覆寫完整端點，host 另接受 `--ime-socket PATH`。
- 使用 newline-delimited UTF-8 JSON，每框上限 256 KiB，驗證版本、id、context、欄位型別、UTF-8 與 UTF-16 游標邊界。透過 `SO_PEERCRED` 檢查 host UID。
- 非阻塞 socket 的連線與單次請求各有 80 ms 的總時限；失敗關閉連線並放行目前按鍵。下一次按鍵重新連線，使用新 context，失敗的請求不重送。addon 在記憶體保留最後成功回應的組字；同一個仍聚焦且非安全的欄位遇到中斷時，先提交這份已知文字一次，再放行目前按鍵。secure、reset 與欄位銷毀直接丟棄快取。未確認的目前按鍵可能已被 host 處理，其結果不重送。
- Fcitx event loop 只在欄位有焦點且已連線時，每 20 ms 送 `poll`；Qt 滑鼠操作的結果由下一次 `poll` 或按鍵回應送達。每個回應只套用到發出請求的 context。
- 送出 Qt 公開 key enum 與 modifier flags，包含 Shift 左右、放開與重複事件；焦點後首個一般按鍵的 lock 狀態透過同 context 的 `focus.capsLock` 同步，Caps Lock 鍵本身由 host 切換一次。Fcitx client preedit 游標從 UTF-16 轉成 UTF-8 位移；`focus.inlinePreedit` 回報欄位能力，未支援行內組字時由共用 Qt popup 顯示組字。Fcitx 原生面板保持空白，候選字一律由共用 Qt 視窗顯示。
- 密碼、敏感、Disable 欄位先通知 host 清除組字與待提交文字、暫停學習，隨後放行按鍵。這些欄位的按鍵與文字不序列化。addon 不讀應用程式前文，也不記錄輸入內容。

## 測試

```sh
tools/zt check input-method
# 只重跑已建置的 Linux 原生測試：
ctest --test-dir platforms/linux/build --output-on-failure
```

根目錄指令先檢查共用 Qt host，再於 `.build-input-method` 建置與檢查原生 addon。測試所需的 Fcitx5 系統插件路徑取自已安裝的 `Fcitx5::Core` SDK，獨立於本專案的 `CMAKE_INSTALL_PREFIX`；自訂安裝 prefix 仍可使用系統的 xcb、D-Bus 與 classicui 插件。

`ZT_IME_HOST` CMake 參數可指定其他共用 host 執行檔，預設是版本庫的 `.build-qt/zhuyintype-desktop`。

| 測試 | 驗證範圍 |
|---|---|
| `fcitx-broker-transport` | Unix socket 實際傳輸、分段讀取、UTF-8／UTF-16、錯誤 id/context/型別、超長框、格式錯誤、逾時與斷線 |
| `fcitx-native` | 真正 Qt host 與 Fcitx InputContext：組字、提交、重設、欄位隔離、安全切換、Caps Lock、Shift 放開、共用快捷鍵、Qt 組字 fallback 與銷毀隔離 |
| `fcitx-application-delivery` | 獨立 Xvfb、D-Bus、Fcitx5 與 Qt 應用程式中的實際按鍵、候選字、滑鼠選字、安全欄位與 host 中斷後保留已知組字一次 |
| `fcitx-self-host-settings` | 真正設定視窗的搜尋 TextField，Qt Fcitx frontend 經 addon 回到同一個 Qt GUI 行程 |

傳輸測試的伺服器只用於邊界測試；原生與應用程式測試使用真正 Qt host。應用程式測試需 Qt6 Widgets 6.8 以上、`xvfb`、`xauth`、`xdotool`、`x11-utils`（`xwininfo`）、`dbus-x11`、`fcitx5-frontend-qt6`。`ZT_DELIVERY_CAPTURE_DIR` 可指定合成測試畫面的保存目錄。已登入的 X11 桌面可用 `DISPLAY=:10 QT_QUICK_BACKEND=software ZT_DELIVERY_SESSION=1 ZT_DELIVERY_INSTALLED=1 dbus-run-session -- bash platforms/linux/tests/delivery.sh platforms/linux/build .build-qt/zhuyintype-desktop` 測試已安裝 addon；桌面須已解除鎖定。加上 `ZT_DELIVERY_SELF_HOST=1` 可改測同一個 host 設定視窗。整合測試固定使用 Qt software renderer，與這台 RDP 測試 VM 的 host 設定相同。測試設定與個人資料使用暫存目錄，只結束測試建立的行程。結果與限制見 [VALIDATION.md](VALIDATION.md)。

## 平台範圍

本階段以 Xfce/X11 驗證共用候選字視窗。Wayland 的原生候選視窗需要 compositor 提供的輸入法 popup 介面與座標轉換，應先確認 Fcitx frontend 和 compositor 的協定接合方式；一般 Qt 頂層視窗的可見性不能證明定位、層級與焦點正確。autostart helper 會在原生 Wayland session 停止啟動，待原生協定整合後再開放。GTK、瀏覽器與沙箱應用程式須另外驗證。

框架介面依 [Fcitx5 官方開發文件](https://fcitx-im.org/wiki/Develop_an_simple_input_method)、[Wayland 說明](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland) 與已安裝的公開標頭實作。

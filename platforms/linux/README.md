# ZhuyinType：Linux 輸入法（Fcitx 5 addon）

Fcitx 5 addon 負責作業系統焦點、按鍵、組字與文字送達；輸入法服務 `zhuyintype`（`platforms/desktop`，GPUI）負責引擎、`InputController`、設定、快捷鍵與候選字視窗。addon 透過本機 Unix socket 使用 [IPC.md](../desktop/IPC.md) 的 broker 協定 v1，只依賴 Fcitx 5 與 nlohmann/json；引擎與介面在服務的行程中執行。

一般使用者安裝 `.deb` 或 `.rpm`，見 [Linux 套件](../desktop/packaging/linux/README.md)。這份文件寫 addon 本身的建置、行為與測試。

## 建置

在 GPU 主機或 CI 上建置，不在 Mac 上建置。addon 與服務一起由 `platforms/desktop/scripts/build.sh` 建置：

```sh
tools/zt --remote build desktop                 # 在 GPU 主機上：zhuyintype 與 addon（debug）
tools/zt --remote check desktop                 # 再加 cargo test 與 addon 的 CTest
```

單獨建置 addon 需要 Linux、C++20、CMake 3.24、Fcitx 5 5.1.9 以上、nlohmann/json 3.11 以上。Debian／Ubuntu 的開發套件：

```sh
sudo apt-get install cmake ninja-build libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev nlohmann-json3-dev
cmake -S platforms/linux -B build-addon -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
cmake --build build-addon
```

版本號取自 `platforms/desktop/Cargo.toml`（addon 的 `zhuyintype-addon.conf` 由 `data/zhuyintype-addon.conf.in` 產生）。`cmake --install` 依 Fcitx SDK 的位置安裝：addon 到 `FCITX_INSTALL_ADDONDIR`（Debian／Ubuntu `/usr/lib/x86_64-linux-gnu/fcitx5`，Fedora／openSUSE `/usr/lib64/fcitx5`），設定到 `/usr/share/fcitx5/addon/` 與 `/usr/share/fcitx5/inputmethod/`，授權到 `/usr/share/licenses/zhuyintype/`。套件放置相同的檔案。

本目錄透過 `find_package(nlohmann_json 3.11 REQUIRED)` 使用系統提供的 header-only 函式庫，未隨版本庫附帶其原始碼。上游 [nlohmann/json](https://github.com/nlohmann/json) 使用 [MIT 授權](https://github.com/nlohmann/json/blob/v3.11.3/LICENSE.MIT)；[第三方授權告知](LICENSES/nlohmann-json.txt) 包含 Niels Lohmann、Florian Loitsch 與 Bjoern Hoehrmann 的告知，隨套件安裝。ZhuyinType 的專案授權維持 AGPL-3.0。

## 啟用

套件安裝後，「ZhuyinType 設定」第一次開啟時以 `/usr/lib/zhuyintype/fcitx5-setup enable` 把 ZhuyinType 加入使用者的 Fcitx 5 輸入方式（見[套件說明](../desktop/packaging/linux/README.md#加入-fcitx-5-輸入方式fcitx5-setup)）。也可以在 `fcitx5-configtool` 的目前群組加入「ZhuyinType」。Fcitx 5 須是這位使用者的輸入法框架：Debian／Ubuntu 用 `im-config -n fcitx5`，Fedora 用 `imsettings-switch fcitx5`，再重新登入。

在文字欄位有焦點時，`fcitx5-remote -n` 應回報 `zhuyintype`。

## broker 與焦點

- 每個焦點週期使用獨立 context，先向舊 context 送 `blur`，再對新 context 送 `focus`。一般失焦的提交送回原欄位；`reset` 清除組字。欄位銷毀時關閉連線。
- 預設連到 `$XDG_RUNTIME_DIR/zhuyintype/ime-v1`；未設定時使用 `/tmp/zhuyintype-UID/ime-v1`。服務建立私人目錄與 user-only socket。`ZT_IME_SOCKET` 可覆寫完整端點，服務另接受 `--ime-socket PATH`。
- 預設端點沒有服務時，addon 啟動 `/usr/bin/zhuyintype --background`（CMake 的 `ZT_HOST_EXECUTABLE`），最多每 3 秒一次，以 `fcitx::startProcess` 脫離 Fcitx 的行程。這一次的按鍵照常放行，下一次按鍵連上服務。`ZT_IME_SOCKET` 指定的端點（測試）不啟動服務。
- 使用 newline-delimited UTF-8 JSON，每框上限 256 KiB，驗證版本、id、context、欄位型別、UTF-8 與 UTF-16 游標邊界。透過 `SO_PEERCRED` 檢查服務的 UID。
- 非阻塞 socket 的連線與單次請求各有 80 ms 的總時限；失敗關閉連線並放行目前按鍵。下一次按鍵重新連線，使用新 context，失敗的請求不重送。addon 在記憶體保留最後成功回應的組字；同一個仍聚焦且非安全的欄位遇到中斷時，先提交這份已知文字一次，再放行目前按鍵。secure、reset 與欄位銷毀直接丟棄快取。未確認的目前按鍵可能已被服務處理，其結果不重送。
- Fcitx event loop 只在欄位有焦點且已連線時，每 20 ms 送 `poll`；滑鼠選字的結果由下一次 `poll` 或按鍵回應送達。每個回應只套用到發出請求的 context。
- 送出 Qt 公開 key enum 與 modifier flags（協定沿用的數值），包含 Shift 左右、放開與重複事件；焦點後首個一般按鍵的 lock 狀態透過同 context 的 `focus.capsLock` 同步，Caps Lock 鍵本身由服務切換一次。Fcitx client preedit 游標從 UTF-16 轉成 UTF-8 位移；`focus.inlinePreedit` 回報欄位能力，未支援行內組字時由服務的候選字視窗顯示組字。Fcitx 原生面板保持空白，候選字一律由服務的視窗顯示。
- 密碼、敏感、Disable 欄位先通知服務清除組字與待提交文字、暫停學習，隨後放行按鍵。這些欄位的按鍵與文字不序列化。addon 只在欄位支援 SurroundingText 時讀取游標前的文字（〈前文〉），不記錄輸入內容。

## 測試

```sh
tools/zt --remote check desktop
# 只重跑已建置的 addon 測試（主機上）：
ctest --test-dir "$CARGO_TARGET_DIR/fcitx5-debug" --output-on-failure
```

| 測試 | 驗證範圍 | 服務 |
|---|---|---|
| `fcitx-broker-transport` | Unix socket 實際傳輸、分段讀取、UTF-8／UTF-16、錯誤 id/context/型別、超長框、格式錯誤、逾時與斷線 | 測試內的假服務 |
| `fcitx-native` | 真正的服務與 Fcitx InputContext：組字、提交、重設、欄位隔離、安全切換、Caps Lock、Shift 放開、共用快捷鍵、組字 fallback 與銷毀隔離 | `zhuyintype --background` |
| `fcitx-application-delivery` | 獨立 Xvfb、D-Bus、Fcitx 5 與 Qt 測試程式中的實際按鍵、候選字、滑鼠選字、安全欄位與服務中斷後保留已知組字一次 | `zhuyintype --background` |
| `fcitx-self-host-settings` | 服務自己的設定視窗文字欄位，經 Fcitx addon 回到同一個行程 | `zhuyintype`（設定視窗） |

需要服務的三項以 `ZT_IME_HOST`（CMake 參數，`build.sh` 會指定剛建置的 `zhuyintype`）啟動服務：`--background --ime-socket PATH --lexicon PATH`，設定與個人資料在暫存目錄。服務還沒有提供 broker 時（新的 Rust broker 在 `zt-core` 中撰寫），這三項回報「略過」（CTest 的 SKIP，結束碼 77）；`ZT_REQUIRE_HOST=1` 改為失敗。這三項原本以 Qt 服務 `zhuyintype-desktop --ime-host` 通過（[VALIDATION.md](VALIDATION.md)），broker 完成後須在新服務上重新驗證。

應用程式測試的測試程式用 Qt6 Widgets 6.8 以上（addon 本身不依賴 Qt），另需 `xvfb`、`xauth`、`xdotool`、`x11-utils`（`xwininfo`）、`dbus-x11`、`fcitx5-frontend-qt6`；缺少時不登記這兩項。`ZT_DELIVERY_CAPTURE_DIR` 可指定合成測試畫面的保存目錄。測試只結束自己建立的行程。

## 平台範圍

本階段以 X11 驗證。Wayland 的原生候選視窗需要 compositor 提供的輸入法 popup 介面與座標轉換，應先確認 Fcitx frontend 和 compositor 的協定接合方式；一般頂層視窗的可見性不能證明定位、層級與焦點正確。GTK、瀏覽器與沙箱應用程式須另外驗證。

框架介面依 [Fcitx5 官方開發文件](https://fcitx-im.org/wiki/Develop_an_simple_input_method)、[Wayland 說明](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland) 與已安裝的公開標頭實作。

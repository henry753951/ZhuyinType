# Linux 原生整合驗證

日期：2026-10-29。Xfce／X11 的實際使用者 profile、登入啟動與正式服務已設定，真實應用程式送達、共用 Qt 候選字視窗、滑鼠操作與同 host 設定欄位均通過。

## 環境與建置

- 新測試 VM：Ubuntu 26.04、Fcitx5 5.1.19、Qt 6.10.2、Xfce/X11，RDP `DISPLAY=:10`。
- CMake 4.2.3、GCC 15.2.0；Ubuntu 官方 `nlohmann-json3-dev`，實際標頭版本 3.11.3。
- 原生建置：`platforms/linux/build`；共用 host：`.build-qt/zhuyintype-desktop`，父工作提供的 11:48 功能快照，包含啟動就緒、Caps Lock 與 `inlinePreedit` 修正。
- addon 與測試以 `-Wall -Wextra -Wpedantic -Werror` 建置通過。`ldd libzhuyintype.so` 僅含 Fcitx5、C++ 與系統函式庫，沒有 Qt 或 Rust 引擎相依。
- 只修改 `platforms/linux/**`。沒有修改 Rust 引擎、共用 Qt UI、字型、排版、其他代理的檔案或 GPU 主機；沒有提交。

## 實際桌面部署

已安裝：

- `/usr/lib/x86_64-linux-gnu/fcitx5/libzhuyintype.so`
- `/usr/share/fcitx5/addon/zhuyintype.conf`
- `/usr/share/fcitx5/inputmethod/zhuyintype.conf`
- `/usr/share/licenses/zhuyintype/nlohmann-json.txt` 與 `AGPL-3.0.txt`
- `~/.local/bin/zhuyintype-ime-host`
- `~/.config/autostart/zhuyintype-ime-host.desktop`

實際 `ztdev` 使用者設定：

- `~/.config/fcitx5/profile` 的 `DefaultIM=zhuyintype`；同群組保留原有 `keyboard-us`，並加入 `zhuyintype`。
- 修改前的 profile 備份：`~/setup/ime-config-backup/fcitx5-profile`，權限 0600；備份目錄 0700。
- `im-config -n fcitx5` 已設定 `~/.xinputrc` 的 `run_im fcitx5`，原先沒有此檔案。Fcitx5 由既有 `im-config` X11 登入流程啟動，host 由使用者 autostart 啟動。
- 已結束此使用者原有的 IBus daemon，啟動 Fcitx5；其他輸入法套件與鍵盤項目保留。GNOME／Wayland 全域設定維持原狀。
- 正式 Fcitx5 與 host 留在桌面執行。最後在設定搜尋欄位有焦點時，`fcitx5-remote -n` 回報 `zhuyintype`，`fcitx5-remote` 回報 `2`。
- 正式預設端點 `/run/user/1000/zhuyintype/ime-v1` 已實際連線使用，目錄與 socket 權限均為 0700。測試程式也支援獨立 `ZT_IME_SOCKET`。

此次檢查包含登入設定檔與實際服務啟動，沒有強制登出使用者來重跑登入流程。

## 自動測試

最終 `ctest --test-dir platforms/linux/build --output-on-failure`：4/4 通過，共 5.47 秒。整合測試固定使用 Qt software renderer，與實際 RDP host 部署一致。

| 測試 | 結果 | 內容 |
|---|---|---|
| `fcitx-broker-transport` | 通過，0.17 秒 | 真實 Unix socket 分段傳輸、錯誤 id/context/型別、UTF-8／UTF-16、NUL、錯誤 JSON、額外框、256 KiB 上限、逾時與斷線 |
| `fcitx-native` | 通過，0.23 秒 | 真實 Qt host 與 Fcitx InputContext；組字、提交、重設、焦點隔離、重新連線、安全切換、Caps Lock、Shift 放開、快捷鍵與 active context 銷毀 |
| `fcitx-application-delivery` | 通過，3.25 秒 | 實際按鍵與 Qt 應用程式；共用 Qt popup、滑鼠選字經 poll 送達、安全欄位、已有組字時終止 host 後保留已知文字一次，再放行英數 |
| `fcitx-self-host-settings` | 通過，1.83 秒 | 同一個 Qt GUI 行程中的設定 TextField，經 Qt Fcitx frontend → Fcitx addon → 同一 host，精確收到合成字串「你好」 |

補充不變條件：

- 安全欄位內切換 Caps Lock 後，在相同 context 回到一般欄位，首個一般按鍵重新同步 lock 狀態。此案例先重現失敗，修正後通過。
- `inlinePreedit=false` 時，真實 broker 回應保有組字並標示 Qt popup 可見；Fcitx native/client preedit 面板保持空白，提交仍正常。能力在相同 context 改變時也有測試。
- 快取僅保存最後成功回應的組字。host 中斷後只向同一個仍聚焦且非安全的欄位提交一次；secure、reset、銷毀丟棄快取，未確認按鍵不重送。
- 成功回應可省略空 `error`；有提供時驗證 UTF-8 字串。這與共用 host 的 snapshot 一致。

200 次 modifier 事件的 addon＋broker 往返：p50 0.059 ms、p95 0.075 ms、p99 0.243 ms。這是小型傳輸量測，未包含實際組字計算，也沒有舊版基準。

## 根目錄 CLI 回歸檢查

`tools/zt check input-method` 使用 `.build-input-method` 與預設安裝 prefix `/usr/local`。原本 harness 將 `FCITX_INSTALL_ADDONDIR` 當成系統插件位置，導致到 `/usr/local/lib/fcitx5` 尋找 xcb、D-Bus、classicui，兩項應用程式測試無法啟動框架。

已改由 CMake 匯入目標 `Fcitx5::Core` 的實際函式庫目錄產生系統插件路徑。本 VM 的兩種建置都使用 `/usr/lib/x86_64-linux-gnu/fcitx5`，原生 addon 的安裝 prefix 維持原設定。

重新執行完整 root 指令成功（exit 0）：共用 Qt 9/9，33.87 秒；Linux 原生 4/4，5.44 秒。紀錄為 `~/setup/ime-root-check-fixed.log`。此輪只修改 Linux CMake 與文件，桌面正式 Fcitx5／host 行程與 profile 保留。

## 實際桌面與詞庫

已以安裝在系統路徑的 addon、實際 `ztdev` profile、預設端點與 `data/build/lexicon.tsv`，通過正式桌面服務測試：

- 候選字由真實 Qt host 顯示，幾何為 `(215,242,204,315)`，應用程式 native caret 為 `(215,238)`，位於游標下方 4 px。小型 fixture 的視窗為 `(215,242,204,119)`。
- 點選前文字欄位保持焦點；滑鼠選取不同候選字後，組字經 poll 更新，Enter 將同一個候選字送到原應用程式。
- 密碼欄位清除組字與放行按鍵，沒有提交先前組字。
- 正式 host 的「快捷鍵」搜尋 TextField 精確收到「你好」。全流程 430 ms，包含 300 ms 固定等待與每鍵 5 ms 注入間隔，屬功能檢查。
- 正式桌面測試保留 host 與 Fcitx5 執行；測試自己的隔離 host／Fcitx 行程均已清理。

真實桌面畫面與紀錄：

- `~/setup/ime-linux-live-profile/candidate-window.png`
- `~/setup/ime-linux-live-profile/mouse-pick-delivered.png`
- `~/setup/ime-linux-live-profile/secure-pass-through.png`
- `~/setup/ime-linux-live-profile/self-host-delivered.png`
- `~/setup/ime-live-profile.log`、`~/setup/ime-live-selfhost.log`

畫面只包含測試用合成輸入。測試改用 `xwininfo` 的 client 絕對位置；Xfce 下 `xdotool getwindowgeometry` 的裝飾座標曾造成點擊偏移，已排除。

## renderer 與範圍

此 VM 的預設 3D renderer 在第一次顯示候選視窗時，有間歇性超過 80 ms 才處理下一個 key／poll 的情況，Xvfb 也曾出現。暫時診斷只記操作種類與送收長度，確認請求送完但期限內未收到回應；診斷碼已移除，正式 addon 不記錄這些事件或輸入。

以 `QT_QUICK_BACKEND=software` 在 RDP 連續三次通過完整滑鼠測試，移除診斷碼後，正式服務與同 host 設定欄位也通過。VM 的 host launcher 已固定這個行程的 Qt 設定，一般安裝未指定時沿用 Qt 預設 renderer。Qt [software adaptation](https://doc.qt.io/qt-6/qtquick-visualcanvas-adaptations-software.html) 繪製同一份場景，其 shader 與縮放限制依官方文件。

斷線時保留最後成功回應的文字。未回應的目前按鍵或滑鼠動作可能已由 host 處理，native 無法確認其結果；目前按鍵放行，不重送未確認請求。已知組字的保留不代表能重建未確認的引擎狀態。

Wayland 的原生 popup 協定、GTK、瀏覽器與沙箱應用程式尚未驗證。一般 Qt 頂層視窗不能視為 Wayland overlay 已完成；host helper 目前停止原生 Wayland 啟動。

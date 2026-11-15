# Linux 套件驗證

日期：2026-11-16。版本 0.5.3（`platforms/desktop/Cargo.toml`）。在 GPU 主機（Ubuntu 26.04.1）上以 `scripts/build.sh linux --package` 建置，Debian 13 建置映像（`rust:1.98.0-slim-trixie` 加上開發套件；glibc 2.41、Fcitx 5.1.12、GCC 14）編譯，nfpm 2.48.0 打包。`tools/zt --remote --pull dist/desktop build desktop --package` 從 Mac 執行同一個流程（含引擎資料下載，約 3.5 分鐘），取回的檔案與 `SHA256SUMS-linux` 相符。

此時的設定程式只有視窗與側邊欄，broker（`--background`、`--ime-socket`）尚未完成，所以這份紀錄驗證的是套件本身：安裝、相依、檔案、Fcitx 登記、移除與視窗啟動。輸入法的實際輸入要等 broker 完成後再以 `platforms/linux` 的測試驗證。

## 套件

| 檔案 | 大小 | 安裝後 |
|---|---|---|
| `zhuyintype_0.5.3_amd64.deb`（xz） | 62.1 MB | 123 MiB |
| `zhuyintype-0.5.3.x86_64.rpm`（zstd） | 69.3 MB | 128 MiB |

內容：`/usr/bin/zhuyintype` 37 MB（release、strip）；addon 225 KB；引擎資料 92 MB（`lexicon.bin` 17.6 MB、`channel.bin` 0.9 MB、`lm/lm.bin` 37.6 MB 與署名清單 `articles.tsv.gz` 27.5 MB、`lm_chat/lm.bin` 7.3 MB 與署名檔 0.75 MB）；8 種大小的圖示；16 個授權檔（`Rust-crates.txt` 涵蓋 562 個 Rust 套件，其中 26 個未附授權檔的依 Cargo.toml 的 MIT／Apache-2.0 補上全文）。

由執行檔算出的相依（deb）：`libc6 (>= 2.39)`、`libfcitx5core7 (>= 5.1.12)`、`libfcitx5utils2 (>= 5.1.12)`、`libgcc-s1`、`libstdc++6 (>= 13.1)`、`libxcb1`、`libxkbcommon0`、`libxkbcommon-x11-0`，另加 `fcitx5 (>= 5.1.9)`、`libvulkan1`、`libegl1`、`libwayland-client0`、`libwayland-egl1`、`fontconfig`；建議 `fonts-noto-cjk`、`mesa-vulkan-drivers | vulkan-icd`。rpm 以 soname 與符號版本表示同一組（最高 `libc.so.6(GLIBC_2.39)(64bit)`、`libstdc++.so.6(GLIBCXX_3.4.32)(64bit)`）。在 Ubuntu 26.04 主機上直接編譯的執行檔需要 glibc 2.43（`asinf` 等數學函式的新符號版本），這是改在 Debian 13 映像中編譯的原因。

## 乾淨容器中的安裝與移除（`verify.sh`）

| 映像 | 安裝 | 檔案與函式庫 | Fcitx 5 | 移除 |
|---|---|---|---|---|
| `debian:13` | apt，`--no-install-recommends`，相依全部解析 | 通過 | `fcitx5-setup enable` 經 D-Bus 加入執行中的 Fcitx：群組變成 `keyboard-us`、`zhuyintype`，addon 已載入 | `apt purge` 後沒有留下任何檔案 |
| `ubuntu:26.04` | 同上 | 通過 | 同上 | 同上 |
| `fedora:latest`（Fedora 45） | dnf，不裝弱相依 | 通過 | — | `dnf remove` 後沒有留下任何檔案 |
| `opensuse/tumbleweed` | zypper，`--allow-unsigned-rpm` | 通過 | — | `zypper remove` 後沒有留下任何檔案 |

這四個映像的結果來自修正 lint 項目之前的建置（內容相同，差在 changelog、addon 的強化旗標與 rpm 中 addon 的權限、移除腳本改讀 `/proc`）。最後的建置在 Debian 13 重新安裝、啟動與 `apt purge`（下節）並通過 lint；Ubuntu、Fedora、openSUSE 的重跑因主機連線中斷尚未完成，下次執行 `verify.sh` 時補上。

「檔案與函式庫」：執行檔、addon、兩個 Fcitx 設定、四個資料檔、manifest、選單項目、autostart、圖示與授權都在預期位置；`ldd` 檢查執行檔與 addon 沒有找不到的函式庫。

`fcitx5-setup` 另外在 Debian 13 測試（Fcitx 沒有執行時）：沒有設定檔時建立含 `keyboard-us` 與 `zhuyintype` 的設定；已有設定檔時在第一個群組最後加入一項並留下 `profile.zhuyintype-backup`；之後啟動的 Fcitx 5 讀到 `keyboard-us`、`zhuyintype` 兩項；再執行一次不做變更；以 root 執行時拒絕（結束碼 3）。

## 設定視窗

`verify.sh --gui`：Debian 13 容器安裝套件（含建議套件）、Xvfb 與 Mesa lavapipe，啟動 `zhuyintype` 後出現視窗並截圖，側邊欄與標題的中文字以 Noto CJK 顯示。接著 `apt purge`，執行中的 `zhuyintype` 在移除時結束（`scripts/preremove.sh`）。

## lintian 與 rpmlint

lintian（Debian 13）沒有錯誤。剩下的警告與說明：

- `no-manual-page`：還沒有 man page（程式的命令列參數尚未定案）。
- `initial-upload-closes-no-bugs`：套件不經 Debian 封存庫發布，沒有對應的 bug。
- `package-contains-timestamped-gzip`：`lm/articles.tsv.gz` 是資料發布檔的原樣。
- `arch-dep-package-has-big-usr-share`：引擎資料與執行檔放在同一個套件；之後可拆成架構無關的 `zhuyintype-data`。
- `hardening-no-fortify-functions`：Rust 執行檔沒有使用 glibc 的 fortify 函式；addon 以 `dpkg-buildflags`（hardening=+all）編譯，已無此項。
- `package-contains-documentation-outside-usr-share-doc`：授權檔依需求放在 `/usr/share/licenses/zhuyintype`。

rpmlint（Fedora）沒有錯誤；警告為 `no-manual-page-for-binary` 與 `non-coherent-filename`（rpm 慣例的檔名是 `zhuyintype-0.5.3-1.x86_64.rpm`，這裡依發布需求用 `zhuyintype-0.5.3.x86_64.rpm`）。

## addon 測試

`build.sh linux --test`（主機原生 debug 建置）：`cargo test` 通過；CTest 的 `fcitx-broker-transport` 通過，`fcitx-native` 因服務還沒有 broker 而略過（結束碼 77）。應用程式送達測試需要 Qt6 Widgets、xdotool 等，主機上沒有，未登記。

## 尚未驗證

- 真實桌面登入後的 autostart、Fcitx 5 以 addon 啟動服務、實際打字（等 broker）。
- Wayland、GNOME／KDE 的輸入法設定流程（`fcitx5-setup framework`）。
- 套件升級（同名新版本覆蓋安裝）。

# Linux 套件驗證

日期：2026-11-16。版本 0.5.3（`platforms/desktop/Cargo.toml`），原始碼含 `zt-core` 的 broker（`gpui-desktop` e3d465eb 之後）。在 GPU 主機（Ubuntu 26.04.1）上以 `scripts/build.sh linux --package` 建置，Debian 13 建置映像（`rust:1.98.0-slim-trixie` 加上開發套件；glibc 2.41、Fcitx 5.1.12、GCC 14）編譯，nfpm 2.48.0 打包。`tools/zt --remote --pull dist/desktop build desktop --package` 從 Mac 執行同一個流程（含引擎資料下載，約 3.5 分鐘），取回的檔案與 `SHA256SUMS-linux` 相符。

## 套件

| 檔案 | 大小 | 安裝後 |
|---|---|---|
| `zhuyintype_0.5.3_amd64.deb`（xz） | 65.1 MB | 136 MiB |
| `zhuyintype-0.5.3.x86_64.rpm`（zstd） | 72.7 MB | 136 MiB |

內容：`/usr/bin/zhuyintype`（release、strip，含 broker 後 46 MB）；addon 225 KB；引擎資料 92 MB（`lexicon.bin` 17.6 MB、`channel.bin` 0.9 MB、`lm/lm.bin` 37.6 MB 與署名清單 `articles.tsv.gz` 27.5 MB、`lm_chat/lm.bin` 7.3 MB 與署名檔 0.75 MB）；8 種大小的圖示；16 個授權檔（`Rust-crates.txt` 涵蓋 562 個 Rust 套件，其中 26 個未附授權檔的依 Cargo.toml 的 MIT／Apache-2.0 補上全文）。

由執行檔算出的相依（deb）：`libc6 (>= 2.39)`、`libfcitx5core7 (>= 5.1.12)`、`libfcitx5utils2 (>= 5.1.12)`、`libgcc-s1`、`libstdc++6 (>= 13.1)`、`libxcb1`、`libxkbcommon0`、`libxkbcommon-x11-0`，另加 `fcitx5 (>= 5.1.9)`、`libvulkan1`、`libegl1`、`libwayland-client0`、`libwayland-egl1`、`fontconfig`；建議 `fonts-noto-cjk`、`mesa-vulkan-drivers | vulkan-icd`。rpm 以 soname 與符號版本表示同一組（最高 `libc.so.6(GLIBC_2.39)(64bit)`、`libstdc++.so.6(GLIBCXX_3.4.32)(64bit)`）。在 Ubuntu 26.04 主機上直接編譯的執行檔需要 glibc 2.43（`asinf` 等數學函式的新符號版本），這是改在 Debian 13 映像中編譯的原因。

## 乾淨容器中的安裝與移除（`verify.sh`）

| 映像 | 安裝 | 檔案與函式庫 | Fcitx 5 | 移除 |
|---|---|---|---|---|
| `debian:13` | apt，`--no-install-recommends`，相依全部解析 | 通過 | `fcitx5-setup enable` 經 D-Bus 加入執行中的 Fcitx：群組變成 `keyboard-us`、`zhuyintype`，addon 已載入 | `apt purge` 後沒有留下任何檔案 |
| `ubuntu:26.04` | 同上 | 通過 | 同上 | 同上 |
| `fedora:latest`（Fedora 45） | dnf，不裝弱相依 | 通過 | — | `dnf remove` 後沒有留下任何檔案 |
| `opensuse/tumbleweed` | zypper，`--allow-unsigned-rpm`（容器中其他套件的 scriptlet 失敗時 zypper 回傳 107，`verify.sh` 以 `rpm -q` 與檔案檢查判定） | 通過 | — | `zypper remove` 後沒有留下任何檔案 |

「檔案與函式庫」：執行檔、addon、兩個 Fcitx 設定、四個資料檔、manifest、選單項目、autostart、圖示與授權都在預期位置；`ldd` 檢查執行檔與 addon 沒有找不到的函式庫。

`fcitx5-setup` 另外在 Debian 13 測試（Fcitx 沒有執行時）：沒有設定檔時建立含 `keyboard-us` 與 `zhuyintype` 的設定；已有設定檔時在第一個群組最後加入一項並留下 `profile.zhuyintype-backup`；之後啟動的 Fcitx 5 讀到 `keyboard-us`、`zhuyintype` 兩項；再執行一次不做變更；以 root 執行時拒絕（結束碼 3）。

## 實際輸入（`verify.sh --typing`）

Debian 13 容器安裝 `.deb`（使用套件的 `/usr/bin/zhuyintype`、系統 addon 目錄與 `/usr/share/zhuyintype/data` 的真實詞庫與語言模型），在 Xvfb、D-Bus 與 Fcitx 5.1.12 中執行 `platforms/linux/tests` 的兩個應用程式測試：

- Qt 測試程式：`su3cl3` 組成「你好」，Enter 送出；`su3` 加 ↓ 開啟候選字視窗（244×315，位於游標下方），滑鼠選字後 Enter 送出所選的字；密碼欄位放行按鍵、不送出組字；結束服務後送出已知組字一次，之後的英數直接放行。
- 服務自己的設定視窗（快捷鍵頁的搜尋欄位）：經 XIM、Fcitx addon 回到同一個服務，收到「你好」。

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

`build.sh linux --test`（`tools/zt check desktop`）在 Debian 13 測試映像中：`cargo test` 全部通過（含 `zt-core` 讀真實資料的測試），CTest 4/4 通過，不略過任何一項：`fcitx-broker-transport`、`fcitx-native`、`fcitx-application-delivery`、`fcitx-self-host-settings`；三項整合測試連續再跑 5 次都通過。

## 2026-11-18：0.6.1 的審查與修正之後

版本 0.6.1（`polish/review-linux` 分支），同樣以 `scripts/build.sh linux --package` 在 Debian 13 映像中建置。

- `verify.sh` 在 `debian:13`、`ubuntu:26.04`、`fedora:latest`（Fedora 45）、`opensuse/tumbleweed` 都通過，並以 `ZT_VERIFY_FROM` 從 0.6.0 的套件升級：升級後版本與檔案正確，管理員改過的 `/etc/xdg/autostart/zhuyintype.desktop`（加上 `Hidden=true`）保留；deb 以更新程式的方式安裝（沒有終端機、`--force-confdef --force-confold`）。移除後連更新程式的下載資料夾 `/var/cache/zhuyintype` 也不留下。
- 沒有終端機時，dpkg 在設定檔詢問處會失敗：以兩個測試套件在 `debian:13` 重現（`end of file on stdin at conffile prompt`，套件停在 unpacked），加上這兩個選項後升級完成並保留改過的檔案。
- `verify.sh --typing`：Qt 測試程式、服務自己的設定欄位，以及焦點在組字中途移到同一視窗的另一欄位、另一個視窗（Qt 與 GTK 3）都通過。
- `verify.sh --lint`：lintian 沒有錯誤（`libonnxruntime.so.1` 在 deb 中改為 0644）；rpmlint 報 `files-duplicated-waste`（資料與授權資料夾中的幾個相同授權檔，共 0.78 MB）與上面列過的警告。
- 設定視窗以一般使用者執行、Fcitx 5 沒有執行時（容器中沒有輸入法框架），頁面上方說明並有「改用 Fcitx 5」；按下後 `im-config -n fcitx5` 寫入 `~/.xinputrc`，說明改為「登出再登入一次」。openSUSE 容器中 `fcitx5-setup framework` 在 `~/.i18n` 設定 `INPUT_METHOD=fcitx5`，`status` 回報 `fcitx5-next-login`。
- polkit 規則：Fedora 容器中的 polkitd 載入規則；沒有登入工作階段的使用者啟動更新服務得到 `auth_admin`。把工作階段條件換成 true 的副本確認規則只允許 `zhuyintype-update.service` 的 `start`（`stop` 與其他服務仍要 `auth_admin`）。

## 尚未驗證

- 真實桌面登入後的 autostart 與 Fcitx 5 的啟動順序：GPUI 只在啟動時尋找 XIM 伺服器，所以 `--background` 在 Fcitx 5 是輸入法時先等 `fcitx5` 行程出現（最多 15 秒）再啟動介面，加入輸入方式也在這之後；尚未在真實桌面登入時驗證。
- Wayland 工作階段（GNOME、KDE）中由 Fcitx 顯示候選字清單；GNOME 上 Fcitx 的清單位置需要 Kimpanel 擴充功能。
- Fedora 的 `imsettings-switch fcitx5`（需要桌面工作階段的 imsettings daemon）。
- 真正的 systemd 與 polkit 下由使用者啟動更新服務、以 apt／dnf／zypper 安裝新版本。

# Linux 套件（.deb、.rpm）

ZhuyinType 的 Linux 版以一般的發行版套件安裝：Debian／Ubuntu 用 `.deb`，Fedora／openSUSE 用 `.rpm`。兩種套件由同一份內容產生，以 [nfpm](https://nfpm.goreleaser.com/) 打包。

## 安裝與移除（使用者）

```sh
sudo apt install ./ZhuyinType-Linux-amd64.deb        # Debian 13、Ubuntu 25.04 以後
sudo dnf install ./ZhuyinType-Linux-x86_64.rpm       # Fedora 42 以後
sudo zypper install ./ZhuyinType-Linux-x86_64.rpm    # openSUSE Tumbleweed

sudo apt remove zhuyintype                         # 或 sudo apt purge zhuyintype
sudo dnf remove zhuyintype
```

在檔案管理員中按兩下套件檔，也會以系統的軟體中心安裝。相依套件（Fcitx 5、Vulkan、字型等）由 apt／dnf 自動安裝。安裝後：

1. 重新登入一次。登入時 `/etc/xdg/autostart/zhuyintype.desktop` 在背景啟動輸入法服務（`zhuyintype --background`）；服務沒有執行時，Fcitx 5 的 addon 在第一次使用 ZhuyinType 時也會啟動它。
2. 登入後服務把 ZhuyinType 加入 Fcitx 5 的輸入方式（原有的保留，只加一次）。Fcitx 5 不是這個工作階段的輸入法框架時（GNOME 預設用 IBus），「ZhuyinType 設定」的頁面上方會說明，按「改用 Fcitx 5」（`fcitx5-setup framework`）後再登出、登入一次。

移除套件會刪除它安裝的所有檔案，並結束執行中的服務。使用者自己的設定與記住的字詞（`~/.config/zhuyintype`、`~/.local/share/zhuyintype`）保留在家目錄，需要時自行刪除。`apt remove` 會留下 `/etc/xdg/autostart/zhuyintype.desktop`（設定檔，`apt purge` 才刪除）；它有 `TryExec`，程式不在時桌面不會執行它。

## 套件內容

| 路徑 | 內容 |
|---|---|
| `/usr/bin/zhuyintype` | 設定介面、候選字視窗與輸入法服務（GPUI，一個行程） |
| `/usr/lib/x86_64-linux-gnu/fcitx5/libzhuyintype.so`（deb）<br>`/usr/lib64/fcitx5/libzhuyintype.so`（rpm） | Fcitx 5 addon（`platforms/linux`），經 [IPC.md](../../IPC.md) 的通訊端連到服務 |
| `/usr/share/fcitx5/addon/zhuyintype.conf`、`/usr/share/fcitx5/inputmethod/zhuyintype.conf` | addon 與輸入方式的登記 |
| `/usr/share/zhuyintype/data/` | `lexicon.bin`、`channel.bin`、`lm/lm.bin`、`lm_chat/lm.bin`（`tools/zt data compile` 產生的 mmap 檔，與 macOS App 相同），各模型的 `NOTICE.md` 與署名檔，`manifest.json`（版本與每個檔案的 SHA-256） |
| `/usr/share/applications/zhuyintype.desktop` | 應用程式選單的「ZhuyinType 設定」 |
| `/etc/xdg/autostart/zhuyintype.desktop` | 登入時啟動服務 |
| `/usr/share/icons/hicolor/*/apps/zhuyintype.png` | 16 到 256 px 的圖示（由 `docs/assets/icon.png` 產生） |
| `/usr/lib/systemd/system/zhuyintype-update.service` | 更新程式（root，`zhuyintype --system-update`）：驗證簽章過的更新清單與套件，用 apt、dnf 或 zypper 安裝（[DESKTOP.md](../../../../docs/product/DESKTOP.md)〈更新〉） |
| `/usr/share/polkit-1/rules.d/50-zhuyintype-update.rules` | 讓目前在電腦前的使用者不輸入密碼就能啟動上面的服務，此外不能對它做任何事 |
| `/usr/lib/zhuyintype/fcitx5-setup` | 把 ZhuyinType 加入使用者 Fcitx 5 輸入方式的小工具（見下節） |
| `/usr/share/licenses/zhuyintype/` | AGPL-3.0 全文、NOTICE、資料來源（SOURCES.md、sources.json）、資料授權、nlohmann/json 告知、`Rust-crates.txt`（連結進執行檔的每個 Rust 套件與授權全文） |
| `/usr/share/doc/zhuyintype/` | README；deb 另有 `copyright`（DEP-5）與 `changelog.Debian.gz` |

Core ML 神經模型只用於 macOS，Linux 套件不含。

相依：

- 執行檔與 addon 連結的函式庫：deb 由 `dpkg-shlibdeps` 算出（含最低版本），rpm 列出每個 soname 與符號版本（例如 `libc.so.6(GLIBC_2.34)(64bit)`），兩者都在建置時由實際的執行檔產生。
- 執行時載入的函式庫：`libvulkan1`、`libegl1`、`libwayland-client0`、`libwayland-egl1`、`fontconfig`，以及 `fcitx5 (>= 5.1.9)`。
- 建議安裝：`fonts-noto-cjk`（rpm：`font(notosanscjktc)`）與 Vulkan 驅動程式 `mesa-vulkan-drivers`。

## 套件對 `zhuyintype` 的要求

套件與 addon 依賴程式的這些行為（UI 與 `zt-core` 實作時須符合）：

- `zhuyintype --background`：只啟動輸入法服務，不開視窗；已有服務在執行時安靜結束（結束碼 0）。登入時的 autostart 與 addon 都這樣啟動它。
- `zhuyintype`（沒有參數）：開啟設定視窗；已有服務在執行時請它顯示設定。
- `--ime-socket PATH` 與 `--lexicon PATH`：測試用的隔離端點與詞庫（`platforms/linux/tests`）。
- 引擎資料：先看 `ZT_DATA_DIR`，再看執行檔旁的 `../share/zhuyintype/data`（套件安裝為 `/usr/share/zhuyintype/data`），`lexicon.bin` 旁有 `lm/`、`lm_chat/`、`channel.bin`。
- 服務第一次啟動時（登入時的 `--background` 先等 Fcitx 5 啟動，最多 15 秒）執行 `/usr/lib/zhuyintype/fcitx5-setup status`，需要時執行 `enable`，見下節。加入後在個人資料夾留下 `fcitx5-added`，之後不再加入：使用者自己從 Fcitx 5 移除 ZhuyinType 後就保持移除。

## 維護腳本

兩個，都只在移除（不是升級）時執行：

- `scripts/preremove.sh`：移除前結束執行 `/usr/bin/zhuyintype` 的行程（只看 `/proc/*/exe`，不需要其他工具）。
- `scripts/postremove.sh`：移除後刪除更新程式的下載資料夾 `/var/cache/zhuyintype`（套件管理員不知道它）。更新程式裝完後也會刪除下載的套件。

安裝時不執行任何指令，也不修改使用者的設定；圖示與應用程式選單的快取由發行版的觸發程序更新。

更新程式用 apt 安裝時，等候其他套件管理員釋放鎖定（最多 10 分鐘），並保留管理員改過的設定檔（`--force-confdef --force-confold`）：它沒有終端機，dpkg 在設定檔的詢問處會失敗，套件停在「unpacked」。

## 加入 Fcitx 5 輸入方式（`fcitx5-setup`）

套件腳本以 root 執行，不能也不應該修改每位使用者的 Fcitx 5 設定。輸入法服務第一次啟動時（登入後，或第一次開啟「ZhuyinType 設定」）以使用者身分執行：

```sh
/usr/lib/zhuyintype/fcitx5-setup status      # fcitx5=… running=… addon=… profile=… framework=…，一行一項
/usr/lib/zhuyintype/fcitx5-setup enable      # 加入目前的輸入方式群組，保留原有項目
/usr/lib/zhuyintype/fcitx5-setup framework   # 把 Fcitx 5 設為這位使用者的輸入法框架（im-config 或 imsettings），下次登入生效
```

- Fcitx 5 正在啟動（行程已在、還沒有出現在 D-Bus 上）時，先等它最多 10 秒。
- Fcitx 5 執行中時，`enable` 經 D-Bus（`busctl --user`）請 Fcitx 加入 ZhuyinType，由 Fcitx 自己存檔；Fcitx 是在套件安裝前啟動、還沒有載入 addon 時，先請它重新啟動一次。
- Fcitx 5 沒有執行時，直接在 `~/.config/fcitx5/profile` 的第一個群組最後加入一項（原檔先備份為 `profile.zhuyintype-backup`）；沒有這個檔案時建立一個，含系統的鍵盤配置與 ZhuyinType。
- 結束狀態：0 完成或不需要變更，1 失敗，2 用法錯誤，3 這台機器做不到（原因寫在 stderr）。

## 建置（GPU 主機或 CI）

在 Mac 上不建置。從 Mac：

```sh
tools/zt --remote --pull dist/desktop build desktop --package
# 或
platforms/desktop/scripts/remote.sh scripts/build.sh linux --package
platforms/desktop/scripts/remote.sh --fetch zhuyintype/work/desktop-<checkout>/dist/desktop dist/desktop
```

`scripts/build.sh linux --package` 依序：

1. 以 `Dockerfile`（Debian 13：glibc 2.41、Fcitx 5.1.12）建立建置映像，標籤含 Dockerfile 與 `rust-toolchain.toml` 的雜湊，內容不變時沿用。映像中編譯 `zhuyintype`（release）與 addon，並以 `dpkg-shlibdeps`、`readelf` 算出相依、以 `scripts/rust-licenses.py` 收集 Rust 套件授權。docker 與 podman 都可以；`--native` 改在這台機器上編譯（套件會需要這台機器的 glibc 版本，只供測試）。
2. 缺少引擎資料時執行 `tools/zt data fetch lexicon lm lm_chat`，再以 `tools/zt data compile` 編譯到 TSV 旁邊（來源沒變時略過；開發建置與 `zt-core` 的測試也從那裡讀）。
3. 整理套件內容到 `<target>/builder-debian13/package-stage`，以 ImageMagick 產生圖示。
4. 把 `nfpm.yaml` 的 `# @shlibs@` 換成算出的相依、`# @licenses@` 換成每個授權檔，寫出 `changelog.yml`（nfpm 依它產生 deb 的 `changelog.Debian.gz` 與 rpm 的 `%changelog`），以 nfpm 產生 `dist/desktop/zhuyintype_<版本>_amd64.deb`、`zhuyintype-<版本>.x86_64.rpm` 與 `SHA256SUMS-linux`，並確認兩個套件都含執行檔、addon 與詞庫。addon 以發行版的強化旗標編譯（`dpkg-buildflags`，hardening=+all）。

nfpm 的版本與 SHA-256 固定在 `build.sh`，第一次使用時下載到 `~/zhuyintype/toolchains`（`ZT_TOOLCHAINS`）；建置映像的 cargo 快取也在那裡。版本號取自 `platforms/desktop/Cargo.toml`。建置映像第一次建立時下載 Debian 套件，主機離 `deb.debian.org` 較遠時可加 `ZT_DEBIAN_MIRROR=http://free.nchc.org.tw/debian`。

## 驗證

```sh
platforms/desktop/scripts/remote.sh packaging/linux/verify.sh ../../dist/desktop             # 安裝與移除
platforms/desktop/scripts/remote.sh packaging/linux/verify.sh ../../dist/desktop --lint
platforms/desktop/scripts/remote.sh packaging/linux/verify.sh ../../dist/desktop --gui ../../dist/desktop-shots
platforms/desktop/scripts/remote.sh packaging/linux/verify.sh ../../dist/desktop --typing   # 先 scripts/build.sh linux --test
```

`verify.sh` 在乾淨的容器（Debian 13、Ubuntu 26.04、Fedora、openSUSE Tumbleweed）中安裝套件（deb 照更新程式的方式：沒有終端機、保留改過的設定檔），確認相依都裝得起來、檔案位置正確、執行檔與 addon 沒有缺少的函式庫、`fcitx5-setup` 能把 ZhuyinType 加入執行中的 Fcitx 5；移除後確認沒有留下任何檔案，包括更新程式的下載資料夾。`ZT_VERIFY_FROM=舊的 dist` 先裝舊版再升級，並確認改過的登入啟動設定保留。`--lint` 執行 lintian 與 rpmlint；`--gui OUT` 在 Xvfb 與 Mesa lavapipe 上啟動設定視窗截圖，再移除套件，確認執行中的程式隨之結束。`--typing [BUILD]` 用安裝好的套件與真實資料，經 Fcitx 5 在 Qt 測試程式與設定視窗中實際打字（測試程式來自 `build.sh linux --test` 的建置目錄）。主機離 `deb.debian.org` 較遠時，`ZT_DEBIAN_MIRROR` 也套用到 Debian 容器（例如 `remote.sh env ZT_DEBIAN_MIRROR=http://free.nchc.org.tw/debian packaging/linux/verify.sh …`）。結果記錄在 [VALIDATION.md](VALIDATION.md)。

## 發布

`desktop.yml` 在每次修改 `platforms/desktop` 或 `platforms/linux` 時建置這兩個套件（並在乾淨的容器中安裝、移除），以 artifact 保存。推送 `vX.Y.Z` 標籤時，`release.yml` 先呼叫 `desktop.yml`，三個平台都建置成功才發布：GitHub release 附上 `zhuyintype_<版本>_amd64.deb`、`zhuyintype-<版本>.x86_64.rpm`，以及固定檔名的 `ZhuyinType-Linux-amd64.deb`、`ZhuyinType-Linux-x86_64.rpm`（README 的下載連結指向 `releases/latest/download/` 的這兩個檔名），檢查碼在 `SHA256SUMS-desktop.txt`。版本號取自 `platforms/desktop/Cargo.toml`，必須和標籤相同。

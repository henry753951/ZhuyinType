# Linux 與 Windows 版

2026-11-16：Linux 與 Windows 改用 `platforms/desktop/` 的 GPUI 程式 `zhuyintype`，取代先前的 Qt 客戶端。架構、建置與介面規則見 [platforms/desktop/ARCHITECTURE.md](../../platforms/desktop/ARCHITECTURE.md)，通訊協定見 [IPC.md](../../platforms/desktop/IPC.md)。

## 和 macOS 版的關係

- **功能與版面相同**：九個主頁、子頁面、設定列的順序與文字、ⓘ 說明與示範、頁面之間的連結、候選字視窗的各種樣式、設定嚮導，都依照 macOS 版（本文件 [DESIGN.md](DESIGN.md) 與 `platforms/macos/Sources/ZhuyinTypeApp/UI/`）。
- **視覺不同**：Linux／Windows 版是 ZhuyinType 自己的一套現代設計語言，不模仿 macOS，也不模仿 Windows 原生外觀。暖色中性的表面、單色線條圖示、以細分隔線區隔設定列、強調色只用在少數地方；視窗不用系統邊框，標題列與視窗按鈕由 App 自己繪製。macOS 版維持原生外觀。
- **用語**：⌥ 寫成 Alt、⌃ 寫成 Ctrl、「這台 Mac」寫成「這台電腦」、Finder 寫成「檔案管理員」（Windows：「檔案總管」）。

## 安裝

| 平台 | 安裝檔 | 內容 |
|---|---|---|
| Windows 10／11 x64 | `ZhuyinType-<版本>-windows-x64-setup.exe`（NSIS） | 一次 UAC；登記 64 與 32 位元的 TSF 文字服務並加入輸入法清單；登入時啟動；開始功能表「ZhuyinType 設定」；從「設定 › 應用程式」解除安裝，個人資料預設保留 |
| Debian／Ubuntu | `zhuyintype_<版本>_amd64.deb` | `/usr/bin/zhuyintype`、Fcitx5 外掛、引擎資料、登入時啟動；第一次執行時把 ZhuyinType 加入 Fcitx5 的輸入法 |
| Fedora／openSUSE | `zhuyintype-<版本>.x86_64.rpm` | 同上 |

建置都在 GPU 主機上進行（`tools/zt --remote build desktop --package`），Windows 版以 cargo-xwin 交叉編譯、`makensis` 打包。細節見 [Windows 打包](../../platforms/desktop/packaging/windows/README.md) 與 [Linux 打包](../../platforms/desktop/packaging/linux/README.md)。

## 尚未完成

- Wayland 的候選字視窗位置（需要合成器的輸入法 popup 介面）。
- 神經模型（重排序與字模型）在 Linux／Windows 尚未載入。
- 自動更新：目前「關於」頁開啟 GitHub 發行頁面。
- 程式碼簽章。

<div align="center">

<img src="docs/assets/icon.png" width="128" height="128" alt="ZhuyinType 圖示">

# ZhuyinType 注音

開源、在本機運作的注音輸入法，支援 macOS、Windows 與 Linux。<br>
整句選字、修正按錯的鍵、依前文調整用語，所有輸入都在這台電腦上處理。

[![最新版本](https://img.shields.io/github/v/release/henry753951/ZhuyinType?label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC&color=3969FA)](https://github.com/henry753951/ZhuyinType/releases/latest)
[![下載次數](https://img.shields.io/github/downloads/henry753951/ZhuyinType/total?label=%E4%B8%8B%E8%BC%89&color=3969FA)](https://github.com/henry753951/ZhuyinType/releases)
[![平台](https://img.shields.io/badge/%E5%B9%B3%E5%8F%B0-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-3969FA)](#安裝)
[![授權](https://img.shields.io/github/license/henry753951/ZhuyinType?label=%E6%8E%88%E6%AC%8A)](LICENSE)
<br>
[![Engine](https://img.shields.io/github/actions/workflow/status/henry753951/ZhuyinType/engine.yml?branch=main&label=Engine&logo=rust)](https://github.com/henry753951/ZhuyinType/actions/workflows/engine.yml)
[![macOS](https://img.shields.io/github/actions/workflow/status/henry753951/ZhuyinType/macos.yml?branch=main&label=macOS&logo=swift&logoColor=white)](https://github.com/henry753951/ZhuyinType/actions/workflows/macos.yml)
[![ML](https://img.shields.io/github/actions/workflow/status/henry753951/ZhuyinType/ml.yml?branch=main&label=ML&logo=python&logoColor=white)](https://github.com/henry753951/ZhuyinType/actions/workflows/ml.yml)
[![ZhuyinBench](https://img.shields.io/github/actions/workflow/status/henry753951/ZhuyinType/benchmark.yml?label=ZhuyinBench)](docs/benchmarks/)

[下載](#安裝) · [發布說明](docs/releases/) · [每版成績](docs/benchmarks/) · [文件](docs/README.md) · [規劃](docs/ROADMAP.md)

</div>

<table>
  <tr>
    <td width="50%"><img src="docs/assets/screenshots/wizard-welcome.png" alt="設定嚮導的歡迎畫面與候選字清單"></td>
    <td width="50%"><img src="docs/assets/screenshots/wizard-preset.png" alt="選擇 Windows 或 macOS 注音的習慣"></td>
  </tr>
  <tr>
    <td align="center">第一次使用時，設定嚮導帶你選好打法並試打</td>
    <td align="center">沿用 Windows 或 macOS 注音的按鍵習慣</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/screenshots/wizard-look.png" alt="調整候選字視窗的間距、位置與選字鍵"></td>
    <td width="50%"><img src="docs/assets/screenshots/wizard-keys.png" alt="依設定產生的按鍵表"></td>
  </tr>
  <tr>
    <td align="center">調整候選字視窗，預覽即時套用</td>
    <td align="center">依你的設定產生按鍵表</td>
  </tr>
</table>

## 特色

- **整句輸入**：一句話連續打完，依整句的讀音與前後文選出最通順的寫法；一聲可以不按空白。
- **按錯鍵也能打對**：相鄰鍵、多按、漏按、順序打反與聲調打錯都會試著修正，並記住你常按錯的鍵。
- **依前文選字**：參考游標前的文字，聊天時偏向口語，寫文章時偏向書面語。
- **記住你的用法**：選過的字詞與常一起出現的詞會越打越順，也可以新增自訂詞。
- **在本機處理**：選字不連網路，前文只放在記憶體中，密碼欄位裡暫停讀取前文與學習。

## 安裝

最新版本的安裝檔固定在 [最新發布頁](https://github.com/henry753951/ZhuyinType/releases/latest)，下方連結會自動指向最新版：

| 平台 | 下載 | 需求 |
|---|---|---|
| macOS | [ZhuyinType-macOS.pkg](https://github.com/henry753951/ZhuyinType/releases/latest/download/ZhuyinType-macOS.pkg) | macOS 14 或更新版本 |
| Windows | [ZhuyinType-Windows-x64.zip](https://github.com/henry753951/ZhuyinType/releases/latest/download/ZhuyinType-Windows-x64.zip) | Windows 10 或 11（x64） |
| Linux | [ZhuyinType-Linux-x86_64.tar.gz](https://github.com/henry753951/ZhuyinType/releases/latest/download/ZhuyinType-Linux-x86_64.tar.gz) | Fcitx 5、Qt 6.8 以上，x86_64 |

### macOS

開啟 `ZhuyinType-<版本>.pkg`，依指示安裝，需要管理員權限。套件會安裝到 `/Library/Input Methods/ZhuyinType.app`。

第一次使用時：

1. 開啟「系統設定 → 鍵盤 → 輸入方式 → 編輯」（部分版本顯示為「文字輸入」）。
2. 按「＋」，選擇「繁體中文」，加入「ZhuyinType 注音」。已加入時可直接進行下一步。
3. 從選單列的輸入法選單選擇「注音 ZhuyinType」，或按 Control-Space 切換（依系統快捷鍵設定）。

清單尚未出現時，請登出再登入。每個帳號各自加入輸入方式。

之後的版本會在你停止打字後自動更新（設定 → 關於 → 更新），更新檔都經過簽章驗證，正在使用的輸入法會在組字結束後換成新版，不需要登出或重新選擇輸入法。

> [!NOTE]
> 目前 app 採 ad-hoc 簽章，pkg 的簽署與公證尚待完成，套件使用建置機的架構。若 macOS 顯示安全性提示，請至「系統設定 → 隱私權與安全性」查看開啟選項。曾從原始碼安裝者，請先移除 `~/Library/Input Methods/ZhuyinType.app`，再使用 pkg 版本。

### Windows

解壓縮 `ZhuyinType-Windows-x64.zip`，在資料夾中以 PowerShell 執行 `install.ps1`。安裝程式會：

- 把程式放到 `%LOCALAPPDATA%\ZhuyinType`，並註冊「ZhuyinType 注音」輸入法；
- 設定登入後自動啟動，並在開始功能表建立「ZhuyinType Settings」捷徑。

安裝後到「設定 → 時間與語言 → 語言與地區」確認已加入「ZhuyinType 注音」，再以 Win＋空白鍵切換。工作列的輸入指示器左格顯示「中」／「英」，按一下切換；右格是 ZhuyinType 的標誌，右鍵可開啟設定。

解除安裝：執行 `install.ps1 -Uninstall`。

> [!NOTE]
> 安裝檔尚未簽章，Windows 可能顯示 SmartScreen 提示；請確認來源為本發布頁後，選擇「其他資訊 → 仍要執行」。

### Linux

Ubuntu 26.04 可用 `apt` 安裝需求：

```sh
sudo apt install fcitx5 libqt6quick6 libqt6qml6 libqt6network6 libqt6quickcontrols2-6 qml6-module-qtquick-controls
```

解開 `ZhuyinType-Linux-x86_64.tar.gz` 後，在資料夾中執行 `./install.sh`。安裝程式會複製 Fcitx 5 附加元件、啟動服務，並設定登入後自動啟動。之後在 Fcitx 5 設定中加入「ZhuyinType」並切換使用。

目前 Linux 版使用 X11 桌面（Xfce、GNOME、KDE 的 X11 工作階段）；Wayland 原生候選字視窗尚未完成，Wayland 工作階段會顯示提示並停止服務。

## 基本按鍵

以下為預設設定；完整的按鍵表見 [產品設計 §2.1](docs/product/DESIGN.md#21-組字與按鍵)，「設定 → 快捷鍵」可重新指定。

| 按鍵 | 操作 |
|---|---|
| 注音鍵、聲調鍵（6 3 4 7） | 輸入注音與聲調，連續輸入整句；空白是一聲 |
| 空白 | 打完注音後送出整段並輸入空白（可在設定改成開啟候選字清單） |
| ↓ | 開啟候選字清單；清單開著時，空白翻下一頁 |
| 1–9 | 選取候選字 |
| ↑ ↓ | 在清單裡移動 |
| ← → | 在組字中移動游標；直式清單開著時改前一個或後一個字 |
| ⇧↓／⇧↑ | 清單只列單字／列出按錯修正與模糊音 |
| Return | 送出整段；清單開著時選取目前的候選字 |
| ⌫ | 刪掉一個注音符號或游標前的字；清單開著時先關閉清單 |
| Esc | 關閉清單；再按清除注音，再按清空組字 |
| 單按 Shift | 中英切換 |

## 功能

<details>
<summary>展開完整的功能說明</summary>

#### 整句輸入

- 一句話連續打完，不必逐字選字。ZhuyinType 依整句的讀音與前後文選出最通順的寫法，打字時即時更新。
- 一聲可以不按空白：音節已完整、下一鍵是聲母時，自動在這裡斷開。
- 「一」「不」照實際發音打變調也能選對，例如「一定」打 ㄧˊ ㄉㄧㄥˋ、「不要」打 ㄅㄨˊ ㄧㄠˋ。
- 多音字依台灣讀音計分，例如「說」打 ㄕㄨㄟˋ 時「睡」排在前面。
- 「台」與「臺」可在設定擇一，候選字清單把兩種寫法合併成一項。

#### 按錯鍵也能打對

- 按到相鄰的鍵、多按一鍵、漏按一鍵、兩鍵順序打反，ZhuyinType 會試著修正，修正後的結果要明顯比照打的通順才會採用，打對的鍵優先。
- 會參考按鍵的時間：35 毫秒內幾乎同時按下的兩鍵，比較可能是順序打反或誤觸。
- 聲調打錯（三聲與四聲、二聲與輕聲）時一樣找得到想打的字。
- 模糊音可在設定個別開啟：ㄗ／ㄓ、ㄘ／ㄔ、ㄙ／ㄕ、ㄣ／ㄥ、ㄢ／ㄤ。
- 「設定」的「按錯鍵修正」可選關閉、標準或積極。
- 從候選字清單選了修正過的字時，ZhuyinType 會記住你把哪個鍵按成哪個鍵，下次同樣的錯誤更容易修正。

#### 依前文選字

- 開始組字時，讀取游標前最多 128 個字作為前文，選字時參考。
- 依前文自動調整用語：前面是聊天，偏向口語；前面是文章，偏向書面語。
- 接續剛送出的字：一句話分幾次送出時，後面的部分仍接著前面選字。
- App 不提供游標前的文字時，改用你在同一個 App 裡 10 分鐘內送出的字，換輸入框也接得上。
- 前文只放在記憶體中，不寫入磁碟。可在設定關閉；在密碼欄位中一律不讀取。

#### 候選字與改字

- ↓ 開啟候選字清單，長詞在前、單字在後，同一長度依前文排序。空白鍵預設送出並輸入空白，也可改成在組字中輸入空白（不送出）或開啟候選字視窗；清單開著時，空白翻頁。
- ⇧↓ 只列單字，⇧↑ 列出按錯修正與模糊音的字。
- 選字後，其他已顯示的字保持不變；選到詞中間的字時，同一個詞在它前面的字也不變。
- 候選字視窗可調整排列（橫式、直式）、字級、每頁的候選字數、選字鍵、間距、位置（游標下方或上方）、外觀與動畫，設定頁的預覽會即時播放打字的過程。

#### 記住你的用法

- 選過的字詞，下次打同樣的讀音時直接採用。
- 常一起出現的詞會越來越順：個人的一、二、三連詞統計分成短期（約一天內）與長期（約幾個月）兩層。
- 選過的詞在同樣的位置不會被神經模型換掉。
- 送出後 30 秒內用 Backspace 刪掉重打成不同的字，視同改字：新的寫法被記住，舊的減弱。
- 可新增自訂詞，把個別字詞設為不記住，並匯出、匯入、清除個人資料。
- 在密碼欄位中暫停學習。

#### 其他

- 標點按法可在設定選擇：
  - Windows（預設）：和 Windows 的微軟注音相同，Shift 加符號鍵打出鍵上的符號（例如 `<`、`_`），Control 加符號鍵打出中文標點（Control 加逗號是「，」）。
  - macOS：和 macOS 內建注音相同，Shift 加符號鍵打出中文標點（Shift 加逗號是「，」）。
- `[` `]` 打出「」，`'` 打出「、」。在標點上按 ↓ 可以換成同類的全形或半形標點。
- 單按 Shift 切換中英文，Caps Lock 開啟時輸入英文。快捷鍵可重新指定。
- 有新版本時在你停止打字後自動更新，更新檔都經過簽章驗證，正在使用的輸入法在組字結束後換成新版。
- 設定的「分析」頁回顧打了多少字、改了幾次字，以及輸入法每個按鍵的反應時間；只記錄次數與時間，存在這台電腦，可以關閉或清除。
- 不常用的按鍵功能（打出注音本身、單字與修正清單、⌃ 與 ⌥ 的符號、Caps Lock 打英文等）可以個別關閉，按鍵交給 App；每個功能最多可以指定三個按鍵。
- 所有輸入都在本機處理，不連網路選字。

</details>

## 運作方式

每按一個鍵，ZhuyinType 依序經過以下幾層，通常在 20 毫秒左右更新畫面。完整流程見 [架構與流程](docs/engine/ARCHITECTURE.md)，各層的方法與評測數字見 [模型與演算法](docs/engine/MODEL.md)。

```text
按鍵 → ① 切音節 → ② 讀音與容錯 → ③ 整句解碼 → ④ 整句重排 → 畫面
                                     ↑                 ↑
                         前文、個人記憶、逐字模型        前文
```

1. 切音節：把連續的按鍵切成音節，同時處理一聲不按空白、跨音節的順序錯亂與多按的鍵（KeyDecoder，以按鍵位置做 Viterbi 搜尋，並參考按鍵時間）。
2. 讀音與容錯：每個音節找出可能的讀音。照打的讀音不扣分；相鄰鍵、多按、漏按、對調、聲調與模糊音各有扣分，並依你的按錯習慣調降（KeyChannel）。
3. 整句解碼：在所有可能的詞組成的詞圖上，找出分數最高的前 16 個整句。分數包含詞的語言模型機率（聊天與百科兩個模型依前文混合，再依個人統計調整）、讀音機率、容錯扣分，以及逐字模型（v3）依前文與整句讀音給每個字的分數。
4. 整句重排：引擎對首選沒有把握時，字級 Transformer 語言模型（v2b）依前文為前 16 句評分，選出最後顯示的整句。你選過的詞會保留。
5. 送出與學習：送出後更新個人統計與記住的字詞；選了修正過的字時，記下按錯的鍵。

詞庫與語言模型以 mmap 載入，常駐記憶體約 29 MB。兩個神經模型以 Core ML 在 CPU 上執行。

## 準確率

每次發布後，CI 會在同一套評測（ZhuyinBench）上測量整句全對率。以下為 [0.4.4 的結果](docs/benchmarks/v0.4.4.md)，沒有前文、每組最多 400 段，括號內把「台／臺」等寫法視為相同：

| 測試集 | 整句全對率 |
|---|---|
| 維基 | 82.8% |
| 口語 | 84.0% |
| 日常短句 | 100.0% |
| Taiwan Tongues | 73.8%（85.0%） |

前文、按錯鍵與候選字排序的結果，以及其他版本，見 [每版成績](docs/benchmarks/)；和其他注音輸入法在相同條件下的比較見 [這份文件](docs/benchmarks/competition.md)。

## 從原始碼建置

需要 [Rust](https://rustup.rs)（引擎；`engine/rust-toolchain.toml` 固定版本）、Xcode（設定視窗使用 SwiftUI，需要 Xcode 內附的 macro 外掛；本專案使用 Swift 6.4）與 [uv](https://docs.astral.sh/uv/)。`xcode-select -p` 指向 Command Line Tools 時，建置腳本會自動改用 `/Applications/Xcode.app`。所有工作都由 `tools/zt` 執行，`tools/zt help` 列出全部指令。

```sh
tools/zt setup           # 檢查並準備開發環境
tools/zt data fetch      # 下載詞庫、語言模型與神經模型（依 resources/assets.json 驗證雜湊）
tools/zt build app       # dist/ZhuyinType.app
tools/zt build install   # 安裝到目前帳號；正在使用的輸入法會在組字結束後換成新版
tools/zt check engine    # 引擎的測試（cargo test --release）
tools/zt build pkg       # dist/ZhuyinType-<版本>.pkg
```

| 目錄 | 內容 |
|---|---|
| `engine/` | 所有平台共用的輸入法引擎（Rust）：組字、按鍵解碼、容錯、語言模型、神經模型、C 介面、`zhuyin-cli` |
| `platforms/macos/` | macOS 輸入法（InputMethodKit）與建置、發布腳本 |
| `ml/` | 研發：資料、訓練、評測與 `zt` 指令（Python，見 [ml/README.md](ml/README.md)） |
| `resources/` | 資料與模型的固定版本、來源與授權、二進位格式 |
| `tools/` | `zt` 啟動器，說明見 [tools/README.md](tools/README.md) |
| `docs/` | 產品設計、引擎架構與模型、評測、規劃、研究、每版成績、發布說明（[文件地圖](docs/README.md)） |

給 AI 代理與貢獻者的說明在 [AGENTS.md](AGENTS.md)。

## 移除

先切換至英文輸入方式，並在「系統設定 → 鍵盤 → 輸入方式」移除 ZhuyinType，再刪除安裝的 app。
原始碼提供 `platforms/macos/scripts/uninstall.sh`，可移除系統與目前帳號的 app，預設保留自訂詞、記住的字詞與設定。
執行 `platforms/macos/scripts/uninstall.sh --purge` 會一併刪除這些個人資料。

## 文件

| 文件 | 內容 |
|---|---|
| [產品設計](docs/product/DESIGN.md) | 按鍵、狀態、預設值、介面 |
| [架構與流程](docs/engine/ARCHITECTURE.md) | 從按鍵到畫面的完整流程與分數公式 |
| [模型與演算法](docs/engine/MODEL.md) | 詞庫、按鍵解碼、容錯、語言模型、神經模型、個人模型與評測數字 |
| [v3 模型設計](docs/engine/MODEL_V3.md) | 按鍵層級解碼器與音節對字模型的設計與實驗紀錄 |
| [評測](docs/ml/EVALUATION.md) | 資料集、評測工具與指令 |
| [規劃](docs/ROADMAP.md) | 開發階段、下一步與待決事項 |
| [研究](docs/research/) | 研究紀錄、相關工作與比較、研究計畫 |
| [每版成績](docs/benchmarks/) | 每個版本在同一套評測（ZhuyinBench）上的結果 |
| [訓練](ml/src/zhuyintype_ml/training/README.md) | 各模型的訓練流程與產物 |
| [文案準則](docs/product/WRITING.md) | 介面與文件用語 |

## 資料與授權

程式碼採 [GNU AGPL 3.0 授權](LICENSE)（`AGPL-3.0-only`），作者為 [Henry753951](https://github.com/henry753951) 與 ZhuyinType 貢獻者。再散布時須保留適用的著作權與授權聲明，並依條款提供對應原始碼；修改後透過網路提供互動服務時，亦須依條款向使用者提供對應原始碼。AGPL 允許商業使用。

著作權與授權聲明見 [NOTICE](NOTICE)。產生的詞庫採 CC BY-SA 4.0。
詞庫使用 CC-CEDICT、Unicode Unihan 與 wordfreq；語言模型與神經模型另外使用維基百科、立法院公報、維基百科討論頁、Common Voice 與 Qwen3-8B 產生的句子。來源、處理方式與署名見 [資料來源](resources/SOURCES.md) 及 [授權文字](platforms/macos/Resources/Licenses)。
發布修改過的詞庫或模型時，請保留來源、修改說明與各自的授權。

#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
platforms/macos/scripts/build-app.sh "${1:-release}"
destination="$HOME/Library/Input Methods/ZhuyinType.app"
mkdir -p "$(dirname "$destination")"
# Copy beside the installed app, then swap it in with renames, so the input method's bundle is never missing.  The
# running input method keeps its files open and moves to the new build when the composition ends (EngineHost).
staging="$destination.new"
previous="$destination.old"
rm -rf "$staging" "$previous"
ditto dist/ZhuyinType.app "$staging"
[ -e "$destination" ] && mv "$destination" "$previous"
mv "$staging" "$destination"
rm -rf "$previous"
if ! "$destination/Contents/MacOS/ZhuyinType" --install; then
    echo '加入輸入方式：系統設定 → 鍵盤 → 輸入方式 → 編輯 → ＋ → 繁體中文 → ZhuyinType 注音。部分版本顯示為「文字輸入」。'
    echo '清單尚未出現時，請登出再登入。'
fi
echo '已安裝至 ~/Library/Input Methods/ZhuyinType.app；正在使用的輸入法會在組字結束後自動換成新版。從選單列選擇「注音 ZhuyinType」，或按 Control-Space 切換（依系統快捷鍵設定）。'

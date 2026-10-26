#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-app.sh "${1:-release}"
pkill -x -u "$(id -u)" ZhuyinType || true
destination="$HOME/Library/Input Methods/ZhuyinType.app"
mkdir -p "$(dirname "$destination")"
rm -rf "$destination"
ditto dist/ZhuyinType.app "$destination"
if ! "$destination/Contents/MacOS/ZhuyinType" --install; then
    echo '加入輸入方式：系統設定 → 鍵盤 → 輸入方式 → 編輯 → ＋ → 繁體中文 → ZhuyinType 注音。部分版本顯示為「文字輸入」。'
    echo '清單尚未出現時，請登出再登入。'
fi
echo '已安裝至 ~/Library/Input Methods/ZhuyinType.app。從選單列選擇「注音 ZhuyinType」，或按 Control-Space 切換（依系統快捷鍵設定）。'
killall TextInputMenuAgent TextInputSwitcher 2>/dev/null || true

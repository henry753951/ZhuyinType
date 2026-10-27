#!/bin/bash
set -euo pipefail
case "${1:-}" in ''|--purge) ;; *) echo '用法：clients/macos/scripts/uninstall.sh [--purge]' >&2; exit 2 ;; esac
if [[ $(id -u) -eq 0 ]]; then
    echo '請以目前登入的使用者執行；移除系統版本時會要求 sudo 權限。' >&2
    exit 1
fi
user_app="$HOME/Library/Input Methods/ZhuyinType.app"
system_app='/Library/Input Methods/ZhuyinType.app'
for app in "$user_app" "$system_app"; do
    if [[ -x "$app/Contents/MacOS/ZhuyinType" ]]; then
        if ! "$app/Contents/MacOS/ZhuyinType" --disable; then
            echo '停用失敗。請在「系統設定 → 鍵盤 → 輸入方式」移除 ZhuyinType 後重試。' >&2
            exit 1
        fi
    fi
done
pkill -x -u "$(id -u)" ZhuyinType || true
if [[ -e "$user_app" ]]; then rm -rf "$user_app"; fi
if [[ -e "$system_app" ]]; then sudo /bin/rm -rf "$system_app"; fi
if [[ "${1:-}" == --purge ]]; then
    rm -rf "$HOME/Library/Application Support/ZhuyinType"
    if defaults read org.zhuyintype.inputmethod.ZhuyinType >/dev/null 2>&1; then
        defaults delete org.zhuyintype.inputmethod.ZhuyinType
    fi
fi
echo 'ZhuyinType 已移除；其他登入中的使用者可能需要登出再登入。'

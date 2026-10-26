#!/bin/bash
# Load the installed build without logging out: quit the input method (macOS starts the new one on the next key)
# and restart the input menu so it picks up a changed icon or name.
pkill -x -u "$(id -u)" ZhuyinType || true
killall TextInputMenuAgent 2>/dev/null || true
echo '已重新載入；下一個按鍵會使用剛安裝的版本。'

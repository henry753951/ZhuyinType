#!/bin/bash
# Load the installed build without logging out: quit the input method (macOS starts the new one on the next key).
# The system's input menu (TextInputMenuAgent) and Caps Lock switcher are left alone: restarting them can leave the
# menu bar without its input menu until the next login.
pkill -x -u "$(id -u)" ZhuyinType || true
echo '已重新載入；下一個按鍵會使用剛安裝的版本。'

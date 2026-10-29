#!/usr/bin/env bash
# ZhuyinType for Linux: install from this extracted release folder, or pass --uninstall.
# Installs the Fcitx 5 addon into the system (sudo) and the host under ~/.local/share/zhuyintype, then registers the
# host to start at sign-in. Only ZhuyinType's own files are touched; Fcitx profiles stay as they are.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
share="$HOME/.local/share/zhuyintype"
# Fcitx 5's addon directory: asked from pkg-config when it is installed, otherwise the Debian/Ubuntu path for x86_64.
libdir=/usr/lib/x86_64-linux-gnu
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists Fcitx5Core; then libdir=$(pkg-config --variable=libdir Fcitx5Core); fi
fcitx_libdir="$libdir/fcitx5"
if [[ ${1:-} == --uninstall ]]; then
    sudo rm -f "$fcitx_libdir/libzhuyintype.so" /usr/share/fcitx5/addon/zhuyintype.conf /usr/share/fcitx5/inputmethod/zhuyintype.conf
    pkill -f "$share/bin/zhuyintype-desktop --ime-host" 2>/dev/null || true
    rm -rf "$share" "$HOME/.local/bin/zhuyintype-ime-host" "$HOME/.config/autostart/zhuyintype-ime-host.desktop"
    echo "ZhuyinType 已移除。Fcitx 5 的輸入方式清單請自行確認。"
    exit 0
fi
for tool in sudo bash; do command -v "$tool" >/dev/null || { echo "需要 $tool" >&2; exit 1; }; done
rm -rf "$share"
mkdir -p "$share"
cp -r "$here/bin" "$here/data" "$share/"
sudo install -Dm644 "$here/addon/libzhuyintype.so" "$fcitx_libdir/libzhuyintype.so"
sudo install -Dm644 "$here/addon/zhuyintype.conf" /usr/share/fcitx5/addon/zhuyintype.conf
sudo install -Dm644 "$here/inputmethod/zhuyintype.conf" /usr/share/fcitx5/inputmethod/zhuyintype.conf
bash "$here/scripts/install-host.sh" "$share/bin/zhuyintype-desktop" "$share/data/lexicon.tsv"
echo "已安裝。執行 fcitx5-configtool，在輸入方式中加入「ZhuyinType」，再用 Ctrl＋Space 切換。"
echo "服務會在下次登入時自動啟動；現在可執行 ~/.local/bin/zhuyintype-ime-host 立即啟動。"

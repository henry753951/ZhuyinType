#!/usr/bin/env bash
set -euo pipefail
host=$(realpath "${1:?Usage: install-host.sh HOST [LEXICON]}")
[[ -x $host ]]
lexicon=${2:-}
if [[ -n $lexicon ]]; then lexicon=$(realpath "$lexicon"); [[ -f $lexicon ]]; fi
bin="$HOME/.local/bin"
config=${XDG_CONFIG_HOME:-$HOME/.config}
mkdir -p "$bin" "$config/autostart"
launcher="$bin/zhuyintype-ime-host"
{
    printf '#!/usr/bin/env bash\nset -euo pipefail\n'
    printf 'if [[ ${XDG_SESSION_TYPE:-x11} == wayland ]]; then\n  echo "ZhuyinType 候選視窗尚待 Wayland 原生協定整合。" >&2\n  exit 1\nfi\n'
    printf 'export QT_IM_MODULE=fcitx\n'
    if [[ -n ${QT_QUICK_BACKEND:-} ]]; then printf 'export QT_QUICK_BACKEND=%q\n' "$QT_QUICK_BACKEND"; fi
    printf 'exec %q --ime-host' "$host"
    if [[ -n $lexicon ]]; then printf ' --lexicon %q' "$lexicon"; fi
    printf '\n'
} > "$launcher"
chmod 700 "$launcher"
# Only our own autostart entry is changed. Fcitx profiles and im-config stay owned by the user.
# Desktop Exec quoted-string escaping differs from shell quoting.
escaped=${launcher//\\/\\\\}
escaped=${escaped//\"/\\\"}
escaped=${escaped//\$/\\\$}
escaped=${escaped//\`/\\\`}
escaped=${escaped//%/%%}
cat > "$config/autostart/zhuyintype-ime-host.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ZhuyinType
Comment=注音輸入法共用桌面服務
Exec=/bin/bash "$escaped"
Terminal=false
X-GNOME-Autostart-enabled=true
DESKTOP
printf '已安裝：%s\n' "$launcher" "$config/autostart/zhuyintype-ime-host.desktop"

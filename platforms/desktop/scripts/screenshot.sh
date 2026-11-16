#!/usr/bin/env bash
# Screenshots of the settings window on a virtual X display (Linux host, software Vulkan).
#   scripts/screenshot.sh OUT_DIR [BINARY] [page ...]
# Every page in light and dark at 860×600; the app runs with a temporary settings file.
# A page may be written page/subpage (`typing/punctuation`) or page#row (`choosing#typo`).
# `popover:ID` shows one ⓘ card on its own (`--popover ID`, 420×820 unless ZT_SHOT_SIZE is set).
# ZT_SHOT_SIZE=640x600 takes the narrow layout (files get a -640x600 suffix).
set -euo pipefail
out=${1:?output directory}; shift
bin=${1:-${CARGO_TARGET_DIR:-target}/debug/zhuyintype}; shift || true
pages=("$@"); [ ${#pages[@]} -eq 0 ] && pages=(intro analytics typing choosing words candidates keys advanced about)
mkdir -p "$out"
display=:$((100 + RANDOM % 400))
Xvfb "$display" -screen 0 1400x1600x24 -nolisten tcp >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
sleep 1
export DISPLAY=$display
# lavapipe: Mesa's software Vulkan, so no GPU or compositor is needed.
export VK_ICD_FILENAMES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)
unset WAYLAND_DISPLAY
settings=$(mktemp -d)/settings.json
# ZT_SHOT_SETTINGS=FILE starts from those settings (features turned off, other choices).
[ -n "${ZT_SHOT_SETTINGS:-}" ] && cp "$ZT_SHOT_SETTINGS" "$settings"
for page in "${pages[@]}"; do
  for theme in light dark; do
    spec=$page; name=${spec//\//-}; name=${name//#/-}
    extra=()
    if [ -n "${ZT_SHOT_SIZE:-}" ]; then extra+=(--size "$ZT_SHOT_SIZE"); name=$name-$ZT_SHOT_SIZE; fi
    case $spec in panel:*) extra+=(--panel "${spec#panel:}"); name=panel-${spec#panel:}; spec=intro;; esac
    case $spec in wizard:*) extra+=(--wizard "${ZT_WIZARD:-quick@${spec#wizard:}}"); name=wizard-${spec#wizard:}; spec=intro;; esac
    case $spec in popover:*) extra+=(--popover "${spec#popover:}"); name=popover-${spec#popover:}${ZT_SHOT_SIZE:+-$ZT_SHOT_SIZE}; spec=intro;; esac
    case $spec in *'#'*) extra+=(--row "${spec#*#}"); spec=${spec%%#*};; esac
    case $spec in */*) extra+=(--subpage "${spec#*/}"); spec=${spec%%/*};; esac
    "$bin" --no-broker --page "$spec" "${extra[@]}" --theme "$theme" --settings-file "$settings" >/dev/null 2>"$out/$name-$theme.log" &
    app=$!
    for _ in $(seq 50); do
      win=$(xdotool search --pid $app 2>/dev/null | tail -1 || true)
      [ -n "$win" ] && break; sleep 0.2
    done
    sleep 1.5
    if [ -n "${win:-}" ]; then import -window "$win" "$out/$name-$theme.png" 2>/dev/null || import -window root "$out/$name-$theme.png"
    else import -window root "$out/$name-$theme.png"; fi
    kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
    [ -s "$out/$name-$theme.log" ] || rm -f "$out/$name-$theme.log"
  done
done
echo "screenshots in $out"

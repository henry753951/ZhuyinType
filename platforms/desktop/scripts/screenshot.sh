#!/usr/bin/env bash
# Screenshots of the settings window on a virtual X display (Linux host, software Vulkan).
#   scripts/screenshot.sh OUT_DIR [BINARY] [page ...]
# Every page in light and dark at 860×600; the app runs with a temporary settings file.
set -euo pipefail
out=${1:?output directory}; shift
bin=${1:-${CARGO_TARGET_DIR:-target}/debug/zhuyintype}; shift || true
pages=("$@"); [ ${#pages[@]} -eq 0 ] && pages=(intro analytics typing choosing words candidates keys advanced about)
mkdir -p "$out"
display=:$((100 + RANDOM % 400))
Xvfb "$display" -screen 0 1400x1000x24 -nolisten tcp >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
sleep 1
export DISPLAY=$display
# lavapipe: Mesa's software Vulkan, so no GPU or compositor is needed.
export VK_ICD_FILENAMES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)
unset WAYLAND_DISPLAY
settings=$(mktemp -d)/settings.json
for page in "${pages[@]}"; do
  for theme in light dark; do
    "$bin" --page "$page" --theme "$theme" --settings-file "$settings" >/dev/null 2>"$out/$page-$theme.log" &
    app=$!
    for _ in $(seq 50); do
      win=$(xdotool search --pid $app 2>/dev/null | tail -1 || true)
      [ -n "$win" ] && break; sleep 0.2
    done
    sleep 1.5
    if [ -n "${win:-}" ]; then import -window "$win" "$out/$page-$theme.png" 2>/dev/null || import -window root "$out/$page-$theme.png"
    else import -window root "$out/$page-$theme.png"; fi
    kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
    [ -s "$out/$page-$theme.log" ] || rm -f "$out/$page-$theme.log"
  done
done
echo "screenshots in $out"

#!/usr/bin/env bash
# Every screen of the app for a design review: pages and sub-pages, candidate window variants, the wizard, in light
# and dark, then one contact sheet per group.  Runs on the host:  scripts/review-shots.sh OUT_DIR
set -euo pipefail
out=${1:?output directory}
bin=${CARGO_TARGET_DIR:-target}/debug/zhuyintype
rm -rf "$out"; mkdir -p "$out"/{pages,panels,wizard}
scripts/screenshot.sh "$out/pages" "$bin" intro analytics analytics/performance typing typing/punctuation typing/typingKeys \
  choosing choosing/fuzzy choosing/context words words/learnedWords words/phrases candidates keys advanced advanced/apps \
  about about/sources >/dev/null 2>&1 || true
panels=(); for variant in vertical-standard horizontal-standard vertical-corrections vertical-compact minimal \
  notice-saved mode-toast composing; do panels+=("panel:$variant"); done
scripts/screenshot.sh "$out/panels" "$bin" "${panels[@]}" >/dev/null 2>&1 || true
ZT_SHOT_SIZE=680x700 scripts/screenshot.sh "$out/wizard" "$bin" wizard:welcome wizard:preset wizard:selecting \
  wizard:look wizard:finish wizard:keys >/dev/null 2>&1 || true
cd "$out"
for group in pages panels wizard; do
  for theme in light dark; do
    files=$(ls "$group"/*-"$theme"*.png 2>/dev/null | sort) || true
    [ -n "$files" ] && montage $files -tile 4x -geometry 900x640\>+6+6 -background '#888' "sheet-$group-$theme.png" 2>/dev/null || true
  done
done
ls "$out"

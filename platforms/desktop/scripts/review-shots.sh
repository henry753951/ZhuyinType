#!/usr/bin/env bash
# Every screen of the app for a design review: pages and sub-pages, candidate window variants, the wizard, in light
# and dark, then one contact sheet per group.  Runs on the host:  scripts/review-shots.sh OUT_DIR [GROUP ...]
# Groups: pages (900×640), narrow (the pages at 640×600), panels, wizard; default: pages panels wizard.
set -euo pipefail
out=${1:?output directory}; shift
groups=("$@"); [ ${#groups[@]} -eq 0 ] && groups=(pages panels wizard)
bin=${CARGO_TARGET_DIR:-target}/debug/zhuyintype
rm -rf "$out"; mkdir -p "$out"
pages=(intro analytics analytics/performance typing typing/punctuation typing/typingKeys choosing choosing/fuzzy
  choosing/context words words/learnedWords words/phrases candidates keys advanced advanced/apps about about/sources)
for group in "${groups[@]}"; do
  mkdir -p "$out/$group"
  case $group in
    pages) scripts/screenshot.sh "$out/pages" "$bin" "${pages[@]}" >/dev/null 2>&1 || true ;;
    narrow) ZT_SHOT_SIZE=640x600 scripts/screenshot.sh "$out/narrow" "$bin" "${pages[@]}" >/dev/null 2>&1 || true ;;
    panels)
      panels=(); for variant in vertical-standard horizontal-standard vertical-corrections vertical-compact minimal \
        notice-saved mode-toast composing; do panels+=("panel:$variant"); done
      scripts/screenshot.sh "$out/panels" "$bin" "${panels[@]}" >/dev/null 2>&1 || true ;;
    wizard)
      ZT_SHOT_SIZE=680x700 scripts/screenshot.sh "$out/wizard" "$bin" wizard:welcome wizard:preset wizard:selecting \
        wizard:look wizard:finish wizard:keys >/dev/null 2>&1 || true ;;
  esac
done
cd "$out"
for group in "${groups[@]}"; do
  for theme in light dark; do
    files=$(ls "$group"/*-"$theme"*.png 2>/dev/null | sort) || true
    [ -n "$files" ] && montage $files -tile 4x -geometry 900x640\>+6+6 -background '#888' "sheet-$group-$theme.png" 2>/dev/null || true
  done
done
ls "$out"

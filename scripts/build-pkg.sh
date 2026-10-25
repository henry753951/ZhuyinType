#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-app.sh release
work=$(mktemp -d "$PWD/dist/.pkg-stage.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/root/Library/Input Methods" "$work/resources/zh_TW.lproj"
ditto --norsrc --noextattr --noacl dist/ZhuyinType.app "$work/root/Library/Input Methods/ZhuyinType.app"
cp -X Resources/pkg/zh_TW.lproj/*.html "$work/resources/zh_TW.lproj/"
cp -X "$work/resources/zh_TW.lproj/"*.html "$work/resources/"
cat LICENSE Resources/Licenses/LEXICON.txt Resources/Licenses/wordfreq-NOTICE.txt Resources/Licenses/CC-BY-SA-4.0.txt Resources/Licenses/Unicode-3.0.txt > "$work/resources/license.txt"
# Fixed installation path; do not relocate to a developer's dist/ or a per-user copy.
pkgbuild --root "$work/root" --component-plist Resources/pkg/components.plist \
    --scripts scripts/pkg --identifier org.zhuyintype.pkg --version 0.1.0 \
    --install-location / "$work/ZhuyinType-component.pkg"
productbuild --distribution Resources/pkg/distribution.xml --resources "$work/resources" \
    --package-path "$work" dist/ZhuyinType-0.1.0.pkg
pkgutil --payload-files dist/ZhuyinType-0.1.0.pkg | /usr/bin/grep -q 'Library/Input Methods/ZhuyinType.app/Contents/MacOS/ZhuyinType'
printf '%s\n' "$PWD/dist/ZhuyinType-0.1.0.pkg"

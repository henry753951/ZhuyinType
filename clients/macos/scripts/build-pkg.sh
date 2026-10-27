#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
clients/macos/scripts/build-app.sh release
version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" clients/macos/Resources/Info.plist)
work=$(mktemp -d "$PWD/dist/.pkg-stage.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/root/Library/Input Methods" "$work/resources/zh_TW.lproj"
ditto --norsrc --noextattr --noacl dist/ZhuyinType.app "$work/root/Library/Input Methods/ZhuyinType.app"
cp -X clients/macos/Resources/pkg/zh_TW.lproj/*.html "$work/resources/zh_TW.lproj/"
cp -X "$work/resources/zh_TW.lproj/"*.html "$work/resources/"
cat LICENSE clients/macos/Resources/Licenses/LEXICON.txt clients/macos/Resources/Licenses/wordfreq-NOTICE.txt clients/macos/Resources/Licenses/CC-BY-SA-4.0.txt clients/macos/Resources/Licenses/Unicode-3.0.txt > "$work/resources/license.txt"
# Fixed installation path; do not relocate to a developer's dist/ or a per-user copy.
pkgbuild --root "$work/root" --component-plist clients/macos/Resources/pkg/components.plist \
    --scripts clients/macos/scripts/pkg --identifier org.zhuyintype.pkg --version "$version" \
    --install-location / "$work/ZhuyinType-component.pkg"
productbuild --distribution clients/macos/Resources/pkg/distribution.xml --resources "$work/resources" \
    --package-path "$work" "dist/ZhuyinType-$version.pkg"
pkgutil --payload-files "dist/ZhuyinType-$version.pkg" | /usr/bin/grep -q 'Library/Input Methods/ZhuyinType.app/Contents/MacOS/ZhuyinType'
printf '%s\n' "$PWD/dist/ZhuyinType-$version.pkg"

#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
# ZT_PREBUILT_APP=1: package dist/ZhuyinType.app as it is (release.yml, with the app main's CI built and checked).
if [[ ${ZT_PREBUILT_APP:-} == 1 ]]; then
    [[ -d dist/ZhuyinType.app ]] || { echo "ZT_PREBUILT_APP=1 but there is no dist/ZhuyinType.app" >&2; exit 1; }
else
    platforms/macos/scripts/build-app.sh release
fi
version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" platforms/macos/Resources/Info.plist)
work=$(mktemp -d "$PWD/dist/.pkg-stage.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/root/Library/Input Methods" "$work/resources/zh_TW.lproj"
ditto --norsrc --noextattr --noacl dist/ZhuyinType.app "$work/root/Library/Input Methods/ZhuyinType.app"
cp -X platforms/macos/Resources/pkg/zh_TW.lproj/*.html "$work/resources/zh_TW.lproj/"
cp -X "$work/resources/zh_TW.lproj/"*.html "$work/resources/"
cat NOTICE LICENSE platforms/macos/Resources/Licenses/LEXICON.txt platforms/macos/Resources/Licenses/wordfreq-NOTICE.txt platforms/macos/Resources/Licenses/CC-BY-SA-4.0.txt platforms/macos/Resources/Licenses/Unicode-3.0.txt > "$work/resources/license.txt"
# Fixed installation path; do not relocate to a developer's dist/ or a per-user copy.
pkgbuild --root "$work/root" --component-plist platforms/macos/Resources/pkg/components.plist \
    --scripts platforms/macos/scripts/pkg --identifier org.zhuyintype.pkg --version "$version" \
    --install-location / "$work/ZhuyinType-component.pkg"
productbuild --distribution platforms/macos/Resources/pkg/distribution.xml --resources "$work/resources" \
    --package-path "$work" "dist/ZhuyinType-$version.pkg"
pkgutil --payload-files "dist/ZhuyinType-$version.pkg" | /usr/bin/grep -q 'Library/Input Methods/ZhuyinType.app/Contents/MacOS/ZhuyinType'
printf '%s\n' "$PWD/dist/ZhuyinType-$version.pkg"

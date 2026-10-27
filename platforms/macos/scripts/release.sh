#!/bin/bash
# Builds a release of the macOS client and updates the Sparkle appcast.  Used by .github/workflows/release.yml and
# runnable locally:
#
#   platforms/macos/scripts/release.sh            # version from Info.plist
#
# Produces dist/release/: ZhuyinType-<v>.zip (the Sparkle update), ZhuyinType-<v>.pkg (first install), delta updates
# from the previous release, and rewrites appcast.xml at the repository root.  The EdDSA key comes from
# SPARKLE_ED_KEY_FILE (CI) or the login keychain account "zhuyintype" (local).
set -euo pipefail
cd "$(dirname "$0")/../../.."
repository=henry753951/ZhuyinType
version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" platforms/macos/Resources/Info.plist)
tag="v$version"
[[ -f "docs/releases/$tag.md" ]] || { echo "docs/releases/$tag.md is missing: write the release notes first." >&2; exit 1; }
tools=$(platforms/macos/scripts/sparkle-tools.sh)

platforms/macos/scripts/build-pkg.sh >/dev/null
out=dist/release
work=dist/appcast-work
rm -rf "$out" "$work"
mkdir -p "$out" "$work"
cp "dist/ZhuyinType-$version.pkg" "$out/"
ditto -c -k --sequesterRsrc --keepParent dist/ZhuyinType.app "$work/ZhuyinType-$version.zip"

# The previous release's archive lets generate_appcast build a delta update (only changed files are downloaded).
previous=$(gh release list -R "$repository" --exclude-pre-releases --limit 20 --json tagName \
    --jq '[.[].tagName | select(startswith("v"))] | map(select((ltrimstr("v") | split(".") | map(tonumber)) < ['"${version//./,}"'])) | sort_by(ltrimstr("v") | split(".") | map(tonumber)) | last // empty' 2>/dev/null || true)
if [[ -n "$previous" ]]; then
    gh release download "$previous" -R "$repository" --pattern "ZhuyinType-${previous#v}.zip" --dir "$work" 2>/dev/null \
        || echo "No archive for $previous; publishing without a delta." >&2
fi
[[ -f appcast.xml ]] && cp appcast.xml "$work/appcast.xml"

key_args=(--account zhuyintype)
[[ -n "${SPARKLE_ED_KEY_FILE:-}" ]] && key_args=(--ed-key-file "$SPARKLE_ED_KEY_FILE")
"$tools/generate_appcast" "${key_args[@]}" \
    --download-url-prefix "https://github.com/$repository/releases/download/$tag/" \
    --link "https://github.com/$repository/releases/tag/$tag" \
    --full-release-notes-url "https://github.com/$repository/blob/main/docs/releases/$tag.md" \
    --maximum-deltas 3 -o "$work/appcast.xml" "$work"
cp "$work/appcast.xml" appcast.xml
cp "$work/ZhuyinType-$version.zip" "$out/"
find "$work" -name "*.delta" -exec cp {} "$out/" \;
ls -la "$out"

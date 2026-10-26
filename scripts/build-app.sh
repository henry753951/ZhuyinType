#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
configuration=${1:-release}
case "$configuration" in debug|release) ;; *) echo '用法：scripts/build-app.sh [debug|release]' >&2; exit 2 ;; esac
scratch=${SWIFT_SCRATCH_PATH:-.build}
# SwiftUI's macros ship with Xcode, not with the Command Line Tools.
if [[ -z "${DEVELOPER_DIR:-}" && "$(xcode-select -p)" == /Library/Developer/CommandLineTools* && -d /Applications/Xcode.app ]]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
scripts/build-lexicon.sh
if [[ ! -f Resources/Assets.car || ! -f Resources/AppIcon.icns || ! -f Resources/InputSourceTemplate.pdf ]]; then
    swift scripts/make_icons.swift
fi
# Record the SDK the app is built with, so macOS draws it with the current system design rather than in the
# compatibility look of the deployment target.
sdk_version=$(xcrun --show-sdk-version)
swift build --scratch-path "$scratch" -c "$configuration" --product ZhuyinType \
    -Xlinker -platform_version -Xlinker macos -Xlinker 14.0 -Xlinker "$sdk_version"
bin_dir=$(swift build --scratch-path "$scratch" -c "$configuration" --show-bin-path)
app="$PWD/dist/ZhuyinType.app"
mkdir -p dist
stage=$(mktemp -d "$PWD/dist/.app-stage.XXXXXX")
trap 'rm -rf "$stage"' EXIT
contents="$stage/ZhuyinType.app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources/Licenses"
cp -X "$bin_dir/ZhuyinType" "$contents/MacOS/"
cp -X Resources/Info.plist "$contents/"
printf 'APPL????' > "$contents/PkgInfo"
cp -X Resources/Assets.car Resources/AppIcon.icns Resources/InputSourceTemplate.pdf data/build/lexicon.tsv "$contents/Resources/"
cp -R -X Resources/*.lproj "$contents/Resources/"
cp -R -X Resources/Licenses/. "$contents/Resources/Licenses/"
cp -X LICENSE "$contents/Resources/Licenses/AGPL-3.0.txt"
cp -X data/SOURCES.md "$contents/Resources/Licenses/SOURCES.md"
if [[ -f data/build/reranker/vocab.json && -d data/build/reranker/Reranker.mlmodelc ]]; then
    mkdir -p "$contents/Resources/reranker"
    cp -R -X data/build/reranker/Reranker.mlmodelc "$contents/Resources/reranker/"
    cp -X data/build/reranker/vocab.json data/build/reranker/config.json "$contents/Resources/reranker/"
fi
if [[ -f data/build/lm/bigram.tsv ]]; then
    mkdir -p "$contents/Resources/lm"
    cp -X data/build/lm/{unigram,bigram,backoff}.tsv data/build/lm/NOTICE.md "$contents/Resources/lm/"
fi
if [[ -f data/build/lm_chat/bigram.tsv ]]; then
    mkdir -p "$contents/Resources/lm_chat"
    cp -X data/build/lm_chat/{unigram,bigram,backoff}.tsv data/build/lm_chat/{NOTICE.md,Taiwan-Tongues-LICENSE,Qwen3-8B-LICENSE} "$contents/Resources/lm_chat/"
fi
if [[ -d data/build/v3/ContextEncoder.mlmodelc && -d data/build/v3/SyllableEncoder.mlmodelc ]]; then
    mkdir -p "$contents/Resources/v3"
    cp -R -X data/build/v3/ContextEncoder.mlmodelc data/build/v3/SyllableEncoder.mlmodelc "$contents/Resources/v3/"
    cp -X data/build/v3/{chars,syllables,config}.json "$contents/Resources/v3/"
fi
plutil -lint "$contents/Info.plist"
codesign --force --deep --sign - --timestamp=none "$stage/ZhuyinType.app"
codesign --verify --deep --strict "$stage/ZhuyinType.app"
rm -rf "$app"
mv "$stage/ZhuyinType.app" "$app"
printf '%s\n' "$app"

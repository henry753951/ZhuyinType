#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
configuration=${1:-release}
case "$configuration" in debug|release) ;; *) echo '用法：platforms/macos/scripts/build-app.sh [debug|release]' >&2; exit 2 ;; esac
scratch=${SWIFT_SCRATCH_PATH:-.build}
# The engine (Rust): its C ABI as a static library, linked into the app by Package.swift.
(cd engine && cargo build --release --locked -p zhuyin-ffi)
# SwiftUI's macros ship with Xcode, not with the Command Line Tools.
if [[ -z "${DEVELOPER_DIR:-}" && "$(xcode-select -p)" == /Library/Developer/CommandLineTools* && -d /Applications/Xcode.app ]]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
tools/zt data lexicon
if [[ ! -f platforms/macos/Resources/Assets.car || ! -f platforms/macos/Resources/AppIcon.icns || ! -f platforms/macos/Resources/InputSourceTemplate.pdf ]]; then
    swift platforms/macos/scripts/make_icons.swift
fi
# Record the SDK the app is built with, so macOS draws it with the current system design rather than in the
# compatibility look of the deployment target.
sdk_version=$(xcrun --show-sdk-version)
swift build --package-path platforms/macos --scratch-path "$scratch" -c "$configuration" --product ZhuyinType \
    -Xlinker -platform_version -Xlinker macos -Xlinker 14.0 -Xlinker "$sdk_version"
bin_dir=$(swift build --package-path platforms/macos --scratch-path "$scratch" -c "$configuration" --show-bin-path)
app="$PWD/dist/ZhuyinType.app"
mkdir -p dist
stage=$(mktemp -d "$PWD/dist/.app-stage.XXXXXX")
trap 'rm -rf "$stage"' EXIT
contents="$stage/ZhuyinType.app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources/Licenses"
cp -X "$bin_dir/ZhuyinType" "$contents/MacOS/"
# Sparkle (OTA updates), loaded through @executable_path/../Frameworks.
mkdir -p "$contents/Frameworks"
ditto "$bin_dir/Sparkle.framework" "$contents/Frameworks/Sparkle.framework"
cp -X platforms/macos/Resources/Info.plist "$contents/"
printf 'APPL????' > "$contents/PkgInfo"
cp -X platforms/macos/Resources/Assets.car platforms/macos/Resources/AppIcon.icns platforms/macos/Resources/InputSourceTemplate.pdf "$contents/Resources/"
cp -R -X platforms/macos/Resources/*.lproj "$contents/Resources/"
cp -R -X platforms/macos/Resources/Licenses/. "$contents/Resources/Licenses/"
cp -X LICENSE "$contents/Resources/Licenses/AGPL-3.0.txt"
cp -X resources/SOURCES.md "$contents/Resources/Licenses/SOURCES.md"
if [[ -f data/build/reranker/vocab.json && -d data/build/reranker/Reranker.mlmodelc ]]; then
    mkdir -p "$contents/Resources/reranker"
    cp -R -X data/build/reranker/Reranker.mlmodelc "$contents/Resources/reranker/"
    # The same weights scoring the candidates as one prefix tree (used when present; see ml/src/zhuyintype_ml/training/reranker/REPORT_tree.md).
    [[ -d data/build/reranker/RerankerTree.mlmodelc ]] && cp -R -X data/build/reranker/RerankerTree.mlmodelc "$contents/Resources/reranker/"
    cp -X data/build/reranker/vocab.json data/build/reranker/config.json "$contents/Resources/reranker/"
fi
# The lexicon and language models ship compiled (lexicon.bin, lm/lm.bin, lm_chat/lm.bin): the engine maps them
# instead of parsing text, so it starts in about 0.1 s and keeps them out of its own memory.  If compiling fails the
# TSV files are copied instead; the engine reads either.
if [[ -f data/build/lm/bigram.tsv ]] && tools/zt data compile --no-channel "$contents/Resources" >/dev/null; then
    :
else
    echo '注意：無法編譯詞庫與語言模型，改用 TSV。' >&2
    cp -X data/build/lexicon.tsv "$contents/Resources/"
    for model in lm lm_chat; do
        [[ -f data/build/$model/bigram.tsv ]] || continue
        mkdir -p "$contents/Resources/$model"
        cp -X data/build/$model/{unigram,bigram,backoff}.tsv "$contents/Resources/$model/"
    done
fi
[[ -d "$contents/Resources/lm" ]] && cp -X data/build/lm/NOTICE.md "$contents/Resources/lm/"
[[ -d "$contents/Resources/lm_chat" ]] && cp -X data/build/lm_chat/{NOTICE.md,attribution.jsonl,source-manifest.json} "$contents/Resources/lm_chat/"
if [[ -d data/build/v3/ContextEncoder.mlmodelc && -d data/build/v3/SyllableEncoder.mlmodelc ]]; then
    mkdir -p "$contents/Resources/v3"
    cp -R -X data/build/v3/ContextEncoder.mlmodelc data/build/v3/SyllableEncoder.mlmodelc "$contents/Resources/v3/"
    cp -X data/build/v3/{chars,syllables,config}.json "$contents/Resources/v3/"
    # Attribution for the training text (the article list itself is in the checkpoints asset).
    for notice in NOTICE.md attribution.jsonl source-manifest.json Licenses; do
        [[ -e data/build/v3/$notice ]] && cp -R -X data/build/v3/$notice "$contents/Resources/v3/"
    done
fi
# What this build ships: version and the SHA-256 of every data and model file, so an installed copy can be checked and
# updates can be told apart from the running one.
/usr/bin/python3 -I - "$contents" <<'PY'
import hashlib, json, os, plistlib, sys
contents = sys.argv[1]
resources = os.path.join(contents, "Resources")
info = plistlib.load(open(os.path.join(contents, "Info.plist"), "rb"))
files = {}
for name in sorted(os.listdir(resources)):
    if not (name.startswith("lexicon.") or name in ("lm", "lm_chat", "reranker", "v3")):
        continue
    path = os.path.join(resources, name)
    for root, _, names in [(resources, [], [name])] if os.path.isfile(path) else os.walk(path):
        for file in names:
            full = os.path.join(root, file)
            files[os.path.relpath(full, resources)] = hashlib.sha256(open(full, "rb").read()).hexdigest()
manifest = {"version": info["CFBundleShortVersionString"], "build": info["CFBundleVersion"],
            "files": dict(sorted(files.items()))}
json.dump(manifest, open(os.path.join(resources, "manifest.json"), "w"), indent=1, ensure_ascii=False)
PY
plutil -lint "$contents/Info.plist"
codesign --force --deep --sign - --timestamp=none "$stage/ZhuyinType.app"
codesign --verify --deep --strict "$stage/ZhuyinType.app"
rm -rf "$app"
mv "$stage/ZhuyinType.app" "$app"
printf '%s\n' "$app"

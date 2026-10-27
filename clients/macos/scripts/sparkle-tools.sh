#!/bin/bash
# Fetches Sparkle's command-line tools (generate_keys, sign_update, generate_appcast) into .tools/sparkle, pinned to
# the same version as clients/macos/Package.swift, and prints their directory.  Run from anywhere.
set -euo pipefail
cd "$(dirname "$0")/../../.."
version=2.10.0
# SHA-256 of Sparkle-2.10.0.tar.xz from https://github.com/sparkle-project/Sparkle/releases/tag/2.10.0
checksum=${SPARKLE_SHA256:-c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c}
dir=.tools/sparkle-$version
if [[ ! -x "$dir/bin/sign_update" ]]; then
    mkdir -p "$dir"
    archive=$(mktemp -t sparkle).tar.xz
    curl -fsSL -o "$archive" "https://github.com/sparkle-project/Sparkle/releases/download/$version/Sparkle-$version.tar.xz"
    actual=$(shasum -a 256 "$archive" | cut -d' ' -f1)
    if [[ -n "$checksum" && "$actual" != "$checksum" ]]; then
        echo "Sparkle $version checksum mismatch: $actual" >&2
        exit 1
    fi
    tar -xJf "$archive" -C "$dir" bin
    rm -f "$archive"
fi
printf '%s\n' "$PWD/$dir/bin"

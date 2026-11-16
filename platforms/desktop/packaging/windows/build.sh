#!/usr/bin/env bash
# The Windows installer, built on the Linux host (never on the Mac):
#   release binaries cross-compiled with cargo-xwin (the app and the 64-bit TSF DLL for x86_64, the 32-bit DLL for
#   i686), the engine data, the licences, then makensis
#   → dist/desktop/ZhuyinType-<version>-windows-x64-setup.exe
#
# From the Mac:
#   platforms/desktop/scripts/remote.sh packaging/windows/build.sh
#   platforms/desktop/scripts/remote.sh --fetch zhuyintype/work/desktop-<checkout>/dist/desktop LOCAL_DIR
#
# Needs cargo-xwin (cargo +1.98.0 install cargo-xwin --locked), clang-cl/lld-link/llvm-rc (LLVM), makensis (apt
# install nsis), ImageMagick for the installer artwork (optional), and uv for tools/zt (the engine data).
# ZT_SIGN_COMMAND, if set, signs every executable: a command with %1 for the file (TODO: no certificate yet).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
desktop=$(cd "$here/../.." && pwd)
root=$(cd "$desktop/../.." && pwd)
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$desktop/Cargo.toml")
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "build.sh: 無法從 Cargo.toml 讀到版本（$version）" >&2; exit 2; }
target_dir=${CARGO_TARGET_DIR:-$desktop/target}
data_dir=${ZT_DATA:-$root/data/build}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-16}
# Only the MSVC libraries for the two targets built here.
export XWIN_ARCH=${XWIN_ARCH:-x86,x86_64}
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH

need() { command -v "$1" >/dev/null || { echo "build.sh: 找不到 $1（$2）" >&2; exit 2; }; }
need cargo-xwin "cargo +1.98.0 install cargo-xwin --locked"
need clang-cl "LLVM"
need lld-link "LLVM"
need llvm-rc "LLVM"
need makensis "apt install nsis"

sign() {
  [[ -n ${ZT_SIGN_COMMAND:-} ]] || return 0
  bash -c "${ZT_SIGN_COMMAND//%1/\"$1\"}"
}

echo "== ZhuyinType $version：建置 Windows 版"
cd "$desktop"
cargo xwin build --release --locked -p zhuyintype -p zt-tsf --target x86_64-pc-windows-msvc
cargo xwin build --release --locked -p zt-tsf --target i686-pc-windows-msvc

work=$target_dir/windows-installer
stage=$work/stage
rm -rf "$work"
mkdir -p "$stage/data" "$stage/licenses" "$stage/fonts"
cp "$target_dir/x86_64-pc-windows-msvc/release/zhuyintype.exe" "$stage/"
cp "$target_dir/x86_64-pc-windows-msvc/release/zhuyintype_tsf.dll" "$stage/zhuyintype-tsf-x64.dll"
cp "$target_dir/i686-pc-windows-msvc/release/zhuyintype_tsf.dll" "$stage/zhuyintype-tsf-x86.dll"
cp "$desktop/crates/zt-tsf/logo.ico" "$stage/logo.ico"
cp "$desktop/crates/zhuyintype/windows/zhuyintype.ico" "$stage/zhuyintype.ico"
# The candidate window the text service draws inside applications uses the app's typeface (panel_draw.rs).
cp "$desktop/crates/zhuyintype/assets/fonts/"Inter-*.ttf "$stage/fonts/"
for binary in zhuyintype.exe zhuyintype-tsf-x64.dll zhuyintype-tsf-x86.dll; do sign "$stage/$binary"; done

# The engine data, at <exe dir>\data, the same set as the Linux packages and the macOS app (build-app.sh): the
# lexicon, the reading channel and both language models compiled for mmap, with their notices.
echo "== 引擎資料"
(cd "$root" && tools/zt data fetch lexicon lm lm_chat && tools/zt data compile --quiet "$stage/data")
mkdir -p "$stage/data/channel"
cp "$data_dir/lm/NOTICE.md" "$data_dir/lm/articles.tsv.gz" "$stage/data/lm/"
cp "$data_dir/lm_chat/NOTICE.md" "$data_dir/lm_chat/attribution.jsonl" "$data_dir/lm_chat/source-manifest.json" \
  "$stage/data/lm_chat/"
cp "$data_dir/channel/NOTICE.md" "$stage/data/channel/"
# These archived notices describe our code under its old licence (the same edit as the macOS build).
sed -i -e 's/Code: MIT\./Code: AGPL-3.0-only./g' -e 's/Scripts are MIT-licensed\./Scripts are AGPL-3.0-only./g' \
  -e 's/is separately licensed under MIT\./is separately licensed under GNU AGPL version 3 (AGPL-3.0-only)./g' \
  "$stage/data"/{lm,lm_chat,channel}/NOTICE.md
for file in lexicon.bin channel.bin lm/lm.bin lm/NOTICE.md lm/articles.tsv.gz lm_chat/lm.bin channel/NOTICE.md; do
  [[ -s $stage/data/$file ]] || { echo "build.sh: 缺少引擎資料 data/$file" >&2; exit 1; }
done
# The neural models (ONNX) and ONNX Runtime, which the app loads from beside itself.
"$desktop/packaging/neural.sh" "$stage/data" "$stage/licenses" "$stage/onnxruntime.dll" windows

echo "== 授權"
cp "$root/LICENSE" "$stage/licenses/LICENSE.txt"
cp "$root/NOTICE" "$stage/licenses/NOTICE.txt"
cp "$root/resources/SOURCES.md" "$root/resources/sources.json" "$stage/licenses/"
for licence in Apache-2.0.txt CC-BY-4.0.txt CC-BY-SA-4.0.txt CC0-1.0.txt LEXICON.txt McBopomofo-MIT.txt OGDL-1.0.txt \
  Reranker-NOTICE.md Unicode-3.0.txt libtabe-COPYING.txt wordfreq-NOTICE.txt; do
  cp "$root/platforms/macos/Resources/Licenses/$licence" "$stage/licenses/"
done
cp "$root/platforms/desktop/crates/zhuyintype/assets/fonts/Inter-LICENSE.txt" \
  "$root/platforms/desktop/crates/zhuyintype/assets/Lucide-LICENSE.txt" "$stage/licenses/"
# Every Rust crate in the Windows build, with its licence.
cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc \
  | python3 -I -c '
import json, sys
meta = json.load(sys.stdin)
used = {node["id"] for node in meta["resolve"]["nodes"]}
rows = sorted({(p["name"], p["version"], p.get("license") or "見套件", p.get("repository") or "")
               for p in meta["packages"] if p["id"] in used})
print("ZhuyinType for Windows 使用的 Rust 套件與授權\n")
for name, version, licence, repository in rows:
    print(f"{name} {version}\t{licence}\t{repository}")
' > "$stage/licenses/Rust-crates.txt"

# The installer's artwork: the app icon on ZhuyinType's light background.
branding=()
if command -v convert >/dev/null; then
  mkdir -p "$work/branding"
  icon=$desktop/crates/zhuyintype/assets/app-icon.png
  convert -size 164x314 xc:'#f3f6fd' \( "$icon" -resize 112x112 \) -gravity north -geometry +0+72 -composite \
    -type TrueColor BMP3:"$work/branding/welcome.bmp"
  convert -size 150x57 xc:white \( "$icon" -resize 44x44 \) -gravity east -geometry +10+0 -composite \
    -type TrueColor BMP3:"$work/branding/header.bmp"
  branding=(-DBRANDING="$work/branding")
fi

echo "== 安裝程式"
out_dir=$root/dist/desktop
mkdir -p "$out_dir"
out=$out_dir/ZhuyinType-$version-windows-x64-setup.exe
signing=()
[[ -n ${ZT_SIGN_COMMAND:-} ]] && signing=(-DSIGN="$ZT_SIGN_COMMAND")
makensis -V2 -INPUTCHARSET UTF8 -DVERSION="$version" -DSTAGE="$stage" -DOUTFILE="$out" "${branding[@]}" \
  "${signing[@]}" "$here/installer.nsi"
echo "安裝程式：$out（$(du -h "$out" | cut -f1)）"
sha256sum "$out"

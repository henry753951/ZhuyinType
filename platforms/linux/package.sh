#!/usr/bin/env bash
# Builds ZhuyinType-Linux-x86_64.tar.gz from a finished build tree (see README). Run from the repository root:
#   platforms/linux/package.sh <build dir with zhuyintype-desktop and libzhuyin.so> <build dir with libzhuyintype.so> <out dir>
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
host_dir=${1:?host build dir}; addon_dir=${2:?addon build dir}; out=${3:?output dir}
stage=$(mktemp -d)/ZhuyinType-Linux-x86_64
mkdir -p "$stage"/{bin,data,addon,inputmethod,scripts}
cp "$host_dir/zhuyintype-desktop" "$host_dir/libzhuyin.so" "$stage/bin/"
cp -r "$root/data/build/." "$stage/data/"
cp "$addon_dir/libzhuyintype.so" "$stage/addon/"
cp "$root/platforms/linux/data/zhuyintype-addon.conf" "$stage/addon/zhuyintype.conf"
cp "$root/platforms/linux/data/zhuyintype.conf" "$stage/inputmethod/"
cp "$root/platforms/linux/scripts/install-host.sh" "$stage/scripts/"
cp "$root/platforms/linux/package/install.sh" "$stage/"
cp "$root/LICENSE" "$root/NOTICE" "$stage/"
mkdir -p "$out"
tar -C "$(dirname "$stage")" -czf "$out/ZhuyinType-Linux-x86_64.tar.gz" "$(basename "$stage")"
(cd "$out" && sha256sum ZhuyinType-Linux-x86_64.tar.gz > ZhuyinType-Linux-x86_64.tar.gz.sha256)
echo "$out/ZhuyinType-Linux-x86_64.tar.gz"

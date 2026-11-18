#!/usr/bin/env bash
# The neural models and ONNX Runtime for a package (zt-core/src/models.rs), the same for Windows and Linux:
#
#   packaging/neural.sh DATA_DIR LICENSES_DIR RUNTIME_FILE windows|linux
#
# - DATA_DIR/reranker and DATA_DIR/v3: the JSON files and notices of the `reranker` and `v3` data components (the
#   macOS app's), with the `onnx` component's models beside them (ORT format, memory-mapped by the app).
# - RUNTIME_FILE: ONNX Runtime's library from Microsoft's release (pinned by version and SHA-256 below), which the app
#   loads at run time: onnxruntime.dll beside zhuyintype.exe, /usr/lib/zhuyintype/libonnxruntime.so.1.  It is
#   downloaded from this repository's toolchains-1 release (the same files), else from Microsoft's.  When neither
#   serves the archive, the same library comes from Microsoft's NuGet package Microsoft.ML.OnnxRuntime
#   of the same version (also pinned; its runtimes/*/native libraries are byte for byte the release archives').
# - LICENSES_DIR: ONNX Runtime's licence and third-party notices.
#
# The archive is cached in $ZT_TOOLCHAINS (default ~/zhuyintype/toolchains).
set -euo pipefail
data=${1:?DATA_DIR} licenses=${2:?LICENSES_DIR} runtime=${3:?RUNTIME_FILE} platform=${4:?windows|linux}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
source_data=${ZT_DATA:-$root/data/build}
toolchains=${ZT_TOOLCHAINS:-$HOME/zhuyintype/toolchains}

version=1.24.4
case $platform in
  windows) archive=onnxruntime-win-x64-$version.zip sum=d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357
    member=lib/onnxruntime.dll nuget_member=runtimes/win-x64/native/onnxruntime.dll ;;
  linux) archive=onnxruntime-linux-x64-$version.tgz sum=3a211fbea252c1e66290658f1b735b772056149f28321e71c308942cdb54b747
    member=lib/libonnxruntime.so.$version nuget_member=runtimes/linux-x64/native/libonnxruntime.so ;;
  *) echo "neural.sh: windows 或 linux" >&2; exit 2 ;;
esac
nuget=microsoft.ml.onnxruntime.$version.nupkg nuget_sum=4b978d5065b85e7004b6c6f60ca494bd978fbe6836cbf0a0b52d82b61ab99638

echo "== 神經模型"
(cd "$root" && tools/zt data fetch reranker v3 onnx)
mkdir -p "$data/reranker" "$data/v3"
cp "$source_data"/reranker/{vocab.json,config.json,NOTICE.md} "$source_data/onnx/RerankerTree.ort" "$data/reranker/"
cp "$source_data"/v3/{chars.json,syllables.json,config.json,NOTICE.md,attribution.jsonl,source-manifest.json,articles.tsv.gz} \
  "$source_data/onnx/ContextEncoder.ort" "$source_data/onnx/SyllableEncoder.ort" "$data/v3/"
cp -R "$source_data/v3/Licenses" "$data/v3/"
cp "$source_data/onnx/manifest.json" "$data/v3/onnx-manifest.json"
for file in reranker/vocab.json reranker/RerankerTree.ort v3/chars.json v3/ContextEncoder.ort v3/SyllableEncoder.ort; do
  [[ -s $data/$file ]] || { echo "neural.sh: 缺少 data/$file" >&2; exit 1; }
done

echo "== ONNX Runtime $version"
mkdir -p "$toolchains"
fetch() { curl -fsSL --retry 3 -o "$toolchains/$1.part" "$2" && mv "$toolchains/$1.part" "$toolchains/$1"; }
# Our own copy first (the toolchains-1 release holds the same files), then Microsoft's release, then NuGet.
mirror=${ZT_MIRROR:-https://github.com/henry753951/ZhuyinType/releases/download/toolchains-1}
if [[ ! -f $toolchains/$archive && ! -f $toolchains/$nuget ]]; then
  fetch "$archive" "$mirror/$archive" \
    || fetch "$archive" "https://github.com/microsoft/onnxruntime/releases/download/v$version/$archive" || {
    echo "neural.sh: 無法下載 $archive，改用 NuGet 的 Microsoft.ML.OnnxRuntime $version" >&2
    fetch "$nuget" "$mirror/$nuget" \
      || fetch "$nuget" "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/$version/$nuget"
  }
fi
if [[ -f $toolchains/$archive ]]; then
  echo "$sum  $toolchains/$archive" | sha256sum -c --quiet - || { echo "neural.sh: $archive 的 SHA-256 不符" >&2; exit 1; }
  prefix=${archive%.zip}
  prefix=${prefix%.tgz}
  source=$toolchains/$archive members=("$prefix/$member" "$prefix/LICENSE" "$prefix/ThirdPartyNotices.txt")
else
  echo "$nuget_sum  $toolchains/$nuget" | sha256sum -c --quiet - || { echo "neural.sh: $nuget 的 SHA-256 不符" >&2; exit 1; }
  source=$toolchains/$nuget members=("$nuget_member" LICENSE ThirdPartyNotices.txt)
fi
mkdir -p "$(dirname "$runtime")" "$licenses"
python3 -I - "$source" "${members[@]}" "$runtime" "$licenses" <<'PY'
import shutil, sys, tarfile, zipfile
archive, runtime_name, license_name, notices_name, runtime, licenses = sys.argv[1:]
wanted = {runtime_name: runtime, license_name: f"{licenses}/ONNX-Runtime-LICENSE.txt",
          notices_name: f"{licenses}/ONNX-Runtime-ThirdPartyNotices.txt"}
if zipfile.is_zipfile(archive):
    with zipfile.ZipFile(archive) as z:
        for name, target in wanted.items():
            with z.open(name) as source, open(target, "wb") as out:
                shutil.copyfileobj(source, out)
else:
    with tarfile.open(archive) as t:
        for name, target in wanted.items():
            with t.extractfile(name) as source, open(target, "wb") as out:
                shutil.copyfileobj(source, out)
PY
chmod 0755 "$runtime"
echo "模型與 ONNX Runtime：$(du -sh "$data/reranker" "$data/v3" "$runtime" | awk '{print $1}' | paste -sd' ')"

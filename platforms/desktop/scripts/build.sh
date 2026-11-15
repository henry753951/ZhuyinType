#!/usr/bin/env bash
# Builds ZhuyinType for Linux and Windows.  Runs on the Linux GPU host or in CI, never on the Mac; from the Mac use
# `tools/zt --remote build desktop ...` or `platforms/desktop/scripts/remote.sh scripts/build.sh ...`.
#
#   scripts/build.sh [linux|windows|all] [--release] [--package] [--native] [--test]
#
#   linux       cargo build of the app (`zhuyintype`) and the CMake build of the Fcitx 5 addon (platforms/linux)
#   --release   optimised build (default: debug)
#   --package   dist/desktop/zhuyintype_<version>_amd64.deb and zhuyintype-<version>.x86_64.rpm (implies --release).
#               Compiled in the Debian 13 builder image (packaging/linux/Dockerfile, docker or podman), so the
#               packages install on Debian 13, Ubuntu 25.04, Fedora 42 and newer.  Engine data is fetched and
#               compiled first when it is missing.
#   --test      also run `cargo test` and the addon's tests against the real host (`tools/zt check desktop`), in the
#               tester image: Fcitx 5, Qt 6 and Xvfb for the application tests (ZT_REQUIRE_HOST=1: none is skipped)
#   --native    compile (and test) on this machine instead of an image; the packages then need this machine's glibc
#   windows     packaging/windows/build.sh (the installer), when it exists
#
# Environment: CARGO_TARGET_DIR (default platforms/desktop/target), CARGO_BUILD_JOBS (default: cores, at most 16),
# ZT_TOOLCHAINS (nfpm and the builder's CARGO_HOME; default ~/zhuyintype/toolchains), ZT_CONTAINER (docker or
# podman), ZT_DEBIAN_MIRROR (the builder image's Debian mirror), ZT_MAINTAINER (the packages' maintainer field),
# SOURCE_DATE_EPOCH (the changelog date).
set -euo pipefail
umask 022

desktop=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$desktop/../.." && pwd)
packaging=$desktop/packaging/linux

fail() { printf '\nbuild.sh：錯誤：%s\n' "$*" >&2; exit 1; }
step() { printf '\n==> %s\n' "$*"; }

target=linux release=0 package=0 native=0 test=0
for arg in "$@"; do
    case "$arg" in
        linux | windows | all) target=$arg ;;
        --release) release=1 ;;
        --package) package=1 release=1 ;;
        --native) native=1 ;;
        --test) test=1 ;;
        -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
        *) fail "不認得的參數 $arg（scripts/build.sh --help 列出用法）" ;;
    esac
done
[[ $(uname -s) == Linux ]] || fail "只在 Linux 上建置（GPU 主機或 CI）；在 Mac 上請用 tools/zt --remote build desktop"

version=$(sed -n 's/^version = "\([0-9.]*\)"$/\1/p' "$desktop/Cargo.toml" | head -1)
[[ -n $version ]] || fail "讀不到 platforms/desktop/Cargo.toml 的 version"
profile=$([[ $release == 1 ]] && echo release || echo debug)
nproc=$(nproc)
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$((nproc < 16 ? nproc : 16))}
toolchains=${ZT_TOOLCHAINS:-$HOME/zhuyintype/toolchains}
target_dir=${CARGO_TARGET_DIR:-$desktop/target}
dist=$root/dist/desktop

# MARK: - Compiling (on this machine, or inside the builder image)

# cargo + CMake into $1 (a cargo target directory).  For packages also the library dependencies and the crate
# licences, into $1/package-inputs.
compile_linux() {
    local dir=$1 cargo_args=(build --locked -p zhuyintype)
    [[ $release == 1 ]] && cargo_args+=(--release)
    command -v cargo >/dev/null || fail "找不到 cargo；請安裝 rustup（https://rustup.rs）"
    command -v cmake >/dev/null || fail "找不到 cmake；Debian/Ubuntu：sudo apt install cmake ninja-build"
    step "cargo ${cargo_args[*]}（$CARGO_BUILD_JOBS 個工作）"
    (cd "$desktop" && CARGO_TARGET_DIR=$dir cargo "${cargo_args[@]}") || fail "cargo build 失敗（見上方）"

    local addon=$dir/fcitx5-$profile generator=()
    command -v ninja >/dev/null && generator=(-G Ninja)
    [[ -f $addon/CMakeCache.txt ]] && generator=()
    # Packages get the distribution's hardening flags (stack protector, fortify, relro, bindnow).
    local flags=()
    if [[ $package == 1 ]] && command -v dpkg-buildflags >/dev/null; then
        flags=(-DCMAKE_CXX_FLAGS="$(DEB_BUILD_MAINT_OPTIONS=hardening=+all dpkg-buildflags --get CPPFLAGS) \
$(DEB_BUILD_MAINT_OPTIONS=hardening=+all dpkg-buildflags --get CXXFLAGS)"
            -DCMAKE_MODULE_LINKER_FLAGS="$(DEB_BUILD_MAINT_OPTIONS=hardening=+all dpkg-buildflags --get LDFLAGS)")
    fi
    step "Fcitx 5 addon（CMake，$addon）"
    cmake -S "$root/platforms/linux" -B "$addon" "${generator[@]}" "${flags[@]}" -Wno-dev \
        -DCMAKE_BUILD_TYPE="$([[ $release == 1 ]] && echo Release || echo Debug)" -DCMAKE_INSTALL_PREFIX=/usr \
        -DZT_IME_HOST="$dir/$profile/zhuyintype" >/dev/null \
        || fail "CMake 設定失敗；需要 libfcitx5core-dev、libfcitx5config-dev、libfcitx5utils-dev、nlohmann-json3-dev"
    cmake --build "$addon" --parallel "$CARGO_BUILD_JOBS" || fail "addon 建置失敗（見上方）"

    if [[ $test == 1 ]]; then
        step "cargo test"
        (cd "$desktop" && CARGO_TARGET_DIR=$dir cargo test --locked --workspace) || fail "cargo test 失敗"
        step "addon 測試（CTest，對真正的 zhuyintype 服務）"
        ZT_REQUIRE_HOST=${ZT_REQUIRE_HOST:-1} ctest --test-dir "$addon" --output-on-failure || fail "addon 測試失敗"
    fi
    [[ $package == 1 ]] || return 0

    local inputs=$dir/package-inputs binary=$dir/$profile/zhuyintype library=$addon/libzhuyintype.so
    rm -rf "$inputs" && mkdir -p "$inputs"
    step "套件相依（dpkg-shlibdeps、ELF sonames）"
    command -v dpkg-shlibdeps >/dev/null || fail "找不到 dpkg-shlibdeps（dpkg-dev 套件）"
    local scratch
    scratch=$(mktemp -d)
    mkdir -p "$scratch/debian"
    printf 'Source: zhuyintype\n\nPackage: zhuyintype\nArchitecture: amd64\n' > "$scratch/debian/control"
    (cd "$scratch" && dpkg-shlibdeps -O --warnings=0 -e"$binary" -e"$library") \
        | sed -n 's/^shlibs:Depends=//p' | tr ',' '\n' | sed 's/^ *//; /^$/d' > "$inputs/deb-shlibs.txt"
    rm -rf "$scratch"
    [[ -s $inputs/deb-shlibs.txt ]] || fail "dpkg-shlibdeps 沒有輸出"
    rpm_requires "$binary" "$library" > "$inputs/rpm-shlibs.txt"
    step "Rust 套件授權"
    (cd "$desktop" && python3 scripts/rust-licenses.py "$inputs/Rust-crates.txt") || fail "Rust 套件授權收集失敗"
}

# What rpmbuild's elfdeps would require: each needed soname and each (non-weak) symbol version, as
# `libc.so.6(GLIBC_2.34)(64bit)`.  The same names on Fedora and openSUSE.
rpm_requires() {
    local file
    for file in "$@"; do
        readelf -dW "$file" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1()(64bit)/p'
        readelf -VW "$file" | awk '
            /File: / { for (i = 1; i <= NF; i++) if ($i == "File:") lib = $(i + 1) }
            /Name: / && lib != "" && !/WEAK/ { for (i = 1; i <= NF; i++) if ($i == "Name:") print lib "(" $(i + 1) ")(64bit)" }'
    done | sort -u
}

# The image of a $packaging/Dockerfile stage (builder, or tester for --test) with the toolchain the workspace pins.
builder_image() {
    local runtime=$1 stage=$2 rust hash image
    rust=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$desktop/rust-toolchain.toml")
    hash=$(cat "$packaging/Dockerfile" "$desktop/rust-toolchain.toml" | sha256sum | cut -c1-12)
    image=zhuyintype-$stage:debian13-$hash
    if ! "$runtime" image inspect "$image" >/dev/null 2>&1; then
        step "建立映像 $image（第一次需要幾分鐘）" >&2
        "$runtime" build --target "$stage" --build-arg "RUST_VERSION=$rust" \
            --build-arg "DEBIAN_MIRROR=${ZT_DEBIAN_MIRROR:-}" -t "$image" "$packaging" >&2 \
            || fail "建置映像失敗（見上方）"
    fi
    echo "$image"
}

# Runs this script with the same target and options in the builder image (packages) or the tester image (--test).
compile_in_builder() {
    local runtime=${ZT_CONTAINER:-}
    if [[ -z $runtime ]]; then
        if command -v podman >/dev/null; then runtime=podman; elif command -v docker >/dev/null; then runtime=docker; fi
    fi
    [[ -n $runtime ]] || fail "找不到 docker 或 podman。套件在 Debian 13 映像中編譯，以便在較舊的發行版安裝；
  請安裝其中一個（Ubuntu：sudo apt install podman），或加上 --native 改在這台機器上編譯"
    local image dir=$target_dir/builder-debian13 cargo_home=$toolchains/cargo-debian13 user=() mounts=() args=(linux)
    image=$(builder_image "$runtime" "$([[ $test == 1 ]] && echo tester || echo builder)")
    [[ $release == 1 ]] && args+=(--release)
    [[ $package == 1 ]] && args+=(--package)
    [[ $test == 1 ]] && args+=(--test)
    mkdir -p "$dir" "$cargo_home"
    # The calling user, so files in the target directory stay theirs; with docker the user's passwd entry comes
    # along (D-Bus in the tests looks it up).
    if [[ $runtime == *podman* ]]; then
        user=(--userns=keep-id)
    else
        user=(-u "$(id -u):$(id -g)" -v /etc/passwd:/etc/passwd:ro -v /etc/group:/etc/group:ro)
    fi
    # The crate download cache is this machine's (cargo locks it while it writes), so crates are fetched once.
    local registry=${CARGO_HOME:-$HOME/.cargo}/registry
    mkdir -p "$registry"
    mounts=(-v "$root:/src" -v "$dir:/target" -v "$cargo_home:/cargo" -v "$registry:/cargo/registry")
    step "在 $image 中編譯"
    # --init: an init process reaps the tests' X servers and daemons (xvfb-run waits for a signal PID 1 ignores).
    "$runtime" run --rm --init "${user[@]}" "${mounts[@]}" \
        -e HOME=/tmp -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS -e ZT_IN_BUILDER=1 \
        -e ZT_REQUIRE_HOST -e ZT_DELIVERY_CAPTURE_DIR \
        -w /src "$image" bash platforms/desktop/scripts/build.sh "${args[@]}" \
        || fail "建置映像中的編譯失敗（見上方）"
    compiled=$dir
}

# MARK: - Packaging (on this machine)

nfpm_binary() {
    local version=2.48.0 sum=6d7f9a28b2f397934403950fcbba83f598b08d02edbe79a0815b3b4897341750
    local dir=$toolchains/nfpm-$version archive=nfpm_${version}_Linux_x86_64.tar.gz
    if [[ ! -x $dir/nfpm ]]; then
        step "安裝 nfpm $version 到 $dir" >&2
        mkdir -p "$dir"
        curl -fsSL -o "$dir/$archive" "https://github.com/goreleaser/nfpm/releases/download/v$version/$archive" \
            || fail "下載 nfpm 失敗"
        (cd "$dir" && echo "$sum  $archive" | sha256sum -c --quiet && tar -xzf "$archive" nfpm && rm "$archive") \
            || fail "nfpm 檢查碼不符"
    fi
    echo "$dir/nfpm"
}

# lexicon.bin, channel.bin, lm/lm.bin, lm_chat/lm.bin and their notices, as the macOS app bundles them
# (platforms/macos/scripts/build-app.sh); neural models (Core ML) are macOS only.
prepare_data() {
    local data=$root/data/build
    # The archive cache `zt --remote` shares between the host's checkouts.
    if [[ -z ${ZHUYIN_ASSET_CACHE:-} && -d $HOME/zhuyintype/asset-cache ]]; then
        export ZHUYIN_ASSET_CACHE=$HOME/zhuyintype/asset-cache
    fi
    if [[ ! -f $data/lexicon.tsv || ! -f $data/lm/bigram.tsv || ! -f $data/lm_chat/bigram.tsv ]]; then
        step "下載引擎資料（tools/zt data fetch lexicon lm lm_chat）"
        "$root/tools/zt" data fetch lexicon lm lm_chat || fail "下載引擎資料失敗"
    fi
    # Next to the TSV files, where the app's development builds, zt-core's tests and the evaluations find them.
    step "編譯詞庫與語言模型（tools/zt data compile）"
    "$root/tools/zt" data compile --quiet || fail "tools/zt data compile 失敗"
}

icon() {
    local size=$1 out=$2 tool
    tool=$(command -v magick || command -v convert || true)
    [[ -n $tool ]] || fail "找不到 ImageMagick（magick 或 convert），無法產生圖示；Debian/Ubuntu：sudo apt install imagemagick"
    mkdir -p "$(dirname "$out")"
    "$tool" "$root/docs/assets/icon.png" -resize "${size}x${size}" -strip "PNG32:$out"
}

stage_linux() {
    local stage=$1 binary=$2 addon=$3 inputs=$4 data=$root/data/build
    step "整理套件內容（$stage）"
    rm -rf "$stage" && mkdir -p "$stage"
    install -Dm755 "$binary" "$stage/usr/bin/zhuyintype"
    install -Dm644 "$addon/libzhuyintype.so" "$stage/addon/libzhuyintype.so"
    strip --strip-unneeded --remove-section=.comment "$stage/addon/libzhuyintype.so"
    strip --remove-section=.comment "$stage/usr/bin/zhuyintype"
    install -Dm644 "$addon/zhuyintype-addon.conf" "$stage/usr/share/fcitx5/addon/zhuyintype.conf"
    install -Dm644 "$root/platforms/linux/data/zhuyintype.conf" "$stage/usr/share/fcitx5/inputmethod/zhuyintype.conf"
    install -Dm755 "$packaging/fcitx5-setup" "$stage/usr/lib/zhuyintype/fcitx5-setup"
    install -Dm644 "$packaging/zhuyintype.desktop" "$stage/usr/share/applications/zhuyintype.desktop"
    install -Dm644 "$packaging/autostart.desktop" "$stage/etc/xdg/autostart/zhuyintype.desktop"
    local size
    for size in 16 22 24 32 48 64 128 256; do
        icon "$size" "$stage/usr/share/icons/hicolor/${size}x${size}/apps/zhuyintype.png"
    done

    local share=$stage/usr/share/zhuyintype/data compiled=$data
    mkdir -p "$share/lm" "$share/lm_chat" "$share/channel"
    cp "$compiled"/lexicon.{bin,json} "$compiled"/channel.{bin,json} "$share/"
    cp "$compiled"/lm/lm.{bin,json} "$data"/lm/{NOTICE.md,articles.tsv.gz} "$share/lm/"
    cp "$compiled"/lm_chat/lm.{bin,json} "$data"/lm_chat/{NOTICE.md,attribution.jsonl,source-manifest.json} \
        "$share/lm_chat/"
    cp "$data/channel/NOTICE.md" "$share/channel/"
    # The archived notices describe our code under its earlier licence (as build-app.sh corrects them).
    sed -i -e 's/Code: MIT\./Code: AGPL-3.0-only./g' -e 's/Scripts are MIT-licensed\./Scripts are AGPL-3.0-only./g' \
        -e 's/is separately licensed under MIT\./is separately licensed under GNU AGPL version 3 (AGPL-3.0-only)./g' \
        "$share"/{lm,lm_chat,channel}/NOTICE.md
    python3 -I - "$share" "$version" <<'PY'
import hashlib, json, os, sys
share, version = sys.argv[1], sys.argv[2]
files = {}
for directory, _, names in os.walk(share):
    for name in names:
        path = os.path.join(directory, name)
        files[os.path.relpath(path, share)] = hashlib.sha256(open(path, "rb").read()).hexdigest()
manifest = {"version": version, "files": dict(sorted(files.items()))}
json.dump(manifest, open(os.path.join(share, "manifest.json"), "w"), indent=1, ensure_ascii=False)
PY

    local licenses=$stage/usr/share/licenses/zhuyintype
    mkdir -p "$licenses" "$stage/usr/share/doc/zhuyintype" "$stage/deb"
    install -m644 "$root/LICENSE" "$licenses/AGPL-3.0.txt"
    install -m644 "$root/NOTICE" "$licenses/NOTICE.txt"
    install -m644 "$root/resources/SOURCES.md" "$root/resources/sources.json" "$licenses/"
    install -m644 "$root/platforms/linux/LICENSES/nlohmann-json.txt" "$inputs/Rust-crates.txt" "$licenses/"
    local file
    for file in "$root"/platforms/macos/Resources/Licenses/*; do
        # Sparkle and the reranker ship with the macOS app only.
        case "$(basename "$file")" in Sparkle-* | Reranker-*) continue ;; esac
        install -m644 "$file" "$licenses/"
    done
    install -m644 "$root/README.md" "$stage/usr/share/doc/zhuyintype/README.md"

    install -m644 "$packaging/copyright" "$stage/deb/copyright"
    # nfpm writes it as the .deb's changelog.Debian.gz and the .rpm's %changelog.
    cat > "$stage/changelog.yml" <<YAML
- semver: $version-1
  date: $(date -u -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y-%m-%dT%H:%M:%SZ)
  packager: $ZT_MAINTAINER
  deb:
    urgency: medium
    distributions: [unstable]
  changes:
    - note: 版本 $version：https://github.com/henry753951/ZhuyinType/releases/tag/v$version
YAML
}

# The nfpm configuration with the `# @shlibs@` lines replaced by the libraries the binaries need, `# @licenses@` by
# the staged licence files (nfpm neither globs typed entries nor puts `type: license` files in a .deb), and
# ${ZT_PACKAGING} (nfpm does not
# expand the scripts' paths) by this directory.
render_config() {
    local inputs=$1 stage=$2 out=$3
    (cd "$stage/usr/share/licenses/zhuyintype" && ls) > "$inputs/licenses.txt"
    awk -v deb="$inputs/deb-shlibs.txt" -v rpm="$inputs/rpm-shlibs.txt" -v licenses="$inputs/licenses.txt" \
        -v packaging="$packaging" -v stage="$stage" '
        /^ *# @shlibs@$/ {
            indent = substr($0, 1, index($0, "#") - 1)
            file = (indent == "  ") ? deb : rpm
            while ((getline line < file) > 0) print indent "- \"" line "\""
            close(file); next
        }
        /^ *# @licenses@/ {
            while ((getline name < licenses) > 0) {
                split("deb rpm", packagers, " ")
                for (i = 1; i <= 2; i++) {
                    print "  - src: ${ZT_STAGE}/usr/share/licenses/zhuyintype/" name
                    print "    dst: /usr/share/licenses/zhuyintype/" name
                    if (packagers[i] == "rpm") print "    type: license"
                    print "    packager: " packagers[i]
                    print "    expand: true"
                }
            }
            close(licenses); next
        }
        { gsub(/\$\{ZT_PACKAGING\}/, packaging); gsub(/\$\{ZT_STAGE\}/, stage); print }' "$packaging/nfpm.yaml" > "$out"
}

package_linux() {
    local inputs=$compiled/package-inputs stage=$compiled/package-stage nfpm config
    [[ -s $inputs/deb-shlibs.txt && -s $inputs/rpm-shlibs.txt ]] || fail "缺少 $inputs 的相依清單"
    export ZT_MAINTAINER=${ZT_MAINTAINER:-ZhuyinType <henry753951@users.noreply.github.com>}
    prepare_data
    stage_linux "$stage" "$compiled/release/zhuyintype" "$compiled/fcitx5-release" "$inputs"
    nfpm=$(nfpm_binary)
    config=$compiled/nfpm.yaml
    render_config "$inputs" "$stage" "$config"
    export ZT_VERSION=$version ZT_STAGE=$stage
    mkdir -p "$dist"
    local deb=$dist/zhuyintype_${version}_amd64.deb rpm=$dist/zhuyintype-${version}.x86_64.rpm
    rm -f "$deb" "$rpm"
    step "nfpm：$(basename "$deb")、$(basename "$rpm")"
    "$nfpm" package --config "$config" --packager deb --target "$deb" >/dev/null || fail "nfpm（deb）失敗"
    "$nfpm" package --config "$config" --packager rpm --target "$rpm" >/dev/null || fail "nfpm（rpm）失敗"
    # nfpm silently drops entries it cannot place; the main files must be in both packages.
    local listing path
    if command -v dpkg-deb >/dev/null; then
        listing=$(dpkg-deb -c "$deb" | awk '{print substr($6, 2)}')
        for path in /usr/bin/zhuyintype /usr/lib/x86_64-linux-gnu/fcitx5/libzhuyintype.so /usr/share/zhuyintype/data/lexicon.bin; do
            grep -qx "$path" <<< "$listing" || fail "$(basename "$deb") 缺少 $path"
        done
    fi
    if command -v rpm >/dev/null; then
        listing=$(rpm -qpl "$rpm" 2>/dev/null)
        for path in /usr/bin/zhuyintype /usr/lib64/fcitx5/libzhuyintype.so /usr/share/zhuyintype/data/lexicon.bin; do
            grep -qx "$path" <<< "$listing" || fail "$(basename "$rpm") 缺少 $path"
        done
    fi
    (cd "$dist" && sha256sum "$(basename "$deb")" "$(basename "$rpm")" > SHA256SUMS-linux)
    step "完成"
    ls -l "$deb" "$rpm"
}

# MARK: - Targets

build_linux() {
    if [[ ${ZT_IN_BUILDER:-0} == 1 ]]; then
        compile_linux "$CARGO_TARGET_DIR"
        return
    fi
    if [[ $test == 1 && -f $root/data/build/lexicon.tsv ]]; then
        # zt-core's real-data tests read the compiled files beside the downloaded TSV files (skipped without data).
        "$root/tools/zt" data compile --quiet || fail "tools/zt data compile 失敗"
    fi
    if [[ ($package == 1 || $test == 1) && $native == 0 ]]; then
        compile_in_builder
    else
        compile_linux "$target_dir"
        compiled=$target_dir
    fi
    if [[ $package == 1 ]]; then
        package_linux
    else
        step "完成：$compiled/$profile/zhuyintype、$compiled/fcitx5-$profile/libzhuyintype.so"
    fi
}

build_windows() {
    local script=$desktop/packaging/windows/build.sh
    if [[ ! -f $script ]]; then
        fail "Windows 的建置腳本 platforms/desktop/packaging/windows/build.sh 還不存在，目前無法建置 Windows 版"
    fi
    local args=()
    [[ $release == 1 ]] && args+=(--release)
    [[ $package == 1 ]] && args+=(--package)
    step "Windows：packaging/windows/build.sh ${args[*]}"
    bash "$script" "${args[@]}" || fail "Windows 建置失敗（見上方）"
}

case "$target" in
    linux) build_linux ;;
    windows) build_windows ;;
    all) build_linux; build_windows ;;
esac

# The Linux build environment of the packages: Debian 13 (glibc 2.41, Fcitx 5.1.12), so the .deb and .rpm install on
# every distribution at least that new (Debian 13, Ubuntu 25.04 and later, Fedora 42 and later, openSUSE Tumbleweed).
# scripts/build.sh builds this image (tagged by the hash of this file and rust-toolchain.toml) and compiles in it.
# The official Rust image already holds the toolchain rust-toolchain.toml pins; RUSTUP_TOOLCHAIN makes cargo use it
# as it is, so nothing is downloaded at build time.
# build.sh passes the version from rust-toolchain.toml; the default only keeps the FROM line valid.
ARG RUST_VERSION=1
FROM rust:${RUST_VERSION}-slim-trixie
ARG RUST_VERSION
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}

# A nearer Debian mirror (ZT_DEBIAN_MIRROR for build.sh), e.g. http://free.nchc.org.tw/debian.
ARG DEBIAN_MIRROR
RUN if [ -n "$DEBIAN_MIRROR" ]; then \
      sed -i "s|^URIs: http://deb.debian.org/debian$|URIs: $DEBIAN_MIRROR|" /etc/apt/sources.list.d/debian.sources; \
    fi \
 && apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      ca-certificates curl git build-essential pkg-config cmake ninja-build python3 binutils dpkg-dev file \
      libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libwayland-dev libvulkan-dev libegl-dev \
      libfontconfig-dev libfreetype-dev \
      libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev fcitx5-modules-dev nlohmann-json3-dev \
 && rm -rf /var/lib/apt/lists/*

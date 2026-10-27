#!/usr/bin/env bash
# User-local Linux build; no system packages or services are modified.
set -euo pipefail
cd "$(dirname "$0")/../../.."
mkdir -p work2/debs work2/sysroot
cd work2/debs
apt download libboost-dev libboost1.90-dev \
  libboost-program-options1.90-dev libboost-program-options1.90.0 \
  libboost-thread1.90-dev libboost-thread1.90.0 libboost-test1.90-dev libboost-test1.90.0 \
  libboost-container1.90-dev libboost-container1.90.0 libboost-atomic1.90-dev libboost-atomic1.90.0 \
  libboost-chrono1.90-dev libboost-chrono1.90.0 libboost-iostreams1.90-dev libboost-iostreams1.90.0 \
  libboost-date-time1.90-dev libboost-date-time1.90.0 libboost-serialization1.90-dev \
  libboost-serialization1.90.0 libboost-exception1.90-dev zlib1g-dev libbz2-dev liblzma-dev libeigen3-dev
for file in *.deb; do dpkg-deb -x "$file" ../sysroot; done
cd ../..
if ! test -d work2/kenlm/.git; then git clone https://github.com/kpu/kenlm.git work2/kenlm; fi
git -C work2/kenlm checkout 4cb443e60b7bf2c0ddf3c745378f76cb59e254e5
# Boost.System became header-only and recent Boost no longer exports a system component.
sed -i '/^  system$/d' work2/kenlm/CMakeLists.txt
.venv-gen/bin/cmake -S work2/kenlm -B work2/kenlm/build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$PWD/work2/sysroot/usr" -DBOOST_ROOT="$PWD/work2/sysroot/usr" \
  -DENABLE_PYTHON=OFF -DKENLM_MAX_ORDER=3
.venv-gen/bin/cmake --build work2/kenlm/build --target lmplz build_binary query -j 24

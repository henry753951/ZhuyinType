#!/bin/sh
# The Rust engine's gate against the Swift engine, on the GPU host (needs ONNX Runtime and both toolchains).
#
#   engine/parity/gate.sh --quick [cli_parity options]   while developing: the first 50 lines of each set, minutes
#   engine/parity/gate.sh [cli_parity options]           before merging (and after merging main): everything
#
# Builds both CLIs (32 compile jobs), then runs at once: the CLI comparison of both suites (engine-only and the app's
# setup with models, 24 processes; cli_parity.py), --check-v3 with both CLIs (their outputs must be identical), and,
# in the full gate, the golden digests (the Swift tool on this host's data, then the Rust golden tests against them).
# Results go to $GATE_OUT (default gate-out/); the exit status is 0 only when every part passes.
# Run from the repository root; data/ must hold the evaluation data (data/build).  Other options, e.g.
# `--only default app`, go to cli_parity.py.
set -u
quick=
if [ "${1:-}" = --quick ]; then quick=--quick; shift; fi
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 1
out=${GATE_OUT:-gate-out}
mkdir -p "$out"
out=$(cd "$out" && pwd)
ort=${ONNXRUNTIME_ROOT:-$HOME/opt/onnxruntime}
swift=${ZT_SWIFT:-$(command -v swift || echo "$HOME/opt/swift/usr/bin/swift")}
export ONNXRUNTIME_ROOT=$ort ZT_SWIFT=$swift

"$swift" build -c release --package-path engine/swift -j 32 > "$out/swift-build.log" 2>&1 &
swift_build=$!
(cd engine && ORT_LIB_PATH=$ort/lib cargo build --release -p zhuyin-cli --features onnx -j 32) > "$out/rust-build.log" 2>&1 &
rust_build=$!
wait $swift_build || { echo "Swift build failed: $out/swift-build.log"; exit 1; }
wait $rust_build || { echo "Rust build failed: $out/rust-build.log"; exit 1; }
S=engine/swift/.build/release/zhuyin-cli
R=engine/target/release/zhuyin-cli

python3 engine/parity/cli_parity.py --swift $S --rust $R --jobs 24 $quick --output "$out/cli-parity.json" "$@" \
    > "$out/cli-parity.log" 2>&1 &
parity=$!
# Both CLIs score the v3 vectors with ONNX Runtime; the vectors are Core ML's outputs, so on Linux both report the
# same few outside Core ML's tolerance.  The gate is that the two reports are identical.
($S --check-v3 data/build/v3 > "$out/check-v3-swift.log" 2>&1; $R --check-v3 data/build/v3 > "$out/check-v3-rust.log" 2>&1;
 cmp -s "$out/check-v3-swift.log" "$out/check-v3-rust.log") &
check=$!
golden=
if [ -z "$quick" ]; then
    (
        dev=$PWD/data/build/corpus/dev_colloquial.txt:$PWD/data/build/corpus/dev_wikipedia.txt:$PWD/data/build/lm_chat/dev_chat_tt.txt
        export ZHUYIN_GOLDEN=$out/digests.tsv ZHUYIN_PARITY_DEV=$dev ZHUYIN_DATA=$PWD/data/build \
            ZHUYIN_PARITY_TEXT=$PWD/data/build/lm/test_sentences.txt
        engine/parity/golden.sh "$ZHUYIN_DATA" "$ZHUYIN_PARITY_TEXT" > "$out/golden-swift.log" 2>&1 &&
            (cd engine && cargo test --release -j 32 -p zhuyin-core --test golden --test golden_engine -- --nocapture) \
                > "$out/golden-rust.log" 2>&1 &&
            ! grep -q skipped "$out/golden-rust.log"
    ) &
    golden=$!
fi

status=0
if wait $parity; then echo "CLI parity: $(tail -n 1 "$out/cli-parity.log")"; else echo "CLI parity FAILED: $(tail -n 1 "$out/cli-parity.log")"; status=1; fi
if wait $check; then echo "check-v3: identical ($(cat "$out/check-v3-rust.log"))"; else echo "check-v3 FAILED: see $out/check-v3-*.log"; status=1; fi
if [ -n "$golden" ]; then
    if wait $golden; then echo "golden digests: identical"; else echo "golden digests FAILED: see $out/golden-*.log"; status=1; fi
fi
exit $status

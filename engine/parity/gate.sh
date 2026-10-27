#!/bin/sh
# The engine's behaviour gate on the GPU host: the zhuyin-cli of this checkout (the candidate) against one built from
# another checkout (the reference, e.g. main), line by line.  Needs ONNX Runtime ($ONNXRUNTIME_ROOT, default
# ~/opt/onnxruntime) and data/build.
#
#   engine/parity/gate.sh --quick --reference DIR [cli_parity options]   while developing: 50 lines per set, minutes
#   engine/parity/gate.sh --reference DIR [cli_parity options]           before merging (and after merging main)
#
# DIR is the reference checkout's root; each checkout builds in its own engine/target.  Builds both CLIs (32
# compile jobs), then runs at once: the CLI comparison of both suites (engine-only and the app's setup with models,
# 24 processes; cli_parity.py), --check-v3 with both CLIs (their reports must be identical) and, in the full gate,
# the engine's tests in release (`cargo test`, with the latency targets).  Results go to $GATE_OUT (default
# gate-out/); the exit status is 0 only when every part passes.  Run from the repository root.  Other options, e.g.
# `--only default app`, go to cli_parity.py.
set -u
quick=
reference=
while [ $# -gt 0 ]; do
    case $1 in
        --quick) quick=--quick; shift ;;
        --reference) reference=$2; shift 2 ;;
        *) break ;;
    esac
done
if [ -z "$reference" ]; then echo "usage: engine/parity/gate.sh [--quick] --reference DIR [cli_parity options]" >&2; exit 2; fi
root=$(cd "$(dirname "$0")/../.." && pwd)
reference=$(cd "$reference" && pwd) || exit 2
cd "$root" || exit 1
out=${GATE_OUT:-gate-out}
mkdir -p "$out"
out=$(cd "$out" && pwd)
export ORT_LIB_PATH=${ONNXRUNTIME_ROOT:-$HOME/opt/onnxruntime}/lib

build() { (cd "$1/engine" && cargo build --release --locked -p zhuyin-cli --features onnx -j 32) > "$2" 2>&1; }
build "$reference" "$out/reference-build.log" &
reference_build=$!
build "$root" "$out/candidate-build.log" &
candidate_build=$!
wait $reference_build || { echo "reference build failed: $out/reference-build.log"; exit 1; }
wait $candidate_build || { echo "candidate build failed: $out/candidate-build.log"; exit 1; }
R=$reference/engine/target/release/zhuyin-cli
C=$root/engine/target/release/zhuyin-cli

python3 engine/parity/cli_parity.py --reference "$R" --candidate "$C" --jobs 24 $quick --output "$out/cli-parity.json" \
    "$@" > "$out/cli-parity.log" 2>&1 &
parity=$!
# The v3 vectors are Core ML's outputs, so with ONNX Runtime both report the same few outside Core ML's tolerance
# (README: 比對); the gate is that the two reports are identical.
("$R" --check-v3 data/build/v3 > "$out/check-v3-reference.log" 2>&1; "$C" --check-v3 data/build/v3 > "$out/check-v3-candidate.log" 2>&1;
 cmp -s "$out/check-v3-reference.log" "$out/check-v3-candidate.log") &
check=$!
tests=
if [ -z "$quick" ]; then
    (cd engine && ZHUYIN_CHECKS_TIMING=warn cargo test --release --locked -j 16 --workspace) > "$out/tests.log" 2>&1 &
    tests=$!
fi

status=0
if wait $parity; then echo "CLI parity: $(tail -n 1 "$out/cli-parity.log")"; else echo "CLI parity FAILED: $(tail -n 1 "$out/cli-parity.log")"; status=1; fi
if wait $check; then echo "check-v3: identical ($(cat "$out/check-v3-candidate.log"))"; else echo "check-v3 FAILED: see $out/check-v3-*.log"; status=1; fi
if [ -n "$tests" ]; then
    if wait $tests; then echo "engine tests: passed"; else echo "engine tests FAILED: see $out/tests.log"; status=1; fi
fi
exit $status

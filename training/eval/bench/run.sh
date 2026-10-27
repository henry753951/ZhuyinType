#!/bin/bash
# ZhuyinBench on the GPU host (the self-hosted runner of .github/workflows/benchmark.yml, or by hand):
#   training/eval/bench/run.sh v0.4.0 [--quick]
# Fetches the pinned data and models, builds the engine for Linux, exports the ONNX models from the checkpoints
# (cached by checkpoint hash), runs suite.py and rebuilds docs/benchmarks/.
set -euo pipefail
cd "$(dirname "$0")/../../.."
version=${1:?usage: run.sh VERSION [--quick]}
shift
export PATH="$HOME/opt/swift/usr/bin:$PATH" ONNXRUNTIME_ROOT=${ONNXRUNTIME_ROOT:-$HOME/opt/onnxruntime}
export PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
torch_python=${ZHUYIN_TORCH_PYTHON:-$HOME/zhuyintype/work/onnx-eval-claude/.venv/bin/python}
python3 nlp/data/assets.py fetch lexicon lm lm_chat reranker v3 eval-sets checkpoints
swift build -c release --package-path nlp/engine --scratch-path .build
stamp=$(cat data/build/reranker/charlm.pt.sha256 2>/dev/null || true)
current=$(sha256sum data/build/reranker/charlm.pt data/build/v3/context.pt data/build/v3/syllable.pt \
    training/remote_eval/export_onnx.py | sha256sum | cut -d' ' -f1)
if [[ "$stamp" != "$current" || ! -f data/build/reranker/RerankerTree.onnx ]]; then
    "$torch_python" training/remote_eval/export_onnx.py --reranker-checkpoint data/build/reranker/charlm.pt \
        --reranker data/build/reranker --v3 data/build/v3 --tree
    echo "$current" > data/build/reranker/charlm.pt.sha256
fi
python3 training/eval/bench/suite.py --version "$version" --cli .build/release/zhuyin-cli \
    --jobs "${JOBS:-$(nproc)}" --out "docs/benchmarks/$version.json" "$@"
python3 training/eval/bench/history.py

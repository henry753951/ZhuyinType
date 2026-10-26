#!/usr/bin/env bash
# Optional experiment; CPU ONNX is the default. Run only in the isolated remote workspace.
set -euo pipefail
cd "$(dirname "$0")/../.."
PYTHON=${PYTHON:-.venv/bin/python}
SOCKET="/tmp/zhuyintype-onnx-benchmark-$UID-$$.sock"
mkdir -p data/build/remote_eval/gpu
"$PYTHON" -u training/remote_eval/model_server.py --socket "$SOCKET" \
    --reranker data/build/reranker/charlm.pt --v3 data/build/v3/syllable.pt \
    --memory-gb 2 --device cuda > data/build/remote_eval/gpu-server.log 2>&1 &
SERVER=$!
trap 'kill "$SERVER" 2>/dev/null || true; wait "$SERVER" 2>/dev/null || true; rm -f "$SOCKET"' EXIT
for ((i=0;i<120;i++)); do
    [[ -S "$SOCKET" ]] && break
    kill -0 "$SERVER" || { cat data/build/remote_eval/gpu-server.log; exit 1; }
    sleep 1
done
[[ -S "$SOCKET" ]]
"$PYTHON" - "$SOCKET" <<'PY'
import json, sys
from pathlib import Path
p=Path('training/remote_eval/validation.json')
m=json.loads(p.read_text())
for c in m['configurations']:
    c['args'] += ['--model-server', sys.argv[1]]
Path('data/build/remote_eval/gpu-matrix.json').write_text(json.dumps(m, indent=2))
PY
"$PYTHON" -u training/remote_eval/sweep.py --matrix data/build/remote_eval/gpu-matrix.json \
    --jobs "${JOBS:-60}" --cli .build/release/zhuyin-cli --output data/build/remote_eval/gpu -n 400 \
    2>&1 | tee data/build/remote_eval/gpu-sweep.log

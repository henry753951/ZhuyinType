#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
HOST=${HOST:-$ZHUYIN_REMOTE_HOST}
KEY=${KEY:-$ZHUYIN_REMOTE_KEY}
REMOTE_DIR=${REMOTE_DIR:-zhuyintype/work/onnx-eval}
JOBS=${JOBS:-60}
BACKEND=${BACKEND:-cpu}
EXTRA_SYNC=${EXTRA_SYNC:-}
V3_MODEL=${V3_MODEL:-v3}
RERANKER_MODEL=${RERANKER_MODEL:-reranker}
[[ "$V3_MODEL" =~ ^v3[_a-zA-Z0-9-]*$ && "$RERANKER_MODEL" =~ ^reranker[_a-zA-Z0-9-]*$ ]] || { echo 'Invalid model directory name' >&2; exit 1; }
[[ "$BACKEND" == cpu || "$BACKEND" == cuda ]] || { echo "BACKEND must be cpu or cuda" >&2; exit 1; }
[[ "$REMOTE_DIR" =~ ^zhuyintype/work/onnx-eval[-a-zA-Z0-9_/]*$ ]] || { echo 'Invalid isolated REMOTE_DIR' >&2; exit 1; }
SSH=(ssh -o ConnectTimeout=15 -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -i "$KEY" "$HOST")
export RSYNC_RSH="ssh -o ConnectTimeout=15 -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -i $KEY"
"${SSH[@]}" "mkdir -p ~/$REMOTE_DIR"
# Copy only the portable products; leave app and training workspaces alone. No --delete.
rsync -azcR Package.swift Tests/ZhuyinMLTests Sources/ZhuyinCore Sources/ZhuyinCoreChecks Sources/ZhuyinML Sources/ONNXBridge Sources/zhuyin-cli \
    training/eval training/remote_eval/*.py training/remote_eval/*.json training/remote_eval/*.sh training/remote_eval/requirements.txt \
    training/reranker/train.py training/v3/models.py \
    data/build/lexicon.tsv data/build/lm/{unigram,bigram,backoff}.tsv \
    data/build/lm_chat/{unigram,bigram,backoff}.tsv data/build/lm_chat/*_chat_tt.txt \
    data/build/corpus/{dev_colloquial,dev_wikipedia,test_colloquial}.txt data/build/lm/test_sentences.txt \
    data/build/reranker/{vocab,config}.json data/build/v3/{chars,syllables,config,test_vectors}.json \
    data/build/v3/{context,syllable}.pt "$HOST:$REMOTE_DIR/"
if [[ -f data/build/reranker/charlm.pt ]]; then
    rsync -azc data/build/reranker/charlm.pt "$HOST:$REMOTE_DIR/data/build/reranker/"
else
    "${SSH[@]}" "cp -p ~/zhuyintype/reranker_v2b/charlm.pt ~/$REMOTE_DIR/data/build/reranker/charlm.pt"
fi
if [[ -n "$EXTRA_SYNC" ]]; then
    read -ra EXTRA_FILES <<< "$EXTRA_SYNC"
    rsync -azcR "${EXTRA_FILES[@]}" "$HOST:$REMOTE_DIR/"
fi
if [[ "$V3_MODEL" != v3 ]]; then
    rsync -azcR data/build/"$V3_MODEL"/{context,syllable}.pt data/build/"$V3_MODEL"/{chars,syllables,config}.json "$HOST:$REMOTE_DIR/"
fi
if [[ "$RERANKER_MODEL" != reranker ]]; then
    rsync -azcR data/build/"$RERANKER_MODEL"/{charlm.pt,vocab.json,config.json} "$HOST:$REMOTE_DIR/"
fi
# Always retrieve logs/results, including an incomplete run on failure.
collect() {
    mkdir -p data/build/remote_eval/remote
    rsync -az "$HOST:$REMOTE_DIR/data/build/remote_eval/" data/build/remote_eval/remote/ || true
}
trap collect EXIT
"${SSH[@]}" bash -s -- "$REMOTE_DIR" "$JOBS" "$BACKEND" "$V3_MODEL" "$RERANKER_MODEL" "$@" <<'REMOTE'
set -euo pipefail
cd "$HOME/$1"
JOBS=$2
BACKEND=$3
V3_MODEL=$4
RERANKER_MODEL=$5
shift 5
export PATH="$HOME/opt/swift/usr/bin:$PATH"
export PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
mkdir -p data/build/remote_eval
PYTHON="$PWD/.venv/bin/python"
[[ -x "$PYTHON" ]] || { echo 'Follow README.md to install the isolated ONNX export environment.' >&2; exit 1; }
swift --version
swift build -c release 2>&1 | tee data/build/remote_eval/build.log
# Re-export only when checkpoint/export source contents change, not once per sweep.
sha256sum data/build/"$RERANKER_MODEL"/charlm.pt data/build/"$V3_MODEL"/{context,syllable}.pt \
    training/remote_eval/export_onnx.py training/reranker/train.py training/v3/models.py > data/build/remote_eval/export-inputs.current
if ! cmp -s data/build/remote_eval/export-inputs.current data/build/remote_eval/export-inputs.done; then
    "$PYTHON" training/remote_eval/export_onnx.py --reranker-checkpoint data/build/"$RERANKER_MODEL"/charlm.pt \
        --reranker data/build/"$RERANKER_MODEL" --v3 data/build/"$V3_MODEL" \
        2>&1 | tee data/build/remote_eval/export.log
    cp data/build/remote_eval/export-inputs.current data/build/remote_eval/export-inputs.done
fi
MODEL_ARGS=()
if [[ "$BACKEND" == cuda ]]; then
    SOCKET="/tmp/zhuyintype-eval-$UID-$$.sock"
    "$PYTHON" -u training/remote_eval/model_server.py --socket "$SOCKET" \
        --reranker data/build/"$RERANKER_MODEL"/charlm.pt --v3 data/build/"$V3_MODEL"/syllable.pt \
        --device cuda --memory-gb 2 > data/build/remote_eval/model-server.log 2>&1 &
    SERVER_PID=$!
    trap 'kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; rm -f "$SOCKET"' EXIT
    for ((i=0;i<120;i++)); do
        [[ -S "$SOCKET" ]] && break
        kill -0 "$SERVER_PID" || { cat data/build/remote_eval/model-server.log; exit 1; }
        sleep 1
    done
    [[ -S "$SOCKET" ]] || { cat data/build/remote_eval/model-server.log; exit 1; }
    MODEL_ARGS=(--cli-arg=--model-server --cli-arg="$SOCKET")
fi
if [[ "${1:-}" == homophone ]]; then
    shift
    "$PYTHON" -u training/homophone/experiment.py --cli .build/release/zhuyin-cli --jobs "$JOBS" "${MODEL_ARGS[@]}" "$@" \
        2>&1 | tee data/build/remote_eval/evaluation.log
elif [[ "${1:-}" == sweep ]]; then
    shift
    "$PYTHON" -u training/remote_eval/sweep.py --cli .build/release/zhuyin-cli --jobs "$JOBS" "${MODEL_ARGS[@]}" "$@" \
        2>&1 | tee data/build/remote_eval/evaluation.log
else
    TOOL=${1:-conversion_eval}
    if [[ $# -gt 0 ]]; then shift; fi
    "$PYTHON" -u training/remote_eval/evaluate.py --jobs "$JOBS" "${MODEL_ARGS[@]}" "$TOOL" \
        --cli .build/release/zhuyin-cli "$@" 2>&1 | tee data/build/remote_eval/evaluation.log
fi
REMOTE

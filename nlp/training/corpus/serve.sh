#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
while ! test -f work2/Qwen3-8B/model-00005-of-00005.safetensors || pgrep -f 'snapshot_download.*Qwen3-8B' >/dev/null; do sleep 10; done
export OMP_NUM_THREADS=8
export VLLM_USE_FLASHINFER_SAMPLER=0
exec .venv-gen/bin/vllm serve work2/Qwen3-8B --served-model-name Qwen3-8B --host 127.0.0.1 --port 8102 --dtype bfloat16 --max-model-len 4096 --gpu-memory-utilization 0.92 --max-num-seqs 192 --max-num-batched-tokens 8192 --generation-config vllm

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Reserve headroom for the existing evaluation/training processes; never restart their server.
FREE=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | head -1)
[[ "$FREE" -ge 40500 ]] || { echo "Need >=40500 MiB free; found $FREE" >&2; exit 1; }
export OMP_NUM_THREADS=4 VLLM_USE_FLASHINFER_SAMPLER=0 PYTHONDONTWRITEBYTECODE=1
exec .venv-gen/bin/vllm serve work2/Qwen3-8B --served-model-name Qwen3-8B \
  --host 127.0.0.1 --port 8112 --dtype bfloat16 --max-model-len 4096 \
  --gpu-memory-utilization 0.80 --max-num-seqs 128 --max-num-batched-tokens 8192 \
  --generation-config vllm

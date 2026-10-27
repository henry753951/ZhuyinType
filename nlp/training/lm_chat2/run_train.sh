#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
export PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
PYTHON="$PWD/.venv/bin/python"
"$PYTHON" nlp/training/lm_chat2/test_pipeline.py
"$PYTHON" -u nlp/training/lm_chat2/manifest.py
for variant in real mixed; do
    "$PYTHON" -u nlp/training/lm_chat2/prepare.py prepare --variant "$variant" --workers 24
    "$PYTHON" -u nlp/training/lm_chat2/train.py --variant "$variant" --workers 24
done

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
synthetic=()
if [ "$#" -gt 0 ]; then
    synthetic=(--synthetic "$1")
else
    while ! test -f data/build/corpus/PHASE1_READY; do sleep 20; done
fi
.venv-gen/bin/python training/corpus/mine_words.py "${synthetic[@]}"
.venv/bin/python training/lm/train2.py --workers 48 "${synthetic[@]}"
.venv/bin/python training/lm/audit2.py
.venv/bin/python training/lm/report2.py

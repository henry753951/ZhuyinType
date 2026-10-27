#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
for spec in "chat_fresh_hard hard 0" "chat_fresh_expected expected 0" "chat_fresh_generated01 expected 0.1"; do
 read -r name method weight <<< "$spec"
 .venv/bin/python -u training/base/train.py fit --source-root "$HOME/zhuyintype" --domain chat --name "$name" --method "$method" --generated-weight "$weight" --fresh --lexicon data/build/base/lexicon_source.tsv --workers 8
done

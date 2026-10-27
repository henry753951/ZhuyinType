#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
source_root="$HOME/zhuyintype"
prepare_pid=$(cat data/build/base/prepare.pid)
while [[ ! -f data/build/base/corpus/prepared.json ]]; do
    kill -0 "$prepare_pid" || { echo 'Preparation stopped without completion'; exit 1; }
    sleep 5
done
for spec in 'chat_hard chat hard 0' 'chat_expected chat expected 0' 'chat_generated01 chat expected 0.1' 'wiki_hard wiki hard 0' 'wiki_expected wiki expected 0'; do
    read -r name domain method generated <<< "$spec"
    .venv/bin/python -u training/base/train.py fit --source-root "$source_root" \
        --name "$name" --domain "$domain" --method "$method" --generated-weight "$generated" --workers 8 \
        --lexicon data/build/base/lexicon_mcbopomofo.tsv
done

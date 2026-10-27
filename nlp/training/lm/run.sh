#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
export PYTHONDONTWRITEBYTECODE=1
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
workers="${WORKERS:-64}"
python="${LM_PYTHON:-$PWD/.venv/bin/python}"
"$python" -m unittest discover -s nlp/training/lm -p test_pipeline.py -v
"$python" nlp/training/lm/download.py raw
"$python" nlp/training/lm/pipeline.py \
  --dump raw/zhwiki-latest-pages-articles.xml.bz2 \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm --workers "$workers"
"$python" nlp/training/lm/verify.py \
  --lexicon data/build/lexicon.tsv --work work --output data/build/lm --workers "$workers"

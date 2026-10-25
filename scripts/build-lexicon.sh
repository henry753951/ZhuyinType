#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -s data/build/lexicon.tsv ]]; then
    printf '%s\n' '沿用詞庫：data/build/lexicon.tsv'
    exit 0
fi
if [[ ! -d data/raw ]]; then
    data/tools/fetch.sh
fi
if [[ ! -x .venv/bin/python ]] || ! .venv/bin/python -c 'import wordfreq' 2>/dev/null; then
    if ! command -v uv >/dev/null; then
        echo '建立詞庫的 Python 環境需要 uv。請先安裝 uv，再重試。' >&2
        exit 1
    fi
    if [[ ! -x .venv/bin/python ]]; then uv venv .venv; fi
    uv pip install --python .venv/bin/python wordfreq
fi
.venv/bin/python data/tools/build_lexicon.py
test -s data/build/lexicon.tsv

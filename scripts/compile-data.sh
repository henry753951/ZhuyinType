#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
[[ $# == 1 ]] || { echo 'Usage: scripts/compile-data.sh OUT_DIR' >&2; exit 1; }
# No third-party packages, dependency restore, or network access.
exec "$ROOT/.venv/bin/python" "$ROOT/training/lm/compile_data.py" "$1"

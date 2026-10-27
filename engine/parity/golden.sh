#!/bin/sh
# Regenerates golden/digests.tsv (committed) and the full Swift dumps in out/ (ignored) from the Swift engine's code.
# Usage: [ZHUYIN_PARITY_DEV=colloquial:wikipedia:chat] parity/golden.sh DATA TEXT [TRIGRAM_LM_DIR]
#   DATA  a directory with lexicon.bin, lexicon.tsv, lm/ and lm_chat/ (the app's Resources layout)
#   TEXT  sentences for the language-model sweeps, one per line
# The Rust golden tests read the same inputs from ZHUYIN_DATA, ZHUYIN_PARITY_TEXT and ZHUYIN_TRIGRAM_LM.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data=$1 text=$2 trigram=${3:-}
size() { if [ -e "$1" ]; then stat -f %z "$1" 2>/dev/null || stat -c %s "$1"; fi; }
swift build -c release --package-path "$here" >&2
mkdir -p "$here/golden" "$here/out"
{
    printf '#size\tlexicon.bin\t%s\n' "$(size "$data/lexicon.bin")"
    printf '#size\tlexicon.tsv\t%s\n' "$(size "$data/lexicon.tsv")"
    printf '#size\tlm/lm.bin\t%s\n' "$(size "$data/lm/lm.bin")"
    printf '#size\tlm_chat/lm.bin\t%s\n' "$(size "$data/lm_chat/lm.bin")"
    printf '#size\ttext\t%s\n' "$(size "$text")"
    if [ -n "$trigram" ]; then printf '#size\ttrigram\t%s\n' "$(size "$trigram/lm.bin")"; fi
    i=0
    for dev in $(printf '%s' "${ZHUYIN_PARITY_DEV:-}" | tr ':' ' '); do printf '#size\tdev%s\t%s\n' "$i" "$(size "$dev")"; i=$((i + 1)); done
    ZHUYIN_TRIGRAM_LM=$trigram "$here/.build/release/SwiftParity" golden "$data" "$text" "$here/out"
} > "$here/golden/digests.tsv"
cat "$here/golden/digests.tsv"

#!/bin/sh
# Regenerates golden/digests.tsv (committed) and the full Swift dumps in out/ (ignored) from the Swift engine's code.
# Usage: [ZHUYIN_PARITY_DEV=colloquial:wikipedia:chat] parity/golden.sh DATA TEXT [TRIGRAM_LM_DIR]
#   DATA  a directory with lexicon.bin, lexicon.tsv, lm/ and lm_chat/ (the app's Resources layout)
#   TEXT  sentences for the language-model sweeps, one per line
# The Rust golden tests read the same inputs from ZHUYIN_DATA, ZHUYIN_PARITY_TEXT and ZHUYIN_TRIGRAM_LM.  ZHUYIN_GOLDEN writes
# the digests to another file (the Rust tests read it from the same variable), leaving the committed ones as they are.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data=$1 text=$2 trigram=${3:-}
size() { if [ -e "$1" ]; then wc -c < "$1" | tr -d ' '; fi; }
"${ZT_SWIFT:-swift}" build -c release --package-path "$here" >&2
digests=${ZHUYIN_GOLDEN:-$here/golden/digests.tsv}
mkdir -p "$(dirname "$digests")" "$here/out"
{
    # The system the digests hold for (Rust's std::env::consts::OS), see crates/zhuyin-core/tests/common/parity.rs.
    case $(uname -s) in Darwin) printf '#system\tmacos\n' ;; Linux) printf '#system\tlinux\n' ;; esac
    printf '#size\tlexicon.bin\t%s\n' "$(size "$data/lexicon.bin")"
    printf '#size\tlexicon.tsv\t%s\n' "$(size "$data/lexicon.tsv")"
    printf '#size\tlm/lm.bin\t%s\n' "$(size "$data/lm/lm.bin")"
    printf '#size\tlm_chat/lm.bin\t%s\n' "$(size "$data/lm_chat/lm.bin")"
    printf '#size\ttext\t%s\n' "$(size "$text")"
    if [ -n "$trigram" ]; then printf '#size\ttrigram\t%s\n' "$(size "$trigram/lm.bin")"; fi
    i=0
    for dev in $(printf '%s' "${ZHUYIN_PARITY_DEV:-}" | tr ':' ' '); do printf '#size\tdev%s\t%s\n' "$i" "$(size "$dev")"; i=$((i + 1)); done
    ZHUYIN_TRIGRAM_LM=$trigram "$here/.build/release/SwiftParity" golden "$data" "$text" "$here/out"
} > "$digests"
cat "$digests"

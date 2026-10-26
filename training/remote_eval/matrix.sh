#!/usr/bin/env bash
set -eo pipefail
# Run from the frozen project root. The server is managed by the caller.
SIDE=$1
JOBS=$2
SOCKET=${3:-}
BASE=(--cli .build-codex2/release/zhuyin-cli --jobs "$JOBS" -n 400)
if [[ "$SIDE" == linux ]]; then
    BASE=(--cli .build/release/zhuyin-cli --jobs "$JOBS" -n 400 --cli-arg=--model-server --cli-arg="$SOCKET")
fi
mkdir -p results
for CONFIG in plain reranker v3 context continue; do
    EXTRA=()
    if [[ "$CONFIG" != plain ]]; then
        EXTRA+=(--reranker data/build/reranker --rerank-count 16)
    fi
    if [[ "$CONFIG" == v3 || "$CONFIG" == context || "$CONFIG" == continue ]]; then
        EXTRA+=(--cli-arg=--v3 --cli-arg=data/build/v3 --cli-arg=--v3-weight --cli-arg=1)
    fi
    if [[ "$CONFIG" == context ]]; then EXTRA+=(--line-context); fi
    if [[ "$CONFIG" == continue ]]; then EXTRA+=(--split continue); fi
    "${PYTHON:-python3}" -u training/remote_eval/capture_eval.py --output "results/$SIDE-$CONFIG.json" \
        "${BASE[@]}" "${EXTRA[@]}" --test data/build/corpus/dev_colloquial.txt \
        data/build/corpus/dev_wikipedia.txt data/build/lm_chat/dev_chat_tt.txt | tee "results/$SIDE-$CONFIG.log"
done

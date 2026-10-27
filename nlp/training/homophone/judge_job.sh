#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/../../.."
server_pid=${1:?verified judge server PID required}
shift
stop_judge() {
    command=$(ps -p "$server_pid" -o args=)
    if [[ "$command" == *'vllm serve work2/Qwen3-8B'* && "$command" == *'--port 8102'* ]]; then
        kill -TERM "$server_pid"
    fi
}
trap stop_judge EXIT
PYTHONDONTWRITEBYTECODE=1 .venv-gen/bin/python -u nlp/training/homophone/judge.py "$@"
status=$?
printf '%s\n' "$status" > logs/homophone-judge.status
exit "$status"

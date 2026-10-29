#!/usr/bin/env bash
set -euo pipefail
host=$(realpath "${1:?Usage: with-host.sh HOST TEST [ARGS...]}")
shift
work=$(mktemp -d)
pid=
cleanup() {
    status=$?
    if [[ -n $pid ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
    if [[ $status != 0 ]]; then tail -30 "$work/host.log"; fi
    rm -rf "$work"
}
trap cleanup EXIT
export ZT_IME_SOCKET="$work/ime-v1"
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/share" XDG_CACHE_HOME="$work/cache"
mkdir -p "$work/assets" "$work/personal"
cat > "$work/assets/lexicon.tsv" <<'LEXICON'
ㄋㄧˇ	你	-3
ㄋㄧˇ	妳	-4
ㄏㄠˇ	好	-3
ㄋㄧˇ ㄏㄠˇ	你好	-2
LEXICON
# Host consumes public C ABI data; native addon only sees the broker endpoint.
QT_QPA_PLATFORM=${ZT_HOST_PLATFORM:-offscreen} QT_IM_MODULE=compose "$host" --ime-host \
    --ime-socket "$ZT_IME_SOCKET" --lexicon "$work/assets/lexicon.tsv" \
    > "$work/host.log" 2>&1 &
pid=$!
ready=0
for ((attempt=0; attempt<100; ++attempt)); do
    if [[ -S $ZT_IME_SOCKET ]]; then ready=1; break; fi
    if ! kill -0 "$pid" 2>/dev/null; then break; fi
    sleep 0.05
done
[[ $ready == 1 ]]
"$@"

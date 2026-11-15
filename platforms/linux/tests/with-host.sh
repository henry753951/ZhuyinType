#!/usr/bin/env bash
# Runs TEST against a real host on an isolated endpoint, settings and personal data:
#   with-host.sh HOST TEST [ARGS...]
# The host is the app binary `zhuyintype` (`--background`: the input method host without a window).  When it is
# missing or does not serve the broker within 5 s the test is skipped (exit 77) unless ZT_REQUIRE_HOST=1.
set -euo pipefail
host=${1:?Usage: with-host.sh HOST TEST [ARGS...]}
shift
skip() {
    echo "SKIP: $1" >&2
    [[ ${ZT_REQUIRE_HOST:-0} == 1 ]] && exit 1
    exit 77
}
[[ -x $host ]] || skip "host $host is not built"
host=$(realpath "$host")
work=$(mktemp -d)
pid=
cleanup() {
    status=$?
    if [[ -n $pid ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
    if [[ $status != 0 && $status != 77 ]]; then tail -30 "$work/host.log"; fi
    rm -rf "$work"
}
trap cleanup EXIT
export ZT_IME_SOCKET="$work/ime-v1"
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/share" XDG_CACHE_HOME="$work/cache"
mkdir -p "$work/assets"
cat > "$work/assets/lexicon.tsv" <<'LEXICON'
ㄋㄧˇ	你	-3
ㄋㄧˇ	妳	-4
ㄏㄠˇ	好	-3
ㄋㄧˇ ㄏㄠˇ	你好	-2
LEXICON
# The host reads the lexicon through the engine; the addon only sees the broker endpoint.
"$host" --background --ime-socket "$ZT_IME_SOCKET" --lexicon "$work/assets/lexicon.tsv" > "$work/host.log" 2>&1 &
pid=$!
for ((attempt=0; attempt<100; ++attempt)); do
    [[ -S $ZT_IME_SOCKET ]] && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.05
done
[[ -S $ZT_IME_SOCKET ]] || skip "host does not serve the broker at --ime-socket yet (see platforms/desktop/IPC.md)"
"$@"

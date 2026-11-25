#!/usr/bin/env bash
# Runs TEST against a real host on an isolated endpoint, settings and personal data:
#   with-host.sh HOST TEST [ARGS...]
# ZT_HOST_ASSOCIATED=1 turns 聯想詞 on (and adds the language model they come from).
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
xvfb=
cleanup() {
    status=$?
    if [[ -n $xvfb ]]; then kill "$xvfb" 2>/dev/null || true; fi
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
# ZT_HOST_ASSOCIATED=1: 聯想詞 on, with a language model in which 你, 妳 and 你好 follow 你好.
if [[ ${ZT_HOST_ASSOCIATED:-0} == 1 ]]; then
    mkdir -p "$XDG_CONFIG_HOME/zhuyintype" "$work/assets/lm"
    printf '{"nextWordSuggestions": true}\n' > "$XDG_CONFIG_HOME/zhuyintype/settings.json"
    printf '你好\t-3\n你\t-2\n妳\t-3\n好\t-2\n</s>\t-1\n' > "$work/assets/lm/unigram.tsv"
    # 你好 after 你好 too, or typing it again after 你好 would convert to 你 + 好 (你 is likely after 你好).
    printf '<s>\t你好\t-1\n你好\t你\t-0.5\n你好\t妳\t-1\n你好\t你好\t-1.5\n' > "$work/assets/lm/bigram.tsv"
    : > "$work/assets/lm/backoff.tsv"
fi
# The host reads the lexicon through the engine; the addon only sees the broker endpoint.  The host is a GPUI app
# and ends when it has no display, so a headless run (CTest, CI) gives it a virtual one.
if [[ -z ${DISPLAY:-} && -z ${WAYLAND_DISPLAY:-} ]]; then
    command -v Xvfb >/dev/null || skip "the host needs a display: install xvfb"
    display=:$((200 + RANDOM % 300))
    Xvfb "$display" -nolisten tcp >/dev/null 2>&1 &
    xvfb=$!
    export DISPLAY=$display
    for ((attempt=0; attempt<50; ++attempt)); do [[ -e /tmp/.X11-unix/X${display#:} ]] && break; sleep 0.05; done
fi
"$host" --background --ime-socket "$ZT_IME_SOCKET" --lexicon "$work/assets/lexicon.tsv" > "$work/host.log" 2>&1 &
pid=$!
for ((attempt=0; attempt<100; ++attempt)); do
    [[ -S $ZT_IME_SOCKET ]] && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.05
done
[[ -S $ZT_IME_SOCKET ]] || skip "host does not serve the broker at --ime-socket yet (see platforms/desktop/IPC.md)"
"$@"

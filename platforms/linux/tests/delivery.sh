#!/usr/bin/env bash
set -euo pipefail

# Isolated bus, display, profile, personal data and addon search path. No changes
# to the desktop's running Fcitx process or input-method configuration.
# The host is the app binary `zhuyintype`: `--background` for the input method host alone, no argument for the host
# with its settings window (ZT_DELIVERY_SELF_HOST=1).  Skipped (exit 77) while the host is missing or does not serve
# the broker, unless ZT_REQUIRE_HOST=1.
build=$(realpath "${1:?Usage: delivery.sh BUILD_DIRECTORY HOST}")
host=${2:?Host executable required}
skip() {
    echo "SKIP: $1" >&2
    [[ ${ZT_REQUIRE_HOST:-0} == 1 ]] && exit 1
    exit 77
}
[[ -x $host ]] || skip "host $host is not built"
host=$(realpath "$host")
if [[ ${ZT_DELIVERY_SESSION:-0} != 1 ]]; then
    exec xvfb-run -a dbus-run-session -- env ZT_DELIVERY_SESSION=1 bash "$0" "$build" "$host"
fi
work=$(mktemp -d)
daemon=
host_pid=
cleanup() {
    status=$?
    if [[ -n $daemon ]]; then kill "$daemon" 2>/dev/null || true; wait "$daemon" 2>/dev/null || true; fi
    if [[ -n $host_pid ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi
    if [[ $status != 0 && $status != 77 ]]; then tail -n 40 "$work/fcitx.log" "$work/host.log"; fi
    rm -rf "$work"
}
trap cleanup EXIT
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/share" XDG_CACHE_HOME="$work/cache"
export XDG_CONFIG_DIRS="$work/config" XDG_DATA_DIRS="$work/share:/usr/share"
source "$build/fcitx-test-paths.sh"
export FCITX_ADDON_DIRS="$build:$system_addon_dir" QT_QPA_PLATFORM=xcb QT_IM_MODULE=fcitx
if [[ ${ZT_DELIVERY_INSTALLED:-0} == 1 ]]; then export FCITX_ADDON_DIRS="$system_addon_dir"; fi
export XMODIFIERS=@im=fcitx
export ZT_IME_SOCKET="$work/ime-v1"
assets="$work/assets"
mkdir -p "$XDG_CONFIG_HOME/fcitx5" "$XDG_DATA_HOME/fcitx5/addon" "$XDG_DATA_HOME/fcitx5/inputmethod" "$assets"
source_dir=$(cd "$(dirname "$0")/.." && pwd)
cp "$build/zhuyintype-addon.conf" "$XDG_DATA_HOME/fcitx5/addon/zhuyintype.conf"
cp "$source_dir/data/zhuyintype.conf" "$XDG_DATA_HOME/fcitx5/inputmethod/zhuyintype.conf"
cat > "$XDG_CONFIG_HOME/fcitx5/profile" <<'PROFILE'
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=zhuyintype

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=zhuyintype
Layout=

[GroupOrder]
0=Default
PROFILE
cat > "$assets/lexicon.tsv" <<'LEXICON'
ㄋㄧˇ	你	-3
ㄋㄧˇ	妳	-4
ㄏㄠˇ	好	-3
ㄋㄧˇ ㄏㄠˇ	你好	-2
LEXICON
# xim: the host's own window (GPUI) reaches Fcitx through XIM; the Qt probe uses the D-Bus frontend.
fcitx5 -D --disable all --enable dbus,dbusfrontend,xcb,xim,classicui,keyboard,zhuyintype > "$work/fcitx.log" 2>&1 &
daemon=$!
ready=0
for ((attempt=0; attempt<50; ++attempt)); do
    if dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus.NameHasOwner string:org.fcitx.Fcitx5 2>/dev/null | grep -q true; then ready=1; break; fi
    if ! kill -0 "$daemon" 2>/dev/null; then exit 1; fi
    sleep 0.1
done
[[ $ready == 1 ]]
# The host after Fcitx, as when the addon starts it: GPUI looks for an XIM server once, when it starts, so its own
# text fields (the self-host case) use the input method only if Fcitx is already there.
host_args=(--background)
if [[ ${ZT_DELIVERY_SELF_HOST:-0} == 1 ]]; then
    # The settings window as a returning user has it: fresh settings would open the setup wizard over it.
    printf '{"setupWizardSeenVersion": "999.0.0"}\n' > "$work/settings.json"
    host_args=(--page keys --settings-file "$work/settings.json")
fi
# ZT_DELIVERY_REAL_DATA=1: the host's own engine data (an installed package) instead of the four-word fixture.
lexicon=(--lexicon "$assets/lexicon.tsv")
if [[ ${ZT_DELIVERY_REAL_DATA:-0} == 1 ]]; then lexicon=(); fi
"$host" "${host_args[@]}" --ime-socket "$ZT_IME_SOCKET" "${lexicon[@]}" > "$work/host.log" 2>&1 &
host_pid=$!
export ZT_DELIVERY_HOST_PID="$host_pid"
for ((attempt=0; attempt<100; ++attempt)); do
    [[ -S $ZT_IME_SOCKET ]] && break
    kill -0 "$host_pid" 2>/dev/null || break
    sleep 0.05
done
[[ -S $ZT_IME_SOCKET ]] || skip "host does not serve the broker at --ime-socket yet (see platforms/desktop/IPC.md)"
if [[ ${ZT_DELIVERY_SELF_HOST:-0} == 1 ]]; then
    QT_IM_MODULE=compose "$build/fcitx-self-host-probe" "$host_pid"
else
    "$build/fcitx-delivery-probe"
fi

#!/usr/bin/env bash
set -euo pipefail

# Isolated bus, display, profile, personal data and addon search path. No changes
# to the desktop's running Fcitx process or input-method configuration.
build=$(realpath "${1:?Usage: delivery.sh BUILD_DIRECTORY HOST}")
host=$(realpath "${2:?Shared Qt host required}")
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
    if [[ $status != 0 ]]; then tail -n 40 "$work/fcitx.log" "$work/host.log"; fi
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
cp "$source_dir/data/zhuyintype-addon.conf" "$XDG_DATA_HOME/fcitx5/addon/zhuyintype.conf"
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
host_args=(--ime-host)
host_im_module=compose
if [[ ${ZT_DELIVERY_SELF_HOST:-0} == 1 ]]; then host_args=(); host_im_module=fcitx; fi
QT_IM_MODULE="$host_im_module" "$host" "${host_args[@]}" --ime-socket "$ZT_IME_SOCKET" --lexicon "$assets/lexicon.tsv" > "$work/host.log" 2>&1 &
host_pid=$!
export ZT_DELIVERY_HOST_PID="$host_pid"
for ((attempt=0; attempt<100; ++attempt)); do
    [[ -S $ZT_IME_SOCKET ]] && break
    kill -0 "$host_pid" 2>/dev/null || exit 1
    sleep 0.05
done
[[ -S $ZT_IME_SOCKET ]]
fcitx5 -D -k --disable all --enable dbus,dbusfrontend,xcb,classicui,keyboard,zhuyintype > "$work/fcitx.log" 2>&1 &
daemon=$!
ready=0
for ((attempt=0; attempt<50; ++attempt)); do
    if dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus.NameHasOwner string:org.fcitx.Fcitx5 2>/dev/null | grep -q true; then ready=1; break; fi
    if ! kill -0 "$daemon" 2>/dev/null; then exit 1; fi
    sleep 0.1
done
[[ $ready == 1 ]]
if [[ ${ZT_DELIVERY_SELF_HOST:-0} == 1 ]]; then
    QT_IM_MODULE=compose "$build/fcitx-self-host-probe" "$host_pid"
else
    "$build/fcitx-delivery-probe"
fi

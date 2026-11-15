#!/usr/bin/env bash
# Installs the packages in clean containers and checks them; runs on a Linux machine with docker or podman.
#
#   packaging/linux/verify.sh DIST_DIR [image ...]      (default: every image below)
#   packaging/linux/verify.sh DIST_DIR --lint           lintian (Debian 13) and rpmlint (Fedora) only
#   packaging/linux/verify.sh DIST_DIR --gui OUT_DIR    the settings window on Xvfb + Mesa lavapipe, screenshot
#
# For each image: the package and its dependencies install non-interactively; the files land where they should; the
# binary and the addon resolve every library; on Debian/Ubuntu also fcitx5-setup adds ZhuyinType to a running Fcitx 5
# (D-Bus session, no display); removal leaves none of the package's files.  Prints PASS/FAIL per image.
set -euo pipefail
dist=$(realpath "${1:?Usage: verify.sh DIST_DIR [image ...|--lint|--gui OUT_DIR]}"); shift
runtime=${ZT_CONTAINER:-$(command -v podman || command -v docker || true)}
[[ -n $runtime ]] || { echo "verify.sh：需要 docker 或 podman" >&2; exit 1; }
deb=$(ls "$dist"/zhuyintype_*_amd64.deb | tail -1)
rpm=$(ls "$dist"/zhuyintype-*.x86_64.rpm | tail -1)
run() { "$runtime" run --rm -v "$dist:/dist:ro" "$@"; }

# Every file of the package (the same list for both formats, addon path aside), checked after install and removal.
read -r -d '' files_check <<'SH' || true
expect() {
    for path in /usr/bin/zhuyintype /usr/lib/zhuyintype/fcitx5-setup "$addon" \
        /usr/share/fcitx5/addon/zhuyintype.conf /usr/share/fcitx5/inputmethod/zhuyintype.conf \
        /usr/share/zhuyintype/data/lexicon.bin /usr/share/zhuyintype/data/channel.bin \
        /usr/share/zhuyintype/data/lm/lm.bin /usr/share/zhuyintype/data/lm_chat/lm.bin \
        /usr/share/zhuyintype/data/manifest.json /usr/share/applications/zhuyintype.desktop \
        /etc/xdg/autostart/zhuyintype.desktop /usr/share/icons/hicolor/48x48/apps/zhuyintype.png \
        /usr/share/licenses/zhuyintype/AGPL-3.0.txt /usr/share/licenses/zhuyintype/Rust-crates.txt; do
        if [ "$1" = present ] && [ ! -e "$path" ]; then echo "FAIL missing $path"; exit 1; fi
    done
}
libraries() {
    if ldd /usr/bin/zhuyintype "$addon" | grep 'not found'; then echo "FAIL unresolved libraries"; exit 1; fi
}
leftovers() {
    left=$(for path in $(cat /tmp/files); do [ -e "$path" ] && [ ! -d "$path" ] && echo "$path"; done; \
        for dir in /usr/share/zhuyintype /usr/lib/zhuyintype /usr/share/licenses/zhuyintype /usr/share/doc/zhuyintype; do \
            [ -e "$dir" ] && echo "$dir"; done; true)
    # A Debian conffile stays after `remove` until `purge`; we purge, so nothing may remain.
    if [ -n "$left" ]; then echo "FAIL left behind:"; echo "$left"; exit 1; fi
}
SH

# fcitx5-setup against a real Fcitx 5 on a private D-Bus session (no display; only the dbus and keyboard addons).
read -r -d '' fcitx_check <<'SH' || true
fcitx_setup() {
    useradd -m tester
    su tester -c 'dbus-run-session -- sh -c "
        fcitx5 -d --disable=all --enable=dbus,keyboard,zhuyintype >/dev/null 2>&1
        for i in \$(seq 50); do busctl --user status org.fcitx.Fcitx5 >/dev/null 2>&1 && break; sleep 0.2; done
        sleep 1
        /usr/lib/zhuyintype/fcitx5-setup status
        /usr/lib/zhuyintype/fcitx5-setup enable
        /usr/lib/zhuyintype/fcitx5-setup status | grep -q profile=added || { echo FAIL profile; exit 1; }
        busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 InputMethodGroupInfo s Default
        busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 Exit
    "' || { echo "FAIL fcitx5-setup"; exit 1; }
}
SH

debian() {
    local image=$1
    echo "== $image"
    run -e DEBIAN_FRONTEND=noninteractive "$image" bash -c "set -e
        $files_check
        $fcitx_check
        addon=/usr/lib/x86_64-linux-gnu/fcitx5/libzhuyintype.so
        apt-get update -qq
        apt-get install -y -qq --no-install-recommends /dist/$(basename "$deb") >/tmp/apt.log 2>&1 \
            || { tail -30 /tmp/apt.log; echo FAIL install; exit 1; }
        dpkg -L zhuyintype > /tmp/files
        echo \"installed: \$(dpkg-query -W -f='\${Version} \${Installed-Size} KiB' zhuyintype)\"
        expect present; libraries
        apt-get install -y -qq --no-install-recommends dbus-daemon systemd >/dev/null 2>&1 || apt-get install -y -qq dbus systemd >/dev/null
        fcitx_setup
        apt-get purge -y -qq zhuyintype >/dev/null
        leftovers
        echo PASS $image"
}

redhat() {
    local image=$1 install remove
    echo "== $image"
    if [[ $image == *suse* ]]; then
        install="zypper --non-interactive --quiet install --allow-unsigned-rpm --no-recommends /dist/$(basename "$rpm")"
        remove="zypper --non-interactive --quiet remove zhuyintype"
    else
        install="dnf install -y -q --setopt=install_weak_deps=False /dist/$(basename "$rpm")"
        remove="dnf remove -y -q zhuyintype"
    fi
    run "$image" bash -c "set -e
        $files_check
        addon=/usr/lib64/fcitx5/libzhuyintype.so
        $install >/tmp/install.log 2>&1 || { tail -30 /tmp/install.log; echo FAIL install; exit 1; }
        rpm -ql zhuyintype > /tmp/files
        echo \"installed: \$(rpm -q --qf '%{VERSION}-%{RELEASE} %{SIZE} bytes' zhuyintype)\"
        expect present; libraries
        $remove >/dev/null
        leftovers
        echo PASS $image"
}

lint() {
    echo "== lintian (debian:13)"
    run debian:13 bash -c "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq lintian >/dev/null \
        && lintian --info --display-info /dist/$(basename "$deb") || true"
    echo "== rpmlint (fedora:latest)"
    run fedora:latest bash -c "dnf install -y -q rpmlint >/dev/null && rpmlint /dist/$(basename "$rpm") || true"
}

gui() {
    local out=$1
    mkdir -p "$out"
    echo "== settings window (debian:13, Xvfb, lavapipe)"
    "$runtime" run --rm -v "$dist:/dist:ro" -v "$(realpath "$out"):/out" -e OWNER="$(id -u):$(id -g)" \
        -e DEBIAN_FRONTEND=noninteractive debian:13 bash -c "set -e
        apt-get update -qq
        apt-get install -y -qq /dist/$(basename "$deb") xvfb xdotool imagemagick mesa-vulkan-drivers >/dev/null 2>&1
        Xvfb :9 -ac -screen 0 1400x1000x24 -nolisten tcp >/dev/null 2>&1 & sleep 1
        export DISPLAY=:9 VK_ICD_FILENAMES=\$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)
        zhuyintype > /tmp/app.log 2>&1 & app=\$!
        for i in \$(seq 50); do win=\$(xdotool search --pid \$app 2>/dev/null | tail -1 || true); [ -n \"\$win\" ] && break; sleep 0.2; done
        sleep 2
        import -window root /out/settings.png && chown \$OWNER /out/settings.png
        echo \"window: \${win:-none}\"; tail -5 /tmp/app.log
        kill \$app 2>/dev/null || true
        [ -n \"\$win\" ] && echo PASS gui || { echo FAIL gui; exit 1; }"
}

case "${1:-}" in
    --lint) lint; exit ;;
    --gui) gui "${2:?output directory}"; exit ;;
esac
images=("$@")
[[ ${#images[@]} -gt 0 ]] || images=(debian:13 ubuntu:26.04 fedora:latest opensuse/tumbleweed)
status=0
for image in "${images[@]}"; do
    case "$image" in
        debian* | ubuntu*) debian "$image" || status=1 ;;
        *) redhat "$image" || status=1 ;;
    esac
done
exit $status

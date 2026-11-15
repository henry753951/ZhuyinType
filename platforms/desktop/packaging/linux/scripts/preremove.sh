#!/bin/sh
# Before the package is removed (not upgraded): stop the hosts running the installed binary, for every user, so
# nothing of ZhuyinType keeps running after removal.  Debian passes "remove", RPM the number of versions left ("0").
# Processes of other binaries with the same name (a development build) are left alone.
set -e
case "$1" in
    remove | 0) ;;
    *) exit 0 ;;
esac
for pid in $(pgrep -x zhuyintype 2>/dev/null || true); do
    case "$(readlink "/proc/$pid/exe" 2>/dev/null || true)" in
        /usr/bin/zhuyintype | "/usr/bin/zhuyintype (deleted)") kill -TERM "$pid" 2>/dev/null || true ;;
    esac
done
exit 0

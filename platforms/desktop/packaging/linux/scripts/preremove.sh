#!/bin/sh
# Before the package is removed (not upgraded): stop the hosts running the installed binary, for every user, so
# nothing of ZhuyinType keeps running after removal.  Debian passes "remove", RPM the number of versions left ("0").
# Only processes of this binary are signalled (not a development build of the same name); no tool beyond /proc.
set -e
case "$1" in
    remove | 0) ;;
    *) exit 0 ;;
esac
binary=/usr/bin/zhuyintype
for exe in /proc/[0-9]*/exe; do
    case "$(readlink "$exe" 2>/dev/null || true)" in
        "$binary" | "$binary (deleted)")
            pid=${exe#/proc/}
            kill -TERM "${pid%/exe}" 2>/dev/null || true
            ;;
    esac
done
exit 0

#!/bin/sh
# After the package is removed (not upgraded): the updater's download folder (zt_core::update), which the package
# manager does not know about.  Debian passes "remove" or "purge", RPM the number of versions left ("0").
set -e
case "$1" in
    remove | purge | 0) rm -rf /var/cache/zhuyintype ;;
esac
exit 0

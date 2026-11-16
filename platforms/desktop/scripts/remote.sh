#!/usr/bin/env bash
# Run a command for platforms/desktop on the GPU host, from the Mac: sync this checkout's sources, then run the
# command in platforms/desktop there.  Nothing is built on the Mac.
#
#   platforms/desktop/scripts/remote.sh cargo build
#   platforms/desktop/scripts/remote.sh scripts/build.sh windows
#   platforms/desktop/scripts/remote.sh scripts/screenshot.sh ~/zhuyintype/shots/NAME
#   platforms/desktop/scripts/remote.sh --fetch ~/zhuyintype/shots/NAME LOCAL_DIR      # copy results back
#
# The host is ZHUYIN_REMOTE_HOST in .remote.env (an ssh alias or user@host).  Each checkout gets its own work
# directory (~/zhuyintype/work/desktop-<checkout>) and cargo target directory (~/zhuyintype/target/desktop-<checkout>),
# so worktrees never rebuild each other's artefacts.
set -euo pipefail
# The checkout's root from this file's place (not git: a git hook's GIT_DIR would make that answer the current
# directory).
root=$(cd "$(dirname "$0")/../../.." && pwd)
host=${ZHUYIN_REMOTE_HOST:-$(sed -n 's/^ZHUYIN_REMOTE_HOST=//p' "$root/.remote.env" 2>/dev/null | tail -1)}
[ -n "$host" ] || { echo "remote.sh: set ZHUYIN_REMOTE_HOST in $root/.remote.env" >&2; exit 2; }
name=desktop-$(basename "$root" | tr -c 'A-Za-z0-9._-\n' '-')
ssh_opts=(-o BatchMode=yes -o ControlMaster=auto -o "ControlPath=/tmp/zt-remote-%C" -o ControlPersist=900)
if [ "${1:-}" = "--fetch" ]; then
  src=${2:?remote path}; dest=${3:?local directory}; mkdir -p "$dest"
  exec rsync -az -e "ssh ${ssh_opts[*]}" "$host:$src/" "$dest/"
fi
rsync -az --delete -e "ssh ${ssh_opts[*]}" \
  --exclude .git --exclude .reference --exclude target/ --exclude /data --exclude /dist --exclude .venv \
  --exclude /references --exclude '.build*' --exclude .remote.env --exclude 'platforms/macos/.build' \
  "$root/" "$host:zhuyintype/work/$name/"
exec ssh "${ssh_opts[@]}" "$host" "export PATH=\$HOME/.cargo/bin:\$PATH CARGO_TARGET_DIR=\$HOME/zhuyintype/target/$name ZT_WORK=\$HOME/zhuyintype/work/$name; cd \$ZT_WORK/platforms/desktop && $(printf '%q ' "$@")"

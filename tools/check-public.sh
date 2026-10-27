#!/bin/bash
# Fails when files about to be published contain private details: the host and key named in .remote.env, or a
# private-network address.  Checks tracked files and untracked files that are not ignored (what `git add -A` would
# publish).  Run before pushing (`tools/zt check public`); CI runs it too (without .remote.env it checks addresses
# only).
set -euo pipefail
cd "$(dirname "$0")/.."
# Private-network addresses (not version numbers such as ==10.4.0.35 or -10.3.7.77 in lock files).
patterns=('(^|[^0-9.=a-zA-Z_-])(10\.[0-9]{1,3}|192\.168|172\.(1[6-9]|2[0-9]|3[01]))\.[0-9]{1,3}\.[0-9]{1,3}([^0-9.]|$)')
if [[ -f .remote.env ]]; then
    while IFS='=' read -r key value; do
        [[ "$key" =~ ^ZHUYIN_REMOTE_(HOST|KEY)$ && -n "$value" ]] || continue
        value=${value/\$HOME/}
        for part in ${value//[@\/]/ }; do
            [[ ${#part} -ge 6 && "$part" != .ssh ]] && patterns+=("$(printf '%s' "$part" | sed 's/[.[\*^$]/\\&/g')")
        done
    done < .remote.env
fi
found=""
for pattern in "${patterns[@]}"; do
    found+=$(git ls-files -z --cached --others --exclude-standard | grep -zv -e '^references/' -e '\.lock$' \
        | xargs -0 grep -nIsE "$pattern" -- 2>/dev/null || true)
done
if [[ -n "$found" ]]; then
    printf '%s\n' "$found"
    echo 'Private details found (see above); remove them before publishing.' >&2
    exit 1
fi
echo 'No private details found.'

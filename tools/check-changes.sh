#!/usr/bin/env bash
# The format and lint checks CI runs, for what a commit or a push changes, so they fail here and not on CI.
# The git hooks in tools/git-hooks call it (`tools/zt setup` turns them on); it also runs by hand:
#
#   tools/check-changes.sh --staged          # pre-commit: formatting and lint of the staged files (seconds, no build)
#   tools/check-changes.sh --push BASE [TIP] # pre-push: also clippy, types and tests for what BASE..TIP changes
#   tools/check-changes.sh --all             # every check, as if everything changed
#
# What runs, by the files that changed:
#   any               tools/check-public.sh (no private hosts, keys or paths in tracked files)
#   engine/**.rs      cargo fmt --check; push: cargo clippy -D warnings (with and without the onnx feature), cargo doc
#   platforms/desktop cargo fmt --check; push: clippy -D warnings for Linux and Windows (x64, x86)
#   Rust lint compiles, so on a push it runs on the GPU host when .remote.env names one (platforms/desktop/scripts/
#   remote.sh), otherwise here; the desktop app is never compiled on a Mac.
#   *.py, pyproject   ruff check and ruff format --check; push: mypy and the Python tests (zt check all)
#   *.swift           swift-format lint with platforms/macos/.swift-format (rules only, see that file)
#   platforms/linux   clang-format --dry-run when a .clang-format exists
#
# Skipping is deliberate only: `git commit --no-verify` / `git push --no-verify`.
set -uo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
mode=${1:---staged}
failed=()

# Rust lint compiles: on the GPU host when .remote.env names one (the Mac builds nothing for this), else here.
remote() { [[ -n ${ZHUYIN_REMOTE_HOST:-} ]] || grep -q '^ZHUYIN_REMOTE_HOST=.' "$root/.remote.env" 2>/dev/null; }
in_rust() {
  local dir=$1 script=$2
  if remote; then
    platforms/desktop/scripts/remote.sh bash -c "set -e; cd \$ZT_WORK/$dir; $script"
  else
    (cd "$dir" && bash -c "set -e; $script")
  fi
}
step() { printf '\033[1m==> %s\033[0m\n' "$*"; }
fail() { failed+=("$1"); printf '\033[31m✗ %s\033[0m\n' "$1"; }
run() { local name=$1; shift; "$@" || fail "$name"; }

case $mode in
  --staged) changed=$(git diff --cached --name-only --diff-filter=ACMR) heavy=0 ;;
  --push) changed=$(git diff --name-only --diff-filter=ACMR "${2:?BASE}" "${3:-HEAD}") heavy=1 ;;
  --all) changed=$(git ls-files) heavy=1 ;;
  *) echo "usage: tools/check-changes.sh --staged | --push BASE [TIP] | --all" >&2; exit 2 ;;
esac
[[ -n $changed ]] || exit 0
has() { grep -qE "$1" <<< "$changed"; }

step "隱私檢查（tools/check-public.sh）"
run "privacy" tools/check-public.sh >/dev/null

if has '^engine/.*\.(rs|toml)$|^engine/Cargo\.lock$'; then
  step "引擎：cargo fmt"
  run "engine fmt (cd engine && cargo fmt --all)" bash -c 'cd engine && cargo fmt --all --check'
  if ((heavy)); then
    step "引擎：cargo clippy、cargo doc"
    run "engine clippy" in_rust engine '
      cargo clippy -q --workspace --all-targets --locked -- -D warnings
      cargo clippy -q --workspace --all-targets --locked --features zhuyin-cli/onnx -- -D warnings
      RUSTDOCFLAGS="-D warnings" cargo doc -q --workspace --no-deps --locked'
  fi
fi

if has '^platforms/desktop/.*\.(rs|toml)$|^platforms/desktop/Cargo\.lock$|^engine/crates/(zhuyin-core|zhuyin-models)/'; then
  if has '^platforms/desktop/'; then
    step "桌面版：cargo fmt"
    run "desktop fmt (cd platforms/desktop && cargo fmt --all)" bash -c 'cd platforms/desktop && cargo fmt --all --check'
  fi
  if ((heavy)); then
    step "桌面版：clippy（Linux、Windows x64／x86）"
    if remote || [[ $(uname) == Linux ]]; then
      run "desktop clippy" in_rust platforms/desktop '
        export XWIN_ARCH=x86,x86_64
        cargo clippy -q --workspace --all-targets --locked -- -D warnings
        cargo xwin clippy -q -p zt-core -p zhuyintype -p zt-tsf --locked --target x86_64-pc-windows-msvc -- -D warnings
        cargo xwin clippy -q -p zt-tsf --locked --target i686-pc-windows-msvc -- -D warnings'
    else
      fail "desktop clippy (no GPU host in .remote.env; the desktop app is not built on a Mac)"
    fi
  fi
fi

if has '\.py$|^ml/pyproject\.toml$|^ml/uv\.lock$'; then
  step "Python：ruff check、ruff format --check"
  run "ruff (tools/zt check lint --fix; ruff format)" tools/zt check lint --format
  if ((heavy)); then
    step "Python：mypy 與測試"
    run "mypy" tools/zt check types
    run "python tests" tools/zt check python
  fi
fi

swift=$(grep -E '^platforms/macos/.*\.swift$' <<< "$changed" || true)
if [[ -n $swift ]]; then
  step "Swift：swift-format lint"
  if xcrun --find swift-format >/dev/null 2>&1; then
    # Only the rule findings: the pretty-printer's (indentation, line length, blank lines, spacing, trailing commas)
    # cannot be turned off one by one, and the existing code is not formatted by swift-format yet.
    files=()
    while IFS= read -r file; do [[ -f $file ]] && files+=("$file"); done <<< "$swift"
    if ((${#files[@]})); then
      out=$(xcrun swift-format lint --configuration platforms/macos/.swift-format "${files[@]}" 2>&1 \
        | grep -E 'warning: \[' | grep -vE '\[(Indentation|LineLength|AddLines|RemoveLine|Spacing|TrailingComma)\]' || true)
      if [[ -n $out ]]; then printf '%s\n' "$out"; fail "swift-format lint"; fi
    fi
  else
    echo "（沒有 swift-format：不是 macOS 或沒有 Xcode，略過）"
  fi
fi

cpp=$(grep -E '^platforms/linux/.*\.(cpp|h)$' <<< "$changed" || true)
if [[ -n $cpp && -f platforms/linux/.clang-format ]] && command -v clang-format >/dev/null; then
  step "C++：clang-format"
  files=()
  while IFS= read -r file; do [[ -f $file ]] && files+=("$file"); done <<< "$cpp"
  ((${#files[@]})) && run "clang-format" clang-format --dry-run -Werror "${files[@]}"
fi

if ((${#failed[@]})); then
  printf '\n\033[31m沒有通過：%s\033[0m\n' "${failed[*]}"
  echo "修正後再試；確定要略過時用 --no-verify。"
  exit 1
fi
printf '\033[32m✓ 檢查通過\033[0m\n'

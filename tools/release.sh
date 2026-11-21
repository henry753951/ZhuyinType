#!/usr/bin/env bash
# A release with one command, built and checked once:
#
#   tools/release.sh X.Y.Z             # version, checks, pull request, its CI; stops there for a go
#   tools/release.sh X.Y.Z --publish   # the same (whatever is already done is skipped), then merge, main's CI, the
#                                      # tag, release.yml (publishing and verifying the release) and its result
#
# Run it on the branch to release (not main), with docs/releases/vX.Y.Z.md committed (docs/product/WRITING.md
# 「發布說明與對外文字」).  Each step finds what is already done and goes on from there, so after a failure: fix it, or
# rerun the failed CI jobs, and run the same command again.
#
# 1. The version: Info.plist's CFBundleShortVersionString (and CFBundleVersion + 1), the desktop workspace and its
#    Cargo.lock, in one commit.
# 2. The checks release.yml's gate makes (versions, notes, privacy), and that the version is newer than the latest.
# 3. Push (the pre-push hook runs tools/check-changes.sh), the pull request, and its CI to the end.
# 4. With --publish: a merge commit, main's CI for it to the end (tools/ci-run.sh), the annotated tag vX.Y.Z on it, and
#    release.yml's run for the tag.  That run takes the installers and the app main's CI built (no second build),
#    signs and publishes them, updates the feeds, then downloads the release and checks every file and signature
#    (tools/verify-release.py).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
version=${1:?usage: tools/release.sh X.Y.Z [--publish]}
publish=0
[[ ${2:-} == --publish ]] && publish=1
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "release.sh: X.Y.Z, not $version" >&2; exit 2; }
tag=v$version
repo=${GH_REPO:-henry753951/ZhuyinType}
export GH_REPO=$repo
plist=platforms/macos/Resources/Info.plist
notes=docs/releases/$tag.md
step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31mrelease.sh: %s\033[0m\n' "$*" >&2; exit 1; }
desktop_version() { sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' platforms/desktop/Cargo.toml; }
newer() { [[ $1 != "$2" && $(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1) == "$1" ]]; }

gh auth status >/dev/null 2>&1 || die "gh is not logged in (gh auth login)"
branch=$(git branch --show-current)
git fetch -q origin main --tags

step "Version $version"
if git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1; then
  sha=$(git rev-parse "$tag^{commit}")
  echo "$tag is already tagged ($sha); going on with its release."
else
  [[ -n $branch && $branch != main ]] || die "run it on the branch to release, not ${branch:-a detached HEAD}"
  git diff --quiet && git diff --cached --quiet || die "commit or stash the changes first"
  [[ -f $notes ]] && git ls-files --error-unmatch "$notes" >/dev/null 2>&1 || die "commit $notes first"
  latest=$(gh release view --json tagName --jq .tagName)
  newer "$version" "${latest#v}" || die "$version is not newer than the latest release, $latest"
  current=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$plist")
  if [[ $current != "$version" || $(desktop_version) != "$version" ]]; then
    build=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$plist")
    python3 -I - "$plist" platforms/desktop/Cargo.toml platforms/desktop/Cargo.lock "$version" "$(desktop_version)" <<'PY'
import re, sys
from pathlib import Path
plist, toml, lock, new, old = sys.argv[1:]
text = Path(plist).read_text()
text = re.sub(r"(<key>CFBundleShortVersionString</key>\s*<string>)[^<]*", rf"\g<1>{new}", text)
text = re.sub(r"(<key>CFBundleVersion</key>\s*<string>)(\d+)", lambda m: f"{m.group(1)}{int(m.group(2)) + 1}", text)
Path(plist).write_text(text)
text = Path(toml).read_text()
text = re.sub(r'(\[workspace\.package\][^\[]*?\nversion = ")[^"]*', rf"\g<1>{new}", text, count=1)
Path(toml).write_text(text)
# The workspace's own packages: no `source` line, and the old workspace version.
blocks = Path(lock).read_text().split("\n[[package]]\n")
for i, block in enumerate(blocks):
    if "\nsource = " not in block and f'\nversion = "{old}"\n' in block:
        blocks[i] = block.replace(f'\nversion = "{old}"\n', f'\nversion = "{new}"\n', 1)
Path(lock).write_text("\n[[package]]\n".join(blocks))
PY
    build=$((build + 1))
    git add "$plist" platforms/desktop/Cargo.toml platforms/desktop/Cargo.lock
    git commit -q -m "$version: version (Info.plist build $build, the desktop workspace and Cargo.lock)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    echo "Committed $version (build $build)."
  else
    echo "Info.plist and the desktop workspace already say $version."
  fi

  step "The release gate's checks"
  [[ $(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$plist") == "$version" ]] || die "Info.plist is not $version"
  [[ $(desktop_version) == "$version" ]] || die "platforms/desktop/Cargo.toml is not $version"
  tools/check-public.sh

  step "Pull request"
  git push -q -u origin HEAD
  pr=$(gh pr list --head "$branch" --state all --json number,state --jq 'map(select(.state != "CLOSED")) | .[0].number // empty')
  if [[ -z $pr ]]; then
    gh pr create --base main --head "$branch" --title "ZhuyinType $version" --body "ZhuyinType $version：發布說明見 [\`$notes\`]($notes)。

合併後由 \`tools/release.sh $version --publish\` 打標籤；Release 會使用 main 上這個合併 commit 的 CI 建置結果。

🤖 Generated with [Claude Code](https://claude.com/claude-code)" >/dev/null
    pr=$(gh pr view "$branch" --json number --jq .number)
  fi
  url=$(gh pr view "$pr" --json url --jq .url)
  echo "$url"

  if [[ $(gh pr view "$pr" --json state --jq .state) != MERGED ]]; then
    step "The pull request's CI"
    head=$(git rev-parse HEAD)
    for _ in $(seq 30); do
      [[ -n $(gh pr checks "$pr" --json name --jq '.[].name' 2>/dev/null) ]] && break
      sleep 10
    done
    [[ $(gh pr view "$pr" --json headRefOid --jq .headRefOid) == "$head" ]] || die "the pull request's head is not $head"
    gh pr checks "$pr" --watch --interval 30 || die "the pull request's CI failed ($url): fix it, or rerun the failed jobs, then run this again"
    if ((!publish)); then
      printf '\n\033[32mReady: %s passed its CI.\033[0m  Publish with: tools/release.sh %s --publish\n' "$url" "$version"
      exit 0
    fi
    step "Merge"
    gh pr merge "$pr" --merge --subject "Merge $branch: ZhuyinType $version" --body "" >/dev/null
  fi
  sha=$(gh pr view "$pr" --json mergeCommit --jq .mergeCommit.oid)
  [[ -n $sha ]] || die "no merge commit for $url"
  git fetch -q origin main

  step "main's CI for $sha"
  # Every run a push starts exists once ci.yml's (which every push starts) has finished; a workflow whose path
  # filters skipped the commit has none.
  tools/ci-run.sh ci.yml "$sha" >/dev/null || die "main's CI failed for $sha: rerun its failed jobs, then run this again"
  for workflow in engine.yml ml.yml desktop.yml macos.yml; do
    ZT_CI_APPEAR=0 tools/ci-run.sh "$workflow" "$sha" >/dev/null \
      || die "main's CI failed for $sha: rerun its failed jobs, then run this again"
  done

  step "Tag $tag"
  git tag -a "$tag" -m "ZhuyinType $version" "$sha"
  git push -q origin "$tag"
fi

((publish)) || { echo "$tag exists; run tools/release.sh $version --publish to follow its release."; exit 0; }
step "release.yml for $tag"
run=$(ZT_CI_BRANCH=$tag ZT_CI_APPEAR=300 tools/ci-run.sh release.yml "$sha") || die "the release failed: see the run above"
[[ -n $run ]] || die "no release.yml run for $tag"
# The run succeeds with a skipped job too: the release is verified only when the verify job passed.
verified=$(gh run view "$run" --json jobs --jq '[.jobs[] | select(.name | test("Verify what users download")) | .conclusion] | first // "missing"')
[[ $verified == success ]] || die "the release is published, but its check of the downloads is $verified: gh workflow run verify-release.yml -f tag=$tag"
printf '\n\033[32mZhuyinType %s is published and verified: https://github.com/%s/releases/tag/%s\033[0m\n' "$version" "$repo" "$tag"

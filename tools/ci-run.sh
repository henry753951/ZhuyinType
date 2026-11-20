#!/usr/bin/env bash
# The run that main's push of COMMIT started for WORKFLOW (a file in .github/workflows), once it has finished, so
# release.yml and tools/release.sh reuse what it built and checked instead of building it again:
#
#   tools/ci-run.sh WORKFLOW COMMIT [ARTIFACT ...]
#
# - Prints the run's id when it succeeded and still holds every ARTIFACT.
# - Prints nothing when there is no such run (the workflow's path filters skipped the commit) or its artifacts expired;
#   the caller then builds that platform itself.
# - Waits while the run is queued or running (ZT_CI_WAIT seconds, default 75 minutes), and up to ZT_CI_APPEAR seconds
#   (default 120) for a run to appear, since a push starts its runs a few seconds late.
# - Fails when the run failed or was cancelled, with its address: rerun its failed jobs, then the caller.
#
# ZT_CI_BRANCH (default main) is the branch or tag the push was to: release.yml's run for a tag is found with
# ZT_CI_BRANCH=vX.Y.Z.  Needs gh, logged in or with GH_TOKEN.  GH_REPO (or GITHUB_REPOSITORY on CI) names the
# repository.
set -euo pipefail
workflow=${1:?usage: tools/ci-run.sh WORKFLOW COMMIT [ARTIFACT ...]}
commit=${2:?usage: tools/ci-run.sh WORKFLOW COMMIT [ARTIFACT ...]}
shift 2
repo=${GH_REPO:-${GITHUB_REPOSITORY:-henry753951/ZhuyinType}}
wait_for=${ZT_CI_WAIT:-4500}
appear=${ZT_CI_APPEAR:-120}
branch=${ZT_CI_BRANCH:-main}
say() { printf '%s\n' "$*" >&2; }

start=$SECONDS last=
while :; do
  run=$(gh api "repos/$repo/actions/workflows/$workflow/runs?head_sha=$commit&event=push&branch=$branch&per_page=1" \
    --jq '.workflow_runs[0] // {} | "\(.id // "-") \(.status // "-") \(.conclusion // "-") \(.html_url // "-")"')
  read -r id status conclusion url <<< "$run"
  elapsed=$((SECONDS - start))
  if [[ $id == - ]]; then
    if ((elapsed >= appear)); then
      say "$workflow: no push run on $branch for ${commit:0:8}."
      exit 0
    fi
  elif [[ $status == completed ]]; then
    case $conclusion in
      success) ;;
      skipped) say "$workflow: run $id for ${commit:0:8} was skipped."; exit 0 ;;
      *) say "$workflow: run $id for ${commit:0:8} ended $conclusion: $url"
         say "Rerun its failed jobs (gh run rerun $id --failed), then this again."
         exit 1 ;;
    esac
    present=$(gh api --paginate "repos/$repo/actions/runs/$id/artifacts?per_page=100" \
      --jq '.artifacts[] | select(.expired | not) | .name')
    for artifact in "$@"; do
      if ! grep -Fxq -- "$artifact" <<< "$present"; then
        say "$workflow: run $id has no artifact $artifact (expired or never uploaded)."
        exit 0
      fi
    done
    say "$workflow: reusing run $id ($url)"
    printf '%s\n' "$id"
    exit 0
  elif ((elapsed >= wait_for)); then
    say "$workflow: run $id for ${commit:0:8} is still $status after $((wait_for / 60)) minutes: $url"
    exit 1
  elif [[ $last != "$id $status" ]]; then
    say "$workflow: waiting for run $id ($status): $url"
    last="$id $status"
  fi
  sleep 20
done

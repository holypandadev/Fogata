#!/usr/bin/env bash
# The checks tools/ci.sh asks of macOS (or any other Unix computer), run in a clone on that
# computer, at the commit being checked:
#
#     FOGATA_CI_SHA=<commit> tools/ci-leg.sh macos
#
# FOGATA_CI_MAC, on the computer that runs tools/ci.sh, is the command that gets the commit
# there (git bundle, a push to the clone), checks it out, and runs this over a remote shell,
# whose exit status and output come back, for instance:
#
#     FOGATA_CI_MAC='ssh mac "cd ~/fogata && git fetch -q origin && git checkout -q --detach $FOGATA_CI_SHA && tools/ci-leg.sh macos"'
#
# It refuses a clone not at that commit, or with any change to a tracked file or any new file
# git does not ignore. Then it runs
# tools/check.sh (formatting, clippy with warnings as errors, every test, cargo deny, the
# tests of the scripts, THIRD-PARTY-NOTICES) without the RustSec advisories and the bridge
# code, which tools/ci.sh checks on the computer that runs it, then `flutter analyze` and
# `flutter test`. Only when every step passed it prints the line tools/ci.sh looks for:
#
#     fogata-ci-leg: system=macos commit=<commit> status=pass steps=<steps> skipped=<steps>
#
# FOGATA_CI_LEG_SKIP names steps (check, analyze, test) to leave out, for what cannot run on
# that computer; they are named in the line, and tools/ci.sh then records the leg as incomplete.
set -euo pipefail
# The copies the checks start never read the releases page on GitHub.
export FOGATA_UPDATE_WEB=off
system=${1:?usage: FOGATA_CI_SHA=<commit> tools/ci-leg.sh <system>}
sha=${FOGATA_CI_SHA:?FOGATA_CI_SHA names the commit to check}
cd "$(dirname "$0")/.."

head=$(git rev-parse --verify 'HEAD^{commit}')
if [[ $head != "$sha" ]]; then
    echo "ci-leg: this clone is at $head, not at $sha" >&2
    exit 1
fi
# Every tracked file (deny.toml, THIRD-PARTY-NOTICES, docs and rust-toolchain.toml too) and any
# new file git does not ignore; a `git status` that fails is a failure, not an empty list.
changes=$(git status --porcelain --untracked-files=normal) ||
    { echo "ci-leg: cannot tell what this clone holds (git status failed)" >&2; exit 1; }
if [[ -n $changes ]]; then
    echo "ci-leg: the clone has changes:" >&2
    echo "$changes" >&2
    exit 1
fi

skip=${FOGATA_CI_LEG_SKIP:-}
skip=" ${skip//,/ } "
steps=()
skipped=()
# Runs step $1 as the rest of the arguments, unless it is named in FOGATA_CI_LEG_SKIP.
step() {
    local name=$1
    shift
    if [[ $skip == *" $name "* ]]; then
        echo "ci-leg: $name SKIPPED"
        skipped+=("$name")
        return
    fi
    echo "ci-leg: $name"
    "$@"
    steps+=("$name")
}
join() { local IFS=,; echo "${*:-none}"; }

step check tools/check.sh --no-advisories --no-codegen
step analyze bash -c 'cd app && flutter pub get --offline && flutter analyze'
step test bash -c 'cd app && flutter test'
echo "fogata-ci-leg: system=$system commit=$sha status=pass steps=$(join ${steps[@]+"${steps[@]}"}) skipped=$(join ${skipped[@]+"${skipped[@]}"})"

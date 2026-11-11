#!/usr/bin/env bash
# The test of this commit against the last release, run here, as the weekly workflow's job
# previous-release runs it on GitHub (docs/releasing-beta.md, step 1):
#
#     tools/previous-release.sh [<tag>]
#
# Builds the command line of <tag> (by default the last release tag before the commit
# checked out: on a release's own commit, the one before it) in a worktree and a target
# folder of its own, then runs crates/fogata-core/tests/previous_release.rs of this checkout
# against it: what this version writes to a group, the last release must take in. It writes
# the record tools/beta/release.py looks for, <git dir>/fogata-ci/<commit>.previous-release,
# saying `result: pass` only when the test ran and passed, and only for a checkout with no
# changes to the files git keeps (the test is of the commit, not of changes on top of it).
#
# FOGATA_PREVIOUS_TARGET is the old command line's target folder (by default
# ~/fogata-previous-target, kept between runs); CARGO_BUILD_JOBS is passed on.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
sha=$(git rev-parse --verify 'HEAD^{commit}')
changes=$(git status --porcelain --untracked-files=no) ||
    { echo "previous-release: git status failed" >&2; exit 1; }
if [[ -n $changes ]]; then
    echo "previous-release: the checkout has changes; commit them first:" >&2
    echo "$changes" >&2
    exit 1
fi
tag=${1:-$(git describe --tags --abbrev=0 --match 'v*' "$sha^")}
tagged=$(git rev-parse --verify "refs/tags/$tag^{commit}")
records="$(cd "$(git rev-parse --git-common-dir)" && pwd -P)/fogata-ci"
mkdir -p "$records"
record="$records/$sha.previous-release"
target=${FOGATA_PREVIOUS_TARGET:-$HOME/fogata-previous-target}
# Outside the checkout, so the old crates never take this checkout's Cargo.toml for theirs.
work=$(mktemp -d "${TMPDIR:-/tmp}/fogata-previous-XXXXXX")
cleanup() {
    git worktree remove --force "$work/old" > /dev/null 2>&1 || true
    git worktree prune
    rm -rf "$work"
}
trap cleanup EXIT

write_record() {
    {
        echo "commit: $sha"
        echo "tag: $tag ($tagged)"
        echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "result: $1"
    } > "$record"
}
# A run that stops anywhere leaves a failure, never an older pass.
write_record fail

echo "previous-release: $sha against $tag ($tagged)"
git worktree add --detach "$work/old" "$tagged" > /dev/null
(cd "$work/old" && CARGO_TARGET_DIR=$target cargo build --release --locked -p fogata)
FOGATA_OLD_CLI=$target/release/fogata \
    cargo test --locked -p fogata-core --test previous_release -- --ignored 2>&1 |
    tee "$work/test.log"
# Some test must have run: a test renamed or gone would pass with none.
if ! grep -Eq '^test result: ok\. [1-9][0-9]* passed' "$work/test.log"; then
    echo "previous-release: no test ran" >&2
    exit 1
fi
# The test of a commit with changes made while it ran is not that commit's.
if [[ -n $(git status --porcelain --untracked-files=no) ||
    $(git rev-parse --verify 'HEAD^{commit}') != "$sha" ]]; then
    echo "previous-release: the checkout changed while the test ran; no pass recorded" >&2
    exit 1
fi
write_record pass
echo "previous-release: passed; record $record"

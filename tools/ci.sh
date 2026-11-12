#!/usr/bin/env bash
# The checks a commit must pass before it is pushed, run on this computer:
#
#     tools/ci.sh [--full] [--no-advisories] [--no-codegen] [--no-notices] [--split app|checks]
#                 [<commit>]   (by default HEAD)
#
# In a clean temporary worktree of that commit: a release build of the Linux application,
# checked as tools/beta/release.py checks a bundle before it packs it (tools/beta/ci_builds.py
# check-build: a release build, every program and library with its hardening, the WebRTC
# plugin linking PipeWire and PulseAudio), the native tests of the WebRTC plugin's shared C++ code (the video renderer's built with
# ThreadSanitizer against the libwebrtc that build unpacks, and the noise suppression
# table's), tools/check.sh (strict: formatting, clippy, every test, cargo deny with the
# RustSec advisories, the release scripts' tests, THIRD-PARTY-NOTICES, the bridge code), the
# libFuzzer targets built (cargo +nightly fuzz build; they are not run), the localizations
# generated again (flutter gen-l10n) and nothing in the worktree changed by the
# generators, the Dart code formatted (dart format), flutter analyze, flutter test, and the integration tests that need one copy of
# the application and nothing else (tools/integration-tests.sh: each on an X display and a
# null sound sink of its own; skipped when Xvfb or a sound server is missing). With --full,
# also the tests of two copies talking to each other (call_test, direct_test, voice_test,
# move_test: tools/integration-pairs.sh), which take much longer: left out otherwise and
# recorded as "not run", which leaves the result "incomplete" (the pre-push hook lets it
# through only when FOGATA_CI_ALLOW_SKIPPED names `pairs`), never as passed. With --full, also
# the smoke test (tools/smoke-linux.sh: two copies through the main things a person does, the
# screen shared with the room's button, on a KDE Wayland session of its own with the desktop
# portal, on X11 through its Xwayland, on the same session started bare, and on Xvfb):
# optional, in that a desktop this computer cannot build (no GPU render node, no unprivileged
# namespaces, a program missing) is skipped with the reason in the record and the log, and
# when none can be built the smoke test is recorded as skipped without making the result
# incomplete; a desktop that fails, or a copy that crashes, fails the run. Then the legs of
# the other systems: FOGATA_CI_MAC and FOGATA_CI_WINDOWS, when set, are commands run with
# FOGATA_CI_SHA (the full commit) in their environment, which build and test that commit on those systems however this computer
# reaches them (tools/ci-leg.sh and tools/ci-leg.ps1 are what runs there). A leg passes only
# when its command exits 0 and its output says, in a line fogata-ci-leg: system=<name>
# commit=<commit> status=pass ... skipped=none, that it checked this commit's steps; a command
# that exits 0 without saying so (a bare `true`) leaves the leg "incomplete". A leg not set is
# recorded as skipped.
# Each --no-... skips that check of tools/check.sh and is recorded as skipped too.
# --split runs one half of the Linux steps, for two computers that run the halves side by side
# (GitHub's CI, .github/workflows/ci.yml): `checks` is tools/check.sh and the fuzz targets'
# build; `app` is everything else (the release build and its checks, the native tests, the
# localizations, dart format, flutter analyze and test, the integration, pair and smoke
# tests). Both halves fetch the Dart packages first and end with the check that nothing in
# the worktree was changed by a generator. What the other half runs is recorded as
# `elsewhere (--split ...)`, and a split record never says `result: pass` (at best
# `incomplete`): the pre-push hook refuses it, and each half's own lines say what it did.
#
# It writes the record of the run to <git dir>/fogata-ci/<commit> and its log beside it
# (<commit>.log): each leg and check (the native, integration and pair tests on their own
# lines, and `legs ran` / `legs not run`), for each leg of another system the command that
# ran, its exit status and the SHA-256 of its output, the tools that ran (rustc, cargo, flutter, the date of the RustSec database, the SHA-256
# of this script and of check.sh) and "result: pass" only when every leg ran and passed and
# nothing was skipped or left incomplete ("result: incomplete" when something was skipped and
# nothing failed). The record and the log are files the user can write: this keeps an honest
# run from being forgotten, it does not stop anyone who means to get past it.
# tools/hooks/pre-push refuses to push a commit without a passing record unless each part
# skipped is named in FOGATA_CI_ALLOW_SKIPPED (tools/install-hooks.sh installs it).
#
# The steps run without the variables that change what the compilers make (RUSTFLAGS,
# CARGO_ENCODED_RUSTFLAGS, RUSTC_WRAPPER, CFLAGS, CXXFLAGS, LDFLAGS, CMAKE): the record names
# those that were set and cleared.
#
# Environment: FOGATA_CI_SLOT, a command the heavy steps run under (one that waits for a
# free build slot, say), and CARGO_BUILD_JOBS, passed on. Build outputs are kept between runs
# in <git dir>/fogata-ci/target and the libwebrtc download in <git dir>/fogata-ci/downloads;
# FOGATA_CI_FRESH=1 builds without them. Cargo never forgets an old build, so a folder of
# them past FOGATA_CI_CACHE_GB (40 by default) is emptied before the run, which then builds
# afresh.
set -euo pipefail
# The copies the checks start take releases from one another only, never from the
# releases page on GitHub.
export FOGATA_UPDATE_WEB=off

usage="usage: tools/ci.sh [--full] [--no-advisories] [--no-codegen] [--no-notices] [--split app|checks] [<commit>]"
skips=()
full=0
split=none
commit=HEAD
while (( $# )); do
    case "$1" in
        --no-advisories | --no-codegen | --no-notices) skips+=("$1") ;;
        --full) full=1 ;;
        --split)
            [[ ${2:-} == app || ${2:-} == checks ]] || { echo "$usage" >&2; exit 2; }
            split=$2
            shift
            ;;
        -*) echo "$usage" >&2; exit 2 ;;
        *) commit=$1 ;;
    esac
    shift
done
# Whether this run does the half $1 (app or checks) of the Linux steps: both without --split.
half() { [[ $split == none || $split == "$1" ]]; }
# What the record says of a part the other half runs.
elsewhere="elsewhere (--split $split)"
script=$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0")
# Whether check $1 (advisories, codegen, notices) runs.
checked() {
    local skip
    for skip in ${skips[@]+"${skips[@]}"}; do
        [[ $skip == "--no-$1" ]] && return 1
    done
    return 0
}

# What changes what the compilers make is cleared for every step, and named in the record.
cleared=()
for name in RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER CFLAGS CXXFLAGS LDFLAGS CMAKE; do
    if [[ -n ${!name+set} ]]; then
        cleared+=("$name")
        unset "$name"
    fi
done

cd "$(git rev-parse --show-toplevel)"
sha=$(git rev-parse --verify "$commit^{commit}")
common=$(cd "$(git rev-parse --git-common-dir)" && pwd -P)
records="$common/fogata-ci"
mkdir -p "$records"
record="$records/$sha"
log="$records/$sha.log"
# On disk, and outside the checkout: a build does not fit a /tmp kept in memory, and a crate
# the workspace leaves out (crates/fogata-state/before) would find the checkout's own
# Cargo.toml above the worktree and take it for its workspace.
work=$(mktemp -d "${FOGATA_CI_WORK:-/var/tmp}/fogata-ci-XXXXXX")
tree="$work/tree"
slot=()
if [[ -n "${FOGATA_CI_SLOT:-}" ]]; then
    read -r -a slot <<< "$FOGATA_CI_SLOT"
fi
webrtc_downloads=app/third_party/flutter_webrtc/third_party/downloads

cleanup() {
    git worktree remove --force "$tree" > /dev/null 2>&1 || true
    rm -rf "${work:?}"
    git worktree prune
}
trap cleanup EXIT

: > "$log"
# Progress goes to the terminal (stderr) and the log.
say() { echo "$*" | tee -a "$log" >&2; }

# Empties a folder of kept build outputs that grew past the limit (see the top).
cache_gb=${FOGATA_CI_CACHE_GB:-40}
[[ $cache_gb =~ ^[0-9]+$ ]] || { echo "ci: FOGATA_CI_CACHE_GB must be a number of GB" >&2; exit 2; }
trim_cache() {
    [[ -d $1 ]] || return 0
    local kb
    kb=$(du -sk "$1" 2> /dev/null | cut -f1) || return 0
    if (( kb > cache_gb * 1024 * 1024 )); then
        say "ci: ${1##*/} held $((kb / 1024 / 1024)) GB of old builds, past ${cache_gb} GB: emptied"
        rm -rf "${1:?}"
    fi
}

say "ci: $sha ($(git log -1 --format=%s "$sha"))"
git worktree add --detach "$tree" "$sha" >> "$log" 2>&1
if [[ "${FOGATA_CI_FRESH:-0}" != 1 ]]; then
    trim_cache "$records/target"
    trim_cache "$records/fuzz-target"
    mkdir -p "$records/target" "$records/downloads"
    ln -s "$records/target" "$tree/target"
    mkdir -p "$tree/$webrtc_downloads"
    cp "$records/downloads/"*.zip "$tree/$webrtc_downloads/" 2> /dev/null || true
fi

# Runs step $1 (a name) as the rest of the arguments in the worktree; its output goes to the log.
step() {
    local name=$1
    shift
    say "ci: $name"
    if (cd "$tree" && "$@") >> "$log" 2>&1; then
        say "ci: $name passed"
    else
        say "ci: $name FAILED (see $log)"
        return 1
    fi
}

# The native tests and the integration tests are recorded on their own too: pass, fail,
# skipped (the integration tests, when this computer lacks what they need) or not run (an
# earlier step failed).
native="not run"
integration="not run"
pairs="not run (--full not given)"
[[ $full == 1 ]] && pairs="not run"
smoke="not run (--full not given)"
[[ $full == 1 ]] && smoke="not run"
if ! half app; then
    native=$elsewhere integration=$elsewhere
    [[ $full == 0 ]] || pairs=$elsewhere smoke=$elsewhere
fi
native_tests() {
    if step "native tests (video renderer with ThreadSanitizer, noise suppression table)" \
        nice -n 10 app/third_party/flutter_webrtc/common/cpp/test/run_tests.sh; then
        native=pass
    else
        native=fail
        return 1
    fi
}
integration_tests() {
    say "ci: integration tests"
    local status=0
    (cd "$tree" && ${slot[@]+"${slot[@]}"} nice -n 10 \
        tools/integration-tests.sh) >> "$log" 2>&1 || status=$?
    case $status in
        0)
            integration=pass
            say "ci: integration tests passed"
            ;;
        3)
            # What tools/integration-tests.sh said was missing.
            local missing
            missing=$(sed -n 's/^integration tests not run: //p' "$log" | tail -n 1)
            integration="skipped ($missing)"
            say "ci: integration tests SKIPPED ($missing)"
            ;;
        *)
            integration=fail
            say "ci: integration tests FAILED (see $log)"
            return 1
            ;;
    esac
}
# The tests of two copies of the application talking to each other: only with --full.
pair_tests() {
    say "ci: integration pairs (call, direct, voice, move)"
    local status=0
    (cd "$tree" && ${slot[@]+"${slot[@]}"} nice -n 10 \
        tools/integration-pairs.sh) >> "$log" 2>&1 || status=$?
    case $status in
        0)
            pairs="pass ($(sed -n 's/^integration pairs passed: //p' "$log" | tail -n 1))"
            say "ci: integration pairs passed"
            ;;
        3)
            local missing
            missing=$(sed -n 's/^integration pairs not run: //p' "$log" | tail -n 1)
            pairs="skipped ($missing)"
            say "ci: integration pairs SKIPPED ($missing)"
            ;;
        *)
            pairs="fail ($(sed -n 's/^integration pairs failed: //p' "$log" | tail -n 1))"
            say "ci: integration pairs FAILED (see $log)"
            return 1
            ;;
    esac
}
# The smoke test on the Linux desktops this computer can build: only with --full.
smoke_tests() {
    say "ci: smoke test (wayland, xwayland, bare, x11)"
    local status=0 summary
    (cd "$tree" && ${slot[@]+"${slot[@]}"} nice -n 10 \
        tools/smoke-linux.sh) >> "$log" 2>&1 || status=$?
    summary=$(sed -n 's/^smoke: passed \(.*\)$/\1/p' "$log" | tail -n 1)
    case $status in
        0)
            smoke="pass ($summary)"
            say "ci: smoke test passed ($summary)"
            ;;
        3)
            smoke="skipped ($summary; $(grep '^smoke session: ' "$log" | tail -n 1))"
            say "ci: smoke test SKIPPED: no desktop could be built here (see $log)"
            ;;
        *)
            smoke="fail ($summary)"
            say "ci: smoke test FAILED (see $log)"
            return 1
            ;;
    esac
}
# The libFuzzer targets (fuzz/, a workspace of its own the other steps never build) must at
# least compile, with the nightly toolchain and cargo-fuzz; they are not run here.
# FOGATA_FUZZ_TOOLCHAIN names the nightly (`nightly` by default; GitHub's job pins a dated one,
# so a new nightly cannot break the run overnight).
fuzz_build() {
    mkdir -p "$records/fuzz-target"
    [[ "${FOGATA_CI_FRESH:-0}" == 1 ]] || ln -s "$records/fuzz-target" fuzz/target
    cd fuzz && ${slot[@]+"${slot[@]}"} nice -n 10 cargo "+${FOGATA_FUZZ_TOOLCHAIN:-nightly}" fuzz build --fuzz-dir .
}
# Runs the rest of the arguments when this run does half $1 (see --split); else passes.
in_half() {
    local which=$1
    shift
    if half "$which"; then "$@"; fi
}
linux=pass
{
    step "flutter pub get" bash -c 'cd app && flutter pub get --offline' &&
    in_half app step "linux build" bash -c 'cd app && "$@" nice -n 10 flutter build linux --release' _ ${slot[@]+"${slot[@]}"} &&
    in_half app step "release checks of the bundle" python3 -I -B tools/beta/ci_builds.py check-build linux app/build/linux/x64/release/bundle &&
    in_half app native_tests &&
    in_half checks step "check.sh" ${slot[@]+"${slot[@]}"} nice -n 10 tools/check.sh ${skips[@]+"${skips[@]}"} &&
    in_half checks step "fuzz targets build" fuzz_build &&
    in_half app step "gen-l10n" bash -c 'cd app && flutter gen-l10n' &&
    step "nothing generated differs" bash -c 'test -z "$(git status --porcelain --untracked-files=no)" || { git status --short; git diff --stat; exit 1; }' &&
    in_half app step "dart format" bash -c 'cd app && dart format --output=none --set-exit-if-changed lib test integration_test test_driver packages/fogata_video' &&
    in_half app step "flutter analyze" bash -c 'cd app && flutter analyze' &&
    in_half app step "flutter test" bash -c 'cd app && "$@" nice -n 10 flutter test' _ ${slot[@]+"${slot[@]}"} &&
    in_half app integration_tests &&
    { [[ $full == 0 ]] || in_half app pair_tests; } &&
    { [[ $full == 0 ]] || in_half app smoke_tests; }
} || linux=fail
if [[ "${FOGATA_CI_FRESH:-0}" != 1 ]]; then
    cp "$tree/$webrtc_downloads/"*.zip "$records/downloads/" 2> /dev/null || true
fi

# What ran: the tools, and the scripts as they were.
tool() { "$@" 2> /dev/null | head -n 1 || true; }
sha256() { { sha256sum 2> /dev/null || shasum -a 256; } | cut -d' ' -f1; }

# A leg on another system: $1 its name (macos, windows), $2 the command (empty: skipped). Sets
# leg_result (skipped, fail, incomplete or pass) and what the record keeps of it: leg_command,
# leg_exit, leg_output (the SHA-256 and the number of lines of its output) and leg_steps.
# It passes only when the command exits 0 and says, in a line of its output, that it checked
# this commit on this system with no step left out (tools/ci-leg.sh prints it); a command that
# exits 0 and says nothing (`true`) is "incomplete". The output goes to the log.
leg() {
    local name=$1 command=$2 out status=0 line
    leg_result=skipped leg_command=none leg_exit=none leg_output=none leg_steps=none
    if [[ -z "$command" ]]; then
        say "ci: $name SKIPPED (not set up on this computer)"
        return
    fi
    say "ci: $name"
    out="$work/leg-$name.out"
    FOGATA_CI_SHA=$sha bash -c "$command" > "$out" 2>&1 || status=$?
    cat "$out" >> "$log"
    leg_command=$(printf '%s' "$command" | tr '\n' ' ')
    leg_exit=$status
    leg_output="$(sha256 < "$out") ($(wc -l < "$out" | tr -d ' ') lines)"
    line=$(grep -E "^fogata-ci-leg: system=$name commit=$sha status=pass( |\$)" "$out" | tail -n 1 || true)
    leg_steps=$(printf '%s' "$line" | sed -n 's/.* steps=\([^ ]*\).*/\1/p')
    if (( status != 0 )); then
        leg_result=fail
        say "ci: $name FAILED (exit $status; see $log)"
    elif [[ -z $line ]]; then
        leg_result=incomplete
        say "ci: $name INCOMPLETE (exit 0, but its output does not say that it checked $sha on $name)"
    elif [[ $line != *" skipped=none" ]]; then
        leg_result=incomplete
        say "ci: $name INCOMPLETE (steps left out: $(printf '%s' "$line" | sed -n 's/.* skipped=\([^ ]*\).*/\1/p'))"
    else
        leg_result=pass
        say "ci: $name passed"
    fi
}
leg macos "${FOGATA_CI_MAC:-}"
mac=$leg_result mac_command=$leg_command mac_exit=$leg_exit mac_output=$leg_output mac_steps=${leg_steps:-none}
leg windows "${FOGATA_CI_WINDOWS:-}"
windows=$leg_result windows_command=$leg_command windows_exit=$leg_exit windows_output=$leg_output windows_steps=${leg_steps:-none}

database=$(sed -n 's/^RustSec database of \([0-9-]*\).*/\1/p' "$log" | tail -n 1)
# Anything that failed fails the run; else anything skipped or incomplete leaves it incomplete.
result=pass
for state in "$linux" "$mac" "$windows"; do
    if [[ $state == fail ]]; then
        result=fail
    elif [[ ( $state == skipped || $state == incomplete ) && $result == pass ]]; then
        result=incomplete
    fi
done
for check in advisories codegen notices; do
    if ! checked "$check" && [[ $result == pass ]]; then
        result=incomplete
    fi
done
if [[ $integration == skipped* && $result == pass ]]; then
    result=incomplete
fi
# Half the steps is never a pass, whatever that half did.
if [[ $split != none && $result == pass ]]; then
    result=incomplete
fi
# The pair tests not run (no --full), or skipped, leave the result incomplete, never a pass: a
# commit that breaks a call, a direct chat, a voice message or an account move gets through
# the hook only when FOGATA_CI_ALLOW_SKIPPED names `pairs`.
case $pairs in
    fail*) result=fail ;;
    skipped* | "not run"* | elsewhere*) [[ $result == pass ]] && result=incomplete ;;
esac
# The smoke test fails the run when it fails; skipped (no desktop could be built here) it is
# only recorded.
[[ $smoke == fail* ]] && result=fail
# Which legs ran (passed or failed) and which did not, so a record never passes in silence.
ran=() notrun=()
for pair in "linux:$linux" "macos:$mac" "windows:$windows" "native:$native" \
    "integration:$integration" "pairs:$pairs" "smoke:$smoke"; do
    case ${pair#*:} in
        pass* | fail*) ran+=("${pair%%:*}") ;;
        *) notrun+=("${pair%%:*} (${pair#*:})") ;;
    esac
done
{
    echo "commit: $sha"
    echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "linux: $linux"
    echo "macos: $mac"
    echo "windows: $windows"
    echo "full: $([[ $full == 1 ]] && echo yes || echo no)"
    echo "split: $split"
    echo "legs ran: ${ran[*]:-nothing}"
    echo "legs not run: ${notrun[*]:-nothing}"
    for leg_name in macos windows; do
        case $leg_name in
            macos) values=("$mac_command" "$mac_exit" "$mac_output" "$mac_steps") ;;
            windows) values=("$windows_command" "$windows_exit" "$windows_output" "$windows_steps") ;;
        esac
        echo "$leg_name command: ${values[0]}"
        echo "$leg_name exit: ${values[1]}"
        echo "$leg_name output: ${values[2]}"
        echo "$leg_name steps: ${values[3]}"
    done
    if ! half checks; then
        for check in advisories codegen notices; do
            echo "$check: $elsewhere"
        done
    else
        if checked advisories; then
            echo "advisories: checked (RustSec database of ${database:-an unknown date})"
        else
            echo "advisories: skipped"
        fi
        for check in codegen notices; do
            echo "$check: $(checked "$check" && echo checked || echo skipped)"
        done
    fi
    echo "native: $native"
    echo "integration: $integration"
    echo "pairs: $pairs"
    echo "smoke: $smoke"
    echo "rustc: $(cd "$tree" 2> /dev/null && tool rustc -V)"
    echo "cargo: $(cd "$tree" 2> /dev/null && tool cargo -V)"
    echo "flutter: $(tool flutter --version --suppress-analytics)"
    echo "ci.sh: $(sha256 < "$script")"
    echo "check.sh: $(git show "$sha:tools/check.sh" | sha256)"
    echo "cleared: ${cleared[*]:-nothing}"
    echo "result: $result"
} > "$record"
say "ci: result $result (linux $linux, macos $mac, windows $windows, pairs $pairs, smoke $smoke); record $record"
[[ $result == pass ]]

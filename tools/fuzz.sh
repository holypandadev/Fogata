#!/usr/bin/env bash
# Fuzzes the decoders of everything that comes from outside (see docs/fuzzing.md).
#
#   tools/fuzz.sh [MINUTES] [TARGET...]
#       Long runs of damaged inputs, with the stable toolchain: each target runs for MINUTES
#       (10 by default), JOBS (4 by default) of them at once.
#   tools/fuzz.sh --libfuzzer [MINUTES] [TARGET...]
#       The same targets under libFuzzer, guided by coverage (nightly toolchain and
#       cargo-fuzz): each runs for MINUTES, JOBS of them at once, one process each. Corpora
#       grow in fuzz/corpus/<target>, findings land in fuzz/artifacts/<target>.
#
# Without TARGET, every target runs. Runs at a lower priority (nice), so the computer stays
# usable. Written for bash 3.2, the one macOS ships: no mapfile, no `wait -n`, no GNU find.
set -euo pipefail
if [[ -z "${BASH_VERSION:-}" ]]; then
    echo "tools/fuzz.sh needs bash (3.2 or newer)" >&2
    exit 2
fi
cd "$(dirname "$0")/.."

libfuzzer=false
if [[ "${1:-}" == "--libfuzzer" ]]; then
    libfuzzer=true
    shift
fi
minutes=${1:-10}
shift || true
jobs=${JOBS:-4}
seconds=$((minutes * 60))

# Every target, as fuzz/fuzz_targets names them.
all=()
for file in fuzz/fuzz_targets/*.rs; do
    name=${file##*/}
    all+=("${name%.rs}")
done
targets=()
if [[ $# -gt 0 ]]; then
    targets=("$@")
else
    targets=("${all[@]}")
fi
for target in "${targets[@]}"; do
    if [[ ! -f "fuzz/fuzz_targets/$target.rs" ]]; then
        echo "no target $target; there are: ${all[*]}" >&2
        exit 2
    fi
done

if ! $libfuzzer; then
    filters=()
    for target in "${targets[@]}"; do
        filters+=("fuzzing::runs::$target")
    done
    FOGATA_FUZZ_SECS=$seconds nice -n 10 cargo test -p fogata-core --lib -- \
        --ignored --exact --test-threads "$jobs" --nocapture "${filters[@]}"
    exit
fi

# The seeds every target starts from, written fresh; the corpora keep what libFuzzer found.
FOGATA_FUZZ_SEEDS="$PWD/fuzz/seeds" nice -n 10 cargo test -p fogata-core --lib -- \
    --ignored --exact fuzzing::runs::write_seeds
cd fuzz
nice -n 10 cargo +nightly fuzz build --fuzz-dir .
mkdir -p logs
status=0
pids=()
# Waits for the oldest run still going (bash 3.2 has no `wait -n`).
reap() {
    wait "${pids[0]}" || status=1
    pids=(${pids[@]+"${pids[@]:1}"})
}
for target in "${targets[@]}"; do
    mkdir -p "corpus/$target"
    echo "$target: $minutes min, log in fuzz/logs/$target.log"
    nice -n 10 cargo +nightly fuzz run --fuzz-dir . "$target" "corpus/$target" "seeds/$target" -- \
        -max_total_time="$seconds" -timeout=10 -rss_limit_mb=2048 \
        -print_final_stats=1 > "logs/$target.log" 2>&1 &
    pids+=($!)
    if [[ ${#pids[@]} -ge $jobs ]]; then
        reap
    fi
done
while [[ ${#pids[@]} -gt 0 ]]; do
    reap
done
for target in "${targets[@]}"; do
    if compgen -G "artifacts/$target/*" > /dev/null; then
        echo "$target: findings in fuzz/artifacts/$target"
        status=1
    fi
done
exit $status

#!/usr/bin/env bash
# Makes reports of what a copy allocated in a measure.sh run with MEASURE_HEAP=<role>, from
# its heaptrack-<role>.zst (the whole run) and heaptrack-<role>-early.zst (the run up to
# MEASURE_EARLY seconds, once the call had settled), with heaptrack_print:
#   heap-<role>-growth.txt      what was allocated after the early part and never freed,
#                               by call stack, largest first: where a call's memory grows
#   heap-<role>-growth.folded   the same as folded stacks, for a flame graph
#   heap-<role>-held.txt        everything held when the copy ended, and the peaks
#   heap-<role>-massif.out      memory over time by allocating function (massif's format)
# Each step that fails is said in heap-<role>-report.log and the others go on.
#
# Usage: tools/call-lab/heap-report.sh <run folder> <role>
set -uo pipefail
RUN=${1:?run folder} ROLE=${2:?role}
WHOLE=$RUN/heaptrack-$ROLE.zst EARLY=$RUN/heaptrack-$ROLE-early.zst
LOG=$RUN/heap-$ROLE-report.log
[ -s "$WHOLE" ] || { echo "no $WHOLE" >&2; exit 1; }
: > "$LOG"
step() { # what, command...
  local what=$1 began=$SECONDS
  shift
  if "$@" 2>> "$LOG"; then echo "ok ($((SECONDS - began)) s): $what" >> "$LOG"; else echo "FAILED ($?): $what" >> "$LOG"; fi
}
echo "whole run $(stat -c %s "$WHOLE") bytes packed, early part $(stat -c %s "$EARLY" 2> /dev/null || echo none)" >> "$LOG"
step "what grew after the early part" sh -c "heaptrack_print --file='$WHOLE' \
  --diff='$EARLY' --print-leaks=1 --print-peaks=0 \
  --print-allocators=0 --print-temporary=0 --peak-limit=60 --sub-peak-limit=12 \
  --print-flamegraph='$RUN/heap-$ROLE-growth.folded' --flamegraph-cost-type=leaked \
  > '$RUN/heap-$ROLE-growth.txt'"
step "what was held at the end" sh -c "heaptrack_print --file='$WHOLE' \
  --print-leaks=1 --print-peaks=1 --print-allocators=0 --print-temporary=0 \
  --peak-limit=40 --sub-peak-limit=10 --print-massif='$RUN/heap-$ROLE-massif.out' \
  > '$RUN/heap-$ROLE-held.txt'"
cat "$LOG"
! grep -q '^FAILED' "$LOG"

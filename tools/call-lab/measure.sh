#!/usr/bin/env bash
# Measures a call of 2 to 8 copies of the app on this computer (Linux), each in the same
# sound session of its own (session.sh) on one virtual display:
#   idle    every copy in the room with its microphone off: what the call costs without media
#   voice   every copy sends a tone as its voice
#   camera  the same, and every copy its camera (the made-up one of debug builds)
#   screen  the voice, and copy a shares the virtual display, watched by all the others
# After every picture arrives and 10 s more, each copy writes every 2 s what it receives from
# each other one and its own processor time and memory (FOGATA_TEST_MEASURE in
# app/integration_test/call_test.dart); sum.py makes tables of the run folders.
#
# Usage: tools/call-lab/measure.sh <copies> <idle|voice|camera|screen> [seconds, default 60]
# Build first: (cd app && flutter build linux --debug -t integration_test/call_test.dart)
# Environment: BUNDLE (another build), CALL_LAB_RUNS (where run folders go, default
# ~/call-lab), MEASURE_ENV (more VAR=value for every copy), LAB_DISPLAY (default 171), LOAD_MAX (the computer's load to wait under before
# starting, default 6: other work on it shows in every figure). Prints the run folder at the end.
#
# For a call's memory over hours: every minute smaps.log gets what each copy's memory is
# made of (smaps.py), and with MEASURE_HEAP=<role> that copy runs under heaptrack's library
# (HEAPTRACK_LIB and HEAPTRACK_INTERPRET, default those under /usr/lib), which notes every
# allocation with its call stack: heaptrack-<role>.zst (the whole run) and
# heaptrack-<role>-early.zst (up to MEASURE_EARLY seconds, default 1800);
# heap-report.sh makes reports of them. That copy runs slower: its other figures are not
# those of a call.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${1:-}" = --inside ]; then
  shift
  N=$1 MODE=$2 SECS=$3
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  lab_start "$N"
  roles=("${ROLES[@]}")
  mic=1
  [ "$MODE" = idle ] && mic=off
  heap=""
  for r in "${roles[@]:0:N}"; do
    extra=(FOGATA_TEST_MEASURE="$SECS")
    if [ "${MEASURE_HEAP:-}" = "$r" ]; then
      lib=${HEAPTRACK_LIB:-$(ls /usr/lib/heaptrack/libheaptrack_preload.so /usr/lib/*/heaptrack/libheaptrack_preload.so 2> /dev/null | head -n 1)}
      interpret=${HEAPTRACK_INTERPRET:-$(ls /usr/lib/heaptrack/libexec/heaptrack_interpret /usr/lib/*/heaptrack/libexec/heaptrack_interpret 2> /dev/null | head -n 1)}
      [ -n "$lib" ] && [ -e "$lib" ] && [ -n "$interpret" ] && [ -x "$interpret" ] ||
        { echo "no heaptrack library or interpreter (HEAPTRACK_LIB, HEAPTRACK_INTERPRET)" | tee -a "$RUN/lab.log" >&2; lab_stop; exit 1; }
      # The library writes as the copy allocates. heaptrack's interpreter gives the call
      # stacks their names on the way to the disk, as heaptrack itself does (done after
      # hours of a call, it took longer than the call), twice: the whole run, and the run
      # up to MEASURE_EARLY seconds (default 1800), once the call has settled, which
      # heap-report.sh takes from the whole to say what grew. Both end when the copy does.
      mkfifo "$RUN/heap-$r.fifo"
      tee >(python3 -I "$HERE/heap_until.py" "${MEASURE_EARLY:-1800}" | "$interpret" 2> /dev/null |
        zstd -q -3 > "$RUN/heaptrack-$r-early.zst") < "$RUN/heap-$r.fifo" |
        "$interpret" 2> "$RUN/heap-$r-interpret.log" | zstd -q -3 > "$RUN/heaptrack-$r.zst" &
      heap=$!
      # The sound libraries the call opens by name when it starts come in with it: opened
      # then, on one thread while others allocate, the library's lock and the loader's
      # were taken in both orders now and then, and the copy stopped for good.
      extra+=(LD_PRELOAD="$lib libpulse.so.0 libasound.so.2 libpipewire-0.3.so.0"
        DUMP_HEAPTRACK_OUTPUT="$RUN/heap-$r.fifo"
        LD_LIBRARY_PATH="$(dirname "$BUNDLE")/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}")
    fi
    [ "$MODE" = camera ] && extra+=(FOGATA_TEST_CAMERA=1)
    [ "$MODE" = screen ] && [ "$r" = a ] && extra+=(FOGATA_TEST_SCREEN=1)
    # MEASURE_ENV: more settings for every copy, such as FOGATA_TEST_CAMERA_PEOPLE=6.
    [ -n "${MEASURE_ENV:-}" ] && read -r -a more <<< "$MEASURE_ENV" && extra+=("${more[@]}")
    lab_copy "$r" "$N" "$mic" "${extra[@]}"
  done
  # The highest memory each copy has held so far and the computer's load, every 5 s.
  (
    while true; do
      line="$(date +%s) load $(cut -d' ' -f1 /proc/loadavg)"
      for r in "${roles[@]:0:N}"; do
        line="$line $r:$(awk '/VmHWM/ {print $2}' "/proc/$(cat "$RUN/$r.pid")/status" 2>/dev/null)"
      done
      echo "$line" >> "$RUN/proc.log"
      sleep 5
    done
  ) &
  sampler=$!
  # What each copy's memory is made of, every minute.
  (
    while true; do
      for r in "${roles[@]:0:N}"; do
        echo "$(date +%s) $r $(python3 -I "$HERE/smaps.py" "$(cat "$RUN/$r.pid")")" >> "$RUN/smaps.log"
      done
      sleep 60
    done
  ) &
  kinds=$!
  lab_wait "${roles[*]:0:N}" $((SECS + 1200))
  kill "$sampler" "$kinds"
  lab_stop
  # The last of the allocations, written once the copy is gone; the early part's own
  # pipe ends by itself a moment after.
  if [ -n "$heap" ]; then
    wait "$heap"
    for _ in $(seq 60); do
      size=$(stat -c %s "$RUN"/heaptrack-?-early.zst 2> /dev/null | head -n 1)
      sleep 1
      [ "$size" = "$(stat -c %s "$RUN"/heaptrack-?-early.zst 2> /dev/null | head -n 1)" ] && [ -n "$size" ] && [ "$size" != 0 ] && break
    done
  fi
  exit 0
fi

[ $# -ge 2 ] || { echo "usage: $0 <copies> <idle|voice|camera|screen> [seconds]" >&2; exit 2; }
N=$1 MODE=$2 SECS=${3:-60}
case "$MODE" in idle | voice | camera | screen) ;; *) echo "no mode $MODE" >&2; exit 2 ;; esac
[ "$N" -ge 2 ] && [ "$N" -le 8 ] || { echo "2 to 8 copies" >&2; exit 2; }
REPO=$(cd "$HERE/../.." && pwd)
export BUNDLE=${BUNDLE:-$REPO/app/build/linux/x64/debug/bundle/fogata}
[ -x "$BUNDLE" ] || { echo "no build at $BUNDLE" >&2; exit 1; }
export RUN=${CALL_LAB_RUNS:-$HOME/call-lab}/measure-$N-$MODE-$(date +%m%d-%H%M%S)
mkdir -p "$RUN"
for _ in $(seq 300); do
  [ "$(cut -d. -f1 /proc/loadavg)" -lt "${LOAD_MAX:-6}" ] && break
  sleep 2
done
echo "copies $N, $MODE, $SECS s, $(git -C "$REPO" describe --always --dirty), nice $(nice), ${MEASURE_ENV:-} started $(date -Is), load $(cut -d' ' -f1-3 /proc/loadavg)" > "$RUN/settings.txt"
"$HERE/session.sh" "$RUN/session" "$0" --inside "$N" "$MODE" "$SECS"
echo "$RUN"

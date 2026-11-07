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
  for r in "${roles[@]:0:N}"; do
    extra=(FOGATA_TEST_MEASURE="$SECS")
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
  lab_wait "${roles[*]:0:N}" $((SECS + 1200))
  kill "$sampler"
  lab_stop
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

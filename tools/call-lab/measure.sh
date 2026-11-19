#!/usr/bin/env bash
# Measures a call of 2 to 8 copies of the app on this computer (Linux), each in the same
# sound session of its own (session.sh) on one virtual display:
#   idle    every copy in the room with its microphone off: what the call costs without media
#   voice   every copy sends a tone as its voice
#   camera  the same, and every copy its camera (the made-up one of debug builds)
#   screen  the voice, and copy a shares the virtual display, watched by all the others
# After every picture arrives and 10 s more, each copy writes every 2 s what it receives from
# each other one and its own processor time and memory (FOGATA_TEST_MEASURE in
# app/integration_test/call_test.dart), with what it sends each one: the encoder's name
# ("libvpx (shared)" when every connection that sends its screen at a size shares one),
# the key frames it encoded and the key frame requests (PLI) it got. Every 5 s, proc.log
# has each copy's highest memory so far and the computer's load, and threads.log its
# encoder threads: VideoStreamEncoders' ("VideoEncoderQue", one for each picture each
# connection sends) and shared screen encoders' ("SharedScreenEnc", one for each size a
# shared screen goes out at), as "<copy>:<VideoEncoderQue>/<SharedScreenEnc>".
# sum.py makes tables of the run folders. MEASURE_ENV=FOGATA_SHARED_SCREEN_ENCODE=off
# measures without the shared screen encode, for before and after on one build.
#
# A screen measured as it is barely moves: the display shows the copies' own windows. With
# LAB_MOVING=video (the whole display moves in every frame) or LAB_MOVING=desktop (a page of
# text scrolling in a window, the rest of the screen still), something moves on the display
# for the whole run (lab.sh's lab_moving; needs xdotool, and ffplay, from the ffmpeg
# package, for video or python3-tk for desktop), and moving.log has what it costs. SCREEN
# sets the display's size (default 1600x900; 1920x1080 for a 1080p screen), and
# MEASURE_ENV=FOGATA_TEST_SCREEN_QUALITY=720 (or 540, 1080, source, with @60 for 60 frames
# a second) the quality copy a shares at.
#
# The copies share the computer's loopback and nothing limits them. LAB_UPLOAD=<Mbit/s>
# gives copy a (the one sharing its screen) a home connection's upload instead: what it sends
# the others from its own ports goes through that rate and a queue of LAB_UPLOAD_QUEUE ms
# (default 100), and nothing else does (upload.sh says how and why). LAB_UPLOAD_AFTER=<s>
# applies it only that long after copy a starts measuring (a connection that gets worse),
# LAB_UPLOAD_LOSS=<percent> loses that share of what goes up, and LAB_UPLOAD_DELAY=<ms>
# delays what comes back to copy a. It needs root (sudo without a password, as on GitHub's
# computers) and leaves the loopback as it was at the end. Every 2 s upload.log has the
# bytes that went up and were dropped there, and the loopback's own count; each copy's
# measure lines then say what each watcher received and the sender's estimate of each
# connection (watchers.py sums them up watcher by watcher).
#
# Usage: tools/call-lab/measure.sh <copies> <idle|voice|camera|screen> [seconds, default 60]
# Build first: (cd app && flutter build linux --debug -t integration_test/call_test.dart)
# Environment: BUNDLE (another build), CALL_LAB_RUNS (where run folders go, default
# ~/call-lab), MEASURE_ENV (more VAR=value for every copy), LAB_DISPLAY (default 171), LOAD_MAX (the computer's load to wait under before
# starting, default 6: other work on it shows in every figure), LAB_MOVING and SCREEN
# (above). Prints the run folder at the end.
#
# For a call's memory over hours: every minute smaps.log gets what each copy's memory is
# made of (smaps.py), and with MEASURE_HEAP=<role> that copy runs under heaptrack's library
# (HEAPTRACK_LIB and HEAPTRACK_INTERPRET, default those under /usr/lib), which notes every
# allocation with its call stack: heaptrack-<role>.zst (the whole run) and
# heaptrack-<role>-early.zst (up to MEASURE_EARLY seconds, default 1800);
# heap-report.sh makes reports of them. That copy runs slower: its other figures are not
# those of a call, and it never unloads a library (below; it needs a C compiler, CC). A copy with no result in time leaves stuck-<role>.txt for every copy still
# running: its threads' state, processor time and stacks before it is ended (lab.sh's lab_stuck).
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
    # A copy under heaptrack starts many times slower, and the others wait for it.
    [ -n "${MEASURE_HEAP:-}" ] && [ "${MEASURE_HEAP:-}" != none ] && extra+=(FOGATA_TEST_PATIENCE=6)
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
      # The library and the loader each have a lock, and the copy stops for good when two
      # threads take them in both orders. The library takes the loader's (dl_iterate_phdr,
      # for the list of what is loaded) holding its own; the loader frees memory holding
      # its own while it unloads a library, and the free waits for the library's lock.
      # Unloading happens each time the copy looks at its output: ALSA's PipeWire plugin
      # makes a PipeWire context and destroys it, and PipeWire's modules and plugins go
      # with it (dlclose). So here dlclose does nothing (no-dlclose.so, ahead of the
      # library): a library once loaded stays loaded, its mapping a constant part of the
      # copy's memory, never a growth, as loading it again only counts it again. The
      # sound libraries the call opens by name come in with it too, as before.
      printf 'int dlclose(void *handle) { (void)handle; return 0; }\n' |
        "${CC:-cc}" -shared -fPIC -O2 -x c -o "$RUN/no-dlclose.so" - ||
        { echo "cannot build no-dlclose.so (CC)" | tee -a "$RUN/lab.log" >&2; lab_stop; exit 1; }
      extra+=(LD_PRELOAD="$RUN/no-dlclose.so $lib libpulse.so.0 libasound.so.2 libpipewire-0.3.so.0"
        DUMP_HEAPTRACK_OUTPUT="$RUN/heap-$r.fifo"
        LD_LIBRARY_PATH="$(dirname "$BUNDLE")/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}")
    fi
    [ "$MODE" = camera ] && extra+=(FOGATA_TEST_CAMERA=1)
    [ "$MODE" = screen ] && [ "$r" = a ] && extra+=(FOGATA_TEST_SCREEN=1)
    # MEASURE_ENV: more settings for every copy, such as FOGATA_TEST_CAMERA_PEOPLE=6.
    [ -n "${MEASURE_ENV:-}" ] && read -r -a more <<< "$MEASURE_ENV" && extra+=("${more[@]}")
    lab_copy "$r" "$N" "$mic" "${extra[@]}"
  done
  # Copy a's own ports, for the upload (LAB_UPLOAD, below) to take what it sends.
  [ -n "${LAB_UPLOAD:-}" ] && lab_upload_ports a &
  lab_moving || { lab_stop; exit 1; }
  # The highest memory each copy has held so far and the computer's load, every 5 s.
  (
    while true; do
      line="$(date +%s) load $(cut -d' ' -f1 /proc/loadavg)"
      for r in "${roles[@]:0:N}"; do
        line="$line $r:$(awk '/VmHWM/ {print $2}' "/proc/$(cat "$RUN/$r.pid")/status" 2>/dev/null)"
      done
      echo "$line" >> "$RUN/proc.log"
      # Encoder threads by name (/proc truncates names to 15 characters).
      line="$(date +%s)"
      for r in "${roles[@]:0:N}"; do
        names=$(cat "/proc/$(cat "$RUN/$r.pid")"/task/*/comm 2>/dev/null)
        line="$line $r:$(grep -c '^VideoEncoderQue' <<< "$names")/$(grep -c '^SharedScreenEnc' <<< "$names")"
      done
      echo "$line" >> "$RUN/threads.log"
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
echo "copies $N, $MODE, $SECS s, $(git -C "$REPO" describe --always --dirty), nice $(nice), ${MEASURE_ENV:-}, moving ${LAB_MOVING:-off}, display ${SCREEN:-1600x900}, upload ${LAB_UPLOAD:-free}${LAB_UPLOAD:+ Mbit/s after ${LAB_UPLOAD_AFTER:-0} s, queue ${LAB_UPLOAD_QUEUE:-100} ms, loss ${LAB_UPLOAD_LOSS:-0} %, delay ${LAB_UPLOAD_DELAY:-0} ms}, started $(date -Is), load $(cut -d' ' -f1-3 /proc/loadavg)" > "$RUN/settings.txt"
# The upload (upload.sh, as root through sudo, outside the session: its namespace cannot
# change the computer's network), once copy a's ports are known, and every 2 s what went up
# and what the loopback carried in all, to upload.log.
shaper=""
if [ -n "${LAB_UPLOAD:-}" ]; then
  [[ $LAB_UPLOAD =~ ^[0-9]+$ ]] || { echo "LAB_UPLOAD is a whole number of Mbit/s" >&2; exit 2; }
  sudo -n "$HERE/upload.sh" off || { echo "LAB_UPLOAD needs sudo without a password" >&2; exit 1; }
  (
    for _ in $(seq 1200); do [ -s "$RUN/upload-ports" ] && break; sleep 0.5; done
    [ -s "$RUN/upload-ports" ] || exit 0
    if [ -n "${LAB_UPLOAD_AFTER:-}" ]; then
      # That long after copy a starts measuring (call_test.dart's mark "measuring").
      for _ in $(seq 3000); do [ -e "$RUN/meet/mark-a-measuring" ] && break; sleep 0.5; done
      sleep "$LAB_UPLOAD_AFTER"
    fi
    sudo -n "$HERE/upload.sh" on "$(cat "$RUN/upload-ports")" $((LAB_UPLOAD * 1000)) \
      "${LAB_UPLOAD_QUEUE:-100}" "${LAB_UPLOAD_LOSS:-0}" "${LAB_UPLOAD_DELAY:-0}" >> "$RUN/upload.log" 2>&1
  ) &
  shaper=$!
fi
( while true; do "$HERE/upload.sh" show >> "$RUN/upload.log" 2>&1; sleep 2; done ) &
uploads=$!
"$HERE/session.sh" "$RUN/session" "$0" --inside "$N" "$MODE" "$SECS"
kill $shaper $uploads 2> /dev/null
[ -n "${LAB_UPLOAD:-}" ] && sudo -n "$HERE/upload.sh" off
echo "$RUN"

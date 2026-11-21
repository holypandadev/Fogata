#!/usr/bin/env bash
# The echo after the application chooses another output in a call (Linux), in a sound
# session of its own (session.sh). Copy a (microphone on) watches copy b's shared screen,
# whose sound is speech (FAR_FILE at -28 dBFS RMS, from pw-cat: b's voice); what a
# plays goes back into a's microphone through a loop of the output it plays on:
#   far      a's first output (FogataQuietOut) loops back DELAY1 ms later (default 60),
#            the other (FogataOther) DELAY2 ms later (default 180): the echo's delay moves
#   same     both DELAY1 ms later: only the restart
#   headset  no loop: no echo path (the cost side)
# After SETTLE s (default 25) of a watching b, every CYCLE s (default 20) a chooses the
# other output (CYCLES times, default 8, starting with FogataOther), through the call's
# own path (call_test.dart's far-echo case, `rig-output`). a's words (unmute-sound.sh's,
# -28 dBFS) go into its microphone 0.5 and 3.5 s after every second change (the odd ones:
# double talk right after the change) and 10.5 and 13.5 s after every change (the steady
# reference). Recorded throughout: what b plays of a (played-b), a's microphone (mic-a)
# and what a plays on each output (out1, out2); changes.log has each change's moments
# (asked, and done as a marked it), words.log each word's moment.
# Measured, not judged: restart_echo.py <run>... (each run), restart_echo_sum.py <list>
# (lines of "<build> <variant> <run>"; median and range per build and variant).
# Usage: restart-echo.sh far|same|headset   (BUNDLE, CALL_LAB_RUNS, LAB_DISPLAY, FAR_FILE,
# CYCLES, CYCLE, SETTLE, DELAY1, DELAY2, RIG_ENV: more VAR=value for both copies)
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
now() { date +%s%N | cut -c1-13; }
if [ "${1:-}" = --inside ]; then
  V=$2
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  export LAB_NO_TONE="a b"
  lab_start 2
  "$LAB_DIR/unmute-sound.sh" "$RUN/sound" >> "$RUN/lab.log" 2>&1
  # a's second output, listed for it (its name ends as its devices' do).
  pactl load-module module-null-sink sink_name=laba_other sink_properties=device.description=FogataOther-laba. > /dev/null
  case "$V" in
    far | same)
      d2=${DELAY2:-180}
      [ "$V" = same ] && d2=${DELAY1:-60}
      pactl load-module module-loopback source=laba_quiet.monitor sink=laba_test latency_msec="${DELAY1:-60}" \
        source_dont_move=true sink_dont_move=true > /dev/null
      pactl load-module module-loopback source=laba_other.monitor sink=laba_test latency_msec="$d2" \
        source_dont_move=true sink_dont_move=true > /dev/null
      echo "loops: FogataQuietOut ${DELAY1:-60} ms, FogataOther $d2 ms" >> "$RUN/lab.log"
      ;;
  esac
  # b's voice: speech at -28 dBFS RMS, which the window b shares plays (below).
  pactl load-module module-null-sink sink_name=lab_apps sink_properties=device.description=LabApps > /dev/null
  mean=$(ffmpeg -hide_banner -nostats -i "$FAR_FILE" -af volumedetect -f null - 2>&1 |
    sed -n 's/.*mean_volume: \(-\{0,1\}[0-9.]*\) dB.*/\1/p')
  : "${mean:?no mean_volume from the volumedetect of ffmpeg for $FAR_FILE: not a sound file it reads?}"
  ffmpeg -hide_banner -loglevel error -y -i "$FAR_FILE" \
    -af "volume=$(python3 -I -c "print(round(-28 - ($mean), 2))")dB" -ar 48000 -ac 1 -sample_fmt s16 "$RUN/sound/far.wav"
  for name in played-b:labb_quiet.monitor mic-a:laba_test.monitor out1:laba_quiet.monitor out2:laba_other.monitor; do
    now > "$RUN/${name%%:*}.start"
    parec --device="${name#*:}" --format=s16le --rate=48000 --channels=1 --raw \
      --latency-msec=20 > "$RUN/${name%%:*}.raw" 2> /dev/null &
    echo $! > "$RUN/rec-${name%%:*}.pid"
  done
  read -ra extra <<< "${RIG_ENV:-}"
  common=(FOGATA_TEST_MIC_CASE=far-echo FOGATA_TEST_ECHO_WINDOW=screen FOGATA_WEBRTC_LOG=info "${extra[@]}")
  # b's voice: pw-cat plays it into its own sink, and b's shared screen takes that
  # program's sound.
  ( trap 'kill "$(cat "$RUN/pwcat-far.pid")" 2>/dev/null; exit 0' TERM HUP INT
    while true; do
      pw-cat --playback --target lab_apps -P "$LAB_APP_PROPS" "$RUN/sound/far.wav" 2>/dev/null &
      echo $! > "$RUN/pwcat-far.pid"
      wait $!
    done ) > /dev/null 2>&1 &
  echo $! > "$RUN/far.pid"
  # Each copy plays into its silent output (the test's own choice is the output its
  # microphone records), chosen as the call is up; a's loops make its echo.
  lab_copy a 2 1 "${common[@]}" FOGATA_TEST_OUTPUT_LATER=FogataQuietOut-laba
  lab_copy b 2 off "${common[@]}" FOGATA_TEST_OUTPUT_LATER=FogataQuietOut-labb FOGATA_SCREEN_SOUND_FROM=pw-cat
  # The rig: a watches b, the changes, a's words.
  until [ -e "$M/mark-a-ready" ]; do
    kill -0 "$(cat "$RUN/a.pid")" 2>/dev/null || { echo "a ended before it was ready" >> "$RUN/lab.log"; lab_stop; exit 1; }
    sleep 0.2
  done
  touch "$M/rig-watch"
  word() { # at (ms since the epoch), what
    python3 -I -c "import time; time.sleep(max(0, $1 / 1000 - time.time()))"
    paplay --device=laba_test "$RUN/sound/words.wav" 2> /dev/null &
    echo "$2 $1 $(now)" >> "$RUN/words.log"
  }
  sleep "${SETTLE:-25}"
  cycle=$(( ${CYCLE:-20} * 1000 ))
  t0=$(now)
  for k in $(seq 0 $(( ${CYCLES:-8} - 1 ))); do
    at=$((t0 + k * cycle))
    python3 -I -c "import time; time.sleep(max(0, $at / 1000 - time.time()))"
    if [ $((k % 2)) = 0 ]; then out=FogataOther-laba; else out=FogataQuietOut-laba; fi
    old=$(cat "$M/mark-a-output-$out" 2> /dev/null)
    asked=$(now)
    echo "$out" > "$M/rig-output"
    ( for _ in $(seq 100); do
        done_at=$(cat "$M/mark-a-output-$out" 2> /dev/null)
        [ -n "$done_at" ] && [ "$done_at" != "$old" ] && break
        sleep 0.05
      done
      echo "$k $out asked $asked done ${done_at:-none}" >> "$RUN/changes.log" ) &
    if [ $((k % 2)) = 1 ]; then
      word $((asked + 500)) "$k dt0.5"
      word $((asked + 3500)) "$k dt3.5"
    fi
    word $((asked + 10500)) "$k ref10.5" &
    ( word $((asked + 13500)) "$k ref13.5" ) &
  done
  sleep $(( ${CYCLE:-20} + 2 ))
  touch "$M/rig-end"
  lab_wait "a b" 120
  for f in far rec-played-b rec-mic-a rec-out1 rec-out2; do
    [ -e "$RUN/$f.pid" ] && kill "$(cat "$RUN/$f.pid")" 2>/dev/null
  done
  sleep 1
  lab_stop
  exit 0
fi
V=${1:?usage: restart-echo.sh far|same|headset}
REPO=$(cd "$HERE/../.." && pwd)
export BUNDLE=${BUNDLE:-$REPO/app/build/linux/x64/debug/bundle/fogata}
export FAR_FILE=${FAR_FILE:?FAR_FILE: a speech recording}
export RUN=${CALL_LAB_RUNS:-$HOME/call-lab}/restart-$V-$(date +%m%d-%H%M%S)
mkdir -p "$RUN"
echo "restart-echo $V, started $(date -Is); BUNDLE=$BUNDLE DELAY1=${DELAY1:-60} DELAY2=${DELAY2:-180} CYCLES=${CYCLES:-8} CYCLE=${CYCLE:-20} SETTLE=${SETTLE:-25} RIG_ENV=${RIG_ENV:-}" > "$RUN/settings.txt"
"$HERE/session.sh" "$RUN/session" "$0" --inside "$V"
echo "  $RUN"

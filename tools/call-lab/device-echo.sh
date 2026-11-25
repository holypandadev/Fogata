#!/usr/bin/env bash
# The echo around changes of devices in a call (Linux), in a sound session of its own:
# copy a (microphone on) watches copy b's shared screen, whose sound is speech (FAR_FILE
# at -28 dBFS RMS, from pw-cat: b's voice). a has two outputs, FogataQuietOut (Q) and
# FogataOther (O), each looped back into its microphone LOOP_Q / LOOP_O ms later ("none":
# no loop, a headset), and two inputs carrying its microphone, FogataMic and FogataMicTwo.
# After SETTLE s, CYCLES cycles of CYCLE s each run a program of timed actions:
# PROG_EVEN for even cycles, PROG_ODD for odd ones (default PROG_EVEN), each a list of
# "<seconds into the cycle>:<action>[:<argument>]" separated by spaces:
#   out:<name>    a chooses that output (call_test's rig-output)
#   in:<name>     a chooses that input (rig-input)
#   mic:on|off    a turns its microphone on or off (rig-mic)
#   words:<label> a's words into its microphone (unmute-sound.sh's, -28 dBFS)
#   pulse         the session's PulseAudio service stopped and started again
#                 (DOWN s away, default 1); the lab's devices and these are made again
#   pulselate     the same, a's chosen output (Q) made again only LATE s later
#                 (default 6): its playback on the default meanwhile (PulseAudio's
#                 FollowChosen moves it back)
#   movesink:<sink>  the server moves a's playback to that sink (pactl move-sink-input)
#   movesrc:<source> the server moves a's recording to that source (move-source-output)
#   jump:<ms>     the loop of a's output Q made again <ms> later: the echo's delay jumps
#                 with no device, stream or server changing (nothing tells WebRTC)
#   unplug        a's output Q taken away (its playback falls to the default output,
#                 looped back LOOP_DEFAULT ms later); plug: Q made again
#   E:<label>     an event measured from here (events.log)
# Recorded throughout (again after `pulse`, as numbered pieces): what b plays of a
# (played-b), a's microphone (mic-a). Measured, not judged: device_echo.py.
# Programs used so far: device-echo-cases.sh.
# Usage: device-echo.sh <name>  (BUNDLE, CALL_LAB_RUNS, LAB_DISPLAY, FAR_FILE, LOOP_Q,
# LOOP_O, LOOP_DEFAULT, CYCLES, CYCLE, SETTLE, PROG_EVEN, PROG_ODD, DOWN, START_OUTPUT,
# RIG_ENV)
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
now() { date +%s%N | cut -c1-13; }
if [ "${1:-}" = --inside ]; then
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  export LAB_NO_TONE="a b"
  piece=0
  record() {
    local name
    piece=$(now)
    for name in played-b:labb_quiet.monitor mic-a:laba_test.monitor; do
      now > "$RUN/${name%%:*}.$piece.start"
      parec --device="${name#*:}" --format=s16le --rate=48000 --channels=1 --raw \
        --latency-msec=20 > "$RUN/${name%%:*}.$piece.raw" 2> /dev/null &
      echo $! > "$RUN/rec-${name%%:*}.pid"
    done
  }
  loop() { # source sink ms
    [ "$3" = none ] && return 0
    pactl load-module module-loopback source="$1" sink="$2" latency_msec="$3" \
      source_dont_move=true sink_dont_move=true > /dev/null
  }
  extras() {
    pactl load-module module-null-sink sink_name=laba_other sink_properties=device.description=FogataOther-laba. > /dev/null
    pactl load-module module-remap-source source_name=laba_mic2 master=laba_test.monitor \
      source_properties=device.description=FogataMicTwo-laba. > /dev/null
    pactl load-module module-null-sink sink_name=lab_apps sink_properties=device.description=LabApps > /dev/null
    loop laba_quiet.monitor laba_test "${LOOP_Q:-60}"
    loop laba_other.monitor laba_test "${LOOP_O:-180}"
    loop lab_default.monitor laba_test "${LOOP_DEFAULT:-none}"
  }
  lab_start 2
  "$LAB_DIR/unmute-sound.sh" "$RUN/sound" >> "$RUN/lab.log" 2>&1
  extras
  echo "loops: Q ${LOOP_Q:-60} O ${LOOP_O:-180} default ${LOOP_DEFAULT:-none}" >> "$RUN/lab.log"
  mean=$(ffmpeg -hide_banner -nostats -i "$FAR_FILE" -af volumedetect -f null - 2>&1 |
    sed -n 's/.*mean_volume: \(-\{0,1\}[0-9.]*\) dB.*/\1/p')
  ffmpeg -hide_banner -loglevel error -y -i "$FAR_FILE" \
    -af "volume=$(python3 -I -c "print(round(-28 - ($mean), 2))")dB" -ar 48000 -ac 1 -sample_fmt s16 "$RUN/sound/far.wav"
  ( trap 'kill "$(cat "$RUN/pwcat-far.pid")" 2>/dev/null; exit 0' TERM HUP INT
    while true; do
      pw-cat --playback --target lab_apps -P "$LAB_APP_PROPS" "$RUN/sound/far.wav" 2>/dev/null &
      echo $! > "$RUN/pwcat-far.pid"
      wait $!
      sleep 0.2
    done ) > /dev/null 2>&1 &
  echo $! > "$RUN/far.pid"
  record
  read -ra extra <<< "${RIG_ENV:-}"
  common=(FOGATA_TEST_MIC_CASE=far-echo FOGATA_TEST_ECHO_WINDOW=screen FOGATA_WEBRTC_LOG=info "${extra[@]}")
  lab_copy a 2 1 "${common[@]}" FOGATA_TEST_OUTPUT_LATER="${START_OUTPUT:-FogataQuietOut-laba}"
  lab_copy b 2 off "${common[@]}" FOGATA_TEST_OUTPUT_LATER=FogataQuietOut-labb FOGATA_SCREEN_SOUND_FROM=pw-cat
  until [ -e "$M/mark-a-ready" ]; do
    kill -0 "$(cat "$RUN/a.pid")" 2>/dev/null || { echo "a ended before it was ready" >> "$RUN/lab.log"; lab_stop; exit 1; }
    sleep 0.2
  done
  touch "$M/rig-watch"
  sleep "${SETTLE:-20}"
  apid=$(cat "$RUN/a.pid")
  # a's own stream (playback or recording) in pactl's list, by its process.
  stream() { # sink-inputs|source-outputs
    pactl -f json list "$1" 2> /dev/null | python3 -I -c "
import json, sys
for s in json.load(sys.stdin):
    p = s.get('properties', {})
    if p.get('application.process.id') == '$apid' and 'loopback' not in p.get('media.name', '').lower():
        print(s['index']); break"
  }
  act() { # k action
    local k=$1 a=${2%%:*} arg=${2#*:} t
    t=$(now)
    case "$a" in
      out) echo "$arg" > "$M/rig-output" ;;
      in) echo "$arg" > "$M/rig-input" ;;
      mic) echo "$arg" > "$M/rig-mic" ;;
      words) paplay --device=laba_test "$RUN/sound/words.wav" 2> /dev/null &
        echo "$k $arg $t" >> "$RUN/words.log" ;;
      pulse)
        kill "$(cat "$RUN/rec-played-b.pid")" "$(cat "$RUN/rec-mic-a.pid")" 2> /dev/null
        lab_sound_stop pipewire-pulse
        sleep "${DOWN:-1}"
        lab_sound_start pipewire-pulse
        lab_devices 2
        extras
        record ;;
      pulselate)
        # The server back without a's devices for LATE s (default 6): a's playback and
        # recording start again on the default devices, and move to the chosen ones
        # once they are listed again (PulseAudio's FollowChosen). E:chosen marks when
        # a's output is back.
        kill "$(cat "$RUN/rec-played-b.pid")" "$(cat "$RUN/rec-mic-a.pid")" 2> /dev/null
        lab_sound_stop pipewire-pulse
        sleep "${DOWN:-1}"
        lab_sound_start pipewire-pulse
        # Every device made again at once but a's chosen output (Q), which comes LATE s
        # later: a's playback starts again on the default output (looped too).
        pw-metadata -n settings 0 clock.force-quantum 480 > /dev/null 2>&1
        pactl load-module module-null-sink sink_name=lab_default sink_properties=device.description=LabDefault > /dev/null
        pactl load-module module-remap-source source_name=lab_default_mic master=lab_default.monitor source_properties=device.description=LabDefaultMic > /dev/null
        for r in a b; do
          pactl load-module module-null-sink sink_name=lab${r}_test sink_properties=device.description=FogataTest-lab$r. > /dev/null
          lab_plug "$r"
          [ "$r" = a ] && continue
          pactl load-module module-null-sink sink_name=lab${r}_quiet sink_properties=device.description=FogataQuietOut-lab$r. > /dev/null
          pactl load-module module-remap-source source_name=lab${r}_quiet_mic master=lab${r}_quiet.monitor source_properties=device.description=FogataQuiet-lab$r. > /dev/null
        done
        pactl set-default-sink lab_default
        pactl set-default-source lab_default_mic
        LOOP_Q=none extras
        record
        ( sleep "${LATE:-6}"
          pactl load-module module-null-sink sink_name=laba_quiet sink_properties=device.description=FogataQuietOut-laba. > /dev/null
          echo "$k E:chosen $(now) $(now)" >> "$RUN/events.log"
          pactl load-module module-remap-source source_name=laba_quiet_mic master=laba_quiet.monitor source_properties=device.description=FogataQuiet-laba. > /dev/null
          loop laba_quiet.monitor laba_test "${LOOP_Q:-60}" ) &
        ;;
      movesink) pactl move-sink-input "$(stream sink-inputs)" "$arg" ;;
      movesrc) pactl move-source-output "$(stream source-outputs)" "$arg" ;;
      jump) for m in $(pactl list short modules | awk '/module-loopback/ && /source=laba_quiet.monitor/ {print $1}'); do
          pactl unload-module "$m"
        done
        loop laba_quiet.monitor laba_test "$arg" ;;
      unplug) pactl unload-module "$(pactl list short modules | awk '/sink_name=laba_quiet( |$)/ {print $1}')" ;;
      plug) pactl load-module module-null-sink sink_name=laba_quiet sink_properties=device.description=FogataQuietOut-laba. > /dev/null
        loop laba_quiet.monitor laba_test "${LOOP_Q:-60}" ;;
      E) ;;
    esac
    echo "$k $2 $t $(now)" >> "$RUN/events.log"
  }
  cycle=$(( ${CYCLE:-20} * 1000 ))
  t0=$(now)
  for k in $(seq 0 $(( ${CYCLES:-6} - 1 ))); do
    prog=${PROG_EVEN:?PROG_EVEN}
    [ $((k % 2)) = 1 ] && prog=${PROG_ODD:-$PROG_EVEN}
    for item in $prog; do
      at=$((t0 + k * cycle + $(python3 -I -c "print(int(float('${item%%:*}') * 1000))")))
      python3 -I -c "import time; time.sleep(max(0, $at / 1000 - time.time()))"
      act "$k" "${item#*:}"
    done
  done
  python3 -I -c "import time; time.sleep(max(0, ($t0 + ${CYCLES:-6} * $cycle) / 1000 - time.time()))"
  touch "$M/rig-end"
  lab_wait "a b" 120
  for f in far rec-played-b rec-mic-a; do
    [ -e "$RUN/$f.pid" ] && kill "$(cat "$RUN/$f.pid")" 2>/dev/null
  done
  sleep 1
  lab_stop
  exit 0
fi
export BUNDLE=${BUNDLE:?BUNDLE} FAR_FILE=${FAR_FILE:?FAR_FILE: a speech recording}
export RUN=${CALL_LAB_RUNS:-$HOME/call-lab}/device-${1:?name}-$(date +%m%d-%H%M%S)
mkdir -p "$RUN"
echo "device-echo $1, started $(date -Is); BUNDLE=$BUNDLE LOOP_Q=${LOOP_Q:-60} LOOP_O=${LOOP_O:-180} LOOP_DEFAULT=${LOOP_DEFAULT:-none} CYCLES=${CYCLES:-6} CYCLE=${CYCLE:-20} SETTLE=${SETTLE:-20} START_OUTPUT=${START_OUTPUT:-} PROG_EVEN=${PROG_EVEN:-} PROG_ODD=${PROG_ODD:-} DOWN=${DOWN:-1}" > "$RUN/settings.txt"
"$HERE/session.sh" "$RUN/session" "$0" --inside
echo "  $RUN"

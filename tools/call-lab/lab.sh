# Sourced by measure.sh and mic.sh inside session.sh: the virtual devices, the tones, the
# copies of the app's call test on a virtual display, and a log of what each copy records.
#   RUN     the run's folder (meet/, data/, logs)
#   BUNDLE  the call test build (flutter build linux --debug -t integration_test/call_test.dart)
#   LAB_DISPLAY  the Xvfb display (default 171), SCREEN its size (default 1600x900)

LAB_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
X=${LAB_DISPLAY:-171}
M=$RUN/meet
ROLES=(a b c d e f g h)

# Each copy has devices of its own, which it picks by their names' end
# (FOGATA_TEST_DEVICES=-lab<role>.): an output lab<role>_test whose monitor is its microphone
# FogataMic, carrying a tone of its own pitch (with one tone for all, each copy's echo
# canceller would take the others' voices for its own echo and send almost nothing), and a
# silent output and input. The session's default input is silent too, so a copy whose
# microphone goes away records silence.
lab_devices() { # copies
  local i r
  pactl load-module module-null-sink sink_name=lab_default sink_properties=device.description=LabDefault > /dev/null
  pactl load-module module-remap-source source_name=lab_default_mic master=lab_default.monitor source_properties=device.description=LabDefaultMic > /dev/null
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    pactl load-module module-null-sink sink_name=lab${r}_test sink_properties=device.description=FogataTest-lab$r. > /dev/null
    lab_plug "$r"
    pactl load-module module-null-sink sink_name=lab${r}_quiet sink_properties=device.description=FogataQuietOut-lab$r. > /dev/null
    pactl load-module module-remap-source source_name=lab${r}_quiet_mic master=lab${r}_quiet.monitor source_properties=device.description=FogataQuiet-lab$r. > /dev/null
  done
  pactl set-default-sink lab_default
  pactl set-default-source lab_default_mic
}

# A copy's microphone with its tone, made again after lab_unplug under the same name.
lab_plug() { # role
  pactl load-module module-remap-source source_name=lab$1_mic master=lab$1_test.monitor source_properties=device.description=FogataMic-lab$1. > "$RUN/mod-mic-$1"
}

lab_unplug() { # role
  pactl unload-module "$(cat "$RUN/mod-mic-$1")"
}

# Plays each copy's tone into its output, again and again, until lab_stop.
lab_tones() { # copies
  local i r
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    python3 -I "$LAB_DIR/tone.py" "$RUN/tone-$r.wav" $((330 + 55 * i))
    ( trap 'kill "$(cat "$RUN/paplay-$r.pid")" 2>/dev/null; exit 0' TERM HUP INT
      while true; do
        paplay --device="lab${r}_test" "$RUN/tone-$r.wav" 2>/dev/null &
        echo $! > "$RUN/paplay-$r.pid"
        wait $!
      done ) > /dev/null 2>&1 &
    echo $! > "$RUN/tone-$r.pid"
  done
}

# Every 0.2 s, what each process records from: "<time> <pid>:<source name> ..." whenever it
# changes, to recordings.log.
lab_recorder() {
  (
    last=""
    while true; do
      now=$(python3 -I "$LAB_DIR/recording.py" 2> /dev/null)
      if [ "$now" != "$last" ]; then
        echo "$(date +%s.%N | cut -c1-14) $now" >> "$RUN/recordings.log"
        last=$now
      fi
      sleep 0.2
    done
  ) &
  echo $! > "$RUN/recorder.pid"
}

lab_start() { # copies
  mkdir -p "$M" "$RUN/data"
  : > "$RUN/recordings.log"
  lab_devices "$1"
  Xvfb ":$X" -screen 0 "${SCREEN:-1600x900}x24" -nolisten tcp > "$RUN/xvfb.log" 2>&1 &
  echo $! > "$RUN/xvfb.pid"
  for _ in $(seq 50); do xdpyinfo -display ":$X" > /dev/null 2>&1 && break; sleep 0.1; done
  lab_tones "$1"
  lab_recorder
}

# A sound server that takes connections and never answers, at $RUN/hang.sock.
lab_hanging_server() {
  python3 -I "$LAB_DIR/hang.py" "$RUN/hang.sock" &
  echo $! > "$RUN/hang.pid"
  for _ in $(seq 50); do [ -S "$RUN/hang.sock" ] && break; sleep 0.1; done
}

# Starts one copy: lab_copy <role> <copies> <mic> [VAR=value...]; mic is 1 (its tone), off
# (enters muted), broken (no sound server at all) or hang (a server that never answers).
lab_copy() {
  local role=$1 copies=$2 mic=$3
  shift 3
  local sound=(PULSE_SINK="lab${role}_quiet" PIPEWIRE_NODE="lab${role}_quiet")
  case "$mic" in
    broken) sound=(PULSE_SERVER=unix:/nonexistent PIPEWIRE_REMOTE=/nonexistent XDG_RUNTIME_DIR="$RUN/data/$role") ;;
    hang) sound=(PULSE_SERVER="unix:$RUN/hang.sock" PIPEWIRE_REMOTE=/nonexistent XDG_RUNTIME_DIR="$RUN/data/$role"); mic=broken ;;
  esac
  mkdir -p "$RUN/data/$role"
  env -u WAYLAND_DISPLAY DISPLAY=":$X" XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 LP_NUM_THREADS=2 \
    "${sound[@]}" FOGATA_CALL_LOG=1 FOGATA_ROLE="$role" FOGATA_CALL="$M" \
    FOGATA_DATA_DIR="$RUN/data/$role" FOGATA_DOWNLOADS="$RUN/data/$role" \
    FOGATA_TEST_MIC="$mic" FOGATA_TEST_DEVICES="-lab$role." FOGATA_PEERS="$copies" \
    FOGATA_SCREEN_SOUND_FROM=paplay RUST_LOG="${RUST_LOG:-warn}" "$@" \
    "$BUNDLE" > "$RUN/$role.out" 2>&1 &
  echo $! > "$RUN/$role.pid"
}

# Waits until each of the roles wrote its result (or its process ended); at most $2 s. One
# that failed ends the wait at once: the others would only wait for it to the end.
lab_wait() { # "a b c" seconds
  local r end=$((SECONDS + $2))
  for r in $1; do
    while [ ! -s "$M/$r-result.txt" ] && kill -0 "$(cat "$RUN/$r.pid")" 2>/dev/null; do
      [ $SECONDS -gt $end ] && { echo "$r: no result in time" >> "$RUN/lab.log"; return 1; }
      if grep -lq '^failed' "$M"/?-result.txt 2> /dev/null; then
        echo "a copy failed: $(grep -l '^failed' "$M"/?-result.txt | tr '\n' ' ')" >> "$RUN/lab.log"
        return 1
      fi
      sleep 1
    done
  done
}

# Stops a process this lab started, by the PID it wrote down, once its command line still
# holds [word] (a PID can be reused once its process ends).
lab_kill() { # pid word
  local p=$1
  [ -n "$p" ] && [ -r "/proc/$p/cmdline" ] || return 0
  tr '\0' ' ' < "/proc/$p/cmdline" | grep -q -- "$2" || return 0
  kill "$p" 2>/dev/null || return 0
  for _ in $(seq 50); do kill -0 "$p" 2>/dev/null || return 0; sleep 0.1; done
  kill -9 "$p" 2>/dev/null
}

lab_stop() {
  local f
  for f in "$RUN"/[a-h].pid; do [ -e "$f" ] && lab_kill "$(cat "$f")" "$BUNDLE"; done
  for f in "$RUN"/tone-?.pid "$RUN/recorder.pid"; do [ -e "$f" ] && lab_kill "$(cat "$f")" bash; done
  [ -e "$RUN/hang.pid" ] && lab_kill "$(cat "$RUN/hang.pid")" hang.py
  [ -e "$RUN/xvfb.pid" ] && lab_kill "$(cat "$RUN/xvfb.pid")" Xvfb
}

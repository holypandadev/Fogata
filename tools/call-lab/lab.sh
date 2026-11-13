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
# microphone goes away records silence. With LAB_LATE_DEVICES=<role>:<seconds>, that copy's
# devices are made that long after the others' (a desktop's sound card coming back after
# its sound server, mic.sh restart).
lab_devices() { # copies
  local i r late
  pactl load-module module-null-sink sink_name=lab_default sink_properties=device.description=LabDefault > /dev/null
  pactl load-module module-remap-source source_name=lab_default_mic master=lab_default.monitor source_properties=device.description=LabDefaultMic > /dev/null
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    late=${LAB_LATE_DEVICES:-}
    [ -n "$late" ] && [ "${late%%:*}" = "$r" ] && sleep "${late#*:}"
    pactl load-module module-null-sink sink_name=lab${r}_test sink_properties=device.description=FogataTest-lab$r. > /dev/null
    # Made again (mic.sh restart): its tone's player, started while the output was missing,
    # may play into the default one; started again, it plays into this one.
    [ -e "$RUN/paplay-$r.pid" ] && kill "$(cat "$RUN/paplay-$r.pid")" 2>/dev/null
    lab_plug "$r"
    pactl load-module module-null-sink sink_name=lab${r}_quiet sink_properties=device.description=FogataQuietOut-lab$r. > /dev/null
    pactl load-module module-remap-source source_name=lab${r}_quiet_mic master=lab${r}_quiet.monitor source_properties=device.description=FogataQuiet-lab$r. > /dev/null
  done
  # CALL_LAB_LOOPBACK=1: a loopback carrying what copy a plays into the default output,
  # and a combined output, both made by pipewire-pulse as on a desktop with such a setup.
  # A shared screen's sound must take neither (they pass on sound from elsewhere, maybe
  # the call itself): with FOGATA_SCREEN_SOUND_FROM= in MEASURE_ENV (so that more than
  # paplay is taken), a's log must say "passed on, not linked" for their nodes and never
  # "linking node ... binary=pipewire-pulse".
  if [ "${CALL_LAB_LOOPBACK:-}" = 1 ]; then
    pactl load-module module-loopback source=laba_quiet.monitor sink=lab_default latency_msec=60 > /dev/null
    pactl load-module module-combine-sink sink_name=lab_combined slaves=lab_default sink_properties=device.description=LabCombined > /dev/null
  fi
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

# Plays each copy's tone into its output, again and again, until lab_stop. With
# LAB_VOICE_BINARY (the app's executable), the players say they are the app: a voice is
# a person's, never another application's, and a shared screen taking every other
# application's sound (FOGATA_SCREEN_SOUND_FROM empty) leaves the app's own out.
lab_tones() { # copies
  local i r
  local as=()
  [ -n "${LAB_VOICE_BINARY:-}" ] && as=("PULSE_PROP_OVERRIDE_application.process.binary=$LAB_VOICE_BINARY")
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    python3 -I "$LAB_DIR/tone.py" "$RUN/tone-$r.wav" $((330 + 55 * i))
    # A player that fails (no sound server, or its output not made again yet after
    # lab_sound_start) is started again a moment later.
    ( trap 'kill "$(cat "$RUN/paplay-$r.pid")" 2>/dev/null; exit 0' TERM HUP INT
      while true; do
        env "${as[@]}" paplay --device="lab${r}_test" "$RUN/tone-$r.wav" 2>/dev/null &
        echo $! > "$RUN/paplay-$r.pid"
        wait $! || sleep 0.2
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

# The session's own sound daemons (session.sh: pipewire, wireplumber, pipewire-pulse),
# stopped and started again as `systemctl --user restart` does on a desktop.
# lab_sound_stop stops the ones named, in that order, each gone before the next;
# lab_sound_start starts them in that order, each taking connections before the next
# (pipewire its socket, pipewire-pulse `pactl info`; their logs go on in the session's
# folder). The lab's devices were pipewire-pulse's modules: lab_devices makes them again.
lab_sound_stop() { # daemon...
  local d
  for d in "$@"; do
    [ -e "$CALL_LAB_SESSION/$d.pid" ] || continue
    lab_kill "$(cat "$CALL_LAB_SESSION/$d.pid")" "$d"
    rm -f "$CALL_LAB_SESSION/$d.pid"
    # Left by a pipewire killed (lab_kill's last resort), its socket would pass for the
    # new one's before that one takes connections.
    [ "$d" = pipewire ] && rm -f "$XDG_RUNTIME_DIR/pipewire-0" "$XDG_RUNTIME_DIR/pipewire-0-manager"
    echo "$(date +%s.%N | cut -c1-14) stopped $d" >> "$RUN/lab.log"
  done
}

lab_sound_start() { # daemon...
  local d
  for d in "$@"; do
    "$d" >> "$CALL_LAB_SESSION/$d.log" 2>&1 &
    echo $! > "$CALL_LAB_SESSION/$d.pid"
    case "$d" in
      pipewire)
        for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done ;;
      pipewire-pulse)
        for _ in $(seq 100); do pactl info > /dev/null 2>&1 && break; sleep 0.1; done ;;
    esac
    echo "$(date +%s.%N | cut -c1-14) started $d" >> "$RUN/lab.log"
  done
}

# A sound server that takes connections and never answers, at $RUN/hang.sock.
lab_hanging_server() {
  python3 -I "$LAB_DIR/hang.py" "$RUN/hang.sock" &
  echo $! > "$RUN/hang.pid"
  for _ in $(seq 50); do [ -S "$RUN/hang.sock" ] && break; sleep 0.1; done
}

# Starts one copy: lab_copy <role> <copies> <mic> [VAR=value...]; mic is 1 (its tone), off
# (enters muted), broken (no sound server at all) or hang (a server that never answers).
# A shared screen takes the sound of pacat alone (the tones' player: paplay is pacat under
# another name, and PipeWire names a program by its executable); VAR=value after it wins.
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
    FOGATA_SCREEN_SOUND_FROM=pacat RUST_LOG="${RUST_LOG:-warn}" "$@" \
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

# Another application's sound, for a shared screen to take: a tone of its own (880 Hz:
# neither a copy's voice nor a harmonic of one, all on a 55 Hz grid of odd and even steps
# that no sum or difference of 880 and 330 reaches) from pw-cat, again and again, into a
# sink of its own that nothing records. Its node names its program itself (LAB_APP_PROPS),
# whatever PipeWire would add.
LAB_APP_PROPS='{ application.process.binary = pw-cat }'

lab_apps_tone() {
  pactl load-module module-null-sink sink_name=lab_apps sink_properties=device.description=LabApps > /dev/null
  python3 -I "$LAB_DIR/tone.py" "$RUN/tone-apps.wav" 880 60
  ( trap 'kill "$(cat "$RUN/pwcat-apps.pid")" 2>/dev/null; exit 0' TERM HUP INT
    while true; do
      pw-cat --playback --target lab_apps -P "$LAB_APP_PROPS" "$RUN/tone-apps.wav" 2>/dev/null &
      echo $! > "$RUN/pwcat-apps.pid"
      wait $!
    done ) > /dev/null 2>&1 &
  echo $! > "$RUN/tone-apps.pid"
}

# A deliberate echo, to see a check catch one: what copy a plays (the others' voices) is
# played again by another application (pw-cat, into lab_apps), as a program that records
# the speakers and plays them on would; a's shared screen then carries it to everyone.
lab_echo_proof() {
  ( parec --device=laba_quiet.monitor --format=s16le --rate=48000 --channels=1 --raw 2>/dev/null |
      pw-cat --playback --target lab_apps -P "$LAB_APP_PROPS" --raw --format s16 --rate 48000 \
        --channels 1 - 2>/dev/null ) &
  echo $! > "$RUN/echo-proof.pid"
}

# Records what each copy of $2 plays (its silent output's monitor, 16-bit mono at 16 kHz)
# for every stretch copy $1 marks (mark-<$1>-<stretch>-start and -end, call_test.dart's
# _mark), to $RUN/played/<stretch>-<role>.raw; and what plays into the server when the
# first stretch starts, to $RUN/sink-inputs.json.
lab_phase_recorder() { # marking role, "roles recorded"
  local marker=$1 roles=$2
  mkdir -p "$RUN/played"
  (
    declare -A started
    while true; do
      for f in "$M"/mark-"$marker"-*-start; do
        [ -e "$f" ] || continue
        stretch=${f#"$M/mark-$marker-"}
        stretch=${stretch%-start}
        [ -n "${started[$stretch]:-}" ] && continue
        started[$stretch]=1
        [ -e "$RUN/sink-inputs.json" ] || pactl -f json list sink-inputs > "$RUN/sink-inputs.json" 2> /dev/null
        pids=""
        for r in $roles; do
          parec --device="lab${r}_quiet.monitor" --format=s16le --rate=16000 --channels=1 \
            --raw --latency-msec=50 > "$RUN/played/$stretch-$r.raw" 2> /dev/null &
          pids="$pids $!"
        done
        echo "$pids" > "$RUN/played/$stretch.pids"
      done
      for f in "$RUN"/played/*.pids; do
        [ -e "$f" ] || continue
        stretch=$(basename "$f" .pids)
        if [ -e "$M/mark-$marker-$stretch-end" ]; then
          # shellcheck disable=SC2046
          kill $(cat "$f") 2> /dev/null
          mv "$f" "$f.done"
        fi
      done
      sleep 0.1
    done
  ) &
  echo $! > "$RUN/phases.pid"
}

lab_stop() {
  local f
  for f in "$RUN"/[a-h].pid; do [ -e "$f" ] && lab_kill "$(cat "$f")" "$BUNDLE"; done
  for f in "$RUN"/tone-?.pid "$RUN/tone-apps.pid" "$RUN/recorder.pid" "$RUN/phases.pid" \
    "$RUN/echo-proof.pid"; do [ -e "$f" ] && lab_kill "$(cat "$f")" bash; done
  for f in "$RUN"/played/*.pids; do
    # shellcheck disable=SC2046
    [ -e "$f" ] && kill $(cat "$f") 2> /dev/null
  done
  [ -e "$RUN/hang.pid" ] && lab_kill "$(cat "$RUN/hang.pid")" hang.py
  [ -e "$RUN/xvfb.pid" ] && lab_kill "$(cat "$RUN/xvfb.pid")" Xvfb
}

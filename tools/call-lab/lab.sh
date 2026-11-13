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
# LAB_TURNS=1 (two copies, mic.sh echo): voices that talk in turns and then hold a steady
# tone alone (tone.py turns), every player starting on the same 16 s of the clock; each
# copy's slot and steady tone are written to $RUN/turns.txt for mic_check.py.
# LAB_FAN=<role> (mic.sh noise): that copy's microphone has a fan's noise (tone.py fan), no
# tone.
LAB_TURN_STEADY=(1265 1650)

lab_tones() { # copies
  local i r turns=""
  local as=()
  [ -n "${LAB_VOICE_BINARY:-}" ] && as=("PULSE_PROP_OVERRIDE_application.process.binary=$LAB_VOICE_BINARY")
  [ "${LAB_TURNS:-}" = 1 ] && turns=1
  [ -n "$turns" ] && : > "$RUN/turns.txt"
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    if [ -n "$turns" ]; then
      python3 -I "$LAB_DIR/tone.py" "$RUN/tone-$r.wav" turns "$i" "${LAB_TURN_STEADY[$i]}"
      echo "$r slot=$i steady=${LAB_TURN_STEADY[$i]}" >> "$RUN/turns.txt"
    elif [ "${LAB_FAN:-}" = "$r" ]; then
      # A fan's noise instead of the copy's tone (mic.sh noise).
      python3 -I "$LAB_DIR/tone.py" "$RUN/tone-$r.wav" fan
    else
      python3 -I "$LAB_DIR/tone.py" "$RUN/tone-$r.wav" $((330 + 55 * i))
    fi
    # A player that fails (no sound server, or its output not made again yet after
    # lab_sound_start) is started again a moment later.
    # With turns, a player starts on every period's start whether the last one has ended
    # or not (one still draining is stopped): waiting for it to end made a player that
    # ended a moment after the period's start wait for the next one, and that copy said
    # nothing for a whole period.
    # The wait for the next period runs in the background: a trap waits for a command in
    # the foreground to end, and lab_stop's kill -9 came first (bash told "Killed").
    ( waiter=""
      trap 'kill "$(cat "$RUN/paplay-$r.pid")" $waiter 2>/dev/null; exit 0' TERM HUP INT
      while true; do
        if [ -n "$turns" ]; then
          python3 -I "$LAB_DIR/tone.py" wait &
          waiter=$!
          wait $waiter
          waiter=""
          [ -e "$RUN/paplay-$r.pid" ] && kill "$(cat "$RUN/paplay-$r.pid")" 2>/dev/null
        fi
        env "${as[@]}" paplay --device="lab${r}_test" "$RUN/tone-$r.wav" 2>/dev/null &
        echo $! > "$RUN/paplay-$r.pid"
        [ -n "$turns" ] || wait $! || sleep 0.2
      done ) > /dev/null 2>&1 &
    echo $! > "$RUN/tone-$r.pid"
  done
}

# A room's echo, as with speakers and a microphone: what each copy plays goes back into its
# own microphone (its output's monitor looped into the output its microphone hears), 30 ms
# later and as loud, while the other copy talks (tone.py room mutes each loop in the
# copy's own turns, on the turns' clock). The echo reaches the audio processing as the playout it was given plus
# the server's buffers, the loop's 30 ms and the recording's: well under the half second
# or so of delay WebRTC's echo canceller (AEC3: 5 matched filters over the signal at
# 4 kHz) looks for; and what a copy plays (the other's voice after its gain control,
# -5 dBFS at most) comes back short of clipping, in the other's turns only. With the echo canceller working, the other copy hears its own
# voice back far below its playing of it; without one, about as loud.
lab_room_echo() { # copies
  local i r
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    pactl load-module module-loopback source="lab${r}_quiet.monitor" sink="lab${r}_test" \
      latency_msec=30 source_dont_move=true sink_dont_move=true > "$RUN/mod-room-$r"
  done
  python3 -I "$LAB_DIR/tone.py" room "$(cat "$RUN/mod-room-a")" "$(cat "$RUN/mod-room-b")" \
    > "$RUN/room.log" 2>&1 &
  echo $! > "$RUN/room.pid"
}

# Once a second, PipeWire's graph (pw-top: each node's block size, rate and xruns) and
# every stream's buffer and device latency, to $RUN/timing.log (timing.py).
# And each copy's output and microphone side by side in one stereo recording per stretch
# (lab_phase_recorder): its output's monitor linked to the left of a sink of its own,
# its microphone's to the right, in the same cycle of the graph, so that mic_check.py
# times the room's loop as it really was (two recordings, one of each, slid against
# each other by tens of ms under load).
lab_timing() {
  local r
  python3 -I "$LAB_DIR/timing.py" "$RUN/timing.log" > /dev/null 2>&1 &
  echo $! > "$RUN/timing.pid"
  for r in a b; do
    pactl load-module module-null-sink sink_name="lab${r}_probe" channels=2 \
      channel_map=front-left,front-right sink_properties=device.description=LabProbe-$r \
      > /dev/null
    lab_link "lab${r}_quiet" "lab${r}_probe:playback_FL"
    lab_link "lab${r}_test" "lab${r}_probe:playback_FR"
  done
  : > "$RUN/probe"
}

# Links the first monitor port of sink $1 to port $2, as PipeWire lists them (a's output's
# monitor was not found by its expected name once in every run), trying for 5 s; what was
# listed goes to lab.log when it cannot.
lab_link() { # sink, input port
  local port _
  for _ in $(seq 20); do
    port=$(pw-link -o 2> /dev/null | grep -m 1 "^$1:monitor_")
    if [ -n "$port" ] && [ -n "$(pw-link -i 2> /dev/null | grep -x "$2")" ] &&
      pw-link "$port" "$2" 2>> "$RUN/lab.log"; then
      echo "linked $port to $2" >> "$RUN/lab.log"
      return 0
    fi
    sleep 0.25
  done
  { echo "could not link $1's monitor to $2; outputs and inputs:"
    pw-link -o; pw-link -i; } >> "$RUN/lab.log" 2>&1
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

# Another application's sound, for a shared screen to take: noise from 5.3 to 6.5 kHz, above
# the copies' voices (tone.py screen), from pw-cat, again and again, into a sink of its own
# that nothing records. Its node names its program itself (LAB_APP_PROPS),
# whatever PipeWire would add.
LAB_APP_PROPS='{ application.process.binary = pw-cat }'

lab_apps_tone() {
  pactl load-module module-null-sink sink_name=lab_apps sink_properties=device.description=LabApps > /dev/null
  python3 -I "$LAB_DIR/tone.py" "$RUN/tone-apps.wav" screen
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
#
# With their own buffers (parec's default is about 2 s), b got its voice back 2 s later, in
# a's next turn, where mic_check.py does not look for it, and its steady tone while it still
# held it (b's echo canceller then took the tone for an echo of what it played and removed
# it: -42 to -57 dB). With 20 ms each, the voice comes back within the round trip of the
# call (about half a second), inside the copy's own turn, as an echo would.
lab_echo_proof() {
  ( parec --device=laba_quiet.monitor --format=s16le --rate=48000 --channels=1 --raw \
      --latency-msec=20 2>/dev/null |
      pw-cat --playback --target lab_apps -P "$LAB_APP_PROPS" --raw --format s16 --rate 48000 \
        --channels 1 --latency=20ms - 2>/dev/null ) &
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
          # When the recording starts, by the clock the turns follow (lab_tones).
          date +%s.%N > "$RUN/played/$stretch-$r.start"
          parec --device="lab${r}_quiet.monitor" --format=s16le --rate=16000 --channels=1 \
            --raw --latency-msec=50 > "$RUN/played/$stretch-$r.raw" 2> /dev/null &
          pids="$pids $!"
          # And what its microphone hears, to time the room's echo against what it played
          # (mic_check.py: whether the loop's delay moves).
          parec --device="lab${r}_test.monitor" --format=s16le --rate=16000 --channels=1 \
            --raw --latency-msec=50 > "$RUN/played/$stretch-$r-mic.raw" 2> /dev/null &
          pids="$pids $!"
          if [ -e "$RUN/probe" ]; then
            parec --device="lab${r}_probe.monitor" --format=s16le --rate=16000 --channels=2 \
              --raw --latency-msec=50 > "$RUN/played/$stretch-$r-probe.raw" 2> /dev/null &
            pids="$pids $!"
          fi
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
  [ -e "$RUN/room.pid" ] && lab_kill "$(cat "$RUN/room.pid")" tone.py
  [ -e "$RUN/timing.pid" ] && lab_kill "$(cat "$RUN/timing.pid")" timing.py
  [ -e "$RUN/xvfb.pid" ] && lab_kill "$(cat "$RUN/xvfb.pid")" Xvfb
}

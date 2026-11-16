# Sourced by measure.sh and mic.sh inside session.sh: the virtual devices, the tones, the
# copies of the app's call test on a virtual display, and a log of what each copy records.
#   RUN     the run's folder (meet/, data/, logs)
#   BUNDLE  the call test build (flutter build linux --debug -t integration_test/call_test.dart)
#   LAB_DISPLAY  the Xvfb display (default 171), SCREEN its size (default 1600x900)
#   LAB_MOVING   video or desktop: something moving over the whole display for as long as
#                the copies run (lab_moving), for a shared screen that changes as a real
#                one does; off (the default) leaves the copies' own windows, which hardly
#                move

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
#
# The session's graph runs at WebRTC's own 10 ms (480 samples at 48 kHz) from the start, as it
# does anyway once any copy records, so that a recording stopped and started again at once
# does not change it. On PipeWire 1.0 (Ubuntu 24.04, GitHub's computers), when a recording
# of 10 ms from a remapped source stopped and another started at once, the source gave
# silence to every recording of it until none was left, 9 times in 12 (parec alone, no
# application); with the quantum fixed, never in 12. A copy that selected its microphone
# while its voice engine was recording (mic.sh late: WebRTC stops the recording and starts
# it again on the same input) sent silence for the rest of the call; the application no
# longer selects again the input it records from already, while nothing changed since.
# Set again with the devices, after mic.sh restart started the sound server again.
# LAB_FIXED_QUANTUM=0 leaves the quantum to PipeWire, as on a person's computer, where a
# recording stopped and started again at once can still give silence (to see whether the
# application does that).
lab_devices() { # copies
  local i r late
  if [ "${LAB_FIXED_QUANTUM:-1}" = 0 ]; then
    echo "the graph's quantum left to PipeWire (LAB_FIXED_QUANTUM=0)" >> "$RUN/lab.log"
  else
    pw-metadata -n settings 0 clock.force-quantum 480 > /dev/null 2>&1 ||
      echo "the graph's quantum could not be fixed (pw-metadata)" >> "$RUN/lab.log"
  fi
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
# tone. LAB_NO_TONE="<role>..." (mic.sh unmute): those copies' microphones get nothing from
# here; the check plays its own sound into them.
LAB_TURN_STEADY=(1265 1650)

lab_tones() { # copies
  local i r turns=""
  local as=()
  [ -n "${LAB_VOICE_BINARY:-}" ] && as=("PULSE_PROP_OVERRIDE_application.process.binary=$LAB_VOICE_BINARY")
  [ "${LAB_TURNS:-}" = 1 ] && turns=1
  [ -n "$turns" ] && : > "$RUN/turns.txt"
  for i in $(seq 0 $(($1 - 1))); do
    r=${ROLES[$i]}
    [[ " ${LAB_NO_TONE:-} " == *" $r "* ]] && continue
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

# LAB_MOVING: something that moves on the display, kept above the copies' windows (there is
# no window manager to raise anything) by xdotool every 2 s and started again if it ends:
#   video    ffmpeg's testsrc2 pattern panning across the whole display at 30 frames a
#            second (about 2.5 pixels a frame each way) under a coarse grain that changes in
#            every frame, as a grainy film does: harder to encode than most films. Drawn by
#            ffplay with SDL's software renderer, in RGB (from YUV it took twice the
#            processor); LAB_MOVING_GRAPH: another filtergraph for it, of the display's size.
#            On one of GitHub's computers (4 threads) it took 0.6 of a core at 1920x1080
#            and 1.1 at 2560x1440 alone, 0.7 to 1.1 at 1920x1080 beside the copies
#   desktop  a window of text (docs/architecture's notes on voice and video) on a still
#            desktop, scrolling 15 lines a second (about 250 pixels), as a page read and
#            scrolled: Tk's text window (scroll.py, python3-tk), which moves what is drawn
#            already and draws only the new line; the rest of the screen stays still. Under
#            1 % of a core, the display server's share included
# Every 10 s moving.log has the processor time of what draws it and of the display server
# (Xvfb, which also serves the copies and the capture) over those 10 s, in % of one core.
lab_moving() {
  local kind=${LAB_MOVING:-off} size=${SCREEN:-1600x900} graph needs
  [ "$kind" = off ] && return 0
  local w=${size%x*} h=${size#*x}
  case "$kind" in
    video)
      needs=ffplay
      graph=${LAB_MOVING_GRAPH:-testsrc2=size=$size:rate=30,scroll=horizontal=0.0013:vertical=0.0021,noise=alls=20:allf=t,format=bgra}
      echo "$graph" > "$RUN/moving-graph.txt"
      lab_moving_start() {
        env -u WAYLAND_DISPLAY DISPLAY=":$X" SDL_VIDEODRIVER=x11 SDL_RENDER_DRIVER=software \
          ffplay -hide_banner -loglevel error -nostats -an -sn -noborder -left 0 -top 0 \
          -x "$w" -y "$h" -window_title fogata-lab-moving -f lavfi "$graph" >> "$RUN/moving.log" 2>&1 &
      }
      ;;
    desktop)
      needs=python3
      python3 -I -c 'import tkinter' 2> /dev/null || needs=python3-tk
      lab_moving_start() {
        env -u WAYLAND_DISPLAY DISPLAY=":$X" python3 -I "$LAB_DIR/scroll.py" \
          "$LAB_DIR/../../docs/architecture/4-voice-and-video.md" \
          "$((w * 3 / 5))x$((h * 3 / 4))+$((w / 5))+$((h / 8))" >> "$RUN/moving.log" 2>&1 &
      }
      ;;
    *)
      echo "no LAB_MOVING $kind (video or desktop)" | tee -a "$RUN/lab.log" >&2
      return 1
      ;;
  esac
  command -v "$needs" > /dev/null && command -v xdotool > /dev/null || {
    echo "LAB_MOVING=$kind needs $needs and xdotool" | tee -a "$RUN/lab.log" >&2
    return 1
  }
  (
    trap 'kill "$(cat "$RUN/moving-player.pid" 2> /dev/null)" 2> /dev/null; exit 0' TERM HUP INT
    tick=$(getconf CLK_TCK)
    # The processor ticks of a process and its children.
    ticks() {
      local pid t=0 s
      for pid in "$1" $(cat "/proc/$1/task/$1/children" 2> /dev/null); do
        s=$(awk '{print $14 + $15}' "/proc/$pid/stat" 2> /dev/null) && t=$((t + ${s:-0}))
      done
      echo $t
    }
    last="" display="" at=0 n=0
    while true; do
      p=$(cat "$RUN/moving-player.pid" 2> /dev/null)
      if [ -z "$p" ] || ! kill -0 "$p" 2> /dev/null; then
        lab_moving_start
        p=$!
        echo $p > "$RUN/moving-player.pid"
        echo "$(date +%s) $kind started" >> "$RUN/moving.log"
        last=""
      fi
      DISPLAY=":$X" xdotool search --name '^fogata-lab-moving$' windowraise > /dev/null 2>&1
      if [ $((n % 5)) = 0 ]; then
        now=$(ticks "$p") xvfb=$(ticks "$(cat "$RUN/xvfb.pid")") t=$(date +%s)
        if [ -n "$last" ] && [ "$t" -gt "$at" ]; then
          echo "$t $kind $(((now - last) * 100 / tick / (t - at))) % of a core," \
            "display $(((xvfb - display) * 100 / tick / (t - at))) %" >> "$RUN/moving.log"
        fi
        last=$now display=$xvfb at=$t
      fi
      n=$((n + 1))
      sleep 2
    done
  ) &
  echo $! > "$RUN/moving.pid"
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
    DBUS_SESSION_BUS_ADDRESS=${CALL_LAB_DAEMON_BUS:-disabled:} "$d" >> "$CALL_LAB_SESSION/$d.log" 2>&1 &
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

# The ports copy $1 listens on for other devices (QUIC and TCP, on every address; not mDNS's
# 5353), comma separated, to $RUN/upload-ports once found, for measure.sh's LAB_UPLOAD: what
# leaves from them is what the copy sends to the others. Its node listens on one port for
# both (fogata-net's node.rs), and its TURN endpoint on 127.0.0.1 only. Waits up to 120 s.
lab_upload_ports() { # role
  local p udp tcp ports
  p=$(cat "$RUN/$1.pid")
  # The ports a socket of that process holds on every address ($1: ss's options).
  wildcard() {
    ss -H"$1"np 2> /dev/null | awk -v pid="pid=$p," '
      index($0, pid) { n = split($4, a, ":"); port = a[n]
        if ($4 ~ /^(0\.0\.0\.0|\*|\[::\]):/ && port != 5353) print port }' | sort -u
  }
  for _ in $(seq 240); do
    udp=$(wildcard ul) tcp=$(wildcard tl)
    ports=$(comm -12 <(echo "$udp") <(echo "$tcp") | grep . | paste -sd,)
    if [ -n "$ports" ]; then
      echo "$ports" > "$RUN/upload-ports"
      echo "copy $1's ports: udp $(echo $udp) tcp $(echo $tcp); upload from $ports" >> "$RUN/lab.log"
      return 0
    fi
    sleep 0.5
  done
  echo "copy $1 listens on no port of its own (LAB_UPLOAD)" | tee -a "$RUN/lab.log" >&2
  return 1
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

# The sound server missing its deadlines, on purpose (mic.sh echo, ECHO_STALL=<ms>,<ms>...):
# once both copies are in the call, every 6 to 9 s, the session's PipeWire is stopped for
# the next of those times and continued, each logged as "<time> <ms>" to $RUN/stalls.log.
# Around such a stall the sound server leaves out some of what a copy played or recorded,
# and its echo comes earlier than its echo canceller has it, as on a busy computer (the
# lab's PipeWire has no realtime scheduling: the session has no system bus for RTKit).
lab_stalls() { # "ms,ms,..."
  local list=$1
  (
    IFS=, read -ra stalls <<< "$list"
    i=0
    trap 'kill -CONT "$(cat "$CALL_LAB_SESSION/pipewire.pid")" 2>/dev/null; exit 0' TERM HUP INT
    until [ -e "$M/level-a.txt" ] && [ -e "$M/level-b.txt" ]; do sleep 1; done
    while true; do
      sleep $((6 + RANDOM % 4))
      ms=${stalls[$((i % ${#stalls[@]}))]}
      i=$((i + 1))
      pid=$(cat "$CALL_LAB_SESSION/pipewire.pid" 2> /dev/null) || continue
      echo "$(date +%s.%N | cut -c1-14) $ms" >> "$RUN/stalls.log"
      kill -STOP "$pid" 2> /dev/null || continue
      sleep "$(awk -v ms="$ms" 'BEGIN { print ms / 1000 }')"
      kill -CONT "$pid"
    done
  ) &
  echo $! > "$RUN/stalls.pid"
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

# mic.sh unmute: a room's noise into copy a's microphone without a break (ffmpeg in real
# time, unmute-sound.sh, into pacat through a pipe), a few words 0.5, 3.5 and 6.5 s after
# each `mark-a-unmute-<n>` (paplay; the moments each started go to words.log), and what b
# plays and what a's microphone hears recorded throughout (48 kHz, mono, 16 bits:
# played-b.raw, mic-a.raw, each with the moment it started in .start), for unmute.py.
# UNMUTE_MIC_VOLUME sets a's virtual microphone's volume (a percentage); UNMUTE_WORDS_AT the
# moments of the words after each unmute, in ms; UNMUTE_WORDS_START after a saw b connected
# (`mark-a-connected`). UNMUTE_ECHO=1: b talks (the same words every 3 s) and what a plays
# goes back into a's microphone 30 ms later, as from speakers; UNMUTE_ROOM_AFTER=<n>: only
# from a's n-th mute on (`mark-a-mute-<n>`), as a headset taken off while muted. When b
# talks, its microphone is recorded too (mic-b.raw).
lab_unmute_sound() {
  "$LAB_DIR/unmute-sound.sh" "$RUN/sound" >> "$RUN/lab.log" 2>&1 ||
    { echo "unmute-sound.sh failed" >> "$RUN/lab.log"; return 1; }
  [ -n "${UNMUTE_MIC_VOLUME:-}" ] && pactl set-source-volume laba_mic "${UNMUTE_MIC_VOLUME}%"
  mkfifo "$RUN/room.fifo"
  pacat --device=laba_test --raw --format=s16le --rate=48000 --channels=1 --latency-msec=40 \
    < "$RUN/room.fifo" > /dev/null 2>&1 &
  echo $! > "$RUN/room-pacat.pid"
  "$RUN/sound/room.sh" > "$RUN/room.fifo" 2>> "$RUN/lab.log" &
  echo $! > "$RUN/room-ffmpeg.pid"
  local far=${UNMUTE_FAR:-} room=${UNMUTE_ROOM:-}
  [ "${UNMUTE_ECHO:-}" = 1 ] && { far=${far:-words}; room=1; }
  local name device names="played-b mic-a"
  [ -n "$far" ] && names="$names mic-b"
  for name in $names; do
    case $name in
      played-b) device=labb_quiet.monitor ;;
      mic-a) device=laba_test.monitor ;;
      mic-b) device=labb_test.monitor ;;
    esac
    date +%s.%N > "$RUN/$name.start"
    parec --device="$device" --format=s16le --rate=48000 --channels=1 --raw \
      --latency-msec=20 > "$RUN/$name.raw" 2> /dev/null &
    echo $! > "$RUN/rec-$name.pid"
  done
  (
    if [ -n "${UNMUTE_WORDS_START:-}" ]; then
      until [ -s "$M/mark-a-connected" ]; do sleep 0.05; done
      at=$(cat "$M/mark-a-connected")
      for after in $UNMUTE_WORDS_START; do
        python3 -I -c "import time; time.sleep(max(0, ($at + $after) / 1000 - time.time()))"
        paplay --device=laba_test "$RUN/sound/words.wav" 2> /dev/null &
        echo "0 $after $(date +%s%3N)" >> "$RUN/words.log"
      done
    fi
    n=1
    while true; do
      at=$(cat "$M/mark-a-unmute-$n" 2> /dev/null)
      if [ -n "$at" ]; then
        for after in ${UNMUTE_WORDS_AT:-500 3500 6500}; do
          python3 -I -c "import time; time.sleep(max(0, ($at + $after) / 1000 - time.time()))"
          paplay --device=laba_test "$RUN/sound/words.wav" 2> /dev/null &
          echo "$n $after $(date +%s%3N)" >> "$RUN/words.log"
        done
        n=$((n + 1))
      fi
      sleep 0.05
    done
  ) &
  echo $! > "$RUN/words.pid"
  if [ "$room" = 1 ] && [ -n "${UNMUTE_ROOM_AFTER:-}" ]; then
    (
      until [ -e "$M/mark-a-mute-$UNMUTE_ROOM_AFTER" ]; do sleep 0.05; done
      pactl load-module module-loopback source=laba_quiet.monitor sink=laba_test latency_msec=30 \
        source_dont_move=true sink_dont_move=true > /dev/null
      echo "the room's echo from $(date +%s%3N)" >> "$RUN/lab.log"
    ) &
    echo $! > "$RUN/room-after.pid"
  elif [ "$room" = 1 ]; then
    pactl load-module module-loopback source=laba_quiet.monitor sink=laba_test latency_msec=30 \
      source_dont_move=true sink_dont_move=true > /dev/null
  fi
  if [ "$far" = tone ]; then
    # A steady tone with harmonics (220 Hz, -26 dBFS RMS), as someone holding a note.
    ffmpeg -hide_banner -loglevel error -f lavfi -i "sine=frequency=220:duration=10" \
      -f lavfi -i "sine=frequency=660:duration=10" -filter_complex \
      "[0][1]amix=inputs=2,volume=-20dB" -ar 48000 -ac 1 -sample_fmt s16 "$RUN/sound/far.wav"
  fi
  if [ -n "$far" ]; then
    (
      while true; do
        if [ "$far" = tone ]; then
          paplay --device=labb_test "$RUN/sound/far.wav" 2> /dev/null
        else
          paplay --device=labb_test "$RUN/sound/words.wav" 2> /dev/null &
          sleep 3
        fi
      done
    ) &
    echo $! > "$RUN/words-b.pid"
  fi
}

lab_unmute_stop() {
  local f
  for f in words words-b room-after; do
    [ -e "$RUN/$f.pid" ] && lab_kill "$(cat "$RUN/$f.pid")" bash
  done
  for f in room-ffmpeg:ffmpeg room-pacat:pacat rec-played-b:parec rec-mic-a:parec rec-mic-b:parec; do
    [ -e "$RUN/${f%%:*}.pid" ] && lab_kill "$(cat "$RUN/${f%%:*}.pid")" "${f#*:}"
  done
  rm -f "$RUN/room.fifo"
}

lab_stop() {
  local f
  for f in "$RUN"/[a-h].pid; do [ -e "$f" ] && lab_kill "$(cat "$f")" "$BUNDLE"; done
  for f in "$RUN"/tone-?.pid "$RUN/tone-apps.pid" "$RUN/recorder.pid" "$RUN/phases.pid" \
    "$RUN/echo-proof.pid" "$RUN/moving.pid" "$RUN/stalls.pid"; do [ -e "$f" ] && lab_kill "$(cat "$f")" bash; done
  for f in "$RUN"/played/*.pids; do
    # shellcheck disable=SC2046
    [ -e "$f" ] && kill $(cat "$f") 2> /dev/null
  done
  [ -e "$RUN/hang.pid" ] && lab_kill "$(cat "$RUN/hang.pid")" hang.py
  [ -e "$RUN/room.pid" ] && lab_kill "$(cat "$RUN/room.pid")" tone.py
  [ -e "$RUN/timing.pid" ] && lab_kill "$(cat "$RUN/timing.pid")" timing.py
  [ -e "$RUN/xvfb.pid" ] && lab_kill "$(cat "$RUN/xvfb.pid")" Xvfb
}

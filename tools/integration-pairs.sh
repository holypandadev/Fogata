#!/usr/bin/env bash
# Runs the integration tests of the application that need two copies of it talking to each
# other (call_test, direct_test, voice_test, move_test), on Linux, on one computer:
#
#     tools/integration-pairs.sh [<test>...]   (by default all of them, by name: call direct voice move)
#
# Each test is built as its own debug bundle (`flutter build linux --debug -t
# integration_test/<name>_test.dart`) and runs as separate copies, with the roles and folders
# the test file describes, on an X display of its own (Xvfb, never the desktop's) and a sound
# server of virtual devices of its own: a null sink the copies play into, and for the call and
# voice tests a virtual microphone fed with a made-up recording (a tone for the call test, a
# voice-like one for the voice test). No copy gets a real microphone, camera or speaker: each is
# given the virtual devices by name, no session bus (so nothing reaches the desktop's tray,
# keychain or file manager) and a data folder of its own. The sinks go only once the copies and
# what feeds them are gone, so nothing they play can move to the computer's real output.
#
#   call    two copies enter a group call muted, with no screen shared (the screen and sound
#           variants of call_test stay with the rigs: sharing a screen's sound needs a guard
#           that no copy records from a real source)
#   direct  Ana and Bea, Bea twice: the second time after she quit
#   voice   Leo and Marta, Marta twice: the second time with ALSA unable to open a microphone
#   move    an account moved from one copy to another, in each scenario of move_test
#
# Needs Xvfb, pactl, paplay, python3, timeout and flutter. Exits 0 when every test passed, 1
# when one failed, and 3 when something it needs is missing, naming it. FOGATA_PAIRS_TIMEOUT
# is how long one copy may run, in seconds (1800); FOGATA_PAIRS_OUT, a folder that keeps each
# test's logs, folders and pictures (else they go); FOGATA_PAIRS_MOVE_SCENARIOS, which scenarios
# of move_test run (all four).
set -euo pipefail
cd "$(dirname "$0")/.."

all=(call direct voice move)
tests=("$@")
[[ ${#tests[@]} -gt 0 ]] || tests=("${all[@]}")

missing=()
for tool in Xvfb pactl paplay python3 timeout flutter; do
    command -v "$tool" > /dev/null || missing+=("$tool")
done
if [[ ${#missing[@]} -eq 0 ]] && ! pactl info > /dev/null 2>&1; then
    missing+=("a sound server pactl reaches")
fi
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "integration pairs not run: missing ${missing[*]}" >&2
    exit 3
fi

repo=$PWD
bundle=$repo/app/build/linux/x64/debug/bundle/fogata
clip=$repo/app/integration_test/clip.mp4
limit=${FOGATA_PAIRS_TIMEOUT:-1800}
work=$(mktemp -d "${TMPDIR:-/tmp}/fogata-pairs-XXXXXX")
runs=${FOGATA_PAIRS_OUT:-$work}
[[ $runs == /* ]] || runs=$PWD/$runs
xvfb=
modules=() # sound modules loaded, unloaded in reverse
copies=()  # processes of the copies and of what feeds them, stopped before the sinks go
sinks=()   # the names of the sinks loaded

# Stops process $1 and waits until it is gone.
stop_pid() {
    local p=$1 n
    [[ -n $p ]] || return 0
    kill "$p" 2> /dev/null || return 0
    for n in $(seq 50); do
        kill -0 "$p" 2> /dev/null || { wait "$p" 2> /dev/null || true; return 0; }
        sleep 0.1
    done
    kill -9 "$p" 2> /dev/null || true
    wait "$p" 2> /dev/null || true
}

# The processes of the streams playing into sink $1.
sink_players() {
    local index
    index=$(pactl list short sinks | awk -v name="$1" '$2 == name {print $1}')
    [[ -n $index ]] || return 0
    pactl list sink-inputs | awk -v sink="$index" '
        /^Sink Input #/ {on = 0}
        $1 == "Sink:" {on = ($2 == sink)}
        on && /application.process.id = / {gsub(/"/, "", $3); print $3}'
}

# Stops what one test left: the copies and what feeds them, then each sink once nothing plays
# into it, the modules, and the display.
finish() {
    local p sink pids n i
    for p in ${copies[@]+"${copies[@]}"}; do
        stop_pid "$p"
    done
    copies=()
    for sink in ${sinks[@]+"${sinks[@]}"}; do
        for n in $(seq 50); do
            pids=$(sink_players "$sink")
            [[ -z $pids ]] && break
            # The copies are gone by now; whatever still plays here is one of theirs.
            [[ $n == 20 ]] && kill $pids 2> /dev/null || true
            sleep 0.1
        done
    done
    sinks=()
    for ((i = ${#modules[@]} - 1; i >= 0; i--)); do
        pactl unload-module "${modules[i]}" 2> /dev/null || true
    done
    modules=()
    if [[ -n $xvfb ]]; then
        stop_pid "$xvfb"
        xvfb=
    fi
}
trap 'finish; rm -rf "${work:?}"' EXIT

# Starts Xvfb on a display nobody uses, from :300 up (the test rigs keep below that): its
# number in $display, its process in $xvfb.
display=
start_display() {
    local n i
    for n in $(seq 300 399); do
        [[ -e /tmp/.X$n-lock || -e /tmp/.X11-unix/X$n ]] && continue
        Xvfb ":$n" -screen 0 1440x900x24 -nolisten tcp > "$work/xvfb-$n.log" 2>&1 &
        xvfb=$!
        for i in $(seq 100); do
            if [[ -S /tmp/.X11-unix/X$n ]]; then
                display=$n
                return 0
            fi
            kill -0 "$xvfb" 2> /dev/null || break
            sleep 0.1
        done
        # Taken meanwhile by another, or it did not start: the next one.
        stop_pid "$xvfb"
        xvfb=
    done
    return 1
}

# Loads a null sink named $1 with a description; its module is noted.
null_sink() {
    local m
    m=$(pactl load-module module-null-sink sink_name="$1" sink_properties=device.description="$2")
    modules+=("$m")
    sinks+=("$1")
}

# Loads a virtual microphone $1 whose input is the monitor of sink $2, described as $3.
remap_source() {
    local m
    m=$(pactl load-module module-remap-source source_name="$1" master="$2.monitor" \
        source_properties=device.description="$3")
    modules+=("$m")
}

# A wav file $1 of a mono 48 kHz signal: a tone of $2 Hz, with its level swelling and fading
# $3 times a second the way speech does ($3 = 0: steady), $4 seconds long.
make_wav() {
    python3 -I - "$1" "$2" "$3" "$4" << 'PY'
import math, struct, sys, wave
path, hz, rate_mod, seconds = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), int(sys.argv[4])
with wave.open(path, "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(48000)
    frames = bytearray()
    for i in range(48000 * seconds):
        t = i / 48000
        level = 0.25 * (0.55 + 0.45 * math.sin(2 * math.pi * rate_mod * t)) if rate_mod else 0.25
        frames += struct.pack("<h", int(32767 * level * math.sin(2 * math.pi * hz * t)))
    w.writeframes(bytes(frames))
PY
}

# Plays wav $2 into sink $1 again and again, in the background (its process joins `copies`),
# never letting the stream move to another output.
feed() {
    (
        trap 'exit 0' TERM
        while true; do
            paplay --device="$1" --property=node.dont-reconnect=true --property=node.dont-move=true "$2" \
                2> /dev/null || sleep 1
        done
    ) > /dev/null 2>&1 &
    copies+=($!)
}

# Runs one copy: $1 the folder it logs to and the tag of its log, $2 the sink it plays into,
# $3 the virtual microphone it records from, the rest NAME=value settings. Its process joins
# `copies` and is left in $pid.
pid=
copy() {
    local run=$1 sink=$2 source=$3
    shift 3
    env -u WAYLAND_DISPLAY DISPLAY=":$display" XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 \
        DBUS_SESSION_BUS_ADDRESS=disabled: LP_NUM_THREADS=2 \
        PULSE_SINK="$sink" PIPEWIRE_NODE="$sink" PULSE_SOURCE="$source" \
        FOGATA_CLIP="$clip" "$@" \
        timeout "$limit" nice -n 10 "$bundle" > "$run.out" 2>&1 &
    pid=$!
    copies+=("$pid")
}

# Waits up to $2 seconds for file $1; fails sooner when every process in $3.. is gone.
wait_file() {
    local file=$1 seconds=$2 n p alive
    shift 2
    for ((n = 0; n < seconds; n++)); do
        [[ -e $file ]] && return 0
        alive=0
        for p in "$@"; do
            kill -0 "$p" 2> /dev/null && alive=1
        done
        [[ $# -gt 0 && $alive == 0 ]] && { [[ -e $file ]] && return 0; return 1; }
        sleep 1
    done
    return 1
}

# Whether the result file $1 says "passed"; prints what it says otherwise.
passed() {
    local what
    what=$(head -n 1 "$1" 2> /dev/null || true)
    [[ $what == passed ]] && return 0
    echo "  ${1##*/}: ${what:-no result}" >&2
    return 1
}

# Builds the bundle of integration_test/$1_test.dart.
build() {
    echo "integration pairs: building $1_test"
    (cd app && flutter build linux --debug -t "integration_test/$1_test.dart" --no-pub) > "$runs/build-$1.log" 2>&1 || {
        tail -n 20 "$runs/build-$1.log" >&2
        return 1
    }
}

# The call test: two copies, muted, no screen shared.
run_call() {
    local d=$runs/call s=pair$$call
    mkdir -p "$d/a" "$d/b" "$d/meet"
    null_sink "${s}_test" "FogataTest-$s."
    remap_source "${s}_mic" "${s}_test" "FogataMic-$s."
    null_sink "${s}_quiet" "FogataQuietOut-$s."
    remap_source "${s}_quiet_mic" "${s}_quiet" "FogataQuiet-$s."
    local role pids=()
    for role in a b; do
        copy "$d/$role" "${s}_quiet" "${s}_quiet_mic" FOGATA_ROLE="$role" FOGATA_CALL="$d/meet" \
            FOGATA_CALL_LOG=1 FOGATA_PEERS=2 FOGATA_TEST_MIC=off FOGATA_TEST_DEVICES="-$s." \
            FOGATA_DATA_DIR="$d/$role" FOGATA_DOWNLOADS="$d/$role" RUST_LOG=warn
        pids+=("$pid")
    done
    local ok=0
    wait_file "$d/meet/a-result.txt" "$limit" "${pids[@]}" || true
    wait_file "$d/meet/b-result.txt" 120 "${pids[@]}" || true
    passed "$d/meet/a-result.txt" || ok=1
    passed "$d/meet/b-result.txt" || ok=1
    return $ok
}

# The direct chat test: Ana (a), and Bea (b) twice.
run_direct() {
    local d=$runs/direct s=pair$$direct
    mkdir -p "$d/a" "$d/b" "$d/meet" "$d/shots" "$d/dl-a" "$d/dl-b"
    null_sink "${s}_quiet" "FogataDirectQuiet-$s"
    start() { # role phase
        copy "$d/$1$2" "${s}_quiet" "${s}_quiet.monitor" FOGATA_ROLE="$1" FOGATA_PHASE="$2" \
            FOGATA_DIRECT="$d/meet" FOGATA_DATA_DIR="$d/$1" FOGATA_DOWNLOADS="$d/dl-$1" \
            FOGATA_SHOTS="$d/shots" RUST_LOG=warn
    }
    local ok=0 a b1 b2
    start a 1
    a=$pid
    sleep 2
    start b 1
    b1=$pid
    wait_file "$d/meet/b1-result.txt" "$limit" "$b1" || echo "  direct: b1 gave no result" >&2
    sleep 2
    stop_pid "$b1"
    wait_file "$d/meet/a-wrote" 600 "$a" || echo "  direct: Ana never wrote" >&2
    start b 2
    b2=$pid
    wait_file "$d/meet/b2-result.txt" "$limit" "$b2" || echo "  direct: b2 gave no result" >&2
    wait_file "$d/meet/a1-result.txt" 300 "$a" || echo "  direct: a1 gave no result" >&2
    sleep 2
    for r in a1 b1 b2; do
        passed "$d/meet/$r-result.txt" || ok=1
    done
    return $ok
}

# The voice message test: Leo (a), and Marta (b) twice, the second time with no microphone to open.
run_voice() {
    local d=$runs/voice sa=pair$$voicea sb=pair$$voiceb
    mkdir -p "$d/a" "$d/b" "$d/meet" "$d/shots"
    make_wav "$d/voice-a.wav" 180 3 20
    make_wav "$d/voice-b.wav" 260 4 20
    local s
    for s in "$sa" "$sb"; do
        null_sink "${s}_test" "FogataTest-$s."
        remap_source "${s}_mic" "${s}_test" "FogataMic-$s."
        null_sink "${s}_quiet" "FogataQuietOut-$s."
    done
    feed "${sa}_test" "$d/voice-a.wav"
    feed "${sb}_test" "$d/voice-b.wav"
    : > "$d/empty-alsa.conf"
    start() { # role phase [NAME=value...]
        local role=$1 phase=$2 s
        shift 2
        [[ $role == a ]] && s=$sa || s=$sb
        copy "$d/$role$phase" "${s}_quiet" "${s}_mic" FOGATA_TEST_MIC="FogataMic-$s." \
            FOGATA_ROLE="$role" FOGATA_PHASE="$phase" FOGATA_DIRECT="$d/meet" \
            FOGATA_DATA_DIR="$d/$role" FOGATA_SHOTS="$d/shots" RUST_LOG=warn "$@"
    }
    local ok=0 a b1 b2
    start a 1
    a=$pid
    sleep 2
    start b 1
    b1=$pid
    wait_file "$d/meet/b-quit" "$limit" "$b1" || echo "  voice: b1 did not get to the end" >&2
    wait_file "$d/meet/b1-result.txt" 30 "$b1" || true
    sleep 1
    stop_pid "$b1"
    wait_file "$d/meet/a-offline-done" 600 "$a" || echo "  voice: a made no offline pictures" >&2
    start b 2 ALSA_CONFIG_PATH="$d/empty-alsa.conf"
    b2=$pid
    wait_file "$d/meet/b2-result.txt" 600 "$b2" || echo "  voice: b2 gave no result" >&2
    wait_file "$d/meet/a1-result.txt" 300 "$a" || echo "  voice: a1 gave no result" >&2
    sleep 2
    for r in a1 b1 b2; do
        passed "$d/meet/$r-result.txt" || ok=1
    done
    return $ok
}

# The account move test, for each of its scenarios.
run_move() {
    local scenario ok=0 d s=pair$$move role pids
    for scenario in ${FOGATA_PAIRS_MOVE_SCENARIOS:-move replace lost keep}; do
        d=$runs/move-$scenario
        mkdir -p "$d/a" "$d/b" "$d/shots" "$d/move"
        null_sink "${s}_quiet" "FogataMoveQuiet-$s"
        pids=()
        for role in a b; do
            # In these two the old computer (a) loses the last message on purpose.
            local lose=0
            [[ $role == a && ( $scenario == lost || $scenario == keep ) ]] && lose=1
            copy "$d/$role" "${s}_quiet" "${s}_quiet.monitor" FOGATA_ROLE="$role" \
                FOGATA_SCENARIO="$scenario" FOGATA_TEST_LOSE_COMMIT="$lose" FOGATA_DATA_DIR="$d/$role" FOGATA_MOVE="$d/move" \
                FOGATA_SHOTS="$d/shots" FOGATA_DOWNLOADS="$d/$role" RUST_LOG=warn
            pids+=("$pid")
        done
        for role in a b; do
            wait_file "$d/move/$role-result.txt" "$limit" "${pids[@]}" || true
        done
        echo "  move $scenario" >&2
        for role in a b; do
            passed "$d/move/$role-result.txt" || ok=1
        done
        # A copy the test started again writes its own pid file.
        local f
        for f in "$d"/move/*.pid; do
            [[ -e $f ]] && stop_pid "$(cat "$f")"
        done
        finish
        start_display || { echo "  move: no free X display" >&2; return 1; }
    done
    return $ok
}

failed=()
for name in "${tests[@]}"; do
    case $name in
        call | direct | voice | move) ;;
        *) echo "no pair test $name (call, direct, voice, move)" >&2; failed+=("$name"); continue ;;
    esac
    build "$name" || { failed+=("$name"); continue; }
    start_display || { echo "$name: no free X display" >&2; failed+=("$name"); continue; }
    echo "integration pairs: $name (display :$display)"
    if "run_$name"; then
        echo "integration pairs: $name passed"
    else
        echo "integration pairs: $name FAILED"
        failed+=("$name")
    fi
    finish
done

if [[ ${#failed[@]} -gt 0 ]]; then
    echo "integration pairs failed: ${failed[*]}" >&2
    exit 1
fi
echo "integration pairs passed: ${tests[*]}"

#!/usr/bin/env bash
# Runs the integration tests of the application that need one copy of it and nothing else (no
# second copy, no command-line peer, no camera or microphone), on Linux:
#
#     tools/integration-tests.sh [<test>...]   (by default all of them, by name: screens_test)
#
# Each runs with `flutter test integration_test/<test>.dart -d linux` on an X display of its
# own (Xvfb, never the desktop's), with sound going to a null sink of its own (whose silent
# monitor is the default input instead of a microphone), no session bus (so nothing reaches the
# desktop's tray, keychain or file manager), and a fresh data folder; pictures and what would
# have opened go to a temporary folder. The sink goes only once the copy and its sound are
# gone, so nothing it plays can move to the computer's real output.
#
# Needs Xvfb and pactl (a PulseAudio or PipeWire sound server). Exits 0 when every test
# passed, 1 when one failed, and 3 when something it needs is missing, naming it.
# FOGATA_ITEST_TIMEOUT is how long one test may take, in seconds (1500); FOGATA_ITEST_OUT, a
# folder that keeps each test's pictures and folders (else they go). Each test builds the
# application for Linux (debug) as `flutter build linux` does.
set -euo pipefail
cd "$(dirname "$0")/.."

# Not window_test: maximizing and minimizing need a window manager, which Xvfb alone lacks.
all=(beta_test chat_tools_test layout_test screens_test settings_layout_test video_test)
tests=("$@")
[[ ${#tests[@]} -gt 0 ]] || tests=("${all[@]}")

missing=()
for tool in Xvfb pactl flutter; do
    command -v "$tool" > /dev/null || missing+=("$tool")
done
if [[ ${#missing[@]} -eq 0 ]] && ! pactl info > /dev/null 2>&1; then
    missing+=("a sound server pactl reaches")
fi
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "integration tests not run: missing ${missing[*]}" >&2
    exit 3
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/fogata-itest-XXXXXX")
runs=${FOGATA_ITEST_OUT:-$work}
[[ $runs == /* ]] || runs=$PWD/$runs
xvfb= display= module= sink=
# Stops what one test left: its display, and its sink once nothing plays into it.
finish() {
    if [[ -n $module ]]; then
        local pids n
        for n in $(seq 50); do
            pids=$(sink_players "$sink")
            [[ -z $pids ]] && break
            # The copies are gone by now; whatever still plays here is one of theirs.
            [[ $n == 20 ]] && kill $pids 2> /dev/null || true
            sleep 0.1
        done
        pactl unload-module "$module" 2> /dev/null || true
        module=
    fi
    if [[ -n $xvfb ]]; then
        kill "$xvfb" 2> /dev/null || true
        wait "$xvfb" 2> /dev/null || true
        xvfb=
    fi
}
trap 'finish; rm -rf "${work:?}"' EXIT

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

# Starts Xvfb on a display nobody uses, from :300 up (the test rigs keep below that): its
# number in $display, its process in $xvfb.
start_display() {
    local n
    for n in $(seq 300 399); do
        [[ -e /tmp/.X$n-lock || -e /tmp/.X11-unix/X$n ]] && continue
        Xvfb ":$n" -screen 0 1440x900x24 -nolisten tcp > "$work/xvfb-$n.log" 2>&1 &
        xvfb=$!
        local i
        for i in $(seq 100); do
            if [[ -S /tmp/.X11-unix/X$n ]]; then
                display=$n
                return 0
            fi
            kill -0 "$xvfb" 2> /dev/null || break
            sleep 0.1
        done
        # Taken meanwhile by another, or it did not start: the next one.
        kill "$xvfb" 2> /dev/null || true
        wait "$xvfb" 2> /dev/null || true
        xvfb=
    done
    return 1
}

failed=()
for name in "${tests[@]}"; do
    file=app/integration_test/$name.dart
    [[ -f $file ]] || { echo "no $file" >&2; failed+=("$name"); continue; }
    run=$runs/$name
    rm -rf "${run:?}"
    mkdir -p "$run/data" "$run/shots"
    start_display || { echo "$name: no free X display" >&2; failed+=("$name"); continue; }
    sink=fogata_itest_$$_$name
    module=$(pactl load-module module-null-sink sink_name="$sink" \
        sink_properties=device.description="FogataTest-$name")
    echo "integration test $name (display :$display, sink $sink)"
    if (cd app && env -u WAYLAND_DISPLAY DISPLAY=":$display" XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 \
        DBUS_SESSION_BUS_ADDRESS=disabled: LP_NUM_THREADS=2 \
        PULSE_SINK="$sink" PIPEWIRE_NODE="$sink" PULSE_SOURCE="$sink.monitor" \
        FOGATA_DATA_DIR="$run/data" FOGATA_DOWNLOADS="$run/data" FOGATA_SHOTS="$run/shots" \
        FOGATA_TEST_OPENED="$run/opened.txt" \
        timeout "${FOGATA_ITEST_TIMEOUT:-1500}" flutter test --no-pub "integration_test/$name.dart" -d linux); then
        echo "integration test $name passed"
    else
        echo "integration test $name FAILED"
        failed+=("$name")
    fi
    finish
done

if [[ ${#failed[@]} -gt 0 ]]; then
    echo "integration tests failed: ${failed[*]}" >&2
    exit 1
fi
echo "integration tests passed: ${tests[*]}"

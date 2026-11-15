#!/usr/bin/env bash
# Whether a screen shared on KDE Plasma Wayland while nothing on it moves shows at once to
# someone who opens it. The portal's capture (PipeWire) gives a picture only when something
# on the shared screen changes, and nothing is sent to a watcher before they open the
# screen: the picture they first get has to come from what the capture already had.
#
#     tools/smoke-wayland/still.sh [calls] [opens] [still|moved|both]
#
# In a KDE Plasma Wayland session of its own (session.sh, with two virtual outputs), two
# copies of the call test (app/integration_test/call_test.dart, FOGATA_TEST_STILL) start
# a call [calls] times (default 30), each time anew (new folders, new connection); in each,
# copy a shares the first output through the portal's dialog and copy b opens the shared
# screen [opens] times (default 1), closing it between. The copies' windows are moved to
# the second output as they appear, so nothing on the shared one moves: KWin draws no
# panel, wallpaper or clock there, and the pointer stays where it is. Before each open:
#   still  nothing is done: the screen has stayed still since the share started (or since
#          the open before);
#   moved  a window is moved onto the shared output and back first;
#   both   (default) still and moved in turn, still first.
# Then b opens the screen only once PipeWire has given copy a no buffer for STILL_QUIET
# seconds (default 5), which the plugin's log proves (FOGATA_WEBRTC_LOG: each buffer and
# each frame given again, with its time); a wait of more than 120 s for that fails the
# open. b logs how long the first picture took, decoded and (on a call's first open) shown;
# with none in FOGATA_TEST_STILL_GIVE_UP seconds (20) the screen is changed (a window
# moved onto it and back) and b waits 30 s more for it.
#
# Each open's line, in <out>/results.txt: the call, the open, the case, the buffers
# PipeWire gave in the still wait and from the go until the picture, the frames given again
# meanwhile, the milliseconds to the picture decoded and shown (-1: none; shown only on a
# call's first open) and those after the change (-1: not needed). Exits 0 when every
# open's picture came within STILL_PROMPT seconds (default 3) with no buffer from PipeWire
# in between, 1 when one did not (or a copy failed), and 3 when the session cannot be built
# here: it needs a GPU's render node for KWin to share a screen (session.sh), as the
# Wayland desktops of tools/smoke-linux.sh, so it does not run on GitHub's computers.
#
# STILL_WINDOWS=shared leaves the copies' windows on the shared output (as on a desktop
# with one screen): what they draw then reaches the capture, and the opens wait for
# stillness in vain (the results say how often PipeWire gave a buffer). The bundle is
# built once (`flutter build linux --debug -t integration_test/call_test.dart`) unless
# FOGATA_STILL_BUNDLE names one; FOGATA_STILL_OUT keeps the run's folders there (else a
# folder of its own under $TMPDIR, kept and named).
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$HERE/../.." && pwd)
cd "$repo" || exit 1

# A KWin script, run once in the session's KWin.
kwin() { # code
  python3 -I -c '
import os, sys, time, dbus
path = os.path.join(sys.argv[1], "kwin.js")
with open(path, "w") as f:
    f.write(sys.argv[2])
bus = dbus.SessionBus()
n = bus.get_object("org.kde.KWin", "/Scripting").loadScript(
    path, dbus_interface="org.kde.kwin.Scripting")
script = bus.get_object("org.kde.KWin", "/Scripting/Script%d" % n)
script.run(dbus_interface="org.kde.kwin.Script")
time.sleep(0.2)
script.stop(dbus_interface="org.kde.kwin.Script")' "$OUT" "$1"
}

# Now, in milliseconds since the epoch, on the clock the plugin's log uses (from bash
# itself: some `date` commands do not pad %N's digits).
ms() {
  local t=${EPOCHREALTIME/./}
  echo "${t:0:${#t}-3}"
}

# The second output's left edge (the first, at 0, is the one shared).
SECOND=${SMOKE_WIDTH:-1600}

# Moves every application window that is on the shared output to the second one.
place() {
  kwin "for (const w of workspace.windowList()) {
  if (!w.normalWindow || w.frameGeometry.x >= $SECOND) continue;
  w.frameGeometry = {x: $SECOND + 40 + w.frameGeometry.x % 200, y: 40,
    width: Math.min(w.frameGeometry.width, $SECOND - 80),
    height: Math.min(w.frameGeometry.height, ${SMOKE_HEIGHT:-900} - 80)};
}"
}

# A change on the shared output: an application window moved onto it, and back.
change() {
  kwin "for (const w of workspace.windowList()) {
  if (w.normalWindow) { w.frameGeometry = {x: 200, y: 100, width: w.frameGeometry.width,
    height: w.frameGeometry.height}; break; }
}"
  sleep 1
  [ "${STILL_WINDOWS:-}" = shared ] || place
  echo "$(ms) changed" >> "$OUT/conductor.log"
}

# The times (ms since the epoch) of PipeWire's buffers in copy a's log, after [from].
buffers_after() { # from [until]
  grep -a '^\[PortalScreenCapturer\] [0-9]* buffer' "$1/a.out" 2> /dev/null |
    awk -v from="$2" -v until="${3:-99999999999999}" '$2 > from && $2 <= until { print $2 }'
}
again_after() { # run from until
  grep -a '^\[PortalScreenCapturer\] [0-9]* last frame again' "$1/a.out" 2> /dev/null |
    awk -v from="$2" -v until="$3" '$2 > from && $2 <= until' | wc -l
}

if [ "${STILL_INSIDE:-}" = 1 ]; then
  # Inside the session: the calls, one after the other.
  CALLS=$1 OPENS=$2 CASE=$3
  quiet=${STILL_QUIET:-5}
  pactl load-module module-null-sink sink_name=still_out \
    sink_properties=device.description=FogataStillOut > /dev/null || exit 1
  pactl set-default-sink still_out
  : > "$OUT/results.txt"
  failed=0
  turn=0
  for call in $(seq "$CALLS"); do
    run=$OUT/call-$call
    rm -rf "${run:?}"
    mkdir -p "$run/meet" "$run/a" "$run/b"
    declare -A pid
    for role in a b; do
      share=()
      [ $role = a ] && share=(FOGATA_TEST_SCREEN=1)
      env FOGATA_ROLE=$role FOGATA_CALL="$run/meet" FOGATA_DATA_DIR="$run/$role" \
        FOGATA_DOWNLOADS="$run/$role" FOGATA_TEST_STILL="$OPENS" "${share[@]}" \
        FOGATA_CALL_LOG=1 FOGATA_WEBRTC_LOG=error LP_NUM_THREADS=2 RUST_LOG=warn \
        timeout $((OPENS * 150 + 400)) "$BUNDLE" > "$run/$role.out" 2>&1 &
      pid[$role]=$!
      sleep 2
    done
    for open in $(seq "$OPENS"); do
      end=$((SECONDS + 400))
      while [ ! -e "$run/meet/still-$open-ready" ] && [ $SECONDS -lt $end ]; do
        [ "${STILL_WINDOWS:-}" = shared ] || place
        kill -0 "${pid[a]}" 2> /dev/null && kill -0 "${pid[b]}" 2> /dev/null || break
        grep -qs '^failed' "$run/meet/a-result.txt" "$run/meet/b-result.txt" && break
        sleep 0.5
      done
      if [ ! -e "$run/meet/still-$open-ready" ]; then
        echo "call $call open $open: b never got ready (see $run)"
        failed=1
        break
      fi
      case $CASE in
        still) what=still ;;
        moved) what=moved ;;
        *) what=$([ $((turn % 2)) = 0 ] && echo still || echo moved) ;;
      esac
      turn=$((turn + 1))
      waited=$(ms)
      [ $what = moved ] && { sleep 1; change; }
      # Until PipeWire has given nothing for $quiet seconds.
      end=$((SECONDS + 120))
      while :; do
        now=$(ms)
        last=$(buffers_after "$run" 0 | tail -n 1)
        since=$(( now - ${last:-0} ))
        [ "$since" -ge $((quiet * 1000)) ] && [ $((now - waited)) -ge $((quiet * 1000)) ] && break
        if [ $SECONDS -ge $end ]; then
          echo "call $call open $open: the screen never stayed still for ${quiet} s" \
            "($(buffers_after "$run" "$waited" | wc -l) buffers in 120 s)"
          failed=1
          break
        fi
        sleep 0.2
      done
      go=$(ms)
      echo "$go go $call/$open $what" >> "$OUT/conductor.log"
      : > "$run/meet/still-$open-go"
      # Until b has its picture, or gives up on it and asks for a change.
      end=$((SECONDS + 120))
      while ! grep -qs "^$open " "$run/meet/still-results.txt" && [ $SECONDS -lt $end ]; do
        if [ -e "$run/meet/still-$open-stuck" ] && [ ! -e "$run/meet/still-$open-changed" ]; then
          change
          : > "$run/meet/still-$open-changed"
        fi
        kill -0 "${pid[b]}" 2> /dev/null || break
        sleep 0.2
      done
      read -r _ decoded shown after_decoded after_shown opened < <(grep "^$open " "$run/meet/still-results.txt")
      if [ -z "${decoded:-}" ]; then
        echo "call $call open $open: no result from b (see $run)"
        failed=1
        break
      fi
      # From the go to the picture (or to the change, if none came): b opens a moment
      # after the go, and its times count from the open.
      took=$decoded
      [ "${shown:--1}" -gt "$took" ] && took=$shown
      [ "$took" -lt 0 ] && took=$((${STILL_GIVE_UP:-20} * 1000))
      # Counted now, from the whole log: none in the quiet seconds before the go either.
      still_buffers=$(buffers_after "$run" "$((go - quiet * 1000))" "$go" | wc -l)
      during=$(buffers_after "$run" "$go" "$((${opened:-$go} + took))" | wc -l)
      again=$(again_after "$run" "$go" "$((${opened:-$go} + took))")
      echo "$call $open $what $still_buffers $during $again $decoded $shown $after_decoded $after_shown" \
        >> "$OUT/results.txt"
      prompt=$((${STILL_PROMPT:-3} * 1000))
      if [ "$decoded" -lt 0 ] || [ "$decoded" -gt $prompt ] || [ "$shown" -gt $prompt ] ||
        { [ "$open" = 1 ] && [ "$shown" -lt 0 ]; } || [ "$still_buffers" != 0 ] ||
        [ "$during" != 0 ]; then
        failed=1
      fi
      echo "call $call open $open ($what): buffers while still $still_buffers, until the" \
        "picture $during, frames given again $again; decoded ${decoded} ms, shown ${shown} ms;" \
        "after a change: decoded ${after_decoded} ms, shown ${after_shown} ms"
      unset decoded shown after_decoded after_shown opened
    done
    # The call ends: both copies wrote their result (or are gone), then they are stopped.
    end=$((SECONDS + 120))
    for role in a b; do
      while [ ! -s "$run/meet/$role-result.txt" ] && kill -0 "${pid[$role]}" 2> /dev/null &&
        [ $SECONDS -lt $end ]; do
        sleep 0.5
      done
    done
    for role in a b; do
      kill "${pid[$role]}" 2> /dev/null
      wait "${pid[$role]}" 2> /dev/null
      what=$(head -n 1 "$run/meet/$role-result.txt" 2> /dev/null)
      [ "$what" = passed ] || { echo "call $call: copy $role ${what:-gave no result}"; failed=1; }
    done
    unset pid
  done
  exit $failed
fi

CALLS=${1:-30} OPENS=${2:-1} CASE=${3:-both}
work=${FOGATA_STILL_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/fogata-still-XXXXXX")}
[[ $work == /* ]] || work=$PWD/$work
mkdir -p "$work"
bundle=${FOGATA_STILL_BUNDLE:-}
if [ -z "$bundle" ]; then
  command -v flutter > /dev/null || { echo "still: flutter missing" >&2; exit 3; }
  echo "still: building integration_test/call_test.dart"
  if ! (cd app && flutter build linux --debug -t integration_test/call_test.dart --no-pub) \
    > "$work/build.log" 2>&1; then
    tail -n 20 "$work/build.log" >&2
    echo "still: build FAILED (log in $work/build.log)" >&2
    exit 1
  fi
  bundle=$repo/app/build/linux/x64/debug/bundle/fogata
fi
[ -x "$bundle" ] || { echo "still: no bundle at $bundle" >&2; exit 1; }
echo "still: $CALLS calls of $OPENS opens ($CASE), in $work"
SMOKE_OUTPUTS=2 "$HERE/session.sh" "$work/session" env STILL_INSIDE=1 OUT="$work" \
  BUNDLE="$bundle" "$HERE/still.sh" "$CALLS" "$OPENS" "$CASE"
status=$?
if [ -s "$work/results.txt" ]; then
  awk '{ n++; if ($7 >= 0 && $7 <= 20000) { got++; sum += $7; if ($7 > max) max = $7 }
         else never++; buf += $5 }
       END { printf "still: %d opens, %d with a picture (mean %d ms, longest %d ms), %d none" \
             " without a change; %d buffers from PipeWire between the go and the picture\n",
             n, got, got ? sum / got : 0, max, never, buf }' "$work/results.txt"
fi
case $status in
  0) echo "still: passed (folders in $work)" ;;
  3) echo "still: SKIPPED: this computer cannot build the session (see above)" ;;
  *) echo "still: FAILED (folders in $work)" ;;
esac
exit $status

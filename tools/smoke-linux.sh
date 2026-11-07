#!/usr/bin/env bash
# The smoke test on Linux: two copies of the app do the main things a person does
# (app/integration_test/smoke_test.dart: profiles, friends, a direct chat both ways, a group,
# its voice room, the camera, the screen shared with the room's button and watched, a file,
# leaving), in the kinds of desktop people run it in:
#
#     tools/smoke-linux.sh [<desktop>...]   (by default all of them: wayland xwayland bare x11)
#
#   wayland  a KDE Plasma Wayland session of its own (tools/smoke-wayland/session.sh: KWin
#            on a virtual output, Xwayland, its own session bus, PipeWire and desktop
#            portal); the copies start as Plasma starts an application, and the screen is
#            picked in the portal's own dialog, answered by the session's picker
#   xwayland the same session, with the copies on X11 through Xwayland (as with
#            GDK_BACKEND=x11 under Plasma): KWin manages their windows, so the system lists
#            the screen and the windows, and the app shows its own list
#   bare     the same session, with the copies started the way a service or a remote shell
#            starts them there: only WAYLAND_DISPLAY and the session bus, no
#            XDG_SESSION_TYPE and no DISPLAY; GTK still draws on Wayland, so the screen is
#            still picked in the portal's dialog
#   x11      an X display of its own (Xvfb) in a sound session of its own
#            (tools/call-lab/session.sh), with no session bus, as the other integration
#            tests run; with no window manager the system lists only the screen, which the
#            button shares at once, with no list
#
# Nothing reaches the computer's own screen, sound, camera or desktop session: see the two
# session scripts. The copies get a made-up camera (FOGATA_TEST_CAMERA=1) and no microphone.
# The bundle is built once (`flutter build linux --debug -t integration_test/smoke_test.dart`)
# unless FOGATA_SMOKE_BUNDLE names one already built.
#
# Exits 0 when every desktop passed, 1 when one failed (a copy that died is named with its
# signal: a crash), and 3 when none could run here (each skipped one is named with why).
# A desktop that cannot be built here (no GPU render node for KWin's screen sharing, no
# unprivileged namespaces, a program missing) is skipped, not failed. FOGATA_SMOKE_OUT keeps
# each run's logs and folders there; without it they go in a folder of their own under
# $TMPDIR, where those of a desktop that failed (and a build that failed) are kept, and
# named, and the rest go. FOGATA_SMOKE_TIMEOUT is how long a copy may run, in seconds (900).
set -uo pipefail
cd "$(dirname "$0")/.."
repo=$PWD

if [ "${SMOKE_INSIDE:-}" = 1 ]; then
  # Inside a session: runs the two copies, with the settings in "$@", and judges them.
  out=$SMOKE_OUT
  limit=$SMOKE_LIMIT
  mkdir -p "$out/meet" "$out/a" "$out/b" "$out/dl-a" "$out/dl-b"
  # A silent default output for whatever plays.
  pactl load-module module-null-sink sink_name=smoke_out \
    sink_properties=device.description=FogataSmokeOut > /dev/null || exit 1
  pactl set-default-sink smoke_out
  declare -A pid
  for role in a b; do
    env FOGATA_ROLE=$role FOGATA_SMOKE="$out/meet" FOGATA_DATA_DIR="$out/$role" \
      FOGATA_DOWNLOADS="$out/dl-$role" FOGATA_TEST_CAMERA=1 LP_NUM_THREADS=2 RUST_LOG=warn \
      "$@" timeout "$limit" nice -n 10 "$SMOKE_BUNDLE" > "$out/$role.out" 2>&1 &
    pid[$role]=$!
    sleep 2
  done
  # Until both copies wrote their result, or one is gone without one.
  declare -A status
  end=$((SECONDS + limit + 30))
  while [ $SECONDS -lt $end ]; do
    done=1
    for role in a b; do
      [ -e "$out/meet/$role-result.txt" ] && continue
      done=0
      if [ -z "${status[$role]:-}" ] && ! kill -0 "${pid[$role]}" 2> /dev/null; then
        wait "${pid[$role]}"
        status[$role]=$?
      fi
    done
    [ $done = 1 ] && break
    # A copy that failed leaves the other waiting for it: a minute more, then judged.
    if grep -qs '^failed' "$out/meet/a-result.txt" "$out/meet/b-result.txt" && [ -z "${gone_at:-}" ]; then
      gone_at=$SECONDS
    fi
    # Both gone, or one gone and the other waiting for it in vain.
    [ -n "${status[a]:-}" ] && [ -n "${status[b]:-}" ] && break
    if [ -n "${status[a]:-}${status[b]:-}" ] && [ -z "${gone_at:-}" ]; then gone_at=$SECONDS; fi
    [ -n "${gone_at:-}" ] && [ $((SECONDS - gone_at)) -gt 60 ] && break
    sleep 1
  done
  ok=0
  for role in a b; do
    what=$(head -n 1 "$out/meet/$role-result.txt" 2> /dev/null)
    s=${status[$role]:-}
    if [ -z "$s" ] && ! kill -0 "${pid[$role]}" 2> /dev/null; then
      wait "${pid[$role]}"
      s=$?
    fi
    if [ -n "$s" ] && [ "$s" -gt 128 ] && [ "$s" != 143 ]; then
      echo "  copy $role CRASHED: signal $((s - 128)) ($(kill -l $((s - 128)) 2> /dev/null))" \
        "after: $(tail -n 1 "$out/meet/$role.log" 2> /dev/null)"
      ok=1
    elif [ "$s" = 124 ]; then
      echo "  copy $role ran out of time after: $(tail -n 1 "$out/meet/$role.log" 2> /dev/null)"
      ok=1
    elif [ "$what" != passed ]; then
      echo "  copy $role: ${what:-no result} (exit ${s:-running})"
      ok=1
    else
      echo "  copy $role passed"
    fi
  done
  for role in a b; do
    kill "${pid[$role]}" 2> /dev/null
  done
  wait 2> /dev/null
  exit $ok
fi

all=(wayland xwayland bare x11)
desktops=("$@")
[ ${#desktops[@]} -gt 0 ] || desktops=("${all[@]}")
limit=${FOGATA_SMOKE_TIMEOUT:-900}
work=$(mktemp -d "${TMPDIR:-/tmp}/fogata-smoke-XXXXXX")
runs=${FOGATA_SMOKE_OUT:-$work}
[[ $runs == /* ]] || runs=$PWD/$runs
mkdir -p "$runs"
# Without FOGATA_SMOKE_OUT, the folder goes at the end unless something in it failed.
keep=
trap '[ -n "$keep" ] || rm -rf "${work:?}"' EXIT

bundle=${FOGATA_SMOKE_BUNDLE:-}
if [ -z "$bundle" ]; then
  command -v flutter > /dev/null || { echo "smoke: flutter missing" >&2; exit 3; }
  echo "smoke: building integration_test/smoke_test.dart"
  if ! (cd app && flutter build linux --debug -t integration_test/smoke_test.dart --no-pub) \
    > "$runs/build.log" 2>&1; then
    tail -n 20 "$runs/build.log" >&2
    keep=1
    echo "smoke: build FAILED (log in $runs/build.log)" >&2
    exit 1
  fi
  bundle=$repo/app/build/linux/x64/debug/bundle/fogata
fi
[ -x "$bundle" ] || { echo "smoke: no bundle at $bundle" >&2; exit 1; }

passed=() failed=() skipped=()
for desktop in "${desktops[@]}"; do
  out=$runs/$desktop
  rm -rf "${out:?}"
  mkdir -p "$out"
  echo "smoke: $desktop"
  # Which way the screen must go there: the portal's dialog where GTK draws on Wayland,
  # else the app's own list (the test fails, named, if the app would take the other).
  case $desktop in
    wayland | bare) share=portal ;;
    *) share=list ;;
  esac
  common=(env SMOKE_INSIDE=1 SMOKE_OUT="$out" SMOKE_LIMIT="$limit" SMOKE_BUNDLE="$bundle"
    FOGATA_SMOKE_SHARE="$share")
  case $desktop in
    wayland)
      tools/smoke-wayland/session.sh "$out/session" "${common[@]}" "$0"
      ;;
    xwayland)
      tools/smoke-wayland/session.sh "$out/session" "${common[@]}" "$0" \
        env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 GDK_BACKEND=x11
      ;;
    bare)
      tools/smoke-wayland/session.sh "$out/session" "${common[@]}" "$0" \
        env -u XDG_SESSION_TYPE -u XDG_CURRENT_DESKTOP -u XDG_SESSION_DESKTOP -u DISPLAY \
        -u KDE_FULL_SESSION -u DESKTOP_SESSION
      ;;
    x11)
      if ! command -v Xvfb > /dev/null; then
        echo "smoke session: missing Xvfb" >&2
        (exit 3)
      else
        # A display number nobody uses, from :400 up (the other tests keep below that).
        n=400
        while [ -e /tmp/.X$n-lock ] || [ -e /tmp/.X11-unix/X$n ]; do n=$((n + 1)); done
        tools/call-lab/session.sh "$out/session" bash -c '
          Xvfb ":$2" -screen 0 1600x900x24 -nolisten tcp > "$1/xvfb.log" 2>&1 &
          for _ in $(seq 100); do [ -S "/tmp/.X11-unix/X$2" ] && break; sleep 0.1; done
          shift 2
          "$@"' _ "$out/session" "$n" "${common[@]}" "$0" env DISPLAY=":$n" \
          XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 DBUS_SESSION_BUS_ADDRESS=disabled:
      fi
      ;;
    *)
      echo "smoke: no desktop $desktop ($(printf '%s ' "${all[@]}"))" >&2
      (exit 1)
      ;;
  esac
  case $? in
    0) passed+=("$desktop"); echo "smoke: $desktop passed" ;;
    3) skipped+=("$desktop"); echo "smoke: $desktop SKIPPED: this computer cannot build that desktop (see above)" ;;
    *) failed+=("$desktop"); keep=1; echo "smoke: $desktop FAILED (logs in $out)"; continue ;;
  esac
  # Only a failed desktop's folder stays, unless FOGATA_SMOKE_OUT keeps them all.
  [ "$runs" = "$work" ] && rm -rf "${out:?}"
done

echo "smoke: passed ${passed[*]:-none}; failed ${failed[*]:-none}; skipped ${skipped[*]:-none}"
[ ${#failed[@]} -gt 0 ] && echo "smoke: the failed desktops' logs are kept in $runs"
[ ${#failed[@]} -gt 0 ] && exit 1
[ ${#passed[@]} -eq 0 ] && exit 3
exit 0

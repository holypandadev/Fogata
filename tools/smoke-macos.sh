#!/usr/bin/env bash
# The smoke test on macOS: two copies of the app on this Mac do the main things a person
# does (app/integration_test/smoke_test.dart: profiles, friends, a direct chat both ways, a
# group, its voice room, the screen shared with the room's button through the system's own
# capture and watched by the other copy, a file, leaving).
#
#     tools/smoke-macos.sh build   builds the test and makes "Fogata Smoke.app" of it
#     tools/smoke-macos.sh run     runs the two copies and judges them
#
# The copies are an application of their own, never the person's Fogata: the build is
# copied to "Fogata Smoke.app" with the bundle identifier dev.holypanda.fogata.smoke (so its
# permissions, keychain items and settings are its own), signed for this Mac only, and each
# copy gets its own data folder. macOS asks the person, the first time, to let it record the
# screen (System Settings > Privacy & Security > Screen Recording) and maybe to accept
# incoming connections (the firewall): only a person may answer those, so the first run is
# an attended one, and the windows of the two copies show on the screen while it runs. No
# microphone is opened and the camera stays off; the app's sounds play to the default
# output, so mute the Mac's sound first.
#
# FOGATA_SMOKE_OUT keeps the run's logs and folders (default ~/fogata-smoke-runs/<time>);
# FOGATA_SMOKE_TIMEOUT is how long a copy may run, in seconds (900); FOGATA_SMOKE_APP names
# the app to run (default app/build/smoke/Fogata Smoke.app).
# Exits 0 when both passed, 1 when one failed (a copy that died is named with its signal: a
# crash), 3 when it cannot run here.
set -uo pipefail
cd "$(dirname "$0")/.."
repo=$PWD
[ "$(uname)" = Darwin ] || { echo "smoke: macOS only" >&2; exit 3; }
id=dev.holypanda.fogata.smoke
smoke_app=${FOGATA_SMOKE_APP:-$repo/app/build/smoke/Fogata Smoke.app}
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

build() {
  command -v flutter > /dev/null || { echo "smoke: flutter missing" >&2; exit 3; }
  echo "smoke: building integration_test/smoke_test.dart"
  (cd app && flutter build macos --debug -t integration_test/smoke_test.dart) || exit 1
  local built=$repo/app/build/macos/Build/Products/Debug/Fogata.app
  rm -rf "${smoke_app:?}"
  mkdir -p "$(dirname "$smoke_app")"
  cp -R "$built" "$smoke_app" || exit 1
  local plist=$smoke_app/Contents/Info.plist
  /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $id" "$plist" || exit 1
  /usr/libexec/PlistBuddy -c "Set :CFBundleName Fogata Smoke" "$plist" || exit 1
  /usr/libexec/PlistBuddy -c "Delete :CFBundleURLTypes" "$plist" 2> /dev/null
  # Signed again for the new identifier, for this Mac only, with the debug build's rights.
  codesign --force --deep --sign - --entitlements app/macos/Runner/DebugProfile.entitlements \
    "$smoke_app" || exit 1
  # The debug build itself goes, so that macOS never opens it for the person's links.
  "$lsregister" -u "$built" 2> /dev/null
  rm -rf "$built"
  echo "smoke: built $smoke_app ($id)"
}

run() {
  local exe=$smoke_app/Contents/MacOS/Fogata
  [ -x "$exe" ] || { echo "smoke: no app at $smoke_app (run: $0 build)" >&2; exit 3; }
  local out=${FOGATA_SMOKE_OUT:-$HOME/fogata-smoke-runs/$(date +%Y%m%d-%H%M%S)}
  local limit=${FOGATA_SMOKE_TIMEOUT:-900}
  mkdir -p "$out/meet"
  # The macOS bash has no maps: copy a is 0, copy b is 1.
  local roles=(a b) pid=() status=('' '') i
  for i in 0 1; do
    mkdir -p "$out/${roles[i]}"
    env FOGATA_ROLE="${roles[i]}" FOGATA_SMOKE="$out/meet" FOGATA_DATA_DIR="$out/${roles[i]}" \
      FOGATA_DOWNLOADS="$out/${roles[i]}" RUST_LOG=warn "$exe" > "$out/${roles[i]}.out" 2>&1 &
    pid[i]=$!
    sleep 2
  done
  local end=$((SECONDS + limit)) gone_at= done
  while [ $SECONDS -lt $end ]; do
    done=1
    for i in 0 1; do
      [ -e "$out/meet/${roles[i]}-result.txt" ] && continue
      done=0
      if [ -z "${status[i]}" ] && ! kill -0 "${pid[i]}" 2> /dev/null; then
        wait "${pid[i]}"
        status[i]=$?
        gone_at=${gone_at:-$SECONDS}
      fi
    done
    [ $done = 1 ] && break
    if grep -qs '^failed' "$out/meet/a-result.txt" "$out/meet/b-result.txt"; then
      gone_at=${gone_at:-$SECONDS}
    fi
    [ -n "$gone_at" ] && [ $((SECONDS - gone_at)) -gt 60 ] && break
    sleep 1
  done
  local ok=0 what s role
  for i in 0 1; do
    role=${roles[i]}
    what=$(head -n 1 "$out/meet/$role-result.txt" 2> /dev/null)
    s=${status[i]}
    if [ -n "$s" ] && [ "$s" -gt 128 ]; then
      echo "  copy $role CRASHED: signal $((s - 128)) after: $(tail -n 1 "$out/meet/$role.log" 2> /dev/null)"
      ok=1
    elif [ "$what" != passed ]; then
      echo "  copy $role: ${what:-no result} after: $(tail -n 1 "$out/meet/$role.log" 2> /dev/null)"
      ok=1
    else
      echo "  copy $role passed"
    fi
  done
  for i in 0 1; do
    kill "${pid[i]}" 2> /dev/null
  done
  wait 2> /dev/null
  echo "smoke: logs in $out"
  [ $ok = 0 ] && echo "smoke: passed" || echo "smoke: FAILED"
  exit $ok
}

case ${1:-} in
  build) build ;;
  run) run ;;
  *) echo "usage: $0 build|run" >&2; exit 2 ;;
esac

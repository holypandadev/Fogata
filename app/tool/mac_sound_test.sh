#!/bin/bash
# macOS: checks that a shared window brings its own application's sound and no other's, as
# a call takes it (integration_test/screen_sound_test.dart, with FOGATA_SOUND_WINDOW_NAME
# and FOGATA_SOUND_FLAG). Run on a Mac, with flutter on the PATH, from a checkout whose
# packages are in place:
#
#   app/tool/mac_sound_test.sh
#
# It builds the test (debug), makes two short spoken sounds with `say`, opens one in
# QuickTime Player (the window shared) and plays the other again and again with `afplay`
# (another program, which must not come through). Once the test asks, QuickTime Player
# plays its own (which must come through). Both play on the Mac's output for about 25
# seconds: turn the sound down. The first run makes macOS ask for Screen Recording for the
# test build; allow it (System Settings > Privacy & Security) and run it again.
#
# Prints the result ("passed", or "failed: ..." with the levels); exits 0 only when it
# passed. Its files (sound.log, app.log) stay in OUT (a new folder in /tmp by default).
set -u
cd "$(dirname "$0")/.." || exit 1
export LANG=en_US.UTF-8
out="${OUT:-$(mktemp -d /tmp/fogata-sound.XXXXXX)}"
mkdir -p "$out"
rm -f "$out/sound-result.txt" "$out/play.flag"
name=fogata-own-sound
app=build/macos/Build/Products/Debug/Fogata.app

echo "building the test (logs in $out)"
if ! flutter build macos --debug --no-pub -t integration_test/screen_sound_test.dart \
  > "$out/build.log" 2>&1; then
  tail -n 20 "$out/build.log"
  echo "failed: the test did not build"
  exit 1
fi

say -o "$out/$name.aiff" "This is what the shared window plays, one, two, three, four, five." &&
  say -o "$out/other.aiff" "Another program plays this, and it must not be heard." || {
  echo "failed: no sounds made"
  exit 1
}

other= watch= test=
finish() {
  [ -n "$test" ] && kill "$test" 2> /dev/null
  [ -n "$watch" ] && kill "$watch" 2> /dev/null
  if [ -n "$other" ]; then
    pkill -P "$other" afplay 2> /dev/null
    kill "$other" 2> /dev/null
  fi
  osascript -e 'tell application "QuickTime Player" to close (every document whose name contains "'"$name"'") saving no' > /dev/null 2>&1
  # So that macOS never opens the test build in place of the application.
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
    -u "$PWD/$app" 2> /dev/null
}
trap finish EXIT

# The window shared: QuickTime Player with its sound loaded, not playing yet.
open -g -a "QuickTime Player" "$out/$name.aiff"
for _ in $(seq 20); do
  osascript -e 'tell application "QuickTime Player" to count (every document whose name contains "'"$name"'")' 2> /dev/null |
    grep -q '^[1-9]' && break
  sleep 0.5
done
# Another program, playing all along.
(while :; do afplay "$out/other.aiff"; done) &
other=$!
# The test creates the flag once it measured the other program alone.
(
  while [ ! -f "$out/play.flag" ]; do sleep 0.2; done
  osascript -e 'tell application "QuickTime Player" to tell (first document whose name contains "'"$name"'") to set looping to true' \
    -e 'tell application "QuickTime Player" to play (first document whose name contains "'"$name"'")' > /dev/null
) &
watch=$!

FOGATA_CALL="$out" FOGATA_SOUND_WINDOW_NAME="$name" FOGATA_SOUND_FLAG="$out/play.flag" \
  "$app/Contents/MacOS/Fogata" > "$out/app.log" 2>&1 &
test=$!
for _ in $(seq 180); do
  [ -s "$out/sound-result.txt" ] && break
  kill -0 "$test" 2> /dev/null || break
  sleep 1
done
result=$(head -n 1 "$out/sound-result.txt" 2> /dev/null || echo "failed: no result (see $out/app.log)")
grep -h 'sound test:\|FogataScreenSound' "$out/app.log" 2> /dev/null | tail -n 12
echo "$result"
[ "$result" = passed ]

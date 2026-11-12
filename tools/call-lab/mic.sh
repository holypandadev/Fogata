#!/usr/bin/env bash
# Checks the microphone in calls of two copies of the app (Linux), in a sound session of
# their own (session.sh) where only virtual devices exist, and judges each check from what
# the sound server saw each copy record (recordings.log), not only from what the app says:
#   muted    b enters muted and records nothing until it unmutes, and nothing again a
#            moment after it mutes; a hears it only meanwhile
#   switch   a changes its input to a silent one and back; its recording follows
#   unplug   a's microphone goes away in the middle of the call and comes back; a is told
#            both, records from it again, and b hears it
#   broken   a reaches no sound server at all, so its microphone can never open (and WebRTC's
#            voice engine would end the process); the call must not start, a must say why
#            and keep running, still in the room
#   alsa     the same with stand-ins of the computer's ALSA devices in /dev/snd (session.sh
#            CALL_LAB_ALSA=1): listed devices do not change it
#   hang     a's sound server takes the connection and never answers: asked twice (5 s each,
#            a slow one answers the second time), then as for broken
#   late     a's microphone opens 8 s after its room does, so b connects first; b must
#            hear it without a muting and unmuting
#   alone    a is alone in the call for 6 s before b enters (its own level read through a
#            loop of its own meanwhile); b must hear it without a muting and unmuting
#   quit     a quits the application as it enters the room with its microphone on, after
#            QUIT_AFTER ms (default 0); it must end by itself within 15 s, without a crash
#   devices  a lists its sound devices as the settings do around the settings' microphone
#            test, before a call and after it ends, 20 tests and its microphone unplugged
#            and plugged in again: its own must stay listed; and after the 20 tests the
#            video library must hold no more connections, streams or tracks than before.
#            FOGATA_TEST_MIC_SOAK=<n> runs n tests first, logging the memory every 20;
#            FOGATA_TEST_CALL_SOAK=<n> has a leave and rejoin the call n times first;
#            FOGATA_TEST_LEAK=open|closed leaks a connection a test, to see the check fail
#   media    a turns its camera (the made-up one) on and shares the display, b shows both,
#            and a stops both, twice: the video library must then hold no more streams,
#            tracks, cameras' capturers or screen stoppers than before, in the call, and
#            nothing on either copy 5 s after it (a capture left running would show).
#            Needs a debug build: the made-up camera exists only there
# Usage: tools/call-lab/mic.sh <check>...   (default: all of them; quit with QUIT_AFTER=0,
#        300 and 1500). Build first as for measure.sh. Prints PASS or FAIL for each and the
#        run folders; exits non-zero when one failed.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${1:-}" = --inside ]; then
  CHECK=$2
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  lab_start 2
  case "$CHECK" in
    muted) lab_copy a 2 1 FOGATA_TEST_MIC_CASE=muted; lab_copy b 2 off FOGATA_TEST_MIC_CASE=muted ;;
    broken | alsa) lab_copy a 2 broken FOGATA_TEST_MIC_CASE="$CHECK"; lab_copy b 2 off FOGATA_TEST_MIC_CASE="$CHECK" ;;
    hang)
      lab_hanging_server
      lab_copy a 2 hang FOGATA_TEST_MIC_CASE=hang
      lab_copy b 2 off FOGATA_TEST_MIC_CASE=hang
      ;;
    late) lab_copy a 2 1 FOGATA_TEST_MIC_CASE=late FOGATA_TEST_MIC_LATE=8; lab_copy b 2 off FOGATA_TEST_MIC_CASE=late ;;
    media) lab_copy a 2 1 FOGATA_TEST_MIC_CASE=media FOGATA_TEST_CAMERA=1; lab_copy b 2 off FOGATA_TEST_MIC_CASE=media ;;
    quit)
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=quit FOGATA_TEST_QUIT_AFTER="${QUIT_AFTER:-0}"
      lab_copy b 2 off FOGATA_TEST_MIC_CASE=quit
      ;;
    *) lab_copy a 2 1 FOGATA_TEST_MIC_CASE="$CHECK"; lab_copy b 2 off FOGATA_TEST_MIC_CASE="$CHECK" ;;
  esac
  now() { date +%s%3N; }
  # Waits for a file the test writes to meet/; gives up with the copies gone or after $2 s.
  wait_mark() {
    local end=$((SECONDS + $2))
    until [ -e "$M/$1" ]; do
      kill -0 "$(cat "$RUN/a.pid")" 2>/dev/null || return 1
      [ $SECONDS -gt $end ] && return 1
      sleep 0.2
    done
  }
  case "$CHECK" in
    unplug | devices)
      if wait_mark mark-a-unplug-ready 900; then
        lab_unplug a
        now > "$M/unplugged"
        wait_mark mark-a-replug-ready 120
        lab_plug a
        now > "$M/replugged"
      fi
      lab_wait "a b" 300
      ;;
    quit)
      apid=$(cat "$RUN/a.pid")
      wait_mark mark-a-quit 900
      end=$((SECONDS + 15))
      while kill -0 "$apid" 2>/dev/null && [ $SECONDS -lt $end ]; do sleep 0.1; done
      if kill -0 "$apid" 2>/dev/null; then
        echo "still running 15 s after quitting" > "$RUN/quit.txt"
        lab_kill "$apid" "$BUNDLE"
      else
        wait "$apid"
        echo "ended with status $? at $(now)" > "$RUN/quit.txt"
      fi
      ;;
    *) lab_wait "a b" 900 ;;
  esac
  sleep 1
  lab_stop
  exit 0
fi

checks=("$@")
[ ${#checks[@]} -gt 0 ] || checks=(muted switch unplug late alone devices media broken alsa hang quit:0 quit:300 quit:1500)
REPO=$(cd "$HERE/../.." && pwd)
export BUNDLE=${BUNDLE:-$REPO/app/build/linux/x64/debug/bundle/fogata}
[ -x "$BUNDLE" ] || { echo "no build at $BUNDLE" >&2; exit 1; }
failed=0
for check in "${checks[@]}"; do
  name=${check%%:*}
  [ "$name" != "$check" ] && export QUIT_AFTER=${check#*:}
  # A debug build runs from the Dart kernel beside it; a profile or release one has none.
  if [ "$name" = media ] && [ ! -e "$(dirname "$BUNDLE")/data/flutter_assets/kernel_blob.bin" ]; then
    echo "FAIL media"
    echo "  problem: $BUNDLE is not a debug build, and the made-up camera exists only in one:"
    echo "  flutter build linux --debug -t integration_test/call_test.dart"
    failed=1
    continue
  fi
  export RUN=${CALL_LAB_RUNS:-$HOME/call-lab}/mic-${check/:/-}-$(date +%m%d-%H%M%S)
  mkdir -p "$RUN"
  echo "mic $check, $(git -C "$REPO" describe --always --dirty), started $(date -Is)" > "$RUN/settings.txt"
  alsa=""
  [ "$name" = alsa ] && alsa=1
  CALL_LAB_ALSA=$alsa "$HERE/session.sh" "$RUN/session" "$0" --inside "$name"
  python3 -I "$HERE/mic_check.py" "$RUN" "$name" || failed=1
  echo "  $RUN"
done
exit $failed

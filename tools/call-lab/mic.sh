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
#   rejoin   b leaves the room and enters it again REJOIN_TIMES times (default 5) at once,
#            as many after 1 s and after 5 s, then a does the same; both copies have their
#            microphone on, and each time both measure from the moment the one leaving
#            entered until the other's voice reaches them again: any over 6 s fails
#   echo     a shares its screen with the other applications' sound (the lab's own, noise
#            from 5.3 to 6.5 kHz, from pw-cat), b watches it, leaves the room, enters it again and
#            watches it again (ECHO_PAUSES, default "1 10": the seconds out of the room
#            each time; 6 or more also measures a alone meanwhile); they talk in turns
#            (speech-like noise), then each holds a steady tone alone (tone.py turns, a
#            16 s period on the clock), and what each plays goes back into its own
#            microphone as a room's echo would while the other talks (ECHO_ROOM=0 for
#            none). Judged (mic_check.py): neither may hear its own voice come back within
#            10 dB of the other's playing of it (what each plays, recorded for every
#            stretch b marks: an echo canceller off, the screen's sound carrying the
#            call); neither may send the other's voice on (its level after its echo
#            canceller, every 200 ms, call_test.dart's level log) less than 30 dB under its
#            own voice, 20 dB in the other's first turn of each call; each copy's WebRTC
#            must have applied, before every stretch, the processing the application asks
#            for (echo cancelling and gain control on, WebRTC's noise suppressor and
#            high-pass filter off: its log, FOGATA_WEBRTC_LOG, info unless WEBRTC_LOG says
#            otherwise); b must hear a and the screen's sound while it watches, and not while
#            it does not. The steady tones as each copy sends them are told, not judged.
#            ECHO_MONO=1 has the screen's sound go in one channel, to compare. ECHO_FROM
#            names the program whose sound a's screen takes (pw-cat when unset; set and
#            empty, every application's, as on a desktop); CALL_LAB_LOOPBACK=1 adds a
#            loopback of what a plays into another output and a combined output (with
#            ECHO_FROM empty, so they could be taken). ECHO_PROOF=1 has another
#            application play again what a plays (b's voice among it), which a's screen
#            carries back to b: the check must fail with b hearing itself in every
#            stretch it watches, and says so when it does not. PipeWire's graph, every
#            stream's latency and the computer's load are logged once a second
#            (timing.py; ECHO_TIMING=0 for none), and told beside every turn whose echo
#            got through (which fails, whatever came before it); a load over the number
#            of processors fails the run as too busy to judge
#   restart  a and b talk and hear each other; the session's sound server is stopped and
#            started again (restart:<how>, RESTART_HOW): pulse, PipeWire's PulseAudio service
#            alone; all, the three daemons, as `systemctl --user restart pipewire
#            pipewire-pulse wireplumber`; down20, the three, away 20 s; gone, the three, away
#            a minute (meanwhile both must stay in the call with the microphone on and say
#            the sound system stopped). The lab's devices are then made again, as a desktop's
#            come back, one copy's later (RESTART_LATE=<role>:<seconds>, b:8 unless set;
#            empty, none). Within 20 s of that each copy must record from its own microphone
#            again (recordings.log), hear the other (its voice's packets and level) and list
#            its devices; one away over 5 s must be told gone and back
# Usage: tools/call-lab/mic.sh <check>...   (default: all of them; quit with QUIT_AFTER=0,
#        300 and 1500, restart as pulse, all, down20 and gone). Build first as for
#        measure.sh. Prints PASS or FAIL for each and the run folders; exits non-zero when
#        one failed.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${1:-}" = --inside ]; then
  CHECK=$2
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  # The voices are people's, not other applications': a shared screen taking every
  # application's sound must not take them (see lab_tones).
  [ "$CHECK" = echo ] && export LAB_VOICE_BINARY=$(basename "$BUNDLE") LAB_TURNS=1
  lab_start 2
  # A room's echo into each copy's microphone (ECHO_ROOM=0 for none).
  [ "$CHECK" = echo ] && [ "${ECHO_ROOM:-1}" = 1 ] && lab_room_echo 2
  # How the sound server moves the sound along, once a second (ECHO_TIMING=0 for none).
  [ "$CHECK" = echo ] && [ "${ECHO_TIMING:-1}" = 1 ] && lab_timing
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
    rejoin)
      for role in a b; do
        lab_copy "$role" 2 1 FOGATA_TEST_MIC_CASE=rejoin FOGATA_TEST_REJOIN_TIMES="${REJOIN_TIMES:-5}"
      done
      ;;
    echo)
      lab_apps_tone
      [ "${ECHO_PROOF:-}" = 1 ] && lab_echo_proof
      # Each copy's WebRTC log tells the processing it applied to its microphone (judged)
      # and, with WEBRTC_LOG=verbose, what its echo canceller did.
      mono=()
      [ "${ECHO_MONO:-}" = 1 ] && mono=(FOGATA_TEST_SCREEN_SOUND_MONO=1)
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=echo FOGATA_TEST_ECHO_PAUSES="${ECHO_PAUSES:-1 10}" \
        FOGATA_SCREEN_SOUND_FROM="${ECHO_FROM-pw-cat}" FOGATA_WEBRTC_LOG="${WEBRTC_LOG:-info}" \
        "${mono[@]}"
      lab_copy b 2 1 FOGATA_TEST_MIC_CASE=echo FOGATA_TEST_ECHO_PAUSES="${ECHO_PAUSES:-1 10}" \
        FOGATA_WEBRTC_LOG="${WEBRTC_LOG:-info}" "${mono[@]}"
      lab_phase_recorder b "a b"
      ;;
    quit)
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=quit FOGATA_TEST_QUIT_AFTER="${QUIT_AFTER:-0}"
      lab_copy b 2 off FOGATA_TEST_MIC_CASE=quit
      ;;
    restart)
      # a's WebRTC log tells how its audio device lost the server and found it again.
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=restart FOGATA_TEST_RESTART="${RESTART_HOW:-pulse}" \
        FOGATA_WEBRTC_LOG=info
      lab_copy b 2 1 FOGATA_TEST_MIC_CASE=restart FOGATA_TEST_RESTART="${RESTART_HOW:-pulse}" \
        FOGATA_WEBRTC_LOG=info
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
    restart)
      all=(pipewire-pulse wireplumber pipewire)
      if wait_mark mark-a-restart-ready 900 && wait_mark mark-b-restart-ready 120; then
        now > "$M/sound-stopped"
        case "${RESTART_HOW:-pulse}" in
          pulse) lab_sound_stop pipewire-pulse ;;
          *) lab_sound_stop "${all[@]}" ;;
        esac
        case "${RESTART_HOW:-pulse}" in
          down20) sleep 20 ;;
          gone)
            sleep 60
            now > "$M/still-gone"
            # The copies check what they show then.
            sleep 3
            ;;
        esac
        case "${RESTART_HOW:-pulse}" in
          pulse) lab_sound_start pipewire-pulse ;;
          *) lab_sound_start pipewire wireplumber pipewire-pulse ;;
        esac
        now > "$M/sound-back"
        # One copy's devices come back late (RESTART_LATE, <role>:<seconds>, b:8 unless
        # set; empty for none): its audio device starts again on the default first and
        # must move to them once they are there.
        LAB_LATE_DEVICES=${RESTART_LATE-b:8} lab_devices 2
        now > "$M/devices-back"
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
[ ${#checks[@]} -gt 0 ] || checks=(muted switch unplug late alone devices media rejoin echo broken alsa hang quit:0 quit:300 quit:1500 restart:pulse restart:all restart:down20 restart:gone)
REPO=$(cd "$HERE/../.." && pwd)
export BUNDLE=${BUNDLE:-$REPO/app/build/linux/x64/debug/bundle/fogata}
[ -x "$BUNDLE" ] || { echo "no build at $BUNDLE" >&2; exit 1; }
failed=0
for check in "${checks[@]}"; do
  name=${check%%:*}
  if [ "$name" != "$check" ]; then
    case "$name" in
      quit) export QUIT_AFTER=${check#*:} ;;
      restart) export RESTART_HOW=${check#*:} ;;
    esac
  fi
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
  [ "$name" = restart ] && echo "RESTART_HOW=${RESTART_HOW:-pulse} RESTART_LATE=${RESTART_LATE-b:8}" >> "$RUN/settings.txt"
  [ "$name" = echo ] && echo "ECHO_FROM=${ECHO_FROM-pw-cat (unset)} CALL_LAB_LOOPBACK=${CALL_LAB_LOOPBACK:-} ECHO_PROOF=${ECHO_PROOF:-} ECHO_MONO=${ECHO_MONO:-} ECHO_PAUSES=${ECHO_PAUSES:-1 10} ECHO_ROOM=${ECHO_ROOM:-1} WEBRTC_LOG=${WEBRTC_LOG:-info} ECHO_TIMING=${ECHO_TIMING:-1} BUNDLE=$BUNDLE" >> "$RUN/settings.txt"
  alsa=""
  [ "$name" = alsa ] && alsa=1
  CALL_LAB_ALSA=$alsa "$HERE/session.sh" "$RUN/session" "$0" --inside "$name"
  python3 -I "$HERE/mic_check.py" "$RUN" "$name" || failed=1
  echo "  $RUN"
done
exit $failed

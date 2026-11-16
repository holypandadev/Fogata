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
#            hear it without a muting and unmuting. a's WebRTC log (FOGATA_WEBRTC_LOG, info
#            unless WEBRTC_LOG says otherwise) tells whether its recording was stopped and
#            started again as the microphone opened; LAB_FIXED_QUANTUM=0 leaves PipeWire's
#            quantum free (lab.sh), where such a restart can leave a's microphone silent
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
#            of processors fails the run as too busy to judge. ECHO_STALL=<ms>,<ms>... stops
#            the session's PipeWire for those times in turn, every 6 to 9 s, as a busy
#            computer's sound server misses its deadlines (lab.sh's lab_stalls, logged to
#            stalls.log): around a stall the sound server leaves out some of what a copy
#            played or recorded, its echo comes earlier, and with WebRTC's echo canceller as
#            it is a turn is let through now and then (ECHO_STALL=40,80,150,300: 13 of 103
#            turns in three runs, none of 763 without it)
#   noise    a enters with noise suppression on and its microphone on, a fan's steady
#            noise in it (tone.py fan), and b with its microphone on a silent source, so
#            that a plays nothing; a never mutes. (With b's tone playing at a, a's echo
#            canceller, made anew at each start of its recording, held the noise ~23 dB
#            down for its first seconds and let it go at once about 6 s after b's sound
#            began: on a runner just as noise suppression was turned off, and RNNoise,
#            which takes such a return for speech, was left as it heard it.) a's level as
#            its processing puts it out (call_test.dart's level log) is taken over four
#            stretches of NOISE_SECONDS (default 6): as it entered, with noise suppression
#            turned off, with it on again, and after muting and unmuting. Judged
#            (mic_check.py, by each stretch's median): the noise must be 10 dB or more lower in each of the three
#            stretches with noise suppression on than with it off, from the entry on (a
#            processing that libwebrtc left off until the microphone was muted and
#            unmuted let it through)
#   unmute   a turns its microphone off and on as the call's button does, off for each of
#            UNMUTE_OFFS seconds in turn (default "2 10 60") and on for UNMUTE_ON (10)
#            after each, once its processing had UNMUTE_SETTLE seconds (10) on the room
#            alone; b listens, its microphone off. a's microphone has a room's noise
#            throughout, at UNMUTE_NOISE_DB dBFS RMS (-52: a fan, its hum and a hiss), and a
#            few words (UNMUTE_WORDS_DB, -28) 0.5, 3.5 and 6.5 s after each unmute, all
#            made with ffmpeg's filters (unmute-sound.sh; ffmpeg is needed);
#            UNMUTE_MIC_VOLUME sets its virtual microphone's volume (a percentage),
#            UNMUTE_WORDS_AT the words' moments in ms ("500 3500 6500"). UNMUTE_FAR=words or
#            tone: b talks meanwhile (the same words every 3 s, or a steady tone of 220 and
#            660 Hz), its microphone on; UNMUTE_ROOM=1: what a plays goes back into a's
#            microphone 30 ms later, as from speakers (without it, as with a headset);
#            UNMUTE_ECHO=1 is both, with words.
#            What b plays and what a's microphone hears are recorded throughout. Measured,
#            not judged (unmute.py): how loud b played the first 100, 300, 1000 and 2500 ms
#            after the sound resumed, against the same windows 3 s and 6 s later, and one
#            clip of each unmute, 2 s before to 4 s after. UNMUTE_TRIALS sets a's WebRTC
#            field trials (FOGATA_WEBRTC_FIELD_TRIALS, debug builds), and UNMUTE_NS=1 has a
#            enter with noise suppression on.
#            unmute:headset and unmute:speakers are judged (unmute_check.py): b holds a tone
#            (UNMUTE_FAR=tone), a turns its microphone off for 2, 10 and 30 s and on for 16 s
#            after each, with words 0.5, 3.5, 6.5, 9.5 and 12.5 s after each unmute; without
#            and with a room's echo. With a headset, a's words and the room's noise must go
#            out as they do later in the call (a new echo canceller held the microphone down
#            until it had heard 6 s of b without echo, then let go at once); with speakers,
#            b's tone must not go back to it at an unmute
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
# Usage: tools/call-lab/mic.sh <check>...   (default: all of them but unmute; quit with QUIT_AFTER=0,
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
  [ "$CHECK" = noise ] && export LAB_FAN=a
  [ "$CHECK" = unmute ] && export LAB_NO_TONE="a b"
  lab_start 2
  # A room's noise and a few words into a's microphone, and what b plays and a's
  # microphone hears recorded throughout (mic.sh unmute).
  [ "$CHECK" = unmute ] && lab_unmute_sound
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
    late)
      # a's WebRTC log tells whether its recording was stopped and started again as its
      # microphone opened.
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=late FOGATA_TEST_MIC_LATE=8 FOGATA_WEBRTC_LOG="${WEBRTC_LOG:-info}"
      lab_copy b 2 off FOGATA_TEST_MIC_CASE=late
      ;;
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
      [ -n "${ECHO_STALL:-}" ] && lab_stalls "$ECHO_STALL"
      ;;
    noise)
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=noise FOGATA_TEST_NOISE_SECONDS="${NOISE_SECONDS:-6}" \
        FOGATA_WEBRTC_LOG="${WEBRTC_LOG:-info}"
      lab_copy b 2 off FOGATA_TEST_MIC_CASE=noise
      ;;
    unmute)
      steps=(FOGATA_TEST_UNMUTE_OFFS="${UNMUTE_OFFS:-2 10 60}" FOGATA_TEST_UNMUTE_ON="${UNMUTE_ON:-10}"
        FOGATA_TEST_UNMUTE_SETTLE="${UNMUTE_SETTLE:-10}")
      tried=()
      [ -n "${UNMUTE_TRIALS:-}" ] && tried+=(FOGATA_WEBRTC_FIELD_TRIALS="$UNMUTE_TRIALS")
      [ "${UNMUTE_NS:-}" = 1 ] && tried+=(FOGATA_TEST_UNMUTE_NS=1)
      lab_copy a 2 1 FOGATA_TEST_MIC_CASE=unmute "${steps[@]}" FOGATA_WEBRTC_LOG="${WEBRTC_LOG:-info}" "${tried[@]}"
      bmic=off
      { [ "${UNMUTE_ECHO:-}" = 1 ] || [ -n "${UNMUTE_FAR:-}" ]; } && bmic=1
      lab_copy b 2 "$bmic" FOGATA_TEST_MIC_CASE=unmute "${steps[@]}"
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
    unmute)
      lab_wait "a b" 1800
      lab_unmute_stop
      ;;
    *) lab_wait "a b" 900 ;;
  esac
  sleep 1
  lab_stop
  exit 0
fi

checks=("$@")
[ ${#checks[@]} -gt 0 ] || checks=(muted switch unplug late alone devices media rejoin echo noise broken alsa hang quit:0 quit:300 quit:1500 restart:pulse restart:all restart:down20 restart:gone)
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
      unmute)
        # The judged forms: b holds a tone, a's words 0.5 to 12.5 s after each unmute.
        export UNMUTE_JUDGE=${check#*:} UNMUTE_FAR=tone UNMUTE_OFFS="2 10 30" UNMUTE_ON=16 \
          UNMUTE_WORDS_AT="500 3500 6500 9500 12500" UNMUTE_ECHO=
        case "$UNMUTE_JUDGE" in
          headset) export UNMUTE_ROOM= ;;
          speakers) export UNMUTE_ROOM=1 ;;
          *) echo "unknown check $check" >&2; exit 2 ;;
        esac
        ;;
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
  [ "${LAB_FIXED_QUANTUM:-1}" = 0 ] && echo "LAB_FIXED_QUANTUM=0" >> "$RUN/settings.txt"
  [ "$name" = restart ] && echo "RESTART_HOW=${RESTART_HOW:-pulse} RESTART_LATE=${RESTART_LATE-b:8}" >> "$RUN/settings.txt"
  [ "$name" = unmute ] && echo "UNMUTE_JUDGE=${UNMUTE_JUDGE:-} UNMUTE_OFFS=${UNMUTE_OFFS:-2 10 60} UNMUTE_ON=${UNMUTE_ON:-10} UNMUTE_SETTLE=${UNMUTE_SETTLE:-10} UNMUTE_NOISE_DB=${UNMUTE_NOISE_DB:--52} UNMUTE_WORDS_DB=${UNMUTE_WORDS_DB:--28} UNMUTE_MIC_VOLUME=${UNMUTE_MIC_VOLUME:-} UNMUTE_TRIALS=${UNMUTE_TRIALS:-} UNMUTE_NS=${UNMUTE_NS:-} UNMUTE_WORDS_AT=${UNMUTE_WORDS_AT:-500 3500 6500} UNMUTE_ECHO=${UNMUTE_ECHO:-} UNMUTE_FAR=${UNMUTE_FAR:-} UNMUTE_ROOM=${UNMUTE_ROOM:-} BUNDLE=$BUNDLE" >> "$RUN/settings.txt"
  [ "$name" = unmute ] && echo "${UNMUTE_OFFS:-2 10 60}" > "$RUN/offs.txt"
  [ "$name" = echo ] && echo "ECHO_FROM=${ECHO_FROM-pw-cat (unset)} CALL_LAB_LOOPBACK=${CALL_LAB_LOOPBACK:-} ECHO_PROOF=${ECHO_PROOF:-} ECHO_MONO=${ECHO_MONO:-} ECHO_PAUSES=${ECHO_PAUSES:-1 10} ECHO_ROOM=${ECHO_ROOM:-1} WEBRTC_LOG=${WEBRTC_LOG:-info} ECHO_TIMING=${ECHO_TIMING:-1} ECHO_STALL=${ECHO_STALL:-} BUNDLE=$BUNDLE" >> "$RUN/settings.txt"
  alsa=""
  [ "$name" = alsa ] && alsa=1
  CALL_LAB_ALSA=$alsa "$HERE/session.sh" "$RUN/session" "$0" --inside "$name"
  if [ "$name" = unmute ]; then
    if [ -n "${UNMUTE_JUDGE:-}" ] && [ "$check" != unmute ]; then
      python3 -I "$HERE/unmute.py" "$RUN" "$UNMUTE_JUDGE" > "$RUN/unmute-measured.txt" 2>&1
      python3 -I "$HERE/unmute_check.py" "$RUN" "$UNMUTE_JUDGE" || failed=1
    else
      python3 -I "$HERE/unmute.py" "$RUN" today || failed=1
    fi
  else
    python3 -I "$HERE/mic_check.py" "$RUN" "$name" || failed=1
  fi
  echo "  $RUN"
done
exit $failed

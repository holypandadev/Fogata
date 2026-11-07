import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart' show WebRTC, navigator;

import 'announce.dart';
import 'app_state.dart';
import 'host.dart';
import 'l10n.dart';
import 'rust/api/direct.dart' as dm;
import 'rust/api/voice.dart' as api;
import 'shown.dart';

/// The speeds a voice message plays at, in the order its pill goes through them.
const voiceSpeeds = [1.0, 1.5, 2.0];

/// Longest voice message, as the core keeps to: 15 minutes.
const voiceMaxMs = 15 * 60 * 1000;

/// Up to how long a recording Esc deletes; a longer one takes the Delete button, so a key
/// pressed by habit never throws away minutes of talk.
const voiceEscDeletesMs = 5000;

/// From when the time left shows while recording: the last minute.
const voiceLastMinuteMs = voiceMaxMs - 60 * 1000;

/// `0:18`, `14:12`: a voice message's length or time.
String voiceTime(int ms) {
  final seconds = (ms / 1000).floor();
  return '${seconds ~/ 60}:${(seconds % 60).toString().padLeft(2, '0')}';
}

/// Voice messages in direct chats as the window has them: the one being recorded, in
/// whose chat, and the one playing. One records and one plays at a time. What is
/// recorded and played is the core's (`rust/api/voice.dart`); this asks it a few times a
/// second while something goes on.
class VoiceMessages extends Piece {
  VoiceMessages(this.app) {
    windowShown.addListener(_window);
  }

  final AppState app;

  /// The chat a recording belongs to, while there is one.
  String? recordingFor;

  /// The recording as last read.
  api.UiVoiceRecording? recording;

  /// Starting to record, before the microphone opened.
  bool starting = false;

  /// The microphone could not be used: the chat says so over the message box.
  bool micProblem = false;

  /// The microphone stopped part way through the recording: the chat says so over the
  /// recording bar.
  bool get micStopped => recording?.failed ?? false;

  /// The name of the microphone recording, as the settings list it.
  String micName = '';

  /// The microphone chosen in the settings could not be used: the system's default
  /// records, and the line under the recording bar says so.
  bool fellBack = false;

  /// Sending the recording.
  bool sending = false;

  /// The message playing or paused, as last read.
  api.UiVoicePlayback? playback;

  /// The speed each message of the chat shown plays at; forgotten with the chat.
  final Map<String, double> speeds = {};

  Timer? _poll;

  /// Counts the starts and deletions: a start whose microphone opens after the recording
  /// was deleted sees it changed.
  int _attempt = 0;

  /// The recording was to wait with the microphone off (its chat was left) while the
  /// microphone still opened: it stops as soon as it opens.
  bool _pauseOnOpen = false;

  void _changed() => changed();

  /// Whether recording is not possible now: in a direct call or a voice room the
  /// microphone belongs to the call.
  bool get inCall =>
      app.callState.call != null || app.callState.activeCall != null;

  /// Whether the chat with `friend` shows the recording bar.
  bool recordingIn(String friend) =>
      recordingFor == friend && (recording != null || starting);

  /// Starts recording a message to `friend` from the microphone chosen in the settings;
  /// what plays stops. While a recording waits in another chat, none starts: the person is
  /// told to send or delete that one first, so it is never thrown away unasked.
  Future<void> start(String friend) async {
    if (starting || inCall) return;
    if (recordingFor case final other? when other != friend) {
      app.say(l10n.directVoiceWaitingElsewhere(_name(other)));
      return;
    }
    stopPlaying();
    final attempt = ++_attempt;
    _pauseOnOpen = false;
    recordingFor = friend;
    starting = true;
    micProblem = false;
    micName = l10n.devicesSystemDefault;
    fellBack = false;
    _changed();
    unawaited(_nameMic());
    try {
      final (input, missing) = await _input();
      final defaultInstead = await api.voiceRecordStart(
        input: input,
        noiseSuppression: app.prefs.noiseSuppression,
      );
      fellBack = missing || defaultInstead;
    } catch (e) {
      // Deleted while the microphone opened: the core closed it again.
      if (attempt != _attempt) return;
      debugPrint('voice: cannot record: $e');
      starting = false;
      recordingFor = null;
      micProblem = true;
      _changed();
      return;
    }
    if (attempt != _attempt) {
      // Deleted while the microphone opened, and no other recording began since: the
      // core closes a microphone that opens late, and this makes sure.
      if (recordingFor == null) {
        try {
          api.voiceRecordCancel();
        } catch (_) {}
      }
      return;
    }
    starting = false;
    recording = api.voiceRecording();
    _keepPolling();
    _changed();
    if (_pauseOnOpen) {
      pauseRecording();
    } else {
      announce(
        fellBack
            ? l10n.directVoiceRecordingDefault
            : l10n.directVoiceRecordingFrom(micName),
      );
    }
  }

  /// The microphone chosen in the settings as the core opens it, and whether it is known
  /// not to be there. On Linux the voice engine names inputs by their description, and
  /// the core opens them by the sound server's own name, which the voice engine's side
  /// looks up: no name, and the system's default records.
  Future<(String?, bool)> _input() async {
    final id = app.prefs.audioInput;
    if (id == null || !onLinux) return (id, false);
    try {
      final name = await WebRTC.invokeMethod<String, dynamic>(
        'fogataInputName',
        {'deviceId': id},
      );
      if (name != null && name.isNotEmpty) return (name, false);
    } catch (e) {
      debugPrint('voice: no name for the microphone: $e');
      return (id, false);
    }
    return (null, true);
  }

  /// Finds the chosen microphone's name for the line under the recording bar.
  Future<void> _nameMic() async {
    final id = app.prefs.audioInput;
    if (id == null) return;
    try {
      for (final d in await navigator.mediaDevices.enumerateDevices()) {
        if (d.kind == 'audioinput' &&
            d.deviceId == id &&
            d.label.trim().isNotEmpty) {
          micName = d.label.trim();
          _changed();
          return;
        }
      }
    } catch (_) {}
  }

  /// The name of the friend of chat `friend`, as the chats list it.
  String _name(String friend) {
    for (final c in app.direct.chats) {
      if (c.friend.id == friend) return c.friend.name;
    }
    for (final f in app.contacts.friends) {
      if (f.person.id == friend) return f.person.name;
    }
    return '';
  }

  /// Throws the recording away, without asking; one whose microphone is still opening
  /// too.
  void cancel() {
    if (recordingFor == null) return;
    _attempt++;
    try {
      api.voiceRecordCancel();
    } catch (_) {}
    recordingFor = null;
    recording = null;
    starting = false;
    _changed();
  }

  /// Deletes the recording, as Delete and Esc do: not while it is being sent, when it is
  /// already on its way.
  void delete() {
    if (sending) return;
    cancel();
  }

  /// Whether Esc deletes the recording: only a short one ([voiceEscDeletesMs]).
  bool get escDeletes => (recording?.elapsedMs ?? 0) < voiceEscDeletesMs;

  /// Esc while recording: deletes a short recording; a longer one stays.
  void escape() {
    if (escDeletes) delete();
  }

  /// Stops the microphone and keeps the recording waiting for Send or Delete, as when the
  /// chat is left, the window is hidden or minimized, or a call starts.
  void pauseRecording() {
    if (starting) {
      _pauseOnOpen = true;
      return;
    }
    if (recording?.capturing ?? false) {
      try {
        api.voiceRecordStop();
        recording = api.voiceRecording();
      } catch (_) {}
      _changed();
    }
  }

  /// Sends the recording to the friend of its chat. One under half a second is not sent:
  /// it goes, and a toast says it was too short.
  Future<void> send() async {
    final friend = recordingFor;
    if (friend == null || sending || starting) return;
    sending = true;
    _changed();
    final sent = await app.attempt(() => api.voiceRecordSend(deviceId: friend));
    sending = false;
    if (sent != null && recordingFor == friend) {
      if (!sent) app.say(l10n.directVoiceTooShort);
      recordingFor = null;
      recording = null;
    } else if (sent == null) {
      recording = api.voiceRecording();
    }
    _changed();
    await app.direct.refresh();
  }

  void closeMicProblem() {
    micProblem = false;
    _changed();
  }

  /// The speed message `file` plays at.
  double speedOf(String file) => speeds[file] ?? 1.0;

  /// Whether message `file` is playing now.
  bool playing(String file) =>
      playback?.file == file && (playback?.playing ?? false);

  /// Where message `file` is, while playing or paused; null otherwise.
  int? positionOf(String file) =>
      playback?.file == file ? playback!.positionMs : null;

  /// Plays message `file` of the chat with `friend`, or pauses it while it plays; another
  /// that played stops. A friend's message not played yet loses its dot.
  Future<void> toggle(String friend, dm.UiDirectFile file) async {
    if (playing(file.id)) {
      try {
        api.voicePause();
      } catch (_) {}
      playback = api.voicePlayback();
      _changed();
      return;
    }
    if (recording != null) pauseRecording();
    final played = await app.succeeds(
      () => api.voicePlay(
        deviceId: friend,
        file: file.id,
        speed: speedOf(file.id),
      ),
    );
    if (!played) return;
    playback = api.voicePlayback();
    _keepPolling();
    _changed();
    if (file.voice?.played == false) {
      try {
        await dm.markDirectPlayed(deviceId: friend, file: file.id);
      } catch (_) {}
    }
  }

  /// Goes to the next speed for message `file`, also while it plays.
  void nextSpeed(String file) {
    final now = speedOf(file);
    final next =
        voiceSpeeds[(voiceSpeeds.indexOf(now) + 1) % voiceSpeeds.length];
    speeds[file] = next;
    if (playback?.file == file) {
      try {
        api.voiceSetSpeed(speed: next);
      } catch (_) {}
      playback = api.voicePlayback();
    }
    _changed();
  }

  /// Stops what plays and forgets where each message was and its speed, as when the chat
  /// closes.
  void stopPlaying() {
    speeds.clear();
    if (playback == null) return;
    try {
      api.voiceStop();
    } catch (_) {}
    playback = null;
    _changed();
  }

  /// The chat with `friend` was read again, as `now`: a message playing or paused that is
  /// gone from it (deleted, or its timer ran out) or whose file is no longer here stops.
  /// One `now` does not hold (more newer messages came than a page holds) is asked of the
  /// core, which holds the whole chat.
  Future<void> chatRead(String friend, dm.UiDirectPage now) async {
    final file = playback?.file;
    if (file == null || playback?.deviceId != friend) return;
    final read = now.items.expand((i) => i.files).where((f) => f.id == file);
    final bool kept;
    if (read.isNotEmpty) {
      kept = read.first.state == dm.UiFileState.here;
    } else if (!now.more) {
      kept = false;
    } else {
      try {
        kept = await dm.directFileKept(deviceId: friend, file: file);
      } catch (_) {
        return;
      }
    }
    // Still the one asked about: another may have started, or this one stopped, meanwhile.
    if (kept || playback?.file != file || playback?.deviceId != friend) return;
    try {
      api.voiceStop();
    } catch (_) {}
    playback = null;
    _changed();
  }

  /// The chat with `friend` is no longer shown: what plays stops, and its recording waits
  /// with the microphone off.
  void leftChat(String friend) {
    if (playback?.deviceId == friend) stopPlaying();
    if (recordingFor == friend) pauseRecording();
  }

  /// Nobody sees a window hidden in the tray or minimized, nor the recording bar in it:
  /// the recording waits with the microphone off, as when its chat is left.
  void _window() {
    if (!windowShown.value && recordingFor != null) pauseRecording();
  }

  /// Reads how the recording and the playing go, a few times a second while either goes.
  void _keepPolling() {
    _poll ??= Timer.periodic(const Duration(milliseconds: 80), (_) => _read());
  }

  void _read() {
    var going = false;
    if (recordingFor != null && !starting) {
      final now = api.voiceRecording();
      recording = now;
      if (now == null) {
        recordingFor = null;
      } else if (now.failed && now.elapsedMs < 2500) {
        // Nothing, or only silence, came from the microphone: as when it does not open.
        cancel();
        micProblem = true;
      } else if (now.failed) {
        // The microphone stopped part way: what was recorded waits, and the chat says so
        // over the recording bar.
      } else if (now.capturing) {
        going = true;
        // A call took the microphone.
        if (inCall) pauseRecording();
      }
    }
    if (playback != null) {
      final now = api.voicePlayback();
      playback = now;
      going |= now?.playing ?? false;
    }
    if (!going) {
      _poll?.cancel();
      _poll = null;
    }
    _changed();
  }

  @override
  void dispose() {
    windowShown.removeListener(_window);
    _poll?.cancel();
    super.dispose();
  }
}

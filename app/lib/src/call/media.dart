part of '../call.dart';

/// Whether a device's id names the system's default rather than a device of its own, as
/// libwebrtc lists it beside the devices: "default: " and the device's name on Linux
/// (PulseAudio, first in the list), "default" and "communications" on Windows (Core
/// Audio) and "default" on macOS; an empty id is no device at all. The app has its own
/// "System default" for that, and an id like these chosen before is taken as it.
bool defaultAlias(String? id) =>
    id != null &&
    (id.isEmpty ||
        id == 'default' ||
        id == 'communications' ||
        id.startsWith('default: '));

/// Sends the application's own sounds (a ring, someone coming in, a message) and its chat
/// videos to the output [id] chosen in the settings, as calls already go there; null is
/// the system's default. Linux's voice engine moves this process's sounds to it; the
/// short sounds' player opens it elsewhere.
Future<void> soundsTo(String? id) async {
  try {
    api.setSoundOutput(id: id);
  } catch (e) {
    _trace('sound output: $e');
  }
  if (!onLinux) return;
  try {
    await WebRTC.invokeMethod<bool, dynamic>('fogataSoundsTo', {
      'deviceId': id ?? '',
    });
  } catch (e) {
    _trace('sound output: $e');
  }
}

/// Opening a camera found none on this computer.
class NoCamera implements Exception {
  @override
  String toString() => 'no camera';
}

/// The system does not let the application use the cameras: on Linux, through the
/// Camera portal (in a Flatpak), the person said no, now or before.
class CameraDenied implements Exception {
  @override
  String toString() => 'camera denied';
}

/// The cameras come through the Camera portal (Linux, in a Flatpak) and the system has
/// none, or one without cameras (xdg-desktop-portal's Camera interface and PipeWire).
class CameraUnavailable implements Exception {
  @override
  String toString() => 'no camera portal';
}

/// How the cameras are reached: opened directly by WebRTC, or (Linux, in a Flatpak)
/// through the system's Camera portal, which asks the person once for all of them.
enum CameraAccess { direct, notAsked, asking, granted, denied, unavailable }

/// Reads how the cameras are reached and, through the portal, whether they were granted;
/// with [request], asks for them first unless granted, which may show the system's
/// question (the plugin answers once the person did). Elsewhere, and if the plugin cannot
/// tell, they are reached directly.
Future<CameraAccess> cameraAccess({bool request = false}) =>
    readCameraAccess(request);

/// The plugin's answer; tests give their own.
@visibleForTesting
Future<CameraAccess> Function(bool request) readCameraAccess = _pluginAccess;

Future<CameraAccess> _pluginAccess(bool request) async {
  if (!onLinux) return CameraAccess.direct;
  try {
    final name = await WebRTC.invokeMethod<String, dynamic>(
      'fogataCameraAccess',
      {'request': request},
    );
    return CameraAccess.values.firstWhere(
      (a) => a.name == name,
      orElse: () => CameraAccess.direct,
    );
  } catch (e) {
    _trace('camera access: $e');
    return CameraAccess.direct;
  }
}

/// What to tell when no camera is listed with [access]: none is plugged in, unless the
/// cameras could not be reached at all.
String noCameraNote(CameraAccess access) => switch (access) {
  CameraAccess.denied => l10n.devicesCameraDenied,
  CameraAccess.unavailable => l10n.devicesCameraUnavailable,
  _ => l10n.devicesNoCamera,
};

/// What to tell when opening a camera failed with [error].
String cameraFailure(Object? error) => switch (error) {
  NoCamera() => l10n.devicesNoCamera,
  CameraDenied() => l10n.devicesCameraDenied,
  CameraUnavailable() => l10n.devicesCameraUnavailable,
  _ => l10n.devicesCameraFailed(_system),
};

/// Opens camera [id] (the system's default when null) as a call sends it; the settings'
/// preview opens it the same way. Throws [NoCamera] when the computer has none, and
/// [CameraDenied] or [CameraUnavailable] when the system's Camera portal did not give
/// the cameras (Linux, in a Flatpak: the plugin asks it first, which may show the
/// system's question).
Future<MediaStream> openCameraStream(String? id) async {
  final MediaStream stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      'audio': false,
      // Plain numbers: macOS ignores numbers given as {'ideal': n}.
      'video': {
        'width': 1280,
        'height': 720,
        'frameRate': 30,
        if (id != null)
          'optional': [
            {'sourceId': id},
          ],
      },
    });
  } catch (e) {
    // What macOS answers without a camera.
    if ('$e'.contains('NotFoundError')) throw NoCamera();
    // Linux's plugin, the cameras coming through the Camera portal.
    if ('$e'.contains('fogataCameraDenied')) throw CameraDenied();
    if ('$e'.contains('fogataCameraUnavailable')) throw CameraUnavailable();
    rethrow;
  }
  // Linux and Windows answer with a stream that has no picture.
  if (stream.getVideoTracks().isEmpty) {
    await stream.dispose();
    throw NoCamera();
  }
  return stream;
}

/// Opens microphone [input] (the system's default when null) as a call records it: with
/// WebRTC's echo cancelling and gain control, and without its own noise suppressor
/// (RNNoise does that job after them, when on). The settings' microphone test opens it the
/// same way.
Future<MediaStream> openMicStream(String? input) =>
    navigator.mediaDevices.getUserMedia({
      'audio': {
        ..._micProcessing,
        if (input != null)
          'optional': [
            {'sourceId': input},
          ],
      },
      'video': false,
    });

/// Whether the microphone, on, reads anything. Where the system refuses it (Windows'
/// privacy switch off) or its device cannot start, WebRTC opens nothing and says nothing:
/// the call shows the microphone on and sends no voice at all. Judged only while some
/// connection has agreed to send voice (its channel's current direction sends: one whose
/// voice channel only received while muted waits for the new description at an unmute,
/// which may take a resend), from what the device gives (the seconds of sound read, the
/// `media-source`'s `totalSamplesDuration`) and what goes out (each such connection's
/// voice packets; silence still goes out now and then, Opus DTX): [after] with neither
/// growing, from one reading to the next, reads nothing. So a device that stops later
/// (another input chosen that cannot be opened) is caught too. Told once until the
/// microphone is turned off and on again ([reset]).
@visibleForTesting
class MicCheck {
  static const after = Duration(seconds: 8);

  DateTime? _since;
  bool _told = false;
  double? _read;
  Map<String, num> _sent = const {};

  /// Starts over, as for the microphone turned on again or another one chosen.
  void reset() {
    _since = null;
    _told = false;
    _read = null;
    _sent = const {};
  }

  /// Takes a reading: the microphone is [on]; some connection agreed to send voice
  /// ([sending]); the seconds of sound read so far ([read], null when not known) and the
  /// voice packets sent so far on each sending connection ([sent], by connection). Returns
  /// true once when the microphone reads nothing.
  bool take({
    required bool on,
    required bool sending,
    required double? read,
    required Map<String, num> sent,
    required DateTime now,
  }) {
    if (!on) {
      reset();
      return false;
    }
    final lastRead = _read;
    final lastSent = _sent;
    _read = read;
    _sent = sent;
    if (!sending || (read == null && sent.isEmpty)) {
      _since = null;
      return false;
    }
    final grew =
        (read != null && lastRead != null && read > lastRead) ||
        sent.entries.any((e) {
          final last = lastSent[e.key];
          return last != null && e.value > last;
        });
    if (grew) {
      _since = now;
      return false;
    }
    final since = _since ??= now;
    if (_told || now.difference(since) < after) return false;
    _told = true;
    return true;
  }
}

/// Whether WebRTC's voice engine reaches the sound server, from the devices it lists
/// (Linux): while it reaches none (PipeWire or PulseAudio restarting, as after a system
/// update) it lists no input and no output at all, not even the system's default, and
/// plays and records nothing; it connects again by itself and starts again as it was
/// (flutter_webrtc's FOGATA.md, "A sound server that goes away and comes back"). The
/// person is told once it has been gone for [tellAfter], and then told when it is back:
/// a server restarting is back within a moment, with nothing to tell.
@visibleForTesting
class SoundServerWatch {
  static const tellAfter = Duration(seconds: 5);

  /// When the lists were first seen without a device, while they still are.
  DateTime? lostSince;

  /// The person was told it is gone.
  bool told = false;

  bool get lost => lostSince != null;

  /// Takes the devices listed [now], on Linux ([linux]; elsewhere a list may well be
  /// empty, and says nothing of a server); returns what to tell, or null.
  String? take(
    List<MediaDeviceInfo> devices, {
    required bool linux,
    required DateTime now,
  }) {
    final listed = devices.any(
      (d) => d.kind == 'audioinput' || d.kind == 'audiooutput',
    );
    if (linux && !listed) {
      final since = lostSince ??= now;
      if (told || now.difference(since) < tellAfter) return null;
      told = true;
      return l10n.callSoundSystemLost;
    }
    final wasTold = told;
    lostSince = null;
    told = false;
    return wasTold ? l10n.callSoundSystemBack : null;
  }
}

/// Whether the microphone chosen for the call ([take]'s `input`) is among the inputs the
/// system lists, from one list to the next: when it goes away (a headset unplugged) WebRTC
/// records from the system's default input until it comes back, and the person is told
/// both, once each. With no microphone chosen (the system's default) there is nothing to
/// tell.
@visibleForTesting
class MicWatch {
  /// The microphone chosen is gone now.
  bool gone = false;

  /// The microphone chosen at the last list: another one chosen since starts over, so
  /// its being there is not told as the last one coming back.
  String? _input;

  /// Takes the inputs listed now; returns what to tell, or null.
  String? take(String? input, List<MediaDeviceInfo> devices) {
    final inputs = devices.where((d) => d.kind == 'audioinput').toList();
    if (input != _input) {
      _input = input;
      gone = false;
    }
    if (input == null || input.isEmpty) {
      gone = false;
      return null;
    }
    final present = inputs.any((d) => d.deviceId == input);
    if (!present && !gone) {
      gone = true;
      return l10n.callMicGone(defaultInputName(inputs));
    }
    if (present && gone) {
      gone = false;
      return l10n.callMicBack;
    }
    return null;
  }

  /// The name of the system's default input, as the list gives it first ("default:
  /// Built-in Microphone" on Linux, "Default - …" on Windows), or "System default".
  static String defaultInputName(List<MediaDeviceInfo> inputs) {
    final label = inputs.firstOrNull?.label.trim() ?? '';
    final name = label
        .replaceFirst(RegExp(r'^default\s*[:\-]\s*', caseSensitive: false), '')
        .trim();
    return name.isEmpty ? l10n.devicesSystemDefault : name;
  }
}

/// What a device watch makes of the voice engine's facts (macOS: flutter_webrtc's method
/// `fogataAudioOutput`): what to tell, if anything, and the device in use when it is not
/// the one chosen (its id, 'default' for the system's), to show beside the choice.
typedef DeviceTold = ({String? text, String? inUse});

const DeviceTold _nothing = (text: null, inUse: null);

/// The id of a device chosen as the engine's facts and the call give it: null for the
/// system's default.
String? _chosenId(Object? id) => id is String && id != 'default' ? id : null;

/// A device's name: as the engine named it ([named]), else as listed now ('default': the
/// first listed of [kind], Call._first), else its id.
String _deviceName(
  List<MediaDeviceInfo> devices,
  String kind,
  String id,
  Object? named,
) {
  if (named is String && named.trim().isNotEmpty) return named.trim();
  final listed = devices.where((d) => d.kind == kind).toList();
  if (id == 'default') return MicWatch.defaultInputName(listed);
  final label = listed.where((d) => d.deviceId == id).firstOrNull?.label.trim();
  return label == null || label.isEmpty ? id : label;
}

/// What a device chosen is, from the voice engine's facts: in use ('ok'), another in use
/// while it is there ('other'), or not there ('gone'). Another in use is taken only when
/// seen twice in a row (4 s): a device plugged in again is given back to the engine within
/// a second or two (FOGATA.md, R6), and a moment between says nothing. Facts about another
/// choice than the call's last ([take]'s `asked`), or taken while a choice is carried out,
/// say nothing; back on the one chosen, a later change is told again.
class _ChosenWatch {
  _ChosenWatch(this.prefix);

  /// The facts' prefix for the device: '' for the output, 'input' for the input.
  final String prefix;

  String? _for;
  String _was = 'ok';
  bool _otherOnce = false;

  String _key(String name) => prefix.isEmpty
      ? name
      : '$prefix${name[0].toUpperCase()}${name.substring(1)}';

  /// What changed: (what it is now, the device in use when another, whether it is new).
  ({String now, String? inUse, bool began})? take(
    Object? facts,
    String? asked,
  ) {
    if (facts is! Map) return null;
    final chosen = _chosenId(facts[prefix.isEmpty ? 'chosen' : 'inputChosen']);
    if (chosen != _chosenId(asked)) return null;
    if (chosen != _for) {
      _for = chosen;
      _was = 'ok';
      _otherOnce = false;
    }
    if (chosen == null || facts['changing'] == true) return null;
    final inUse = facts[_key('inUse')];
    var now = _was;
    if (facts[prefix.isEmpty ? 'chosenPresent' : 'inputChosenPresent'] ==
        false) {
      now = 'gone';
    } else if (inUse is String) {
      now = inUse == chosen ? 'ok' : 'other';
    }
    if (now == 'other' && _was != 'other' && !_otherOnce) {
      _otherOnce = true;
      return (now: _was, inUse: null, began: false);
    }
    _otherOnce = false;
    final began = now != _was && now != 'ok';
    _was = now;
    return (
      now: now,
      inUse: now == 'other' && inUse is String ? inUse : null,
      began: began,
    );
  }
}

/// The output chosen, from the voice engine's facts every 2 s (flutter_webrtc's FOGATA.md,
/// "The devices chosen, with the microphone"): the engine plays on the system's output when
/// the one chosen cannot be put together with the microphone, or the engine refused it,
/// and when it is unplugged. Each time either begins it is told ("… with this microphone"
/// when the facts say the rates differ), and the output in use is shown beside the choice,
/// which stays the person's.
@visibleForTesting
class OutputWatch {
  final _watch = _ChosenWatch('');

  DeviceTold take(
    Object? facts,
    List<MediaDeviceInfo> devices, {
    required String? asked,
  }) {
    final seen = _watch.take(facts, asked);
    if (seen == null || facts is! Map) return _nothing;
    String? text;
    if (seen.began) {
      final chosen = _chosenId(facts['chosen'])!;
      final was = _deviceName(
        devices,
        'audiooutput',
        chosen,
        facts['chosenName'],
      );
      final inUse = facts['inUse'];
      final where = inUse is String
          ? _deviceName(devices, 'audiooutput', inUse, facts['inUseName'])
          : _deviceName(devices, 'audiooutput', 'default', null);
      text = seen.now == 'gone'
          ? l10n.callOutputGone(was, where)
          : facts['reason'] == 'rate'
          ? l10n.callOutputNotWithMicrophone(was, where)
          : l10n.callOutputUnusable(was, where);
    }
    return (text: text, inUse: seen.inUse);
  }
}

/// The microphone chosen, as [OutputWatch] does the output: told when the engine records
/// from another one while the one chosen is there. One that is unplugged is told by
/// [MicWatch], from the devices listed.
@visibleForTesting
class InputWatch {
  final _watch = _ChosenWatch('input');

  DeviceTold take(
    Object? facts,
    List<MediaDeviceInfo> devices, {
    required String? asked,
  }) {
    final seen = _watch.take(facts, asked);
    if (seen == null || facts is! Map) return _nothing;
    String? text;
    if (seen.began && seen.now == 'other') {
      text = l10n.callInputUnusable(
        _deviceName(
          devices,
          'audioinput',
          _chosenId(facts['inputChosen'])!,
          facts['inputChosenName'],
        ),
        _deviceName(
          devices,
          'audioinput',
          seen.inUse!,
          facts['inputInUseName'],
        ),
      );
    }
    return (text: text, inUse: seen.inUse);
  }
}

/// Whether the voice engine lost the recording the call wants (macOS), from its facts
/// every 2 s: the call has its microphone on and the engine's input is let in, no choice
/// is being carried out, and either the module was asked to record but nothing records
/// (its engine stopped or not made), or it was not asked although the call sends voice
/// (a start that failed: the module then drops WebRTC's request, and nothing asks again).
/// Seen twice in a row, it answers true once, for the call to stop and start its voice
/// encodings so that WebRTC records again its own way ([Call._recoverVoice]); a recording
/// seen again, or the pair chosen now refused by the engine (a new engine following the
/// system's devices), lets it answer true again.
@visibleForTesting
class RecordingWatch {
  int _seen = 0;
  bool _recovered = false;
  bool _refused = false;
  Object? _combinable;
  bool _listened = false;

  /// The system's devices chosen again (nothing read meanwhile): what was seen of the
  /// devices chosen before is not this choice's.
  void reset() {
    _seen = 0;
    _recovered = false;
    _refused = false;
    _combinable = null;
    _listened = false;
  }

  /// [listened]: someone in the call takes this device's voice now (a connection up).
  bool take(
    Object? facts, {
    required bool mic,
    required bool sending,
    bool listened = true,
  }) {
    // The engine changed what it is since (the pair chosen now refused, R4: a new engine
    // following the system's devices): a restart of the recording may work now, and a
    // loss seen now has that known cause, so one reading is enough. It holds until
    // something records or the microphone is off. While it holds, nothing recorded with
    // the microphone on, while someone takes the voice, is the loss, asked or not: a
    // start that failed drops the request, and the voice sent is read from the reading
    // before, with the microphone maybe still off then. Alone in the call nothing is to
    // record, and the refusal waits for someone; one who has just come may not be
    // recorded for yet (WebRTC starts the recording as the voice to them starts), so
    // only a second reading with them takes it for the loss.
    final listenedBefore = _listened;
    _listened = listened;
    if (facts is Map) {
      final combinable = facts['combinable'];
      if (_combinable == true && combinable == false) {
        _recovered = false;
        _refused = true;
      }
      _combinable = combinable;
    }
    if (facts is! Map || facts['recording'] == true || !mic) {
      _seen = 0;
      _refused = false;
      if (facts is Map && facts['recording'] == true) _recovered = false;
      return false;
    }
    final lost =
        facts['changing'] != true &&
        facts['inputUsable'] == true &&
        (facts['recordingAsked'] == true || sending || _refused && listened);
    _seen = lost ? _seen + 1 : 0;
    final atOnce = lost && _refused && listenedBefore;
    if ((_seen < 2 && !atOnce) || _recovered) return false;
    _refused = false;
    _recovered = true;
    return true;
  }
}

/// WebRTC's processing of the microphone, the same for every source that sends sound
/// (see [Call._capture]). Linux and Windows read the plain names; macOS reads only the
/// `mandatory` map, WebRTC's own names as text, and takes whatever WebRTC last set for
/// any name left out, so all three are there.
const _micProcessing = {
  'echoCancellation': true,
  'autoGainControl': true,
  'noiseSuppression': false,
  'mandatory': {
    'googEchoCancellation': 'true',
    'googAutoGainControl': 'true',
    'googNoiseSuppression': 'false',
  },
};

/// The microphone's noise suppression: RNNoise, a small neural network in the Rust
/// library, which flutter_webrtc's native code runs on every 10 ms the microphone records,
/// after echo cancelling and gain control. It lowers what is not speech (a fan, a hum,
/// typing, traffic) and keeps the voice as loud as it was. On or off from the next 10 ms,
/// in a call or not; false where it cannot run.
Future<bool> suppressNoise(bool on) async {
  var running = false;
  try {
    running =
        await WebRTC.invokeMethod<bool, dynamic>('fogataNoiseSuppression', {
          'on': on,
          'api': api.noiseSuppressor(),
        }) ??
        false;
  } catch (e) {
    _trace('noise suppression: $e');
  }
  _trace(
    'noise suppression ${on ? 'on' : 'off'}${running ? '' : ': cannot run'}',
  );
  return running;
}

/// How long opening the microphone waits first; changed by tests, so that a participant
/// connects before the microphone is open, as with a system slow to open it or one that
/// asks first.
@visibleForTesting
Duration micOpensAfter = Duration.zero;

/// What this device sends from its own microphone, camera and screen: opening and closing
/// them, and following the microphone as the system's devices come and go.
extension _LocalMedia on Call {
  Future<void> _openMic() => _whileOpening(() async {
    if (micOpensAfter > Duration.zero) {
      await Future<void>.delayed(micOpensAfter);
    }
    // Nothing is opened for a call that ended, or a microphone turned off, meanwhile: one
    // opened then would be let go of after the voice engine, maybe (see [_stop]).
    if (_closed || !mic) return;
    final MediaStream stream;
    try {
      stream = await _getMic();
    } catch (e) {
      _trace('microphone: $e');
      mic = false;
      _report(l10n.callMicFailed(_system));
      return;
    }
    // The call ended or the microphone went off while the system answered (it may have
    // asked first), or another one opened meanwhile.
    if (_closed || !mic || _micStream != null) {
      await Call._free(stream);
      return;
    }
    _micStream = stream;
  });

  Future<MediaStream> _getMic() => openMicStream(input);

  Future<void> _openCamera() => _whileOpening(() async {
    // The settings' preview gives the camera up first.
    await cameraPreview.stop();
    final MediaStream stream;
    try {
      stream = await openCameraStream(cameraId);
    } catch (e) {
      _trace('camera: $e');
      camera = false;
      _report(cameraFailure(e));
      return;
    }
    if (_closed || !camera || _cameraStream != null) {
      await Call._free(stream);
      return;
    }
    _cameraStream = stream;
    _cameraLines = 0;
    _cameraWidth = 0;
    localCamera.srcObject = stream;
  });

  Future<void> _closeMic() async {
    final stream = _micStream;
    _micStream = null;
    if (stream == null) return;
    // The own level's loop sends this track: it goes first.
    await _alone.close();
    for (final track in stream.getTracks()) {
      await track.stop();
    }
    await stream.dispose();
  }

  Future<void> _closeCamera() async {
    final stream = _cameraStream;
    _cameraStream = null;
    localCamera.srcObject = null;
    if (stream == null) return;
    for (final track in stream.getTracks()) {
      await track.stop();
    }
    await stream.dispose();
  }

  /// Linux cannot narrow a window's sound to its application: every other one's would go
  /// with it, so a window is shared without sound. (Under Wayland the system's dialog
  /// picks, and the capture leaves the sound out itself when a window was chosen.)
  static bool _windowWithoutSound(DesktopCapturerSource? source) =>
      onLinux && source?.type == SourceType.Window;

  /// What to ask the system for to capture `source` at `fps` frames a second. Under
  /// Wayland the system's dialog picks it, and the system is asked for up to 60 a second,
  /// since it cannot be asked again without that dialog; the capture then lets through
  /// only the quality's rate (see [_portalRate]).
  Map<String, dynamic> _capture(DesktopCapturerSource? source, int fps) => {
    // The screen's sound is not processed, but WebRTC sets its one sound processing
    // from every source that sends: the microphone's own settings, so they stay on.
    'audio': _displaySound && !_windowWithoutSound(source)
        ? _micProcessing
        : false,
    'video': source != null
        ? {
            'deviceId': {'exact': source.id},
            'mandatory': {'frameRate': fps.toDouble()},
          }
        : _wayland
        ? {
            'mandatory': {'frameRate': 60.0},
          }
        : true,
  };

  /// Waits, 3 s at most, for the shared screen's first picture (drawn, or told too large
  /// to draw), and scales it as its size says. Until then it would go out unscaled to those who watch (another screen picked
  /// while they do), and one larger than a participant takes ([fairPicture]) would get
  /// this device kept out of the call.
  Future<void> _screenSized() async {
    final end = callClock().add(const Duration(seconds: 3));
    while (localScreen.value.width <= 0 &&
        _screenTooLarge == null &&
        callClock().isBefore(end)) {
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    await _rescale();
  }

  /// Captures the shared screen again at the rate its quality needs, without asking which
  /// one (only a screen or window picked from Fogata's own list can be). Those who watch
  /// it get the new capture in place of the old.
  Future<void> _recapture() => _whileOpening(() async {
    final source = _source;
    if (source == null || !sharing) return;
    final fps = screenQuality.fps;
    final MediaStream stream;
    try {
      stream = await navigator.mediaDevices.getDisplayMedia(
        _capture(source, fps),
      );
    } catch (e) {
      _trace('screen at $fps fps: $e');
      // What was shared can no longer be captured (a window that closed): the old
      // capture would go on with no picture, so sharing stops, and the person is told.
      if (_closed || !sharing || _source != source) return;
      await stopSharing();
      _report(l10n.callShareLost);
      return;
    }
    final old = _screenStream;
    if (_closed || !sharing || _source != source || old == null) {
      await Call._free(stream);
      return;
    }
    old.getVideoTracks().firstOrNull?.onEnded = null;
    _screenStream = stream;
    _capturedAt = fps;
    localScreen.srcObject = stream;
    stream.getVideoTracks().firstOrNull?.onEnded = () => stopSharing();
    await _swapCapture(old);
  });

  /// Puts the shared screen captured anew ([_screenStream] now) in place of [old] on
  /// every sender, its picture and its sound, and only then lets go of [old]: a watcher
  /// whose turn is busy still gets the sound meanwhile. A track replaced changes no
  /// direction and needs nothing agreed, and every step that gives a sender its track
  /// ([_voiceTo]) gives the one there is now, so this, outside a participant's turn,
  /// cannot undo theirs.
  Future<void> _swapCapture(MediaStream old) async {
    for (final peer in peers.values.toList()) {
      try {
        await _resendTo(peer, _screen);
        await _resendTo(peer, _screenSound);
      } catch (e) {
        _trace('screen to ${peer.id}: $e');
      }
    }
    await _soundChannel();
    await Call._free(old);
  }

  /// Tells a screen captured through the system's dialog (Wayland) how many frames a
  /// second to let through; nothing elsewhere.
  Future<void> _portalRate(int fps) async {
    final track = _screenStream?.getVideoTracks().firstOrNull;
    if (!_wayland || track?.id == null) return;
    try {
      await WebRTC.invokeMethod<bool, dynamic>('fogataScreenFps', {
        'trackId': track!.id,
        'fps': fps,
      });
    } catch (e) {
      _trace('screen at $fps fps: $e');
    }
  }

  /// Has the shared screen's capture give its last picture again, now, to every encoder
  /// it feeds: one that gives a picture only when the screen changes (the portal's on
  /// Linux, ScreenCaptureKit on macOS) gives none to the encoder of someone who just
  /// started watching while the screen stays still, and they would see nothing until
  /// something on it moved. The others give pictures all the time, and the platform
  /// answers no.
  Future<void> _refreshScreen() async {
    final track = _screenStream?.getVideoTracks().firstOrNull;
    if (!sharing || track?.id == null) return;
    try {
      await WebRTC.invokeMethod<bool, dynamic>('fogataScreenRefresh', {
        'trackId': track!.id,
      });
    } catch (e) {
      _trace('screen picture again: $e');
    }
  }

  /// Whether [connection] has agreed to send voice: its channel's current direction (as
  /// last negotiated) sends. Not known counts as not.
  Future<bool> _sendsVoice(RTCPeerConnection connection) =>
      _agreedSends(connection, _audio);

  /// Whether [channel] of [connection] is agreed to send ([_sendsVoice]): only then can
  /// WebRTC run a send stream of it, and record for it.
  Future<bool> _agreedSends(RTCPeerConnection connection, int channel) async {
    try {
      final channels = await _channels(connection);
      if (channels.length <= channel) return false;
      final direction = await channels[channel].getCurrentDirection();
      return direction == TransceiverDirection.SendRecv ||
          direction == TransceiverDirection.SendOnly;
    } catch (_) {
      return false;
    }
  }

  /// The microphone on reads nothing: says why as well as can be told, and turns it off.
  Future<void> _tellMicUnusable() async {
    final stream = _micStream;
    List<MediaDeviceInfo>? devices;
    try {
      devices = await listDevices();
    } catch (e) {
      _trace('devices: $e');
    }
    // Turned off, or opened anew, meanwhile: that is judged again from the start.
    if (_closed || !mic || _micStream != stream) return;
    final text = Call.micUnusable(devices, _system);
    _trace('microphone reads nothing: $text');
    if (onMicUnusable case final tell?) {
      tell(text);
    } else {
      _report(text);
      await setMic(false);
    }
  }

  Future<void> _readDevices() async {
    if (_closed) return;
    if (_checkingDevices) {
      _devicesAgain = true;
      return;
    }
    _checkingDevices = true;
    try {
      do {
        _devicesAgain = false;
        final devices = await listDevices();
        if (_closed) return;
        takeDevices(devices);
      } while (_devicesAgain && !_closed);
    } catch (e) {
      _trace('devices: $e');
    } finally {
      _checkingDevices = false;
    }
  }
}

/// Whether a source of the list to share is Fogata's own window, as far as the list tells
/// it here. Under X11 ([linux]) it does not tell a window's process: a window with the
/// title Fogata's has (linux/runner/my_application.cc) is taken for it. macOS and Windows
/// leave Fogata's windows out of the list themselves, by their process.
@visibleForTesting
bool ownWindow(SourceType type, String name, {required bool linux}) =>
    linux && type == SourceType.Window && name == 'Fogata';

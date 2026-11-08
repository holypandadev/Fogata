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
/// connection has agreed to send voice (its channel's current direction sends: a microphone
/// just turned on waits for the new description, which may take a resend), from what the
/// device gives (the seconds of sound read, the `media-source`'s `totalSamplesDuration`)
/// and what goes out (each such connection's voice packets; silence still goes out now and
/// then, Opus DTX): [after] with neither growing, from one reading to the next, reads
/// nothing. So a device that stops later (another input chosen that cannot be opened) is
/// caught too. Told once until the microphone is turned off and on again ([reset]).
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

  /// What to ask the system for to capture `source` at `fps` frames a second. Under
  /// Wayland the system's dialog picks it, and the system is asked for up to 60 a second,
  /// since it cannot be asked again without that dialog; the capture then lets through
  /// only the quality's rate (see [_portalRate]).
  Map<String, dynamic> _capture(DesktopCapturerSource? source, int fps) => {
    // The screen's sound is not processed, but WebRTC sets its one sound processing
    // from every source that sends: the microphone's own settings, so they stay on.
    'audio': _displaySound ? _micProcessing : false,
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
    final end = DateTime.now().add(const Duration(seconds: 3));
    while (localScreen.value.width <= 0 &&
        _screenTooLarge == null &&
        DateTime.now().isBefore(end)) {
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
    for (final peer in peers.values.toList()) {
      await _resendTo(peer, _screen);
    }
    await _soundChannel();
    await Call._free(old);
  });

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

  /// Whether [connection] has agreed to send voice: its channel's current direction (as
  /// last negotiated) sends. Not known counts as not.
  Future<bool> _sendsVoice(RTCPeerConnection connection) async {
    try {
      final channels = await _channels(connection);
      if (channels.isEmpty) return false;
      final direction = await channels[_audio].getCurrentDirection();
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

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'call.dart'
    show closeConnection, openMicStream, silenceLabel, suppressNoise;

/// How loud a microphone is, read where nobody else is connected to read it from.
///
/// WebRTC's voice engine records only while a connection sends sound, and it says how loud
/// that sound is only in the sending connection's statistics. So the microphone's track is
/// sent from one connection to a second one in this same process, and the first one's
/// statistics give the level (the `media-source` report, as in a call). What the second
/// one receives is not played unless asked ([hear]): its track stays disabled, which
/// silences it.
/// A microphone's track being measured: [LevelLoop], or a test's own.
abstract interface class TrackLevel {
  /// The track measured.
  String get trackId;
  Future<double> level();
  Future<void> close();
}

class LevelLoop implements TrackLevel {
  LevelLoop._(this._sender, this._receiver, this.trackId);

  final RTCPeerConnection _sender;
  final RTCPeerConnection _receiver;

  /// The track whose level this reads.
  @override
  final String trackId;

  MediaStreamTrack? _heard;
  bool _hear = false;
  bool _closed = false;

  /// Sends [track] of [stream] from one connection to the other. Whatever fails on the
  /// way closes what was made, and is thrown.
  static Future<LevelLoop> open(
    MediaStream stream,
    MediaStreamTrack track, {
    bool hear = false,
  }) async {
    final sender = await createPeerConnection({'iceServers': []});
    final RTCPeerConnection receiver;
    try {
      receiver = await createPeerConnection({'iceServers': []});
    } catch (_) {
      await closeConnection(sender);
      rethrow;
    }
    final loop = LevelLoop._(sender, receiver, track.id ?? '').._hear = hear;
    try {
      await loop._connect(stream, track);
    } catch (_) {
      await loop.close();
      rethrow;
    }
    return loop;
  }

  Future<void> _connect(MediaStream stream, MediaStreamTrack track) async {
    // Candidates found before the other side has a description wait for it.
    final toSender = <RTCIceCandidate>[];
    final toReceiver = <RTCIceCandidate>[];
    var described = false;
    void pass(
      RTCIceCandidate c,
      RTCPeerConnection to,
      List<RTCIceCandidate> held,
    ) {
      if (c.candidate == null || _closed) return;
      if (!described) {
        held.add(c);
        return;
      }
      unawaited(to.addCandidate(c).catchError((_) {}));
    }

    _sender.onIceCandidate = (c) => pass(c, _receiver, toReceiver);
    _receiver.onIceCandidate = (c) => pass(c, _sender, toSender);
    _receiver.onTrack = (event) {
      if (event.track.kind != 'audio') return;
      _heard = event.track;
      // Silent until asked: what the microphone hears is not played back unasked.
      event.track.enabled = _hear;
    };
    await _sender.addTrack(track, stream);
    final offer = await _sender.createOffer({});
    await _sender.setLocalDescription(offer);
    await _receiver.setRemoteDescription(offer);
    final answer = await _receiver.createAnswer({});
    await _receiver.setLocalDescription(answer);
    await _sender.setRemoteDescription(answer);
    described = true;
    for (final c in toReceiver) {
      await _receiver.addCandidate(c).catchError((_) {});
    }
    for (final c in toSender) {
      await _sender.addCandidate(c).catchError((_) {});
    }
  }

  /// The level now, from 0 (silence) to 1; 0 when it cannot be read.
  @override
  Future<double> level() async {
    if (_closed) return 0;
    var level = 0.0;
    try {
      for (final report in await _sender.getStats()) {
        final value = report.values['audioLevel'];
        if (report.type == 'media-source' && value is num) {
          level = math.max(level, value.toDouble());
        }
      }
    } catch (_) {}
    return level;
  }

  /// Plays what the microphone hears through the speaker, or stops playing it.
  set hear(bool on) {
    _hear = on;
    _heard?.enabled = on;
  }

  /// Closes both connections. The track is not stopped: it is the caller's.
  @override
  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    _heard?.enabled = false;
    for (final connection in [_sender, _receiver]) {
      try {
        await closeConnection(connection);
      } catch (_) {}
    }
  }
}

/// This device's own level in a call while it is alone in it: nobody's connection says
/// how loud it is then, so a [LevelLoop] of its own does, for the microphone's track as
/// it is now. A track that could not be looped is not tried again.
class AloneLevel {
  AloneLevel({this.open = LevelLoop.open});

  /// How the loop is made; tests give their own.
  final Future<TrackLevel> Function(MediaStream, MediaStreamTrack) open;

  TrackLevel? _loop;
  Future<TrackLevel?>? _opening;
  String? _failed;

  /// The level of [stream]'s microphone, opening the loop if needed; 0 without one.
  Future<double> read(MediaStream? stream) async {
    final track = stream?.getAudioTracks().firstOrNull;
    if (stream == null || track == null) {
      await close();
      return 0;
    }
    var loop = _loop;
    if (loop != null && loop.trackId != track.id) {
      // Another microphone: the loop of the one before goes before another is made.
      await close();
      loop = null;
    }
    if (loop == null) {
      if (_failed == track.id) return 0;
      final opening = _opening ??= _open(stream, track);
      loop = await opening;
      if (identical(_opening, opening)) _opening = null;
      if (loop == null || !identical(_loop, loop)) return 0;
    }
    return loop.level();
  }

  Future<TrackLevel?> _open(MediaStream stream, MediaStreamTrack track) async {
    try {
      return _loop = await open(stream, track);
    } catch (e) {
      _failed = track.id;
      debugPrint('own level: $e');
      return null;
    }
  }

  /// Lets the loop go; the next [read] opens another.
  Future<void> close() async {
    final opening = _opening;
    _opening = null;
    final loop = _loop;
    _loop = null;
    await loop?.close();
    // One still opening is closed once open.
    final late = await opening;
    if (late != null && !identical(late, loop)) {
      if (identical(_loop, late)) _loop = null;
      await late.close();
    }
  }
}

/// A microphone opened for a test, and how loud it is.
abstract class MicLoop {
  /// From 0 (silence) to 1.
  Future<double> level();

  /// Plays what it hears through the speaker, or stops.
  set hear(bool on);

  /// Closes the microphone and all that was opened for it.
  Future<void> stop();
}

/// Opens microphone [input] for a test, its sound played through [output] when [hear];
/// null for the system's defaults.
typedef OpenMicLoop = Future<MicLoop> Function({
  String? input,
  String? output,
  required bool hear,
  required bool noiseSuppression,
});

/// The microphone as a call opens it, with the same processing (and noise suppression,
/// when on), sent through a [LevelLoop].
Future<MicLoop> openWebrtcMic({
  String? input,
  String? output,
  required bool hear,
  required bool noiseSuppression,
}) async {
  // WebRTC's voice engine starts with the first connection and takes the system's default
  // devices then: the chosen ones are taken once it runs, as a call does.
  final engine = await createPeerConnection({});
  MediaStream? stream;
  try {
    await suppressNoise(noiseSuppression);
    await _choose(output, 'audiooutput', Helper.selectAudioOutput);
    await _choose(input, 'audioinput', Helper.selectAudioInput);
    final opened = stream = await openMicStream(input);
    final track = opened.getAudioTracks().firstOrNull;
    if (track == null) throw StateError('no microphone');
    final loop = await LevelLoop.open(opened, track, hear: hear);
    return _WebrtcMic(engine, opened, loop);
  } catch (_) {
    // The track goes before the last connection: freed after the voice engine stopped, an
    // audio track brought the application down on Linux.
    if (stream != null) await _free(stream);
    await closeConnection(engine);
    rethrow;
  }
}

/// Points the voice engine at device [id], or at the system's default (listed first) when
/// null; one no longer connected leaves the current one.
Future<void> _choose(
  String? id,
  String kind,
  Future<void> Function(String) select,
) async {
  try {
    var chosen = id;
    if (chosen == null) {
      final devices = await navigator.mediaDevices.enumerateDevices();
      chosen = devices
          .where((d) => d.kind == kind && !d.label.contains(silenceLabel))
          .firstOrNull
          ?.deviceId;
    }
    if (chosen != null) await select(chosen);
  } catch (e) {
    debugPrint('microphone test: device $id: $e');
  }
}

Future<void> _free(MediaStream stream) async {
  for (final track in stream.getTracks()) {
    try {
      await track.stop();
    } catch (_) {}
  }
  try {
    await stream.dispose();
  } catch (_) {}
}

class _WebrtcMic implements MicLoop {
  _WebrtcMic(this._engine, this._stream, this._loop);
  final RTCPeerConnection _engine;
  final MediaStream _stream;
  final LevelLoop _loop;

  @override
  Future<double> level() => _loop.level();

  @override
  set hear(bool on) => _loop.hear = on;

  @override
  Future<void> stop() async {
    _loop.hear = false;
    // The track first, while connections still keep the voice engine running.
    await _free(_stream);
    await _loop.close();
    await closeConnection(_engine);
  }
}

/// The microphone test of the settings: it opens the chosen microphone only when asked,
/// shows how loud it is, and gives it up when asked to stop: when its part of the settings
/// goes away, the window is hidden, and before a call opens the microphone.
class MicTest extends ChangeNotifier {
  MicTest({this.open = openWebrtcMic});

  /// How the microphone is opened; tests give their own.
  OpenMicLoop open;

  /// How often the level is read while the test runs.
  static const every = Duration(milliseconds: 100);

  MicLoop? _mic;
  bool _opening = false;
  bool _failed = false;
  bool _hear = false;
  Timer? _reading;

  /// Counts starts and stops, so a microphone that opens after a stop is closed at once.
  int _turn = 0;

  /// The starts not yet done, each closing what it opened if stopped meanwhile.
  final Set<Future<void>> _starting = {};

  /// A call is starting or has the microphone: no test starts ([block]).
  bool _blocked = false;
  bool get blocked => _blocked;

  /// How long [stop] waits for microphones still opening, at most: one whose opening hangs
  /// is closed once open, without holding up a call.
  @visibleForTesting
  static Duration stopWait = const Duration(seconds: 3);

  /// The level as last read, from 0 to 1.
  final level = ValueNotifier<double>(0);

  /// Opening or open.
  bool get on => _opening || _mic != null;
  bool get opening => _opening;

  /// The last start could not open the microphone.
  bool get failed => _failed;

  /// Whether what the microphone hears plays through the speaker.
  bool get hearing => _hear;
  set hearing(bool on) {
    if (_hear == on) return;
    _hear = on;
    _mic?.hear = on;
    notifyListeners();
  }

  Future<void> start({
    String? input,
    String? output,
    bool noiseSuppression = false,
  }) {
    if (on || _blocked) return Future.value();
    final starting = _start(input, output, noiseSuppression);
    _starting.add(starting);
    return starting.whenComplete(() => _starting.remove(starting));
  }

  /// Gives the microphone up to a call that starts, before it does anything else: this test
  /// stops, and none starts until [unblock], once the call is over.
  Future<void> block() {
    _blocked = true;
    notifyListeners();
    return stop();
  }

  void unblock() {
    if (!_blocked) return;
    _blocked = false;
    notifyListeners();
  }

  Future<void> _start(String? input, String? output, bool noise) async {
    final turn = ++_turn;
    _opening = true;
    _failed = false;
    notifyListeners();
    final MicLoop mic;
    try {
      mic = await open(
        input: input,
        output: output,
        hear: _hear,
        noiseSuppression: noise,
      );
    } catch (e) {
      debugPrint('microphone test: $e');
      if (turn == _turn) {
        _opening = false;
        _failed = true;
        notifyListeners();
      }
      return;
    }
    if (turn != _turn) {
      // Stopped while it opened.
      await mic.stop();
      return;
    }
    // Asked for while it opened.
    mic.hear = _hear;
    _opening = false;
    _mic = mic;
    _reading = Timer.periodic(every, (_) => _read(mic, turn));
    notifyListeners();
  }

  Future<void> _read(MicLoop mic, int turn) async {
    final now = await mic.level();
    if (turn == _turn && identical(mic, _mic)) level.value = now;
  }

  Future<void> stop() async {
    _turn++;
    _reading?.cancel();
    _reading = null;
    final mic = _mic;
    final was = on;
    _mic = null;
    _opening = false;
    level.value = 0;
    if (was) notifyListeners();
    await mic?.stop();
    // Microphones still opening are closed before this returns, so a call can open it
    // next; one that hangs is not waited for past [stopWait], and closes once open.
    if (_starting.isEmpty) return;
    await Future.wait(_starting.toList())
        .timeout(stopWait, onTimeout: () => []);
  }
}

/// The one test the settings show; a call stops it before it opens the microphone.
final micTest = MicTest();

/// [level] (0 to 1, as WebRTC gives it) as a share of a meter that shows from -60 dB up to
/// full scale: the voice's usual levels fill most of it, rather than its first few pixels.
double meterShare(double level) {
  if (level <= 0) return 0;
  final db = 20 * math.log(level) / math.ln10;
  return ((db + 60) / 60).clamp(0.0, 1.0);
}

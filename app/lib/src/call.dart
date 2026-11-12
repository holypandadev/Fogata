import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'camera_preview.dart';
import 'host.dart';
import 'l10n.dart';
import 'mic_test.dart' show AloneLevel;
import 'quality.dart';
import 'rust/api/app.dart' as api;
import 'rust/api/types.dart';
import 'shown.dart';

part 'call/fitting.dart';
part 'call/measuring.dart';
part 'call/media.dart';
part 'call/peer.dart';
part 'call/screen_sound.dart';
part 'call/sdp.dart';
part 'call/signalling.dart';

/// Every connection carries four channels in this order, so turning the microphone,
/// camera or screen on and off only swaps tracks and never renegotiates.
const _audio = 0;
const _camera = 1;
const _screen = 2;

/// The sound of a shared screen, sent with it to those who watch it.
const _screenSound = 3;

/// Where capturing the screen brings the other applications' sound with it.
bool get _displaySound => onLinux || onWindows;

/// Where that sound goes instead on a connection of its own to each watcher: macOS's
/// WebRTC hands what it records to every channel of sound, the voice's included, so the
/// screen's sound is recorded and sent by a WebRTC of its own.
bool get _soundConnections => onMacOS;

/// The input that only gives silence, made where the system allows it (Linux): WebRTC
/// records from it while the microphone is off and a shared screen's sound goes out.
const silenceLabel = 'Fogata silence';

bool get _silentInputs => onLinux;

/// A level above this means someone is talking.
const _speaking = 0.02;

/// For messages that say where to let Fogata use the microphone, the camera or the
/// screen: macOS asks once, and a "no" stays until changed there.
String get _system => onMacOS
    ? 'macos'
    : onWindows
    ? 'windows'
    : 'other';

/// With FOGATA_CALL_LOG set, the call's steps go to standard error.
final _verbose = Platform.environment['FOGATA_CALL_LOG'] != null;

/// How long one step of letting go of a call (a track, a stream, a connection, a
/// renderer) may take before the next goes on; changed by tests.
@visibleForTesting
Duration letGoLimit = const Duration(seconds: 5);

/// Runs one step of letting go of a call: one that fails or hangs is noted, and the steps
/// after it still run. Completes with whether it ended in time ([letGoLimit], or [limit]
/// for a step made of several); [late] is told when one that did not ends after all.
Future<bool> _letGo(
  String what,
  FutureOr<void> Function() step, {
  void Function(Future<void> step)? late,
  Duration? limit,
}) async {
  limit ??= letGoLimit;
  final Future<void> running;
  try {
    running = Future.sync(step);
  } catch (e) {
    _trace('letting go of $what: $e');
    return true;
  }
  try {
    await running.timeout(limit);
    return true;
  } on TimeoutException {
    _trace('letting go of $what: still going after ${limit.inSeconds} s');
    late?.call(running.catchError((Object _) {}));
    return false;
  } catch (e) {
    _trace('letting go of $what: $e');
    return true;
  }
}

/// Closes [connection], then lets the platform free it: closed only, it kept its
/// observer, event channel and Dart subscription until the application quit, one for every
/// connection made in a long call (each participant, each retry, the screen's sound, the
/// voice engine, the microphone test). Freed even when the close fails; a free that fails
/// is only noted, as nothing is left to do with that connection.
Future<void> closeConnection(RTCPeerConnection connection) async {
  try {
    await connection.close();
  } finally {
    try {
      await connection.dispose();
    } catch (e) {
      _trace('freeing a connection: $e');
    }
  }
}

void _trace(String line) {
  if (!_verbose) return;
  // The time of day to the millisecond, to line the call's steps up with other logs.
  final time = DateTime.now().toIso8601String().substring(11, 23);
  stderr.writeln('call $time: $line');
}

/// Whether the video library captures the screen through the system's dialog (the portal
/// and PipeWire), with no list of screens and windows to pick from: a Wayland session, as
/// the video library decides it from the display GTK draws on ([_askWayland]); until it
/// has answered, or where it cannot, from the environment ([waylandSession]).
bool get _wayland =>
    onLinux && (_waylandAnswer ?? waylandSession(Platform.environment));

bool? _waylandAnswer;

/// Asks the video library, once, whether GTK draws on a Wayland display (its
/// `WaylandSession`, `linux/desktop_session.h`). The environment can say otherwise: a
/// session that names both displays but no session type (sway, or variables imported by
/// systemd) draws on Wayland, where a list of X11's screens would only show black.
Future<void> _askWayland() async {
  if (!onLinux || _waylandAnswer != null) return;
  try {
    final answer = await WebRTC.invokeMethod<bool, dynamic>(
      'fogataWaylandSession',
    );
    if (answer != null) _waylandAnswer = answer;
  } catch (e) {
    _trace('desktop session: $e');
  }
}

/// Whether the screen is shared through the system's dialog (the portal) rather than
/// picked from a list of screens and windows: the rule [Call.screens] and
/// [Call.shareScreen] go by.
Future<bool> screenThroughPortal() async {
  await _askWayland();
  return _wayland;
}

/// Forgets the video library's answer, for tests.
@visibleForTesting
void forgetWaylandAnswer() => _waylandAnswer = null;

/// A Wayland session as the environment tells it, where GTK's own display is not known:
/// a Wayland display is named (an empty name counts, as for WebRTC), and either the
/// session says it is Wayland or no X display is named. An application started from a
/// service may be given only WAYLAND_DISPLAY; GTK draws on that display, and with no X
/// display there is nothing a list could show. The video library's fallback is the same.
@visibleForTesting
bool waylandSession(Map<String, String> environment) {
  bool named(String variable) => (environment[variable] ?? '').isNotEmpty;
  return environment.containsKey('WAYLAND_DISPLAY') &&
      ((environment['XDG_SESSION_TYPE'] ?? '').startsWith('wayland') ||
          !named('DISPLAY'));
}

/// The call of the room this device is in: microphone, camera and screen, sent to every
/// other participant over WebRTC. WebRTC only reaches this device's media bridge, which
/// carries the packets over the member connections; no other server is involved.
class Call extends ChangeNotifier {
  Call({
    required this.group,
    required this.me,
    this.mic = true,
    this.camera = false,
    this.input,
    this.output,
    this.cameraId,
    this.noiseSuppression = false,
    this.gainOf = _unchanged,
    this.screenGainOf = _unchanged,
    this.screenQuality = const ScreenQuality(),
    this.nameOf = _someone,
  }) : quality = CallQuality(me: me);

  static double _unchanged(String id) => 1;
  static String _someone(String id) => l10n.appSomeone;

  /// A participant's name, for what the call tells about them.
  final String Function(String id) nameOf;

  /// How loud each participant's voice plays on this device: 1 as it arrives, 0 not at
  /// all, up to 2. Only this device hears the change.
  final double Function(String id) gainOf;

  /// The same for the sound of each participant's shared screen.
  final double Function(String id) screenGainOf;

  /// A shared screen goes with the other applications' sound, where it can be captured.
  bool shareSound = true;

  /// How large and how smooth this device's shared screen goes out.
  ScreenQuality screenQuality;

  /// Devices to use, by their WebRTC device id; null for the system's default.
  String? input;
  String? output;
  String? cameraId;

  /// The microphone's sound goes through RNNoise (see [suppressNoise]), after WebRTC's
  /// echo cancelling and gain control, which are always on.
  bool noiseSuppression;

  final String group;
  final String me;
  bool mic;
  bool camera;
  bool sharing = false;

  /// The participant whose shared screen this device watches. Until someone watches a
  /// screen, it is not sent to them, and its sound is not played here.
  String? get watching => _watching;
  set watching(String? id) {
    if (id == _watching) return;
    final before = _watching;
    _watching = id;
    // A screen watched anew waits for its first picture from the start.
    _pictureWait = null;
    for (final changed in [before, id].nonNulls) {
      unawaited(
        applyScreenVolume(changed).catchError((Object e) => _trace('$e')),
      );
    }
  }

  String? _watching;

  /// The screen watched while it shows no picture yet, on the connection it is to come
  /// on, and the readings of the statistics since it was watched there ([_awaitPicture]).
  ({String id, RTCPeerConnection on, int readings})? _pictureWait;

  /// The readings after which a watched screen that shows no picture yet is shown again
  /// and its sharer told again ([_awaitPicture]): 2 to 4, 6 to 8 and 14 to 16 s after it
  /// was watched (the first reading comes within 2 s of it, and the others 2 s apart), and
  /// never after.
  static const pictureReadings = {2, 4, 8};

  /// How loud the sound of [id]'s shared screen plays here: as [screenGainOf] says while
  /// this device watches that screen, not at all otherwise, whoever sends it.
  @visibleForTesting
  double screenSoundGain(String id) => watching == id ? screenGainOf(id) : 0;

  /// The participant whose camera fills the room on this device, this device included.
  /// Only how it is shown here: cameras are always sent.
  String? focused;

  /// This device's own shared screen fills the room here.
  bool ownScreen = false;

  /// This device's voice is being picked up.
  bool speaking = false;

  /// Ticks each time who is talking ([speaking], [CallPeer.speaking]) changes. That is
  /// several times a second while people talk: only what shows it listens here, rather
  /// than the whole window being told through the call's own listeners.
  final talking = ValueNotifier<int>(0);
  String? problem;

  final localCamera = RTCVideoRenderer();
  final localScreen = RTCVideoRenderer();

  /// How this device's shared screen reaches each person watching it now, by device id,
  /// from the readings taken every 2 s.
  final screenOut = ValueNotifier<Map<String, ScreenOut>>(const {});

  /// The height the shared screen is captured at, in lines; null before its first picture.
  int? get screenLines {
    final height = localScreen.value.height.round();
    return sharing && height > 0 ? height : null;
  }

  void _showScreenOut(String id, ScreenOut? out) {
    final now = screenOut.value;
    if (now[id] == out) return;
    screenOut.value = {
      for (final MapEntry(:key, :value) in now.entries)
        if (key != id) key: value,
      id: ?out,
    };
  }

  final Map<String, CallPeer> peers = {};

  /// How well the connections carry the call, measured every 2 s.
  final CallQuality quality;

  /// Plays a sound for something that happened in the call: sharing started or stopped,
  /// here or by someone else.
  void Function(UiSound sound)? onSound;

  /// Tells the person what went wrong with the call, the microphone, the camera or the
  /// screen, in words for people.
  void Function(String problem)? onProblem;

  /// Tells the person something about the call that is no problem, such as the
  /// microphone in use going away and coming back.
  void Function(String notice)? onNotice;

  MediaStream? _micStream;
  MediaStream? _cameraStream;
  MediaStream? _screenStream;

  /// The screen or window shared, when it was picked from Fogata's own list (not the
  /// system's dialog), and the frame rate it is captured at.
  DesktopCapturerSource? _source;
  int _capturedAt = 0;

  /// The frame rate the shared screen is captured at now; 0 while not sharing.
  @visibleForTesting
  int get capturedFps => _capturedAt;

  UiMediaBridge? _bridge;
  RTCPeerConnection? _engine;
  Timer? _levels;
  Timer? _measuring;

  /// Tells this device's state again now and then (see [stateAgainEvery]).
  Timer? _stateAgain;
  bool _closed = false;
  final _early = EarlySignals();

  /// Signals waiting for their sender's connection here (see [EarlySignals]), for tests.
  @visibleForTesting
  int get signalsWaiting => _early.waiting;

  /// The local renderers being made ready by [start]; [stop] waits for them.
  Future<void>? _rendering;

  /// Starts, and microphones, cameras and screens being opened, while the system may be
  /// asking whether Fogata may use them. A call that ends meanwhile keeps the voice engine
  /// until they are done, so what they opened is let go of before it stops.
  int _opening = 0;

  /// Completes once [stop] has let go of everything but the voice engine, including this
  /// device's own tracks and participants whose letting go was still going on when its
  /// time was up: the one condition on which the voice engine closes, whichever closes it
  /// ([stop], or the last of the starts and devices still opening).
  final _released = Completer<void>();

  /// Participants that left, still being let go of ([setParticipants]): no longer in
  /// [peers], but [stop] waits for them too before the voice engine stops.
  final _leaving = <Future<void>>{};

  /// Runs [work] as a start or a device being opened is run, for tests of [stop].
  @visibleForTesting
  Future<T> openingForTests<T>(Future<T> Function() work) =>
      _whileOpening(work);

  Future<T> _whileOpening<T>(Future<T> Function() work) async {
    _opening++;
    try {
      return await work();
    } finally {
      _opening--;
      // The opening returns at once: what waits for the steps still letting go is the
      // voice engine's close, never the start or the device that asked.
      if (_closed && _opening == 0) _closeEngineWhenReleased();
    }
  }

  /// Closes the voice engine once [_released], however long that takes: closing it under a
  /// track still being let go of brought the whole application down on Linux, so it is
  /// never closed on a timer. A step that still hangs after a minute is noted, once.
  void _closeEngineWhenReleased() {
    unawaited(
      _released.future.then((_) => _letGo('the voice engine', _closeEngine)),
    );
    if (_released.isCompleted) return;
    final noted = letGoLimit * 12;
    unawaited(
      Future<void>.delayed(noted, () {
        if (_released.isCompleted) return;
        _trace(
          'the voice engine stays open: letting go of a track or a participant still '
          'goes on after ${noted.inSeconds} s',
        );
      }),
    );
  }

  Future<void> _closeEngine() async {
    final engine = _engine;
    _engine = null;
    if (engine != null) await closeConnection(engine);
  }

  /// Says what went wrong, unless the call is over.
  void _report(String text) {
    problem = text;
    if (_closed) return;
    onProblem?.call(text);
    notifyListeners();
  }

  /// Stops and lets go of a stream that is not wanted any more.
  static Future<void> _free(MediaStream stream) async {
    for (final track in stream.getTracks()) {
      await track.stop();
    }
    await stream.dispose();
  }

  /// Every step checks whether the call ended meanwhile, and stops there.
  Future<void> start() => _whileOpening(() async {
    await (_rendering = _prepareRenderers());
    if (_closed) return;
    localScreen.addListener(_rescale);
    localScreen.onFrameTooLarge = localScreenTooLarge;
    localCamera.addListener(_stepCamera);
    final UiMediaBridge bridge;
    try {
      bridge = await api.mediaBridge();
    } catch (e) {
      _trace('media bridge: $e');
      _report(l10n.callSetupFailed);
      return;
    }
    if (_closed) return;
    _bridge = bridge;
    // WebRTC's voice engine starts with the first connection, picking the system's default
    // devices; our copy of flutter_webrtc keeps it running from then on (its FOGATA.md,
    // "The voice engine stays"). An idle connection kept for the whole call keeps it
    // running all the same, so the devices chosen here stay.
    _engine = await createPeerConnection({});
    if (_closed) return;
    await suppressNoise(noiseSuppression);
    await _chooseOutput();
    await _chooseInput();
    if (mic && !_closed) await _openMic();
    if (camera && !_closed) await _openCamera();
    if (_closed) return;
    // Someone may have connected while the microphone or the camera opened: what was agreed
    // with them then had nothing on those channels, and nothing else puts it there.
    await _resend(_audio);
    await _resend(_camera);
    if (_closed) return;
    _levels = Timer.periodic(
      const Duration(milliseconds: 600),
      (_) => _readLevels(),
    );
    _measuring = Timer.periodic(const Duration(seconds: 2), (_) => _measure());
    _stateAgain = Timer.periodic(stateAgainEvery, (_) => _sendState());
    navigator.mediaDevices.ondevicechange = _devicesChanged;
    _followingDevices = true;
    // The microphone chosen may be gone already when the call starts.
    unawaited(_readDevices());
    notifyListeners();
  });

  /// Chooses the input the call records from; the system's default too, by its own id (the
  /// first listed): the voice engine keeps the input it was last given from one call to
  /// the next, and from a microphone test to a call. That may be another microphone, or
  /// (Linux) a silent input left by a call that ended as it was being chosen; recording
  /// from it, every call after would send only silence, and nothing would notice.
  Future<void> _chooseInput() async {
    await _select(input ?? await _first('audioinput'), Helper.selectAudioInput);
  }

  /// Chooses the output the call plays through, the system's default too, as
  /// [_chooseInput] does.
  Future<void> _chooseOutput() async {
    await _select(
      output ?? await _first('audiooutput'),
      Helper.selectAudioOutput,
    );
  }

  /// Chooses the input as [start] does, for tests.
  @visibleForTesting
  Future<void> chooseInputForTests() => _chooseInput();

  /// Chooses the output as [start] does, for tests.
  @visibleForTesting
  Future<void> chooseOutputForTests() => _chooseOutput();

  Future<void> _prepareRenderers() async {
    await localCamera.initialize();
    await localScreen.initialize();
  }

  /// Ends the call and lets go of all it holds, once however many ask. Each step goes on
  /// whatever the one before did: one that fails or hangs ([_letGo]) leaves the others to
  /// be let go of all the same, and the next call can start.
  Future<void> stop() => _stopping ??= _stop();
  Future<void>? _stopping;

  /// This device's own streams and the voice engine, for tests of [stop].
  @visibleForTesting
  void holdForTests({
    List<MediaStream?> streams = const [],
    RTCPeerConnection? engine,
  }) {
    _micStream = streams.elementAtOrNull(0);
    _cameraStream = streams.elementAtOrNull(1);
    _screenStream = streams.elementAtOrNull(2);
    _engine = engine;
  }

  Future<void> _stop() async {
    _closed = true;
    // Only a call that started follows the devices (one made for tests never touches them).
    _devicesSoon?.cancel();
    if (_followingDevices &&
        navigator.mediaDevices.ondevicechange == _devicesChanged) {
      navigator.mediaDevices.ondevicechange = null;
    }
    // Steps of this device's own tracks still going on once their time is up.
    final hung = <Future<void>>[];
    try {
      _levels?.cancel();
      _measuring?.cancel();
      _stateAgain?.cancel();
      // Before the tracks, as every connection sending them.
      await _letGo('the own level', _alone.close);
      await _letGo('what is sent', () => screenOut.value = const {});
      // This device's own tracks go first, while WebRTC's voice engine still runs: an
      // audio track freed after the engine stopped crashed the application on Linux.
      final streams = [_micStream, _cameraStream, _screenStream].nonNulls;
      _micStream = null;
      _cameraStream = null;
      _screenStream = null;
      for (final stream in streams) {
        var tracks = const <MediaStreamTrack>[];
        await _letGo('a stream', () => tracks = stream.getTracks());
        for (final track in tracks) {
          await _letGo('a track', track.stop, late: hung.add);
        }
        await _letGo('a stream', stream.dispose, late: hung.add);
      }
      // All together, so that leaving waits for one at most however many there are. The
      // steps of each take up to [letGoLimit] ([CallPeer.disposeSteps] of them): each is
      // let go of whole before the voice engine stops, or after those that hang. Those
      // that left and are still being let go of too.
      await Future.wait([
        for (final peer in peers.values.toList())
          _letGo(
            peer.id,
            peer.dispose,
            late: hung.add,
            limit: letGoLimit * CallPeer.disposeSteps,
          ),
        for (final leaving in _leaving.toList())
          _letGo(
            'a participant that left',
            () => leaving,
            late: hung.add,
            limit: letGoLimit * CallPeer.disposeSteps,
          ),
      ]);
      peers.clear();
    } finally {
      // Only once the steps that hung are done too, for the reason above.
      unawaited(
        Future.wait(hung).whenComplete(() {
          if (!_released.isCompleted) _released.complete();
        }),
      );
    }
    // A start or a device still opening (the system asking about the microphone, say)
    // closes the voice engine itself once done, after [_released] as here.
    if (_opening == 0) {
      if (hung.isEmpty) {
        await _released.future;
        await _letGo('the voice engine', _closeEngine);
      } else {
        _closeEngineWhenReleased();
      }
    }
    // Nothing records any more.
    await _letGo('the input level', _loudInput);
    if (_ownSound) {
      _ownSound = false;
      await _letGo('the screen sound', () => _takeSound(false));
    }
    // Made ready by start(), which may still be at it.
    await _letGo('the own pictures', () async {
      await _rendering;
      localCamera.srcObject = null;
      localScreen.srcObject = null;
    });
    await _letGo('the own camera picture', localCamera.dispose);
    await _letGo('the own screen picture', localScreen.dispose);
  }

  // ----- Local media -----

  /// Turns noise suppression on or off, at once also in the middle of the call: the
  /// microphone stays open and keeps sending.
  Future<void> useNoiseSuppression(bool on) async {
    noiseSuppression = on;
    if (!_closed) await suppressNoise(on);
  }

  /// What goes to [to] on a channel: the shared screen and its sound only to those who
  /// watch it.
  MediaStreamTrack? _track(int channel, CallPeer to) => switch (channel) {
    _audio => mic ? _micStream?.getAudioTracks().firstOrNull : null,
    _camera => camera ? _cameraStream?.getVideoTracks().firstOrNull : null,
    _screen =>
      sharing && to.watches == me
          ? _screenStream?.getVideoTracks().firstOrNull
          : null,
    _screenSound => sharing && to.watches == me ? _soundTrack : null,
    _ => null,
  };

  /// The sound sent with this device's shared screen. WebRTC opens its input for any
  /// channel of sound that sends, whatever it sends, so with the microphone off the
  /// screen's sound only goes while that input is the silent one.
  MediaStreamTrack? get _soundTrack => shareSound && (mic || _silent)
      ? _screenStream?.getAudioTracks().firstOrNull
      : null;

  /// WebRTC records from the silent input, not the microphone.
  bool _silent = false;

  /// The changes of input between the silent one and the microphone, one after the other
  /// ([_inputTurn]).
  Future<void> _inputs = Future.value();

  bool get _wantSilence =>
      _silentInputs &&
      !_closed &&
      !mic &&
      sharing &&
      shareSound &&
      _screenStream?.getAudioTracks().isNotEmpty == true;

  /// The silent input of this process. Every Fogata running makes its own, named after
  /// its process; another one's goes away with it, and a recording whose input goes away
  /// moves to the system's default microphone.
  static final _ownSilence = '$silenceLabel ($pid)';

  static Future<String?> _ownSilenceId() async {
    final devices = await navigator.mediaDevices.enumerateDevices();
    return devices
        .where((d) => d.kind == 'audioinput' && d.label.contains(_ownSilence))
        .firstOrNull
        ?.deviceId;
  }

  /// Whether WebRTC's voice engine can start, as far as can be told: on Linux it ends the
  /// whole application when it reaches no sound server (PulseAudio, or PipeWire's service
  /// of it), so the application asks first (`fogataSoundServer`, up to 5 s each time, off
  /// the platform thread; see [soundServerAfter]). Every other system counts as having one.
  static Future<bool> soundServer() async {
    if (!onLinux) return true;
    return soundServerAfter(() async {
      try {
        final answer = await WebRTC.invokeMethod<String, dynamic>(
          'fogataSoundServer',
        );
        _trace('sound server: $answer');
        return answer;
      } catch (e) {
        _trace('sound server: $e');
        return null;
      }
    });
  }

  /// Whether a call may start, from the platform's answers to [ask] ("ready", "none",
  /// "timeout", "unknown", or null when it could not be asked). A definite "none" stops it.
  /// A server that took the connection but did not answer in 5 s is asked once more: a
  /// slow one (a fresh session, a busy computer) answers then. One that still does not
  /// answer stops it too: WebRTC's own connection to it fails as well, and that ends the
  /// application (seen with a server that never answers, tools/call-lab/mic.sh hang).
  /// What cannot be told lets it start.
  @visibleForTesting
  static Future<bool> soundServerAfter(Future<String?> Function() ask) async {
    var answer = await ask();
    if (answer == 'timeout') answer = await ask();
    return answer != 'none' && answer != 'timeout';
  }

  static Future<bool> _silentInput(bool on) async {
    try {
      return await WebRTC.invokeMethod<bool, dynamic>('fogataSilentInput', {
            'on': on,
          }) ??
          false;
    } catch (e) {
      _trace('silent input: $e');
      return false;
    }
  }

  /// The other applications' sound is being taken for this device's shared screen, to go
  /// on connections of its own; [_soundOf] is the shared window it was taken for, null
  /// for a whole screen.
  bool _ownSound = false;
  String? _soundOf;

  /// Starts or stops taking the sound that goes with a shared screen (macOS): with a
  /// [window], only the sound of the application it belongs to, so what other
  /// applications play stays on this computer.
  static Future<bool> _takeSound(bool on, {String? window}) async {
    try {
      return await WebRTC.invokeMethod<bool, dynamic>('fogataScreenSound', {
            'on': on,
            'window': ?window,
          }) ??
          false;
    } catch (e) {
      _trace('screen sound: $e');
      return false;
    }
  }

  /// This device's shared screen goes with sound.
  bool get sharingSound =>
      _soundConnections ? sharing && shareSound && _ownSound : _soundInChannel;

  /// The screen's sound goes on the fourth channel of each connection.
  bool get _soundInChannel => sharing && _soundTrack != null;

  /// The screen's sound waits for the microphone: shared with sound while muted.
  bool get soundWaitsForMic =>
      sharing &&
      shareSound &&
      !mic &&
      !_silent &&
      _screenStream?.getAudioTracks().isNotEmpty == true;

  /// Whether this device's shared screen can take the other applications' sound. Where
  /// the capture brings it (Linux, Windows), only while it did: Linux gives none for a
  /// window, whose sound cannot be narrowed to its application, nor without PipeWire,
  /// where it would carry the call's own sound.
  bool get canShareSound =>
      _soundConnections ||
      (_displaySound && _screenStream?.getAudioTracks().isNotEmpty == true);

  /// Linux: this device's shared screen goes without the other applications' sound
  /// because no PipeWire server takes a connection here, which the share's sound button
  /// says instead (see [needsPipeWire]). Known once a share without sound began.
  bool soundNeedsPipeWire = false;

  /// Whether a share that came without sound came so for want of PipeWire: on Linux,
  /// for a screen ([window] is a window picked from the list: none has sound there,
  /// whatever the server), and when the video library says no PipeWire server takes a
  /// connection ([server], `fogataScreenSoundServer`; null when it could not be asked,
  /// as with an older library, which says nothing then).
  @visibleForTesting
  static bool needsPipeWire({
    required bool linux,
    required bool sound,
    required bool window,
    required String? server,
  }) => linux && !sound && !window && server == 'none';

  /// The video library's word on PipeWire for a shared screen's sound (Linux).
  static Future<String?> _screenSoundServer() async {
    try {
      return await WebRTC.invokeMethod<String, dynamic>(
        'fogataScreenSoundServer',
      );
    } catch (e) {
      _trace('screen sound server: $e');
      return null;
    }
  }

  /// Sends the sound of the shared screen, or stops it, without stopping the picture.
  Future<void> setShareSound(bool on) async {
    shareSound = on;
    await _soundChannel();
  }

  /// The screen's sound channel only sends while there is sound to send, so it never
  /// opens the microphone on its own; each change is agreed again with every participant.
  Future<void> _soundChannel() async {
    if (_soundConnections) {
      final want = sharing && shareSound && !_closed;
      final source = _source;
      final window = source?.type == SourceType.Window ? source!.id : null;
      // Another window shared, or a screen: taken again for it.
      if (want && _ownSound && window != _soundOf) {
        _ownSound = false;
        await _takeSound(false);
      }
      if (want && !_ownSound) {
        _soundOf = window;
        _ownSound = await _takeSound(true, window: window);
        // The call ended while it started: [_stop] found nothing to stop then.
        if (_ownSound && _closed) {
          _ownSound = false;
          await _takeSound(false);
        }
      } else if (!want && _ownSound) {
        _ownSound = false;
        await _takeSound(false);
      }
    }
    await _quietInput();
    final changed = _soundInChannel != _soundSending;
    _soundSending = _soundInChannel;
    await _eachPeer('screen sound', (peer) async {
      await _soundTo(peer);
      final transceivers = await _channels(peer.connection);
      if (transceivers.length <= _screenSound) return;
      final channel = transceivers[_screenSound];
      await channel.sender.replaceTrack(_track(_screenSound, peer));
      if (changed) {
        await channel.setDirection(_soundDirection);
        await _renegotiate(peer);
      }
    });
    _sendState();
    notifyListeners();
  }

  /// Runs [step] for each participant there is now, one after the other (a copy:
  /// participants can come and go meanwhile). One whose connection fails meanwhile
  /// (closed, or replaced as it is made again) is noted, and the others still get theirs:
  /// a mute stopped at the first failure left the rest hearing "unmuted", and the state
  /// was not sent.
  Future<void> _eachPeer(
    String what,
    Future<void> Function(CallPeer peer) step,
  ) async {
    for (final peer in peers.values.toList()) {
      if (peers[peer.id] != peer) continue;
      try {
        await step(peer);
      } catch (e) {
        _trace('$what for ${peer.id}: $e');
      }
    }
  }

  /// The direction the screen's sound channels were last given.
  bool _soundSending = false;

  TransceiverDirection get _soundDirection => _soundInChannel
      ? TransceiverDirection.SendRecv
      : TransceiverDirection.RecvOnly;

  /// The others watching this device's shared screen.
  int get watchers => peers.values.where((p) => p.watches == me).length;

  /// How many watch [id]'s shared screen, this device included.
  int watchersOf(String id) =>
      peers.values.where((p) => p.watches == id).length +
      (watching == id ? 1 : 0);

  /// Starts or stops watching a participant's shared screen; it is sent here only while
  /// watched.
  /// How the camera or the screen of `id` arrives here now. Losses count since the last
  /// reading of the same picture, so reading every few seconds gives the recent share.
  Future<StreamQuality?> qualityOf(String id, {required bool screen}) async {
    final peer = peers[id];
    final connection = peer?.connection;
    if (peer == null || connection == null) return null;
    final channel = screen ? _screen : _camera;
    final track = (screen ? peer.screen : peer.camera).srcObject
        ?.getVideoTracks()
        .firstOrNull
        ?.id;
    final List<StatsReport> reports;
    try {
      reports = await connection.getStats();
    } catch (_) {
      return null;
    }
    final delay = roundTripMs(reports);
    Map<dynamic, dynamic>? video;
    for (final report in reports) {
      final v = report.values;
      if (report.type == 'inbound-rtp' && v['kind'] == 'video') {
        final mid = v['mid'];
        final ours = mid != null
            ? '$mid' == '$channel'
            : track != null && v['trackIdentifier'] == track;
        if (ours) video = v;
      }
    }
    double? loss;
    int? width, height;
    double? fps;
    if (video != null) {
      final lost = (video['packetsLost'] as num?)?.toInt() ?? 0;
      final received = (video['packetsReceived'] as num?)?.toInt() ?? 0;
      final before = peer._packets[channel];
      peer._packets[channel] = (lost, received);
      if (before != null) {
        final newLost = math.max(0, lost - before.$1);
        final newReceived = math.max(0, received - before.$2);
        final total = newLost + newReceived;
        if (total > 0) loss = newLost * 100 / total;
      }
      width = (video['frameWidth'] as num?)?.toInt();
      height = (video['frameHeight'] as num?)?.toInt();
      fps = (video['framesPerSecond'] as num?)?.toDouble();
    }
    return StreamQuality(
      level: StreamQuality.levelOf(delay, loss),
      delayMs: delay,
      lossPercent: loss,
      width: width,
      height: height,
      fps: fps,
    );
  }

  /// Someone whose screen was watched, or whose camera filled the room, and who went out
  /// of the room: if they come back soon (their network was gone a while, say) and share
  /// it still, it is shown again, unless something else is shown meanwhile.
  ({String id, DateTime until})? _watchBack;
  ({String id, DateTime until})? _focusBack;
  static const _backWithin = Duration(minutes: 3);

  /// Shows again what [_watchBack] or [_focusBack] kept, now that [peer] says what it
  /// shares.
  void _showBack(CallPeer peer) {
    final now = DateTime.now();
    final shown = watching != null || focused != null || ownScreen;
    final screen = _watchBack;
    if (screen != null && screen.id == peer.id) {
      _watchBack = null;
      if (!shown && peer.sharing && now.isBefore(screen.until)) {
        watching = peer.id;
        _focusBack = null;
        _sendState();
        return;
      }
    }
    final camera = _focusBack;
    if (camera != null && camera.id == peer.id) {
      _focusBack = null;
      if (!shown && peer.cam && now.isBefore(camera.until)) focused = peer.id;
    }
  }

  void watch(String? id) {
    _watchBack = null;
    _focusBack = null;
    if (watching == id) return;
    watching = id;
    if (id != null) {
      focused = null;
      ownScreen = false;
    }
    onSound?.call(id != null ? UiSound.watchStart : UiSound.watchStop);
    _sendState();
    notifyListeners();
  }

  /// Enlarges a participant's camera, or brings the room back; one at a time, so a
  /// watched screen stops being watched.
  void focus(String? id) {
    _watchBack = null;
    _focusBack = null;
    if (focused == id) return;
    if (id != null) {
      watch(null);
      ownScreen = false;
    }
    focused = id;
    notifyListeners();
  }

  /// Enlarges this device's own shared screen, or brings the room back.
  void showOwnScreen(bool on) {
    _watchBack = null;
    _focusBack = null;
    if (ownScreen == on) return;
    if (on) {
      watch(null);
      focused = null;
    }
    ownScreen = on;
    notifyListeners();
  }

  /// The microphone is open only while it is on. Muted, the voice channel only receives,
  /// so WebRTC stops recording and the system shows the microphone unused; WebRTC records
  /// from the default microphone whenever a voice channel may send, even with nothing to
  /// send. Each change is agreed again with every participant.
  Future<void> setMic(bool on) async {
    mic = on;
    _micCheck.reset();
    if (on) {
      if (_micStream == null) await _openMic();
      await _loudInput();
    } else {
      // To the silent input first, if the screen's sound keeps going out, so the
      // microphone is not held open by it.
      await _quietInput();
      await _closeMic();
    }
    await _eachPeer('microphone', (peer) async {
      final transceivers = await _channels(peer.connection);
      if (transceivers.isEmpty) return;
      await transceivers[_audio].sender.replaceTrack(_track(_audio, peer));
      await transceivers[_audio].setDirection(_voiceDirection(peer));
      if (transceivers.length > _screenSound) {
        await transceivers[_screenSound].sender.replaceTrack(
          _track(_screenSound, peer),
        );
        await transceivers[_screenSound].setDirection(_soundDirection);
      }
      _soundSending = _soundInChannel;
      await _renegotiate(peer);
    });
    _sendState();
    notifyListeners();
  }

  TransceiverDirection _voiceDirection(CallPeer peer) => peer._voicePaused
      ? (mic ? TransceiverDirection.SendOnly : TransceiverDirection.Inactive)
      : (mic ? TransceiverDirection.SendRecv : TransceiverDirection.RecvOnly);

  Future<void> setCamera(bool on) async {
    final was = camera;
    camera = on;
    if (on && _cameraStream == null) {
      await _openCamera();
    } else if (!on) {
      await _closeCamera();
      if (focused == me) focused = null;
    }
    await _resend(_camera);
    if (camera != was) {
      onSound?.call(camera ? UiSound.shareStart : UiSound.shareStop);
    }
  }

  // ----- Devices -----

  /// Changes the microphone in the middle of the call; null is the system's default.
  Future<void> useInput(String? id) async {
    input = id;
    // Another input is judged from the start.
    _micCheck.reset();
    // Taken when the silent input is left.
    if (_silent) return;
    await _select(id ?? await _first('audioinput'), Helper.selectAudioInput);
  }

  /// Changes the speakers or headphones; null is the system's default.
  Future<void> useOutput(String? id) async {
    output = id;
    await _select(id ?? await _first('audiooutput'), Helper.selectAudioOutput);
  }

  /// Changes the camera; if it is on, the others see the new one at once.
  Future<void> useCamera(String? id) async {
    cameraId = id;
    if (!camera) return;
    await _closeCamera();
    await _openCamera();
    await _resend(_camera);
  }

  /// The voice engine lists the system's default device first (on Windows, where
  /// libwebrtc's own list has no default first, our copy of flutter_webrtc puts it there:
  /// its FOGATA.md, "The system's default microphone and speaker").
  static Future<String?> _first(String kind) async {
    try {
      final devices = await navigator.mediaDevices.enumerateDevices();
      final listed = devices
          .where((d) => d.kind == kind && !d.label.contains(silenceLabel))
          .toList();
      // Which device the system's default was taken for, in the call's log.
      _trace(
        '$kind, the first taken: ${listed.map((d) => d.label).join(' | ')}',
      );
      return listed.firstOrNull?.deviceId;
    } catch (_) {
      return null;
    }
  }

  /// A device that is no longer connected leaves the current one in use.
  static Future<void> _select(
    String? id,
    Future<void> Function(String) select,
  ) async {
    if (id == null) return;
    try {
      await select(id);
    } catch (e) {
      _trace('device $id: $e');
    }
  }

  /// The screens and windows that can be shared. Under Wayland there is no list: the
  /// system's own dialog offers screens and windows when sharing starts. Fogata's own
  /// window is not offered (shared, the call's tiles would show the share inside itself):
  /// macOS and Windows leave it out of the list by its process; under X11 the list does
  /// not tell a window's process, and it is known by its title.
  Future<List<DesktopCapturerSource>> screens() async {
    if (await screenThroughPortal()) return const [];
    final sources = await desktopCapturer.getSources(
      types: [SourceType.Screen, SourceType.Window],
    );
    return [
      for (final source in sources)
        if (!ownWindow(source.type, source.name, linux: onLinux)) source,
    ];
  }

  Future<void> shareScreen(DesktopCapturerSource? source) =>
      _whileOpening(() async {
        await stopSharing(resend: false);
        await _askWayland();
        MediaStream? stream;
        // The encoder does not keep to a lower rate by itself, so the screen is captured
        // at the rate chosen. Without a source (no list to pick from) the system's own
        // dialog or the library's default rate decides.
        final fps = source != null ? screenQuality.fps : (_wayland ? 60 : 30);
        try {
          stream = await navigator.mediaDevices.getDisplayMedia(
            _capture(source, fps),
          );
        } catch (e) {
          // Closing the system's dialog is a choice, not a failure.
          if (!'$e'.contains('cancelled')) {
            _trace('screen: $e');
            _report(l10n.callScreenFailed(_system));
          }
        }
        // The call ended while the system's dialog was open, or another share began.
        if (stream != null && (_closed || _screenStream != null)) {
          await _free(stream);
          return;
        }
        if (_closed) return;
        sharing = stream != null;
        if (stream != null) {
          _screenStream = stream;
          _source = source;
          _capturedAt = fps;
          localScreen.srcObject = stream;
          stream.getVideoTracks().firstOrNull?.onEnded = () => stopSharing();
          if (source == null) await _portalRate(screenQuality.fps);
          await _screenSized();
          // Asked only when that is the question: a share with sound, or of a window,
          // has its answer.
          final sound = stream.getAudioTracks().isNotEmpty;
          final window = source?.type == SourceType.Window;
          final server = onLinux && !sound && !window
              ? await _screenSoundServer()
              : null;
          soundNeedsPipeWire =
              _screenStream == stream &&
              needsPipeWire(
                linux: onLinux,
                sound: sound,
                window: window,
                server: server,
              );
        }
        await _resetFits();
        await _resend(_screen);
        await _soundChannel();
        if (sharing) onSound?.call(UiSound.shareStart);
      });

  Future<void> stopSharing({bool resend = true}) async {
    screenOut.value = const {};
    if (_screenStream != null) {
      for (final track in _screenStream!.getTracks()) {
        await track.stop();
      }
      await _screenStream!.dispose();
      _screenStream = null;
    }
    _source = null;
    _capturedAt = 0;
    _screenTooLarge = null;
    soundNeedsPipeWire = false;
    localScreen.srcObject = null;
    final was = sharing;
    sharing = false;
    ownScreen = false;
    if (resend && was) {
      await _resend(_screen);
      await _soundChannel();
    }
    if (was) onSound?.call(UiSound.shareStop);
  }

  /// Sets how this device's shared screen goes out. It applies at once: the encoder is
  /// told the new size, and the screen is captured again at the new rate (or, captured
  /// through the system's dialog, lets that rate through). Chosen before a share, every
  /// connection is told the new rate at once too: a share only puts its track on the
  /// connections, and kept the rate they were told last (30 frames a second sent of 60
  /// captured, when 60 was chosen in the call before sharing).
  Future<void> setScreenQuality(ScreenQuality quality) async {
    if (quality == screenQuality) return;
    screenQuality = quality;
    notifyListeners();
    if (_closed) return;
    if (!sharing) {
      await _retune();
      return;
    }
    if (_source == null) {
      await _portalRate(quality.fps);
    } else if (quality.fps != _capturedAt) {
      await _recapture();
    }
    _screenScale = _scaleNow();
    _trace('screen out: ${quality.label}, scaled down by $_screenScale');
    for (final peer in peers.values) {
      peer._fit.reset();
    }
    await _retune();
  }

  /// Puts the current track of a channel on every connection and tells everyone.
  Future<void> _resend(int channel) async {
    await _eachPeer('channel $channel', (peer) => _resendTo(peer, channel));
    _sendState();
    notifyListeners();
  }

  Future<void> _resendTo(CallPeer peer, int channel) async {
    final transceivers = await _channels(peer.connection);
    if (transceivers.length > channel) {
      await transceivers[channel].sender.replaceTrack(_track(channel, peer));
    }
  }

  // ----- Participants and signals -----

  /// [letting], the letting go of a participant that left, kept in [_leaving] until done.
  Future<void> _whileLeaving(Future<void> letting) {
    late final Future<void> tracked;
    tracked = letting.whenComplete(() => _leaving.remove(tracked));
    _leaving.add(tracked);
    return tracked;
  }

  /// The room's participants changed: connect to new ones and drop those who left.
  Future<void> setParticipants(Iterable<String> ids) async {
    if (_closed) return;
    final others = ids.where((id) => id != me).toSet();
    _early.participants(others);
    _people = others.length + 1;
    _stepCamera();
    final left = [
      for (final id in peers.keys.where((id) => !others.contains(id)).toList())
        peers.remove(id)!,
    ];
    _voiceless.removeWhere((id) => !others.contains(id));
    // All together, so that several leaving at once hold the call up for one at most;
    // known to [stop] until they are done, so the voice engine waits for them.
    await Future.wait([for (final peer in left) _whileLeaving(peer.dispose())]);
    for (final gone in left.map((peer) => peer.id)) {
      _showScreenOut(gone, null);
      final until = DateTime.now().add(_backWithin);
      if (watching == gone) {
        watching = null;
        _watchBack = (id: gone, until: until);
      }
      if (focused == gone) {
        focused = null;
        _focusBack = (id: gone, until: until);
      }
    }
    if (_bridge == null) return;
    quality.keep(others);
    _trace(
      'participants: ${others.map((id) => id.substring(0, 8)).join(', ')}',
    );
    for (final id in others) {
      // Someone the room has said since is no longer in it (several leaving at once give
      // changes one after the other, and an earlier one may get here after a later one).
      if (peers.containsKey(id) || !_early.isParticipant(id)) continue;
      final peer = CallPeer(id)..voiceless = _voiceless.contains(id);
      peers[id] = peer;
      await peer._prepare();
      // The call ended, or the participant left, while its pictures were made ready: what
      // let it go then found them not ready yet.
      if (_closed || peers[id] != peer) {
        await peer.dispose();
        continue;
      }
      // The device with the smaller id makes the offer; the other one waits for it. Nobody
      // dials someone who said they have no call.
      if (peer.voiceless) continue;
      if (me.compareTo(id) < 0) {
        try {
          await peer._inTurn(() => _offer(peer));
        } catch (e) {
          _trace('offer to ${id.substring(0, 8)}: $e');
        }
      } else {
        // Asked for an offer at once. One that has no connection with this device offers
        // by itself as it hears this device came in, and lets this pass (its offer is on
        // its way); one that kept a connection from before, with a call here that is gone
        // (this device crashed, or its leaving and coming back reached it as one), would
        // otherwise offer only when the first asking goes, seconds later.
        _signal(id, {'t': 'again'});
      }
      _watch(peer);
      // Its signals may have come before it was known to be in the room, or before its
      // connection here could be made.
      for (final message in _early.take(id)) {
        await peer._inTurn(() => _handleSafely(peer, id, message));
      }
    }
    notifyListeners();
  }

  /// The signal a device in the room without a call sends (see [AppState]'s refused call):
  /// whoever gets it stops connecting to it and shows it as there without sound.
  static const noVoice = 'novoice';

  /// Who said they are in the room without a call, until they leave it or send anything
  /// else.
  final Set<String> _voiceless = {};

  Future<void> _theyHaveNoCall(String from, CallPeer? peer) async {
    _voiceless.add(from);
    if (peer == null || peer.voiceless) return;
    peer.voiceless = true;
    await peer._inTurn(peer._close);
    notifyListeners();
  }

  // ----- The microphone going away -----

  /// Whether the microphone, on, reads anything.
  final _micCheck = MicCheck();

  /// Hands a microphone that reads nothing to whoever shows the call, with what to say;
  /// without one, the call says it and turns its microphone off itself.
  void Function(String text)? onMicUnusable;

  /// What to say of a microphone that reads nothing, with the system's devices as listed
  /// now (null when they could not be read) on [system] ("macos", "windows", "other"):
  /// that none is connected when the list has no input at all, or else that it could not
  /// be used, and where to look.
  @visibleForTesting
  static String micUnusable(List<MediaDeviceInfo>? devices, String system) =>
      devices != null && !devices.any((d) => d.kind == 'audioinput')
      ? l10n.callNoMicrophone
      : l10n.callMicSilent(system);

  /// The microphone chosen going away and coming back, told once each way.
  final _micWatch = MicWatch();

  bool _checkingDevices = false;
  bool _followingDevices = false;

  /// A change told while the list was being read: read again once it is done.
  bool _devicesAgain = false;

  /// Changes come in bursts (a Bluetooth headset switching profiles goes and comes back
  /// within a moment): the list is read once they settle.
  static const devicesSettle = Duration(milliseconds: 300);
  Timer? _devicesSoon;

  void _devicesChanged(dynamic _) {
    if (_closed) return;
    _devicesSoon?.cancel();
    _devicesSoon = Timer(devicesSettle, () => unawaited(_readDevices()));
  }

  /// Reads the system's devices; tests give their own.
  @visibleForTesting
  Future<List<MediaDeviceInfo>> Function() listDevices = () =>
      navigator.mediaDevices.enumerateDevices();

  /// The system's devices changed, as its event says; for tests.
  @visibleForTesting
  void devicesChangedForTests() => _devicesChanged(null);

  /// Takes a list of the system's devices, as read when they change: tells the person when
  /// the microphone chosen went away or came back.
  @visibleForTesting
  void takeDevices(List<MediaDeviceInfo> devices) {
    final notice = _micWatch.take(input, devices);
    if (notice != null) {
      _trace('microphone: $notice');
      onNotice?.call(notice);
    }
  }

  Future<void> onSignal(String from, String text) async {
    if (_closed) return;
    final Map<String, dynamic> message;
    try {
      message = jsonDecode(text) as Map<String, dynamic>;
    } catch (_) {
      return;
    }
    _trace('got ${message['t']} from ${from.substring(0, 8)}');
    final peer = peers[from];
    if (message['t'] == noVoice) {
      await _theyHaveNoCall(from, peer);
      return;
    }
    // Anything else from them: they have a call now.
    if (_voiceless.remove(from) && peer != null && peer.voiceless) {
      peer.voiceless = false;
      // Back in the call: tried again soon, as someone new.
      peer._waits = 0;
      _watch(peer);
      notifyListeners();
    }
    if (message['t'] == 'net') {
      // Their own network, kept apart from the call's state so it never redraws the
      // room; from someone not connected yet it is sent again once connected.
      if (peer != null && !peer._keptOut) {
        quality.theirs(from, NetLevel.named(message['l']));
      }
      return;
    }
    if (!fairSignal(message)) {
      _trace('${message['t']} from ${from.substring(0, 8)} too large: dropped');
      return;
    }
    if (peer == null) {
      // Only participants of the room get a connection. The room may just not have told
      // this device yet that the sender is in it, or this call may not have made its
      // connection with them yet (it is still starting: its media bridge, its microphone):
      // the signal waits for that. Dropped, the first offer of someone already in the room
      // when this device comes in would be lost, and the next one is seconds away.
      _early.keep(from, message);
      return;
    }
    if (peer._queued >= CallPeer.maxQueued && message['t'] != 'answer') {
      _trace(
        '${message['t']} from ${from.substring(0, 8)}: too many waiting, dropped',
      );
      return;
    }
    // A newer offer replaces one still waiting its turn. Offers are taken one a second
    // at most (see [CallPeer.offerEvery]); the newest of those that come meanwhile is the
    // one taken.
    if (message['t'] == 'offer') {
      final mine = ++peer._offersIn;
      await _offerTurn(peer, peer._offerTaken);
      if (mine != peer._offersIn) return;
      peer._offerTaken = DateTime.now();
      await peer._inTurn(() async {
        if (mine == peer._offersIn) await _handleSafely(peer, from, message);
      });
    } else if (message['t'] == 'sound') {
      final mine = ++peer._soundOffersIn;
      await _offerTurn(peer, peer._soundOfferTaken);
      if (mine != peer._soundOffersIn) return;
      peer._soundOfferTaken = DateTime.now();
      await peer._inTurn(() async {
        if (mine == peer._soundOffersIn) {
          await _handleSafely(peer, from, message);
        }
      });
    } else {
      await peer._inTurn(() => _handleSafely(peer, from, message));
    }
    notifyListeners();
  }

  /// Handles a signal from a participant already connected, as [onSignal] does once the
  /// call has its media bridge.
  @visibleForTesting
  Future<void> handleSignal(String from, Map<String, dynamic> message) async {
    final peer = peers[from];
    if (peer != null) await _handle(peer, from, message);
  }

  /// The number of the latest offer to [id], for tests.
  @visibleForTesting
  int? offerNumberTo(String id) => peers[id]?._offerNumber;

  /// Makes the connections with participants, in place of WebRTC, for tests.
  @visibleForTesting
  Future<RTCPeerConnection> Function()? connectionsForTests;

  /// Gives the call a media bridge without asking the core, for tests of [onSignal].
  @visibleForTesting
  set bridgeForTests(UiMediaBridge bridge) => _bridge = bridge;

  Future<RTCPeerConnection> _connect(CallPeer peer) async {
    if (peer._keptOut) throw StateError('kept out of the call');
    await peer._close();
    final connection =
        await (connectionsForTests?.call() ??
            createPeerConnection(_network(peer.id)));
    // The call ended, the participant left or was kept out meanwhile: nothing would ever
    // close it.
    if (_closed || peers[peer.id] != peer || peer._keptOut) {
      await closeConnection(connection);
      throw StateError('no longer in the call');
    }
    peer.connection = connection;
    peer._madeAt = DateTime.now();
    peer.state = RTCPeerConnectionState.RTCPeerConnectionStateNew;
    final gathered = peer._gathered = Completer<void>();
    connection.onTrack = (event) => _onTrack(peer, connection, event);
    connection.onIceGatheringState = (state) {
      _trace('gathering ${peer.id.substring(0, 8)}: $state');
      if (state == RTCIceGatheringState.RTCIceGatheringStateComplete &&
          !gathered.isCompleted) {
        gathered.complete();
      }
    };
    connection.onIceConnectionState = (state) =>
        _trace('ice ${peer.id.substring(0, 8)}: $state');
    var wasUp = false;
    var failed = false;
    connection.onConnectionState = (state) {
      // One this device replaced or closed says nothing of the connection there is now.
      if (peer.connection != connection) return;
      _trace('connection ${peer.id.substring(0, 8)}: $state');
      peer.state = state;
      failed = state == RTCPeerConnectionState.RTCPeerConnectionStateFailed;
      final up =
          state == RTCPeerConnectionState.RTCPeerConnectionStateConnected;
      if (up && !peer.connected) {
        _signal(peer.id, _state);
        _signal(peer.id, _net);
      }
      if (up) {
        wasUp = true;
        peer._attempt = 0;
      }
      peer.connected = up;
      if (state == RTCPeerConnectionState.RTCPeerConnectionStateFailed) {
        _retry(peer);
      }
      notifyListeners();
    };
    // A connection that never comes up is tried again as one that failed: one whose
    // descriptions do not match (offers that crossed, a signal lost on the way) may take
    // minutes to give up by itself.
    Future<void>.delayed(connectWithin, () {
      if (_closed ||
          peers[peer.id] != peer ||
          peer.connection != connection ||
          wasUp ||
          failed) {
        return;
      }
      _trace('connection ${peer.id.substring(0, 8)}: not up in time');
      _retry(peer);
    });
    return connection;
  }

  /// How long a new connection has to come up before it is tried again.
  static const connectWithin = Duration(seconds: 15);

  /// How many times a renegotiation's offer, or its asking for one, goes again unanswered.
  static const _renegotiationTries = 5;

  /// The wait before an unanswered renegotiation goes again, after [tries] went:
  /// [renegotiationAfter], then twice as long each time, six times it at most.
  static Duration renegotiationWait(int tries) {
    final wait = renegotiationAfter * (1 << tries.clamp(0, 3));
    return wait > renegotiationAfter * 6 ? renegotiationAfter * 6 : wait;
  }

  /// The people in the call, this device included, as the room last said.
  int _people = 1;

  /// Counts that many people for the camera instead of those in the call, for tests that
  /// measure a larger call's step with fewer copies.
  @visibleForTesting
  set peopleForCamera(int? people) {
    _peopleForCamera = people;
    _stepCamera();
  }

  int? _peopleForCamera;

  /// The most lines the camera's pictures had since it was opened, and the widest they
  /// were: what it gives.
  double _cameraLines = 0;
  double _cameraWidth = 0;

  /// How the camera goes out to everyone now (see [cameraStep]).
  ({CameraStep step, double scale}) _cameraOut = (
    step: cameraStep(1),
    scale: 1,
  );

  /// How much this device's shared screen is scaled down before it goes out, from the
  /// size it is captured at; 1 until its first picture tells.
  double _screenScale = 1;

  /// The most the shared screen may use at that scale, in kbit/s, when its size last
  /// changed (see [screenKbps]): a new size the scale keeps (a window made larger, within
  /// its quality) still changes it.
  int _screenKbps = 0;

  /// How far the shared screen is lowered for everyone, for this device's processor.
  final _screenLoad = ScreenLoad();

  /// The shared screen's lowering for the processor now, for tests.
  @visibleForTesting
  ScreenLoad get screenLoad => _screenLoad;

  /// The size of the shared screen when it is captured larger than its renderer draws
  /// (over 8K): the renderer then tells its size this way only, and draws nothing.
  ({double width, double height})? _screenTooLarge;

  /// The shared screen's renderer dropped a picture of `width` by `height`, too large to
  /// draw: it is scaled down from that size.
  @visibleForTesting
  void localScreenTooLarge(int width, int height) {
    _screenTooLarge = (width: width.toDouble(), height: height.toDouble());
    unawaited(_rescale());
  }

  /// How much the shared screen is scaled down now, for tests.
  @visibleForTesting
  double get screenScaleNow => _scaleNow();

  /// How much the shared screen goes out scaled down, as the connections were told, for
  /// tests.
  @visibleForTesting
  double get screenScaleSent => _screenScale;

  /// Takes the shared screen's size as its renderer says it now, as every change of it
  /// does, for tests.
  @visibleForTesting
  Future<void> rescaleNow() => _rescale();

  Future<void> _onTrack(
    CallPeer peer,
    RTCPeerConnection connection,
    RTCTrackEvent event,
  ) async {
    if (event.track.kind == 'audio') {
      final screen = event.transceiver?.mid == '$_screenSound';
      if (event.transceiver?.mid == '$_audio' &&
          peer.connection == connection) {
        peer._voice = (on: connection, track: event.track);
      }
      await _setGain(
        event.track,
        screen ? screenSoundGain(peer.id) : gainOf(peer.id),
      );
      return;
    }
    if (event.track.kind != 'video') return;
    final mid = int.tryParse(event.transceiver?.mid ?? '');
    await _show(peer, connection, mid, event.track, event.streams.firstOrNull);
  }

  /// Shows [track], the camera or the screen [connection] receives on channel [mid], in
  /// that channel's renderer: in [given], the stream it came in, if any.
  Future<void> _show(
    CallPeer peer,
    RTCPeerConnection connection,
    int? mid,
    MediaStreamTrack track,
    MediaStream? given,
  ) async {
    final MediaStream stream;
    if (given != null) {
      stream = given;
    } else {
      // A stream of this device's own to show it in, which the video library keeps until
      // it is let go of: it goes with the connection the track came on ([CallPeer._close]),
      // or now if that one is closed already. Kept, two were left for every connection
      // made in a call (camera and screen), each holding its track.
      stream = await createLocalMediaStream('${peer.id}-$mid');
      if (peer.connection != connection) {
        await _letGo('a stream of ${peer.id}', stream.dispose);
        return;
      }
      peer._shown.add(stream);
      try {
        await stream.addTrack(track);
      } catch (e) {
        // Let go of meanwhile, with its connection.
        _trace('a stream of ${peer.id}: $e');
        return;
      }
    }
    await peer._prepare();
    // Let go of meanwhile (gone from the room, or the call ended): its pictures are freed.
    // Or its connection was replaced meanwhile: the new one's pictures, which may have
    // come first, are the ones to show, and this track has ended.
    if (_closed ||
        peers[peer.id] != peer ||
        !peer.rendering ||
        peer.connection != connection) {
      return;
    }
    final renderer = switch (mid) {
      _camera => peer.camera,
      _screen => peer.screen,
      _ => null,
    };
    if (renderer == null || mid == null) return;
    _attach(peer, mid, renderer, stream);
    notifyListeners();
  }

  /// Follows, at each reading of the statistics, whether the screen this device watches
  /// shows a picture: one that shows none at the readings [pictureReadings] counts since
  /// it was watched on [connection] is shown again ([_showScreenAgain]), and its sharer is
  /// told again that this device watches it, as closing and opening it would. That is
  /// told every [stateAgainEvery] all the same, so a telling lost on its way is not lost
  /// for good, and changes nothing there if it had arrived; the sharer itself follows
  /// whether its screen goes out to whoever watches it ([_followScreenOut]).
  void _awaitPicture(CallPeer peer, RTCPeerConnection connection) {
    final wait = _pictureWait;
    final picture = peer.screen.value;
    if (watching != peer.id ||
        !peer.sharing ||
        !peer.rendering ||
        (picture.width > 0 && picture.height > 0)) {
      if (wait?.id == peer.id) _pictureWait = null;
      return;
    }
    final readings =
        (wait != null && wait.id == peer.id && wait.on == connection
            ? wait.readings
            : 0) +
        1;
    _pictureWait = (id: peer.id, on: connection, readings: readings);
    if (!pictureReadings.contains(readings)) return;
    _trace(
      'no picture of ${peer.id.substring(0, 8)}\'s screen yet: shown again, '
      'and told again',
    );
    _signal(peer.id, _state);
    unawaited(
      _showScreenAgain(peer, connection).catchError(
        (Object e) => _trace('${peer.id.substring(0, 8)}\'s screen again: $e'),
      ),
    );
  }

  /// Shows [peer]'s screen again from what [connection] receives on its channel now: the
  /// renderer starts over, and tells the size of the next picture it gets, as for a
  /// connection made anew.
  Future<void> _showScreenAgain(
    CallPeer peer,
    RTCPeerConnection connection,
  ) async {
    final channels = await _channels(connection);
    if (channels.length <= _screen) return;
    final track = channels[_screen].receiver.track;
    if (track == null || track.kind != 'video') return;
    if (_closed ||
        peers[peer.id] != peer ||
        !peer.rendering ||
        peer.connection != connection) {
      return;
    }
    final renderer = peer.screen;
    final shown = renderer.srcObject;
    if (shown != null && shown.getVideoTracks().any((t) => t.id == track.id)) {
      // Given nothing first: given the stream it shows, the video library would change
      // nothing.
      renderer.srcObject = null;
      _attach(peer, _screen, renderer, shown);
      return;
    }
    await _show(peer, connection, _screen, track, null);
  }

  /// Gives [renderer] the pictures of [stream]. Every picture of the track reaches its
  /// renderer, drawn or not: one larger than a participant sends is seen at once, not
  /// only if a reading of the statistics falls on it.
  void _attach(
    CallPeer peer,
    int mid,
    RTCVideoRenderer renderer,
    MediaStream stream,
  ) {
    renderer.onFrameTooLarge = (width, height) =>
        pictureArrived(peer.id, mid, width, height);
    renderer.onResize = () =>
        pictureArrived(peer.id, mid, renderer.videoWidth, renderer.videoHeight);
    renderer.onError = (error) => unawaited(_renew(peer, mid, renderer, error));
    renderer.srcObject = stream;
  }

  /// How many times a participant's renderer is made again after its channel failed
  /// (one that failed shows nothing more and only waits to be let go of).
  static const maxRenewals = 3;

  /// How many times this device's shared screen is sent again to someone who watches it
  /// and to whom nothing has gone out ([_followScreenOut]).
  static const screenNudges = 3;

  /// Puts a new renderer, with the same pictures, in the place of one that failed; after
  /// [maxRenewals] it is given up: what shows it is left, and the person is not kept
  /// looking at nothing.
  Future<void> _renew(
    CallPeer peer,
    int mid,
    RTCVideoRenderer failed,
    Object error,
  ) async {
    bool current() =>
        !_closed &&
        peers[peer.id] == peer &&
        peer.rendering &&
        (mid == _camera ? peer.camera : peer.screen) == failed;
    _trace(
      'picture of ${peer.id.substring(0, 8)} on channel $mid failed: $error',
    );
    final stream = failed.srcObject;
    if (!current()) return;
    if (stream == null || ++peer._renewed > maxRenewals) {
      if (mid == _screen && watching == peer.id) watch(null);
      if (mid == _camera && focused == peer.id) focus(null);
      notifyListeners();
      return;
    }
    final fresh = RTCVideoRenderer();
    try {
      await fresh.initialize();
    } catch (e) {
      _trace('making a picture of ${peer.id.substring(0, 8)} again: $e');
    }
    // Let go of meanwhile: nothing would free the new one.
    if (fresh.textureId == null || !current()) {
      await _letGo('a new picture of ${peer.id}', fresh.dispose);
      return;
    }
    if (mid == _camera) {
      peer.camera = fresh;
    } else {
      peer.screen = fresh;
    }
    try {
      // What the failed one shows by now: a connection made again meanwhile gave it the
      // new connection's pictures, and the stream taken before has ended; one kept out
      // meanwhile left it none.
      final shown = failed.srcObject;
      if (shown != null) _attach(peer, mid, fresh, shown);
    } catch (e) {
      _trace('showing a picture of ${peer.id.substring(0, 8)} again: $e');
    }
    notifyListeners();
    await _letGo('a failed picture of ${peer.id}', failed.dispose);
  }

  /// A picture of `width` by `height` arrived from participant [id] on [channel]: one
  /// larger than [fairPicture] allows keeps them out of the call, whatever the state of
  /// the connection.
  @visibleForTesting
  void pictureArrived(String id, int channel, int width, int height) {
    final peer = peers[id];
    if (_closed || peer == null || peer._keptOut) return;
    if (width <= 0 && height <= 0) return;
    if (!fairPicture(channel, width, height)) {
      unawaited(
        _keepOut(peer, 'video on channel $channel at ${width}x$height'),
      );
    }
  }

  /// Plays a participant's voice as loud as [gainOf] now says.
  Future<void> applyVolume(String id) async {
    final channels = await _channels(peers[id]?.connection);
    final track = channels.isEmpty ? null : channels[_audio].receiver.track;
    if (track != null) await _setGain(track, gainOf(id));
  }

  /// Plays the sound of a participant's shared screen as loud as [screenSoundGain] says.
  Future<void> applyScreenVolume(String id) async {
    final peer = peers[id];
    final channels = await _channels(peer?.connection);
    final apart = await peer?._soundIn?.getReceivers() ?? const [];
    final tracks = [
      if (channels.length > _screenSound) channels[_screenSound].receiver.track,
      for (final receiver in apart) receiver.track,
    ];
    for (final track in tracks.nonNulls) {
      await _setGain(track, screenSoundGain(id));
    }
  }

  /// Until the other side first sends its voice, WebRTC does not know the track yet; the
  /// gain is set again when it arrives.
  Future<void> _setGain(MediaStreamTrack track, double gain) async {
    try {
      await Helper.setVolume(gain, track);
    } catch (e) {
      _trace('volume: $e');
    }
  }

  // ----- Connection quality -----

  /// This device's own network as last told to the others.
  NetLevel? _told;

  bool _reading = false;
  bool _readingLevels = false;

  /// Reads the statistics of every connection now, as every 2 s, for tests.
  @visibleForTesting
  Future<void> measureNow() => _measure();

  /// Reads who talks now, as every 600 ms, for tests.
  @visibleForTesting
  Future<void> readLevelsNow() => _readLevels();

  // ----- Who is talking -----

  /// This device's own level while it is alone in the call.
  final _alone = AloneLevel();
}

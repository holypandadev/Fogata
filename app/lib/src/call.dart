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
part 'call/test_taps.dart';

/// Every connection carries four channels in this order, so turning the microphone,
/// camera or screen on and off only swaps tracks and never renegotiates.
const _audio = 0;
const _camera = 1;
const _screen = 2;

/// The sound of a shared screen, sent with it to those who watch it.
const _screenSound = 3;

/// The waits before the voice's encoding, not taken, is tried again
/// ([Call._startLater]); changed by tests.
@visibleForTesting
List<Duration> voiceStartAgainAfter = const [
  Duration(seconds: 1),
  Duration(seconds: 2),
  Duration(seconds: 4),
];

/// The time the call's waits and limits go by (not the time of day its log lines and
/// state numbers carry); tests put fake time in its place, with that of their timers.
@visibleForTesting
DateTime Function() callClock = DateTime.now;

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

/// A device id as the call's log shortens it; "nobody" for none.
String _short(String? id) =>
    id == null ? 'nobody' : id.substring(0, math.min(8, id.length));

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
    bool mic = true,
    this.camera = false,
    this.input,
    this.output,
    this.cameraId,
    this.noiseSuppression = false,
    this.gainOf = _unchanged,
    this.screenGainOf = _unchanged,
    this.screenQuality = const ScreenQuality(),
    this.nameOf = _someone,
  }) : _micOn = mic,
       quality = CallQuality(me: me);

  static double _unchanged(String id) => 1;
  static String _someone(String id) => l10n.appSomeone;

  /// A participant's name, for what the call tells about them.
  final String Function(String id) nameOf;

  /// How loud each participant's voice plays on this device: 1 as it arrives, 0 not at
  /// all, up to 2. Only this device hears the change.
  final double Function(String id) gainOf;

  /// The same for the sound of each participant's shared screen.
  final double Function(String id) screenGainOf;

  // ----- What each participant's channels of sound follow -----
  //
  // The step that brings a participant's voice and screen sound channels to what they
  // should be ([_voiceTo]) reads these, and each of them is changed only through its
  // setter, which has that step run again for every participant ([_soundChanged]):
  // test/call_mute_test.dart reads the sources for any other assignment. What is a
  // participant's own (their connection coming up, the screen they watch, their voice
  // taken or not) has its own setter in the same way ([_comeUp], [_setWatches],
  // [_setVoicePaused]), and every description agreed with them runs the step too.

  /// The microphone is on.
  bool get mic => _micOn;
  set mic(bool on) {
    if (on == _micOn) return;
    _micOn = on;
    _soundChanged();
  }

  bool _micOn;

  /// This device's shared screen.
  bool get sharing => _sharingNow;
  set sharing(bool on) {
    if (on == _sharingNow) return;
    _sharingNow = on;
    _soundChanged();
  }

  bool _sharingNow = false;

  /// A shared screen goes with the other applications' sound, where it can be captured.
  bool get shareSound => _shareSoundNow;
  set shareSound(bool on) {
    if (on == _shareSoundNow) return;
    _shareSoundNow = on;
    _soundChanged();
  }

  bool _shareSoundNow = true;

  /// The microphone's stream, open while it is on.
  MediaStream? get _micStream => _micStreamNow;
  set _micStream(MediaStream? stream) {
    if (stream == _micStreamNow) return;
    _micStreamNow = stream;
    // No connection follows a track that is not the live one any more.
    for (final peer in peers.values) {
      peer._voiceDone = null;
    }
    _soundChanged();
  }

  MediaStream? _micStreamNow;

  /// The shared screen's capture, with its sound where it has some.
  MediaStream? get _screenStream => _screenStreamNow;
  set _screenStream(MediaStream? stream) {
    if (stream == _screenStreamNow) return;
    _screenStreamNow = stream;
    _soundChanged();
  }

  MediaStream? _screenStreamNow;

  /// WebRTC records from the silent input, not the microphone.
  bool get _silent => _silentNow;
  set _silent(bool on) {
    if (on == _silentNow) return;
    _silentNow = on;
    _soundChanged();
  }

  bool _silentNow = false;

  /// A mute asked while the screen's sound goes out on Linux: the silent input is about
  /// to be chosen for it ([setMic]), until the mute's change of the microphone is done.
  bool get _silenceComing => _silenceComingNow;
  set _silenceComing(bool on) {
    if (on == _silenceComingNow) return;
    _silenceComingNow = on;
    _soundChanged();
  }

  bool _silenceComingNow = false;

  /// One shared capture is being replaced by another ([shareScreen]): the share goes on,
  /// and the screen's sound channel keeps its track and direction until the new capture
  /// is in place, so that the moment in between agrees nothing; its encoding still
  /// follows the microphone ([_voiceTo]).
  bool get _replacing => _replacingNow;
  set _replacing(bool on) {
    if (on == _replacingNow) return;
    _replacingNow = on;
    _soundChanged();
  }

  bool _replacingNow = false;

  /// One of the above changed: every participant's step runs again.
  void _soundChanged() {
    if (_closed) return;
    for (final peer in peers.values.toList()) {
      _queueVoice(peer);
    }
  }

  /// Their connection came up: WebRTC may record for it from now on.
  void _comeUp(CallPeer peer) {
    if (peer._upOnceNow) return;
    peer._upOnceNow = true;
    _queueVoice(peer);
  }

  /// The participant whose shared screen [peer] watches: this device's screen sound goes
  /// to them only while they watch it.
  void _setWatches(CallPeer peer, String? id) {
    if (id == peer._watchesNow) return;
    _trace(
      '${_short(peer.id)} watches ${_short(peer._watchesNow)} -> ${_short(id)}',
    );
    peer._watchesNow = id;
    _queueVoice(peer);
  }

  /// Takes [peer]'s voice or not ([_flushLateVoice]): set in their turn, and their step
  /// run there at once, so that no other step takes the change first. Returns what
  /// completes once a description made after the change is agreed ([CallPeer._agreed],
  /// made just before it: an agreement described before it never completes it), or
  /// null when the step started no agreement for it.
  Future<Completer<void>?> _setVoicePaused(CallPeer peer, bool paused) async {
    Completer<void>? agreed;
    await peer._inTurn(() async {
      final made = peer._agreed = Completer<void>();
      peer._voicePausedNow = paused;
      if (await _voiceTo(peer)) agreed = made;
    });
    return agreed;
  }

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
  bool camera;

  /// The participant whose shared screen this device watches. Until someone watches a
  /// screen, it is not sent to them, and its sound is not played here.
  String? get watching => _watching;
  set watching(String? id) => _watchingTo(id, 'set');

  /// Sets [watching], and with the call's log on says what changed it and why.
  void _watchingTo(String? id, String why) {
    if (id == _watching) return;
    final before = _watching;
    _trace('watching ${_short(before)} -> ${_short(id)}: $why');
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

  MediaStream? _cameraStream;

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

  /// This call's own id, named in what starts or agrees a connection (see `_theirCall` in
  /// signalling.dart): another call of this device, after it left the room and
  /// entered again or started again, has another.
  final String _callId = _newCallId();

  /// The number of the latest state this call told (see [_state]): the time it was told,
  /// in milliseconds, or one more than the one before if that is larger. A call made
  /// anew, after leaving the room or starting again, goes on from the clock, above the
  /// numbers of the call before it.
  int _stateNumber = 0;

  static String _newCallId() {
    final random = math.Random.secure();
    return [
      for (var i = 0; i < 8; i++)
        random.nextInt(256).toRadixString(16).padLeft(2, '0'),
    ].join();
  }

  /// How long a reading of a connection's statistics may take. WebRTC answers in a few
  /// milliseconds, but one asked of a connection as it is closed (someone leaving the
  /// room) may never answer; the readings each wait for the one before, so a single one
  /// that never answers would stop who-talks, the connection's quality and the checks of
  /// what others send for the rest of the call. Past it the reading counts as failed.
  static const statsWithin = Duration(seconds: 2);

  Future<List<StatsReport>> _stats(
    RTCPeerConnection connection, [
    MediaStreamTrack? track,
  ]) => connection
      .getStats(track)
      .timeout(
        statsWithin,
        onTimeout: () {
          _trace('statistics not read within ${statsWithin.inSeconds} s');
          throw TimeoutException('statistics', statsWithin);
        },
      );

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
    if (onMacOS) {
      // What an earlier call chose, or saw refused, is not this one's (flutter_webrtc's
      // FOGATA.md, "The devices chosen, with the microphone").
      try {
        await WebRTC.invokeMethod<void, dynamic>('fogataNewCall');
      } catch (e) {
        _trace('new call told: $e');
      }
    }
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

  /// Takes [peer]'s connection for one that came up, for tests.
  @visibleForTesting
  void comeUpForTests(CallPeer peer) {
    peer.connected = true;
    _comeUp(peer);
  }

  /// Opens the microphone as [start] does once the call's connection to the media bridge
  /// is there, for tests (start itself needs the core).
  @visibleForTesting
  Future<void> openMicForTests() => _openMic();

  /// The shared screen captured anew as [stream], as [setScreenQuality] does it, for
  /// tests.
  @visibleForTesting
  Future<void> swapCaptureForTests(MediaStream stream) {
    final old = _screenStream!;
    _screenStream = stream;
    return _swapCapture(old);
  }

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
    _soundServerAgain?.cancel();
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
      _sharesQuiet?.cancel();
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
  /// screen's sound only goes while that input is the silent one, or is being made so for
  /// a mute ([_silenceComing]): the sound goes on through the mute, not stopped and
  /// agreed again for the moment in between.
  MediaStreamTrack? get _soundTrack =>
      shareSound && (mic || _silent || _silenceComing)
      ? _screenStream?.getAudioTracks().firstOrNull
      : null;

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
      !_silenceComing &&
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
  /// opens the microphone on its own: its track, encoding and direction are given in each
  /// participant's turn with the voice's ([_voiceTo]).
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
    await _eachPeer('screen sound', (peer) async {
      _queueVoice(peer);
      await _soundTo(peer);
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

  TransceiverDirection get _soundDirection => _soundInChannel
      ? TransceiverDirection.SendRecv
      : TransceiverDirection.RecvOnly;

  /// The others watching this device's shared screen.
  int get watchers => peers.values.where((p) => p.watches == me).length;

  /// Those watching this device's shared screen whose connection is up, which the screen
  /// is sent on: the upload's budget is divided among them ([screenShareKbps]). One kept
  /// out (no connection while it stays) or making its connection again (5 to 30 s after
  /// it failed) still watches, but takes nothing of it meanwhile.
  int get _watchersSentTo => peers.values
      .where((p) => p.watches == me && p.connection != null && p.connected)
      .length;

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
      reports = await _stats(connection);
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
    final now = callClock();
    final shown = watching != null || focused != null || ownScreen;
    final screen = _watchBack;
    if (screen != null && screen.id == peer.id) {
      _watchBack = null;
      if (!shown && peer.sharing && now.isBefore(screen.until)) {
        _watchingTo(peer.id, 'its sharer came back sharing');
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

  void watch(String? id, {String why = 'chosen'}) {
    _watchBack = null;
    _focusBack = null;
    if (watching == id) return;
    _watchingTo(id, why);
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

  /// The microphone is open only while it is on, and WebRTC records while any send stream
  /// of sound runs, with a track or not: a voice channel's whose encoding is on, or the
  /// screen's sound channel's. That one sends only while the screen's sound goes out in
  /// the call's own channels ([_soundInChannel]): on Linux with the microphone off too,
  /// recording then from the silent input ([_quietInput]); on Windows only with the
  /// microphone on; on macOS never (its sound goes on connections of its own).
  ///
  /// A mute first disables the microphone's track, as it is asked: every sender holding
  /// it sends silence at once (WebRTC's AudioRtpSender mutes its stream as its track is
  /// disabled), whatever each connection is busy with. Then the microphone is closed or
  /// opened: changes one after the other, a change not started yet when a later one is
  /// asked being left to the later one. The future completes there, the state signal
  /// sent and listeners told. Each participant's voice channel is then brought to what
  /// [mic] asks in that participant's own turn ([_voiceTo]), apart from the others, so
  /// one whose turn is busy holds up neither them nor the caller.
  ///
  /// A mute waits for nothing: the state signal and every participant's step go out at
  /// once, before the microphone is closed, and the future completes then. An open still
  /// going on (the system asking first, say) finds the microphone off when it ends and
  /// lets go of what it opened ([_openMic]).
  Future<void> setMic(bool on) {
    mic = on;
    _micCheck.reset();
    _micAskedAt = callClock();
    _silenceComing = !on && _wantSilence;
    final asked = ++_micAsked;
    if (!on) {
      _silenceSenders();
      _micToPeers();
    } else if (_micStream != null) {
      // Still open (a mute's change not done, waiting for the silent input, say): on at
      // once, as a mute is, whatever that change is waiting for; the change of this
      // unmute still comes in its turn.
      _enableMic();
      _micToPeers();
      _micToldOn = asked;
    }
    final turn = _micTurn.then((_) => _micDevice(asked));
    _micTurn = turn.catchError((Object e) => _trace('microphone: $e'));
    return on ? _micTurn : Future.value();
  }

  /// Silences every voice sender at once, as a mute is asked, whatever each connection
  /// is busy with. On Linux and Windows the microphone's track is disabled: every sender
  /// holding it then sends silence (AudioRtpSender mutes its stream), and the engine
  /// only tells its audio processing (tools/webrtc/patches/webrtc/0008). On macOS that
  /// would have WebRTC-SDK's audio module mute its input (MuteStream calls
  /// SetMicrophoneMute, voice processing's own mute), a way the call never took: the
  /// track is taken off every sender instead, as a step does (replaceTrack(null), which
  /// needs nothing agreed). A step that gives a sender its track computes it and asks
  /// for it at once, so one asked before this was asked before this, and one after it
  /// finds the microphone off: none can put the track back.
  void _silenceSenders() {
    if (_closed) return;
    if (onMacOS) {
      for (final peer in peers.values.toList()) {
        unawaited(
          _resendTo(
            peer,
            _audio,
          ).catchError((Object e) => _trace('voice to ${peer.id}: $e')),
        );
      }
      return;
    }
    for (final track in _micStream?.getAudioTracks() ?? <MediaStreamTrack>[]) {
      try {
        track.enabled = false;
      } catch (e) {
        _trace('microphone track: $e');
      }
    }
  }

  /// Enables the microphone's track, disabled by a mute ([_silenceSenders]).
  void _enableMic() {
    for (final track in _micStream?.getAudioTracks() ?? <MediaStreamTrack>[]) {
      try {
        if (!track.enabled) track.enabled = true;
      } catch (e) {
        _trace('microphone track: $e');
      }
    }
  }

  /// Takes [peer]'s voice for a moment as late voice does, for tests ([_pauseVoiceIn]).
  @visibleForTesting
  Future<void> pauseVoiceInForTests(CallPeer peer) => _pauseVoiceIn(peer);

  /// When the microphone was last asked to change ([setMic]).
  DateTime _micAskedAt = DateTime(0);

  /// Tells everyone the microphone's state (unless [tell] says it was told already) and
  /// has every participant's channels brought to it.
  void _micToPeers({bool tell = true}) {
    if (_closed) return;
    if (tell) _sendState();
    notifyListeners();
    for (final peer in peers.values.toList()) {
      _queueVoice(peer);
    }
  }

  /// The changes of the microphone asked so far, and the last one's turn to open or close
  /// it ([setMic]).
  int _micAsked = 0;

  /// The unmute last told to everyone at once, before its turn ([setMic]).
  int _micToldOn = 0;
  Future<void> _micTurn = Future.value();

  /// Opens or closes the microphone for change [asked], unless a later change was asked
  /// before it started or the call ended; then has every participant's voice channel
  /// brought to it. A step that fails is noted and the next ones still go: whatever the
  /// device does, the connections follow [mic].
  Future<void> _micDevice(int asked) async {
    if (_closed || asked != _micAsked) return;
    Future<void> step(String what, Future<void> Function() run) async {
      if (_closed) return;
      try {
        await run();
      } catch (e) {
        _trace('microphone, $what: $e');
      }
    }

    if (mic) {
      if (_micStream == null) await step('opening', _openMic);
      // Disabled by a mute left to this change before it closed the microphone.
      await step('enabling', () async => _enableMic());
      await step('input', _loudInput);
      // Told at once already ([setMic]): the voice channels only, back on the microphone.
      _micToPeers(tell: _micToldOn != asked);
    } else {
      // To the silent input first, if the screen's sound keeps going out, so the
      // microphone is not held open by it.
      final silence = _silenceComing;
      await step('silent input', _quietInput);
      // An unmute asked meanwhile keeps the microphone open: its own change comes next.
      if (asked != _micAsked) return;
      _silenceComing = false;
      await step('closing', _closeMic);
      // The mute went out already ([setMic]); again only where the screen's sound waited
      // for the silent input, which may not have been made.
      if (silence) _micToPeers();
    }
  }

  /// Has [_voiceTo] run in [peer]'s turn, once however many ask before it starts: it reads
  /// everything again when it runs.
  void _queueVoice(CallPeer peer) {
    if (peer._voiceDue) return;
    peer._voiceDue = true;
    unawaited(
      peer
          ._inTurn(() async {
            peer._voiceDue = false;
            await _voiceTo(peer);
          })
          .catchError((Object e) => _trace('voice to ${peer.id}: $e')),
    );
  }

  /// The microphone's changes asked so far, and every participant's turn after them,
  /// done, for tests.
  @visibleForTesting
  Future<void> voiceSettledForTests() async {
    await _micTurn;
    await Future.wait([
      for (final peer in peers.values.toList())
        peer._inTurn(() async {}).catchError((Object _) {}),
    ]);
  }

  /// Brings [peer]'s two channels of sound, the voice and the shared screen's sound, to
  /// what [mic] and the share ask, from what is there now: run in [peer]'s turn after
  /// each change of the microphone or of the screen's sound, after each description
  /// agreed with them (offered or answered), and when their connection comes up. Each
  /// channel gets its track, its encoding on where it has something to send and stopped
  /// where it has not, and its direction ([_voiceWanted], [_soundDirection]), which is
  /// set, and agreed, only where it differs from the one this device last asked for
  /// ([_askedOn]). Returns whether a new agreement was started. A failure (the
  /// connection closed meanwhile) is noted, and whatever runs it goes on.
  Future<bool> _voiceTo(CallPeer peer) async {
    try {
      return await _voiceToNow(peer);
    } catch (e) {
      _trace('voice to ${peer.id}: $e');
      return false;
    }
  }

  Future<bool> _voiceToNow(CallPeer peer) async {
    final connection = peer.connection;
    // Read again after every wait: the call may end, they may leave, or their
    // connection be made anew meanwhile.
    bool gone() =>
        _closed || peers[peer.id] != peer || peer.connection != connection;
    if (connection == null || gone()) return false;
    // The change of the microphone this run brings the channel to (see [_voiceDone]).
    final asked = _micAsked;
    final transceivers = await _channels(connection);
    if (transceivers.isEmpty || gone()) return false;
    final track = _track(_audio, peer);
    await transceivers[_audio].sender.replaceTrack(track);
    // On with the microphone's track there to send; until it is (still opening), the
    // encoding stays as [_voiceWanted] leaves it, so nothing records for nothing.
    var voiceOn = false;
    // While the recording is started again ([_recoverVoice]) every voice encoding is
    // wanted off, and this step is the only one that sets them.
    if (mic && track != null && !_recovering) {
      voiceOn = await _encode(connection, _audio, true) == true;
      if (!voiceOn && !gone()) _startLater(peer, connection, _audio);
    }
    final voice = await _voiceWanted(peer, connection);
    if (transceivers.length > _screenSound && _replacing && !gone()) {
      // A capture being replaced: its track and direction stay as they are, so that
      // nothing is agreed for the change; its encoding follows the microphone as
      // outside it, on only while the screen's sound could go out (with the microphone
      // on, or through the silent input on Linux). While the recording is started again
      // it is off too, as the voice: WebRTC stops recording only with no sound sent.
      if (shareSound && (mic || _silent || _silenceComing) && !_recovering) {
        if (await _encode(connection, _screenSound, true) == false && !gone()) {
          _startLater(peer, connection, _screenSound);
        }
      } else if ((peer._upOnce || _recovering) &&
          await _agreedSends(connection, _screenSound)) {
        await _encode(connection, _screenSound, false);
      }
    }
    if (transceivers.length > _screenSound && !_replacing && !gone()) {
      await transceivers[_screenSound].sender.replaceTrack(
        _track(_screenSound, peer),
      );
      // Sending, the screen's sound goes with its encoding on. Not sending, a send
      // stream of it that is still agreed (until its new direction is) records nothing.
      // While the recording is started again it is off ([_recoverVoice]), its track and
      // direction as they are, and on again after it.
      if (_soundInChannel && !_recovering) {
        if (await _encode(connection, _screenSound, true) == false && !gone()) {
          _startLater(peer, connection, _screenSound);
        }
      } else if ((peer._upOnce || _recovering) &&
          await _agreedSends(connection, _screenSound)) {
        await _encode(connection, _screenSound, false);
      }
    }
    if (gone()) return false;
    var agree = false;
    if (_askedOn(connection, _audio) != voice) {
      await _ask(connection, transceivers, _audio, voice);
      agree = true;
    }
    if (transceivers.length > _screenSound &&
        !_replacing &&
        _askedOn(connection, _screenSound) != _soundDirection) {
      await _ask(connection, transceivers, _screenSound, _soundDirection);
      agree = true;
    }
    if (gone()) return false;
    if (agree) await _renegotiate(peer);
    // Followed: with the microphone on, its encoding on with the live track (see
    // [Call._measure]); not one left by a mute and unmute meanwhile.
    if (!gone() &&
        asked == _micAsked &&
        (!mic || (voiceOn && track == _track(_audio, peer)))) {
      peer._voiceDone = asked;
    }
    return agree;
  }

  /// The direction this device last asked for on each channel of a connection, by the
  /// channel's place, kept with the connection itself so that it goes with it: set when
  /// the connection's offer makes the channels, and through [_ask], the only place that
  /// sets a channel's direction (test/call_mute_test.dart reads the sources for any
  /// other). The plugin tells a channel's direction only on macOS (Linux's and Windows'
  /// have no `rtpTransceiverGetDirection`).
  final _asked = Expando<Map<int, TransceiverDirection>>();

  TransceiverDirection? _askedOn(RTCPeerConnection connection, int channel) =>
      _asked[connection]?[channel];

  /// Asks for [direction] on [channel] of [connection] ([transceivers], its channels in
  /// order), and keeps it ([_askedOn]).
  Future<void> _ask(
    RTCPeerConnection connection,
    List<RTCRtpTransceiver> transceivers,
    int channel,
    TransceiverDirection direction,
  ) async {
    await transceivers[channel].setDirection(direction);
    (_asked[connection] ??= {})[channel] = direction;
  }

  /// Takes [direction] as asked already on [channel] of [connection], for tests of a
  /// connection made by them.
  @visibleForTesting
  void askedForTests(
    RTCPeerConnection connection,
    int channel,
    TransceiverDirection direction,
  ) => (_asked[connection] ??= {})[channel] = direction;

  /// The direction [peer]'s voice channel on [connection] is to be asked for. It sends
  /// while the microphone is on. With the microphone off, where the channel is agreed to
  /// send on a connection that has been up, whatever its state now, the encoding of its
  /// send stream is stopped first: WebRTC stops the stream, and with the last one it
  /// stops recording, the same stop as for a channel that no longer sends (the one its
  /// echo canceller keeps the room through, tools/webrtc/patches/webrtc/0010), and an
  /// unmute only turns it on again. It then keeps sending where its encoding is stopped.
  /// Anywhere else it only receives, and its encoding is left as it is: WebRTC prepares
  /// the microphone whenever a channel comes to send (SetSend false to true, which on
  /// macOS opens it), and lets go of that only once a stream starts and stops; a stream
  /// made with its encoding on starts, and the step after the agreement stops it, while
  /// one made off would leave the microphone prepared.
  Future<TransceiverDirection> _voiceWanted(
    CallPeer peer,
    RTCPeerConnection connection,
  ) async {
    var stopped = false;
    if (peer.connection == connection &&
        (mic && _recovering ||
            !mic && peer._upOnce && await _sendsVoice(connection))) {
      // While the recording is started again ([_recoverVoice]), off on every connection,
      // up or not: one WebRTC finds up before the call does runs its stream already.
      stopped = await _encode(connection, _audio, false) == true;
    }
    // [mic] as it is after the wait: turned on meanwhile, the channel sends.
    final sends = mic || stopped;
    return peer._voicePaused
        ? (sends
              ? TransceiverDirection.SendOnly
              : TransceiverDirection.Inactive)
        : (sends
              ? TransceiverDirection.SendRecv
              : TransceiverDirection.RecvOnly);
  }

  /// The encoding of [channel] (the voice, or the screen's sound) was not taken: tried
  /// again 1, 3 and 7 s later, in [peer]'s turn, while it still has something to send
  /// and [connection] is still theirs. The voice's still not taken, the connection is
  /// made anew ([_connectAnew]); the screen's sound's is left as it is (traced). Once for
  /// a channel of a connection at a time.
  void _startLater(CallPeer peer, RTCPeerConnection connection, int channel) {
    if (peer._starting[channel] == connection) return;
    peer._starting[channel] = connection;
    final what = channel == _audio ? 'voice' : 'screen sound';
    _trace('$what to ${peer.id.substring(0, 8)} not started: trying again');
    bool wanted() => channel == _audio
        ? mic && !_recovering && _track(_audio, peer) != null
        : !_recovering &&
              (_soundInChannel ||
                  (_replacing &&
                      shareSound &&
                      (mic || _silent || _silenceComing)));
    unawaited(() async {
      try {
        for (final wait in voiceStartAgainAfter) {
          await Future<void>.delayed(wait);
          var done = true;
          await peer._inTurn(() async {
            if (_closed ||
                !wanted() ||
                peers[peer.id] != peer ||
                peer.connection != connection) {
              return;
            }
            done = await _encode(connection, channel, true) != false;
            _trace(
              '$what to ${peer.id.substring(0, 8)} '
              '${done ? 'started' : 'not started'} after '
              '${wait.inMilliseconds} ms more',
            );
            // Started: the step marks the voice followed ([_voiceDone]).
            if (done) _queueVoice(peer);
          });
          if (done) return;
        }
        if (channel == _audio) {
          await peer._inTurn(() => _connectAnew(peer, connection));
        } else {
          _trace(
            '$what to ${peer.id.substring(0, 8)} not started: left as it is',
          );
        }
      } catch (e) {
        _trace('$what to ${peer.id}: $e');
      } finally {
        if (peer._starting[channel] == connection) {
          peer._starting.remove(channel);
        }
      }
    }());
  }

  /// Makes [peer]'s connection anew, the voice's encoding not taken, while the microphone
  /// is on, [connection] still theirs and the call going on. Where this side offers, it
  /// offers a new connection at once, as after a failure. Where the other side offers,
  /// this one is closed and the other side asked for a new offer ([_askAgain]).
  Future<void> _connectAnew(CallPeer peer, RTCPeerConnection connection) async {
    if (_closed ||
        !mic ||
        peers[peer.id] != peer ||
        peer._keptOut ||
        peer.connection != connection) {
      return;
    }
    // A few times in a call: an encoding refused on every new connection would have it
    // made again and again, with nothing between the two meanwhile. Left as it is then,
    // with only this line in the log: nobody is told, and in a call of two the
    // microphone check may say the microphone reads nothing ([_measure]), which is not
    // what happened.
    if (peer._voiceAnew >= maxVoiceAnew) {
      _trace('voice to ${peer.id.substring(0, 8)} not started: left as it is');
      return;
    }
    peer._voiceAnew++;
    _trace('voice to ${peer.id.substring(0, 8)} not started: connecting anew');
    if (me.compareTo(peer.id) < 0) {
      // Counted as opening: the voice engine waits for it if the call ends meanwhile.
      await _whileOpening(() async {
        if (!_closed) await _offer(peer);
      });
      return;
    }
    await peer._close();
    unawaited(_askAgain(peer));
  }

  /// Asks [peer], who offers, for a new offer (`again`) until one comes, this side's
  /// connection with them closed: they take it only once their own connection is no
  /// longer up and was made more than [connectWithin] ago (the `again` case of
  /// [_handle]). How soon WebRTC on their side finds a connection whose other end
  /// closed no longer up, the code here does not decide; at the latest it fails, 30 s
  /// with nothing received (kDeadConnectionReceiveTimeout), and they offer anew 2 to 6 s
  /// after that by themselves ([_retry]). Asked every 2 s over the first 10 s, then
  /// every 5 s up to 30 s, then once more by [_retry]. Outside their turn: the offer is
  /// taken in it.
  Future<void> _askAgain(CallPeer peer) async {
    for (final wait in _againEvery) {
      await Future<void>.delayed(wait);
      if (_closed ||
          peers[peer.id] != peer ||
          peer._keptOut ||
          peer.connection != null) {
        return;
      }
      _signal(peer.id, {'t': 'again'});
    }
    // Still no offer: tried again as any connection that failed.
    if (!_closed && peers[peer.id] == peer && peer.connection == null) {
      _retry(peer);
    }
  }

  /// How many times in a call a participant's connection is made anew for a voice
  /// encoding not taken ([_connectAnew]).
  static const maxVoiceAnew = 3;

  static const _againEvery = [
    Duration(seconds: 1),
    Duration(seconds: 2),
    Duration(seconds: 2),
    Duration(seconds: 2),
    Duration(seconds: 2),
    Duration(seconds: 5),
    Duration(seconds: 5),
    Duration(seconds: 5),
    Duration(seconds: 5),
  ];

  /// Starts or stops the encoding of [channel] (the voice or the screen's sound) on
  /// [connection] ([RTCRtpEncoding.active]) with nothing to agree. Returns whether the
  /// encoding is so, as read back from the sender afterwards: macOS's plugin answers yes
  /// whatever WebRTC did; null when the channel cannot be read at all (the connection
  /// closed). With no send stream, WebRTC's sender (pc/rtp_sender.cc) answers the
  /// parameters it keeps for the next one: one encoding until its first stream, none
  /// after it; one set then is applied to the next stream (ApplyInitParameters_w), so a
  /// stream can be made with its encoding off, and [_voiceTo], run once it is agreed,
  /// turns it on. A channel with none has nothing to set, and nothing records.
  Future<bool?> _encode(
    RTCPeerConnection connection,
    int channel,
    bool on,
  ) async {
    final what =
        '${channel == _audio ? 'voice' : 'screen sound'} '
        '${on ? 'on' : 'off'}';
    try {
      Future<RTCRtpSender?> sender() async {
        final channels = await _channels(connection);
        return channels.length > channel ? channels[channel].sender : null;
      }

      bool so(RTCRtpSender s) =>
          (s.parameters.encodings ?? const <RTCRtpEncoding>[]).every(
            (e) => e.active == on,
          );

      final RTCRtpSender? before;
      try {
        before = await sender();
      } catch (e) {
        _trace('$what: $e');
        return null;
      }
      if (before == null) return null;
      if (so(before)) return true;
      final parameters = before.parameters;
      // Only what is set here goes back (see [_tune]): the encoding's other values as
      // read (a bitrate of 0, say) would change it, or be refused.
      parameters.encodings = [
        RTCRtpEncoding(
          active: on,
          numTemporalLayers: null,
          scaleResolutionDownBy: null,
          priority: parameters.encodings!.first.priority,
        ),
      ];
      parameters.degradationPreference = null;
      if (!await before.setParameters(parameters)) {
        _trace('$what: refused');
        return false;
      }
      final after = await sender();
      if (after != null && so(after)) return true;
      _trace('$what: not taken');
      return false;
    } catch (e) {
      _trace('$what: $e');
      return false;
    }
  }

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
  /// Chosen in the input turn ([_inputTurn]), so never in the middle of choosing the
  /// silent input.
  /// A choice made while an earlier change of input is still going on (the sound server
  /// slow to answer, say) waits for it; of the choices made meanwhile only the last is
  /// carried out, with what is chosen when it runs.
  Future<void> useInput(String? id) async {
    input = id;
    final asked = ++_inputAsked;
    // Another input is judged from the start.
    _micCheck.reset();
    await _inputTurn(() async {
      // Taken when the silent input is left; or left to a later choice.
      if (_silent || asked != _inputAsked) return;
      await _select(
        input ?? await _first('audioinput'),
        Helper.selectAudioInput,
      );
    });
  }

  /// The choices of input made so far ([useInput]).
  int _inputAsked = 0;

  /// Changes the speakers or headphones; null is the system's default.
  Future<void> useOutput(String? id) async {
    output = id;
    await _select(id ?? await _first('audiooutput'), Helper.selectAudioOutput);
    // On macOS the voice engine is made again for it, recording nothing for a moment
    // (flutter_webrtc's FOGATA.md, "The devices chosen, with the microphone"): judged
    // from the start again, from when it has been chosen.
    if (onMacOS) _micCheck.reset();
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
        // Another capture in place of the one shared: not a share that stops.
        _replacing = sharing;
        try {
          await _shareScreen(source);
        } finally {
          _replacing = false;
        }
      });

  Future<void> _shareScreen(DesktopCapturerSource? source) async {
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
    if (sharing) await _askSharedEncode();
    await _resetFits();
    await _resend(_screen);
    await _soundChannel();
    if (sharing) onSound?.call(UiSound.shareStart);
  }

  /// Asks the plugin, once, whether the connections that send the shared screen share
  /// one encoder of it (`fogataSharedScreenEncode`: libwebrtc's shared screen encode, on
  /// unless `FOGATA_SHARED_SCREEN_ENCODE=off`): the screen's load then no longer grows
  /// with each watcher ([ScreenLoad.sharedEncode]). No answer counts as no.
  Future<void> _askSharedEncode() async {
    if (_sharedEncodeAsked) return;
    _sharedEncodeAsked = true;
    try {
      final answer = await WebRTC.invokeMethod<bool, dynamic>(
        'fogataSharedScreenEncode',
      );
      _screenLoad.sharedEncode = answer == true;
    } catch (_) {
      _screenLoad.sharedEncode = false;
    }
  }

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
    // The bitrate the connections are told below, so that the next reading does not take
    // it for a change.
    _screenKbps = _screenKbpsAt(_screenScale);
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

  /// The calls of each participant that left the room while this call went on, as their
  /// signals named them: the one they had then and those that ended before it. Someone who
  /// comes back with another call has that one ended, and a late signal of it (an offer, an
  /// answer, an asking) is dropped rather than taken for a newer call that ends theirs; of
  /// those, the ones taken for ended on a guess ([CallPeer._guessedEnded]). The last
  /// [leftKept] to leave; a new call of this device starts with none.
  final Map<String, ({String? call, List<String> ended, Set<String> guessed})>
  _left = {};
  static const leftKept = 64;

  /// The number of the last offer made for each participant that left (the last
  /// [leftKept]): the offers made for them when they are back go on above it, so that a
  /// late copy of one before is told apart by its number (see 'offer' in [_handle]).
  final Map<String, int> _offersTo = {};

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
    await _letGoOf(left);
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
      // Its offers numbered above every one made for them before in this call, whatever
      // the clock did meanwhile.
      peer._offerNumber = math.max(
        (_offersTo[id] ?? 0) + 1,
        DateTime.now().millisecondsSinceEpoch,
      );
      if (_left.remove(id) case (:final call, :final ended, :final guessed)) {
        peer._callWhenLeft = call;
        peer._endedCalls.addAll(ended);
        peer._guessedEnded.addAll(guessed);
      }
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

  /// Participants whose link with this device is down, as the core said ([memberGone]):
  /// signals to them go nowhere until it is back.
  final Set<String> _unreachable = {};

  /// The core lost its link with [id] (a network change, a short outage). A connection
  /// with it that is not up, and any made until the link is back, cannot be agreed: the
  /// signals of either side are dropped on the way. Each is tried anew as soon as the link
  /// is back ([memberBack]), rather than seconds later, after its tries ran out or it was
  /// given up on as one that never came up ([connectWithin]).
  void memberGone(String id) {
    if (_closed) return;
    _unreachable.add(id);
    final peer = peers[id];
    if (peer != null && !peer.connected) peer._blind = true;
  }

  /// The core has a link with [id] again (or tells a new way to it): a connection with it
  /// that is not up after it could not be reached is tried again now, as with one that
  /// failed: the side with the smaller id offers, the other asks for an offer. Once: a
  /// connection made since then is no longer blind, so the core telling the same again,
  /// or the other side's asking crossing this side's offer, makes no second one (see
  /// 'again' in [_handle]). A whole network coming back tries each participant once.
  void memberBack(String id) {
    final wasGone = _unreachable.remove(id);
    final peer = peers[id];
    if (_closed ||
        _bridge == null ||
        peer == null ||
        peer.connected ||
        peer._keptOut ||
        peer.voiceless ||
        // Told of a way to it while nothing was lost (a better path), or tried already.
        !(wasGone || peer._blind)) {
      return;
    }
    _trace('${id.substring(0, 8)} reachable again; trying again');
    peer._waits = 0;
    _watch(peer);
    if (me.compareTo(id) > 0) {
      peer._blind = false;
      _signal(id, {'t': 'again'});
      return;
    }
    // In its turn, after what came before it: the other side's asking may have made a
    // connection meanwhile ('again' in [_handle]), and so may another word of the link;
    // one made since the link came back is not blind, and is left to come up.
    unawaited(
      peer
          ._inTurn(() async {
            if (_closed || peers[id] != peer || peer.connected) return;
            if (!peer._blind && peer.connection != null) return;
            await _offer(peer);
          })
          .catchError((Object e) {
            _trace('offer to ${id.substring(0, 8)}: $e');
          }),
    );
  }

  /// [peer]'s offer or answer was just sent: one sent while the link to it is down went
  /// nowhere, and its connection will not come up ([memberBack] tries again).
  void _sentWhileGone(CallPeer peer) {
    peer._blind = _unreachable.contains(peer.id);
  }

  /// Lets go of participants that left: all together, so that several leaving at once hold
  /// the call up for one at most; known to [stop] until they are done, so the voice engine
  /// waits for them.
  Future<void> _letGoOf(List<CallPeer> left) async {
    // Before letting go, as they are taken out: they may be in the room again before that
    // is done, and their calls must be known then.
    for (final peer in left) {
      _left.remove(peer.id);
      // One that came back and left again before any signal named its call still had
      // the call it left with first; the ended calls it was given are kept with it.
      _left[peer.id] = (
        call: peer._theirCall ?? peer._callWhenLeft,
        ended: [...peer._endedCalls],
        guessed: {...peer._guessedEnded},
      );
      if (_left.length > leftKept) _left.remove(_left.keys.first);
      _offersTo.remove(peer.id);
      _offersTo[peer.id] = peer._offerNumber;
      if (_offersTo.length > leftKept) _offersTo.remove(_offersTo.keys.first);
    }
    // And what showed them: once they are let go of, they may be back and shown again.
    for (final gone in left.map((peer) => peer.id)) {
      _showScreenOut(gone, null);
      final until = callClock().add(_backWithin);
      if (watching == gone) {
        _watchingTo(null, 'its sharer left');
        _watchBack = (id: gone, until: until);
      }
      if (focused == gone) {
        focused = null;
        _focusBack = (id: gone, until: until);
      }
    }
    await Future.wait([for (final peer in left) _whileLeaving(peer.dispose())]);
  }

  /// The room says [id] left, by its own event: its connection goes now, before the
  /// participants are read anew ([setParticipants]). Someone who leaves and enters again
  /// at once is named in every list read meanwhile, and its connection with the call that
  /// ended there would be kept, still counted up until WebRTC finds it dead, with nobody
  /// offering a new one. Its signals waiting here go too: they were for that call. The
  /// next list that names it makes its connection anew, as for anyone who comes in.
  Future<void> participantLeft(String id) async {
    if (_closed || id == me) return;
    _early.forget(id);
    final peer = peers.remove(id);
    if (peer == null) return;
    _trace('${id.substring(0, 8)} left');
    await _letGoOf([peer]);
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

  @visibleForTesting
  MicCheck get micCheckForTests => _micCheck;

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

  /// The output and the microphone chosen, as the voice engine uses them (macOS), told when
  /// it uses others, and its recording lost.
  final _outputWatch = OutputWatch();
  final _inputWatch = InputWatch();
  final _recordingWatch = RecordingWatch();

  /// Hands what to say of an output or microphone chosen that the voice engine does not use
  /// (macOS) to whoever shows the call; without one, the call says it itself. The choice
  /// stays as the person made it.
  void Function(String text)? onOutputRefused;

  /// The output and the microphone the voice engine uses when they are not those chosen
  /// (their ids, 'default' for the system's; macOS), to show beside the choice; null when
  /// they are, or it is not known.
  String? outputInUse;
  String? inputInUse;

  /// Whether some connection agreed to send voice, at the last reading of the statistics.
  bool _voiceSending = false;

  /// macOS, with an output or a microphone chosen: reads the voice engine's facts
  /// (flutter_webrtc's FOGATA.md, "The devices chosen, with the microphone") and tells
  /// what they show ([OutputWatch], [InputWatch], [RecordingWatch]); read with the
  /// statistics, every 2 s. With nothing chosen nothing is asked, as before devices were
  /// given to the engine by the plugin.
  Future<void> _checkOutput() async {
    if (!onMacOS || _closed || _checkingOutput) return;
    if (output == null && input == null) {
      // The system's devices chosen again: nothing in use beside a choice any more, and
      // nothing seen of the recording with the devices chosen before holds for the next.
      _recordingWatch.reset();
      if (outputInUse != null || inputInUse != null) {
        outputInUse = null;
        inputInUse = null;
        notifyListeners();
      }
      return;
    }
    _checkingOutput = true;
    try {
      final facts = await WebRTC.invokeMethod<Object?, dynamic>(
        'fogataAudioOutput',
      );
      if (_closed || facts is! Map) return;
      final out = _outputWatch.take(facts, const [], asked: output);
      final inp = _inputWatch.take(facts, const [], asked: input);
      if (out.inUse != outputInUse || inp.inUse != inputInUse) {
        outputInUse = out.inUse;
        inputInUse = inp.inUse;
        notifyListeners();
      }
      for (final text in [out.text, inp.text].whereType<String>()) {
        _trace('devices chosen: $text');
        if (onOutputRefused case final tell?) {
          tell(text);
        } else {
          _report(text);
        }
      }
      if (_recordingWatch.take(
        facts,
        mic: mic,
        sending: _voiceSending,
        listened: peers.values.any((p) => p.connected),
      )) {
        await _recoverVoice();
      }
    } catch (e) {
      _trace('output: $e');
    } finally {
      _checkingOutput = false;
    }
  }

  bool _checkingOutput = false;

  /// The voice engine lost the recording the call wants ([RecordingWatch]): every voice
  /// encoding, and the screen's sound's where it is sent, on every connection up or not,
  /// is to be off, then on again, so that WebRTC stops recording (its last sending
  /// stream gone) and starts it afresh its own way: AudioState asks the audio module to
  /// record and restarts its echo canceller, as at any unmute. Each participant's own step
  /// ([_voiceTo]) is the only one that sets their encoding: while [_recovering] it wants
  /// it off (a step of any origin meanwhile, a participant who arrives, does the same), and
  /// once no voice encoding is on anywhere (3 s at most) it is cleared and every step runs
  /// again, turning them on with their own retries. Not through [setMic]: the others are
  /// told nothing and the microphone's check is not started over. Given up as soon as the
  /// person turns the microphone off or the call ends. Once for each loss.
  Future<void> _recoverVoice() async {
    final asked = _micAsked;
    bool on() => !_closed && mic && _micAsked == asked;
    if (!on() || _recovering) return;
    _trace(
      'recording lost with the microphone on: every voice encoding off, then on',
    );
    voiceRecoveries++;
    _recovering = true;
    try {
      for (final peer in peers.values.toList()) {
        _queueVoice(peer);
      }
      final end = callClock().add(const Duration(seconds: 3));
      while (on()) {
        final still = await _voiceOn();
        if (still.isEmpty) break;
        // A participant who arrived meanwhile, or whose step came before: their step
        // again (once at a time: [_queueVoice]).
        still.forEach(_queueVoice);
        if (callClock().isAfter(end)) {
          _trace('recording restart: a voice encoding still on after 3 s');
          break;
        }
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      if (!on()) {
        _trace(
          'recording restart given up: the microphone was turned off, or the call ended',
        );
      }
    } finally {
      _recovering = false;
      for (final peer in peers.values.toList()) {
        _queueVoice(peer);
      }
    }
  }

  /// While the recording is started again ([_recoverVoice]).
  bool _recovering = false;

  /// How many times the recording was started again, for tests.
  @visibleForTesting
  int voiceRecoveries = 0;

  /// The participants with an encoding of sound on, as their sender reads back: the
  /// voice's, or the screen's sound's where it is agreed to send. Every connection, up or
  /// not: one WebRTC finds up before the call does runs its streams already.
  Future<List<CallPeer>> _voiceOn() async {
    final on = <CallPeer>[];
    bool active(RTCRtpSender sender) =>
        (sender.parameters.encodings ?? const <RTCRtpEncoding>[]).any(
          (e) => e.active,
        );
    for (final peer in peers.values.toList()) {
      final connection = peer.connection;
      if (connection == null) continue;
      try {
        final channels = await _channels(connection);
        if (channels.length <= _audio) continue;
        if (active(channels[_audio].sender) ||
            (channels.length > _screenSound &&
                active(channels[_screenSound].sender) &&
                await _agreedSends(connection, _screenSound))) {
          on.add(peer);
        }
      } catch (_) {
        // A connection closed meanwhile sends nothing.
      }
    }
    return on;
  }

  /// Runs [_recoverVoice], for tests.
  @visibleForTesting
  Future<void> recoverVoiceForTests() => _recoverVoice();

  /// The sound server going away and coming back (Linux), told once each way.
  final _soundServer = SoundServerWatch();

  /// Reads the list again once the sound server has been gone long enough to tell.
  Timer? _soundServerAgain;

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
  /// the sound server or the microphone chosen went away or came back, and makes the
  /// silent input again if it went (PipeWire restarted).
  @visibleForTesting
  void takeDevices(List<MediaDeviceInfo> devices) {
    final wasLost = _soundServer.lost;
    final sound = _soundServer.take(devices, linux: onLinux, now: callClock());
    if (sound != null) {
      _trace('sound server, said: $sound');
      onNotice?.call(sound);
    }
    if (_soundServer.lost) {
      // Nothing is listed, so nothing to say of the microphone; the list is read again
      // when the server is back (the voice engine tells), or to tell it is still gone.
      if (!wasLost) {
        _trace('sound server went away');
        _micCheck.reset();
        _soundServerAgain?.cancel();
        _soundServerAgain = Timer(
          SoundServerWatch.tellAfter,
          () => unawaited(_readDevices()),
        );
      }
      return;
    }
    if (wasLost) {
      _trace('sound server back');
      _soundServerAgain?.cancel();
      // Judged from the start again: nothing was recorded meanwhile.
      _micCheck.reset();
    }
    final notice = _micWatch.take(input, devices);
    if (notice != null) {
      _trace('microphone: $notice');
      onNotice?.call(notice);
    }
    // The silent input in use went away with PipeWire: WebRTC records from the default
    // microphone until it is made again and chosen.
    final silenceListed = devices.any(
      (d) => d.kind == 'audioinput' && d.label.contains(_ownSilence),
    );
    if (_silent && !silenceListed) {
      _trace('silent input gone: made again');
      unawaited(_quietInput());
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
    // Whatever comes from them came over a link that works: their outage is over, whether
    // or not the core has said so yet (see [memberBack]).
    _unreachable.remove(from);
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
      // A late copy of an earlier offer of their call (an offer goes again as a copy)
      // that comes while a later one waits: replacing that one, it would be answered in
      // its place, and the later one, which their side waits on, not at all. Their
      // numbers only rise when their signals say they answer a copy (`w`).
      final pending = peer._offerPending;
      final number = message['n'];
      final before = pending?['n'];
      if (pending != null &&
          message['w'] == 1 &&
          pending['w'] == 1 &&
          message['c'] is String &&
          message['c'] == pending['c'] &&
          number is int &&
          before is int &&
          number < before) {
        _trace(
          'offer $number from ${from.substring(0, 8)} older than $before waiting: '
          'dropped',
        );
        return;
      }
      final mine = ++peer._offersIn;
      peer._offerPending = message;
      // Waiting its turn, or being answered, until its step is over: the side that asks
      // for offers does not ask meanwhile ([_watch]).
      peer._offersWaiting++;
      try {
        await _offerTurn(peer, peer._offerTaken);
        if (mine != peer._offersIn) return;
        peer._offerTaken = callClock();
        await peer._inTurn(() async {
          if (mine != peer._offersIn) return;
          peer._offerPending = null;
          await _handleSafely(peer, from, message);
        });
      } finally {
        peer._offersWaiting--;
        if (mine == peer._offersIn) peer._offerPending = null;
      }
    } else if (message['t'] == 'sound') {
      final mine = ++peer._soundOffersIn;
      await _offerTurn(peer, peer._soundOfferTaken);
      if (mine != peer._soundOffersIn) return;
      peer._soundOfferTaken = callClock();
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
    peer._madeAt = callClock();
    // Tests only (FOGATA_TEST_TAP_DIR): its received sound's statistics, every 100 ms.
    _tapStats(
      connection,
      peer.id,
      () => !_closed && peer.connection == connection,
    );
    // Nothing of it sent yet (see [_sentWhileGone]).
    peer._blind = false;
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
      if (up) _comeUp(peer);
      if (up && !peer.connected) {
        _signal(peer.id, _state);
        _signal(peer.id, _net);
        // Its voice channel as the microphone asks, now that WebRTC may record for it.
        _queueVoice(peer);
      }
      if (up) {
        wasUp = true;
        peer._attempt = 0;
      }
      final went = peer.connected != up;
      peer.connected = up;
      // A watcher's connection came up or went: the others' share of the upload follows.
      if (went && peer.watches == me) _sharesChanged();
      if (state == RTCPeerConnectionState.RTCPeerConnectionStateFailed) {
        _retry(peer);
      }
      notifyListeners();
    };
    // A connection that never comes up is tried again as one that failed: one whose
    // descriptions do not match (offers that crossed, a signal lost on the way) may take
    // minutes to give up by itself.
    // One made before is no longer [connection]: its own wait would do nothing.
    // Counted from the last step of its negotiation ([_timeLeft]), not from its making:
    // on a loaded computer an offer took seconds to make, its answer tens of seconds to
    // come, and each try given up on just before its answer came began the next.
    void notUp() {
      if (_closed ||
          peers[peer.id] != peer ||
          peer.connection != connection ||
          wasUp ||
          failed) {
        return;
      }
      final left = _timeLeft(peer);
      if (left > Duration.zero) {
        peer._connectWait = Timer(left, notUp);
        return;
      }
      _trace('connection ${peer.id.substring(0, 8)}: not up in time');
      _retry(peer);
    }

    peer._checkUp = notUp;
    peer._connectWait?.cancel();
    peer._connectWait = Timer(connectWithin, notUp);
    return connection;
  }

  /// How long a new connection has to come up before it is tried again.
  static const connectWithin = Duration(seconds: 15);

  /// How long this side's first offer waits for its answer, with copies of it going
  /// meanwhile ([_watch]): the answers of a loaded computer took up to 35 s.
  static const answerWithin = Duration(seconds: 45);

  /// An offer waiting for its answer goes again as a copy when they ask for one, this
  /// often at most.
  static const copyEvery = Duration(seconds: 2);

  /// The time [peer]'s connection still has to be agreed and come up, from the last step
  /// of its negotiation: [connectWithin] once it has both descriptions; before, while
  /// this side's offer waits for its answer, [answerWithin] from that offer; else
  /// [connectWithin] from its making. In all, its making, then [answerWithin] and
  /// [connectWithin] at most: nothing the other side sends lengthens it.
  Duration _timeLeft(CallPeer peer) {
    final described = peer._describedAt;
    final offered = peer._offeredAt;
    final made = peer._madeAt;
    final (since, within) = described != null
        ? (described, connectWithin)
        : offered != null
        ? (offered, answerWithin)
        : (made, connectWithin);
    if (since == null) return Duration.zero;
    return within - callClock().difference(since);
  }

  /// A `which` with no answer is asked again this often, as many times in all as
  /// [whichTries] ([_askWhich]): an answer may come in the [connectWithin] after the first.
  static const whichEvery = Duration(seconds: 3);
  static const whichTries = 5;

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

  /// The most the shared screen may use at that scale, in kbit/s, to each of those it is
  /// sent to, as the connections were last told when its size, its rate or that number
  /// changed (see [screenKbps] and [screenShareKbps]): a new size the scale keeps (a
  /// window made larger, within its quality) still changes it.
  int _screenKbps = 0;

  /// While the bitrate each watcher may use keeps quiet after it was told (see
  /// [_sharesChanged]), and whether a change came meanwhile.
  Timer? _sharesQuiet;
  bool _sharesOwed = false;

  /// How often at most the bitrate each watcher may use follows a change of their number.
  static const sharesEvery = Duration(seconds: 1);

  /// How far the shared screen is lowered for everyone, for this device's processor.
  final _screenLoad = ScreenLoad();

  /// Whether the plugin was asked if the screen's watchers share one encoder of it
  /// ([_askSharedEncode]).
  bool _sharedEncodeAsked = false;

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
      if (mid == _screen && watching == peer.id) {
        watch(null, why: 'its picture failed too often');
      }
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

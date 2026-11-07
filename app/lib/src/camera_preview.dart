import 'package:flutter/widgets.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'call.dart' show openCameraStream;

/// A camera's picture being shown, and how to stop it.
abstract class CameraFeed {
  /// The picture, drawn as in a mirror when [mirror] (the person's choice in the
  /// settings, as the room draws this device's own camera).
  Widget view({required bool mirror});

  Future<void> stop();
}

/// Opens a camera (the system's default when [id] is null) for a preview.
typedef OpenCamera = Future<CameraFeed> Function(String? id);

/// The camera as a call opens it.
Future<CameraFeed> openWebrtcCamera(String? id) async {
  final stream = await openCameraStream(id);
  final renderer = RTCVideoRenderer();
  try {
    await renderer.initialize();
    renderer.srcObject = stream;
  } catch (_) {
    await _WebrtcFeed._free(stream);
    // A texture made before the failure goes too; the first failure is the one told.
    try {
      await renderer.dispose();
    } catch (_) {}
    rethrow;
  }
  return _WebrtcFeed(stream, renderer);
}

class _WebrtcFeed implements CameraFeed {
  _WebrtcFeed(this._stream, this._renderer);
  final MediaStream _stream;
  final RTCVideoRenderer _renderer;

  @override
  Widget view({required bool mirror}) => RTCVideoView(
    _renderer,
    mirror: mirror,
    objectFit: RTCVideoViewObjectFit.RTCVideoViewObjectFitCover,
  );

  @override
  Future<void> stop() async {
    _renderer.srcObject = null;
    await _free(_stream);
    await _renderer.dispose();
  }

  static Future<void> _free(MediaStream stream) async {
    for (final track in stream.getTracks()) {
      await track.stop();
    }
    await stream.dispose();
  }
}

/// The camera's preview in the settings. It opens the camera only when asked, and gives
/// it up when asked to stop: when its part of the settings goes away, and before a call
/// opens the camera.
class CameraPreview extends ChangeNotifier {
  CameraPreview({this.open = openWebrtcCamera});

  /// How the camera is opened; tests give their own.
  OpenCamera open;

  CameraFeed? _feed;
  bool _opening = false;
  bool _failed = false;

  /// Counts starts and stops, so a camera that opens after a stop is closed at once.
  int _turn = 0;

  /// The picture, while it shows.
  CameraFeed? get feed => _feed;

  /// Opening or showing.
  bool get on => _opening || _feed != null;
  bool get opening => _opening;

  /// The last start could not open the camera.
  bool get failed => _failed;

  /// Why, when [failed] (see `cameraFailure`).
  Object? get failure => _failure;
  Object? _failure;

  /// The opening under way, if any.
  Future<void>? _starting;

  Future<void> start(String? id) {
    if (on) return Future.value();
    final starting = _starting = _start(id);
    return starting.whenComplete(() {
      if (_starting == starting) _starting = null;
    });
  }

  Future<void> _start(String? id) async {
    final turn = ++_turn;
    _opening = true;
    _failed = false;
    _failure = null;
    notifyListeners();
    final CameraFeed feed;
    try {
      feed = await open(id);
    } catch (e) {
      if (turn == _turn) {
        _opening = false;
        _failed = true;
        _failure = e;
        notifyListeners();
      }
      return;
    }
    if (turn != _turn) {
      // Stopped while it opened.
      await feed.stop();
      return;
    }
    _opening = false;
    _feed = feed;
    notifyListeners();
  }

  Future<void> stop() async {
    _turn++;
    final feed = _feed;
    final was = on;
    _feed = null;
    _opening = false;
    if (was) notifyListeners();
    await feed?.stop();
    // A camera still opening is closed before this returns, so a call can open it next.
    await _starting;
  }
}

/// The one preview the settings show; a call stops it before it opens the camera.
final cameraPreview = CameraPreview();

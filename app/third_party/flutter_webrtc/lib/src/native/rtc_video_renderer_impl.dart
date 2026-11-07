import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'package:webrtc_interface/webrtc_interface.dart';

import '../helper.dart';
import '../video_renderer_extension.dart' show AudioControl;
import 'event_errors.dart';
import 'utils.dart';

class RTCVideoRenderer extends ValueNotifier<RTCVideoValue>
    implements VideoRenderer, AudioControl {
  RTCVideoRenderer() : super(RTCVideoValue.empty);
  Completer? _initializing;
  int? _textureId;
  bool _disposed = false;
  MediaStream? _srcObject;
  StreamSubscription<dynamic>? _eventSubscription;

  /// Fogata: a renderer disposed while its texture is still being made lets go of that
  /// texture as soon as it comes, before it is ever published or listened to; otherwise
  /// nothing would ever let go of it.
  @override
  Future<void> initialize() async {
    if (_initializing != null) {
      await _initializing!.future;
      return;
    }
    final initializing = _initializing = Completer();
    try {
      final response = await WebRTC.invokeMethod('createVideoRenderer', {});
      final int? textureId = response['textureId'];
      if (_disposing != null) {
        // Nobody asked this initialize for a dispose: a failure to let go is noted here,
        // never thrown to whoever started the renderer.
        if (textureId != null) {
          try {
            await _releaseTexture(textureId);
          } catch (e) {
            print('letting go of a late renderer texture: $e');
          }
        }
        return;
      }
      _textureId = textureId;
      _eventSubscription = EventChannel('FlutterWebRTC/Texture$textureId')
          .receiveBroadcastStream()
          .listen(eventListener, onError: errorListener);
    } finally {
      initializing.complete(null);
    }
  }

  /// Lets go of the native side of texture [textureId].
  static Future<void> _releaseTexture(int textureId) async {
    try {
      await WebRTC.invokeMethod('videoRendererDispose', <String, dynamic>{
        'textureId': textureId,
      });
    } on PlatformException catch (e) {
      throw 'Failed to RTCVideoRenderer::dispose: ${e.message}';
    }
  }

  @override
  int get videoWidth => value.width.toInt();

  @override
  int get videoHeight => value.height.toInt();

  @override
  int? get textureId => _textureId;

  @override
  MediaStream? get srcObject => _srcObject;

  @override
  Function? onResize;

  @override
  Function? onFirstFrameRendered;

  /// Fogata: the width and height of a frame too large to draw, which the renderer
  /// dropped; told once for each track it is given.
  void Function(int width, int height)? onFrameTooLarge;

  @override
  set srcObject(MediaStream? stream) {
    if (_disposed) {
      throw 'Can\'t set srcObject: The RTCVideoRenderer is disposed';
    }
    if (textureId == null) throw 'Call initialize before setting the stream';
    _srcObject = stream;
    WebRTC.invokeMethod('videoRendererSetSrcObject', <String, dynamic>{
      'textureId': textureId,
      'streamId': stream?.id ?? '',
      'ownerTag': stream?.ownerTag ?? ''
    }).then((_) {
      // Fogata: freed while the change was on its way (a call ending as a stream
      // arrives): nothing is left to tell.
      if (_disposed) return;
      value = (stream == null)
          ? RTCVideoValue.empty
          : value.copyWith(renderVideo: renderVideo);
    }).catchError((e) {
      print('Got exception for RTCVideoRenderer::setSrcObject: ${e.message}');
    }, test: (e) => e is PlatformException);
  }

  Future<void> setSrcObject({MediaStream? stream, String? trackId}) async {
    if (_disposed) {
      throw 'Can\'t set srcObject: The RTCVideoRenderer is disposed';
    }
    if (_textureId == null) throw 'Call initialize before setting the stream';
    _srcObject = stream;
    var oldTextureId = _textureId;
    try {
      await WebRTC.invokeMethod('videoRendererSetSrcObject', <String, dynamic>{
        'textureId': _textureId,
        'streamId': stream?.id ?? '',
        'ownerTag': stream?.ownerTag ?? '',
        'trackId': trackId ?? '0'
      });
      // Fogata: as above.
      if (_disposed) return;
      value = (stream == null)
          ? RTCVideoValue.empty
          : value.copyWith(renderVideo: renderVideo);
    } on PlatformException catch (e) {
      throw 'Got exception for RTCVideoRenderer::setSrcObject: textureId $oldTextureId [disposed: $_disposed] with stream ${stream?.id}, error: ${e.message}';
    }
  }

  @override
  Future<void> dispose() => _disposing ??= _dispose();

  Future<void>? _disposing;

  Future<void> _dispose() async {
    if (_disposed) return;
    await _eventSubscription?.cancel();
    _eventSubscription = null;
    final textureId = _textureId;
    _textureId = null;
    _disposed = true;
    try {
      if (textureId != null) await _releaseTexture(textureId);
    } finally {
      super.dispose();
    }
  }

  void eventListener(dynamic event) {
    if (_disposed) return;
    final Map<dynamic, dynamic> map = event;
    switch (map['event']) {
      case 'didTextureChangeRotation':
        value =
            value.copyWith(rotation: map['rotation'], renderVideo: renderVideo);
        onResize?.call();
        break;
      case 'didTextureChangeVideoSize':
        value = value.copyWith(
            width: 0.0 + map['width'],
            height: 0.0 + map['height'],
            renderVideo: renderVideo);
        onResize?.call();
        break;
      case 'didFirstFrameRendered':
        value = value.copyWith(renderVideo: renderVideo);
        onFirstFrameRendered?.call();
        break;
      case 'fogataFrameTooLarge':
        onFrameTooLarge?.call(map['width'] as int, map['height'] as int);
        break;
    }
  }

  /// Fogata: an error of the event channel is reported ([onWebRtcEventError]) and the
  /// renderer fails, once: it stops drawing, listens to the channel no more and tells
  /// [onError]; its owner disposes it as usual (the call makes the picture again in a
  /// new one). It used to be rethrown from the listener, where nothing caught it.
  void errorListener(Object obj, [StackTrace? stack]) {
    reportWebRtcEventError('video renderer $_textureId', obj, stack);
    if (_failed || _disposed) return;
    _failed = true;
    value = RTCVideoValue.empty;
    _eventSubscription?.cancel().catchError((Object _) {});
    _eventSubscription = null;
    onError?.call(obj);
  }

  /// Whether its event channel failed: nothing is drawn, and it only waits to be disposed.
  bool get failed => _failed;
  bool _failed = false;

  /// Told once when the event channel fails.
  void Function(Object error)? onError;

  @override
  bool get renderVideo => _textureId != null && _srcObject != null;

  @override
  bool get muted => _srcObject?.getAudioTracks()[0].muted ?? true;

  @override
  set muted(bool mute) {
    if (_disposed) {
      throw Exception('Can\'t be muted: The RTCVideoRenderer is disposed');
    }
    if (_srcObject == null) {
      throw Exception('Can\'t be muted: The MediaStream is null');
    }
    if (_srcObject!.ownerTag != 'local') {
      throw Exception(
          'You\'re trying to mute a remote track, this is not supported');
    }
    if (_srcObject!.getAudioTracks().isEmpty) {
      throw Exception('Can\'t be muted: The MediaStreamTrack(audio) is empty');
    }

    Helper.setMicrophoneMute(mute, _srcObject!.getAudioTracks()[0]);
  }

  @override
  Future<bool> audioOutput(String deviceId) async {
    try {
      await Helper.selectAudioOutput(deviceId);
    } catch (e) {
      print('Helper.selectAudioOutput ${e.toString()}');
      return false;
    }
    return true;
  }

  @override
  Future<void> setVolume(double value) async {
    try {
      if (_srcObject == null) {
        throw Exception('Can\'t set volume: The MediaStream is null');
      }
      for (MediaStreamTrack track in _srcObject!.getAudioTracks()) {
        await Helper.setVolume(value, track);
      }
    } catch (e) {
      print('Helper.setVolume ${e.toString()}');
    }
  }
}

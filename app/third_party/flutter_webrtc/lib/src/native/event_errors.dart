import 'package:flutter/foundation.dart' show debugPrint;

/// Fogata: where the errors of the platform's event channels go. The package rethrew them
/// from a stream listener, where nothing could catch them and the resource went on as if
/// nothing had happened. Now each one is reported here, to whoever owns the call, and the
/// resource moves to a failed state of its own (see `RTCPeerConnectionNative.errorListener`
/// and `RTCVideoRenderer.errorListener`).
typedef WebRtcEventErrorCallback = void Function(
    String resource, Object error, StackTrace? stack);

/// Called for every error of an event channel, with what it was about ("peer connection
/// 3"). Set once by the owner of the calls; without it the error is printed.
WebRtcEventErrorCallback? onWebRtcEventError;

/// Reports [error] of [resource] to [onWebRtcEventError], or prints it.
void reportWebRtcEventError(String resource, Object error, StackTrace? stack) {
  final report = onWebRtcEventError;
  if (report == null) {
    debugPrint('WebRTC event channel of $resource failed: $error');
    return;
  }
  report(resource, error, stack);
}

import 'dart:async';

import 'package:flutter/services.dart';

import 'event_errors.dart';

/// Fogata: what an error on an event channel of the plugin becomes. It is reported (see
/// [reportWebRtcEventError]) and the events go on: thrown from a listener, it would only
/// reach the zone as an uncaught error, and a listener without a handler does the same.
void reportEventError(String channel, Object error) {
  reportWebRtcEventError(channel, error, null);
}

class FlutterWebRTCEventChannel {
  FlutterWebRTCEventChannel._internal() {
    EventChannel('FlutterWebRTC.Event')
        .receiveBroadcastStream()
        .listen(eventListener, onError: errorListener);
  }

  static final FlutterWebRTCEventChannel instance =
      FlutterWebRTCEventChannel._internal();

  final StreamController<Map<String, dynamic>> handleEvents =
      StreamController.broadcast();

  void eventListener(dynamic event) async {
    final Map<dynamic, dynamic> map = event;
    handleEvents.add(<String, dynamic>{map['event'] as String: map});
  }

  /// Fogata: handed on to every listener of [handleEvents], each of which logs it.
  void errorListener(Object obj) {
    handleEvents.addError(obj);
  }
}

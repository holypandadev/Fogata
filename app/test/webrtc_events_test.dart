// An error on the WebRTC plugin's events is logged, and the events go on: none reaches the
// zone as an uncaught error.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
// ignore: implementation_imports
import 'package:flutter_webrtc/src/native/media_stream_track_impl.dart';
// ignore: implementation_imports
import 'package:flutter_webrtc/src/native_logs_listener.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'an error on the plugin\'s events stops neither them nor the app',
    () async {
      MockStreamHandlerEventSink? events;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockStreamHandler(
            const EventChannel('FlutterWebRTC.Event'),
            MockStreamHandler.inline(onListen: (_, sink) => events = sink),
          );
      // Every listener of the plugin's own events.
      var changes = 0;
      navigator.mediaDevices.ondevicechange = (_) => changes++;
      desktopCapturer.onAdded;
      NativeLogsListener.instance;
      MediaStreamTrackNative('t', 'screen', 'video', true, 'local');
      await pumpEventQueue();
      expect(events, isNotNull);
      // An error from the native side, then an event.
      events!.error(code: 'broken', message: 'a native error');
      await pumpEventQueue();
      events!.success({'event': 'onDeviceChange'});
      await pumpEventQueue();
      expect(changes, 1);
    },
  );
}

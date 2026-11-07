import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:flutter_webrtc/src/native/rtc_peerconnection_impl.dart';

// An error of the video library's event channel used to be rethrown from the stream
// listener, where nothing caught it. Now it is reported and the resource fails, once.
void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  final calls = <String>[];
  final reported = <String>[];

  setUp(() {
    calls.clear();
    reported.clear();
    onWebRtcEventError = (what, error, stack) => reported.add('$what: $error');
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        calls.add(call.method);
        return <String, dynamic>{};
      },
    );
  });
  tearDown(() {
    onWebRtcEventError = null;
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  test(
    'a peer connection whose events fail is reported, failed and closed once',
    () async {
      final channel = EventChannel('FlutterWebRTC/peerConnectionEvent7');
      messenger.setMockStreamHandler(
        channel,
        MockStreamHandler.inline(
          onListen: (arguments, sink) {
            sink.error(code: 'broken', message: 'the platform lost it');
            sink.error(code: 'broken', message: 'again');
          },
        ),
      );
      final connection = RTCPeerConnectionNative('7', {});
      final states = <RTCPeerConnectionState>[];
      connection.onConnectionState = states.add;
      await Future<void>.delayed(const Duration(milliseconds: 50));
      // Both errors are reported; it fails and closes once.
      expect(reported.length, 2);
      expect(reported.first, startsWith('peer connection 7'));
      expect(states, [RTCPeerConnectionState.RTCPeerConnectionStateFailed]);
      expect(calls.where((c) => c == 'peerConnectionClose').length, 1);
      // The owner closing it as well does not close it again.
      await connection.close();
      expect(calls.where((c) => c == 'peerConnectionClose').length, 1);
    },
  );

  test(
    'a connection its owner closed does not fail when its channel errors late',
    () async {
      final channel = EventChannel('FlutterWebRTC/peerConnectionEvent8');
      MockStreamHandlerEventSink? events;
      messenger.setMockStreamHandler(
        channel,
        MockStreamHandler.inline(onListen: (arguments, sink) => events = sink),
      );
      final connection = RTCPeerConnectionNative('8', {});
      final states = <RTCPeerConnectionState>[];
      connection.onConnectionState = states.add;
      await Future<void>.delayed(const Duration(milliseconds: 50));
      await connection.close();
      // The platform reports an error while it tears the connection down.
      events!.error(code: 'broken', message: 'late');
      await Future<void>.delayed(const Duration(milliseconds: 50));
      expect(reported.length, 1);
      expect(states, isEmpty);
      expect(calls.where((c) => c == 'peerConnectionClose').length, 1);
    },
  );

  test(
    'a renderer whose events fail stops drawing, and tells its owner once',
    () async {
      final channel = EventChannel('FlutterWebRTC/Texture5');
      messenger.setMockStreamHandler(
        channel,
        MockStreamHandler.inline(
          onListen: (arguments, sink) {
            sink.error(code: 'broken', message: 'lost');
            sink.error(code: 'broken', message: 'again');
          },
        ),
      );
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Method'),
        (call) async {
          calls.add(call.method);
          return call.method == 'createVideoRenderer'
              ? <String, dynamic>{'textureId': 5}
              : null;
        },
      );
      final renderer = RTCVideoRenderer();
      final told = <Object>[];
      renderer.onError = told.add;
      await renderer.initialize();
      await Future<void>.delayed(const Duration(milliseconds: 50));
      // It listens no more after the first.
      expect(reported.length, 1);
      expect(renderer.failed, isTrue);
      expect(told.length, 1);
      // Disposed twice, it goes once.
      await Future.wait([renderer.dispose(), renderer.dispose()]);
      expect(calls.where((c) => c == 'videoRendererDispose').length, 1);
    },
  );

  test('a renderer disposed while its texture is being made lets go of it once it comes', () async {
    var made = 0;
    final released = <int>[];
    final answers = <Completer<Map<String, dynamic>>>[];
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        switch (call.method) {
          case 'createVideoRenderer':
            final answer = Completer<Map<String, dynamic>>();
            answers.add(answer);
            return answer.future;
          case 'videoRendererDispose':
            released.add((call.arguments as Map)['textureId'] as int);
        }
        return null;
      },
    );
    // A hundred renderers made and dropped at once, the native side answering late.
    final renderers = [for (var i = 0; i < 100; i++) RTCVideoRenderer()];
    final starting = [for (final r in renderers) r.initialize()];
    await pumpEventQueue();
    final disposing = [for (final r in renderers) r.dispose()];
    for (final answer in answers) {
      answer.complete({'textureId': 1000 + made++});
    }
    await Future.wait([...starting, ...disposing]);
    await pumpEventQueue();
    expect(made, 100);
    expect(released.toSet(), {for (var i = 0; i < 100; i++) 1000 + i});
    expect(released.length, 100);
    // None is published, and disposing again lets go of nothing more.
    expect(renderers.every((r) => r.textureId == null), isTrue);
    await Future.wait([for (final r in renderers) r.dispose()]);
    expect(released.length, 100);
  });

  test('a late texture that cannot be let go of is noted, not thrown from initialize', () async {
    final answer = Completer<Map<String, dynamic>>();
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        switch (call.method) {
          case 'createVideoRenderer':
            return answer.future;
          case 'videoRendererDispose':
            throw PlatformException(code: 'gone', message: 'no such texture');
        }
        return null;
      },
    );
    final renderer = RTCVideoRenderer();
    final starting = renderer.initialize();
    await pumpEventQueue();
    final disposing = renderer.dispose();
    answer.complete({'textureId': 7});
    await expectLater(starting, completes);
    await disposing;
    expect(renderer.textureId, isNull);
  });
}

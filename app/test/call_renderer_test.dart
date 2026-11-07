import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

class _Connection implements RTCPeerConnection {
  @override
  Function(RTCPeerConnectionState state)? onConnectionState;
  @override
  Function(RTCTrackEvent event)? onTrack;
  @override
  Function(RTCIceGatheringState state)? onIceGatheringState;
  @override
  Function(RTCIceConnectionState state)? onIceConnectionState;

  @override
  Future<void> close() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Stream implements MediaStream {
  @override
  String get id => 'stream';

  @override
  String get ownerTag => 'remote';

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Track implements MediaStreamTrack {
  @override
  String? get kind => 'video';

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transceiver implements RTCRtpTransceiver {
  _Transceiver(this.mid);
  @override
  final String mid;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

const me = 'f000000000000000';
const ana = 'a000000000000000';

// A renderer whose channel fails stops for good; the call makes its picture again, a few
// times, and then gives it up.
void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  final calls = <String>[];
  final disposed = <int>[];
  final events = <int, MockStreamHandlerEventSink>{};
  var textures = 0;

  setUp(() {
    calls.clear();
    disposed.clear();
    events.clear();
    textures = 0;
    onWebRtcEventError = (what, error, stack) {};
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        calls.add(call.method);
        if (call.method == 'createVideoRenderer') {
          final id = ++textures;
          messenger.setMockStreamHandler(
            EventChannel('FlutterWebRTC/Texture$id'),
            MockStreamHandler.inline(
              onListen: (arguments, sink) => events[id] = sink,
            ),
          );
          return <String, dynamic>{'textureId': id};
        }
        if (call.method == 'videoRendererDispose') {
          disposed.add((call.arguments as Map)['textureId'] as int);
        }
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

  Future<void> settle() =>
      Future<void>.delayed(const Duration(milliseconds: 50));

  Future<(Call, CallPeer, _Connection)> arrive({required int mid}) async {
    final call = Call(group: 'g', me: me);
    final connection = _Connection();
    final peer = CallPeer(ana)..connection = connection;
    call.peers[ana] = peer;
    // Wired as a connection of the call is.
    call.connectionsForTests = () async => connection;
    try {
      await call.handleSignal(ana, {
        't': 'offer',
        'sdp':
            'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 opus/48000/2\r\na=mid:0\r\n'
            'm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\na=mid:1\r\n'
            'm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\na=mid:2\r\n'
            'm=audio 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 opus/48000/2\r\na=mid:3\r\n',
        'n': 1,
      });
    } catch (_) {
      // The fake goes no further than making the connection.
    }
    expect(connection.onTrack, isNotNull);
    connection.onTrack!(
      RTCTrackEvent(
        streams: [_Stream()],
        track: _Track(),
        transceiver: _Transceiver('$mid'),
      ),
    );
    await settle();
    return (call, peer, connection);
  }

  test('a camera picture whose channel failed is made again', () async {
    final (call, peer, _) = await arrive(mid: 1);
    expect(peer.rendering, isTrue);
    final first = peer.camera;
    final firstTexture = first.textureId;
    expect(first.srcObject, isNotNull);
    events[first.textureId!]!.error(code: 'broken', message: 'lost');
    await settle();
    // A new renderer shows the same stream; the failed one was let go of.
    expect(peer.camera, isNot(same(first)));
    expect(peer.camera.failed, isFalse);
    expect(peer.camera.srcObject?.id, 'stream');
    expect(disposed, contains(firstTexture));
    // The one made again can fail too, and is made again.
    final second = peer.camera;
    events[second.textureId!]!.error(code: 'broken', message: 'lost again');
    await settle();
    expect(peer.camera, isNot(same(second)));
    await call.stop();
  });

  test(
    'a picture that keeps failing is given up, and no longer watched',
    () async {
      final (call, peer, _) = await arrive(mid: 2);
      call.watch(ana);
      expect(call.watching, ana);
      for (var i = 0; i < 3; i++) {
        events[peer.screen.textureId!]!.error(code: 'broken', message: 'lost');
        await settle();
        expect(call.watching, ana);
      }
      final last = peer.screen;
      events[last.textureId!]!.error(code: 'broken', message: 'for good');
      await settle();
      expect(peer.screen, same(last));
      expect(last.failed, isTrue);
      expect(call.watching, isNull);
      await call.stop();
    },
  );

  test(
    'a call that ended meanwhile does not keep the renderer made again',
    () async {
      final (call, peer, _) = await arrive(mid: 1);
      final failing = peer.camera;
      final made = textures;
      events[failing.textureId!]!.error(code: 'broken', message: 'lost');
      unawaited(call.stop());
      await settle();
      // Whatever was made after the call ended is let go of, and nothing is left over.
      expect(disposed.toSet(), {for (var i = 1; i <= textures; i++) i});
      expect(textures, greaterThanOrEqualTo(made));
    },
  );
}
